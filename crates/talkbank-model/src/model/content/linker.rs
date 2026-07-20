//! Utterance-linker tokens (`+<`, `++`, `+^`, ...).
//!
//! Linkers are modeled separately from terminators because they describe how a
//! turn connects to surrounding turns, not how it ends prosodically.
//!
//! A linker is a real source token, so it carries its byte [`Span`] like every
//! other token in the model. The classification (which linker) lives in
//! [`LinkerKind`]; [`Linker`] pairs that kind with its position. The span is
//! provenance only: it is skipped in serialization, JSON schema, and semantic
//! equality, so the wire format is unchanged and two linkers of the same kind
//! at different positions compare semantically equal.
//!
//! # CHAT Format References
//!
//! - [Utterance Linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
//! - [Lazy Overlap Linker](https://talkbank.org/0info/manuals/CHAT.html#LazyOverlap_Linker)

use super::WriteChat;
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Which cross-utterance linker a token is (its classification, not its
/// position).
///
/// Linkers appear at the start of an utterance to indicate its relationship
/// to the previous utterance(s). The serialized form (`{"type": "..."}`) is
/// the stable wire contract; [`Linker`] is transparent over this type, so the
/// JSON of a linker is exactly this enum's tagged representation.
///
/// # CHAT Format Examples
///
/// ```text
/// *MOT: are you ready ?
/// *CHI: +< yes .          Lazy overlap (started before previous finished)
/// *MOT: what do you want ?
/// *CHI: ++ cookie !       Quick uptake (no gap)
/// *MOT: she said +".      Quotation follows
/// *CHI: I'm hungry +".
/// ```
///
/// # References
///
/// - [Utterance Linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkerKind {
    /// `+<` lazy-overlap-precedes linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#LazyOverlap_Linker>
    LazyOverlapPrecedes,
    /// `++` other-completion linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
    /// Note: Serialized as "quick_uptake" for backward compatibility with existing JSON.
    #[serde(rename = "quick_uptake")] // Keep old serialization name for backward compatibility
    OtherCompletion,
    /// `+^` quick-uptake-overlap linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#QuickUptake_Linker>
    QuickUptakeOverlap,
    /// `+"` quotation-follows linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
    QuotationFollows,
    /// `+,` self-completion linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
    SelfCompletion,
    /// `+≋` TCU continuation linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TCU_Continuation_Linker>
    TcuContinuation,
    /// `+≈` no-break TCU continuation linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TCU_NoBreak_Linker>
    NoBreakTcuContinuation,
}

impl WriteChat for LinkerKind {
    /// Serializes the canonical CHAT token for this linker kind.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            LinkerKind::LazyOverlapPrecedes => w.write_str("+<"),
            LinkerKind::OtherCompletion => w.write_str("++"),
            LinkerKind::QuickUptakeOverlap => w.write_str("+^"),
            LinkerKind::QuotationFollows => w.write_str("+\""),
            LinkerKind::SelfCompletion => w.write_str("+,"),
            LinkerKind::TcuContinuation => w.write_str("+\u{224B}"),
            LinkerKind::NoBreakTcuContinuation => w.write_str("+\u{2248}"),
        }
    }
}

/// A cross-utterance linker token with its source position.
///
/// Pairs the linker [`LinkerKind`] with the byte [`Span`] it occupied in the
/// source. The span is provenance only (`#[serde(skip)]`, `#[schemars(skip)]`,
/// `#[semantic_eq(skip)]`): the serialized form is transparently that of
/// [`LinkerKind`], so the JSON and schema are unchanged by carrying a span, and
/// semantic equality ignores position.
#[derive(
    Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct Linker {
    /// Which linker this token is.
    pub kind: LinkerKind,
    /// Source byte span of the linker token. Provenance only: skipped in
    /// serialization, schema, and semantic equality. Used by source-spacing
    /// validation (E758) to detect a leading space before the linker.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub span: Span,
}

impl Linker {
    /// Builds a linker of `kind` at `span`.
    pub fn new(kind: LinkerKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl From<LinkerKind> for Linker {
    /// Wraps a kind with a dummy span, for programmatic construction (tests,
    /// builders) where the source position is not meaningful.
    fn from(kind: LinkerKind) -> Self {
        Self {
            kind,
            span: Span::DUMMY,
        }
    }
}

impl WriteChat for Linker {
    /// Serializes the canonical CHAT token for this linker.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.kind.write_chat(w)
    }
}

impl std::fmt::Display for Linker {
    /// Formats this linker using its CHAT token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips the lazy-overlap-precedes linker (`+<`).
    #[test]
    fn linker_lazy_overlap_precedes_roundtrip() {
        let linker = Linker::from(LinkerKind::LazyOverlapPrecedes);
        assert_eq!(linker.to_string(), "+<", "Linker +< roundtrip failed");
    }

    /// Round-trips the other-completion linker (`++`).
    #[test]
    fn linker_other_completion_roundtrip() {
        let linker = Linker::from(LinkerKind::OtherCompletion);
        assert_eq!(
            linker.to_string(),
            "++",
            "Linker ++ (other completion) roundtrip failed"
        );
    }

    /// Round-trips the quick-uptake-overlap linker (`+^`).
    #[test]
    fn linker_quick_uptake_overlap_roundtrip() {
        let linker = Linker::from(LinkerKind::QuickUptakeOverlap);
        assert_eq!(linker.to_string(), "+^", "Linker +^ roundtrip failed");
    }

    /// Round-trips the self-completion linker (`+,`).
    #[test]
    fn linker_self_completion_roundtrip() {
        let linker = Linker::from(LinkerKind::SelfCompletion);
        assert_eq!(linker.to_string(), "+,", "Linker +, roundtrip failed");
    }

    /// Round-trips the quotation-follows linker (`+"`).
    #[test]
    fn linker_quotation_follows_roundtrip() {
        let linker = Linker::from(LinkerKind::QuotationFollows);
        assert_eq!(linker.to_string(), "+\"", "Linker +\" roundtrip failed");
    }

    /// The span is provenance only: two linkers of the same kind at different
    /// positions are semantically equal.
    #[test]
    fn span_is_not_semantic() {
        use crate::model::SemanticEq;
        let a = Linker::new(LinkerKind::SelfCompletion, Span::new(3, 5));
        let b = Linker::new(LinkerKind::SelfCompletion, Span::new(40, 42));
        assert!(a.semantic_eq(&b), "linker span must not affect semantic eq");
    }
}
