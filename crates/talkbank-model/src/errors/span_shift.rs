//! Shared trait for shifting byte-based spans after text edits.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::{DiagnosticLocation, ErrorLabel, Location, ParseError, Span};

/// Trait for shifting byte-offset spans after an insertion or deletion.
///
/// All spans at or after `offset` are shifted by `delta` bytes.
/// Implementations exist for AST nodes, errors, and primitive types (no-ops).
pub trait SpanShift {
    /// Shift all spans at or after `offset` by `delta` bytes.
    fn shift_spans_after(&mut self, offset: usize, delta: isize);
}

/// Move `span` by `delta` if it lies at or after `offset`.
///
/// Retains the existing floor at byte zero for a deletion that reaches past the
/// start of a span. Both ends move by the same amount, so order is preserved.
/// `start <= end` is a `Span` invariant, so a start at or after the edit
/// places the whole span there.
fn move_after(span: &mut Span, offset: usize, delta: isize) {
    if span.start >= offset {
        span.start = span.start.saturating_add_signed(delta);
        span.end = span.end.saturating_add_signed(delta);
    }
}

impl SpanShift for Span {
    /// Every span is a real place, including a point at byte 0, so every span
    /// at or after the edit moves.
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        move_after(self, offset, delta);
    }
}

impl SpanShift for Location {
    /// A located value moves with the edit, including a point at byte 0; an
    /// unlocated value has nothing to move.
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        match self {
            Location::Located(span) => move_after(span, offset, delta),
            Location::Unlocated => {}
        }
    }
}

impl SpanShift for ErrorLabel {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        self.span.shift_spans_after(offset, delta);
    }
}

impl SpanShift for DiagnosticLocation {
    /// A known span moves with the edit. If it moved, its line and column
    /// described the old text and are dropped rather than left stale; a span
    /// before the edit keeps them. An unlocated diagnostic has nothing to move.
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        match self {
            DiagnosticLocation::Known { span, position } => {
                let before = *span;
                span.shift_spans_after(offset, delta);
                if *span != before {
                    *position = None;
                }
            }
            DiagnosticLocation::Unlocated => {}
        }
    }
}

impl SpanShift for ParseError {
    /// Rebase document locations while preserving the independently owned
    /// context snapshot. `ErrorContext::span` indexes its own `source_text`,
    /// which is not edited or replaced by this operation.
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        self.location.shift_spans_after(offset, delta);
        for label in &mut self.labels {
            label.shift_spans_after(offset, delta);
        }
    }
}

impl SpanShift for String {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for std::sync::Arc<str> {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for smol_str::SmolStr {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for bool {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for u8 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for u32 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for u64 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for std::num::NonZeroUsize {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for usize {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for i32 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl SpanShift for i64 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
}

impl<T: SpanShift> SpanShift for Option<T> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        if let Some(value) = self {
            value.shift_spans_after(offset, delta);
        }
    }
}

impl<T: SpanShift> SpanShift for Vec<T> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        for item in self {
            item.shift_spans_after(offset, delta);
        }
    }
}

impl<T: SpanShift, const N: usize> SpanShift for smallvec::SmallVec<[T; N]> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        for item in self {
            item.shift_spans_after(offset, delta);
        }
    }
}

impl<T: SpanShift> SpanShift for Box<T> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        self.as_mut().shift_spans_after(offset, delta);
    }
}

impl<K, V: SpanShift> SpanShift for indexmap::IndexMap<K, V> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        for value in self.values_mut() {
            value.shift_spans_after(offset, delta);
        }
    }
}
