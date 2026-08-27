//! Parsing for base (non-group) main-tier content items.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
#![deny(clippy::wildcard_enum_match_arm)]

mod internal_bullet;
mod long_feature;
mod nonvocal;
mod other_spoken;
mod overlap_point;

// Re-export overlap_point parser for use in other modules
pub(crate) use overlap_point::parse_overlap_point;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use crate::node_types::{
    BULLET, FREECODE, LONG_FEATURE, NONVOCAL, NONWORD_WITH_OPTIONAL_ANNOTATIONS,
    OTHER_SPOKEN_EVENT, PAUSE_TOKEN, UNDERLINE_BEGIN, UNDERLINE_END,
    WORD_WITH_OPTIONAL_ANNOTATIONS,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::super::freecode::parse_freecode;
use super::nonword::parse_nonword_content;
use super::word::parse_word_content;
use crate::parser::tree_parsing::parser_helpers::{expect_child_at, parse_pause_node};

/// Which of `base_content_item`'s alternatives a child node is.
///
/// # INTERIM. The generated `BaseContentItemChoice` is the real answer
///
/// `generated_traversal::BaseContentItemChoice` is this enum, derived from
/// `grammar.json` and `node-types.json`, kept current by
/// `generated_traversal_is_current`, and paired with `extract_base_content_item`
/// returning a `NodeSlot` that also models Missing, Error and Unexpected. The
/// root `CLAUDE.md` calls hand-walking `node.kind()` BANNED and that traversal
/// mandatory; this file is one of the hold-outs.
///
/// **What this hand-written mirror buys, and it is only one thing:** the E340
/// trigger becomes a total function over a node kind, so it can be tested. What
/// it does NOT buy is protection from the grammar changing. A thirteenth
/// alternative would regenerate `node_types.rs` and `generated_traversal.rs`,
/// leave this enum untouched, and fire E340 at runtime exactly as before. An
/// earlier version of this comment claimed otherwise; the compile error is
/// guaranteed only for a new VARIANT here, which only a human writing one can
/// create. Migrating to the generated type is what makes the claim true, and it
/// changes which diagnostic an unexpected child reports, so it owes the corpus
/// differential and its own commit.
///
/// # Why an enum rather than the string match it replaced
///
/// The dispatch below is exhaustive, and the file denies
/// `clippy::wildcard_enum_match_arm`, so a future `_` arm is a compile error.
/// Neither was available over `&'static str` constants: that lint cannot see a
/// catch-all over strings, so the one place the parser admits "I do not
/// recognise this" was exempt from the lint written to find that shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaseContentKind {
    /// A word, with any annotations attached to it.
    Word,
    /// A pause token.
    Pause,
    /// A typed nonword, with any annotations attached to it.
    Nonword,
    /// A freecode.
    Freecode,
    /// A media bullet.
    Bullet,
    /// The opening underline marker.
    UnderlineBegin,
    /// The closing underline marker.
    UnderlineEnd,
    /// A scoped annotation (`&` long feature).
    LongFeature,
    /// A nonvocal event.
    Nonvocal,
    /// An other-spoken event.
    OtherSpokenEvent,
}

impl BaseContentKind {
    /// Classify a node kind, or `None` when the grammar produced something new.
    ///
    /// `None` is the E340 condition, and it is a real answer rather than a
    /// sentinel: the caller reports it and rejects, which is what an
    /// unexpected pathway owes.
    ///
    /// # Do not "optimize" the extra call away
    ///
    /// This is a hot path, so it was measured at the instruction level on
    /// 2026-08-20 rather than argued about. Splitting the classification out
    /// costs one `bl`/`ret`, a three-instruction niche-packed `None` test, and
    /// one jump-table branch, roughly 5 cycles per content item. The string
    /// comparisons still happen exactly ONCE: the enum match is a jump table
    /// over the discriminant, not a second comparison, and there is no
    /// `memcmp` and no allocation. Against that, `parse_base_content` shrank
    /// from 1167 to 549 instructions and its frame from 1104 to 1056 bytes, so
    /// icache and spill pressure moved the other way; the net is plausibly
    /// zero. For scale, `Node::kind()` is an out-of-line FFI call plus a
    /// `strlen`, once per item, in both shapes.
    ///
    /// `#[inline]` is the only lever, and it would restore the larger caller.
    /// An A/B validate run over 2,346 files cannot resolve the difference: the
    /// predicted effect is ~0.01%, two orders of magnitude under that
    /// benchmark's own standard error. Anyone proposing a change here owes a
    /// profile showing this function near the top of a real run, which it is
    /// not.
    fn from_node_kind(kind: &str) -> Option<Self> {
        match kind {
            WORD_WITH_OPTIONAL_ANNOTATIONS => Some(Self::Word),
            PAUSE_TOKEN => Some(Self::Pause),
            NONWORD_WITH_OPTIONAL_ANNOTATIONS => Some(Self::Nonword),
            FREECODE => Some(Self::Freecode),
            BULLET => Some(Self::Bullet),
            UNDERLINE_BEGIN => Some(Self::UnderlineBegin),
            UNDERLINE_END => Some(Self::UnderlineEnd),
            LONG_FEATURE => Some(Self::LongFeature),
            NONVOCAL => Some(Self::Nonvocal),
            OTHER_SPOKEN_EVENT => Some(Self::OtherSpokenEvent),
            _ => None,
        }
    }
}

/// Parse one `base_content_item` into `UtteranceContent`.
///
/// The base content choices cover words, pauses, separators, typed nonwords, media bullets, overlap points,
/// underline markers, and scoped annotations (`&` long features) as described in the Main Tier section of the
/// CHAT manual. This function enforces that exactly one expected child exists, dispatches to the dedicated parser,
/// and rejects unexpected nodes so the parser mirrors the grammar in `grammar.js`.
pub(crate) fn parse_base_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let child_count = node.child_count();

    // Position 0: require exactly one child (one of the choice alternatives)
    if child_count == 0 {
        return ParseOutcome::rejected();
    }

    // CRITICAL: Use expect_child_at to check for MISSING nodes - prevents fake objects
    if let ParseOutcome::Parsed(child) = expect_child_at(node, 0u32, source, errors, "base_content")
    {
        let Some(base_kind) = BaseContentKind::from_node_kind(child.kind()) else {
            // The grammar produced an alternative this parser does not know:
            // a grammar/parser mismatch, not a fault in the CHAT input.
            errors.report(
                ParseError::new(
                    ErrorCode::UnknownBaseContent,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!("Unknown base content type '{}'", child.kind()),
                )
                .with_suggestion("This may be a new grammar feature not yet supported"),
            );
            return ParseOutcome::rejected();
        };

        // Exhaustive: a new `BaseContentKind` variant fails to compile here
        // rather than reaching a catch-all at runtime.
        let content = match base_kind {
            BaseContentKind::Word => parse_word_content(child, source, errors),
            BaseContentKind::Pause => {
                // Parse pause using node kind dispatch
                parse_pause_node(child, source, errors).map(UtteranceContent::Pause)
            }
            BaseContentKind::Nonword => parse_nonword_content(child, source, errors),
            BaseContentKind::Freecode => parse_freecode(child, source, errors),
            BaseContentKind::Bullet => {
                internal_bullet::parse_internal_bullet(child, source, errors)
            }
            BaseContentKind::UnderlineBegin => {
                // Underline begin marker (\u0002\u0001)
                let span =
                    talkbank_model::Span::new(child.start_byte() as u32, child.end_byte() as u32);
                ParseOutcome::parsed(UtteranceContent::UnderlineBegin(
                    talkbank_model::UnderlineMarker::from_span(span),
                ))
            }
            BaseContentKind::UnderlineEnd => {
                // Underline end marker (\u0002\u0002)
                let span =
                    talkbank_model::Span::new(child.start_byte() as u32, child.end_byte() as u32);
                ParseOutcome::parsed(UtteranceContent::UnderlineEnd(
                    talkbank_model::UnderlineMarker::from_span(span),
                ))
            }
            BaseContentKind::LongFeature => long_feature::parse_long_feature(child, source, errors),
            BaseContentKind::Nonvocal => nonvocal::parse_nonvocal(child, source, errors),
            BaseContentKind::OtherSpokenEvent => {
                other_spoken::parse_other_spoken_event(child, source, errors)
            }
        };

        // Check for unexpected extra children
        if child_count > 1 {
            for idx in 1..child_count {
                if let Some(extra) = node.child(idx as u32) {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(extra.start_byte(), extra.end_byte()),
                        ErrorContext::new(source, extra.start_byte()..extra.end_byte(), ""),
                        format!(
                            "Unexpected extra child '{}' at position {} of base_content",
                            extra.kind(),
                            idx
                        ),
                    ));
                }
            }
        }

        return content;
    }

    ParseOutcome::rejected()
}

#[cfg(test)]
mod tests {
    // The parent module already imports every node-kind constant used here.
    use super::*;

    /// The E340 condition, and the reason this classification is a function.
    ///
    /// SURVIVES A TYPE, and says which kind it is: this reaches the OUTSIDE
    /// world in the sense that matters, the GENERATED grammar. No type of ours
    /// constrains which node kinds `parser.c` emits, so "the parser met a kind
    /// it does not know" cannot be made unrepresentable. It can only be
    /// detected and reported.
    ///
    /// This is what `spec/errors/E340.md` names as the out-of-corpus test
    /// its `unreachable_from_chat` status owes. Before the classification was
    /// split out of `parse_base_content`, covering it meant fabricating a
    /// `tree_sitter::Node` whose kind the grammar cannot emit, so
    /// `UnknownBaseContent` appeared exactly once in the whole tree: at the
    /// line that emits it.
    #[test]
    fn unknown_kind_is_not_recognised() {
        assert_eq!(BaseContentKind::from_node_kind("no_such_node_kind"), None);
        // A near miss, because the realistic failure is a grammar RENAME
        // rather than an invented string.
        assert_eq!(BaseContentKind::from_node_kind("word"), None);
    }

    /// The ten pairings, to catch a TRANSPOSED classification.
    ///
    /// **It does NOT prove the grammar is covered, and an earlier version of
    /// this test claimed it did.** It was named `every_grammar_alternative_is_
    /// recognised` over a hand-written table, which cannot notice the grammar
    /// gaining an alternative: the table would simply not mention it. It also
    /// carried two pairings the grammar does not declare, `separator` and
    /// `overlap_point`, which are dispatched by the CALLER
    /// (`structure/contents.rs`) and can never reach this function. So a test
    /// asserting grammar coverage was pinning dead code instead.
    ///
    /// What it does catch is real and worth a test: two arms of
    /// `from_node_kind` swapped, which no type prevents because every arm has
    /// the same shape.
    ///
    /// The honest grammar-coverage check is `generated_traversal_is_current`,
    /// which regenerates `BaseContentItemChoice` from `node-types.json`; this
    /// hand mirror is what that gate cannot see, and migrating to the generated
    /// enum is what would delete this test outright.
    #[test]
    fn classification_is_not_transposed() {
        let expected = [
            (WORD_WITH_OPTIONAL_ANNOTATIONS, BaseContentKind::Word),
            (PAUSE_TOKEN, BaseContentKind::Pause),
            (NONWORD_WITH_OPTIONAL_ANNOTATIONS, BaseContentKind::Nonword),
            (FREECODE, BaseContentKind::Freecode),
            (BULLET, BaseContentKind::Bullet),
            (UNDERLINE_BEGIN, BaseContentKind::UnderlineBegin),
            (UNDERLINE_END, BaseContentKind::UnderlineEnd),
            (LONG_FEATURE, BaseContentKind::LongFeature),
            (NONVOCAL, BaseContentKind::Nonvocal),
            (OTHER_SPOKEN_EVENT, BaseContentKind::OtherSpokenEvent),
        ];
        for (kind, want) in expected {
            assert_eq!(
                BaseContentKind::from_node_kind(kind),
                Some(want),
                "node kind {kind} lost its classification"
            );
        }
    }
}
