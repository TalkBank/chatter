//! An annotation retained without the content it would scope over.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Observed annotation source with no admitted host.
///
/// This is always invalid recovery, even after JSON import. Its original
/// delimiters are retained; it is never rewritten as an authored freecode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct OrphanAnnotation {
    source: smol_str::SmolStr,
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    location: crate::Location,
}

impl OrphanAnnotation {
    /// Retain observed source as an annotation without a host.
    pub fn new(location: impl Into<crate::Location>, source: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            source: source.into(),
            location: location.into(),
        }
    }

    /// Complete annotation spelling, including its delimiters.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Original source location, when available.
    pub fn location(&self) -> crate::Location {
        self.location
    }

    /// Report retained invalidity without depending on source coordinates.
    pub(crate) fn validate_recovery(&self, errors: &impl crate::ErrorSink) {
        errors.report(crate::ParseError::at_location(
            crate::ErrorCode::AnnotationAtUtteranceStart,
            crate::Severity::Error,
            self.location,
            "Annotation has no preceding content to scope over",
        ));
    }
}

impl super::WriteChat for OrphanAnnotation {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(&self.source)
    }
}
