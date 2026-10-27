// Out-of-bounds position conversion regression tests.

use crate::position_conversion::{assert_offset_to_position, assert_position_to_offset};

#[test]
fn protocol_line_endings_preserve_utf16_and_atomic_crlf() {
    let text = "a😀\rb\r\nc\n";
    let index = talkbank_lsp::backend::utils::LineIndex::new(text);
    for (offset, line, character) in [
        (0, 0, 0),
        (1, 0, 1),
        (3, 0, 1),
        (5, 0, 3),
        (6, 1, 0),
        (7, 1, 1),
        (8, 1, 1),
        (9, 2, 0),
        (10, 2, 1),
        (11, 3, 0),
        (99, 3, 0),
    ] {
        assert_offset_to_position(text, offset, line, character);
        assert_eq!(
            index.offset_to_position(offset),
            tower_lsp::lsp_types::Position::new(line, character)
        );
    }
    for (line, column, offset) in [
        (0, 2, 1),
        (0, 3, 5),
        (0, 99, 5),
        (1, 0, 6),
        (1, 99, 7),
        (2, 1, 10),
        (3, 0, 11),
        (99, 0, 11),
    ] {
        assert_position_to_offset(text, line, column, offset);
    }
}

/// Verify clamping when offsets extend beyond the document.
#[test]
fn test_offset_beyond_text() {
    let text = "hello";

    assert_offset_to_position(text, 5, 0, 5);
    assert_offset_to_position(text, 100, 0, 5);
}

/// Verify clamping when positions extend beyond the document.
#[test]
fn test_position_beyond_document() {
    let text = "line1\nline2";

    assert_position_to_offset(text, 10, 0, 11);
    assert_position_to_offset(text, 0, 100, 5);
}

/// Verify empty-document behavior.
#[test]
fn test_empty_document() {
    let text = "";

    assert_offset_to_position(text, 0, 0, 0);
    assert_position_to_offset(text, 0, 0, 0);
}

/// UTF-16 columns exclude line endings and never split UTF-8 characters.
#[test]
fn test_utf16_and_crlf_boundaries() {
    let text = "😀\r\n";
    for (offset, line, character) in [
        (1, 0, 0),
        (3, 0, 0),
        (4, 0, 2),
        (5, 0, 2),
        (6, 1, 0),
        (99, 1, 0),
    ] {
        assert_offset_to_position(text, offset, line, character);
    }
    for (line, character, offset) in [(0, 1, 0), (0, 2, 4), (0, 99, 4), (1, 0, 6)] {
        assert_position_to_offset(text, line, character, offset);
    }
}
