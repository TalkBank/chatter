//! Exact scalar equality is a downstream API contract, not a CHAT specimen.
//! Keep these controls in the separate public-boundary measurement family.

use std::fmt::Debug;
use talkbank_model::SemanticEq;

/// Every position names a distinct scalar. Checking the entire relation also
/// exercises reflexivity and both directions without deriving the oracle from
/// the implementation's own equality operation.
fn assert_distinct_scalar_relation<T: SemanticEq + Debug>(values: &[T]) {
    for (left_index, left) in values.iter().enumerate() {
        for (right_index, right) in values.iter().enumerate() {
            assert_eq!(
                left.semantic_eq(right),
                left_index == right_index,
                "{left:?} versus {right:?}",
            );
        }
    }
}

#[test]
fn char_equality_preserves_case_and_unicode_scalar_identity() {
    // NUL and noncharacters are legal Rust scalars, not claims of valid CHAT.
    // Case folding, accent removal or truncated code points would lose identity.
    assert_distinct_scalar_relation(&[
        '\0',
        'A',
        'a',
        'e',
        'é',
        '\u{100}',
        '\u{10000}',
        '\u{10FFFE}',
        '\u{10FFFF}',
    ]);
}

macro_rules! signed_scalar_contract {
    ($name:ident, $scalar:ty) => {
        #[test]
        fn $name() {
            // No subtraction, absolute value or narrowing: MIN must retain its
            // sign and full range, just as -1, zero, one and MAX retain theirs.
            assert_distinct_scalar_relation(&[<$scalar>::MIN, -1, 0, 1, <$scalar>::MAX]);
        }
    };
}

signed_scalar_contract!(i8_equality_preserves_sign_and_extrema, i8);
signed_scalar_contract!(i16_equality_preserves_sign_and_extrema, i16);
signed_scalar_contract!(i32_equality_preserves_sign_and_extrema, i32);
signed_scalar_contract!(i64_equality_preserves_sign_and_extrema, i64);
