//! Media bullet parsing for tree-sitter parser
//!
//! This module handles parsing of media bullets (timestamp markers).
//! Media bullets mark time ranges in audio/video files: ·start_end·
//!
//! After grammar coarsening, `inline_bullet` and `media_url` are single token nodes
//! (not multi-child sequences). The shared `parse_bullet_text()` helper extracts
//! timestamps from the token text.
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

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ErrorVec, ParseError, Severity, SourceLocation,
};
use tree_sitter::Node;

/// True when a bullet time component is written with a leading zero
/// before another digit (`012`). A bare `0` is legal; CLAN CHECK calls
/// the leading-zero form an illegal time representation (CHECK 90).
fn has_leading_zero(component: &str) -> bool {
    component.len() > 1 && component.starts_with('0')
}

/// Build an E748 diagnostic for each bullet time component written with
/// a leading zero. The bullet still parses (its numeric value is
/// unambiguous); the diagnostics alone make the file invalid, mirroring
/// CHECK 90. Returned rather than sunk so both the `ErrorSink`-based
/// structured-bullet path and the `ErrorVec`-returning token path can
/// consume it.
fn leading_zero_errors(start_text: &str, end_text: &str, node: Node, source: &str) -> ErrorVec {
    let mut out = ErrorVec::new();
    for (component, which) in [(start_text, "start"), (end_text, "end")] {
        if has_leading_zero(component) {
            out.push(ParseError::new(
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
    out
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
        /// `"start"` or `"end"`, so the message can say which.
        which: &'static str,
    },
    /// The digits are there and do not fit a `u64`.
    TimeNotRepresentable {
        /// `"start"` or `"end"`.
        which: &'static str,
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
    let bullet_text = match node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(_) => node.kind(),
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
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Result<(u64, u64), BulletRejection> {
    if node.has_error() {
        return Err(BulletRejection::ContainsRecoveryNode);
    }
    let text = |field: &str| {
        node.child_by_field_name(field)
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
    };
    let start_text =
        text("start_time").ok_or(BulletRejection::TimeFieldAbsent { which: "start" })?;
    let end_text = text("end_time").ok_or(BulletRejection::TimeFieldAbsent { which: "end" })?;
    let start_ms: u64 = start_text
        .parse()
        .map_err(|_| BulletRejection::TimeNotRepresentable {
            which: "start",
            text: start_text.to_owned(),
        })?;
    let end_ms: u64 = end_text
        .parse()
        .map_err(|_| BulletRejection::TimeNotRepresentable {
            which: "end",
            text: end_text.to_owned(),
        })?;
    errors.report_vec(leading_zero_errors(start_text, end_text, node, source));
    Ok((start_ms, end_ms))
}
