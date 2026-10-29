#![deny(clippy::wildcard_enum_match_arm)]

//! Core [`Word`] model and related helper types.
//!
//! This module defines the canonical typed representation of one parsed CHAT
//! word token, including structured content and optional marker fields.
//!
//! # CHAT Format References
//!
//! - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
//! - [Special Form Markers](https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker)
//! - [Compounds](https://talkbank.org/0info/manuals/CHAT.html#Compounds)
//! - [Clitics](https://talkbank.org/0info/manuals/CHAT.html#Clitics)
//! - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
//! - [Media Linking](https://talkbank.org/0info/manuals/CHAT.html#Media_Linking)

use crate::model::WriteChat;
use serde::Deserialize;
use talkbank_derive::{SemanticEq, SpanShift};

use super::category::WordCategory;
use super::content::{WordContent, WordText};
use super::form::FormType;
use super::language::WordLanguageMarker;
use super::pos_tag::WordPosTag;
use super::suffix::WordSuffix;
use super::untranscribed::UntranscribedStatus;
use super::word_contents::WordContents;
use crate::model::Bullet;

/// A cached string value that is transparent to equality comparisons.
///
/// Used for memoization fields in derived-`PartialEq` structs. Two `CachedStr`
/// values always compare equal regardless of their contents, so the cache
/// never affects structural equality.
#[derive(Clone, Debug, Default)]
pub(super) struct CachedStr(pub(super) std::sync::OnceLock<smol_str::SmolStr>);

impl PartialEq for CachedStr {
    /// Always returns `true` so memoization never influences semantic equality.
    fn eq(&self, _other: &Self) -> bool {
        true // Derived from other fields; skip in equality
    }
}

/// A word in a CHAT transcript with optional markers and internal structure.
///
/// Words are the fundamental units of CHAT transcripts. They can be simple dictionary
/// forms, compounds, cliticizations, or include special markers indicating dialectal
/// variations, child-invented forms, and other linguistic phenomena.
///
/// # CHAT Format Examples
///
/// ```text
/// dog                 Simple word
/// ice+cream          Compound (legacy format)
/// gonna              Cliticization (going to)
/// sit(ting)          Shortening (incomplete word)
/// gumma@c            Child-invented form (@c marker)
/// younz@d            Dialect form (@d marker)
/// istenem@s:hu       Second language (@s:code marker)
/// bana:nas           Lengthened syllable (: marker)
/// ```
///
/// # Special Markers
///
/// - **Form markers** (`@b`, `@c`, `@d`, etc.) - See [`FormType`]
/// - **Language markers** (`@s`, `@s:code`) - See [`WordLanguageMarker`]
/// - **Category prefixes** (`&`, `&-`, etc.) - See [`WordCategory`]
/// - **Untranscribed status** - See [`UntranscribedStatus`]
///
/// # References
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
/// - [Special Form Markers](https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker)
/// - [Compounds](https://talkbank.org/0info/manuals/CHAT.html#Compounds)
/// - [Shortenings](https://talkbank.org/0info/manuals/CHAT.html#Shortenings)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
/// - [Media Linking](https://talkbank.org/0info/manuals/CHAT.html#Media_Linking)
///
/// # Alignment
///
/// Words participate in tier alignment with %mor, %pho, and other dependent
/// tiers. **Do not hand-roll the walk or the filter.** Use
/// [`walk_words`](crate::alignment::helpers::walk_words) to visit them and
/// [`counts_for_tier`](crate::alignment::helpers::counts_for_tier) to decide
/// which ones a given tier domain aligns against.
///
/// Deciding which words align splits into two questions, and only one of them
/// is easy to guess. WHICH CATEGORIES count is a short list a careful reader
/// will reproduce. WHICH PARTS OF THE TREE you visit is spread across a dozen
/// content variants, differs by tier domain, and a `match` arm you forget to
/// write compiles cleanly and drops a whole subtree in silence.
///
/// A downstream implementation that guessed the category list correctly and
/// hand-rolled the traversal shipped three bugs it had not diagnosed: words
/// inside `‹…›` phonological groups vanished from its token stream, `[e]`
/// -excluded material was counted so valid utterances reported false
/// mismatches, and every CA separator was counted although only tag, vocative
/// and comma carry `%mor` items. Its own tests passed throughout, because on
/// flat utterances the two implementations agree.
#[derive(Clone, Debug, PartialEq, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "super::word_wire::WordWire")]
pub struct Word {
    /// Source location (byte offsets in original input).
    #[serde(skip, default = "crate::Location::unlocated")]
    pub location: crate::Location,

    /// Unique identifier for tier alignment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_id: Option<smol_str::SmolStr>,

    /// Structured content breakdown.
    ///
    /// Uses a SmallVec-backed newtype - most words are simple (1 item)
    /// or compounds (2-3 items).
    content: WordContents,

    /// Word category prefix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<WordCategory>,

    /// At most one form or language marker in the word's `@` slot.
    pub suffix: Option<WordSuffix>,

    /// Part-of-speech tag ($adj, $n, $v, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_of_speech: Option<WordPosTag>,

    /// Inline timing bullet parsed directly from %wor tier.
    ///
    /// When a %wor word has an inline bullet (e.g., `word •start_end•`), this field
    /// preserves the parsed bullet as a first-class entity with its span and timing data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_bullet: Option<Bullet>,

    /// Cached cleaned text (derived from `content`). Computed on first access.
    #[serde(skip, default)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub(super) cached_cleaned_text: CachedStr,
}

impl Word {
    /// Builds a plain word from nonempty lexical text. Both surface spelling
    /// and cleaned text are derived from the resulting structure.
    ///
    /// Use [`WordText::new`] for dynamic text or convert
    /// [`crate::non_empty_literal!`] output to `WordText`. This constructs a
    /// plain lexical leaf, not parsed CHAT syntax. Deserialized recovery
    /// payloads still require validation.
    ///
    /// ```compile_fail,E0308
    /// let word = talkbank_model::Word::new(talkbank_model::Location::Unlocated, "");
    /// ```
    pub fn new(location: impl Into<crate::Location>, text: WordText) -> Self {
        Self::from_parts(location, text)
    }

    /// Construct from explicit structured content. Surface spelling and cleaned
    /// text are derived from this single owner; no temporary lexical text is
    /// needed. This does not certify CHAT validity or require spoken material.
    pub fn from_content(location: impl Into<crate::Location>, content: WordContents) -> Self {
        Self {
            location: location.into(),
            word_id: None,
            content,
            category: None,
            suffix: None,
            part_of_speech: None,
            inline_bullet: None,
            cached_cleaned_text: CachedStr::default(),
        }
    }

    /// Plain lexical construction enters the same structured-content owner.
    fn from_parts(location: impl Into<crate::Location>, cleaned: WordText) -> Self {
        Self::from_content(
            location,
            WordContents::new(smallvec::smallvec![WordContent::Text(cleaned)]),
        )
    }

    /// Derives the current CHAT spelling from typed structure.
    /// Original source spelling belongs to the source-bound parser, not the word.
    pub fn raw_text(&self) -> String {
        self.to_chat()
    }

    /// Returns the typed elements that make up this word.
    pub fn content(&self) -> &WordContents {
        &self.content
    }

    /// Returns cleaned lexical text suitable for downstream NLP.
    ///
    /// Computed from `content` by concatenating `Text` and `Shortening` elements.
    /// Cached on first access via `OnceLock`, subsequent calls return a pointer.
    pub fn cleaned_text(&self) -> &str {
        self.cached_cleaned_text
            .0
            .get_or_init(|| smol_str::SmolStr::new(self.compute_cleaned_text()))
    }

    /// What this word's letters ARE: spelling, or a rendering of sound.
    ///
    /// Delegates to [`WordCategory::material`]. A word with NO category prefix
    /// is plain orthography, and that is an answer rather than a default: the
    /// absence of `&+`, `&-`, `&~` or `0` is exactly what makes it an ordinary
    /// word.
    #[must_use]
    pub fn material(&self) -> crate::model::WordMaterial {
        match &self.category {
            Some(category) => category.material(),
            None => crate::model::WordMaterial::Orthography,
        }
    }

    /// Returns untranscribed marker classification, if any.
    ///
    /// Computed from content: returns `Some(...)` when cleaned text is "xxx", "yyy", or "www".
    /// Reads the `OnceLock`-cached cleaned text rather than recomputing it.
    ///
    /// This used to delegate to `compute_untranscribed`, which calls
    /// `compute_cleaned_text()` and so allocated a fresh `String` on every call:
    /// measured 20.96 ns against 2.22 ns for the cached form. Ten call sites pay
    /// it, several unconditionally per word, including `Serialize` (so every
    /// `chatter to-json` over the corpus) and the `%mor` / `%wor` alignment
    /// rules.
    ///
    /// Classification is not spoken-content or validity evidence. Cleaned text
    /// includes shortening segments, so a shortening-only word can have one of
    /// these spellings while still lacking spoken content. Validators inspect
    /// typed content leaves independently.
    pub fn untranscribed(&self) -> Option<UntranscribedStatus> {
        UntranscribedStatus::from_marker_text(self.cleaned_text())
    }

    /// Sets an optional stable identifier used by alignment/metadata pipelines.
    ///
    /// This ID is treated as auxiliary metadata and does not participate in text rendering.
    pub fn with_word_id(mut self, id: impl Into<smol_str::SmolStr>) -> Self {
        self.word_id = Some(id.into());
        self
    }

    /// Replaces structured internal word content.
    ///
    /// Invalidates cached lexical text. Surface spelling is always derived
    /// from the current structure and needs no separate update.
    pub fn with_content(mut self, content: impl Into<WordContents>) -> Self {
        self.replace_content(content.into());
        self
    }

    /// Replace complete typed content and invalidate derived lexical text.
    ///
    /// Build the replacement before calling this method; the word never
    /// exposes an intermediate empty content sequence during a rewrite.
    pub fn replace_content(&mut self, content: WordContents) {
        self.content = content;
        self.cached_cleaned_text = CachedStr::default();
    }

    /// Replaces one typed content element and invalidates derived lexical text.
    ///
    /// Content mutation must pass through `Word`: mutating the collection
    /// alone cannot know that this word's `cleaned_text` cache is now stale.
    pub fn replace_content_at(&mut self, index: usize, item: WordContent) {
        self.content.replace_at(index, item);
        self.cached_cleaned_text = CachedStr::default();
    }

    /// Sets the optional category prefix (for example fillers/fragments).
    ///
    /// Category prefixes are rendered before lexical content in CHAT surface order.
    pub fn with_category(mut self, category: WordCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Replace the complete `@` suffix with one explicit form or language marker.
    pub fn with_suffix(mut self, suffix: WordSuffix) -> Self {
        self.suffix = Some(suffix);
        self
    }

    /// Borrow a form marker from the single suffix slot.
    pub fn form_type(&self) -> Option<&FormType> {
        self.suffix.as_ref().and_then(WordSuffix::form_type)
    }

    /// Borrow a language marker from the single suffix slot.
    pub fn language_marker(&self) -> Option<&WordLanguageMarker> {
        self.suffix.as_ref().and_then(WordSuffix::language_marker)
    }

    /// Mutate only an existing language payload, preserving suffix exclusivity.
    pub fn language_marker_mut(&mut self) -> Option<&mut WordLanguageMarker> {
        self.suffix
            .as_mut()
            .and_then(WordSuffix::language_marker_mut)
    }

    /// Sets an optional part-of-speech tag (`$...`).
    ///
    /// The admitted payload preserves transcript spelling without imposing a vocabulary.
    pub fn with_part_of_speech(mut self, pos: WordPosTag) -> Self {
        self.part_of_speech = Some(pos);
        self
    }

    /// Sets an inline timing bullet parsed from `%wor`.
    ///
    /// Inline bullets are timing metadata only; lexical text continues to come from `content`.
    pub fn with_inline_bullet(mut self, bullet: Bullet) -> Self {
        self.inline_bullet = Some(bullet);
        self
    }

    /// Compute cleaned text by concatenating Text and Shortening content elements.
    ///
    /// This is the authoritative way to derive the cleaned (NLP-ready) text from
    /// the word's structured `content`. It includes only lexical segments:
    /// - `WordContent::Text`, base graphemes
    /// - `WordContent::Shortening`, elided material restored (e.g., `som(e)` → `some`)
    ///
    /// All prosodic/analytical markers (lengthening, stress, CA elements, overlap
    /// points, compound markers, underline markers) are excluded.
    ///
    /// # CA segment repetition (`↫`)
    ///
    /// The `↫` (U+21AB) CA delimiter is a *paired bracket* around a stuttered,
    /// repeated segment that is NOT lexical material. Per the CHAT manual's
    /// Disfluency Transcription chapter, "the ↫ brackets the repetition":
    /// `↫b-b-b↫boy` is the word "boy" and `↫sch↫schaap` is "schaap". The pair may
    /// also bracket a repeated *final* segment (`like↫ike-ike↫` -> "like"). Text
    /// (and shortenings) appearing *between* a `↫ ... ↫` pair are therefore
    /// excluded from the cleaned text. This is specific to `SegmentRepetition`;
    /// every other CA delimiter (`∆` faster, `∇` slower, ...) wraps genuinely
    /// spoken material whose enclosed text IS kept (`∆fast∆` -> "fast").
    pub fn compute_cleaned_text(&self) -> String {
        let mut result = String::new();
        for part in self.lexical_parts() {
            if let super::LexicalContribution::Spoken(text) = part.contribution() {
                result.push_str(text);
            }
        }
        result
    }

    /// Inspect lexical contributions without losing the original typed leaves.
    ///
    /// Unlike [`Self::cleaned_text`], this preserves compound/clitic boundaries
    /// and distinguishes repeated sounds from spoken lexical text. It does not
    /// parse raw spelling or infer morphology such as an apostrophe suffix.
    pub fn lexical_parts(&self) -> super::WordLexicalParts<'_> {
        super::WordLexicalParts::new(self.content.as_slice())
    }

    /// Replaces content with one plain-text segment.
    ///
    /// Used by ASR-postprocess pipelines when substituting a word with a
    /// replacement text. Like [`Self::new`], accepts the typed lexical payload
    /// rather than constructing it unchecked from a raw string. Sets content
    /// to a single `Text` element and changes the derived spelling accordingly.
    /// JSON recovery payloads still require validation.
    ///
    /// ```compile_fail,E0308
    /// fn replace(word: &mut talkbank_model::Word) {
    ///     word.replace_simple_text("");
    /// }
    /// ```
    pub fn replace_simple_text(&mut self, text: WordText) {
        self.content = WordContents::new(smallvec::smallvec![WordContent::Text(text)]);
        self.cached_cleaned_text = CachedStr::default();
    }

    /// Serializes this word to an owned CHAT string.
    ///
    /// This is a convenience wrapper over [`WriteChat`] for callers that need
    /// an owned value instead of writing into an existing buffer.
    pub fn to_chat(&self) -> String {
        crate::model::ChatString::render(|w| self.write_chat(w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_suffix_wire_refuses_conflicting_form_and_language() {
        let word = Word::new(crate::Location::Unlocated, WordText::new("word").unwrap());
        let mut wire = serde_json::to_value(word).unwrap();
        wire["form_type"] = serde_json::json!("b");
        wire["lang"] = serde_json::json!({ "type": "explicit", "code": "eng" });
        assert!(serde_json::from_value::<Word>(wire).is_err());
    }

    #[test]
    fn word_suffix_recovery_invalidity_survives_json() {
        use crate::validation::Validate;
        for payload in ["zz", "c@d", "c@s:spa", "@c"] {
            let word = Word::new(crate::Location::Unlocated, WordText::new("word").unwrap())
                .with_suffix(WordSuffix::Form(FormType::Undeclared(payload.to_owned())));
            let wire = serde_json::to_value(word).unwrap();
            let readback: Word = serde_json::from_value(wire).unwrap();
            let errors = crate::ErrorCollector::new();
            readback.validate(&crate::validation::ValidationContext::default(), &errors);
            let diagnostics = errors.into_vec();
            let form_errors: Vec<_> = diagnostics
                .iter()
                .filter(|error| error.code == crate::ErrorCode::InvalidFormType)
                .collect();
            assert_eq!(form_errors.len(), 1, "{payload}: {diagnostics:?}");
            assert!(form_errors[0].message.contains(payload));
        }
    }

    #[test]
    fn word_json_refuses_pos_payloads_that_change_chat_structure() {
        let word = Word::new(
            crate::Location::Unlocated,
            WordText::new("hello").expect("word"),
        );
        let original = serde_json::to_value(word).expect("word JSON");
        for payload in ["", "n v", "n\tv", "n\n@End", "n$adj", "n@f", "n1", "名"] {
            let mut wire = original.clone();
            wire["part_of_speech"] = serde_json::json!(payload);
            assert!(
                serde_json::from_value::<Word>(wire).is_err(),
                "accepted POS payload {payload:?}"
            );
        }
    }

    #[test]
    fn structured_content_constructor_derives_text_without_a_seed() {
        let word = Word::from_content(
            crate::Location::Unlocated,
            WordContents::new(smallvec::smallvec![
                WordContent::Text(WordText::new("som").expect("text")),
                WordContent::Shortening(
                    crate::model::WordShortening::new("e").expect("shortening")
                ),
            ]),
        );
        assert_eq!(word.cleaned_text(), "some");
        assert_eq!(word.to_chat(), "som(e)");
        let empty = Word::from_content(
            crate::Location::Unlocated,
            WordContents::new(smallvec::smallvec![]),
        );
        assert_eq!(empty.cleaned_text(), "");
        assert_eq!(empty.to_chat(), "");
    }

    /// Every public content-replacement path must invalidate the derived text
    /// cache. Keeping the paths in one test makes additions visible beside the
    /// complete mutation API rather than duplicating lexical behavior tests.
    #[test]
    fn content_replacement_paths_invalidate_cleaned_text() {
        let mut indexed = Word::new(
            crate::Location::Unlocated,
            crate::WordText::from(crate::non_empty_literal!("old")),
        );
        assert_eq!(indexed.cleaned_text(), "old");
        indexed.replace_content_at(
            0,
            WordContent::Text(WordText::new("indexed").expect("a non-empty literal")),
        );
        assert_eq!(indexed.cleaned_text(), "indexed");
        indexed.replace_content(WordContents::new(smallvec::smallvec![WordContent::Text(
            WordText::from(crate::non_empty_literal!("whole"))
        ),]));
        assert_eq!(indexed.cleaned_text(), "whole");

        let mut simple = Word::new(
            crate::Location::Unlocated,
            crate::WordText::from(crate::non_empty_literal!("old")),
        );
        assert_eq!(simple.cleaned_text(), "old");
        simple.replace_simple_text(WordText::from(crate::non_empty_literal!("simple")));
        assert_eq!(simple.cleaned_text(), "simple");

        let warmed = Word::new(
            crate::Location::Unlocated,
            crate::WordText::from(crate::non_empty_literal!("old")),
        );
        assert_eq!(warmed.cleaned_text(), "old");
        let replaced = warmed.with_content(vec![WordContent::Text(
            WordText::new("builder").expect("a non-empty literal"),
        )]);
        assert_eq!(replaced.cleaned_text(), "builder");
    }
}
