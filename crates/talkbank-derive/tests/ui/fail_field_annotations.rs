use talkbank_derive::{SemanticEq, SpanShift};

#[derive(SemanticEq)]
struct Typo {
    #[semantic_eq(skp)]
    value: u32,
}

#[derive(SpanShift)]
struct ShiftTypo(#[span_shift(skp)] u32);

#[derive(SemanticEq)]
struct Duplicate {
    #[semantic_eq(skip)]
    #[semantic_eq(skip)]
    value: u32,
}

#[derive(SpanShift)]
struct ShiftDuplicate(#[span_shift(skip)] #[span_shift(skip)] u32);

#[derive(SemanticEq)]
#[semantic_eq(skip)]
struct Container;

#[derive(SpanShift)]
enum Variant {
    #[span_shift(skip)]
    Empty,
}

fn main() {}
