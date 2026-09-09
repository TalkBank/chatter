//! Deterministic placeholder generation.
//!
//! A monotonic [`PlaceholderState`] counter is incremented in document-
//! order traversal, same input + same traversal order yields identical
//! placeholders, which is what the sanitizer needs for both determinism
//! and idempotence (sanitizing a sanitized file matches `wN` against
//! `wN` and reproduces the same numbering).

use talkbank_model::model::NonEmptyString;
use talkbank_model::non_empty_literal;

/// Sequential placeholder index for word/lemma replacement.
///
/// Counts from 1; the first replaced item is `w1` / `lemma1`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlaceholderIndex(u32);

impl PlaceholderIndex {
    /// Creates an index. Callers should use values ≥ 1.
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the underlying numeric value.
    pub fn value(self) -> u32 {
        self.0
    }
}

/// A placeholder string used in serialized CHAT output: a literal prefix
/// followed by the index, so non-empty by construction, which is what lets
/// the sanitizer hand it to the word-text types without a check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceholderToken(NonEmptyString);

impl PlaceholderToken {
    /// Builds a word placeholder (`wN`).
    pub fn word(index: PlaceholderIndex) -> Self {
        Self(non_empty_literal!("w").with_suffix(&index.value().to_string()))
    }

    /// Builds a `%mor` lemma placeholder (`lemmaN`).
    ///
    /// Distinct prefix from `wN` so an auditor can see at a glance
    /// whether a placeholder came from the main tier or from `%mor`.
    /// Word and lemma counters share the same `PlaceholderState` so
    /// their numeric ranges interleave; the indices do not match the
    /// corresponding main-tier word.
    pub fn lemma(index: PlaceholderIndex) -> Self {
        Self(non_empty_literal!("lemma").with_suffix(&index.value().to_string()))
    }

    /// Borrows the placeholder text as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The placeholder as the proven text it is.
    pub fn text(&self) -> &NonEmptyString {
        &self.0
    }
}

impl AsRef<str> for PlaceholderToken {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

/// Per-document mutable state for placeholder generation.
#[derive(Debug, Default)]
pub struct PlaceholderState {
    next_word: u32,
}

impl PlaceholderState {
    /// Creates a fresh placeholder state with the counter at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the next placeholder index, advancing the counter.
    pub fn next(&mut self) -> PlaceholderIndex {
        self.next_word += 1;
        PlaceholderIndex::new(self.next_word)
    }
}
