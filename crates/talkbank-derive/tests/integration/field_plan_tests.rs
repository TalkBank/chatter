//! Native Rust boundary contracts, not canonical CHAT coverage.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use talkbank_derive::{SemanticEq as Equality, SpanShift as Shift};
use talkbank_model::model::{
    SemanticDiff, SemanticDiffContext, SemanticDiffReport, SemanticEq, SemanticPath,
};
use talkbank_model::{Span, SpanShift};

// Inherent methods must never replace the generated trait's recursive calls.
#[derive(Clone, Equality, Shift)]
struct MethodCollision {
    value: String,
    #[semantic_eq(skip)]
    span: Span,
}

#[allow(dead_code)] // Deliberate traps: correct generated calls bypass these.
impl MethodCollision {
    fn semantic_eq(&self, _other: &Self) -> bool {
        panic!("inherent equality selected instead of SemanticEq")
    }

    fn semantic_diff_into(
        &self,
        _other: &Self,
        _path: &mut SemanticPath,
        _report: &mut SemanticDiffReport,
        _ctx: &mut SemanticDiffContext,
    ) {
        panic!("inherent diff selected instead of SemanticDiff")
    }

    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {
        panic!("inherent shift selected instead of SpanShift")
    }
}

#[derive(Equality, Shift)]
struct CollisionStruct {
    payload: MethodCollision,
}

#[derive(Equality, Shift)]
enum CollisionEnum {
    Tuple(MethodCollision),
    Named { payload: MethodCollision },
}

fn collision(value: &str) -> MethodCollision {
    MethodCollision {
        value: value.into(),
        span: Span::new(10, 20),
    }
}

fn collision_variants(value: &str) -> [CollisionEnum; 2] {
    [
        CollisionEnum::Tuple(collision(value)),
        CollisionEnum::Named {
            payload: collision(value),
        },
    ]
}

#[test]
fn equality_dispatch_cannot_select_an_inherent_field_method() {
    let left = CollisionStruct {
        payload: collision("left"),
    };
    assert!(left.semantic_eq(&CollisionStruct {
        payload: collision("left")
    }));
    assert!(!left.semantic_eq(&CollisionStruct {
        payload: collision("right")
    }));
    for ((left, same), different) in collision_variants("left")
        .into_iter()
        .zip(collision_variants("left"))
        .zip(collision_variants("right"))
    {
        assert!(left.semantic_eq(&same));
        assert!(!left.semantic_eq(&different));
    }
}

#[test]
fn diff_dispatch_cannot_select_an_inherent_field_method() {
    let left = CollisionStruct {
        payload: collision("left"),
    };
    assert!(
        left.semantic_diff(&CollisionStruct {
            payload: collision("left")
        })
        .is_empty()
    );
    assert_eq!(
        left.semantic_diff(&CollisionStruct {
            payload: collision("right")
        })
        .differences()
        .len(),
        1
    );
    for ((left, same), different) in collision_variants("left")
        .into_iter()
        .zip(collision_variants("left"))
        .zip(collision_variants("right"))
    {
        assert!(left.semantic_diff(&same).is_empty());
        assert_eq!(left.semantic_diff(&different).differences().len(), 1);
    }
}

#[test]
fn shift_dispatch_cannot_select_an_inherent_field_method() {
    let mut node = CollisionStruct {
        payload: collision("left"),
    };
    node.shift_spans_after(10, 5);
    assert_eq!(node.payload.span, Span::new(15, 25));
    for mut node in collision_variants("left") {
        node.shift_spans_after(10, 5);
        let payload = match node {
            CollisionEnum::Tuple(payload) | CollisionEnum::Named { payload } => payload,
        };
        assert_eq!(payload.span, Span::new(15, 25));
    }
}

// This payload deliberately implements neither generated trait.
#[derive(Clone)]
struct Frozen;

#[derive(Clone, Equality, Shift)]
enum Raw {
    Named {
        r#type: u32,
        #[semantic_eq(skip)]
        r#match: Option<Span>,
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        __tb_left_0: Frozen,
    },
    Tuple(
        u32,
        #[semantic_eq(skip)] Span,
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        Frozen,
    ),
    Empty,
}

#[test]
fn raw_fields_and_generated_name_spellings_preserve_diff_and_shift() {
    let left = Raw::Named {
        r#type: 1,
        r#match: Some(Span::new(10, 12)),
        __tb_left_0: Frozen,
    };
    let mut right = Raw::Named {
        r#type: 2,
        r#match: Some(Span::new(20, 22)),
        __tb_left_0: Frozen,
    };
    assert!(!left.semantic_eq(&right));
    let report = left.semantic_diff(&right);
    assert_eq!(report.differences().len(), 1);
    assert_eq!(report.differences()[0].path, "r#type");
    assert_eq!(
        report.differences()[0].location,
        crate::Location::Located(Span::new(10, 12))
    );
    right.shift_spans_after(20, -5);
    match right {
        Raw::Named { r#match, .. } => assert_eq!(r#match, Some(Span::new(15, 17))),
        _ => panic!("wrong variant"),
    }
    assert!(Raw::Empty.semantic_eq(&Raw::Empty));
}

#[test]
fn tuple_enum_keeps_skipped_context_without_requiring_payload_traits() {
    let left = Raw::Tuple(1, Span::new(10, 12), Frozen);
    let mut right = Raw::Tuple(2, Span::new(20, 22), Frozen);
    let report = left.semantic_diff(&right);
    assert_eq!(report.differences()[0].path, "[0]");
    assert_eq!(
        report.differences()[0].location,
        crate::Location::Located(Span::new(10, 12))
    );
    right.shift_spans_after(20, 5);
    match right {
        Raw::Tuple(_, span, _) => assert_eq!(span, Span::new(25, 27)),
        _ => panic!("wrong variant"),
    }
}

#[derive(Shift)]
enum ParameterNames {
    Named {
        offset: Span,
        delta: Span,
        r#type: Span,
    },
}

#[test]
fn field_names_cannot_shadow_shift_parameters() {
    let mut value = ParameterNames::Named {
        offset: Span::new(1, 3),
        delta: Span::new(10, 12),
        r#type: Span::new(20, 22),
    };
    value.shift_spans_after(10, 5);
    let ParameterNames::Named {
        offset,
        delta,
        r#type,
    } = value;
    assert_eq!(offset, Span::new(1, 3));
    assert_eq!(delta, Span::new(15, 17));
    assert_eq!(r#type, Span::new(25, 27));
}

#[derive(Equality)]
struct Generic<'a, T = String, const N: usize = 2>
where
    T: SemanticEq + SemanticDiff,
{
    text: &'a str,
    payload: T,
    #[semantic_eq(skip)]
    marker: std::marker::PhantomData<[u8; N]>,
}

#[derive(Shift)]
struct GenericShift<T> {
    values: Vec<T>,
}

#[test]
fn generics_preserve_lifetimes_defaults_where_clauses_and_recursive_bounds() {
    let left: Generic<'_> = Generic {
        text: "same",
        payload: "left".into(),
        marker: std::marker::PhantomData,
    };
    let right: Generic<'_> = Generic {
        text: "same",
        payload: "right".into(),
        marker: std::marker::PhantomData,
    };
    assert!(!left.semantic_eq(&right));
    assert_eq!(left.semantic_diff(&right).differences()[0].path, "payload");
    let mut shifted = GenericShift {
        values: vec![Span::new(10, 20)],
    };
    shifted.shift_spans_after(10, 5);
    assert_eq!(shifted.values, vec![Span::new(15, 25)]);
}

#[derive(Equality)]
struct Fallback {
    value: u32,
    #[semantic_eq(skip)]
    earlier: Span,
    #[semantic_eq(skip)]
    later: Option<Span>,
}

#[test]
fn named_context_keeps_last_candidate_and_absent_option_policy() {
    let make = |value, later| Fallback {
        value,
        earlier: Span::new(1, 3),
        later,
    };
    let left = make(1, Some(Span::new(10, 12)));
    assert_eq!(left.earlier, Span::new(1, 3));
    let right = make(2, Some(Span::new(20, 22)));
    assert_eq!(
        left.semantic_diff(&right).differences()[0].location,
        crate::Location::Located(Span::new(10, 12))
    );
    assert_eq!(
        make(1, None).semantic_diff(&right).differences()[0].location,
        crate::Location::Unlocated
    );
}

#[derive(Equality)]
struct Priority {
    first: u32,
    second: u32,
    third: u32,
    #[semantic_eq(skip)]
    span: Span,
    #[semantic_eq(skip)]
    speaker_span: Span,
}

#[test]
fn truncation_restores_caller_path_and_context_after_selected_span() {
    let left = Priority {
        first: 1,
        second: 2,
        third: 3,
        span: Span::new(10, 12),
        speaker_span: Span::new(20, 22),
    };
    let right = Priority {
        first: 4,
        second: 5,
        third: 6,
        span: Span::new(30, 32),
        speaker_span: Span::new(40, 42),
    };
    let mut path = SemanticPath::new();
    path.push_field("parent");
    let mut ctx = SemanticDiffContext::new();
    ctx.push_location(crate::Location::Located(Span::new(1, 3)));
    let mut report = SemanticDiffReport::new(1);
    left.semantic_diff_into(&right, &mut path, &mut report, &mut ctx);
    assert!(report.is_truncated());
    assert_eq!(report.differences().len(), 1);
    assert_eq!(report.differences()[0].path, "parent.first");
    assert_eq!(
        report.differences()[0].location,
        crate::Location::Located(Span::new(10, 12))
    );
    assert_eq!(
        ctx.current_location(),
        crate::Location::Located(Span::new(1, 3))
    );
    let mut tail = SemanticDiffReport::new(1);
    1u32.semantic_diff_into(&2, &mut path, &mut tail, &mut ctx);
    assert_eq!(tail.differences()[0].path, "parent");
    assert_eq!(
        tail.differences()[0].location,
        crate::Location::Located(Span::new(1, 3))
    );
    assert_eq!(left.speaker_span, Span::new(20, 22));
}
