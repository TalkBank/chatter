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

// Integration tests for the error_code_enum attribute macro.
//
// The macro generates serde/schemars derives, Display, as_str(), new(), and
// documentation_url(). We test the generated API surface here.

use talkbank_derive::error_code_enum;

// No hand-written `#[derive(PartialOrd, Ord)]` here: the macro emits both, and
// a second copy is a conflicting implementation rather than a harmless one. The
// hand-written pair used to sit on this line, unused by any test, which is how
// a fixture quietly records that somebody wanted an ordering the macro did not
// give them.
#[error_code_enum]
enum TestErrorCode {
    #[code("E001")]
    InternalError,
    #[code("E101")]
    InvalidFormat,
    #[code("E201")]
    MissingHeader,
    #[code("E999")]
    UnknownError,
}

// ---------------------------------------------------------------------------
// Task 5: error_code_enum tests (4 tests)
// ---------------------------------------------------------------------------

#[test]
fn as_str_returns_code_string() {
    assert_eq!(TestErrorCode::InternalError.as_str(), "E001");
    assert_eq!(TestErrorCode::InvalidFormat.as_str(), "E101");
    assert_eq!(TestErrorCode::MissingHeader.as_str(), "E201");
    assert_eq!(TestErrorCode::UnknownError.as_str(), "E999");
}

#[test]
fn new_parses_known_codes() {
    assert_eq!(TestErrorCode::new("E001"), TestErrorCode::InternalError);
    assert_eq!(TestErrorCode::new("E101"), TestErrorCode::InvalidFormat);
    assert_eq!(TestErrorCode::new("E201"), TestErrorCode::MissingHeader);
}

#[test]
fn new_returns_unknown_for_unrecognized_code() {
    assert_eq!(TestErrorCode::new("E000"), TestErrorCode::UnknownError);
    assert_eq!(TestErrorCode::new("ZZZZ"), TestErrorCode::UnknownError);
    assert_eq!(TestErrorCode::new(""), TestErrorCode::UnknownError);
}

#[test]
fn display_shows_code() {
    assert_eq!(format!("{}", TestErrorCode::InternalError), "E001");
    assert_eq!(format!("{}", TestErrorCode::MissingHeader), "E201");
    assert_eq!(format!("{}", TestErrorCode::UnknownError), "E999");
}

/// A fixture that ascends, including across a digit-count boundary.
///
/// `E1000` is declared AFTER `E999`, which the macro accepts only because it
/// orders by the parsed NUMBER. A string comparison would call `"E1000"` less
/// than `"E999"` and reject this enum, so the fixture pins that the key is
/// numeric rather than lexicographic. No real code has four digits yet, which
/// is exactly why the fixture must.
#[error_code_enum]
enum OrderedCode {
    #[code("E100")]
    Earlier,
    #[code("E500")]
    Later,
    #[code("E999")]
    UnknownError,
    #[code("E1000")]
    FourDigit,
    #[code("W108")]
    Warning,
}

/// SURVIVES: behaviour a signature cannot describe. `Ord` promises only that
/// SOME total order exists; that it is the order of the CODE is what a caller
/// printing a `BTreeSet<ErrorCode>` relies on.
///
/// This used to be a test that declaration order and code order DIFFER, over a
/// deliberately scrambled fixture. It is gone because the macro now refuses to
/// expand a descending declaration, so the two orders cannot differ: the type
/// obsoleted the test. Verified by watching it fail, which is how the fixture
/// above came to be ascending at all:
///
/// ```text
/// error: error codes must be declared in ascending order: E100 follows E500.
///        Declaration order IS the sort order (see the `Ord` derive), so move
///        this variant rather than relaxing the rule.
/// ```
#[test]
fn ordering_follows_the_code() {
    use std::collections::BTreeSet;

    let codes: BTreeSet<OrderedCode> = [
        OrderedCode::Warning,
        OrderedCode::Later,
        OrderedCode::UnknownError,
        OrderedCode::FourDigit,
        OrderedCode::Earlier,
    ]
    .into_iter()
    .collect();

    let ordered: Vec<&str> = codes.iter().map(OrderedCode::as_str).collect();
    assert_eq!(ordered, ["E100", "E500", "E999", "E1000", "W108"]);
}

/// `Ord` must agree with the derived `Eq`. Both now come from declaration
/// order, and the macro's ascending check rejects a duplicate code (equal keys
/// are not ascending), so this holds by construction rather than by luck.
#[test]
fn ordering_is_consistent_with_equality() {
    for left in OrderedCode::all() {
        for right in OrderedCode::all() {
            assert_eq!(
                left.cmp(right) == std::cmp::Ordering::Equal,
                left == right,
                "cmp and eq disagree for {left} and {right}"
            );
        }
    }
}
