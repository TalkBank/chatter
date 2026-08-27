//! The text a parse ran over, and the only thing that can place a borrowed
//! token slice back into it.
//!
//! # Why this exists
//!
//! Every `Token` in this crate borrows a `&str` INTO the source, so a token's
//! byte offset is recoverable: it is the distance between the slice's pointer
//! and the source's. Nothing did that recovery. `convert::items` opened
//! `separator_from_kind` with `let s = Span::DUMMY;` and the parser wrote
//! `try_map(|tok, _span| ..)`, discarding the position, so every separator and
//! every word reached the model at `Span::DUMMY`.
//!
//! That is not a cosmetic loss. `Span::DUMMY` is `{0, 0}`, which is also a
//! real position, and validation rules FILTER ON IT: `comma_span()` ends in
//! `.filter(|span| *span != Span::DUMMY)`, so a dummy span makes the rule
//! answer "there is no comma here" and E258 never fires. Measured 2026-08-27:
//! `*CHI:\thello ,, world .` reports E258 under tree-sitter and nothing under
//! re2c, and `semantic_eq` skips spans, so no equivalence test could see it.
//!
//! # Why a newtype rather than passing `&str`
//!
//! A span is only meaningful for a slice of THIS text; asking one string for
//! the offset of a slice of another is nonsense that a bare `&str` parameter
//! cannot refuse. This type owns the pairing, which is the cure this
//! repository names for "a relationship between two values maintained by
//! convention": possession of the `SourceText` is the proof.

use talkbank_model::Span;

/// The source text a parse is running over.
#[derive(Clone, Copy, Debug)]
pub struct SourceText<'a>(&'a str);

impl<'a> SourceText<'a> {
    /// Wrap the text a parse is running over.
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self(text)
    }

    /// The span of `slice` within this source.
    ///
    /// `None` when `slice` does not lie inside this text, which means the
    /// caller paired a slice with the wrong source. Deliberately NOT
    /// `Span::DUMMY`: answering with the sentinel is exactly the silent
    /// default this module exists to remove, and it would be indistinguishable
    /// from a real zero-length span at offset 0.
    #[must_use]
    pub fn span_of(self, slice: &str) -> Option<Span> {
        let base = self.0.as_ptr() as usize;
        let start = slice.as_ptr() as usize;
        let end = start.checked_add(slice.len())?;
        (start >= base && end <= base.checked_add(self.0.len())?)
            .then(|| Span::from_usize(start - base, end - base))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_of_this_source_gets_its_real_offset() {
        let text = "*CHI:\thello ,, world .\n";
        let source = SourceText::new(text);
        let comma = &text[12..13];
        assert_eq!(source.span_of(comma), Some(Span::from_usize(12, 13)));
    }

    #[test]
    fn the_whole_source_spans_the_whole_source() {
        let text = "abc";
        let source = SourceText::new(text);
        assert_eq!(source.span_of(text), Some(Span::from_usize(0, 3)));
    }

    /// A slice of a DIFFERENT string is refused rather than given a dummy.
    ///
    /// The case that matters: two separately allocated strings with equal
    /// contents must not be mistaken for one another, so this compares by
    /// address, not by value.
    #[test]
    fn a_slice_of_another_string_is_refused() {
        let text = String::from("*CHI:\thello .\n");
        let other = String::from("*CHI:\thello .\n");
        let source = SourceText::new(&text);
        assert_eq!(source.span_of(&other[6..11]), None);
    }

    /// An empty slice at the very end is still inside the source.
    #[test]
    fn an_empty_slice_at_the_end_is_inside() {
        let text = "abc";
        let source = SourceText::new(text);
        assert_eq!(source.span_of(&text[3..3]), Some(Span::from_usize(3, 3)));
    }
}
