//! Source-bound byte offsets and UTF-16 LSP positions.
//!
//! LSP defaults to UTF-16 code units. Default indexed and standalone conversions share
//! the same column calculation, clamp out-of-range positions to line/document
//! ends, and round positions inside an encoded character down to its boundary.
//! Exact projection instead refuses offsets that require rounding.

use tower_lsp::lsp_types::*;

mod lines;
pub(crate) use lines::protocol_lines;
use lines::{ProtocolLine, split_lines};

/// An index borrowing the exact source whose line starts it records.
///
/// Build once for many conversions; callers cannot pair it with another text.
pub struct LineIndex<'a> {
    text: &'a str,
    first: ProtocolLine<'a>,
    remaining: Vec<ProtocolLine<'a>>,
}

impl<'a> LineIndex<'a> {
    /// Build the source-bound index in O(n) time.
    pub fn new(text: &'a str) -> Self {
        let (first, remaining) = split_lines(text);
        Self {
            text,
            first,
            remaining: remaining.collect(),
        }
    }

    /// Find the line in O(log n), then count UTF-16 units up to the offset.
    pub fn offset_to_position(&self, offset: usize) -> Position {
        self.position_and_line(offset).0
    }

    /// Project an offset and its original line text through the same index.
    pub(crate) fn position_and_line(&self, offset: usize) -> (Position, &'a str) {
        let offset = boundary_offset(self.text, offset);
        let (line, content) = self.line_at(offset);
        (position_on_line(line, content, offset), content.text())
    }

    /// Project without rounding. Interior UTF-8/CRLF offsets and values outside
    /// the source or protocol integer space have no exact editor position.
    pub(crate) fn exact_position_and_line(&self, offset: usize) -> Option<(Position, &'a str)> {
        if !self.text.is_char_boundary(offset) {
            return None;
        }
        let (line, content) = self.line_at(offset);
        let prefix = content.text().get(..offset.checked_sub(content.start())?)?;
        Some((
            Position {
                line: u32::try_from(line).ok()?,
                character: u32::try_from(prefix.encode_utf16().count()).ok()?,
            },
            content.text(),
        ))
    }

    fn line_at(&self, offset: usize) -> (usize, ProtocolLine<'a>) {
        let line = self.byte_to_line(offset);
        let content = if line == 0 {
            self.first
        } else {
            self.remaining[line - 1]
        };
        (line, content)
    }

    /// Zero-based protocol line, including the final empty line after an EOL.
    pub(crate) fn byte_to_line(&self, offset: usize) -> usize {
        self.remaining
            .partition_point(|line| line.start() <= offset)
    }
}

/// Clamp a wire/model offset without ever slicing inside a UTF-8 character.
fn boundary_offset(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Offsets within an atomic line ending project to the preceding line end.
fn position_on_line(line: usize, content: ProtocolLine<'_>, offset: usize) -> Position {
    let prefix = &content.text()[..offset.min(content.end()) - content.start()];
    Position {
        // Saturate only if the document exceeds the protocol's integer space.
        line: u32::try_from(line).unwrap_or(u32::MAX),
        character: u32::try_from(prefix.encode_utf16().count()).unwrap_or(u32::MAX),
    }
}

/// Convert one byte offset without allocating an index. O(n) worst case.
pub fn offset_to_position(text: &str, offset: usize) -> Position {
    let offset = boundary_offset(text, offset);
    let (mut content, remaining) = split_lines(text);
    let mut number = 0;
    for line in remaining {
        if line.start() > offset {
            break;
        }
        number += 1;
        content = line;
    }
    position_on_line(number, content, offset)
}

/// Convert a UTF-16 LSP position to a UTF-8 byte boundary in one pass.
///
/// Columns beyond a line clamp before its LF/CRLF/CR terminator. A column between
/// surrogate units clamps to the character's start. Missing lines clamp to EOF.
pub fn position_to_offset(text: &str, position: Position) -> usize {
    match resolve_position(text, position) {
        ResolvedPosition::Boundary(offset)
        | ResolvedPosition::SplitCharacter(offset)
        | ResolvedPosition::MissingLine(offset) => offset,
    }
}

/// Resolution retains whether a byte boundary required lossy rounding.
enum ResolvedPosition {
    Boundary(usize),
    SplitCharacter(usize),
    MissingLine(usize),
}

/// Input edits may clamp past a line's end, but cannot split a character or
/// refer to a line absent from the source revision.
#[derive(Debug, thiserror::Error)]
pub(crate) enum EditPositionError {
    #[error("edit position {0:?} splits a UTF-16 character")]
    SplitCharacter(Position),
    #[error("edit position {0:?} names a nonexistent line")]
    MissingLine(Position),
}

pub(crate) fn edit_position_to_offset(
    text: &str,
    position: Position,
) -> Result<usize, EditPositionError> {
    match resolve_position(text, position) {
        ResolvedPosition::Boundary(offset) => Ok(offset),
        ResolvedPosition::SplitCharacter(_) => Err(EditPositionError::SplitCharacter(position)),
        ResolvedPosition::MissingLine(_) => Err(EditPositionError::MissingLine(position)),
    }
}

fn resolve_position(text: &str, position: Position) -> ResolvedPosition {
    for (line, content) in protocol_lines(text).enumerate() {
        if line == position.line as usize {
            let mut units = 0;
            for (byte, ch) in content.text().char_indices() {
                if units + ch.len_utf16() > position.character as usize {
                    let offset = content.start() + byte;
                    return if units == position.character as usize {
                        ResolvedPosition::Boundary(offset)
                    } else {
                        ResolvedPosition::SplitCharacter(offset)
                    };
                }
                units += ch.len_utf16();
            }
            return ResolvedPosition::Boundary(content.end());
        }
    }
    ResolvedPosition::MissingLine(text.len())
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
    let offset = position_to_offset(document, position);
    chat_file
        .utterance_containing(offset)
        .or_else(|| chat_file.utterance_containing(offset.saturating_sub(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_positions_preserve_line_end_clamping_without_surrogate_rounding() {
        for ending in ["\n", "\r\n", "\r"] {
            let source = format!("a😀{ending}b{ending}");
            for (column, offset) in [(0, 0), (1, 1), (3, 5), (4, 5), (u32::MAX, 5)] {
                assert_eq!(
                    edit_position_to_offset(&source, Position::new(0, column)).unwrap(),
                    offset
                );
            }
            assert!(matches!(
                edit_position_to_offset(&source, Position::new(0, 2)),
                Err(EditPositionError::SplitCharacter(_))
            ));
            assert_eq!(
                edit_position_to_offset(&source, Position::new(2, 0)).unwrap(),
                source.len()
            );
            assert!(matches!(
                edit_position_to_offset(&source, Position::new(3, 0)),
                Err(EditPositionError::MissingLine(_))
            ));
        }
    }

    #[test]
    fn line_index_and_standalone_offset_conversion_match() {
        for text in ["ab\ncd\n你z", "😀\r\n", "", "\n"] {
            let index = LineIndex::new(text);
            for offset in 0..=text.len() + 2 {
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
