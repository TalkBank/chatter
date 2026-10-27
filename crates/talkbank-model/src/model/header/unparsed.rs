//! Explicit preservation of a header source line that could not be parsed.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

/// Which language-header boundary rejected the observed source.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(rename_all = "snake_case")]
pub enum LanguageHeaderFailure {
    /// The header envelope could not be parsed.
    Syntax,
    /// The header was recognized but its ordered language list was rejected.
    List,
}

/// Unparsed language-header source, retained in its original header position.
///
/// This is always unparsed data, including after JSON deserialization.
/// It contains the observed header source, including its prefix and any line
/// ending owned by the parser span,
/// not invented codes or other successful payload. Consumers needing parsed
/// content must use the corresponding parsed `Header` variant instead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct UnparsedLanguageHeader {
    source: SmolStr,
    failure: LanguageHeaderFailure,
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub(crate) location: crate::Location,
}

impl UnparsedLanguageHeader {
    /// Retain observed source as unparsed data, never as parsed content.
    pub fn new(
        location: impl Into<crate::Location>,
        source: impl Into<SmolStr>,
        failure: LanguageHeaderFailure,
    ) -> Self {
        Self {
            source: source.into(),
            failure,
            location: location.into(),
        }
    }

    /// Observed header source, including its prefix and any owned line ending.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The failed admission boundary, retained through JSON.
    pub fn failure(&self) -> LanguageHeaderFailure {
        self.failure
    }

    /// Location of the retained source, if known.
    pub fn location(&self) -> crate::Location {
        self.location
    }
}

impl super::WriteChat for UnparsedLanguageHeader {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        // The enclosing file writes the line ending. Remove only one
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

/// Unparsed participant-header source, retained in its original header position.
///
/// This is always unparsed data, including after JSON deserialization.
/// It contains the observed header source, including its prefix and any line
/// ending owned by the parser span,
/// not invented speaker identities or other successful payload. Consumers needing parsed
/// content must use the corresponding parsed `Header` variant instead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(deny_unknown_fields)]
pub struct UnparsedParticipantHeader {
    source: SmolStr,
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub(crate) location: crate::Location,
}

impl UnparsedParticipantHeader {
    /// Retain observed source as unparsed data, never as parsed content.
    pub fn new(location: impl Into<crate::Location>, source: impl Into<SmolStr>) -> Self {
        Self {
            source: source.into(),
            location: location.into(),
        }
    }

    /// Observed header source, including its prefix and any owned line ending.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Location of the retained source, if known.
    pub fn location(&self) -> crate::Location {
        self.location
    }
}

impl super::WriteChat for UnparsedParticipantHeader {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        // The enclosing file writes the line ending. Remove only one
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

/// Unparsed ID-header source, retained in its original header position.
///
/// This is always unparsed data, including after JSON deserialization.
/// It contains the observed header source, including its prefix and any line
/// ending owned by the parser span,
/// not invented demographic fields or other successful payload. Consumers needing parsed
/// content must use the corresponding parsed `Header` variant instead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(deny_unknown_fields)]
pub struct UnparsedIDHeader {
    source: SmolStr,
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub(crate) location: crate::Location,
}

impl UnparsedIDHeader {
    /// Retain observed source as unparsed data, never as parsed content.
    pub fn new(location: impl Into<crate::Location>, source: impl Into<SmolStr>) -> Self {
        Self {
            source: source.into(),
            location: location.into(),
        }
    }

    /// Observed header source, including its prefix and any owned line ending.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Location of the retained source, if known.
    pub fn location(&self) -> crate::Location {
        self.location
    }
}

impl super::WriteChat for UnparsedIDHeader {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        // The enclosing file writes the line ending. Remove only one
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
