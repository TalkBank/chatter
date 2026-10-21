use talkbank_derive::SpanShift;
use talkbank_model::SpanShift as _;

struct Frozen;

#[derive(SpanShift)]
struct Active<T> {
    value: T,
}

fn main() {
    let mut node = Active { value: Frozen };
    node.shift_spans_after(0, 1);
}
