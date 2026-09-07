// Out-of-bounds position conversion regression tests.

use crate::position_conversion::{assert_offset_to_position, assert_position_to_offset};

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
    for (offset, line, character) in [(1, 0, 0), (3, 0, 0), (4, 0, 2), (5, 0, 2), (6, 1, 0), (99, 1, 0)] {
        assert_offset_to_position(text, offset, line, character);
    }
    for (line, character, offset) in [(0, 1, 0), (0, 2, 4), (0, 99, 4), (1, 0, 6)] {
        assert_position_to_offset(text, line, character, offset);
    }
}
