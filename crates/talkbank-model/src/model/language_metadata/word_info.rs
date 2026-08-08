//! Per-word language-resolution types used for CHAT language switching.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Multiple>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Ambiguous>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SpanShift, ValidationTagged};

use super::super::LanguageCode;
use super::LanguageSource;

/// The languages applicable to a word, preserving code-mixing and ambiguity information.
///
/// This enum captures the complete semantic information about a word's language(s):
/// - **Single**: One definitive language (explicit marker, shortcut resolved, or tier default)
/// - **Multiple**: Code-mixed - word contains content from multiple languages simultaneously (@s:eng+fra)
/// - **Ambiguous**: Ambiguous between languages - transcriber couldn't decide (@s:eng&spa)
/// - **Unresolved**: No language context is available for this word
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Multiple>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Ambiguous>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift, ValidationTagged,
)]
pub enum WordLanguages {
    /// Single definitive language
    Single(LanguageCode),
    /// Multiple languages mixed together (code-mixing)
    Multiple(Vec<LanguageCode>),
    /// Ambiguous between languages
    Ambiguous(Vec<LanguageCode>),
    /// No language could be resolved for this word
    #[validation_tag(error)]
    Unresolved,
}

impl WordLanguages {
    /// Return all language codes referenced by this assignment.
    ///
    /// `Single` returns one entry; `Multiple` and `Ambiguous` return each
    /// member in source order; `Unresolved` returns an empty list.
    pub fn languages(&self) -> Vec<&LanguageCode> {
        match self {
            Self::Single(code) => vec![code],
            Self::Multiple(codes) | Self::Ambiguous(codes) => codes.iter().collect(),
            Self::Unresolved => Vec::new(),
        }
    }
}

/// Language metadata for a single word.
///
/// Stores the resolved language(s) and source for one word of the main tier.
///
/// Position is the record's position in [`LanguageMetadata::word_languages`],
/// which serializes as a JSON array, so a consumer reads it with `enumerate()`.
/// There is deliberately no stored index. One existed, documented as "the same
/// indexing used for tier alignment", and that could not be true: Mor excludes
/// retraces and Pho counts them, so no single integer indexes both. Nothing
/// enforced it and nothing read it, so when the walk skipped containers every
/// following record carried a position for a different word and no test, type
/// or reader could see it.
///
/// This structure is used for:
/// - **Code-switching analysis**: Identify which words are in which language(s)
/// - **Code-mixing detection**: Identify words with @s:eng+fra style markers
/// - **Ambiguity tracking**: Identify words with @s:eng&spa style markers
/// - **Validation**: Ensure language markers are used correctly
/// - **Data extraction**: Associate morphological annotations with language context
///
/// # Fields
///
/// - `languages`: Resolved language(s): Single, Multiple (code-mixed), or Ambiguous
/// - `source`: How the language was determined (see [`LanguageSource`])
///
/// # CHAT Format Examples
///
/// **Example 1: Code-switching with shortcuts**
///
/// ```text
/// @Languages: eng, spa
/// *CHI: I want @s galletas @s please .
/// ```
///
/// Language metadata:
/// - Word 0 "I": languages=Single("eng"), source=Default
/// - Word 1 "want": languages=Single("eng"), source=Default
/// - Word 2 "galletas": languages=Single("spa"), source=WordShortcut
/// - Word 3 "please": languages=Single("eng"), source=WordShortcut
/// - Word 4 ".": languages=Single("eng"), source=Default
///
/// **Example 2: Code-mixed word**
///
/// ```text
/// @Languages: eng, spa
/// *CHI: hello habla@s:eng+spa .
/// ```
///
/// Language metadata:
/// - Word 0 "hello": languages=Single("eng"), source=Default
/// - Word 1 "habla": languages=Multiple(["eng", "spa"]), source=WordExplicit
/// - Word 2 ".": languages=Single("eng"), source=Default
///
/// **Example 3: Ambiguous word**
///
/// ```text
/// @Languages: eng, spa
/// *CHI: hello word@s:eng&spa .
/// ```
///
/// Language metadata:
/// - Word 0 "hello": languages=Single("eng"), source=Default
/// - Word 1 "word": languages=Ambiguous(["eng", "spa"]), source=WordExplicit
/// - Word 2 ".": languages=Single("eng"), source=Default
///
/// # References
///
/// - [Language Codes](https://talkbank.org/0info/manuals/CHAT.html#Language_Codes)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
/// - [Second-Language Marker (single)](https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single)
/// - [Second-Language Marker (multiple)](https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Multiple)
/// - [Second-Language Marker (ambiguous)](https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Ambiguous)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift)]
pub struct WordLanguageInfo {
    /// Resolved language(s): Single, Multiple (code-mixed), or Ambiguous
    pub languages: WordLanguages,

    /// How the language was determined
    pub source: LanguageSource,
}

impl WordLanguageInfo {
    /// Build a record.
    ///
    /// `pub(crate)` on purpose: the public way to add a word is
    /// `LanguageMetadata::add_word`, so the vector's order stays the order the
    /// walk produced. Eight public constructors used to exist, seven of them
    /// dead and all eight taking a caller-supplied position.
    pub(crate) fn new(languages: WordLanguages, source: LanguageSource) -> Self {
        Self { languages, source }
    }
}
