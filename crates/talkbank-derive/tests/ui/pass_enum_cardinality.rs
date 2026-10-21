#![deny(unused_variables, unreachable_patterns, unreachable_code)]

use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};
use talkbank_model::SpanShift as _;
// The SemanticEq expansion names `crate::model` and `crate::Location`.
use talkbank_model::Location;
use talkbank_model::model;
use talkbank_model::model::{SemanticDiff as _, SemanticEq as _};

#[derive(SemanticEq, SpanShift, ValidationTagged)]
enum Empty {}

#[derive(SemanticEq, SpanShift, ValidationTagged)]
enum Unit {
    Only,
}

#[derive(SemanticEq, SpanShift, ValidationTagged)]
enum Tuple {
    Only(String),
}

#[derive(SemanticEq, SpanShift, ValidationTagged)]
enum Named {
    Only { value: String },
}

struct Frozen;

#[derive(SemanticEq, SpanShift, ValidationTagged)]
enum Skipped {
    Only(
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        Frozen,
    ),
}

fn assert_contract<
    T: model::SemanticEq + model::SemanticDiff + model::ValidationTagged + talkbank_model::SpanShift,
>() {
}

fn main() {
    assert_contract::<Empty>();
    assert_contract::<Unit>();
    assert_contract::<Tuple>();
    assert_contract::<Named>();
    assert_contract::<Skipped>();

    assert!(Unit::Only.semantic_eq(&Unit::Only));
    assert!(Unit::Only.semantic_diff(&Unit::Only).is_empty());
    assert!(Tuple::Only("same".into()).semantic_eq(&Tuple::Only("same".into())));
    assert!(
        !Named::Only {
            value: "left".into()
        }
        .semantic_eq(&Named::Only {
            value: "right".into()
        })
    );
    let mut skipped = Skipped::Only(Frozen);
    skipped.shift_spans_after(0, 1);
    assert!(skipped.semantic_eq(&Skipped::Only(Frozen)));
    assert!(skipped.semantic_diff(&Skipped::Only(Frozen)).is_empty());
}
