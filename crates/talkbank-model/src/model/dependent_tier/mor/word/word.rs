//! `%mor` word representation (`POS|lemma[-feature]*`) used in the morphological tier.
//!
//! CHAT reference anchors:
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
//! - [Grammatical relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use talkbank_derive::{SemanticEq, SpanShift};

use super::super::super::WriteChat;
use super::super::analysis::{MorFeature, MorStem, PosCategory};

/// POS string for the `L2|xxx` placeholder. BA2-equivalent fallback
/// when secondary-language morphology is unavailable.
const L2_PLACEHOLDER_POS: &str = "L2";
/// Lemma string for the `L2|xxx` placeholder. Pairs with
/// [`L2_PLACEHOLDER_POS`].
const L2_PLACEHOLDER_LEMMA: &str = "xxx";

/// Single morphological word in UD format.
///
/// A `MorWord` represents the complete morphological analysis of a single word,
/// consisting of a required POS tag and lemma, and optional feature chains.
///
/// # Structure
///
/// The format is: `POS|lemma[-Feature]*`
/// - **POS**: UD-style part-of-speech tag (e.g., `noun`, `verb`, `pron`, `det`)
/// - **Lemma**: Word lemma/base form (required)
/// - **Features**: UD morphological feature values separated by `-` (e.g., `-Plur`, `-Fin-Ind-Pres-S3`)
///
/// # CHAT Format Examples
///
/// Simple noun:
/// ```text
/// noun|dog
/// ```
///
/// Plural noun with features:
/// ```text
/// noun|dog-Plur
/// ```
///
/// Verb with multiple features:
/// ```text
/// verb|make-Part-Pres-S
/// ```
///
/// Auxiliary with complex features:
/// ```text
/// aux|be-Fin-Ind-Pres-S3
/// ```
///
/// # References
///
/// - [CHAT Manual: Morphological Tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct MorWord {
    /// UD-style part-of-speech tag (e.g., `noun`, `verb`, `pron`, `det`)
    pub pos: PosCategory,

    /// Word lemma/base form (e.g., `dog`, `be`, `I`)
    pub lemma: MorStem,

    /// Morphological feature values (e.g., `Plur`, `Fin`, `Ind`, `Pres`, `S3`)
    ///
    /// Uses SmallVec with inline capacity of 4 - most words have 0-4 features
    #[serde(skip_serializing_if = "SmallVec::is_empty", default)]
    #[schemars(with = "Vec<MorFeature>")]
    pub features: SmallVec<[MorFeature; 4]>,
}

impl MorWord {
    /// Return `true` for `%mor` items whose POS tag is one of the CHAT
    /// punctuation markers (`cm`, `punct`, `end`, `beg`).
    ///
    /// CLAN's analysis commands (`mlu`, `wdlen`, `vocd`, …) exclude
    /// these from morpheme / token counts even though they appear as
    /// real chunks on the `%mor` tier and on the `%gra` index. The
    /// mapping table in
    /// `crates/talkbank-model/src/model/dependent_tier/mor/analysis/clan_ud_mapping.rs`
    /// projects all four onto `PUNCT` in UD-space; we surface the
    /// same set here so callers can filter without re-reading the
    /// raw POS string.
    ///
    /// Important: this is for **counting only**. Do **not** use it
    /// to filter `Mor::count_chunks()` output, which addresses
    /// `%gra` chunks, punctuation tokens have a real GRA relation
    /// (`PUNCT`) and must be preserved at the chunk level.
    pub fn is_punctuation_marker(&self) -> bool {
        matches!(self.pos.as_str(), "cm" | "punct" | "end" | "beg")
    }

    /// Create a new morphological word with the given POS and lemma.
    ///
    /// Features start empty and can be layered in with builder helpers.
    /// Validation of lexical quality runs later at `%mor` tier validation time.
    pub fn new(pos: impl Into<PosCategory>, lemma: impl Into<MorStem>) -> Self {
        Self {
            pos: pos.into(),
            lemma: lemma.into(),
            features: SmallVec::new(),
        }
    }

    /// Append a morphological feature (e.g., `Plur`, `Past`).
    ///
    /// Feature order is preserved because serialization emits exactly the stored
    /// sequence after `POS|lemma`.
    pub fn with_feature(mut self, feature: impl Into<MorFeature>) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Replace all features.
    ///
    /// This is useful when callers already parsed a complete feature vector and
    /// want one assignment instead of repeated `with_feature` chaining.
    pub fn with_features(mut self, features: impl Into<SmallVec<[MorFeature; 4]>>) -> Self {
        self.features = features.into();
        self
    }

    /// Build the canonical `L2|xxx` placeholder used when secondary
    /// language morphology cannot be analyzed (or is rejected by the
    /// L2 splice's invariant guard), BA2-equivalent fallback shape.
    pub fn l2_placeholder() -> Self {
        Self::new(
            PosCategory::new(L2_PLACEHOLDER_POS),
            MorStem::new(L2_PLACEHOLDER_LEMMA),
        )
    }

    /// Reset this word to the `L2|xxx` placeholder in place. Used by
    /// the synthesis path and the L2 splice rollback path to demote a
    /// word to BA2-equivalent fallback morphology without reallocating
    /// the surrounding structure.
    pub fn reset_to_l2_placeholder(&mut self) {
        self.pos = PosCategory::new(L2_PLACEHOLDER_POS);
        self.lemma = MorStem::new(L2_PLACEHOLDER_LEMMA);
        self.features.clear();
    }

    /// Borrows the ANALYSIS half of this item: `lemma[-Feature]*`, with no
    /// `POS|` prefix.
    ///
    /// A `%mor` item is a tag and an analysis joined by `|`. Consumers whose
    /// own token model keeps the two apart (the tag in one field, the lemma
    /// and features in another) can render the second half directly rather
    /// than serializing the whole item and stripping the prefix back off.
    ///
    /// The returned view borrows, allocates nothing, and implements both
    /// [`Display`](std::fmt::Display) and [`WriteChat`], so it composes with a
    /// formatter or with a streaming writer.
    ///
    /// ```rust
    /// # use talkbank_model::model::{MorWord, MorFeature, MorStem, PosCategory, WriteChat};
    /// let word = MorWord::new(PosCategory::new("n"), MorStem::new("dog"))
    ///     .with_features(vec![MorFeature::new("PL")]);
    /// assert_eq!(word.to_chat_string(), "n|dog-PL");
    /// assert_eq!(word.analysis().to_chat_string(), "dog-PL");
    ///
    /// // With no features the analysis is the bare lemma, never an empty
    /// // string and never a trailing separator.
    /// let bare = MorWord::new(PosCategory::new("n"), MorStem::new("dog"));
    /// assert_eq!(bare.analysis().to_string(), "dog"); // Display agrees
    /// ```
    pub fn analysis(&self) -> MorAnalysis<'_> {
        MorAnalysis { word: self }
    }

    /// Serializes one `%mor` word as `POS|lemma[-Feature]*`.
    ///
    /// The method writes directly into the provided formatter so callers can
    /// stream full tiers without per-token allocations.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(&self.pos)?;
        w.write_char('|')?;
        // The analysis half has exactly one renderer, which is this one, so
        // the two forms cannot drift apart.
        self.analysis().write_chat(w)
    }
}

/// The analysis half of a `%mor` item: `lemma[-Feature]*`, without the `POS|`
/// prefix.
///
/// Returned by [`MorWord::analysis`]; borrows its word and allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MorAnalysis<'a> {
    word: &'a MorWord,
}

impl WriteChat for MorAnalysis<'_> {
    /// Writes `lemma[-Feature]*`.
    ///
    /// Implementing the trait rather than declaring an inherent method of the
    /// same name is what lets a generic `fn render(_: &impl WriteChat)` accept
    /// an analysis, and supplies `to_chat_string` for free.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(&self.word.lemma)?;
        for feature in &self.word.features {
            w.write_char('-')?;
            feature.write_chat(w)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for MorAnalysis<'_> {
    /// Renders `lemma[-Feature]*`, the same bytes
    /// [`write_chat`](WriteChat::write_chat) produces.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

impl WriteChat for MorWord {
    /// Serializes this `%mor` token as `POS|lemma[-Feature]*`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        MorWord::write_chat(self, w)
    }
}
