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

// Generated method parameters must not collide with declared generic names.
#[derive(SpanShift)]
struct TypeCollision<__TalkbankSpanMapError> {
    span: Span,
    #[span_shift(skip)]
    opaque: __TalkbankSpanMapError,
}

#[allow(non_upper_case_globals)]
#[derive(SpanShift)]
struct ConstCollision<const __TalkbankSpanMapError: usize> {
    span: Span,
    #[span_shift(skip)]
    opaque: [u8; __TalkbankSpanMapError],
}

#[derive(SpanShift)]
struct Unit;

fn main() {
    use talkbank_model::SpanShift as _;
    let result: Result<Unit, ()> = Unit.try_map_spans(&mut Ok);
    let _ = result;
}
