//! Source admission for ordered delimiter pairs.

use serde::Serialize;

/// Ordered, source-owned delimiters and the complete enclosed source extent.
///
/// The parser owns delimiter-kind and content grammar checks. This carrier
/// admits nonempty delimiter slices from one source in nonoverlapping order.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Enclosure<'a> {
    open: &'a str,
    close: &'a str,
    #[serde(skip)]
    source: crate::source_text::SourceSlice<'a>,
}

/// Delimiter source ownership or authored ordering could not be established.
#[derive(Debug)]
pub(crate) struct RefusedEnclosureSource;

impl<'a> Enclosure<'a> {
    pub(crate) fn new(
        source: crate::source_text::SourceText<'a>,
        open: &'a str,
        close: &'a str,
    ) -> Result<Self, RefusedEnclosureSource> {
        let open_span = source.span_of(open).ok_or(RefusedEnclosureSource)?;
        let close_span = source.span_of(close).ok_or(RefusedEnclosureSource)?;
        if open.is_empty() || close.is_empty() || open_span.end() > close_span.start() {
            return Err(RefusedEnclosureSource);
        }
        Ok(Self {
            open,
            close,
            source: source
                .slice(open_span.start()..close_span.end())
                .map_err(|_| RefusedEnclosureSource)?,
        })
    }

    /// Opening delimiter exactly as written.
    pub fn open(self) -> &'a str {
        self.open
    }

    /// Closing delimiter exactly as written.
    pub fn close(self) -> &'a str {
        self.close
    }

    /// Admitted extent from the opening through the closing delimiter.
    pub fn span(self) -> talkbank_model::Span {
        self.source.span()
    }

    pub(crate) fn source(self) -> crate::source_text::SourceSlice<'a> {
        self.source
    }

    /// Inspect the complete extent in a supplied source, refusing foreign bytes.
    pub fn span_in(
        self,
        source: crate::source_text::SourceText<'_>,
    ) -> Option<talkbank_model::Span> {
        source.span_of(self.source.text())
    }
}

// Keep the delimiter-spelling equality contract independent of location.
impl PartialEq for Enclosure<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.open == other.open && self.close == other.close
    }
}

impl Eq for Enclosure<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_text::SourceText;
    use talkbank_model::Span;

    #[test]
    fn enclosure_admits_ordered_source_and_preserves_wire_and_equality() {
        let input = String::from("prefix <word> suffix");
        let source = SourceText::new(&input);
        let open = &input[7..8];
        let close = &input[12..13];
        let admitted = Enclosure::new(source, open, close).unwrap();
        assert_eq!(admitted.span(), Span::new(7, 13));
        assert_eq!(admitted.open(), "<");
        assert_eq!(admitted.close(), ">");
        assert_eq!(admitted.source().text(), "<word>");
        assert_eq!(admitted.span_in(source), Some(admitted.span()));
        let wire = serde_json::to_value(admitted).unwrap();
        assert_eq!(wire, serde_json::json!({"open": "<", "close": ">"}));

        let foreign = String::from("<word>");
        assert_eq!(admitted.span_in(SourceText::new(&foreign)), None);
        assert!(Enclosure::new(SourceText::new(&foreign), open, close).is_err());
        assert!(Enclosure::new(source, &foreign[..1], close).is_err());
        assert!(Enclosure::new(source, open, &foreign[5..]).is_err());
        let at_zero =
            Enclosure::new(SourceText::new(&foreign), &foreign[..1], &foreign[5..]).unwrap();
        assert_eq!(
            admitted, at_zero,
            "equality retains delimiter spelling only"
        );

        for (open, close) in [
            (close, open),
            (&input[7..10], &input[9..13]),
            (&input[7..7], close),
            (open, &input[13..13]),
        ] {
            assert!(Enclosure::new(source, open, close).is_err());
        }
    }
}
