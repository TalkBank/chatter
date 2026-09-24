//! Media bullet parsing for tree-sitter parser
//!
//! This module handles parsing of media bullets (timestamp markers).
//! Media bullets mark time ranges in audio/video files: ·start_end·
//!
//! Structured bullets retain their generated timestamp fields. Every consumer
//! supplies a `BulletNode`; textual media references are a separate boundary.
//!
//! The grammar rule for `bullet` does not permit any character other
//! than the two timestamps and the `_` separator between them. A
//! trailing `-` (legacy "skip" marker, removed from the grammar
//! 2026-03-31) is treated as a parse error rather than silently
//! stripped, stripping hid real data corruption and defeated the
//! grammar's purpose.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, BulletNode, BulletTimestampNode, KindSlot, NoChild, SlotView, extract_bullet,
};
use tree_sitter::Node;

/// Closed timestamp roles shared by field admission and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BulletTime {
    Start,
    End,
}

impl std::fmt::Display for BulletTime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Start => "start",
            Self::End => "end",
        })
    }
}

/// An admitted leading-zero spelling, retaining its timestamp role.
/// A bare `0` cannot construct this diagnostic-producing value.
struct LeadingZeroTime<'text> {
    component: &'text str,
    which: BulletTime,
}

impl<'text> LeadingZeroTime<'text> {
    fn admit(component: &'text str, which: BulletTime) -> Option<Self> {
        (component.len() > 1 && component.starts_with('0')).then_some(Self { component, which })
    }

    fn report(self, node: Node, source: &str, errors: &impl ErrorSink) {
        let Self { component, which } = self;
        errors.report(ParseError::new(
            ErrorCode::LeadingZeroBulletTime,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), component),
            format!(
                "Bullet {which} time '{component}' has a leading zero; \
                     bullet times are plain millisecond integers"
            ),
        ));
    }
}

/// Why a structured `bullet` CST node could not be read as a pair of times.
///
/// # Four routes, and a diagnostic that guessed between them
///
/// This was the `None` of an `Option`, and the caller that reports E360 wrote
/// one sentence for all four: "grammar rejected '...'. Legal form: ·START_END·
/// with numeric timestamps only". For the overflow route both halves of that
/// are false, since the grammar ACCEPTED the bullet and its timestamps ARE
/// numeric; a first attempt to fix it said "numeric but too large", which is
/// false for the skip-marker route, where a characterization test caught it.
/// A message cannot be right about a fact the return type threw away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BulletRejection {
    /// The bullet or a descendant carries a tree-sitter ERROR node.
    ///
    /// Catches ill-formed bullets like the removed `·\d+_\d+-·` skip marker,
    /// where the grammar reports ERROR on the trailing `-` but the named
    /// fields still resolve. Without this gate the parser would silently
    /// accept data that violates the grammar.
    ContainsRecoveryNode,
    /// A named time field is absent, or its bytes are not UTF-8.
    TimeFieldAbsent {
        /// The grammar field whose text could not be admitted.
        which: BulletTime,
    },
    /// The digits are there and do not fit a `u64`.
    TimeNotRepresentable {
        /// The grammar field whose numeric value could not be admitted.
        which: BulletTime,
        /// The digits as written, for the message.
        text: String,
    },
}

impl BulletRejection {
    /// The sentence a reader gets, one per route.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::ContainsRecoveryNode => {
                "legal form is ·START_END·, two millisecond integers and nothing \
                 else"
                    .to_owned()
            }
            Self::TimeFieldAbsent { which } => format!("no readable {which} time"),
            Self::TimeNotRepresentable { which, text } => format!(
                "the {which} time '{text}' is numeric but too large to read as \
                 milliseconds"
            ),
        }
    }
}

/// Report E360 for a bullet that could not be read, saying which route.
///
/// # One reporter, because there were three
///
/// Every caller of [`parse_bullet_node_timestamps`] that cannot proceed
/// reports E360, and each wrote its own sentence: "Invalid media bullet:
/// grammar rejected '...'. Legal form: ·START_END· with numeric timestamps
/// only" in `ending.rs`, and "Invalid bullet: could not extract timestamps"
/// in the two others. The first was false for the only input that reaches it;
/// the second says nothing a reader can act on. A verb three callers each
/// spell for themselves is a type, and the reason lives here with the
/// [`BulletRejection`] that supplies it.
pub(crate) fn report_bullet_rejection(
    node: Node,
    source: &str,
    rejection: &BulletRejection,
    errors: &impl ErrorSink,
) {
    // The context carries the bullet's text where it can be read; bytes that
    // are not UTF-8 leave the node's kind as the context, a fact about the
    // node rather than a text the parser invented.
    let bullet_text = match source.get(node.byte_range()) {
        Some(text) => text,
        None => node.kind(),
    };
    errors.report(ParseError::new(
        ErrorCode::InvalidMediaBullet,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), bullet_text),
        format!("Invalid media bullet: {}", rejection.describe()),
    ));
}

/// Extract `(start_ms, end_ms)` from a structured `bullet` CST node.
///
/// The grammar's `bullet` rule has field names `start_time` and `end_time`.
///
/// Reports E748 (leading-zero time representation, CHECK 90) through
/// `errors` while still returning the parsed values: the numeric value
/// is unambiguous, so the bullet is kept and the diagnostic alone makes
/// the file invalid. Centralized here because every structured-bullet
/// consumer (main tier, `%wor`, endings, bullet content) flows through
/// this function.
pub(crate) fn parse_bullet_node_timestamps(
    typed: BulletNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Result<(u64, u64), BulletRejection> {
    let node = typed.raw_node();
    if node.has_error() {
        return Err(BulletRejection::ContainsRecoveryNode);
    }
    let children = extract_bullet(typed);
    crate::parser::tree_parsing::parser_helpers::surface_displaced(
        &children.unexpected,
        "bullet",
        source,
        errors,
    );
    // Keep the role and the generated field together at the admission point.
    // The producer retains recovery slots; kind admission alone is not validity.
    let text = |which: BulletTime| -> Result<&str, BulletRejection> {
        let slot: &KindSlot<'_, BulletTimestampNode<'_>> = match which {
            BulletTime::Start => children.start_time.slot(),
            BulletTime::End => children.end_time.slot(),
        };
        let timestamp = match slot.view() {
            SlotView::Present(timestamp) => timestamp,
            SlotView::Missing(_) | SlotView::Error(_) | SlotView::Absent(NoChild) => {
                return Err(BulletRejection::TimeFieldAbsent { which });
            }
        };
        source
            .get(timestamp.raw_node().byte_range())
            .ok_or(BulletRejection::TimeFieldAbsent { which })
    };
    let start_text = text(BulletTime::Start)?;
    let end_text = text(BulletTime::End)?;
    let start_ms: u64 = start_text
        .parse()
        .map_err(|_| BulletRejection::TimeNotRepresentable {
            which: BulletTime::Start,
            text: start_text.to_owned(),
        })?;
    let end_ms: u64 = end_text
        .parse()
        .map_err(|_| BulletRejection::TimeNotRepresentable {
            which: BulletTime::End,
            text: end_text.to_owned(),
        })?;
    // Report only after both numeric fields have been admitted, preserving
    // overflow precedence and start-before-end diagnostic ordering.
    for (component, which) in [(start_text, BulletTime::Start), (end_text, BulletTime::End)] {
        if let Some(spelling) = LeadingZeroTime::admit(component, which) {
            spelling.report(node, source, errors);
        }
    }
    Ok((start_ms, end_ms))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::error::ErrorCollector;
    use crate::generated_traversal::FromNodeKind;

    /// Real parsed bullets exercise both field/source admission failures;
    /// a wrong source must be refused, not indexed or replaced by zero times.
    #[test]
    fn timestamp_fields_reject_out_of_source_ranges() {
        let source = include_str!("../../../../../corpus/reference/content/media-bullets.cha");
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut checked = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(bullet) = BulletNode::from_node(node) else {
                continue;
            };
            let errors = ErrorCollector::new();
            assert!(parse_bullet_node_timestamps(bullet, source, &errors).is_ok());
            assert!(errors.to_vec().is_empty());
            assert_eq!(
                parse_bullet_node_timestamps(bullet, "", &errors),
                Err(BulletRejection::TimeFieldAbsent {
                    which: BulletTime::Start
                }),
            );
            let children = extract_bullet(bullet);
            let SlotView::Present(start) = children.start_time.slot().view() else {
                panic!("valid corpus bullet must expose its start field");
            };
            assert_eq!(
                parse_bullet_node_timestamps(
                    bullet,
                    &source[..start.raw_node().end_byte()],
                    &errors
                ),
                Err(BulletRejection::TimeFieldAbsent {
                    which: BulletTime::End
                }),
            );
            checked += 1;
        }
        assert!(checked > 0, "fixture must exercise structured bullets");
    }
}
