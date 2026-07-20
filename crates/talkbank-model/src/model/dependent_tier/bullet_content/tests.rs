//! `%x...` bullet-content serialization tests.
//!
//! These tests protect roundtrip formatting of inline timing bullets, picture
//! markers, and continuation boundaries in free-form dependent-tier text.

use super::{BulletContent, BulletContentSegment};

/// Serializes plain text with no embedded bullets.
#[test]
fn test_plain_text() {
    let content = BulletContent::from_text("hello world");
    assert_eq!(content.to_chat_string(), "hello world");
}

/// Serializes mixed text and one embedded timing bullet.
///
/// Spaces are canonical delimiters that the model does not store inside text
/// runs (see `write_chat`): a text segment carries no leading/trailing
/// delimiter space, and the serializer re-inserts exactly one space between
/// consecutive content items. The input segments therefore hold no boundary
/// spaces; the canonical single-space output is unchanged.
#[test]
fn test_text_with_bullet() {
    let content = BulletContent::new(vec![
        BulletContentSegment::text("text before"),
        BulletContentSegment::bullet(1000, 2000),
        BulletContentSegment::text("text after"),
    ]);
    assert_eq!(
        content.to_chat_string(),
        "text before \u{0015}1000_2000\u{0015} text after"
    );
}

/// Serializes mixed text with multiple embedded timing bullets.
///
/// Text runs carry no boundary delimiter spaces (see `test_text_with_bullet`);
/// the serializer joins each content item with one canonical space.
#[test]
fn test_multiple_bullets() {
    let content = BulletContent::new(vec![
        BulletContentSegment::text("this is junk"),
        BulletContentSegment::bullet(2051689, 2052652),
        BulletContentSegment::text("and more"),
        BulletContentSegment::bullet(2062689, 2063652),
    ]);
    assert_eq!(
        content.to_chat_string(),
        "this is junk \u{0015}2051689_2052652\u{0015} and more \u{0015}2062689_2063652\u{0015}"
    );
}

/// Serializes `%pic` picture references inside bullet content.
///
/// The text run carries no trailing delimiter space; the serializer inserts
/// the single canonical space before the picture marker.
#[test]
fn test_picture_reference() {
    let content = BulletContent::new(vec![
        BulletContentSegment::text("see image:"),
        BulletContentSegment::picture("photo.jpg"),
    ]);
    assert_eq!(
        content.to_chat_string(),
        "see image: \u{0015}%pic:\"photo.jpg\"\u{0015}"
    );
}

/// Preserves continuation newlines and tab indentation markers.
#[test]
fn test_continuation_preserved() {
    let content = BulletContent::new(vec![
        BulletContentSegment::text("first line"),
        BulletContentSegment::continuation(),
        BulletContentSegment::text("second line"),
    ]);
    assert_eq!(content.to_chat_string(), "first line\n\tsecond line");
}

/// Recognizes empty bullet content payloads.
#[test]
fn test_empty() {
    let content = BulletContent::from_text("");
    assert!(content.is_empty());
}
