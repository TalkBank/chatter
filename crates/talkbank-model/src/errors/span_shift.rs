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

    /// Consume a value and translate each document span through a checked map.
    /// Failure exposes no partially translated value. Independently owned
    /// diagnostic snippets and fields marked `span_shift(skip)` stay unchanged.
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E>
    where
        Self: Sized;
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
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        map(self)
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
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        match self {
            Self::Located(span) => map(span).map(Self::Located),
            Self::Unlocated => Ok(Self::Unlocated),
        }
    }
}

impl SpanShift for ErrorLabel {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        self.span.shift_spans_after(offset, delta);
    }
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        Ok(Self {
            span: map(self.span)?,
            ..self
        })
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
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        match self {
            Self::Known { span, position } => {
                let mapped = map(span)?;
                Ok(Self::Known {
                    span: mapped,
                    position: if mapped == span { position } else { None },
                })
            }
            Self::Unlocated => Ok(Self::Unlocated),
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
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        Ok(Self {
            location: self.location.try_map_spans(map)?,
            labels: self.labels.try_map_spans(map)?,
            ..self
        })
    }
}

impl SpanShift for String {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for std::sync::Arc<str> {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for smol_str::SmolStr {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for bool {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for u8 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for u32 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for u64 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for std::num::NonZeroUsize {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for usize {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for i32 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl SpanShift for i64 {
    fn shift_spans_after(&mut self, _offset: usize, _delta: isize) {}
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        let _ = map;
        Ok(self)
    }
}

impl<T: SpanShift> SpanShift for Option<T> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        if let Some(value) = self {
            value.shift_spans_after(offset, delta);
        }
    }
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        self.map(|value| value.try_map_spans(map)).transpose()
    }
}

impl<T: SpanShift> SpanShift for Vec<T> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        for item in self {
            item.shift_spans_after(offset, delta);
        }
    }
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        self.into_iter()
            .map(|value| value.try_map_spans(map))
            .collect()
    }
}

impl<T: SpanShift, const N: usize> SpanShift for smallvec::SmallVec<[T; N]> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        for item in self {
            item.shift_spans_after(offset, delta);
        }
    }
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        self.into_iter()
            .map(|value| value.try_map_spans(map))
            .collect()
    }
}

impl<T: SpanShift> SpanShift for Box<T> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        self.as_mut().shift_spans_after(offset, delta);
    }
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        (*self).try_map_spans(map).map(Box::new)
    }
}

impl<K: Eq + std::hash::Hash, V: SpanShift> SpanShift for indexmap::IndexMap<K, V> {
    fn shift_spans_after(&mut self, offset: usize, delta: isize) {
        for value in self.values_mut() {
            value.shift_spans_after(offset, delta);
        }
    }
    fn try_map_spans<E>(self, map: &mut impl FnMut(Span) -> Result<Span, E>) -> Result<Self, E> {
        self.into_iter()
            .map(|(key, value)| value.try_map_spans(map).map(|mapped| (key, mapped)))
            .collect()
    }
}

/// Translate document coordinates without cloning independently owned snippets.
/// Failure is a tool failure, never evidence that the CHAT input is invalid.
pub(super) fn translate_diagnostic<E: std::fmt::Display>(
    error: ParseError,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> ParseError {
    let translated = error
        .location
        .rebased(|span| span)
        .try_map_spans(map)
        .and_then(|location| {
            error
                .labels
                .try_map_spans(map)
                .map(|labels| (location, labels))
        });
    match translated {
        Ok((location, labels)) => ParseError {
            location,
            labels,
            ..error
        },
        Err(failure) => {
            let mut rejected = ParseError::at_location(
                crate::ErrorCode::InternalError,
                crate::Severity::Error,
                DiagnosticLocation::Unlocated,
                format!(
                    "Cannot translate diagnostic {} ({:?}): {failure}",
                    error.code, error.message
                ),
            );
            rejected.context = error.context;
            rejected
        }
    }
}
