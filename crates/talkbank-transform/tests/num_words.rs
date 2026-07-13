// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Boundary test for the extracted number-normalization utility.
//!
//! Any CHAT generator (the MICASE/SBCSAE converters, ASR pipelines, external
//! tools) that emits digit-bearing content needs to spell digits out so the
//! output satisfies E220. This is the general core of what was the batchalign
//! ASR post-processor's `expand_number`, now a chatter utility.

use talkbank_transform::num_words::expand_number;

#[test]
fn expands_digits_per_language() {
    // Cardinal expansion across the table languages.
    assert_eq!(expand_number("5", "eng"), "five");
    assert_eq!(expand_number("99", "eng"), "ninety-nine");
    assert_eq!(expand_number("5", "spa"), "cinco");

    // CJK via num2chinese.
    assert_eq!(expand_number("42", "zho"), "四十二");

    // Non-digit content passes through unchanged.
    assert_eq!(expand_number("hello", "eng"), "hello");

    // Currency and English ordinals (the converters feed English text that can
    // carry these forms).
    assert!(expand_number("$5", "eng").contains("dollars"));
    assert_eq!(expand_number("21st", "eng"), "twenty-first");

    // An unknown language leaves the digit unchanged (honest passthrough; the
    // validator's E220 catches it if that language forbids digits).
    assert_eq!(expand_number("42", "xxx"), "42");
}
