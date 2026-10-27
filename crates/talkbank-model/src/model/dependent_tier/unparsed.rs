//! Explicit preservation of a dependent-tier source line that could not be parsed.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

/// Unparsed dependent-tier source, retained in its original dependent-tier position.
///
/// This is always unparsed data, including after JSON deserialization.
/// It contains the complete observed source line (prefix and ending included),
/// not invented words or a fallback terminator. Consumers needing parsed
/// content must use the corresponding parsed `DependentTier` variant instead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct UnparsedTier {
    source: SmolStr,
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub(crate) location: crate::Location,
}

impl UnparsedTier {
    /// Retain observed source as unparsed data, never as parsed content.
    pub fn new(location: impl Into<crate::Location>, source: impl Into<SmolStr>) -> Self {
        Self {
            source: source.into(),
            location: location.into(),
        }
    }

    /// Complete observed tier source, including its prefix and line ending.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Location of the retained source, if known.
    pub fn location(&self) -> crate::Location {
        self.location
    }
}

impl super::WriteChat for UnparsedTier {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        // The enclosing utterance writes the line ending. Remove only one
        // observed ending; preserve internal continuation and malformed bytes.
        let source = self.source();
        let content = match source.strip_suffix("\r\n") {
            Some(content) => content,
            None => match source.strip_suffix('\n') {
                Some(content) => content,
                None => match source.strip_suffix('\r') {
                    Some(content) => content,
                    None => source,
                },
            },
        };
        w.write_str(content)
    }
}

/// Rejected text-tier source with its observed label retained for tier identity.
/// This payload never certifies parsed text, including after JSON readback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct UnparsedTextTier {
    label: crate::NonEmptyString,
    raw: UnparsedTier,
}

impl UnparsedTextTier {
    /// Preserve an observed label and complete source as explicitly invalid text.
    pub fn new(label: crate::NonEmptyString, raw: UnparsedTier) -> Self {
        Self { label, raw }
    }

    /// Original tier label, without the percent marker or colon.
    pub fn label(&self) -> &str {
        self.label.as_str()
    }

    /// Original rejected tier source and location.
    pub fn raw(&self) -> &UnparsedTier {
        &self.raw
    }

    /// Re-express the retained source location without changing its contents.
    pub fn map_location(mut self, project: impl FnOnce(crate::Span) -> crate::Span) -> Self {
        self.raw.location = self.raw.location.map(project);
        self
    }
}

impl super::WriteChat for UnparsedTextTier {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        self.raw.write_chat(w)
    }
}

/// Whether retained recovery withdraws a particular alignment guarantee.
pub(crate) enum UnparsedRecovery<'a> {
    Aligned(crate::model::ParseHealthTier, &'a UnparsedTier),
    Text(&'a UnparsedTier),
}

impl<'a> UnparsedRecovery<'a> {
    pub(crate) fn raw(&self) -> &'a UnparsedTier {
        match self {
            Self::Aligned(_, raw) | Self::Text(raw) => raw,
        }
    }

    pub(crate) fn taint(&self, health: &mut crate::model::ParseHealthState) {
        match self {
            Self::Aligned(tier, _) => health.taint(*tier),
            Self::Text(_) => {}
        }
    }
}
