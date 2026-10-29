//! Source-admitted action and event leaves of native marker chains.

use crate::source_text::SourceSlice;
use serde::Serialize;
use talkbank_model::{Event, EventType, Span};

/// An event whose source extent cannot be absent or changed during folding.
/// The descriptor uses the shared model's checked vocabulary type.
#[derive(Clone, Debug, Serialize)]
pub struct ParsedEvent<'a> {
    event_type: EventType,
    #[serde(skip)]
    source: SourceSlice<'a>,
}

impl<'a> ParsedEvent<'a> {
    pub(crate) fn new(source: SourceSlice<'a>, event_type: EventType) -> Self {
        Self { event_type, source }
    }

    /// The admitted event descriptor.
    pub fn event_type(&self) -> &EventType {
        &self.event_type
    }

    /// The complete event's retained source extent.
    pub fn span(&self) -> Span {
        self.source.span()
    }

    /// Lower to the shared model without another source lookup.
    pub fn to_model(&self) -> Event {
        Event::new(self.span(), self.event_type.clone())
    }
}

// Match the previous shared Event equality: descriptor and located extent.
impl PartialEq for ParsedEvent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.event_type == other.event_type && self.span() == other.span()
    }
}

/// An observed standalone `0` action, with its admitted source extent.
#[derive(Clone, Copy, Debug)]
pub struct ZeroAction<'a>(SourceSlice<'a>);

/// A purported zero action does not carry the standalone zero spelling.
#[derive(Debug)]
pub(crate) struct RefusedZeroAction;

impl<'a> ZeroAction<'a> {
    pub(crate) fn new(source: SourceSlice<'a>) -> Result<Self, RefusedZeroAction> {
        if source.text() == "0" {
            Ok(Self(source))
        } else {
            Err(RefusedZeroAction)
        }
    }

    /// The exact standalone zero spelling.
    pub fn lexeme(self) -> &'a str {
        self.0.text()
    }

    /// Coordinates retained at source admission.
    pub fn span(self) -> Span {
        self.0.span()
    }
}

impl PartialEq for ZeroAction<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.lexeme() == other.lexeme()
    }
}

impl Serialize for ZeroAction<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.lexeme())
    }
}
