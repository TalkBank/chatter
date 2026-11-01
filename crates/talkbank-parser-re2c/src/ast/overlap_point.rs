//! Admitted word-internal overlap-point spelling and its semantic payload.

use serde::{Serialize, Serializer};
use talkbank_model::model::{OverlapIndex, OverlapPointKind};

/// One complete lexical overlap point: a glyph and an optional ASCII 1–9.
///
/// This is lexical admission, not semantic validation: index 1 is preserved
/// for the shared model's range diagnostic. The kind and index cannot drift
/// from the borrowed spelling, and lowering never reparses that spelling.
///
/// Independently supplied semantic fields cannot forge an admitted point:
///
/// ```compile_fail,E0451
/// use talkbank_parser_re2c::ast::WordOverlapPoint;
/// use talkbank_model::model::OverlapPointKind;
/// let point = WordOverlapPoint {
///     text: "⌉2",
///     kind: OverlapPointKind::TopOverlapBegin,
///     index: None,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordOverlapPoint<'a> {
    text: &'a str,
    kind: OverlapPointKind,
    index: Option<OverlapIndex>,
}

impl<'a> WordOverlapPoint<'a> {
    /// Admit a whole overlap-point spelling, with no trailing input.
    pub fn new(text: &'a str) -> Option<Self> {
        let (point, remaining) = Self::take_prefix(text)?;
        remaining.is_empty().then_some(point)
    }

    /// Consume the longest lexical point prefix, retaining the remaining input.
    pub(crate) fn take_prefix(source: &'a str) -> Option<(Self, &'a str)> {
        let glyph = source.chars().next()?;
        let kind = match glyph {
            '⌈' => OverlapPointKind::TopOverlapBegin,
            '⌉' => OverlapPointKind::TopOverlapEnd,
            '⌊' => OverlapPointKind::BottomOverlapBegin,
            '⌋' => OverlapPointKind::BottomOverlapEnd,
            _ => return None,
        };
        let glyph_end = glyph.len_utf8();
        let (index, end) = match source.as_bytes().get(glyph_end) {
            Some(digit @ b'1'..=b'9') => (
                Some(OverlapIndex::new(u32::from(*digit - b'0'))),
                glyph_end + 1,
            ),
            _ => (None, glyph_end),
        };
        let (text, remaining) = source.split_at(end);
        Some((Self { text, kind, index }, remaining))
    }

    /// The exact borrowed lexical spelling.
    pub fn text(self) -> &'a str {
        self.text
    }

    /// The shape decoded during admission.
    pub fn kind(self) -> OverlapPointKind {
        self.kind
    }

    /// The optional index decoded during admission.
    pub fn index(self) -> Option<OverlapIndex> {
        self.index
    }
}

impl Serialize for WordOverlapPoint<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Preserve the native AST's existing [kind, spelling] inspection wire.
        (self.kind, self.text).serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::WordBodyItem;

    #[test]
    fn complete_points_own_kind_index_and_existing_wire() {
        for (glyph, kind) in [
            ("⌈", OverlapPointKind::TopOverlapBegin),
            ("⌉", OverlapPointKind::TopOverlapEnd),
            ("⌊", OverlapPointKind::BottomOverlapBegin),
            ("⌋", OverlapPointKind::BottomOverlapEnd),
        ] {
            for (suffix, index) in [("", None), ("1", Some(1)), ("2", Some(2)), ("9", Some(9))] {
                let text = format!("{glyph}{suffix}");
                let point = WordOverlapPoint::new(&text).expect("complete lexical point");
                assert_eq!(point.kind(), kind);
                assert_eq!(point.index().map(OverlapIndex::get), index);
                assert_eq!(point.text().as_ptr(), text.as_ptr());
                assert_eq!(
                    serde_json::to_value(WordBodyItem::OverlapPoint(point)).unwrap(),
                    serde_json::json!({"OverlapPoint": [kind, text]}),
                );
            }
        }
        for text in ["", "x", "⌈0", "⌈12", "⌈a", "⌈２", "⌈⌉"] {
            assert!(WordOverlapPoint::new(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn scanner_retains_nonindex_and_extra_digits_as_remaining_source() {
        for (source, point, remaining) in [
            ("⌈2rest", "⌈2", "rest"),
            ("⌉0rest", "⌉", "0rest"),
            ("⌊12rest", "⌊1", "2rest"),
            ("⌋２rest", "⌋", "２rest"),
        ] {
            let (admitted, rest) = WordOverlapPoint::take_prefix(source).unwrap();
            assert_eq!(admitted.text(), point);
            assert_eq!(rest, remaining);
            assert_eq!(admitted.text().len() + rest.len(), source.len());
        }
    }
}
