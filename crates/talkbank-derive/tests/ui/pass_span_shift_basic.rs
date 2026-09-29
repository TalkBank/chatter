#![deny(unused_variables)]

use talkbank_derive::SpanShift;
use talkbank_model::Span;

// Verify SpanShift can be derived on a struct with Span fields
#[derive(Debug, Clone, SpanShift)]
struct Located {
    value: String,
    span: Span,
}

// A skipped payload need not implement SpanShift or produce an unused binding.
struct Frozen;

#[derive(SpanShift)]
enum Positioned {
    Empty,
    Tuple(Span, #[span_shift(skip)] Frozen),
    Named {
        span: Span,
        #[span_shift(skip)]
        frozen: Frozen,
    },
}

fn main() {}
