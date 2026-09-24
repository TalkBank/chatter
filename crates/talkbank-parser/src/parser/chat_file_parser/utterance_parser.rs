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
    AsRawNode, KindSlotValue, MainTierNode, NoChild, SourceBound, SourceField, SourceSlotView,
    UtteranceChild1Choice, UtteranceChild1ChoiceBoundView, UtteranceNode, XDependentTierNode,
    extract_x_dependent_tier,
};
use crate::model::{ParseHealth, ParseHealthTier, Utterance};
use crate::parser::tree_parsing::helpers::ReadableRecovery;
use crate::parser::tree_parsing::main_tier::structure::convert_main_tier_node;
use crate::parser::tree_parsing::parser_helpers::{
    analyze_readable_dependent_error, present, surface_displaced,
};
use talkbank_model::ParseOutcome;

/// Builds one `Utterance` from a CST utterance subtree and attaches dependent tiers.
///
/// The parser keeps going after local tier failures, reports every error through
/// `errors`, and records taint on `ParseHealth` so downstream alignment logic can
/// treat this utterance conservatively. The entry point consumes the document
/// owner's source association; callers cannot supply a separate source string.
/// Internal leaf adapters still receive this same source while their generated
/// field projections are migrated. Recovery states are not removed by binding.
pub fn parse_utterance_node<'tree>(
    typed: SourceBound<'tree, '_, UtteranceNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Utterance> {
    let input = typed.source();
    let mut utterance_builder: Option<UtteranceUnderConstruction> = None;
    let mut parse_health = ParseHealth::untainted();

    // Drive dispatch through the generated, exhaustive typed visitor instead of a
    // `node.kind()` hand-walk. `extract_utterance` exposes the utterance's two
    // child positions as typed `Positioned<NodeSlot<..>>`s: `child_0` is the
    // required `main_tier`, and `child_1` is the `dependent_tier` repeat (its
    // supertype already expanded to the concrete tier kinds, one `<Rule>Choice`
    // variant per tier). Every slot variant is handled explicitly so a recovery
    // node can never be silently dropped.
    let associated = typed.extract();
    let children = associated.children();

    // child_0: the main tier. It is processed FIRST so it is built before any
    // dependent tier is attached (the dependent-tier branch consumes the built
    // utterance via `utterance_builder.take()`). `Present` and `Missing` both
    // retain the same generated `MainTierNode` identity, with a separate
    // placeholder state when content is missing,
    // matching the pre-migration `kind() == MAIN_TIER` branch, which did not
    // distinguish a MISSING placeholder.
    match children.child_0.slot().known_or_placeholder() {
        // `Present` and `Placeholder` were two arms calling the same function on
        // two spellings of the same `main_tier`-kinded node, which is what the
        // comment above had to say in prose.
        KindSlotValue::Present(main_tier) | KindSlotValue::Placeholder(main_tier) => {
            utterance_builder =
                build_main_tier_from_node(main_tier, input, errors, &mut parse_health);
        }
        KindSlotValue::Error(error_node) => {
            // An ERROR at the main-tier position routes to the same recovery
            // analysis the old hand-walk ran for any ERROR utterance child.
            handle_utterance_error_node(error_node, input, errors, &mut parse_health);
        }
        KindSlotValue::Absent(NoChild) => {
            // No main-tier child at all: nothing to build. The utterance is
            // rejected below, matching the old loop which left
            // `utterance_builder == None` when no `main_tier` child appeared.
        }
    }

    // child_1: the dependent-tier repeat. Each tier attaches to the already-built
    // main tier in document order. Lookahead supplies a count independently
    // followed by extraction; that is not proof that every extracted element is
    // present. Retain every recovery state exposed by the generated slot type.
    for element in associated.field_child_1().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(tier_choice) => {
                // By value, in and out: there is no window in which the
                // utterance exists only inside this call.
                utterance_builder = attach_dependent_tier_child(
                    utterance_builder,
                    tier_choice,
                    errors,
                    &mut parse_health,
                );
            }
            // Existing malformed-tier fixtures recover at the document level,
            // not as MISSING repeat elements. That finite observation is not a
            // producer invariant. Retain conservative alignment taint here; the
            // whole-tree recovery backstop emits the E342 for the MISSING node.
            SourceSlotView::Missing(_) => {
                parse_health.taint_all_alignment_dependents();
            }
            SourceSlotView::Error(error_node) => {
                handle_utterance_error_node(
                    error_node.raw_node(),
                    error_node.source(),
                    errors,
                    &mut parse_health,
                );
            }
            SourceSlotView::Absent(NoChild) => {}
        }
    }

    // Surface the carrier's own `unexpected` sink (nodes that filled no grammar
    // position at all: neither `main_tier` nor a `dependent_tier` repeat
    // element) through the shared backstop-equivalent mapping, per the R2
    // migration template. Believed empty on current fixtures, which is a
    // statement about the fixtures rather than about the grammar: the identical
    // claim over `tier_body`'s sink turned out to be false the moment a
    // generator fix changed where recovery nodes land. Load-bearing
    // once the whole-tree backstop is deleted (Task D).
    surface_displaced(&children.unexpected, "utterance", input, errors);

    // The phase ends here: the construction wrapper is unwrapped exactly once,
    // where the finished utterance leaves this function.
    match utterance_builder {
        Some(utterance) => ParseOutcome::parsed(utterance.finish(parse_health)),
        None => ParseOutcome::rejected(),
    }
}

/// Recover a complete main tier whose enclosing utterance/line wrappers were
/// lost at document EOF. Reuse the normal builder and parse-health transition.
pub(super) fn parse_recovered_main_tier(
    main: MainTierNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Utterance> {
    let mut health = ParseHealth::untainted();
    match build_main_tier_from_node(main, input, errors, &mut health) {
        Some(utterance) => ParseOutcome::parsed(utterance.finish(health)),
        None => ParseOutcome::rejected(),
    }
}

/// Build the main tier from its CST node, and with it the only thing a
/// dependent tier can attach to.
///
/// This is the body of the pre-migration `kind() == MAIN_TIER` branch, unchanged:
/// it converts the node with [`convert_main_tier_node`], reports any errors, and
/// taints `Main` when conversion failed or produced errors. The internals of
/// `convert_main_tier_node` are migrated separately (Task 3b).
///
/// It RETURNS the utterance rather than seeding an out-parameter, so it is the
/// sole producer of [`UtteranceUnderConstruction`] and the build order is a
/// consequence of the signatures rather than of the order of two blocks.
fn build_main_tier_from_node(
    typed: MainTierNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
    parse_health: &mut ParseHealth,
) -> Option<UtteranceUnderConstruction> {
    let Some(readable) = ReadableMainTier::admit(typed, input) else {
        let node = typed.raw_node();
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(input, node.byte_range(), "main_tier"),
            "Main-tier node range is not a UTF-8 slice of the supplied source",
        ));
        parse_health.taint(ParseHealthTier::Main);
        return None;
    };
    build_readable_main_tier(readable, errors, parse_health)
}

/// Typed main tier with its checked line slice. This proves range compatibility,
/// not identity with the original tree source or validity of the utterance.
struct ReadableMainTier<'tree, 'source> {
    typed: MainTierNode<'tree>,
    source: &'source str,
    line: &'source str,
}

impl<'tree, 'source> ReadableMainTier<'tree, 'source> {
    fn admit(typed: MainTierNode<'tree>, source: &'source str) -> Option<Self> {
        let line = source.get(typed.raw_node().byte_range())?;
        Some(Self {
            typed,
            source,
            line,
        })
    }
}

fn build_readable_main_tier(
    readable: ReadableMainTier<'_, '_>,
    errors: &impl ErrorSink,
    parse_health: &mut ParseHealth,
) -> Option<UtteranceUnderConstruction> {
    let ReadableMainTier {
        typed,
        source,
        line,
    } = readable;
    let main_tier_errors = ErrorCollector::new();
    let main_tier = convert_main_tier_node(typed, source, line, &main_tier_errors);
    let main_tier_error_vec = main_tier_errors.into_vec();
    if has_actual_errors(&main_tier_error_vec) {
        parse_health.taint(ParseHealthTier::Main);
    }
    errors.report_all(main_tier_error_vec);
    match main_tier {
        Ok(main_tier) => Some(UtteranceUnderConstruction(Utterance::new(main_tier))),
        Err(_reported) => {
            parse_health.taint(ParseHealthTier::Main);
            None
        }
    }
}

/// An utterance whose MAIN TIER has been built.
///
/// Only [`build_readable_main_tier`] produces one, and it is the only thing a
/// dependent tier can attach to, so "attach a dependent tier before the main
/// tier exists" has no signature to travel through. That ordering used to be
/// stated in two prose paragraphs ("processed FIRST so it is built before any
/// dependent tier is attached", "only once a main tier has been built") and
/// pinned by a characterization test.
///
/// It is also why [`attach_dependent_tier_child`] takes the utterance BY VALUE
/// and returns it. The previous `&mut Option<Utterance>` was `take()`n and put
/// back around the attach, so `Option` carried two meanings at once, "no main
/// tier" and "currently borrowed out", and any early return added inside that
/// window would have dropped the utterance and rejected the whole line with no
/// diagnostic saying so.
#[derive(Debug)]
struct UtteranceUnderConstruction(Utterance);

impl UtteranceUnderConstruction {
    fn finish(mut self, health: ParseHealth) -> Utterance {
        self.0.parse_health = health.into_state();
        self.0
    }
}

/// Attach one dependent-tier to the in-progress utterance from its typed
/// [`UtteranceChild1Choice`] (the `dependent_tier` supertype already classified
/// into its concrete subtype by `extract_utterance`).
///
/// It maps the tier to its alignment domain for taint via the typed
/// [`parse_health_tier_for`], attaches it via the typed
/// [`parse_and_attach_dependent_tier`] (only once a main tier has been built),
/// and taints the matching alignment domain when the attach reported an
/// error.
fn attach_dependent_tier_child<'tree>(
    utterance: Option<UtteranceUnderConstruction>,
    choice: SourceField<'_, 'tree, '_, UtteranceChild1Choice<'tree>>,
    errors: &impl ErrorSink,
    parse_health: &mut ParseHealth,
) -> Option<UtteranceUnderConstruction> {
    let Some(choice) = crate::parser::typed_cst::read_source_field(choice, errors) else {
        parse_health.taint_all_alignment_dependents();
        return utterance;
    };
    let mut tier_had_parse_errors = false;
    let dependent_tier = parse_health_tier_for(choice);

    // The tier's own recovery nodes are reported by the typed dispatch below
    // (each tier kind's `report_tier_parse_error`, and the tier parser's own
    // diagnostics), and whatever no region reports, the whole-tree backstop
    // does. Until 2026-09-08 this function walked the tier's children first
    // and reported every ERROR and MISSING node itself, so a `%wor` line with
    // an unparsable word carried the same E316 twice, at the same span. With
    // no main tier to attach to, the backstop is the reporter.
    let utterance = utterance.map(|UtteranceUnderConstruction(utt)| {
        let tier_errors = ErrorCollector::new();
        let utt = parse_and_attach_dependent_tier(utt, choice, &tier_errors);
        let tier_error_vec = tier_errors.into_vec();
        if has_actual_errors(&tier_error_vec) {
            tier_had_parse_errors = true;
        }
        errors.report_all(tier_error_vec);
        UtteranceUnderConstruction(utt)
    });

    if tier_had_parse_errors {
        match dependent_tier {
            Some(tier) => parse_health.taint(tier),
            None => parse_health.taint_all_alignment_dependents(),
        }
    }

    utterance
}

/// Report an `ERROR` that sits directly under the utterance, at the main-tier
/// position or in the dependent-tier repeat.
///
/// Two kinds arrive here. A `%` line the grammar could not shape is reported
/// by the dependent-tier analyzer and taints the tier it names (or every
/// alignment tier when the label is unreadable). Anything else is reported
/// as unrecognized and taints the main tier. This handler used to scan the
/// ERROR's text for a bare `@` (E202) and an unclosed `[:` (E311) as well;
/// both are the content analyzer's (`main_tier/content/errors.rs`), which
/// reports them at the word where they occur before an ERROR could ever
/// reach this level, so those scans had been dead since the content analyzer
/// took them, and text-classifying an ERROR is the banned pattern besides.
fn handle_utterance_error_node(
    error_node: tree_sitter::Node,
    input: &str,
    errors: &impl ErrorSink,
    parse_health: &mut ParseHealth,
) {
    let Some(recovery) = ReadableRecovery::admit(error_node, input) else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(input, error_node.byte_range(), "utterance"),
            "Utterance recovery node range is not a UTF-8 slice of the supplied source",
        ));
        // No readable text exists to choose a tier. Do not invent a label or
        // certify any potentially affected alignment domain as clean.
        parse_health.taint(ParseHealthTier::Main);
        parse_health.taint_all_alignment_dependents();
        return;
    };
    handle_readable_utterance_error(recovery, errors, parse_health);
}

fn handle_readable_utterance_error(
    recovery: ReadableRecovery<'_, '_>,
    errors: &impl ErrorSink,
    parse_health: &mut ParseHealth,
) {
    let error_node = recovery.node();
    let input = recovery.source();
    let error_start = error_node.start_byte();
    let error_end = error_node.end_byte();
    let error_text = recovery.text();

    if matches!(error_text.chars().next(), Some('%')) {
        errors.report(analyze_readable_dependent_error(recovery, None));
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

/// Return `true` when at least one diagnostic has `Severity::Error`.
fn has_actual_errors(errors: &[ParseError]) -> bool {
    errors
        .iter()
        .any(|error| matches!(error.severity, Severity::Error))
}

/// Best-effort tier classification for malformed `%tier` text from `ERROR` nodes.
pub(super) fn classify_percent_error_text(text: &str) -> Option<ParseHealthTier> {
    RecoveryTierLabel::admit(text)?.alignment_domain()
}

/// Nonempty label after a leading percent sign, without claiming CHAT validity.
/// Unknown labels remain admitted but do not identify an alignment domain.
struct RecoveryTierLabel<'text>(&'text str);

impl<'text> RecoveryTierLabel<'text> {
    fn admit(text: &'text str) -> Option<Self> {
        let tail = text.strip_prefix('%')?;
        let label = tail
            .split_once([':', '\t', ' ', '\r', '\n'])
            .map_or(tail, |(label, _)| label);
        if label.is_empty() {
            None
        } else {
            Some(Self(label))
        }
    }

    fn alignment_domain(self) -> Option<ParseHealthTier> {
        match self.0 {
            "mor" => Some(ParseHealthTier::Mor),
            "gra" => Some(ParseHealthTier::Gra),
            "pho" => Some(ParseHealthTier::Pho),
            "mod" | "xmod" => Some(ParseHealthTier::Mod),
            "wor" => Some(ParseHealthTier::Wor),
            "sin" => Some(ParseHealthTier::Sin),
            _ => None,
        }
    }
}

/// Map a typed dependent-tier choice to its parse-health tier category.
///
/// Replaces the removed `classify_dependent_tier_node` `node.kind()` dispatch: the
/// tier arrives already classified as a [`UtteranceChild1Choice`] variant, so this
/// is an exhaustive typed match (no `_ =>`). Only the alignment-bearing tiers map
/// to a domain; every text / raw / unsupported tier returns `None`, exactly as the
/// pre-migration `_ => None` arm did. The `%x*` case still reads the label text to
/// route `%xmod` onto `Mod` (byte-identical to the removed code).
fn parse_health_tier_for<'tree>(
    choice: SourceBound<'tree, '_, UtteranceChild1Choice<'tree>>,
) -> Option<ParseHealthTier> {
    use UtteranceChild1ChoiceBoundView as C;
    match choice.view() {
        C::MorDependentTier(_) => Some(ParseHealthTier::Mor),
        C::GraDependentTier(_) => Some(ParseHealthTier::Gra),
        C::PhoDependentTier(_) => Some(ParseHealthTier::Pho),
        C::ModDependentTier(_) => Some(ParseHealthTier::Mod),
        C::WorDependentTier(_) => Some(ParseHealthTier::Wor),
        C::SinDependentTier(_) => Some(ParseHealthTier::Sin),
        C::XDependentTier(n) => classify_x_tier_label(n.node(), n.source()),
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
/// Only a generated Present `x_tier_prefix` can supply a label. Recovery and
/// unreadable ranges yield no specific alignment domain, preserving the caller's
/// conservative all-domain taint when attachment reports errors.
fn classify_x_tier_label(node: XDependentTierNode<'_>, input: &str) -> Option<ParseHealthTier> {
    let children = extract_x_dependent_tier(node);
    let prefix = present(children.child_0.slot())?;
    let text = input.get(prefix.raw_node().byte_range())?;
    (text == "%xmod").then_some(ParseHealthTier::Mod)
}

#[cfg(test)]
mod tests {
    use super::{classify_percent_error_text, classify_x_tier_label};
    use crate::TreeSitterParser;
    use crate::generated_traversal::{FromNodeKind, XDependentTierNode};
    use crate::model::ParseHealthTier;

    /// Direct recovery-boundary test using real ERROR nodes; this does not
    /// claim that the fixture routes through an utterance slot in production.
    #[test]
    fn unreadable_utterance_recovery_rejects_without_inventing_a_tier() {
        use super::{ParseHealth, handle_utterance_error_node};
        use crate::error::{ErrorCode, ErrorCollector};
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../talkbank-parser-tests/tests/error_corpus/validation_errors/E312_2.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = false;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            if !node.is_error() || source.get(node.byte_range()) != Some("[") {
                continue;
            }
            // The readable side of this same recovery boundary preserves its
            // utterance-specific diagnostic and taints only the main tier.
            let errors = ErrorCollector::new();
            let mut health = ParseHealth::untainted();
            handle_utterance_error_node(node, source, &errors, &mut health);
            let diagnostics = errors.into_vec();
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, ErrorCode::UnrecognizedUtteranceError);
            assert!(health.is_tier_tainted(ParseHealthTier::Main));
            assert!(!health.is_tier_tainted(ParseHealthTier::Mor));
            let split_utf8 = format!("{}é", "x".repeat(node.end_byte() - 1));
            for incompatible in ["", split_utf8.as_str()] {
                let errors = ErrorCollector::new();
                let mut health = ParseHealth::untainted();
                handle_utterance_error_node(node, incompatible, &errors, &mut health);
                let diagnostics = errors.into_vec();
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, ErrorCode::TreeParsingError);
                for tier in [
                    ParseHealthTier::Main,
                    ParseHealthTier::Mor,
                    ParseHealthTier::Gra,
                    ParseHealthTier::Pho,
                    ParseHealthTier::Mod,
                    ParseHealthTier::Wor,
                    ParseHealthTier::Sin,
                    ParseHealthTier::Modsyl,
                    ParseHealthTier::Phosyl,
                    ParseHealthTier::Phoaln,
                    ParseHealthTier::Xphoint,
                ] {
                    assert!(health.is_tier_tainted(tier));
                }
            }
            witnessed = true;
        }
        assert!(
            witnessed,
            "retained bracket fixture must supply a real ERROR"
        );
    }

    #[test]
    fn main_tier_construction_requires_readable_source_and_taints_rejection() {
        use super::{MainTierNode, ParseHealth, ReadableMainTier, build_main_tier_from_node};
        use crate::error::{ErrorCode, ErrorCollector};
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/linkers-multiple.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(typed) = MainTierNode::from_node(node) else {
                continue;
            };
            let readable = ReadableMainTier::admit(typed, source).expect("original line");
            assert_eq!(readable.line, &source[node.byte_range()]);
            let errors = ErrorCollector::new();
            let mut health = ParseHealth::untainted();
            assert!(build_main_tier_from_node(typed, source, &errors, &mut health).is_some());
            assert!(health.is_clean());
            assert!(errors.into_vec().is_empty());
            let split_utf8 = format!("{}é", "x".repeat(node.end_byte() - 1));
            for incompatible in ["", split_utf8.as_str()] {
                assert!(ReadableMainTier::admit(typed, incompatible).is_none());
                let errors = ErrorCollector::new();
                let mut health = ParseHealth::untainted();
                assert!(
                    build_main_tier_from_node(typed, incompatible, &errors, &mut health).is_none()
                );
                assert!(health.is_tier_tainted(ParseHealthTier::Main));
                let diagnostics = errors.into_vec();
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, ErrorCode::TreeParsingError);
            }
            witnessed += 1;
        }
        assert!(witnessed > 0);
    }

    #[test]
    fn typed_x_tier_labels_distinguish_mod_from_other_real_user_tiers() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/tiers/user-defined.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut mod_count = 0;
        let mut other_count = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(tier) = XDependentTierNode::from_node(node) else {
                continue;
            };
            let expected = if source[node.byte_range()].starts_with("%xmod:") {
                mod_count += 1;
                Some(ParseHealthTier::Mod)
            } else {
                other_count += 1;
                None
            };
            assert_eq!(classify_x_tier_label(tier, source), expected);
            assert_eq!(classify_x_tier_label(tier, ""), None);
        }
        assert_eq!(mod_count, 1);
        assert!(other_count > 0);
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

    #[test]
    fn recovery_label_admission_preserves_exact_delimiters_and_unknown_labels() {
        use super::RecoveryTierLabel;
        for (label, domain) in [
            ("mor", ParseHealthTier::Mor),
            ("gra", ParseHealthTier::Gra),
            ("pho", ParseHealthTier::Pho),
            ("mod", ParseHealthTier::Mod),
            ("xmod", ParseHealthTier::Mod),
            ("wor", ParseHealthTier::Wor),
            ("sin", ParseHealthTier::Sin),
        ] {
            for suffix in ["", ":body", "\tbody", " body", "\rbody", "\nbody"] {
                let text = format!("%{label}{suffix}");
                let admitted = RecoveryTierLabel::admit(&text).expect("nonempty label");
                assert_eq!(admitted.0, label);
                assert_eq!(admitted.alignment_domain(), Some(domain));
                assert_eq!(classify_percent_error_text(&text), Some(domain));
            }
        }
        for text in ["", "mor", "%", "%:", "%\t", "% ", "%\r", "%\n"] {
            assert!(RecoveryTierLabel::admit(text).is_none());
            assert_eq!(classify_percent_error_text(text), None);
        }
        // Do not broaden the delimiter vocabulary to Unicode whitespace or
        // infer an alignment tier from a prefix of an unknown label.
        for label in ["xfoo", "猫", "morning", "mor\u{a0}body", "mor\u{b}body"] {
            let text = format!("%{label}");
            let admitted = RecoveryTierLabel::admit(&text).expect("unknown label");
            assert_eq!(admitted.0, label);
            assert_eq!(admitted.alignment_domain(), None);
        }
    }
}
