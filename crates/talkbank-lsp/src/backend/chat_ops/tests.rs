//! Unit tests for chat execute-command helpers.

use serde_json::json;

use super::format_bullet::handle_format_bullet_line;
use super::scoped_find::{test_build_matcher, test_find_matches_in_span, test_make_match};
use crate::backend::execute_commands::FormatBulletLineRequest;
use crate::backend::utils::LineIndex;

#[test]
/// `LineIndex` should translate byte offsets into zero-based line numbers.
fn line_index_works() {
    let text = "line0\nline1\nline2";
    let idx = LineIndex::new(text);
    assert_eq!(idx.byte_to_line(0), 0);
    assert_eq!(idx.byte_to_line(6), 1);
    assert_eq!(idx.byte_to_line(12), 2);
}

#[test]
/// Plain-text scoped-find should produce a match on the expected line and tier.
fn scoped_find_plain_text() {
    let matcher = test_build_matcher("hello", false).unwrap();
    let text = "*CHI:\thello world .\n%mor:\tn|hello n|world .\n";
    let hits = test_find_matches_in_span(text, 0, text.len(), &matcher);
    assert!(!hits.is_empty());
    let (offset, len) = hits[0];
    let m = test_make_match(offset, len, "main", "CHI", text);
    assert!(m.is_some());
    let (range, tier, speaker) = m.unwrap();
    assert_eq!(range.start.line, 0);
    assert_eq!(tier, "main");
    assert_eq!(speaker, "CHI");
}

#[test]
/// Regex scoped-find should return the expected match span.
fn scoped_find_regex() {
    let matcher = test_build_matcher(r"hel+o", true).unwrap();
    let hits = test_find_matches_in_span("hello world", 0, 11, &matcher);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0], (0, 5));
}

#[test]
/// Plain-text matching should be case-insensitive.
fn scoped_find_case_insensitive() {
    let matcher = test_build_matcher("HELLO", false).unwrap();
    let hits = test_find_matches_in_span("Hello World", 0, 11, &matcher);
    assert_eq!(hits.len(), 1);
}

#[test]
/// Queries with no hits should return an empty match list.
fn scoped_find_no_matches() {
    let matcher = test_build_matcher("zzzzz", false).unwrap();
    let hits = test_find_matches_in_span("hello world", 0, 11, &matcher);
    assert!(hits.is_empty());
}

#[test]
/// Empty spans should never produce matches.
fn scoped_find_empty_span() {
    let matcher = test_build_matcher("hello", false).unwrap();
    let hits = test_find_matches_in_span("hello", 5, 5, &matcher);
    assert!(hits.is_empty());
}

#[test]
/// Scoped-find should report repeated matches in left-to-right order.
fn scoped_find_multiple_matches() {
    let matcher = test_build_matcher("the", false).unwrap();
    let hits = test_find_matches_in_span("the cat and the dog", 0, 19, &matcher);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0], (0, 3));
    assert_eq!(hits[1], (12, 3));
}

#[test]
/// Search ranges should not leak beyond the end byte supplied to the matcher.
fn scoped_find_respects_span_boundary() {
    let matcher = test_build_matcher("world", false).unwrap();
    let hits = test_find_matches_in_span("hello world", 0, 6, &matcher);
    assert!(hits.is_empty());
}

#[test]
/// Invalid regex input should be surfaced as an error.
fn scoped_find_invalid_regex_returns_error() {
    let result = test_build_matcher("[invalid", true);
    assert!(result.is_err());
}

#[test]
/// Single-line documents should still be indexed correctly.
fn line_index_single_line() {
    let text = "no newlines here";
    let idx = LineIndex::new(text);
    assert_eq!(idx.byte_to_line(0), 0);
    assert_eq!(idx.byte_to_line(10), 0);
}

#[test]
/// BMP source characters occupy one UTF-16 unit even when UTF-8 uses more.
fn make_match_utf8_char_offset() {
    let text = "café hello\n";
    let m = test_make_match(6, 5, "main", "CHI", text);
    assert!(m.is_some());
    let (range, _, _) = m.unwrap();
    assert_eq!(range.start.line, 0);
    assert_eq!(range.start.character, 5);
    assert_eq!(range.end.line, 0);
    assert_eq!(range.end.character, 10);
}

#[test]
/// Bullet formatting should preserve the timestamp range and speaker prefix.
fn format_bullet_output() {
    let request = serde_json::from_value::<FormatBulletLineRequest>(json!({
        "prev_ms": 1000,
        "current_ms": 2500,
        "speaker": "CHI",
    }))
    .unwrap();
    let result = handle_format_bullet_line(&request).unwrap();
    let output = serde_json::to_string(&result).unwrap();
    assert!(output.contains("1000_2500"));
    assert!(output.contains("*CHI:\\t"));
}

#[test]
fn scoped_find_unicode_source_spans() {
    for (source, query, expected) in [
        ("éé", "é", vec![(0, 2), (2, 2)]),
        ("İ hello", "hello", vec![(3, 5)]),
        ("K", "k", vec![(0, 3)]),
        ("banana", "ana", vec![(1, 3), (3, 3)]),
        ("a.b", ".", vec![(1, 1)]),
        ("😀", "", vec![]),
    ] {
        let matcher = test_build_matcher(query, false).unwrap();
        assert_eq!(
            test_find_matches_in_span(source, 0, source.len(), &matcher),
            expected,
            "source={source:?}, query={query:?}"
        );
    }
}

#[test]
fn scoped_find_utf16_selection() {
    let text = "😀 café 😀";
    let result = test_make_match(11, 4, "main", "CHI", text).unwrap();
    assert_eq!(result.0.start, tower_lsp::lsp_types::Position::new(0, 8));
    assert_eq!(result.0.end, tower_lsp::lsp_types::Position::new(0, 10));
}

#[test]
fn scoped_find_explicit_regex_preserves_zero_width_matches() {
    let matcher = test_build_matcher("", true).unwrap();
    assert_eq!(
        test_find_matches_in_span("x😀", 0, 5, &matcher),
        vec![(0, 0), (1, 0), (5, 0)]
    );
}
