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

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use talkbank_model::ErrorSink;

/// One forbidden character, located by byte offset in the input.
///
/// Built only by [`control_characters`], from the input itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ControlCharacter {
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
pub(crate) fn control_characters(input: &str) -> impl Iterator<Item = ControlCharacter> + '_ {
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
            return Some(ControlCharacter { offset, ch });
        }
    })
}

impl ControlCharacter {
    /// The E315 diagnostic for this character, spanning exactly it.
    pub(crate) fn into_diagnostic(self, input: &str) -> ParseError {
        let end = self.offset + self.ch.len_utf8();
        ParseError::new(
            ErrorCode::InvalidControlCharacter,
            Severity::Error,
            SourceLocation::from_offsets(self.offset, end),
            ErrorContext::new(input, self.offset..end, ""),
            format!(
                "Control character U+{:04X} is not allowed in CHAT (only TAB, line endings and the bullet delimiter U+0015 are)",
                self.ch as u32
            ),
        )
        .with_suggestion("Remove it, or replace it with the Unicode character that was meant")
    }
}

/// Report every forbidden control character in `input` to `errors`.
pub(crate) fn report_control_characters(input: &str, errors: &impl ErrorSink) {
    for found in control_characters(input) {
        errors.report(found.into_diagnostic(input));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use talkbank_model::ErrorCollector;

    fn document(lines: &str) -> String {
        format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n{lines}\n@End\n"
        )
    }

    fn codes_of(input: &str) -> Vec<(String, usize)> {
        let parser = TreeSitterParser::new().expect("parser");
        let errors = ErrorCollector::new();
        let _file = parser.parse_chat_file_streaming(input, &errors);
        errors
            .into_vec()
            .into_iter()
            .map(|e| (e.code.as_str().to_owned(), e.location.span.start as usize))
            .collect()
    }

    /// The delimiters CHAT itself uses are not control characters for this
    /// rule: a bullet, CRLF, and the CA underline pairs yield nothing.
    #[test]
    fn chat_delimiters_and_bullets_are_permitted() {
        let input =
            document("*CHI:\twe \u{2}\u{1}need\u{2}\u{2} to . \u{15}0_1000\u{15}\r\n%com:\tfine");
        assert_eq!(control_characters(&input).count(), 0);
        assert!(
            !codes_of(&input).iter().any(|(c, _)| c == "E315"),
            "{:?}",
            codes_of(&input)
        );
    }

    /// A control character is reported once, at its own offset, wherever it
    /// sits: a `%com` line and a header value parse as free text and used to
    /// pass silently.
    #[test]
    fn a_control_character_in_free_text_is_reported_at_its_offset() {
        let input = document("*CHI:\thello .\n%com:\tnote\u{1}here");
        let offset = input.find('\u{1}').expect("present");
        let found: Vec<ControlCharacter> = control_characters(&input).collect();
        assert_eq!(
            found,
            [ControlCharacter {
                offset,
                ch: '\u{1}'
            }]
        );
        let e315: Vec<usize> = codes_of(&input)
            .into_iter()
            .filter(|(c, _)| c == "E315")
            .map(|(_, at)| at)
            .collect();
        assert_eq!(e315, [offset]);

        let header = document("@Comment:\tnote\u{7f}here\n*CHI:\thello .");
        let at = header.find('\u{7f}').expect("present");
        assert_eq!(
            codes_of(&header)
                .into_iter()
                .filter(|(c, _)| c == "E315")
                .map(|(_, a)| a)
                .collect::<Vec<_>>(),
            [at]
        );
    }

    /// An attribute lead byte with any other code (CLAN italics) is not an
    /// underline marker: both characters are reported, each at its offset,
    /// and a lone lead byte is reported too.
    #[test]
    fn a_non_underline_attribute_pair_is_reported_character_by_character() {
        let input = document("*CHI:\t\u{2}\u{3}hey\u{2}\u{4} you .");
        let offsets: Vec<usize> = control_characters(&input).map(|c| c.offset).collect();
        let lead = input.find('\u{2}').expect("present");
        assert_eq!(offsets, [lead, lead + 1, lead + 5, lead + 6]);
        let lone = document("*CHI:\they\u{2} you .");
        assert_eq!(control_characters(&lone).count(), 1);
    }

    /// Inside a word the character also breaks the parse; E315 is still
    /// reported exactly once, by this rule, not by an ERROR-node classifier.
    #[test]
    fn a_control_character_inside_a_word_is_reported_once() {
        let input = document("*CHI:\tword\u{1}test .");
        let e315 = codes_of(&input).iter().filter(|(c, _)| c == "E315").count();
        assert_eq!(e315, 1);
    }
}
