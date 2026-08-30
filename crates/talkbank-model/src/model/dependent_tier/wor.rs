//! Word timing tier (`%wor`) with word-level timing annotations.
//!
//! The %wor tier provides word-level timing information. It is a **flat** list
//! of items, primarily words, each optionally paired with a timing bullet,
//! plus untimed tag-marker separators (comma `,`, tag `„`, vocative `‡`).
//! Unlike the main tier, %wor never contains groups, annotations, replacements,
//! or events.
//!
//! # Format
//!
//! ```text
//! %wor: [- lang]? (word •start_end•? | separator)* terminator
//! ```
//!
//! # Examples
//!
//! ```text
//! *CHI:    I want cookies .
//! %wor:    I 1000_1200 want 1200_1400 cookies 1400_1800 .
//!
//! *CHI:    he's in the water , too .
//! %wor:    he's 307628_307788 in 307848_307948 the 307948_308008 water 308028_308248 , too 308429_308929 .
//!
//! *CHI:    [- eng] religious studies .
//! %wor:    [- eng] religious 53335_53415 studies 53415_53676 .
//! ```
//!
//! # Reference
//!
//! - [CHAT Manual: Word Tier](https://talkbank.org/0info/manuals/CHAT.html#Word_Tier)

use crate::Span;
use crate::model::content::word::Word;
use crate::model::{Bullet, LanguageCode, Terminator};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// A single item in a %wor tier: either a timed word or an untimed
/// tag-marker separator.
///
/// Tag-marker separators (comma `,`, tag `„`, vocative `‡`) appear in %wor
/// between timed words. They correspond to `cm|cm`, `end|end`, and `beg|beg`
/// in %mor. They are never timed and do not count toward alignment with the
/// main tier's word slots.
///
/// # Word Text Is Corroboration, Not Lexical Authority
///
/// The word text exists solely for:
/// - Human readability when viewing CHAT files
/// - Canonical lexical-correspondence diagnostics
/// - CHAT format serialization compliance
///
/// **The word text is NOT used for:**
/// - Forced alignment (uses main tier words)
/// - Morphosyntax processing (uses main tier words)
/// - Lexical identity (always comes from the typed main tier)
///
/// **Current convention**: `generate_wor_tier()` copies `cleaned_text` from
/// main-tier words. Timing consumers may compare a parsed `%wor` sequence with
/// that canonical generated sequence to refuse unsafe positional reuse after a
/// same-count edit. A different display convention therefore requires a new,
/// explicitly named correspondence policy. It cannot change silently.
///
/// **What matters**: The `inline_bullet` field on each word, which contains
/// the actual timing data (start_ms, end_ms). This is parsed, stored, and
/// used for all timing-related operations.
///
/// # Reference
///
/// - [Word tier](https://talkbank.org/0info/manuals/CHAT.html#Word_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "kind")]
pub enum WorItem {
    /// A word with optional timing bullet.
    ///
    /// The `Word.cleaned_text` and `Word.raw_text` fields are display material
    /// and optional lexical corroboration. They never replace main-tier lexical
    /// identity. Timing data remains in `Word.inline_bullet`.
    #[serde(rename = "word")]
    Word(Box<Word>),

    /// An untimed tag-marker separator (`,`, `„`, or `‡`).
    #[serde(rename = "separator")]
    Separator {
        /// The separator text (e.g. ",", "„", "‡").
        text: String,

        /// Source span for error reporting.
        #[serde(skip, default = "crate::Span::dummy")]
        #[schemars(skip)]
        span: Span,
    },
}

impl WorItem {
    /// Returns a reference to the inner `Word` if this is a `WorItem::Word`.
    ///
    /// This helper is used by `%wor` alignment code to skip untimed separators
    /// without cloning item payloads.
    pub fn as_word(&self) -> Option<&Word> {
        match self {
            WorItem::Word(w) => Some(w.as_ref()),
            WorItem::Separator { .. } => None,
        }
    }
}

/// Flat %wor tier: a list of `WorItem`s (words and separators) with a terminator.
///
/// Words carry `inline_bullet` set to a `Bullet` if an inline bullet followed
/// them in the source, or `None` if not. `%wor` has no tier-level bullet:
/// timing belongs to individual word entries.
/// Separators are always untimed and not counted for alignment.
///
/// This model is intentionally simple. Unlike the main tier, %wor tiers
/// do not contain groups, annotations, events, or nested structures.
/// Use `words()` to iterate only over `Word` items for alignment purposes.
///
/// # Word Text Is Corroboration, Not Authority
///
/// Generation remains one-way:
/// ```text
/// Main tier AST
///   ↓
/// generate_wor_tier() copies {cleaned_text, inline_bullet}
///   ↓
/// WorTier AST
///   ↓
/// write_chat() serializes to CHAT format
///   ↓
/// %wor: word 1000_1200 another 1200_1500 .
///       ^^^^ canonical display token
///            ^^^^^^^^^^^ REAL DATA (timing)
/// ```
///
/// When we parse a CHAT file with %wor tiers back:
/// 1. Parser builds a `WorTier` AST and retains its display tokens
/// 2. Timing consumers bind the tier to a typed main-tier projection
/// 3. Count drift prevents positional comparison without invalidating CHAT
/// 4. Canonical token drift prevents positional timing recovery
/// 5. Only corroborated slots expose timing from `inline_bullet`
///
/// **What this means**:
/// - Main-tier words remain the only lexical authority
/// - `%wor` text can refuse reuse but cannot supply replacement lexical data
/// - Current `cleaned_text` output is a named canonical convention
/// - Timing itself comes only from `inline_bullet`
///
/// See: `book/src/architecture/wor-timing.md` for comprehensive analysis.
///
/// # References
///
/// - [Word tier](https://talkbank.org/0info/manuals/CHAT.html#Word_Tier)
/// - [Media bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct WorTier {
    /// Optional language code (e.g., `[- spa]`)
    pub language_code: Option<LanguageCode>,

    /// Flat list of items: words with timing and untimed tag-marker separators.
    pub items: Vec<WorItem>,

    /// Terminator punctuation (`.`, `?`, `!`, etc.)
    pub terminator: Option<Terminator>,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip, default = "crate::Span::dummy")]
    #[schemars(skip)]
    pub span: Span,
}

/// Presence of an actual timing bullet on a typed `%wor` tier.
///
/// This state is deliberately independent of main-tier correspondence. Equal
/// word counts do not create timing evidence, and count drift does not erase a
/// bullet that is physically present in CHAT.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorTimingEvidence<'tier> {
    /// No word-level bullet is present.
    Absent,
    /// A real parsed or generated bullet is present.
    Recorded(RecordedWorTiming<'tier>),
}

/// First recorded timing bullet found on a typed `%wor` tier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecordedWorTiming<'tier> {
    bullet: &'tier Bullet,
}

impl<'tier> RecordedWorTiming<'tier> {
    /// Bullet that supplies the timing observation.
    pub fn bullet(self) -> &'tier Bullet {
        self.bullet
    }
}

impl Default for WorTier {
    /// Returns an empty `%wor` tier with unset language and terminator fields.
    ///
    /// `Default` is primarily for builder-style assembly and test fixtures.
    /// Parser output should still attach real spans and parsed content explicitly.
    fn default() -> Self {
        Self {
            language_code: None,
            items: Vec::new(),
            terminator: None,
            span: Span::DUMMY,
        }
    }
}

impl WorTier {
    /// Create a new word timing tier from a list of items.
    ///
    /// This constructor keeps item ordering unchanged, because timing alignment
    /// and serialization both depend on original sequence.
    pub fn new(items: Vec<WorItem>) -> Self {
        Self {
            language_code: None,
            items,
            terminator: None,
            span: Span::DUMMY,
        }
    }

    /// Create a new word timing tier from a list of words (no separators).
    ///
    /// This is a convenience for generated tiers where every slot is a word.
    /// Separator-bearing tiers should use [`Self::new`] with explicit `WorItem`s.
    pub fn from_words(words: Vec<Word>) -> Self {
        Self::new(
            words
                .into_iter()
                .map(|w| WorItem::Word(Box::new(w)))
                .collect(),
        )
    }

    /// Iterate over only the `Word` items (skipping separators).
    ///
    /// Use this for alignment counting, separators are not alignable.
    ///
    /// **RESTRICTED**: Used for timing extraction, canonical lexical
    /// corroboration, and TextGrid export. Lexical processing still uses the
    /// main tier.
    pub fn words(&self) -> impl Iterator<Item = &Word> {
        self.items.iter().filter_map(WorItem::as_word)
    }

    /// Count of words only (excludes separators). Used for alignment.
    ///
    /// This mirrors `words().count()` and intentionally ignores tag-marker
    /// separators that are not part of main-tier slot alignment.
    pub fn word_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, WorItem::Word(_)))
            .count()
    }

    /// Return whether this tier contains an actual recorded timing bullet.
    ///
    /// The operation inspects the typed tier directly. It never substitutes a
    /// word-count relationship for timing evidence.
    pub fn timing_evidence(&self) -> WorTimingEvidence<'_> {
        match self.words().find_map(|word| word.inline_bullet.as_ref()) {
            Some(bullet) => WorTimingEvidence::Recorded(RecordedWorTiming { bullet }),
            None => WorTimingEvidence::Absent,
        }
    }

    /// Create with terminator.
    ///
    /// The terminator is optional so callers can represent partial `%wor`
    /// payloads during intermediate processing.
    pub fn with_terminator(mut self, terminator: Option<Terminator>) -> Self {
        self.terminator = terminator;
        self
    }

    /// Set the language code (builder pattern).
    ///
    /// Language tags affect serialization only; timing data still comes from
    /// per-word inline bullets.
    pub fn with_language_code(mut self, language_code: Option<LanguageCode>) -> Self {
        self.language_code = language_code;
        self
    }

    /// Set the source span (builder pattern).
    ///
    /// Parser paths should set concrete spans so `%wor` mismatch diagnostics can
    /// point to source locations in the original transcript.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }
}

impl crate::model::WriteChat for WorTier {
    /// Serializes `%wor` items, inline timing bullets, and an optional terminator.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "%wor:\t")?;

        // Language code
        if let Some(ref lc) = self.language_code {
            write!(w, "[- {}] ", lc.as_str())?;
        }

        // Items: words with optional timing bullets, and separators.
        // The word's first-class inline bullet is the only `%wor` timing
        // surface; correspondence metadata does not synthesize timing.
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            match item {
                WorItem::Word(word) => {
                    w.write_str(word.cleaned_text())?;
                    if let Some(ref bullet) = word.inline_bullet {
                        w.write_char(' ')?;
                        bullet.write_chat(w)?;
                    }
                }
                WorItem::Separator { text, .. } => {
                    w.write_str(text)?;
                }
            }
        }

        // Terminator
        if let Some(ref term) = self.terminator {
            if !self.items.is_empty() {
                w.write_char(' ')?;
            }
            term.write_chat(w)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
