//! Utterance-level language metadata (tier default + per-word resolution).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use talkbank_derive::SpanShift;

use super::super::LanguageCode;
use super::{LanguageSource, WordLanguageInfo, WordLanguages};
use crate::ErrorSink;
use crate::validation::{Validate, ValidationContext};

/// Language metadata for an entire utterance.
///
/// Tracks the resolved language for each WORD of the main tier, at any depth,
/// including inside quotations, phonological groups, sign groups and retraces.
///
/// This structure stores:
/// - **tier language**: utterance baseline (`[- code]` or `@Languages` primary)
/// - **word languages**: resolved per-word language/provenance entries
///
/// The vector is in in-order traversal order. It is NOT an alignment index:
/// the tier domains disagree about what they count (Mor excludes retraces,
/// Pho counts them), so correlating with `%mor`/`%gra` positions requires the
/// alignment layer's own index types, not this order.
///
/// # Structure
///
/// ```text
/// LanguageMetadata
///   ├─ tier_language: Option<LanguageCode>  (e.g., "eng")
///   └─ word_languages: WordLanguageInfos
///        ├─ [0]: languages=Single("eng"), source=Default
///        ├─ [1]: languages=Single("spa"), source=WordShortcut
///        └─ ...
/// ```
///
/// # CHAT Format Examples
///
/// **Example 1: Single language utterance**
///
/// ```text
/// @Languages: eng
/// *CHI: I want cookie .
/// ```
///
/// Language metadata:
/// - `tier_language`: Some("eng")
/// - `word_languages`: All words have language="eng", source=Default
/// - `is_code_switching()`: false
///
/// **Example 2: Code-switching utterance**
///
/// ```text
/// @Languages: eng, spa
/// *CHI: I want galletas@s please .
/// ```
///
/// Language metadata:
/// - `tier_language`: Some("eng")
/// - `word_languages`:
///   - "I" → eng (Default)
///   - "want" → eng (Default)
///   - "galletas" → spa (WordShortcut)
///   - "please" → eng (Default)
/// - `is_code_switching()`: true (uses both "eng" and "spa")
///
/// **Example 3: Tier-scoped language change**
///
/// ```text
/// @Languages: eng, fra
/// *CHI: [- fra] je veux cookie .
/// ```
///
/// Language metadata:
/// - `tier_language`: Some("fra") (from `[- fra]` marker)
/// - `word_languages`: All words have language="fra", source=TierScoped
/// - `is_code_switching()`: false
///
/// # Use Cases
///
/// **Code-switching detection:**
/// ```rust
/// # use talkbank_model::LanguageMetadata;
/// # fn report_switching(language_metadata: &LanguageMetadata) {
/// if language_metadata.is_code_switching() {
///     let counts = language_metadata.count_by_language();
///     println!("Utterance uses {} languages", counts.len());
/// }
/// # }
/// ```
///
/// **Per-word language lookup:**
/// ```rust
/// # use talkbank_model::LanguageMetadata;
/// # fn report_third_word(language_metadata: &LanguageMetadata) {
/// if let Some(info) = language_metadata.word_languages.as_slice().get(2) {
///     println!("Word 2 language assignment: {:?}", info.languages);
/// }
/// # }
/// ```
///
/// # References
///
/// - [Language Codes](https://talkbank.org/0info/manuals/CHAT.html#Language_Codes)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
/// - [Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift)]
pub struct LanguageMetadata {
    /// Effective utterance baseline language.
    ///
    /// Applies to words without explicit language markers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier_language: Option<LanguageCode>,

    /// Resolved language and provenance for each word, in traversal order.
    ///
    /// Position in this vector is the only index; nothing stores one.
    pub word_languages: WordLanguageInfos,
}

impl LanguageMetadata {
    /// Create metadata with a baseline tier language and no word entries.
    ///
    /// This constructor is typically used before word-level resolution begins.
    /// Appended in in-order traversal order as content is walked.
    pub fn new(tier_language: Option<LanguageCode>) -> Self {
        Self {
            tier_language,
            word_languages: WordLanguageInfos::new(Vec::new()),
        }
    }

    /// Append one resolved word-language entry.
    ///
    /// Append a word's resolved language.
    ///
    /// Takes the PIECES, not a built record, so a caller cannot construct an
    /// entry out of band and push it. Position is the append position and is
    /// not stored; a stored copy of it is what silently disagreed with this
    /// list when the walk that filled it skipped containers.
    pub fn add_word(&mut self, languages: WordLanguages, source: LanguageSource) {
        self.word_languages
            .push(WordLanguageInfo::new(languages, source));
    }

    /// Count word assignments by language code.
    ///
    /// For single-language words, increments the count for that language.
    /// For code-mixed or ambiguous words, increments counts for ALL applicable languages.
    pub fn count_by_language(&self) -> HashMap<LanguageCode, usize> {
        let mut counts = HashMap::new();
        for word_info in self.word_languages.iter() {
            // Count all applicable languages, even for code-mixed or ambiguous words
            for lang in word_info.languages.languages() {
                *counts.entry(lang.clone()).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Return whether this utterance should be treated as code-switching.
    ///
    /// This returns `true` when at least two distinct languages appear across
    /// words, or when any word is explicitly marked as mixed (`@s:eng+spa`) or
    /// ambiguous (`@s:eng&spa`), even if distinct-language counts collapse.
    pub fn is_code_switching(&self) -> bool {
        // No assignment yet versus one observed language are the only states
        // before switching is established. Borrow that evidence, rather than
        // cloning every code into a set and maintaining parallel flags.
        let mut first_language: Option<&LanguageCode> = None;
        for word_info in self.word_languages.iter() {
            match &word_info.languages {
                WordLanguages::Single(language) => {
                    if *first_language.get_or_insert(language) != language {
                        return true;
                    }
                }
                WordLanguages::Multiple(_) | WordLanguages::Ambiguous(_) => return true,
                WordLanguages::Unresolved => {}
            }
        }
        false
    }
}

/// Newtype wrapper around per-word language resolution entries for one utterance.
///
/// The vector is in in-order traversal order, one entry per word. It is not
/// an alignment index; see [`LanguageMetadata`].
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct WordLanguageInfos(Vec<WordLanguageInfo>);

crate::collection_newtype_ops!(WordLanguageInfos, WordLanguageInfo);

impl WordLanguageInfos {
    /// Appends one word's language record.
    ///
    /// A NAMED length-changing operation, added when `DerefMut` was removed
    /// from this family.
    pub fn push(&mut self, info: WordLanguageInfo) {
        self.0.push(info);
    }

    /// Wrap a vector of per-word language entries.
    ///
    /// This constructor is mostly used by parser/build pipelines that already
    /// assembled a full utterance-level assignment vector.
    pub fn new(infos: Vec<WordLanguageInfo>) -> Self {
        Self(infos)
    }

    /// Return whether no per-word language entries are present.
    ///
    /// Empty metadata is valid for empty/error-recovered utterances and should
    /// not be interpreted as a parser failure by itself.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for WordLanguageInfos {
    type Target = Vec<WordLanguageInfo>;

    /// Expose the underlying vector for read-only iteration and indexing.
    ///
    /// The transparent wrapper keeps schema typing while still allowing normal
    /// `Vec` ergonomics in analysis code.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<WordLanguageInfo>> for WordLanguageInfos {
    /// Wrap a raw vector as a typed language-info list.
    ///
    /// Prefer this conversion when bridging from parser internals to the
    /// strongly-typed model surface.
    fn from(infos: Vec<WordLanguageInfo>) -> Self {
        Self(infos)
    }
}

impl Validate for WordLanguageInfos {
    /// Word-level validation is performed where entries are produced.
    ///
    /// This container itself currently has no extra invariants beyond storing
    /// a list of `WordLanguageInfo`. The hook remains in place so future
    /// cross-entry invariants can be enforced without changing call sites.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

impl Default for LanguageMetadata {
    /// Create empty metadata with no tier language and no word entries.
    ///
    /// This default is used for parser-recovery and builder initialization
    /// before language resolution has run.
    fn default() -> Self {
        Self::new(None)
    }
}
