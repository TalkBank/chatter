//! Scoped annotation model types (`[*]`, `[=]`, retracing, overlaps, and related markers).
//!
//! These types capture the parser's normalized representation of CHAT scoped
//! symbols so validation and serialization can operate on a closed enum instead
//! of stringly marker handling.
//!

use crate::LanguageCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Scoped annotation that modifies or provides information about speech content.
///
/// Scoped annotations in CHAT format are enclosed in square brackets and provide
/// contextual information about errors, clarifications, overlaps, and repetitions.
///
/// # Annotation Types
///
/// - **Error marking** (`[*]`, `[* code]`): Indicates speech errors, grammatical mistakes,
///   or phonological errors that need correction or special attention.
///
/// - **Explanations** (`[= text]`): Clarifies unintelligible speech, unusual pronunciations,
///   or ambiguous utterances. Often used with `xxx` for unintelligible material.
///
/// - **Retracing** (`[/]`, `[//]`, `[///]`): Marks self-corrections and repeated words.
///   Single `/` for partial repetition, double for full retracing, triple for multiple.
///
/// - **Overlaps** (`[<]`, `[>]`): Marks simultaneous speech by different speakers.
///   `[<]` at overlap start, `[>]` at overlap end.
///
/// # CHAT Manual Reference
///
/// - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
/// - [Explanation Scope](https://talkbank.org/0info/manuals/CHAT.html#Explanation_Scope)
/// - [Retracing](https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition)
/// - [Overlap Precedes Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope)
/// - [Overlap Follows Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{ContentAnnotation, ScopedError, ScopedExplanation};
///
/// // Error marking
/// let error = ContentAnnotation::Error(ScopedError { code: Some("grammar".into()) });
///
/// // Explanation
/// let explanation = ContentAnnotation::Explanation(ScopedExplanation {
///     text: "probably said ball".into()
/// });
/// ```
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentAnnotation {
    /// Error marking (`[*]` or `[* code]`).
    ///
    /// Marks speech errors, grammatical mistakes, or phonological errors.
    /// Optional error code specifies the type of error (e.g., "grammar", "phonology").
    ///
    /// **Examples:**
    /// - `[*]` - Generic error marker
    /// - `[* grammar]` - Grammatical error
    /// - `[* phonology]` - Phonological error
    ///
    /// See: [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
    Error(ScopedError),

    /// Explanation (`[= text]`).
    ///
    /// Clarifies unclear or unintelligible speech. Commonly used with `xxx` to explain
    /// what was likely said when the actual utterance is unintelligible.
    ///
    /// **Examples:**
    /// - `xxx [= probably ball]`
    /// - `doggie [= referring to cat]`
    ///
    /// See: [Explanation Scope](https://talkbank.org/0info/manuals/CHAT.html#Explanation_Scope)
    Explanation(ScopedExplanation),

    /// Overlap beginning marker (`[<]`).
    ///
    /// Marks the point where simultaneous speech begins. Used when two or more
    /// speakers talk at the same time.
    ///
    /// **Example:**
    /// ```text
    /// *CHI: I want [<] that .
    /// *MOT: you want [>] what ?
    /// ```
    ///
    /// See: [Overlap Precedes Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope)
    #[serde(rename = "overlap_begin")]
    OverlapBegin(ScopedOverlapBegin),

    /// Overlap ending marker (`[>]`).
    ///
    /// Marks the point where simultaneous speech ends.
    ///
    /// See: [Overlap Follows Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope)
    #[serde(rename = "overlap_end")]
    OverlapEnd(ScopedOverlapEnd),

    /// Scoped stressing marker (`[!]`).
    ///
    /// Marks emphatic stress or emphasis on preceding word/phrase.
    ///
    /// **Example:** `that [!]` - emphatic stress
    ///
    /// See: [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
    Stressing,

    /// Scoped contrastive stressing (`[!!]`).
    ///
    /// Marks strong contrastive stress.
    ///
    /// **Example:** `mine [!!]` - strong contrastive stress
    ///
    /// See: [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
    ContrastiveStressing,

    /// Scoped uncertain (`[?]`).
    ///
    /// Marks uncertain or unclear transcription.
    ///
    /// **Example:** `doggie [?]` - uncertain transcription
    ///
    /// See: [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
    Uncertain,

    /// Paralinguistic annotation (`[=! text]`).
    ///
    /// Describes paralinguistic features like whispering, laughing, etc.
    ///
    /// **Example:** `hello [=! whispers]`
    ///
    /// See: [Paralinguistic Material Scope](https://talkbank.org/0info/manuals/CHAT.html#ParalinguisticMaterial_Scope)
    Paralinguistic(ScopedParalinguistic),

    /// Alternative transcription (`[=? text]`).
    ///
    /// Provides alternative interpretation or uncertain transcription.
    ///
    /// **Example:** `xxx [=? maybe ball]`
    ///
    /// See: [Alternative Transcription Scope](https://talkbank.org/0info/manuals/CHAT.html#AlternativeTranscription_Scope)
    Alternative(ScopedAlternative),

    /// Percent annotation (`[% text]`).
    ///
    /// General comment or note about the utterance.
    ///
    /// **Example:** `hey [% comment about context]`
    ///
    /// See: [Comment Scope](https://talkbank.org/0info/manuals/CHAT.html#Comment_Scope)
    PercentComment(ScopedPercentComment),

    /// Exclude marker (`[e]`).
    ///
    /// Marks content to be excluded from analysis.
    ///
    /// See: [Excluded Material](https://talkbank.org/0info/manuals/CHAT.html#MorExclude_Scope)
    Exclude,

    /// Code-switch span (`[@s]`, `[@s:lang]`).
    ///
    /// Every word in the annotated `<...>` scope takes the switched language,
    /// exactly as if each carried the `@s` / `@s:lang` word suffix. The span is
    /// a main-tier construct only; dependent tiers stay word-aligned and gain
    /// nothing new from it.
    ///
    /// **Example:** `ik weet niet <how to do it> [@s] .`
    CodeSwitch(CodeSwitchSpan),

    /// Unknown annotation (lenient parsing).
    ///
    /// Captures annotations with unrecognized markers. This allows the parser
    /// to accept all CHAT files while flagging unusual annotations for review.
    Unknown(ScopedUnknown),
}

/// Which language a [`ContentAnnotation::CodeSwitch`] span switches to.
///
/// Two variants rather than an `Option<LanguageCode>`, because the bare form is
/// not a MISSING code: it is its own resolution rule, the same one bare
/// `word@s` uses. An `Option` would invite a caller to treat `None` as "no
/// language" and fall through to the default, which is the opposite of what the
/// bare form means.
///
/// The span is deliberately single-language. `WordLanguageMarker` additionally
/// carries `Multiple` and `Ambiguous`; a span is homogeneous by construction,
/// so those states are not representable here rather than being rejected later.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, SemanticEq, SpanShift, JsonSchema,
)]
// ADJACENTLY tagged (`tag` + `content`), matching `WordLanguageMarker`, and
// the pairing is load-bearing rather than cosmetic. An INTERNALLY tagged enum
// (`tag` alone) cannot serialize a newtype variant whose payload is a string,
// and serde reports that only at RUNTIME, when a document containing one is
// written. So `[@s]` round-tripped through JSON while `[@s:hin]` failed, and
// the compiler had nothing to say about it. The committed JSON Schema was no
// help either: it described the explicit variant via an `allOf` workaround for
// a shape the serializer could never actually emit.
//
// The general shape, for the next enum to gain a payload-carrying variant: a
// serde container attribute is a claim about every variant, checked against
// none of them until one is written. When adding a variant with a payload to a
// tagged enum, serialize a value of it.
#[serde(tag = "kind", content = "code", rename_all = "snake_case")]
pub enum CodeSwitchSpan {
    /// `[@s]`: resolves the way a bare `word@s` does.
    ///
    /// With two declared languages that is the non-primary one. With more, it
    /// resolves to the SECOND declared language, unless the current language is
    /// itself tertiary, in which case it is left unresolved with a diagnostic
    /// asking for an explicit code. It never reports `Ambiguous`; only
    /// `@s:eng&spa` produces that.
    Shortcut,

    /// `[@s:lang]`: names the code directly.
    ///
    /// Deliberately NOT required to appear in `@Languages`, matching the
    /// word-level `@s:code` ruling of 2026-07-15: that header declares a
    /// transcript's substantial languages, and an embedded insertion is not
    /// substantial presence. The code must still be a real language, which
    /// registry validation checks.
    Explicit(LanguageCode),
}

/// Error marking data for `[*]` or `[* code]` annotations.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Error_Coding>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedError {
    /// Optional error type code
    pub code: Option<smol_str::SmolStr>,
}

/// Explanation data for `[= text]` annotations.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Scope>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedExplanation {
    /// Explanatory text
    pub text: smol_str::SmolStr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, JsonSchema, SemanticEq, SpanShift)]
/// Numeric index (1-9) for distinguishing multiple overlaps in a single utterance.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope>
/// - <https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope>
#[serde(transparent)]
pub struct OverlapMarkerIndex(#[schemars(range(min = 1, max = 9))] u8);

/// A scoped overlap index outside the single-digit range 1–9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("scoped overlap index {0} must be a single digit from 1 to 9")]
pub struct InvalidOverlapMarkerIndex(u8);

impl OverlapMarkerIndex {
    /// Admit only the scoped overlap indices 1–9.
    pub fn new(index: u8) -> Result<Self, InvalidOverlapMarkerIndex> {
        match index {
            1..=9 => Ok(Self(index)),
            _ => Err(InvalidOverlapMarkerIndex(index)),
        }
    }

    /// The digit payload.
    pub fn get(self) -> u8 {
        self.0
    }
}

impl std::fmt::Display for OverlapMarkerIndex {
    /// Formats the stored overlap index digit (`1`-`9`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for OverlapMarkerIndex {
    /// Wire input crosses the same range-checked admission boundary.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(u8::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod overlap_index_boundaries {
    use super::OverlapMarkerIndex;

    /// Numeric construction and untrusted wire input share exactly one range.
    #[test]
    fn admission_and_wire_range() {
        for value in u8::MIN..=u8::MAX {
            let admitted = OverlapMarkerIndex::new(value);
            let decoded = serde_json::from_str::<OverlapMarkerIndex>(&value.to_string());
            assert_eq!(admitted.is_ok(), (1..=9).contains(&value));
            assert_eq!(decoded.is_ok(), admitted.is_ok());
            if let Ok(index) = admitted {
                assert_eq!(index.get(), value);
                assert_eq!(decoded.unwrap(), index);
                assert_eq!(serde_json::to_string(&index).unwrap(), value.to_string());
            }
        }
        for invalid in ["-1", "256", "1.5", "null", "\"1\""] {
            assert!(serde_json::from_str::<OverlapMarkerIndex>(invalid).is_err());
        }
    }
}

/// Overlap begin marker data for `[<]` or `[<N]` annotations.
///
/// # Reference
///
/// - [Overlap precedes scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedOverlapBegin {
    /// Optional index for multiple overlaps (`[<1]`, `[<2]`, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<OverlapMarkerIndex>,
}

/// Overlap end marker data for `[>]` or `[>N]` annotations.
///
/// # Reference
///
/// - [Overlap follows scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedOverlapEnd {
    /// Optional index for multiple overlaps (`[>1]`, `[>2]`, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<OverlapMarkerIndex>,
}

/// Paralinguistic annotation data for `[=! text]`.
///
/// # Reference
///
/// - [Paralinguistic material scope](https://talkbank.org/0info/manuals/CHAT.html#ParalinguisticMaterial_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedParalinguistic {
    /// Description of paralinguistic feature
    pub text: smol_str::SmolStr,
}

/// Alternative transcription data for `[=? text]`.
///
/// # Reference
///
/// - [Alternative transcription scope](https://talkbank.org/0info/manuals/CHAT.html#AlternativeTranscription_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedAlternative {
    /// Alternative transcription text
    pub text: smol_str::SmolStr,
}

/// Percent comment data for `[% text]` annotations.
///
/// # Reference
///
/// - [Comment scope](https://talkbank.org/0info/manuals/CHAT.html#Comment_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedPercentComment {
    /// Comment text
    pub text: smol_str::SmolStr,
}

/// Unknown annotation captured during lenient parsing.
///
/// # Reference
///
/// - [Scoped symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedUnknown {
    /// The annotation marker (e.g., custom markers)
    pub marker: smol_str::SmolStr,
    /// The annotation text
    pub text: smol_str::SmolStr,
}

impl ScopedUnknown {
    /// The E207 diagnostic message for this annotation.
    ///
    /// # One owner, because there are two emitters and they had drifted
    ///
    /// `AnnotatedContentAnnotations::report_unknown_markers` and
    /// `ReplacedWordAnnotations::validate` both report `UnknownAnnotation`, and
    /// each built its own sentence. Both said `"{marker}" is not a known scoped
    /// annotation type`, which the parser does not know: an annotation reaches
    /// `Unknown` whenever no specific rule matched it WHOLE, which happens both
    /// when the marker is genuinely unknown (`[qq]`, `[@ xyz]`) and when a
    /// KNOWN marker carries content the rule refuses. Under `--parser=re2c`,
    /// whose rule set is narrower, `[x 0]` and `[:]` land there, and the
    /// message then denied that `x` and `:` are known types. Both are.
    ///
    /// The replacement copy went further and appended an INVENTORY, "known
    /// types are *, =, +, <, >, //, ///", which omitted `x`, `:`, `!`, `?`,
    /// `%`, `-` and `e`, and printed on a word whose `[: cat]` had just parsed
    /// successfully: a list asserting `:` is not known, beside a demonstration
    /// that it is. An inventory in a message is a copy nothing checks, so
    /// there is none here.
    ///
    /// Naming the annotation AS WRITTEN is true in every case and shows more
    /// than the marker alone, which is the half the reader can already see.
    #[must_use]
    pub fn unreadable_message(&self) -> String {
        if self.text.is_empty() {
            format!("could not read [{}] as a scoped annotation", self.marker)
        } else {
            format!(
                "could not read [{} {}] as a scoped annotation",
                self.marker, self.text
            )
        }
    }
}
