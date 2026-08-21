//! Word-internal markers that attach to a word token.
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

// CAElementType is GENERATED from spec/symbols/symbol_registry.json; what stays
// here is what the registry cannot express: validation and serialization.
pub use crate::generated::ca_symbols::{CAElementType, NotationFamily};

/// One marker token that attaches to a word rather than bracketing a stretch.
///
/// # Structure
///
/// A CA element consists of:
/// - **type**: which marker, from [`CAElementType`]
/// - **span**: Optional source location information
///
/// # Not all of these are Conversation Analysis notation
///
/// The category name describes the PARSE ROLE (attaches to a word) and not
/// the provenance. `BlockedSegments` (`≠`) is a disfluency mark from the CHAT
/// manual's Disfluency Transcription chapter, a word attack rather than a
/// prosodic feature, and CLAN names it `NOTCA_CROSSED_EQUAL` for that reason.
/// It lives here because it attaches to a word exactly as `↑` does.
///
/// # CHAT Format Examples
///
/// Every example here is a variant of [`CAElementType`]. The stress markers
/// `ˈ` and `ˌ` are NOT (they are `WordContent::StressMarker`), so an example
/// using one cannot be built as a `CAElement`.
///
/// ```text
/// *CHI: ↑hello .                         # Shift to high pitch
/// *CHI: I ↓know .                        # Shift to low pitch
/// *INV: ≠wait .                          # Blocking, a disfluency word attack
/// *PAR: swi≠mming .                      # Blocking inside a word
/// ```
///
/// # Usage
///
/// ```rust
/// use talkbank_model::{CAElement, CAElementType};
///
/// let pitch_up = CAElement::new(CAElementType::PitchUp);
/// let pitch = CAElement::new(CAElementType::PitchUp);
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
pub struct CAElement {
    /// Marker variant.
    pub element_type: CAElementType,
    /// Optional source location metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<Span>,
}

impl CAElement {
    /// Build a CA element token with no span metadata.
    ///
    /// Parser paths generally call this first and attach spans later if source
    /// tracking is available.
    pub fn new(element_type: CAElementType) -> Self {
        Self {
            element_type,
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

impl WriteChat for CAElement {
    /// Writes the exact Unicode symbol for this CA element.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.element_type.to_symbol())
    }
}

impl Validate for CAElement {
    /// Element-level constraints are validated by higher-level word structure checks.
    ///
    /// A single CA element token has no independent balance constraints, so
    /// this validator is intentionally a no-op.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}
