//! Word category prefixes (`0`, `&~`, `&-`, `&+`) for non-canonical lexical tokens.
//!
//! CHAT reference anchors:
//! - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
//! - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)

use crate::model::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Prefix category for non-canonical lexical tokens.
///
/// Categories encode omission/filler/nonword/fragment forms that change lexical
/// interpretation before morphological analysis.
///
/// # CHAT Format Examples
///
/// ```text
/// 0is               Omitted word (0)
/// 0det the          Omitted determiner
/// &~gaga            Nonword/babbling (&~)
/// &-uh              Filler (&-)
/// &-um              Filler
/// &+fr              Phonological fragment (&+)
/// &+w               Fragment starting with 'w'
/// ```
///
/// # Important Distinction
///
/// - `0` alone = Action (see [`crate::model::Action`])
/// - `0word` = Omitted word (this category)
///
/// # References
///
/// - [Omitted Words](https://talkbank.org/0info/manuals/CHAT.html#Omitted_Words)
/// - [Filler Code](https://talkbank.org/0info/manuals/CHAT.html#Filler_Code)
/// - [Fragments](https://talkbank.org/0info/manuals/CHAT.html#Fragments)
/// - [Nonwords](https://talkbank.org/0info/manuals/CHAT.html#Nonwords)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(rename_all = "lowercase")]
pub enum WordCategory {
    /// `0` - Omitted word (e.g., `0is`, `0det`)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Omitted_Words>
    Omission,
    /// `(word)` - CA-style omission (standalone shortening)
    ///
    /// In CA mode (`@Options: CA`), `(word)` represents an omitted or uncertain word,
    /// semantically equivalent to `0word` in standard CHAT format.
    ///
    /// # Serialization
    ///
    /// Unlike `Omission` which serializes with a `0` prefix, `CAOmission` serializes
    /// as a parenthesized word `(word)` at the word level.
    ///
    /// # Validation
    ///
    /// In non-CA mode, `(word)` alone (without following text) is a validation error.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CA.html>
    #[serde(rename = "ca_omission")]
    CAOmission,
    /// `&~` - Nonword/babbling (e.g., `&~gaga`)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Nonwords>
    Nonword,
    /// `&-` - Filler (e.g., `&-uh`, `&-um`)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Filler_Code>
    Filler,
    /// `&+` - Phonological fragment (e.g., `&+fr`, `&+w`)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Fragments>
    PhonologicalFragment,
}

impl WordCategory {
    /// Returns canonical CHAT prefix text for this category.
    ///
    /// Note: `CAOmission` returns empty string because the parentheses are handled
    /// at the word level, not as a prefix. This method only covers prefix-bearing
    /// categories and is intentionally serialization-focused.
    pub fn to_chat_prefix(&self) -> &'static str {
        match self {
            WordCategory::Omission => "0",
            WordCategory::CAOmission => "", // No prefix - handled as shortening content
            WordCategory::Nonword => "&~",
            WordCategory::Filler => "&-",
            WordCategory::PhonologicalFragment => "&+",
        }
    }

    /// What this category's letters ARE: spelling, or a rendering of sound.
    ///
    /// # Why this is a type and not four hand-written matches
    ///
    /// Four places matched a subset of this enum to answer some version of the
    /// question, and they did not agree. `validation/utterance/spacing.rs` and
    /// `alignment/helpers/rules.rs::is_fragment_like` each listed the same
    /// three `&` categories under a different name; `validation/word/structure.rs`
    /// listed them a third time to exempt them from E241; and
    /// `Word::compute_untranscribed` listed none of them, so a phonological
    /// fragment spelled `xxx` was classified as untranscribed material.
    ///
    /// The disagreement is what makes it a type. A lexical rule asking "may I
    /// judge these letters as a word?" has one right answer, and it should not
    /// be re-derived by each rule that asks.
    ///
    /// Exhaustive with no catch-all, so a sixth category has to answer this at
    /// compile time rather than inheriting whichever branch was the fallback.
    #[must_use]
    pub fn material(&self) -> WordMaterial {
        match self {
            // A word that was not spoken is still a WORD: `0dog` and `(dog)`
            // are ordinary orthography with a note attached, so a misspelling
            // inside one is still a misspelling.
            Self::Omission | Self::CAOmission => WordMaterial::Orthography,
            // `&~`, `&-`, `&+`: a transcriber's rendering of a noise. The
            // letters approximate a sound and are not a spelling of anything.
            Self::Nonword | Self::Filler | Self::PhonologicalFragment => WordMaterial::Sound,
        }
    }

    /// Return `true` for omission categories in either CHAT style.
    ///
    /// This unifies standard omission (`0word`) and CA-style parenthesized
    /// omission (`(word)`) for validators that care about omission semantics.
    pub fn is_omission(&self) -> bool {
        matches!(self, WordCategory::Omission | WordCategory::CAOmission)
    }
}

/// What a word's letters are: spelling, or a rendering of sound.
///
/// The distinction every lexical rule needs and none of them owned. A rule that
/// judges orthography is meaningless applied to [`Self::Sound`]: those letters
/// were chosen to approximate a noise, so checking them against a vocabulary
/// reports a defect in the transcriber's ear.
///
/// # What actually consults this, as of 2026-08-15
///
/// E241 (`validation/word/structure.rs`) and the `%mor` fragment filter
/// (`alignment/helpers/rules.rs`). TWO sites. This paragraph exists because the
/// first draft named three rules as though they all did, which is the prose rot
/// this repository treats as the worst kind: nothing gates it.
///
/// Three more ask the same question and answer it differently, none of the
/// differences adjudicated: E220 (`validation/word/language/digits.rs`) and E763
/// (`validation/word/prefix_marker.rs`) exempt `Omission` ONLY, and
/// `Word::compute_untranscribed` exempts nothing, so `&+xxx` still reads as
/// untranscribed material. Converting them changes validation output in BOTH
/// directions (E220 would start judging `0dog2` and stop judging `&+3`) and owes
/// a corpus differential. Until that runs this type is deliberately not
/// universal, and saying so is cheaper than letting a reader infer from its
/// existence that the question is settled.
///
/// A sum type rather than a `bool` named `is_orthography`, so a call site reads
/// as a match on what the material IS rather than on a flag whose polarity the
/// reader has to remember.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordMaterial {
    /// Words as written. Lexical rules apply.
    Orthography,
    /// A rendering of sound. Lexical rules do not apply.
    Sound,
}

impl WriteChat for WordCategory {
    /// Writes the category prefix that precedes the lexical word body.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.to_chat_prefix())
    }
}
