//! Parse utterance CST nodes into model utterances with parse-health tainting.
//!
//! This file is the bridge between raw utterance CST and `model::Utterance`.
//! It performs three critical tasks:
//! 1. Builds the main tier from CST.
//! 2. Dispatches each dependent tier to typed parsers.
//! 3. Marks `ParseHealth` taint when dependency-bearing tiers are malformed.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use super::dependent_tier_dispatch::parse_and_attach_dependent_tier;
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation,
};
use crate::generated_traversal::{
    AsRawNode, NodeSlot, UtteranceChild1Choice, UtteranceNode, extract_utterance,
};
use crate::model::{ParseHealth, ParseHealthTier, Utterance};
use crate::parser::TreeSitterParser;
use crate::parser::tree_parsing::main_tier::structure::convert_main_tier_node;
use crate::parser::tree_parsing::parser_helpers::{
    analyze_dependent_tier_error, check_for_errors_recursive, surface_unexpected,
};
use talkbank_model::ParseOutcome;

impl TreeSitterParser {
    /// Parse a CST utterance node into a model Utterance, streaming errors.
    pub fn parse_utterance_cst(
        &self,
        utt_node: tree_sitter::Node,
        input: &str,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        parse_utterance_node(utt_node, input, errors)
    }
}

/// Builds one `Utterance` from a CST utterance subtree and attaches dependent tiers.
///
/// The parser keeps going after local tier failures, reports every error through
/// `errors`, and records taint on `ParseHealth` so downstream alignment logic can
/// treat this utterance conservatively.
pub fn parse_utterance_node(
    utt_node: tree_sitter::Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Utterance> {
    let mut utterance_builder: Option<Utterance> = None;
    let mut parse_health = ParseHealth::default();

    // Drive dispatch through the generated, exhaustive typed visitor instead of a
    // `node.kind()` hand-walk. `extract_utterance` exposes the utterance's two
    // child positions as typed `Positioned<NodeSlot<..>>`s: `child_0` is the
    // required `main_tier`, and `child_1` is the `dependent_tier` repeat (its
    // supertype already expanded to the concrete tier kinds, one `<Rule>Choice`
    // variant per tier). Every slot variant is handled explicitly so a recovery
    // node can never be silently dropped.
    let children = extract_utterance(UtteranceNode(utt_node));

    // child_0: the main tier. It is processed FIRST so it is built before any
    // dependent tier is attached (the dependent-tier branch consumes the built
    // utterance via `utterance_builder.take()`). `Present` and `Missing` both
    // resolve to the same `main_tier`-kinded raw node (a typed wrapper for
    // `Present`, a bare `Node` for `Missing` under the NEW closed `NodeSlot`),
    // matching the pre-migration `kind() == MAIN_TIER` branch, which did not
    // distinguish a MISSING placeholder.
    match children.child_0.slot {
        NodeSlot::Present(main_tier) => {
            build_main_tier_from_node(
                main_tier.raw_node(),
                input,
                errors,
                &mut utterance_builder,
                &mut parse_health,
            );
        }
        NodeSlot::Missing(main_tier_node) => {
            build_main_tier_from_node(
                main_tier_node,
                input,
                errors,
                &mut utterance_builder,
                &mut parse_health,
            );
        }
        NodeSlot::Error(error_node) => {
            // An ERROR at the main-tier position routes to the same recovery
            // analysis the old hand-walk ran for any ERROR utterance child.
            handle_utterance_error_node(error_node, input, errors, &mut parse_health);
        }
        NodeSlot::Unexpected(node) => {
            report_unexpected_utterance_child(node, input, errors, &mut parse_health);
        }
        NodeSlot::Absent => {
            // No main-tier child at all: nothing to build. The utterance is
            // rejected below, matching the old loop which left
            // `utterance_builder == None` when no `main_tier` child appeared.
        }
    }

    // child_1: the dependent-tier repeat. Each tier attaches to the already-built
    // main tier in document order. `extract_utterance`'s repeat_split only ever
    // pushes `Present` / `Missing` / `Error` elements into this Vec; the
    // `Unexpected` / `Absent` arms below are unreachable-by-construction (a
    // non-matching child stops the repeat and is swept into the carrier's own
    // `unexpected` sink instead of becoming a Vec element) but are handled
    // explicitly so the match stays exhaustive without a silent `_`-drop.
    for element in children.child_1.slot {
        match element.slot {
            NodeSlot::Present(tier_choice) => {
                attach_dependent_tier_child(
                    tier_choice,
                    input,
                    errors,
                    &mut utterance_builder,
                    &mut parse_health,
                );
            }
            // A tree-sitter MISSING `dependent_tier` repeat element is unreachable:
            // `repeat(dependent_tier)` never forces a MISSING element, and a
            // malformed `%` line recovers as a TOP-LEVEL `ERROR` node (handled by
            // the document backstop), NOT as an utterance-internal repeat element
            // (verified via `tree-sitter parse`). The arm is kept for
            // exhaustiveness: taint the alignment domains defensively; the
            // whole-tree recovery backstop emits the E342 for the MISSING node.
            NodeSlot::Missing(_) => {
                parse_health.taint_all_alignment_dependents();
            }
            NodeSlot::Error(error_node) => {
                handle_utterance_error_node(error_node, input, errors, &mut parse_health);
            }
            NodeSlot::Unexpected(node) => {
                report_unexpected_utterance_child(node, input, errors, &mut parse_health);
            }
            NodeSlot::Absent => {}
        }
    }

    // Surface the carrier's own `unexpected` sink (nodes that filled no grammar
    // position at all: neither `main_tier` nor a `dependent_tier` repeat
    // element) through the shared backstop-equivalent mapping, per the R2
    // migration template. Empty on every fixture probed so far; load-bearing
    // once the whole-tree backstop is deleted (Task D).
    surface_unexpected(&children.unexpected, input, errors);

    if let Some(mut utterance) = utterance_builder {
        utterance.parse_health = parse_health.into_state();
        ParseOutcome::parsed(utterance)
    } else {
        ParseOutcome::rejected()
    }
}

/// Build the main tier from its CST node and seed `utterance_builder`.
///
/// This is the body of the pre-migration `kind() == MAIN_TIER` branch, unchanged:
/// it converts the node with [`convert_main_tier_node`], reports any errors,
/// taints `Main` when conversion failed or produced errors, and seeds the
/// utterance. The internals of `convert_main_tier_node` are migrated separately
/// (Task 3b).
fn build_main_tier_from_node(
    main_tier_node: tree_sitter::Node,
    input: &str,
    errors: &impl ErrorSink,
    utterance_builder: &mut Option<Utterance>,
    parse_health: &mut ParseHealth,
) {
    let line = &input[main_tier_node.start_byte()..main_tier_node.end_byte()];
    let main_tier_errors = ErrorCollector::new();
    let main_tier =
        convert_main_tier_node(main_tier_node, input, line, &main_tier_errors).into_option();
    let main_tier_error_vec = main_tier_errors.into_vec();
    if has_actual_errors(&main_tier_error_vec) {
        parse_health.taint(ParseHealthTier::Main);
    }
    errors.report_all(main_tier_error_vec);
    if let Some(main_tier) = main_tier {
        *utterance_builder = Some(Utterance::new(main_tier));
    } else {
        parse_health.taint(ParseHealthTier::Main);
    }
}

/// Attach one dependent-tier to the in-progress utterance from its typed
/// [`UtteranceChild1Choice`] (the `dependent_tier` supertype already classified
/// into its concrete subtype by `extract_utterance`).
///
/// It maps the tier to its alignment domain for taint via the typed
/// [`parse_health_tier_for`] (replacing the removed `classify_dependent_tier_node`
/// `node.kind()` dispatch), walks its children for parse errors, attaches it via
/// the now-typed [`parse_and_attach_dependent_tier`] (only once a main tier has
/// been built, via `utterance_builder.take()`), and taints the matching alignment
/// domain when the tier had parse errors. Behavior is byte-identical to the
/// pre-migration hand-walk.
fn attach_dependent_tier_child(
    choice: UtteranceChild1Choice,
    input: &str,
    errors: &impl ErrorSink,
    utterance_builder: &mut Option<Utterance>,
    parse_health: &mut ParseHealth,
) {
    let tier_node = choice.raw_node();
    let mut tier_had_parse_errors = false;
    let dependent_tier = parse_health_tier_for(&choice, input);

    let mut dep_cursor = tier_node.walk();
    for dep_child in tier_node.children(&mut dep_cursor) {
        if dep_child.is_error() {
            errors.report(analyze_dependent_tier_error(dep_child, input));
            tier_had_parse_errors = true;
        } else {
            // check_for_errors_recursive needs to be converted to use ErrorSink
            let mut temp_errors = Vec::new();
            check_for_errors_recursive(dep_child, input, &mut temp_errors);
            if has_actual_errors(&temp_errors) {
                tier_had_parse_errors = true;
            }
            errors.report_all(temp_errors);
        }
    }

    if let Some(mut utt) = utterance_builder.take() {
        let tier_errors = ErrorCollector::new();
        utt = parse_and_attach_dependent_tier(utt, choice, input, &tier_errors);
        let tier_error_vec = tier_errors.into_vec();
        if has_actual_errors(&tier_error_vec) {
            tier_had_parse_errors = true;
        }
        errors.report_all(tier_error_vec);
        *utterance_builder = Some(utt);
    }

    if tier_had_parse_errors {
        match dependent_tier {
            Some(tier) => parse_health.taint(tier),
            None => parse_health.taint_all_alignment_dependents(),
        }
    }
}

/// Run the recovery analysis for an `ERROR` utterance child.
///
/// This is the body of the pre-migration `utt_child.is_error()` branch, unchanged
/// byte-for-byte (the form-type / unclosed-replacement / `%`-tier / unrecognized
/// classification and the matching taint). Text-hacking removal is a separate
/// paused workstream; this dispatch only routes the typed `NodeSlot::Error` arms
/// (at the main-tier position and within the dependent-tier repeat) to it.
fn handle_utterance_error_node(
    error_node: tree_sitter::Node,
    input: &str,
    errors: &impl ErrorSink,
    parse_health: &mut ParseHealth,
) {
    let error_start = error_node.start_byte();
    let error_end = error_node.end_byte();
    let error_text = &input[error_start..error_end];

    if let Some(relative_at) = find_missing_form_type_offset(error_text) {
        let at_start = error_start + relative_at;
        let at_end = at_start + 1;
        errors.report(
            ParseError::new(
                ErrorCode::MissingFormType,
                Severity::Error,
                SourceLocation::from_offsets(at_start, at_end),
                ErrorContext::new(input, at_start..at_end, "@"),
                "Missing form type after @",
            )
            .with_suggestion("Add a form type after @ (e.g., @b for babbling)"),
        );
    } else if let Some((relative_start, relative_end)) =
        find_unclosed_replacement_offset(error_text)
    {
        let bracket_start = error_start + relative_start;
        let bracket_end = error_start + relative_end;
        errors.report(
            ParseError::new(
                ErrorCode::UnexpectedNode,
                Severity::Error,
                SourceLocation::from_offsets(bracket_start, bracket_end),
                ErrorContext::new(
                    input,
                    bracket_start..bracket_end,
                    &input[bracket_start..bracket_end],
                ),
                "Unclosed replacement bracket",
            )
            .with_suggestion("Close replacement brackets and provide replacement text"),
        );
    // NOTE (2026-06-25): the former `error_text.contains("[:]")` (empty
    // replacement) arm was removed here. An empty replacement `word [:]`
    // PARSES into a structured `replacement` node (zero-width body with a
    // MISSING word_segment); the typed replacement path emits E376 and the
    // MISSING slot emits E342. No utterance-level ERROR node ever carries
    // `[:]` text, so this scan was DEAD. Classifying ERROR-node text is the
    // banned anti-pattern (root CLAUDE.md "CST Traversal Rules").
    // Regression: crates/talkbank-parser/tests/e208_empty_replacement_regression.rs.
    } else if matches!(error_text.chars().next(), Some('%')) {
        errors.report(analyze_dependent_tier_error(error_node, input));
        match classify_percent_error_text(error_text) {
            Some(tier) => parse_health.taint(tier),
            None => parse_health.taint_all_alignment_dependents(),
        }
    } else {
        errors.report(ParseError::new(
            ErrorCode::UnrecognizedUtteranceError,
            Severity::Error,
            SourceLocation::from_offsets(error_start, error_end),
            ErrorContext::new(input, error_start..error_end, error_text),
            format!(
                "Unrecognized ERROR node in utterance: {}",
                match error_text.lines().next() {
                    Some(line) => line,
                    None => error_text,
                }
            ),
        ));
        parse_health.taint(ParseHealthTier::Main);
    }
}

/// Report an unexpected (non-`main_tier`, non-dependent-tier, non-`ERROR`)
/// utterance child.
///
/// This is the body of the pre-migration `else` branch, unchanged: it reports
/// [`ErrorCode::UnexpectedUtteranceChild`] at the node span and taints `Main`.
fn report_unexpected_utterance_child(
    node: tree_sitter::Node,
    input: &str,
    errors: &impl ErrorSink,
    parse_health: &mut ParseHealth,
) {
    errors.report(ParseError::new(
        ErrorCode::UnexpectedUtteranceChild,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(input, node.start_byte()..node.end_byte(), node.kind()),
        format!("Unexpected child '{}' in utterance", node.kind()),
    ));
    parse_health.taint(ParseHealthTier::Main);
}

/// Return `true` when at least one diagnostic has `Severity::Error`.
fn has_actual_errors(errors: &[ParseError]) -> bool {
    errors
        .iter()
        .any(|error| matches!(error.severity, Severity::Error))
}

/// Best-effort tier classification for malformed `%tier` text from `ERROR` nodes.
pub(super) fn classify_percent_error_text(text: &str) -> Option<ParseHealthTier> {
    match dependent_tier_label_bytes(text)? {
        b"mor" => Some(ParseHealthTier::Mor),
        b"gra" => Some(ParseHealthTier::Gra),
        b"pho" => Some(ParseHealthTier::Pho),
        b"mod" | b"xmod" => Some(ParseHealthTier::Mod),
        b"wor" => Some(ParseHealthTier::Wor),
        b"sin" => Some(ParseHealthTier::Sin),
        _ => None,
    }
}

/// Extract the raw tier label bytes after `%` (e.g. `%mor` -> `b"mor"`).
fn dependent_tier_label_bytes(text: &str) -> Option<&[u8]> {
    let bytes = text.as_bytes();
    if bytes.first().copied() != Some(b'%') {
        return None;
    }

    let mut end = 1usize;
    while end < bytes.len() {
        match bytes[end] {
            b':' | b'\t' | b' ' | b'\r' | b'\n' => break,
            _ => end += 1,
        }
    }

    if end == 1 {
        return None;
    }

    Some(&bytes[1..end])
}

/// Find the byte offset of an `@` marker that is missing its form-type suffix.
fn find_missing_form_type_offset(error_text: &str) -> Option<usize> {
    let bytes = error_text.as_bytes();

    for idx in 0..bytes.len() {
        if bytes[idx] != b'@' {
            continue;
        }

        let missing = match bytes.get(idx + 1).copied() {
            None => true,
            Some(next) if next.is_ascii_whitespace() => true,
            Some(b'.' | b',' | b';' | b'!' | b'?' | b')' | b']') => true,
            _ => false,
        };

        if missing {
            return Some(idx);
        }
    }

    None
}

/// Find the span of an unclosed `[:` replacement marker.
fn find_unclosed_replacement_offset(error_text: &str) -> Option<(usize, usize)> {
    let bytes = error_text.as_bytes();
    let mut idx = 0usize;

    while idx + 1 < bytes.len() {
        if bytes[idx] == b'[' && bytes[idx + 1] == b':' {
            let has_closing = bytes[idx + 2..].contains(&b']');
            if !has_closing {
                return Some((idx, idx + 2));
            }
        }
        idx += 1;
    }

    None
}

/// Map a typed dependent-tier choice to its parse-health tier category.
///
/// Replaces the removed `classify_dependent_tier_node` `node.kind()` dispatch: the
/// tier arrives already classified as a [`UtteranceChild1Choice`] variant, so this
/// is an exhaustive typed match (no `_ =>`). Only the alignment-bearing tiers map
/// to a domain; every text / raw / unsupported tier returns `None`, exactly as the
/// pre-migration `_ => None` arm did. The `%x*` case still reads the label text to
/// route `%xmod` onto `Mod` (byte-identical to the removed code).
fn parse_health_tier_for(choice: &UtteranceChild1Choice, input: &str) -> Option<ParseHealthTier> {
    use UtteranceChild1Choice as C;
    match choice {
        C::MorDependentTier(_) => Some(ParseHealthTier::Mor),
        C::GraDependentTier(_) => Some(ParseHealthTier::Gra),
        C::PhoDependentTier(_) => Some(ParseHealthTier::Pho),
        C::ModDependentTier(_) => Some(ParseHealthTier::Mod),
        C::WorDependentTier(_) => Some(ParseHealthTier::Wor),
        C::SinDependentTier(_) => Some(ParseHealthTier::Sin),
        C::XDependentTier(n) => classify_x_tier_label(n.raw_node(), input),
        // Text / raw / unsupported tiers do not map to an alignment domain
        // (the removed `classify_dependent_tier_node` returned `None` via `_`).
        C::ActDependentTier(_)
        | C::AddDependentTier(_)
        | C::AltDependentTier(_)
        | C::CodDependentTier(_)
        | C::CohDependentTier(_)
        | C::ComDependentTier(_)
        | C::DefDependentTier(_)
        | C::EngDependentTier(_)
        | C::ErrDependentTier(_)
        | C::ExpDependentTier(_)
        | C::FacDependentTier(_)
        | C::FloDependentTier(_)
        | C::GlsDependentTier(_)
        | C::GpxDependentTier(_)
        | C::IntDependentTier(_)
        | C::ModsylDependentTier(_)
        | C::OrtDependentTier(_)
        | C::ParDependentTier(_)
        | C::PhoalnDependentTier(_)
        | C::PhosylDependentTier(_)
        | C::SitDependentTier(_)
        | C::SpaDependentTier(_)
        | C::TimDependentTier(_)
        | C::UnsupportedDependentTier(_)
        | C::XphointDependentTier(_) => None,
    }
}

/// Classify `%x...` tiers that map onto known alignment tiers (currently `%xmod`).
///
/// Reads the `x_tier_prefix` label text (a positional read of the concrete tier's
/// first child, byte-identical to the pre-migration code); this is a text read to
/// recover the user label, not a `node.kind()` structural dispatch.
fn classify_x_tier_label(node: tree_sitter::Node, input: &str) -> Option<ParseHealthTier> {
    // x_tier_prefix is a single token like "%xmod", extract label by stripping "%x"
    let prefix_node = node.child(0u32)?;
    let prefix_text = prefix_node.utf8_text(input.as_bytes()).ok()?;
    let label = prefix_text.strip_prefix("%x")?;
    if label == "mod" {
        Some(ParseHealthTier::Mod)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_percent_error_text, find_missing_form_type_offset,
        find_unclosed_replacement_offset,
    };
    use crate::model::ParseHealthTier;

    #[test]
    fn missing_form_type_offset_detects_lone_at() {
        assert_eq!(find_missing_form_type_offset("hello @ world"), Some(6));
        assert_eq!(find_missing_form_type_offset("@"), Some(0));
        assert_eq!(find_missing_form_type_offset("hello@"), Some(5));
    }

    #[test]
    fn missing_form_type_offset_skips_valid_marker_prefixes() {
        assert_eq!(find_missing_form_type_offset("hello@s:eng"), None);
        assert_eq!(find_missing_form_type_offset("word@b"), None);
    }

    #[test]
    fn unclosed_replacement_offset_detects_open_bracket_without_close() {
        assert_eq!(
            find_unclosed_replacement_offset("hello [: world"),
            Some((6, 8))
        );
        assert_eq!(find_unclosed_replacement_offset("[:]"), None);
        assert_eq!(find_unclosed_replacement_offset("hello [: fixed]"), None);
    }

    #[test]
    fn classify_percent_error_text_accepts_malformed_labels_without_colon() {
        assert_eq!(
            classify_percent_error_text("%mor no_tab_separator"),
            Some(ParseHealthTier::Mor)
        );
        assert_eq!(
            classify_percent_error_text("%gra no_tab_separator"),
            Some(ParseHealthTier::Gra)
        );
        assert_eq!(
            classify_percent_error_text("%pho no_tab_separator"),
            Some(ParseHealthTier::Pho)
        );
        assert_eq!(
            classify_percent_error_text("%wor no_tab_separator"),
            Some(ParseHealthTier::Wor)
        );
        assert_eq!(
            classify_percent_error_text("%xmod no_tab_separator"),
            Some(ParseHealthTier::Mod)
        );
        assert_eq!(classify_percent_error_text("%xfoo no_tab_separator"), None);
    }
}
