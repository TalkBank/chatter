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
    ErrorCode, ErrorContext, ErrorSink, ErrorVec, ParseError, Severity, SourceLocation, Span,
};
use crate::model::Bullet;
use tree_sitter::Node;

/// Bullet delimiter character (U+0015)
const BULLET_CHAR: char = '\u{15}';

/// Split bullet token text into its raw `(start, end)` digit components.
///
/// Input format: `\u{15}START_END\u{15}`. Returns `None` when the
/// delimiters or the `_` separator are missing; the components are NOT
/// yet validated as numbers (callers parse and, separately, check the
/// leading-zero representation rule, E748).
pub(crate) fn parse_bullet_components(text: &str) -> Option<(&str, &str)> {
    let inner = text.strip_prefix(BULLET_CHAR)?.strip_suffix(BULLET_CHAR)?;
    inner.split_once('_')
}

/// Parse bullet text content from a token node.
///
/// Input format: `\u{15}START_END\u{15}`, exactly digits on both
/// sides of the underscore, nothing else. Returns `None` for any
/// deviation (including a trailing `-` or any non-digit byte in
/// either timestamp).
pub(crate) fn parse_bullet_text(text: &str) -> Option<(u64, u64)> {
    let (start_str, end_str) = parse_bullet_components(text)?;
    let start_ms = start_str.parse::<u64>().ok()?;
    let end_ms = end_str.parse::<u64>().ok()?;
    Some((start_ms, end_ms))
}

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

/// Extract `(start_ms, end_ms)` from a structured `bullet` CST node.
///
/// The grammar's `bullet` rule has field names `start_time` and `end_time`.
/// Returns `None` if either field is missing, unparseable, OR if the
/// bullet node (or any descendant) carries a tree-sitter ERROR node
///, that latter case catches ill-formed bullets like the removed
/// `·\d+_\d+-·` skip marker, where the grammar reports ERROR on the
/// trailing `-` but the named fields still resolve. Without the
/// has_error gate we'd silently accept data that violates the
/// grammar.
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
) -> Option<(u64, u64)> {
    if node.has_error() {
        return None;
    }
    let start_text = node
        .child_by_field_name("start_time")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())?;
    let end_text = node
        .child_by_field_name("end_time")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())?;
    let start_ms: u64 = start_text.parse().ok()?;
    let end_ms: u64 = end_text.parse().ok()?;
    errors.report_vec(leading_zero_errors(start_text, end_text, node, source));
    Some((start_ms, end_ms))
}

/// Parse media_url node into Bullet
///
/// After grammar coarsening, `media_url` is a single token matching
/// `\u0015\d+_\d+-?\u0015`. We extract the node text and parse it
/// with `parse_bullet_text()`.
///
/// Format: ·start_end· or ·start_end-· (where · represents \u0015)
///
/// Returns: (Option<Bullet>, ErrorVec)
pub fn parse_media_bullet(node: Node, source: &str) -> (Option<Bullet>, ErrorVec) {
    let mut errors = ErrorVec::new();

    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(e) => {
            errors.push(ParseError::new(
                ErrorCode::InvalidMediaBullet,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!("UTF-8 decoding error in media bullet: {e}"),
            ));
            return (None, errors);
        }
    };

    let Some((start_ms, end_ms)) = parse_bullet_text(text) else {
        errors.push(ParseError::new(
            ErrorCode::InvalidMediaBullet,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), text),
            format!("Invalid media bullet: could not parse timestamps from '{text}'"),
        ));
        return (None, errors);
    };

    // parse_bullet_text succeeded, so the components are present; check
    // the leading-zero representation rule (E748, CHECK 90) on the raw
    // digit text while keeping the parsed bullet.
    if let Some((start_text, end_text)) = parse_bullet_components(text) {
        errors.extend(leading_zero_errors(start_text, end_text, node, source));
    }

    if start_ms == 0 && end_ms == 0 {
        errors.push(ParseError::new(
            ErrorCode::InvalidMediaBullet,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), text),
            "Invalid media bullet: could not parse timestamps (both start and end are 0)",
        ));
        return (None, errors);
    }

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let bullet = Bullet::new(start_ms, end_ms).with_span(span);
    (Some(bullet), errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bullet_text_normal() {
        assert_eq!(parse_bullet_text("\u{15}123_456\u{15}"), Some((123, 456)));
    }

    /// Legacy skip dash (deprecated 2026-03-31) is NOT accepted.
    /// The grammar rejects it; `parse_bullet_text` must as well.
    #[test]
    fn test_parse_bullet_text_legacy_skip_rejected() {
        assert_eq!(parse_bullet_text("\u{15}123_456-\u{15}"), None);
    }

    /// Tests parse bullet text invalid.
    #[test]
    fn test_parse_bullet_text_invalid() {
        assert_eq!(parse_bullet_text("not a bullet"), None);
        assert_eq!(parse_bullet_text("\u{15}abc_def\u{15}"), None);
        assert_eq!(parse_bullet_text("\u{15}123\u{15}"), None);
        assert_eq!(parse_bullet_text(""), None);
    }
}
