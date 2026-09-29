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

// Integration tests for the SemanticEq derive macro.
//
// The macro generates `impl crate::model::SemanticEq`, so we must bring
// `talkbank_model::model` into the crate root so that `crate::model::*`
// resolves correctly in the generated code.

use talkbank_derive::SemanticEq as DeriveSemanticEq;
use talkbank_model::Span;
use talkbank_model::model::{SemanticDiff, SemanticEq};

// ---------------------------------------------------------------------------
// Test types
// ---------------------------------------------------------------------------

/// Named struct with a mix of semantic and skip fields.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct Word {
    text: String,
    category: u32,
    #[semantic_eq(skip)]
    span: Span,
}

/// Empty (unit) struct -- should always be semantically equal to itself.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct Empty;

/// Struct where every field is skipped -- always equal.
///
/// The fields are intentionally never read; the test exercises the derive's
/// "all fields skipped" code path, not the field values themselves.
#[derive(Debug, Clone, DeriveSemanticEq)]
#[allow(dead_code)]
struct AllSkipped {
    #[semantic_eq(skip)]
    span: Span,
    #[semantic_eq(skip)]
    cache_key: u64,
}

/// Enum with unit, unnamed, and named variants.
///
/// The comparison matrix exercises every shape, including skipped metadata.
#[derive(Debug, Clone, DeriveSemanticEq)]
#[allow(dead_code)]
enum Resolution {
    Single(String),
    Multiple(String, String),
    Named {
        value: String,
        #[semantic_eq(skip)]
        span: Span,
    },
    Unknown,
}

/// Nested struct: outer contains an inner SemanticEq type.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct Inner {
    value: String,
}

#[derive(Debug, Clone, DeriveSemanticEq)]
struct Outer {
    inner: Inner,
    label: String,
    #[semantic_eq(skip)]
    span: Span,
}

/// Tuple struct.
#[derive(Debug, Clone, DeriveSemanticEq)]
struct Pair(String, u32);

/// Enum with only unit variants.
#[derive(Debug, Clone, DeriveSemanticEq)]
enum Color {
    Red,
    Green,
    Blue,
}

/// The two generated implementations must agree with an independent semantic
/// partition, not merely agree with each other on a potentially wrong result.
#[test]
fn generated_equality_and_diff_agree_across_all_enum_shapes() {
    let values = [
        (Resolution::Single("a".into()), 0),
        (Resolution::Single("b".into()), 1),
        (Resolution::Multiple("a".into(), "b".into()), 2),
        (Resolution::Multiple("a".into(), "c".into()), 3),
        (Resolution::Unknown, 4),
        (
            Resolution::Named {
                value: "a".into(),
                span: Span::new(10, 12),
            },
            5,
        ),
        (
            Resolution::Named {
                value: "a".into(),
                span: Span::new(20, 22),
            },
            5,
        ),
        (
            Resolution::Named {
                value: "b".into(),
                span: Span::new(30, 32),
            },
            6,
        ),
    ];
    for (left, left_key) in &values {
        for (right, right_key) in &values {
            let equal = left_key == right_key;
            assert_eq!(left.semantic_eq(right), equal, "{left:?} / {right:?}");
            let report = left.semantic_diff(right);
            assert_eq!(report.is_empty(), equal, "{left:?} / {right:?}: {report}");
            assert!(!report.is_truncated());
        }
    }
    let report = values[5].0.semantic_diff(&values[7].0);
    assert_eq!(report.differences().len(), 1);
    assert_eq!(report.differences()[0].path, "value");
    assert_eq!(report.differences()[0].span, Some(Span::new(10, 12)));
}

#[test]
fn generated_struct_diffs_preserve_field_paths_and_skipped_metadata() {
    let left = Word {
        text: "one".into(),
        category: 1,
        span: Span::new(10, 13),
    };
    let right = Word {
        text: "two".into(),
        category: 2,
        span: Span::new(20, 23),
    };
    let report = left.semantic_diff(&right);
    let paths: Vec<_> = report
        .differences()
        .iter()
        .map(|diff| diff.path.as_str())
        .collect();
    assert_eq!(paths, ["text", "category"]);
    assert!(
        report
            .differences()
            .iter()
            .all(|diff| diff.span == Some(left.span))
    );

    let report = Pair("one".into(), 1).semantic_diff(&Pair("two".into(), 2));
    let paths: Vec<_> = report
        .differences()
        .iter()
        .map(|diff| diff.path.as_str())
        .collect();
    assert_eq!(paths, ["[0]", "[1]"]);
    assert!(Empty.semantic_diff(&Empty).is_empty());
    let left = AllSkipped {
        span: Span::new(1, 2),
        cache_key: 1,
    };
    let right = AllSkipped {
        span: Span::new(3, 4),
        cache_key: 2,
    };
    assert!(left.semantic_diff(&right).is_empty());
}

// ---------------------------------------------------------------------------
// Task 2: Core SemanticEq tests (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn simple_struct_equal() {
    let a = Word {
        text: "hello".into(),
        category: 1,
        span: Span::new(0, 5),
    };
    let b = Word {
        text: "hello".into(),
        category: 1,
        span: Span::new(100, 200),
    };
    assert!(a.semantic_eq(&b));
}

#[test]
fn simple_struct_not_equal_text() {
    let a = Word {
        text: "hello".into(),
        category: 1,
        span: Span::new(0, 5),
    };
    let b = Word {
        text: "world".into(),
        category: 1,
        span: Span::new(0, 5),
    };
    assert!(!a.semantic_eq(&b));
}

#[test]
fn skip_field_ignored() {
    // Same semantic fields, different skipped spans.
    let a = Word {
        text: "x".into(),
        category: 42,
        span: Span::new(0, 1),
    };
    let b = Word {
        text: "x".into(),
        category: 42,
        span: Span::new(999, 1000),
    };
    assert!(a.semantic_eq(&b));
}

#[test]
fn non_skip_field_checked() {
    // Same text but different category (non-skipped) should differ.
    let a = Word {
        text: "x".into(),
        category: 1,
        span: Span::new(0, 1),
    };
    let b = Word {
        text: "x".into(),
        category: 2,
        span: Span::new(0, 1),
    };
    assert!(!a.semantic_eq(&b));
}

#[test]
fn empty_struct_always_equal() {
    assert!(Empty.semantic_eq(&Empty));
}

#[test]
fn all_skipped_struct_always_equal() {
    let a = AllSkipped {
        span: Span::new(0, 10),
        cache_key: 111,
    };
    let b = AllSkipped {
        span: Span::new(50, 60),
        cache_key: 999,
    };
    assert!(a.semantic_eq(&b));
}

#[test]
fn enum_same_variant_equal() {
    let a = Resolution::Single("eng".into());
    let b = Resolution::Single("eng".into());
    assert!(a.semantic_eq(&b));
}

#[test]
fn enum_different_variant_not_equal() {
    let a = Resolution::Single("eng".into());
    let b = Resolution::Unknown;
    assert!(!a.semantic_eq(&b));
}

// ---------------------------------------------------------------------------
// Task 7: Nested/complex SemanticEq tests (5 bonus)
// ---------------------------------------------------------------------------

#[test]
fn nested_struct_equal() {
    let a = Outer {
        inner: Inner {
            value: "abc".into(),
        },
        label: "ok".into(),
        span: Span::new(0, 10),
    };
    let b = Outer {
        inner: Inner {
            value: "abc".into(),
        },
        label: "ok".into(),
        span: Span::new(99, 199),
    };
    assert!(a.semantic_eq(&b));
}

#[test]
fn nested_struct_inner_differs() {
    let a = Outer {
        inner: Inner {
            value: "abc".into(),
        },
        label: "ok".into(),
        span: Span::new(0, 10),
    };
    let b = Outer {
        inner: Inner {
            value: "xyz".into(),
        },
        label: "ok".into(),
        span: Span::new(0, 10),
    };
    assert!(!a.semantic_eq(&b));
}

#[test]
fn tuple_struct_equal() {
    let a = Pair("hello".into(), 5);
    let b = Pair("hello".into(), 5);
    assert!(a.semantic_eq(&b));
}

#[test]
fn tuple_struct_not_equal() {
    let a = Pair("hello".into(), 5);
    let b = Pair("hello".into(), 6);
    assert!(!a.semantic_eq(&b));
}

#[test]
fn enum_unit_variants_equal() {
    assert!(Color::Red.semantic_eq(&Color::Red));
    assert!(Color::Green.semantic_eq(&Color::Green));
    assert!(!Color::Red.semantic_eq(&Color::Blue));
}
