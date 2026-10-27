//! The text a parse ran over, and the only thing that can place a borrowed
//! token slice back into it.
//!
//! # Why this exists
//!
//! Every `Token` in this crate borrows a `&str` INTO the source, so a token's
//! byte offset is recoverable: it is the distance between the slice's pointer
//! and the source's. Nothing did that recovery. `convert::items` opened
//! the former separator conversion with `let s = Span::DUMMY;` and the parser wrote
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

/// A byte range that is outside the source or splits a UTF-8 character.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("native source range {start}..{end} is not a UTF-8 slice of {source_len} bytes")]
pub struct InvalidSourceRange {
    start: usize,
    end: usize,
    source_len: usize,
}

/// Original bytes and their coordinates admitted against one source owner.
///
/// Accessors need no further range check and accept no replacement source.
/// Equality compares the selected bytes and coordinates, not the entire source
/// buffer, and does not certify that two slices have the same source owner.
#[derive(Clone, Copy, Debug)]
pub struct SourceSlice<'a> {
    source: SourceText<'a>,
    text: &'a str,
    span: Span,
}

impl PartialEq for SourceSlice<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.span == other.span && self.text == other.text
    }
}

impl Eq for SourceSlice<'_> {}

impl<'a> SourceSlice<'a> {
    /// The exact admitted bytes, including any whitespace or line endings.
    pub fn text(self) -> &'a str {
        self.text
    }

    /// Coordinates in the retained source.
    pub fn span(self) -> Span {
        self.span
    }

    /// The source against which these coordinates were admitted.
    pub fn source(self) -> SourceText<'a> {
        self.source
    }
}

impl<'a> SourceText<'a> {
    /// Wrap the text a parse is running over.
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self(text)
    }

    /// The complete source is already an admitted UTF-8 slice.
    pub fn whole_slice(self) -> SourceSlice<'a> {
        SourceSlice {
            source: self,
            text: self.0,
            span: self.whole(),
        }
    }

    /// Physical line contents with their original byte coordinates, excluding
    /// LF, CRLF or lone CR endings. CRLF is one ending; a trailing ending does
    /// not invent another line. Every yielded slice keeps this source owner.
    pub(crate) fn physical_lines(self) -> impl Iterator<Item = SourceSlice<'a>> {
        let mut remaining = self.0;
        std::iter::from_fn(move || {
            if remaining.is_empty() {
                return None;
            }
            let start = self.0.len() - remaining.len();
            let content_len = remaining.find(['\r', '\n']).unwrap_or(remaining.len());
            let (text, ending) = remaining.split_at(content_len);
            let after_cr = ending.strip_prefix('\r').unwrap_or(ending);
            remaining = after_cr.strip_prefix('\n').unwrap_or(after_cr);
            Some(SourceSlice {
                source: self,
                text,
                span: Span::new(start, start + text.len()),
            })
        })
    }

    /// The span of the whole text.
    ///
    /// The honest fallback for a diagnostic whose exact position cannot be
    /// resolved: "somewhere in this file" is a range a reader can act on, and
    /// it is NOT `Span::DUMMY`, which is also the legal zero-width position at
    /// byte 0 and would place the finding at the top of the file as though
    /// that were where it is.
    #[must_use]
    pub fn whole(self) -> Span {
        Span::new(0, self.0.len())
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
            .then(|| Span::new(start - base, end - base))
    }

    /// Admit borrowed token bytes against this source and retain their position.
    pub(crate) fn bind_slice(self, text: &'a str) -> Option<SourceSlice<'a>> {
        Some(SourceSlice {
            source: self,
            text,
            span: self.span_of(text)?,
        })
    }
}

impl<'a> SourceText<'a> {
    /// Admit a byte range once, refusing reversed, outside and non-UTF-8 bounds.
    pub fn slice(
        self,
        range: std::ops::Range<usize>,
    ) -> Result<SourceSlice<'a>, InvalidSourceRange> {
        let text = self.0.get(range.clone()).ok_or(InvalidSourceRange {
            start: range.start,
            end: range.end,
            source_len: self.0.len(),
        })?;
        Ok(SourceSlice {
            source: self,
            text,
            span: Span::new(range.start, range.end),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_lines_preserve_mixed_endings_empty_lines_and_utf8_coordinates() {
        for source in ["é\r\n\r二\nx\r\nz", "é\r\n\r二\nx\r\nz\r\n"] {
            let lines: Vec<_> = SourceText::new(source).physical_lines().collect();
            let expected = [
                ("é", 0..2),
                ("", 4..4),
                ("二", 5..8),
                ("x", 9..10),
                ("z", 12..13),
            ];
            assert_eq!(lines.len(), expected.len());
            for (line, (text, range)) in lines.into_iter().zip(expected) {
                assert_eq!(line.text(), text);
                assert_eq!(line.span().to_range(), range);
                assert_eq!(line.source().0.as_ptr(), source.as_ptr());
            }
        }
        assert!(SourceText::new("").physical_lines().next().is_none());
    }

    #[test]
    fn slice_admission_checks_bounds_and_retains_its_owner() {
        let input = include_str!("../../../corpus/reference/edge-cases/unicode-ipa-content.cha");
        let source = SourceText::new(input);
        let (start, character) = input.char_indices().find(|(_, c)| !c.is_ascii()).unwrap();
        let end = start + character.len_utf8();
        let slice = source.slice(start..end).unwrap();
        assert_eq!(slice.text(), character.to_string());
        assert_eq!(slice.span(), Span::new(start, end));
        assert_eq!(slice.source().span_of(slice.text()), Some(slice.span()));
        assert_eq!(slice.source().whole_slice().text().as_ptr(), input.as_ptr());

        for range in [
            start + 1..end,
            start..end - 1,
            end..start,
            0..input.len() + 1,
        ] {
            assert!(source.slice(range).is_err());
        }
        let empty = source.slice(input.len()..input.len()).unwrap();
        assert!(empty.text().is_empty());
        assert_eq!(empty.span(), Span::new(input.len(), input.len()));
    }

    #[test]
    fn a_slice_of_this_source_gets_its_real_offset() {
        let text = "*CHI:\thello ,, world .\n";
        let source = SourceText::new(text);
        let comma = &text[12..13];
        assert_eq!(source.span_of(comma), Some(Span::new(12, 13)));
    }

    #[test]
    fn the_whole_source_spans_the_whole_source() {
        let text = "abc";
        let source = SourceText::new(text);
        assert_eq!(source.span_of(text), Some(Span::new(0, 3)));
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
        assert_eq!(source.span_of(&text[3..3]), Some(Span::new(3, 3)));
    }
}
