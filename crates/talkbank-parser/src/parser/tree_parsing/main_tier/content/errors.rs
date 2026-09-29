//! Error classification for main-tier word/content parse failures.
//!
//! This module upgrades generic tree-sitter error spans to domain-specific
//! error codes used by TalkBank diagnostics.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, ContentItemCaNoBreakLinkerChoice, SourceBindingError, SourceBound,
};
use crate::node_types::{CONTENT_ITEM, LINKER_QUICK_UPTAKE, TAB, WHITESPACES};
use crate::parser::tree_parsing::helpers::ReadableRecovery;
use crate::parser::tree_parsing::parser_helpers::find_child_by_kind;
use talkbank_model::chars::{LEFT_SINGLE_QUOTE, RIGHT_SINGLE_QUOTE};
use tree_sitter::Node;

/// Where in the main tier a recovery node was found, stated by the caller that
/// knows it.
///
/// # Why this is a parameter and not something we look up
///
/// A recovery node's KIND OF FAULT is a fact about the user's file. Which
/// classifier it reached used to be a fact about tree-sitter's recovery: the
/// same construct was named precisely when it landed in one position and
/// reported as generic "unparsable content" when it landed in another, and
/// nothing in any type said the two were the same question.
///
/// So when a generator fix changed where absorbed ERROR nodes are placed, six
/// error codes silently degraded to E316 with every gate green: each code still
/// had a passing test, because the same construct at a different position still
/// took the old route. The tests could not see it, because the thing that
/// changed was not any value they asserted on.
///
/// # What it does and does not guarantee
///
/// It fixes the ORDER in which classifications are tried, per region, in one
/// place. That is the whole claim, and it is worth being exact about, because
/// an earlier draft of this doc said a caller outside the body "cannot reach
/// word-level classification", which the code contradicts three lines into
/// [`classify_outside_body_recovery`]: word classification is the FALLBACK
/// there. A comment asserting an invariant the type does not carry is the tell
/// this module exists to remove, so it should not appear in the module's own
/// documentation.
///
/// What the enum does buy: a new region cannot be added without every match on
/// it failing to compile, and there is one place to read to learn what any
/// region does. What it cannot buy: it cannot stop a caller naming the WRONG
/// region, which is why each variant says exactly which node it lives under.
/// Displaced body sinks no longer accept this independent region choice:
/// their reporter derives both the sink and its context from a sealed typed
/// carrier. Raw slot-level recovery still chooses the region explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MainTierRegion {
    /// A direct child of `main_tier` that is not inside `tier_body`: the region
    /// holding the speaker prefix and anything recovery leaves beside it.
    /// Faults here are STRUCTURAL first, because a word cannot be a direct
    /// child of `main_tier`, so the line's SHAPE is the likelier fault.
    ///
    /// Named for the region rather than for the prefix: the classifier runs
    /// over every direct ERROR child, including material after the tab, and
    /// `SpeakerPrefix` read at a call site as "inside the prefix".
    OutsideBody,
    /// Inside `tier_body` or `contents`: spoken material, where word-level
    /// classification applies.
    Body,
}

/// Classify a recovery node found in `region`.
///
/// Single entry point, so the answer depends on the region the caller states
/// rather than on where recovery placed the node.
pub(crate) fn classify_main_tier_recovery(
    error_node: Node,
    source: &str,
    region: MainTierRegion,
) -> ParseError {
    match region {
        MainTierRegion::OutsideBody => classify_outside_body_recovery(error_node, source),
        MainTierRegion::Body => analyze_word_error(error_node, source),
    }
}

/// Classify a recovery node found OUTSIDE `tier_body`, as a direct child of
/// `main_tier`.
///
/// The tab after the speaker code is the tier DELIMITER, so a second one
/// mid-line (CLAN CHECK 132) breaks the line's structure and no word-level code
/// can say so. This was previously reported as a structural error only by
/// accident: a recovery node displaced `tier_body` into an `Unexpected` slot,
/// and the ordering complaint that produced happened to be true. With the
/// displacement fixed, `tier_body` is Present and correct, so the fault has to
/// be named directly rather than inferred from a broken shape.
///
/// The tab is found by looking for a `tab` CHILD of the recovery node, not by
/// searching its text: a tab inside a word or a comment is not this fault, and
/// a substring search cannot tell the difference.
fn classify_outside_body_recovery(error_node: Node, source: &str) -> ParseError {
    if find_child_by_kind(error_node, TAB).is_some() {
        return ParseError::new(
            ErrorCode::StructuralOrderError,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(source, error_node.start_byte()..error_node.end_byte(), "\t"),
            "Unexpected tab inside the main tier: the tab after the speaker code is the tier \
             delimiter, so a further tab breaks the line's structure"
                .to_string(),
        )
        .with_suggestion("Separate words with spaces; use the tab only after the speaker code");
    }

    analyze_word_error(error_node, source)
}

/// Classifies a word/content `ERROR` node into a specific `ParseError`.
///
/// Private on purpose. Reaching it means going through
/// [`classify_main_tier_recovery`] and naming a region, so the pre-fix
/// affordance (call the word classifier wherever you happen to be) no longer
/// exists. Leaving it reachable would have left the cheaper path the old one,
/// and the guarantee would hold only where someone remembered to opt in.
fn analyze_word_error(error_node: Node, source: &str) -> ParseError {
    match ReadableRecovery::admit(error_node, source) {
        Some(recovery) => analyze_readable_word_error(recovery),
        None => crate::parser::typed_cst::cst_failure_diagnostic(
            error_node,
            source,
            SourceBindingError::InvalidRange,
        ),
    }
}

fn analyze_readable_word_error(recovery: ReadableRecovery<'_, '_>) -> ParseError {
    let error_node = recovery.node();
    let source = recovery.source();
    let error_text = recovery.text();

    if let crate::parser::tree_parsing::parser_helpers::error_analysis::dedicated::QuotationDelimiterScan::Unbalanced(finding) =
        crate::parser::tree_parsing::parser_helpers::error_analysis::dedicated::scan_quotation_delimiters(error_node)
    {
        return finding.into_diagnostic(source);
    }

    // Invalid, redundant, or followed-by-text delimiters do not establish a
    // missing terminator. That rule belongs to validation of the typed main
    // tier. Recovery remains an error here without fabricating that state.
    // E316: Unparsable content (LOWEST PRIORITY fallback)
    // Use error_text with span 0..len to avoid span/source mismatch that causes OutOfBounds
    // This is safe because error_text is extracted from the ERROR node itself
    recovery
        .fragment_diagnostic(
            ErrorCode::UnparsableContent,
            format!("Unparsable content on main tier: '{}'", error_text),
        )
        .with_suggestion("Check CHAT format manual for valid syntax at this position")
}

/// E330 at a node that sits where a bracketed construct (angle group,
/// quotation, pho or sin group) expected a delimiter, its contents or its
/// annotations: the construct has lost its shape there. One owner for the
/// message-only reporter the four construct parsers had each written per
/// position.
pub(crate) fn report_tree_shape(bad: Node, message: String, source: &str, errors: &impl ErrorSink) {
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
        ErrorContext::new(source, bad.start_byte()..bad.end_byte(), ""),
        message,
    ));
}

/// Diagnose the grammar's dedicated illegal-curly-quote token, not a substring
/// of an otherwise malformed recovery region.
pub(crate) fn illegal_curly_quote_error(node: Node, source: &str) -> ParseError {
    ParseError::new(
        ErrorCode::IllegalCurlyQuote,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
        format!(
            "Curly single quotation mark ({LEFT_SINGLE_QUOTE}/{RIGHT_SINGLE_QUOTE}) is not \
             a legal word character; CHAT requires the ASCII apostrophe (')"
        ),
    )
    .with_suggestion("Replace the curly single quote with the ASCII apostrophe (')")
}

/// Builds the diagnostic for a linker parsed in content position:
/// normally E766, with one carve-out.
///
/// Linkers (`+"`, `++`, `+<`, `+^`, `+,`, `+≈`, `+≋`) connect an utterance
/// to the PREVIOUS one, so they are utterance-initial by definition; the
/// grammar deliberately parses a misplaced one into the CST (the
/// strict+catch-all pattern) so this rule can name and locate it instead
/// of leaving it to ERROR-node recovery's generic E316. `node` is exactly
/// the offending linker token.
///
/// **Carve-out (E233):** a `++` glued to items on BOTH sides (`un++do`)
/// is not a linker at all; it is a word run with an empty compound part.
/// The linker token only wins that lex because `++` outranks the
/// compound marker wherever the content position admits a linker, so
/// naming it "linker must be utterance-initial" would be the
/// message-does-not-match-input defect this rule exists to remove. The
/// glued shape keeps E233, matching the word-level
/// `check_compound_markers` rule in talkbank-model. This parser diagnostic
/// names the adjacent `++` spelling; model validation additionally recognizes
/// empty spoken parts containing only nonlexical markers. The re2c front end
/// still lexes the run as one word, so its E233 comes from the model rule.
/// Adjacency is judged at the enclosing `content_item` wrapper against
/// its non-whitespace siblings, the same span-adjacency mechanism as the
/// E764/E765 family.
pub(crate) fn misplaced_linker_error<'tree>(
    linker: SourceBound<'tree, '_, ContentItemCaNoBreakLinkerChoice<'tree>>,
) -> ParseError {
    let node = linker.raw_node();
    let source = linker.source();
    if node.kind() == LINKER_QUICK_UPTAKE && is_glued_both_sides(node) {
        return ParseError::new(
            ErrorCode::EmptyCompoundPart,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Compound marker '+' cannot have empty parts (++)".to_string(),
        )
        .with_suggestion("Remove one '+' or add content between compound markers");
    }

    // The source-bound choice already admitted this exact token's text.
    let linker_text = linker.text();
    ParseError::new(
        ErrorCode::LinkerNotUtteranceInitial,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
        format!(
            "Linker '{linker_text}' must be utterance-initial; it links this \
             utterance to the previous one and cannot follow content"
        ),
    )
    .with_suggestion(
        "Move the linker to the start of the utterance, or remove it if no link is intended",
    )
}

/// Whether the (wrapped) item containing `node` is directly glued, with
/// no intervening whitespace sibling, to a sibling item on each side.
fn is_glued_both_sides(node: Node) -> bool {
    // Judge adjacency at the `content_item` wrapper when present: the
    // wrapper is the sibling-level item inside `contents`.
    let item = match node.parent() {
        Some(parent) if parent.kind() == CONTENT_ITEM => parent,
        _ => node,
    };
    let glued_prev = item
        .prev_sibling()
        .is_some_and(|prev| prev.kind() != WHITESPACES && prev.end_byte() == item.start_byte());
    let glued_next = item
        .next_sibling()
        .is_some_and(|next| next.kind() != WHITESPACES && next.start_byte() == item.end_byte());
    glued_prev && glued_next
}

#[cfg(test)]
mod tests {
    #[test]
    fn real_recovery_text_refuses_incompatible_sources_before_classification() {
        use super::{ErrorCode, MainTierRegion, ReadableRecovery, classify_main_tier_recovery};
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../talkbank-parser-tests/tests/error_corpus/validation_errors/E312_2.cha"
        ));
        let parser = crate::TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let foreign = "é".repeat(source.len());
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = false;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            if !node.is_error() || source.get(node.byte_range()) != Some("[") {
                continue;
            }
            let readable = ReadableRecovery::admit(node, source).expect("real source range");
            assert_eq!(readable.text(), "[");
            assert_eq!(
                classify_main_tier_recovery(node, source, MainTierRegion::Body).code,
                ErrorCode::UnparsableContent
            );
            for incompatible in ["", foreign.as_str()] {
                assert!(ReadableRecovery::admit(node, incompatible).is_none());
                let error = classify_main_tier_recovery(node, incompatible, MainTierRegion::Body);
                assert_eq!(error.code, ErrorCode::InternalError);
                assert!(talkbank_model::CompletedDiagnostics::admit(vec![error]).is_err());
                let mut findings = Vec::new();
                crate::parser::tree_parsing::parser_helpers::collect_recovery_nodes(
                    node,
                    incompatible,
                    &mut findings,
                );
                assert_eq!(findings.len(), 1);
                assert_eq!(findings[0].code, ErrorCode::InternalError);
                assert!(talkbank_model::CompletedDiagnostics::admit(findings).is_err());
            }
            witnessed = true;
        }
        assert!(
            witnessed,
            "retained spec must supply a real one-byte recovery node"
        );
    }
}
