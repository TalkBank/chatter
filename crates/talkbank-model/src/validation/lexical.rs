//! Characters CHAT forbids anywhere in a file, found BEFORE any parse.
//!
//! A control character is not a construct: it has no place in a header, a
//! main tier or a dependent tier, so the rule is lexical and is decided once
//! over the whole input rather than wherever a parser happens to notice it.
//! Until 2026-09-03 it was decided in two ERROR-node classifiers instead, which
//! meant a control character reached E315 only when tree-sitter failed to
//! parse around it: inside a word it surfaced as generic E316, and inside a
//! `%com` line or a header value, which parse as free text, it was silently
//! accepted. Deciding it here makes every position equal.
//!
//! What is permitted is the closed set CHAT itself uses: TAB (the tier
//! delimiter), LF and CR (line endings), U+0015 (the bullet delimiter), and
//! the two CA underline markers, which are the attribute pairs U+0002 U+0001
//! (begin) and U+0002 U+0002 (end) that the grammar tokenizes. CLAN's other
//! attribute pairs (italics, U+0002 U+0003 and U+0002 U+0004) are not CHAT and
//! are reported here character by character, as CLAN CHECK's error 102 and
//! error 86 do for theirs.

use crate::ErrorSink;
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

/// One forbidden character, located by byte offset in the input.
///
/// Built only by [`control_characters`], from the input itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ControlCharacter<'source> {
    source: &'source str,
    offset: usize,
    ch: char,
}

/// The single control characters CHAT uses as delimiters, and nothing else.
const fn permitted_alone(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\r' | '\u{0015}')
}

/// The attribute lead byte and the two codes that make an underline marker
/// with it (`grammar.js`: `underline_begin`, `underline_end`).
const ATTRIBUTE_LEAD: char = '\u{0002}';
const fn underline_code(ch: char) -> bool {
    matches!(ch, '\u{0001}' | '\u{0002}')
}

/// Every control character in `input` that CHAT does not use, in order.
///
/// An underline pair is consumed as one unit, so neither of its characters
/// is reported; a lead byte followed by anything else is reported, and so
/// is whatever follows it, each at its own offset.
fn control_characters(input: &str) -> impl Iterator<Item = ControlCharacter<'_>> {
    let mut chars = input.char_indices().peekable();
    std::iter::from_fn(move || {
        loop {
            let (offset, ch) = chars.next()?;
            if !ch.is_control() || permitted_alone(ch) {
                continue;
            }
            if ch == ATTRIBUTE_LEAD
                && let Some((_, code)) = chars.peek().copied()
                && underline_code(code)
            {
                chars.next();
                continue;
            }
            return Some(ControlCharacter {
                source: input,
                offset,
                ch,
            });
        }
    })
}

impl ControlCharacter<'_> {
    /// The E315 diagnostic for this character, spanning exactly it.
    fn into_diagnostic(self) -> ParseError {
        let end = self.offset + self.ch.len_utf8();
        ParseError::new(
            ErrorCode::InvalidControlCharacter,
            Severity::Error,
            SourceLocation::from_offsets(self.offset, end),
            ErrorContext::new(self.source, self.offset..end, ""),
            format!(
                "Control character U+{:04X} is not allowed in CHAT (only TAB, line endings and the bullet delimiter U+0015 are)",
                self.ch as u32
            ),
        )
        .with_suggestion("Remove it, or replace it with the Unicode character that was meant")
    }
}

/// Report every forbidden control character in `input` to `errors`.
pub fn report_control_characters(input: &str, errors: &impl ErrorSink) {
    for found in control_characters(input) {
        errors.report(found.into_diagnostic());
    }
}
