//! Source-bound byte offsets and UTF-16 LSP positions.
//!
//! LSP defaults to UTF-16 code units. Indexed and standalone conversions share
//! the same column calculation, clamp out-of-range positions to line/document
//! ends, and round positions inside an encoded character down to its boundary.

use tower_lsp::lsp_types::*;

/// An index borrowing the exact source whose line starts it records.
///
/// Build once for many conversions; callers cannot pair it with another text.
pub struct LineIndex<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    /// Build the source-bound index in O(n) time.
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(i, byte)| (byte == b'\n').then_some(i + 1)),
        );
        Self { text, line_starts }
    }

    /// Find the line in O(log n), then count UTF-16 units up to the offset.
    pub fn offset_to_position(&self, offset: u32) -> Position {
        let offset = boundary_offset(self.text, offset);
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        position_on_line(self.text, line, self.line_starts[line], offset)
    }
}

/// Clamp a wire/model offset without ever slicing inside a UTF-8 character.
fn boundary_offset(text: &str, offset: u32) -> usize {
    let mut offset = (offset as usize).min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Both entry points use the same column convention, including CRLF ends.
fn position_on_line(text: &str, line: usize, start: usize, offset: usize) -> Position {
    let prefix = &text[start..offset];
    let prefix = prefix.strip_suffix('\r').unwrap_or(prefix);
    Position {
        // Saturate only if the document exceeds the protocol's integer space.
        line: u32::try_from(line).unwrap_or(u32::MAX),
        character: u32::try_from(prefix.encode_utf16().count()).unwrap_or(u32::MAX),
    }
}

/// Convert one byte offset without allocating an index. O(n) in the prefix.
pub fn offset_to_position(text: &str, offset: u32) -> Position {
    let offset = boundary_offset(text, offset);
    let mut line = 0;
    let mut start = 0;
    for (i, byte) in text.bytes().take(offset).enumerate() {
        if byte == b'\n' {
            line += 1;
            start = i + 1;
        }
    }
    position_on_line(text, line, start, offset)
}

/// Convert a UTF-16 LSP position to a UTF-8 byte boundary in one pass.
///
/// Columns beyond a line clamp before its CRLF/LF terminator. A column between
/// surrogate units clamps to the character's start. Missing lines clamp to EOF.
pub fn position_to_offset(text: &str, position: Position) -> usize {
    let mut start = 0;
    for (line, content) in text.split_inclusive('\n').enumerate() {
        if line == position.line as usize {
            let content = content.strip_suffix('\n').unwrap_or(content);
            let content = content.strip_suffix('\r').unwrap_or(content);
            let mut units = 0;
            for (byte, ch) in content.char_indices() {
                if units + ch.len_utf16() > position.character as usize {
                    return start + byte;
                }
                units += ch.len_utf16();
            }
            return start + content.len();
        }
        start += content.len();
    }
    text.len()
}

/// Finds the utterance at a cursor position.
///
/// The cursor may rest at an utterance's exclusive end (for example right
/// after the last character of a word), so the offset is tried as-is and
/// then one byte back. Containment itself lives in the model
/// (`ChatFile::utterance_containing`, half-open); this crate owns only the
/// position-to-offset conversion and this cursor-affinity fallback.
pub fn find_utterance_at_position<'a>(
    chat_file: &'a talkbank_model::model::ChatFile,
    position: Position,
    document: &str,
) -> Option<&'a talkbank_model::model::Utterance> {
    let offset = position_to_offset(document, position) as u32;
    chat_file
        .utterance_containing(offset)
        .or_else(|| chat_file.utterance_containing(offset.saturating_sub(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_and_standalone_offset_conversion_match() {
        for text in ["ab\ncd\n你z", "😀\r\n", "", "\n"] {
            let index = LineIndex::new(text);
            for offset in 0..=text.len() as u32 + 2 {
                let indexed = index.offset_to_position(offset);
                let standalone = offset_to_position(text, offset);
                assert_eq!(indexed, standalone, "text={text:?}, offset={offset}");
            }
        }
    }

    #[test]
    fn position_to_offset_handles_multibyte_characters() {
        let text = "a\n你z";
        let pos_after_chinese = Position {
            line: 1,
            character: 1,
        };
        let pos_after_z = Position {
            line: 1,
            character: 2,
        };

        assert_eq!(position_to_offset(text, pos_after_chinese), 5);
        assert_eq!(position_to_offset(text, pos_after_z), 6);
    }
}
