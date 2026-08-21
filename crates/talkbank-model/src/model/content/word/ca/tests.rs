//! Tests for word-internal CA element and paired delimiter tokens.
//!
//! What is NOT here any more: the enum-to-glyph mapping. It is generated from
//! spec/symbols/symbol_registry.json, so a test asserting `PitchUp` renders as
//! `↑` only restates the record it was generated from. Two further tests, which
//! asserted the enums and the generated character arrays held the same set,
//! went with it: with one owner there is no second representation to compare.
//!
//! What survives is what a type cannot hold: that `write_chat` and the symbol
//! table agree, which is a roundtrip between two functions, and that the
//! generated symbols actually PARSE, which lives at the parser boundary in
//! talkbank-parser.

use super::*;
use crate::Span;
use crate::model::WriteChat;

/// Builds a CA element with default span metadata.
#[test]
fn test_ca_element_creation() {
    let elem = CAElement::new(CAElementType::PitchUp);
    assert_eq!(elem.element_type, CAElementType::PitchUp);
    assert_eq!(elem.span, None);
}

/// Preserves explicit span metadata on CA elements.
#[test]
fn test_ca_element_with_span() {
    let span = Span::new(0, 3);
    let elem = CAElement::new(CAElementType::PitchUp).with_span(span);
    assert_eq!(elem.span, Some(span));
}

/// Builds a CA delimiter with default span metadata.
#[test]
fn test_ca_delimiter_creation() {
    let delim = CADelimiter::new(CADelimiterType::Faster);
    assert_eq!(delim.delimiter_type, CADelimiterType::Faster);
    assert_eq!(delim.span, None);
}

/// Preserves explicit span metadata on CA delimiters.
#[test]
fn test_ca_delimiter_with_span() {
    let span = Span::new(5, 8);
    let delim = CADelimiter::new(CADelimiterType::Softer).with_span(span);
    assert_eq!(delim.span, Some(span));
}

/// Serializes CA elements to their CHAT glyph form.
#[test]
fn test_ca_element_write_chat() {
    let elem = CAElement::new(CAElementType::PitchUp);
    let mut output = String::new();
    let _ = elem.write_chat(&mut output);
    assert_eq!(output, "↑");
}

/// Serializes CA delimiters to their CHAT glyph form.
#[test]
fn test_ca_delimiter_write_chat() {
    let delim = CADelimiter::new(CADelimiterType::Faster);
    let mut output = String::new();
    let _ = delim.write_chat(&mut output);
    assert_eq!(output, "∆");
}
