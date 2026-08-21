//! Word-internal PAIRED markers that bracket a stretch of a word.
//!
//! Named for the PARSE ROLE, not the provenance: one of these is a disfluency
//! mark rather than Conversation Analysis notation. Ask `notation_family()`.
//!
//! CHAT reference anchors:
//! - [CA Subwords](https://talkbank.org/0info/manuals/CHAT.html#CA_Subwords)
//! - [CA Delimiters](https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters)

use crate::model::WriteChat;
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorSink, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

// CADelimiterType is GENERATED from spec/symbols/symbol_registry.json; what stays
// here is what the registry cannot express: validation and serialization.
pub use crate::generated::ca_symbols::CADelimiterType;

/// One CA delimiter token used to bound a prosodic region.
///
/// # Structure
///
/// A CA delimiter consists of:
/// - **type**: The kind of prosodic modification (rate, volume, voice quality)
/// - **span**: Optional source location information
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want ∆that∆ .                  # Faster speech
/// *MOT: °okay° .                         # Softer speech
/// *CHI: ∬thank you∬ .                    # Smile voice
/// *INV: ∇very slow∇ .                    # Slower speech
/// ```
///
/// # Delimiter Pairing
///
/// CA delimiters should be balanced within an utterance:
/// ```text
/// ∆fast∆             # ✅ Balanced
/// ∆fast              # ❌ Unbalanced - validation error E230
/// °soft∆             # ❌ Mismatched - validation error E230
/// ```
///
/// # Usage
///
/// ```rust
/// use talkbank_model::{CADelimiter, CADelimiterType};
///
/// let faster = CADelimiter::new(CADelimiterType::Faster);
/// let softer = CADelimiter::new(CADelimiterType::Softer);
/// ```
///
/// # References
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
/// - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
pub struct CADelimiter {
    /// Delimiter variant.
    pub delimiter_type: CADelimiterType,
    /// Optional source location metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<Span>,
}

impl CADelimiter {
    /// Build a CA delimiter token with no span metadata.
    ///
    /// Parser paths generally call this first and attach spans later if source
    /// tracking is available.
    pub fn new(delimiter_type: CADelimiterType) -> Self {
        Self {
            delimiter_type,
            span: None,
        }
    }

    /// Attach source span metadata.
    ///
    /// Spans are optional and only affect diagnostics, never semantic equality.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl WriteChat for CADelimiter {
    /// Writes the exact Unicode symbol for this CA delimiter.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.delimiter_type.to_symbol())
    }
}

impl Validate for CADelimiter {
    /// Pairing/balance constraints are validated at utterance-level CA checks.
    ///
    /// Single delimiter tokens are structurally valid on their own; only
    /// cross-token pairing state determines delimiter errors.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}
