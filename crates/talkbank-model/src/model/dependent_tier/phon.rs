//! Phon project extension tiers: `%xmodsyl`, `%xphosyl`, `%xphoaln`, `%xphoint`.
//!
//! These tiers originate from the [Phon](https://www.phon.ca/) phonological
//! analysis tool and provide syllable-annotated phonological transcription,
//! segmental alignment between target (model) and actual (phone) IPA forms, and
//! per-phone time intervals. Phon writes them with a leading `x` (extension
//! tiers); the grammar also accepts the historical non-`x` names.
//!
//! # Tier Types
//!
//! | CHAT Tier   | Phon Internal Name | Aligns With                                |
//! |-------------|--------------------|--------------------------------------------|
//! | `%xmodsyl`  | `TargetSyllables`  | `%mod` (content-based)                      |
//! | `%xphosyl`  | `ActualSyllables`  | `%pho` (content-based)                      |
//! | `%xphoaln`  | `PhoneAlignment`   | `%mod` & `%pho` (positional, word-by-word)  |
//! | `%xphoint`  | `PhoneIntervals`   | `%pho` (per-phone time bullets)             |
//!
//! # Format Examples
//!
//! Syllabified target (each segment has `phoneme:PositionCode`):
//! ```text
//! %modsyl:    ˈb:Oe:Ns:Ct:R m:Oɔ̃:N
//! ```
//!
//! Syllabified actual:
//! ```text
//! %phosyl:    ˈb:Oe:Nt͡j:Oe:Nĭ:Ns:C
//! ```
//!
//! Phone alignment (source↔target pairs, comma within word, space between words):
//! ```text
//! %phoaln:    a↔a,p↔p b↔b,ɛ↔ɛ,t↔t̪
//! ```
//!
//! # Alignment Semantics
//!
//! - **%modsyl → %mod**: Stripping position codes (`:N`, `:O`, `:C`, etc.) and
//!   stress markers (`ˈ`, `ˌ`) from %modsyl should yield the same phonemes as %mod.
//! - **%phosyl → %pho**: Same content-based alignment as %modsyl → %mod.
//! - **%phoaln → %mod & %pho**: Word N in %phoaln aligns with word N in both
//!   %mod and %pho. `∅` represents insertions/deletions.
//!
//! Reference: Phon CHAT Extension Tier Alignment specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

use crate::Span;
use crate::model::{Bullet, NonEmptyString};

// ---------------------------------------------------------------------------
// Syllabified phonology tier (%modsyl, %phosyl)
// ---------------------------------------------------------------------------

/// Which flavour of syllabified phonology tier this is.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub enum SylTierType {
    /// `%modsyl`, syllabified target/model pronunciation.
    Modsyl,
    /// `%phosyl`, syllabified actual/phone production.
    Phosyl,
}

/// A syllabified phonology tier (`%modsyl` or `%phosyl`).
///
/// Content is organized as space-separated **words**, each containing
/// IPA phonemes annotated with syllable position codes
/// (`phoneme:Position` pairs, e.g. `b:Oɛ:Nt:C`).
///
/// Each unit is `phone:CODE`; the legal constituent codes are
/// `O N C L R E A D U` (see [`PositionCode`]). Stress markers (`ˈ` primary,
/// `ˌ` secondary) may precede a segment.
///
/// # Alignment
///
/// Each word aligns 1-to-1 with a word in the corresponding phonological
/// tier (`%mod` for modsyl, `%pho` for phosyl). Stripping position codes
/// and stress markers yields the raw phonemes which must match the
/// corresponding tier's content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct SylTier {
    /// Which tier this is (Modsyl or Phosyl).
    pub tier_type: SylTierType,

    /// Syllabified words (space-separated in CHAT serialization).
    ///
    /// Each word is a raw string containing `phoneme:Position` sequences.
    /// Full segment-level parsing of these strings is deferred, the word
    /// boundary structure is sufficient for alignment validation.
    pub words: Vec<NonEmptyString>,

    /// Source span for error reporting.
    #[serde(skip, default = "crate::Span::dummy")]
    #[schemars(skip)]
    pub span: Span,
}

impl SylTier {
    /// Creates a new syllabified tier from pre-split words.
    pub fn new(tier_type: SylTierType, words: Vec<NonEmptyString>) -> Self {
        Self {
            tier_type,
            words,
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Returns `true` when this tier declares nothing. Derived from
    /// [`Self::word_count`] so the two answers cannot drift.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.word_count() == 0
    }

    /// Returns the number of syllabified words.
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    /// Returns the CHAT tier prefix.
    ///
    /// Currently outputs `%xmodsyl` / `%xphosyl` to match the Phon project's
    /// existing convention. When the tiers are officially adopted into CHAT
    /// (dropping the `x` prefix), update this to `%modsyl` / `%phosyl`.
    pub fn prefix(&self) -> &'static str {
        match self.tier_type {
            SylTierType::Modsyl => "%xmodsyl",
            SylTierType::Phosyl => "%xphosyl",
        }
    }
}

impl std::fmt::Display for SylTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for word in &self.words {
            if !first {
                write!(f, " ")?;
            }
            write!(f, "{}", word)?;
            first = false;
        }
        Ok(())
    }
}

impl super::WriteChat for SylTier {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "{}:\t{}", self.prefix(), self)
    }
}

// ---------------------------------------------------------------------------
// Syllable constituent codes (the `:CODE` of a `phone:CODE` unit)
// ---------------------------------------------------------------------------

/// The syllable-constituent code following the `:` in a `phone:CODE` unit on
/// `%xmodsyl` / `%xphosyl`.
///
/// These are the Phon `SyllableConstituentType` mnemonics that appear on the
/// syllabification tiers. IPA length is written `ː` (U+02D0), so the ASCII `:`
/// (U+003A) separating phone from code is unambiguous. A phone may carry `U`
/// (Unknown) when Phon could not assign it a concrete constituent; this is
/// common on `%xphosyl` (the actual production) even when the model `%xmodsyl`
/// is fully syllabified. The remaining mnemonics, `B` (boundary), `S` (stress),
/// `W` (word boundary) and `T` (tone), are never emitted on these tiers:
/// boundary, stress, and tone need no per-phone marker.
///
/// Reference: Greg Hedlund, "Phon `%x` Dependent Tiers, Format & Validation"
/// (the published spec mistakenly omitted `U`; corrected by Greg 2026-06-23).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionCode {
    /// `O`, syllable onset.
    Onset,
    /// `N`, monophthong nucleus.
    Nucleus,
    /// `C`, syllable coda.
    Coda,
    /// `L`, left appendix (e.g. /s/ in an /s/-stop cluster).
    LeftAppendix,
    /// `R`, right appendix (e.g. final /z/ in a complex coda).
    RightAppendix,
    /// `E`, onset of an empty-headed syllable (e.g. the stop of an affricate).
    Oehs,
    /// `A`, ambisyllabic.
    Ambisyllabic,
    /// `D`, nucleus member of a diphthong/triphthong (treated as a nucleus).
    Diphthong,
    /// `U`, unknown: Phon could not assign a concrete syllable constituent to
    /// this phone. Common on `%xphosyl` (the actual production) where the model
    /// `%xmodsyl` is fully syllabified but a produced segment is unsyllabifiable.
    Unknown,
}

impl PositionCode {
    /// The single CHAT character for this constituent code.
    pub const fn as_char(self) -> char {
        match self {
            PositionCode::Onset => 'O',
            PositionCode::Nucleus => 'N',
            PositionCode::Coda => 'C',
            PositionCode::LeftAppendix => 'L',
            PositionCode::RightAppendix => 'R',
            PositionCode::Oehs => 'E',
            PositionCode::Ambisyllabic => 'A',
            PositionCode::Diphthong => 'D',
            PositionCode::Unknown => 'U',
        }
    }
}

impl TryFrom<char> for PositionCode {
    /// The offending character when it is not a legal constituent code.
    type Error = char;

    fn try_from(c: char) -> Result<Self, char> {
        match c {
            'O' => Ok(PositionCode::Onset),
            'N' => Ok(PositionCode::Nucleus),
            'C' => Ok(PositionCode::Coda),
            'L' => Ok(PositionCode::LeftAppendix),
            'R' => Ok(PositionCode::RightAppendix),
            'E' => Ok(PositionCode::Oehs),
            'A' => Ok(PositionCode::Ambisyllabic),
            'D' => Ok(PositionCode::Diphthong),
            'U' => Ok(PositionCode::Unknown),
            other => Err(other),
        }
    }
}

impl std::fmt::Display for PositionCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

/// One `phone:CODE` unit parsed from a syllabification word.
///
/// Produced on demand by [`tokenize_syl_word`] for validation. `SylTier` stores
/// words as raw strings (consistent with how `%pho`/`%mod` store flat phone
/// words), so this typed view is the boundary at which the `phone:CODE`
/// structure is checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyllableUnit {
    /// The IPA phone, verbatim, identical to the corresponding source-tier phone.
    pub phone: NonEmptyString,
    /// The syllable constituent this phone fills.
    pub code: PositionCode,
}

/// The intra-word pause marker (`^`, U+005E) on syllabification tiers.
///
/// Per the Phon `%x` tier spec: intra-word pauses occur inside a word on
/// `%mod`/`%pho`, pass through verbatim on the syllabification tiers, and are
/// excluded from `%xphoaln` alignment while still occupying time. A bare `^`
/// carries no `:CODE` suffix and may appear between units or after the last
/// unit in a word (word-final placement is legal; rule 3 governs
/// reconstruction, not rule 2's "between units" wording).
const INTRA_WORD_PAUSE: char = '^';

/// One token of a tokenized syllabification word: either a `phone:CODE` unit
/// or a bare intra-word pause.
///
/// Reconstruction (Phon `%x` tier spec rule 3: strip `:CODE` from every unit
/// and concatenate the phones, in order, with pause characters preserved in
/// place, to reproduce the source `%mod`/`%pho` word exactly) must visit
/// every variant here. Representing a syllabification word as `Vec<SylToken>`
/// rather than `Vec<SyllableUnit>` is what makes an exhaustive match the only
/// way to write that reconstruction: the prior struct-only representation let
/// a `^` silently fuse into a phone string instead of being carried as its
/// own token, and [`reconstruct_syl_word`]'s exhaustive match is the fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SylToken {
    /// A `phone:CODE` syllable-constituent unit.
    Unit(SyllableUnit),
    /// A bare `^` intra-word pause, carrying no `:CODE` suffix.
    IntraWordPause,
}

/// Reconstructs the source-tier word text from tokenized syllabification
/// tokens.
///
/// Phon `%x` tier spec rule 3: stripping the `:CODE` from every unit and
/// concatenating the phones, in order, must reproduce the corresponding
/// `%mod`/`%pho` word exactly; pause characters, which carry no code, are
/// preserved in place. The match is exhaustive over [`SylToken`] (no `_ =>`
/// arm), so a future variant cannot be silently dropped from reconstruction
/// the way `^` previously was.
pub fn reconstruct_syl_word(tokens: &[SylToken]) -> String {
    tokens
        .iter()
        .map(|token| match token {
            SylToken::Unit(unit) => unit.phone.as_str(),
            SylToken::IntraWordPause => "^",
        })
        .collect()
}

/// Why a syllabification word failed to tokenize into `phone:CODE` units.
///
/// These map to the syllabification validation diagnostics: a structurally
/// malformed unit and an illegal constituent code are distinct conditions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SylWordError {
    /// The word (or a trailing fragment) contains no `:` separator.
    #[error("syllabification unit has no ':' separator: {0:?}")]
    MissingColon(String),
    /// A unit had an empty phone before its `:CODE`.
    #[error("syllabification unit has an empty phone before ':{0}'")]
    EmptyPhone(char),
    /// A unit ended at `:` with no constituent code character.
    #[error("syllabification unit is missing its constituent code after ':'")]
    EmptyCode,
    /// The character after `:` is not one of the legal codes `O N C L R E A D U`.
    #[error("'{0}' is not a legal syllable-constituent code (expected one of O N C L R E A D U)")]
    IllegalCode(char),
}

impl SylWordError {
    /// True when this is the illegal-constituent-code condition (vs a structural
    /// malformation), so the validator can pick the right diagnostic.
    pub fn is_illegal_code(&self) -> bool {
        matches!(self, SylWordError::IllegalCode(_))
    }
}

/// Classification of one whitespace-delimited word on a syllabification tier.
///
/// Phon keeps every word-aligned phonology tier in index lockstep with the
/// main tier. When the main tier carries a pause, Phon mirrors the pause
/// token at the same word position on `%mod`, `%pho`, `%xmodsyl`, and
/// `%xphosyl` (and as a `(..)↔(..)` pair on `%xphoaln`). Such a filler is
/// not a syllabified word and carries no `phone:CODE` structure, so the
/// classifier recognizes it BEFORE unit tokenization.
#[derive(Debug, Clone, PartialEq)]
pub enum SylWordKind {
    /// A pause filler (`(.)`, `(..)`, `(...)`, or numeric `(x.x)`) mirroring
    /// a pause at the same word position on the source tier. Greg Hedlund's
    /// "Phon `%x` Dependent Tiers" spec lists numeric inter-word pauses as
    /// legal alongside the three untimed forms (§"Pauses"); an earlier
    /// version of this classifier excluded them on "unattested in the wild
    /// corpora" grounds, which is not a basis for rejecting a construct the
    /// authority declares legal.
    PauseFiller(crate::model::PauseDuration),
    /// A syllabified word: a sequence of `phone:CODE` units and bare `^`
    /// intra-word pauses, in source order.
    Units(Vec<SylToken>),
}

/// True when `word` is a legal inter-word pause marker: `(.)`, `(..)`,
/// `(...)`, or numeric `(<duration>)` (seconds, or minutes:seconds).
///
/// Shared by the syllabification-tier pause-filler classifier and the
/// `%xphoaln` one-sided-pause word-count exception (Greg Hedlund's spec,
/// §2 rule 5): a pause word present on only one of `%mod`/`%pho` consumes a
/// word slot only on the tier that contains it.
pub fn is_pause_marker(word: &str) -> bool {
    matches!(word, "(.)" | "(..)" | "(...)") || numeric_pause_duration(word).is_some()
}

/// Parse `word` as a numeric inter-word pause marker `(<duration>)`, returning
/// its typed duration when it is one, `None` otherwise (including for the
/// three untimed forms, which [`classify_syl_word`] matches directly).
fn numeric_pause_duration(word: &str) -> Option<crate::model::PauseTimedDuration> {
    use crate::model::PauseTimedDuration;
    let inner = word.strip_prefix('(')?.strip_suffix(')')?;
    match PauseTimedDuration::new(inner) {
        parsed @ PauseTimedDuration::Parsed { .. } => Some(parsed),
        PauseTimedDuration::Unsupported(_) => None,
    }
}

/// Classify one syllabification-tier word: pause filler or `phone:CODE` units.
///
/// The untimed pause forms are matched exactly (the serialized forms of the
/// untimed [`crate::model::PauseDuration`] variants); a numeric pause is
/// recognized via `numeric_pause_duration`; anything else must tokenize as
/// units via [`tokenize_syl_word`].
pub fn classify_syl_word(word: &str) -> Result<SylWordKind, SylWordError> {
    use crate::model::PauseDuration;
    match word {
        "(.)" => Ok(SylWordKind::PauseFiller(PauseDuration::Short)),
        "(..)" => Ok(SylWordKind::PauseFiller(PauseDuration::Medium)),
        "(...)" => Ok(SylWordKind::PauseFiller(PauseDuration::Long)),
        _ => match numeric_pause_duration(word) {
            Some(duration) => Ok(SylWordKind::PauseFiller(PauseDuration::Timed(duration))),
            None => tokenize_syl_word(word).map(SylWordKind::Units),
        },
    }
}

/// Tokenize one syllabification word (e.g. `k:Oæ:Nt:C`) into `phone:CODE`
/// units and bare `^` intra-word pauses.
///
/// Units concatenate with no internal whitespace; a phone may be any
/// multi-codepoint IPA sequence (length is written `ː`, U+02D0, never ASCII
/// `:`), so each ASCII `:` unambiguously introduces a one-character constituent
/// code. A bare `^` (with no `:CODE` suffix) may occur between units or after
/// the last unit; it is checked for at each token boundary before attempting
/// to find a `:`, so it can never fuse into an adjacent phone string.
pub fn tokenize_syl_word(word: &str) -> Result<Vec<SylToken>, SylWordError> {
    const COLON: char = ':';
    if !word.contains(COLON) {
        return Err(SylWordError::MissingColon(word.to_string()));
    }
    let mut tokens = Vec::new();
    let mut rest = word;
    while !rest.is_empty() {
        if let Some(after_pause) = rest.strip_prefix(INTRA_WORD_PAUSE) {
            tokens.push(SylToken::IntraWordPause);
            rest = after_pause;
            continue;
        }
        let Some(colon) = rest.find(COLON) else {
            return Err(SylWordError::MissingColon(rest.to_string()));
        };
        let phone_str = &rest[..colon];
        let after = &rest[colon + COLON.len_utf8()..];
        let Some(code_char) = after.chars().next() else {
            return Err(SylWordError::EmptyCode);
        };
        let phone =
            NonEmptyString::new(phone_str).map_err(|_| SylWordError::EmptyPhone(code_char))?;
        let code = PositionCode::try_from(code_char).map_err(SylWordError::IllegalCode)?;
        tokens.push(SylToken::Unit(SyllableUnit { phone, code }));
        rest = &after[code_char.len_utf8()..];
    }
    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Phone alignment tier (%phoaln)
// ---------------------------------------------------------------------------

/// A single segment alignment pair from `%phoaln`.
///
/// Represents the mapping of one phonological segment (from %mod/modsyl)
/// to one phonetic segment (from %pho/phosyl). `None` represents the null
/// symbol `∅`, indicating an insertion or deletion.
///
/// # Format
///
/// `source↔target` where either side may be `∅`:
/// - `a↔a`, identity mapping
/// - `ɪ↔ɛ`, substitution (lowering)
/// - `∅↔ʔ`, insertion (epenthesis)
/// - `b↔∅`, deletion (elision)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct AlignmentPair {
    /// Source segment (from target/model), `None` = `∅` (insertion).
    pub source: Option<NonEmptyString>,
    /// Target segment (from actual/phone), `None` = `∅` (deletion).
    pub target: Option<NonEmptyString>,
}

impl std::fmt::Display for AlignmentPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.source, &self.target) {
            (Some(s), Some(t)) => write!(f, "{}↔{}", s, t),
            (Some(s), None) => write!(f, "{}↔∅", s),
            (None, Some(t)) => write!(f, "∅↔{}", t),
            (None, None) => write!(f, "∅↔∅"),
        }
    }
}

/// Word-level alignment: a sequence of segment alignment pairs.
///
/// Corresponds to one word position in the utterance. Pairs are
/// comma-separated in CHAT serialization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct WordAlignment {
    /// Segment-level alignment pairs for this word.
    pub pairs: Vec<AlignmentPair>,
}

impl std::fmt::Display for WordAlignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for pair in &self.pairs {
            if !first {
                write!(f, ",")?;
            }
            write!(f, "{}", pair)?;
            first = false;
        }
        Ok(())
    }
}

/// Phone alignment tier (`%phoaln`).
///
/// Provides a segmental alignment between the target (model) and actual
/// (phone) IPA transcriptions, organized word-by-word.
///
/// # Format
///
/// `source↔target` pairs are comma-separated within a word, and words
/// are space-separated:
/// ```text
/// %phoaln:    a↔a,p↔p b↔b,ɛ↔ɛ,t↔t̪
/// ```
///
/// The null symbol `∅` marks insertions (source=∅) or deletions (target=∅).
///
/// # Alignment
///
/// Word N in %phoaln aligns positionally with word N in both %mod and %pho.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct PhoalnTier {
    /// Per-word alignment data.
    pub words: Vec<WordAlignment>,

    /// Source span for error reporting.
    #[serde(skip, default = "crate::Span::dummy")]
    #[schemars(skip)]
    pub span: Span,
}

impl PhoalnTier {
    /// Creates a new phone alignment tier from pre-parsed word alignments.
    pub fn new(words: Vec<WordAlignment>) -> Self {
        Self {
            words,
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Returns `true` when this tier declares nothing. Derived from
    /// [`Self::word_count`] so the two answers cannot drift.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.word_count() == 0
    }

    /// Returns the number of aligned words.
    pub fn word_count(&self) -> usize {
        self.words.len()
    }
}

impl std::fmt::Display for PhoalnTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for word in &self.words {
            if !first {
                write!(f, " ")?;
            }
            write!(f, "{}", word)?;
            first = false;
        }
        Ok(())
    }
}

impl super::WriteChat for PhoalnTier {
    /// Serializes as `%xphoaln:` to match Phon's current convention.
    /// When officially adopted into CHAT, update to `%phoaln:`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "%xphoaln:\t{}", self)
    }
}

// ---------------------------------------------------------------------------
// Phone interval tier (%xphoint)
// ---------------------------------------------------------------------------

/// The CLAN time-bullet delimiter (`0x15`, NEGATIVE ACKNOWLEDGE).
const BULLET_DELIM: char = '\u{0015}';

/// Word-group separator on `%xphoint`: space, slash, space. A distinct separator
/// is needed because single spaces already separate phone and bullet tokens
/// inside a group.
const XPHOINT_GROUP_SEP: &str = " / ";

/// One phone and its CLAN time-alignment bullet on `%xphoint`.
///
/// The bullet uses the same `\u{0015}start_end\u{0015}` convention CLAN uses on
/// `%wor` and utterance lines, but per phone rather than per word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct PhoneInterval {
    /// The phone, identical to the corresponding phone of the `%pho` word.
    pub phone: NonEmptyString,
    /// The phone's time interval (start/end media offsets, milliseconds).
    pub bullet: Bullet,
}

impl std::fmt::Display for PhoneInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.phone, self.bullet)
    }
}

/// One word-group on `%xphoint`: the time-aligned phones of a single `%pho`
/// word. Groups are separated by ` / ` in serialization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct XphointGroup {
    /// The phones of this word, each with its bullet, in order.
    pub phones: Vec<PhoneInterval>,
}

impl std::fmt::Display for XphointGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for phone in &self.phones {
            if !first {
                write!(f, " ")?;
            }
            write!(f, "{}", phone)?;
            first = false;
        }
        Ok(())
    }
}

/// Phone interval tier (`%xphoint`): the per-phone time segmentation of `%pho`.
///
/// Each `%pho` word becomes a group of `(phone, bullet)` pairs; groups are
/// separated by ` / `. Analogous to `%wor` word timing, one level finer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct XphointTier {
    /// One group per `%pho` word, in order.
    pub groups: Vec<XphointGroup>,

    /// Source span for error reporting.
    #[serde(skip, default = "crate::Span::dummy")]
    #[schemars(skip)]
    pub span: Span,
}

impl XphointTier {
    /// Creates a new phone interval tier from parsed groups.
    pub fn new(groups: Vec<XphointGroup>) -> Self {
        Self {
            groups,
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Returns `true` when this tier declares nothing. Derived from
    /// [`Self::word_count`] so the two answers cannot drift.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.word_count() == 0
    }

    /// Number of word groups (aligns 1-to-1 with `%pho` words).
    pub fn word_count(&self) -> usize {
        self.groups.len()
    }
}

impl std::fmt::Display for XphointTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for group in &self.groups {
            if !first {
                write!(f, "{}", XPHOINT_GROUP_SEP)?;
            }
            write!(f, "{}", group)?;
            first = false;
        }
        Ok(())
    }
}

impl super::WriteChat for XphointTier {
    /// Serializes as `%xphoint:` to match Phon's convention.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "%xphoint:\t{}", self)
    }
}

/// Errors from parsing `%xphoint` content into time-aligned phone groups.
///
/// Structural problems (a phone with no bullet, a bullet that is not
/// `\u{0015}<int>_<int>\u{0015}`) are parse errors. Semantic problems
/// (`start >= end`, non-monotonic intervals, phones not reproducing `%pho`) are
/// validation concerns checked later, so a well-formed-but-invalid bullet still
/// parses here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum XphointParseError {
    /// A group contained no phone/bullet tokens.
    #[error("empty %xphoint group")]
    EmptyGroup,
    /// A phone token was not followed by a bullet token.
    #[error("%xphoint phone {0:?} is not followed by a time bullet")]
    MissingBullet(String),
    /// A bullet token was not `\u{0015}<int>_<int>\u{0015}`.
    #[error("malformed %xphoint bullet: {0:?}")]
    MalformedBullet(String),
    /// A bullet's start or end was not a non-negative integer.
    #[error("%xphoint bullet has a non-integer offset: {0:?}")]
    NonIntegerOffset(String),
    /// An empty phone token.
    #[error("empty phone token in %xphoint group")]
    EmptyPhone,
}

/// Parse a `%xphoint` content string into time-aligned phone groups.
///
/// Groups are separated by ` / `; within a group, tokens alternate phone then
/// bullet (`\u{0015}start_end\u{0015}`).
pub fn parse_xphoint_content(content: &str) -> Result<Vec<XphointGroup>, XphointParseError> {
    let mut groups = Vec::new();
    for group_str in content.trim().split(XPHOINT_GROUP_SEP) {
        groups.push(parse_xphoint_group(group_str)?);
    }
    Ok(groups)
}

/// Parse one ` / `-delimited group: alternating phone and bullet tokens.
fn parse_xphoint_group(group: &str) -> Result<XphointGroup, XphointParseError> {
    let tokens: Vec<&str> = group.split_whitespace().collect();
    let mut phones = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let phone_tok = tokens[i];
        let bullet_tok = tokens
            .get(i + 1)
            .ok_or_else(|| XphointParseError::MissingBullet(phone_tok.to_string()))?;
        let phone = NonEmptyString::new(phone_tok).map_err(|_| XphointParseError::EmptyPhone)?;
        let bullet = parse_xphoint_bullet(bullet_tok)?;
        phones.push(PhoneInterval { phone, bullet });
        i += 2;
    }
    if phones.is_empty() {
        return Err(XphointParseError::EmptyGroup);
    }
    Ok(XphointGroup { phones })
}

/// Parse one `\u{0015}start_end\u{0015}` bullet token into a [`Bullet`].
///
/// Only the integer parse is enforced here; `start < end` is a validation rule.
fn parse_xphoint_bullet(tok: &str) -> Result<Bullet, XphointParseError> {
    let inner = tok
        .strip_prefix(BULLET_DELIM)
        .and_then(|s| s.strip_suffix(BULLET_DELIM))
        .ok_or_else(|| XphointParseError::MalformedBullet(tok.to_string()))?;
    let (start, end) = inner
        .split_once('_')
        .ok_or_else(|| XphointParseError::MalformedBullet(tok.to_string()))?;
    let start: u64 = start
        .parse()
        .map_err(|_| XphointParseError::NonIntegerOffset(tok.to_string()))?;
    let end: u64 = end
        .parse()
        .map_err(|_| XphointParseError::NonIntegerOffset(tok.to_string()))?;
    Ok(Bullet::new(start, end))
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse a `%phoaln` content string into word alignments.
///
/// Format: space-separated words, each word has comma-separated `source↔target`
/// pairs where either side may be `∅`.
pub fn parse_phoaln_content(content: &str) -> Result<Vec<WordAlignment>, PhoalnParseError> {
    let mut words = Vec::new();

    for word_str in content.split_whitespace() {
        let mut pairs = Vec::new();
        for pair_str in word_str.split(',') {
            let pair = parse_alignment_pair(pair_str)?;
            pairs.push(pair);
        }
        if pairs.is_empty() {
            return Err(PhoalnParseError::EmptyWord);
        }
        words.push(WordAlignment { pairs });
    }

    Ok(words)
}

/// Parse a single `source↔target` alignment pair.
///
/// The spec requires exactly one `↔` per pair; a second arrow most often
/// means a missing space swallowed a word boundary (`a↔b c↔c` typed as
/// `a↔bc↔c`), which would otherwise silently fold into one malformed
/// segment rather than surface as the structural error it is.
fn parse_alignment_pair(s: &str) -> Result<AlignmentPair, PhoalnParseError> {
    // The ↔ character is U+2194 (LEFT RIGHT ARROW), 3 bytes in UTF-8
    let mut arrows = s.match_indices('↔');
    let Some((arrow_pos, _)) = arrows.next() else {
        return Err(PhoalnParseError::MissingArrow(s.to_string()));
    };
    if arrows.next().is_some() {
        return Err(PhoalnParseError::MultipleArrows(s.to_string()));
    }

    let source_str = &s[..arrow_pos];
    let target_str = &s[arrow_pos + '↔'.len_utf8()..];

    let source = if source_str == "∅" || source_str.is_empty() {
        None
    } else {
        Some(NonEmptyString::new(source_str).map_err(|_| PhoalnParseError::EmptySegment)?)
    };

    let target = if target_str == "∅" || target_str.is_empty() {
        None
    } else {
        Some(NonEmptyString::new(target_str).map_err(|_| PhoalnParseError::EmptySegment)?)
    };

    Ok(AlignmentPair { source, target })
}

/// Errors from parsing `%phoaln` content.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PhoalnParseError {
    /// Missing `↔` separator in an alignment pair.
    #[error("missing '↔' separator in alignment pair: {0}")]
    MissingArrow(String),
    /// More than one `↔` separator in an alignment pair (the spec requires
    /// exactly one).
    #[error("more than one '↔' separator in alignment pair: {0}")]
    MultipleArrows(String),
    /// Empty word (no alignment pairs).
    #[error("empty word in alignment (no pairs)")]
    EmptyWord,
    /// Empty segment string (not ∅, just empty).
    #[error("empty segment string in alignment pair")]
    EmptySegment,
}

/// Parse `%modsyl` or `%phosyl` content into word strings.
///
/// Simply splits on whitespace to get word-level boundaries.
/// Within-word segment parsing (position codes) is deferred.
pub fn parse_syl_content(content: &str) -> Vec<NonEmptyString> {
    content
        .split_whitespace()
        .filter_map(|token| NonEmptyString::new(token).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_phoaln() {
        let words = parse_phoaln_content("a↔a,p↔p b↔b,ɛ↔ɛ,t↔t̪").unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].pairs.len(), 2);
        assert_eq!(words[1].pairs.len(), 3);
        assert_eq!(words[0].to_string(), "a↔a,p↔p");
        assert_eq!(words[1].to_string(), "b↔b,ɛ↔ɛ,t↔t̪");
    }

    #[test]
    fn parse_phoaln_with_null_segments() {
        let words = parse_phoaln_content("∅↔ʔ,æ̃↔ʌ̃,n↔n ð↔d,æ↔æ,t↔tʰ").unwrap();
        assert_eq!(words.len(), 2);
        assert!(words[0].pairs[0].source.is_none());
        assert_eq!(words[0].pairs[0].target.as_ref().unwrap().as_str(), "ʔ");
    }

    #[test]
    fn parse_phoaln_deletion() {
        let words = parse_phoaln_content("b↔∅").unwrap();
        assert_eq!(words[0].pairs[0].source.as_ref().unwrap().as_str(), "b");
        assert!(words[0].pairs[0].target.is_none());
    }

    #[test]
    fn roundtrip_phoaln() {
        let input = "a↔a,p↔p b↔b,ɛ↔ɛ,t↔t̪";
        let words = parse_phoaln_content(input).unwrap();
        let tier = PhoalnTier::new(words);
        assert_eq!(tier.to_string(), input);
    }

    #[test]
    fn roundtrip_phoaln_with_nulls() {
        let input = "∅↔ʔ,æ̃↔ʌ̃ b↔∅";
        let words = parse_phoaln_content(input).unwrap();
        let tier = PhoalnTier::new(words);
        assert_eq!(tier.to_string(), input);
    }

    #[test]
    fn parse_syl_words() {
        let words = parse_syl_content("ˈb:Oe:Ns:Ct:R m:Oɔ̃:N");
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].as_str(), "ˈb:Oe:Ns:Ct:R");
        assert_eq!(words[1].as_str(), "m:Oɔ̃:N");
    }

    #[test]
    fn syl_tier_roundtrip() {
        let words = parse_syl_content("ˈb:Oe:Ns:Ct:R m:Oɔ̃:N");
        let tier = SylTier::new(SylTierType::Modsyl, words);
        assert_eq!(tier.to_string(), "ˈb:Oe:Ns:Ct:R m:Oɔ̃:N");

        let mut chat = String::new();
        super::super::WriteChat::write_chat(&tier, &mut chat).unwrap();
        assert_eq!(chat, "%xmodsyl:\tˈb:Oe:Ns:Ct:R m:Oɔ̃:N");
    }

    #[test]
    fn phoaln_write_chat() {
        let words = parse_phoaln_content("a↔a,p↔p").unwrap();
        let tier = PhoalnTier::new(words);
        let mut chat = String::new();
        super::super::WriteChat::write_chat(&tier, &mut chat).unwrap();
        assert_eq!(chat, "%xphoaln:\ta↔a,p↔p");
    }

    #[test]
    fn missing_arrow_error() {
        let result = parse_phoaln_content("a,b");
        assert!(result.is_err());
    }

    /// The spec requires exactly one `↔` per pair; a second arrow (e.g. a
    /// swallowed space between two pairs) must be rejected, not silently
    /// folded into one malformed segment.
    #[test]
    fn multiple_arrows_error() {
        let result = parse_phoaln_content("a↔b↔c");
        assert!(matches!(result, Err(PhoalnParseError::MultipleArrows(_))));
    }

    #[test]
    fn position_code_roundtrips_all_legal_chars() {
        for c in ['O', 'N', 'C', 'L', 'R', 'E', 'A', 'D', 'U'] {
            let code = PositionCode::try_from(c).expect("legal code");
            assert_eq!(code.as_char(), c);
        }
    }

    #[test]
    fn position_code_rejects_illegal_char() {
        assert_eq!(PositionCode::try_from('Z'), Err('Z'));
        // 'S' (stress) and 'B' (boundary) are not emitted on these tiers.
        assert_eq!(PositionCode::try_from('S'), Err('S'));
    }

    /// Unwraps a [`SylToken::Unit`], panicking (test-only) with a clear
    /// message if the token was an [`SylToken::IntraWordPause`] instead.
    fn unit(token: &SylToken) -> &SyllableUnit {
        match token {
            SylToken::Unit(u) => u,
            SylToken::IntraWordPause => panic!("expected a Unit token, got IntraWordPause"),
        }
    }

    #[test]
    fn tokenize_syl_word_splits_units() {
        let units = tokenize_syl_word("k:Oæ:Nt:C").expect("well-formed");
        assert_eq!(units.len(), 3);
        assert_eq!(unit(&units[0]).phone.as_str(), "k");
        assert_eq!(unit(&units[0]).code, PositionCode::Onset);
        assert_eq!(unit(&units[1]).phone.as_str(), "æ");
        assert_eq!(unit(&units[1]).code, PositionCode::Nucleus);
        assert_eq!(unit(&units[2]).code, PositionCode::Coda);
    }

    #[test]
    fn tokenize_syl_word_preserves_multibyte_phone() {
        // ʌ̾ is U+028C + U+033E (combining); the ASCII ':' still delimits.
        let units = tokenize_syl_word("ʌ̾:N").expect("well-formed");
        assert_eq!(units.len(), 1);
        assert_eq!(unit(&units[0]).phone.as_str(), "ʌ̾");
        assert_eq!(unit(&units[0]).code, PositionCode::Nucleus);
    }

    /// Phon `%x` tier spec: "An intra-word pause appears inside a word as a
    /// bare `^` between units, with no `:CODE` suffix, e.g.
    /// `b:Oʌ:N^b:Oʌ:N`." The pause must land as its own token, never fused
    /// into the phone before or after it.
    #[test]
    fn tokenize_syl_word_mid_word_intra_word_pause() {
        let tokens = tokenize_syl_word("b:Oʌ:N^b:Oʌ:N").expect("well-formed");
        assert_eq!(tokens.len(), 5);
        assert_eq!(unit(&tokens[0]).phone.as_str(), "b");
        assert_eq!(unit(&tokens[0]).code, PositionCode::Onset);
        assert_eq!(unit(&tokens[1]).phone.as_str(), "ʌ");
        assert_eq!(unit(&tokens[1]).code, PositionCode::Nucleus);
        assert_eq!(tokens[2], SylToken::IntraWordPause);
        assert_eq!(unit(&tokens[3]).phone.as_str(), "b");
        assert_eq!(unit(&tokens[3]).code, PositionCode::Onset);
        assert_eq!(unit(&tokens[4]).phone.as_str(), "ʌ");
        assert_eq!(unit(&tokens[4]).code, PositionCode::Nucleus);
    }

    /// Already adjudicated (do not re-litigate): word-final `^` is legal.
    /// Rule 2's "between units" describes the common case; rule 3 governs.
    /// Verified against the real corpus occurrence `%xphosyl: d:Oo:Nd:Oo:N^`
    /// (source `%pho: dodo^`), minimized here to one unit.
    #[test]
    fn tokenize_syl_word_word_final_intra_word_pause() {
        let tokens = tokenize_syl_word("k:O^").expect("well-formed");
        assert_eq!(tokens.len(), 2);
        assert_eq!(unit(&tokens[0]).phone.as_str(), "k");
        assert_eq!(unit(&tokens[0]).code, PositionCode::Onset);
        assert_eq!(tokens[1], SylToken::IntraWordPause);
    }

    /// A bare caret with no `phone:CODE` unit at all is not a legal
    /// syllabification word (the spec's intra-word-pause wording presupposes
    /// at least one unit); it must still be rejected as code-less, not
    /// silently accepted as a zero-unit tokenization.
    #[test]
    fn tokenize_syl_word_rejects_bare_pause_with_no_units() {
        assert_eq!(
            tokenize_syl_word("^").unwrap_err(),
            SylWordError::MissingColon("^".to_string())
        );
    }

    #[test]
    fn tokenize_syl_word_reports_illegal_code() {
        let err = tokenize_syl_word("t:Z").unwrap_err();
        assert_eq!(err, SylWordError::IllegalCode('Z'));
        assert!(err.is_illegal_code());
    }

    #[test]
    fn tokenize_syl_word_reports_missing_colon() {
        assert_eq!(
            tokenize_syl_word("kæt").unwrap_err(),
            SylWordError::MissingColon("kæt".to_string())
        );
    }

    #[test]
    fn tokenize_syl_word_reports_empty_phone() {
        assert_eq!(
            tokenize_syl_word(":O").unwrap_err(),
            SylWordError::EmptyPhone('O')
        );
    }

    #[test]
    fn reconstruct_syllabification_yields_source_word() {
        let tokens = tokenize_syl_word("k:Oæ:Nt:C").unwrap();
        assert_eq!(reconstruct_syl_word(&tokens), "kæt");
    }

    /// Real corpus occurrence: `%pho: dodo^`, `%xphosyl: d:Oo:Nd:Oo:N^`.
    /// Reconstruction must reproduce the source word exactly, caret included.
    #[test]
    fn reconstruct_syl_word_preserves_word_final_pause() {
        let tokens = tokenize_syl_word("d:Oo:Nd:Oo:N^").unwrap();
        assert_eq!(reconstruct_syl_word(&tokens), "dodo^");
    }

    /// Real corpus occurrence: `%pho: u̯e̞ə^ˈtʰ^`, `%xphosyl:
    /// u̯:Ne̞:Nə:N^ˈtʰ:C^`. Exercises BOTH a mid-word pause (between `ə:N` and
    /// `ˈtʰ:C`) and a word-final pause in the same word.
    #[test]
    fn reconstruct_syl_word_preserves_mid_and_final_pause() {
        let tokens = tokenize_syl_word("u̯:Ne̞:Nə:N^ˈtʰ:C^").unwrap();
        assert_eq!(reconstruct_syl_word(&tokens), "u̯e̞ə^ˈtʰ^");
    }

    #[test]
    fn parse_xphoint_groups_and_bullets() {
        let content =
            "t \u{0015}0_110\u{0015} w \u{0015}110_220\u{0015} / b \u{0015}220_330\u{0015}";
        let groups = parse_xphoint_content(content).expect("well-formed");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].phones.len(), 2);
        assert_eq!(groups[0].phones[0].phone.as_str(), "t");
        assert_eq!(groups[0].phones[0].bullet.timing.start_ms, 0);
        assert_eq!(groups[0].phones[0].bullet.timing.end_ms, 110);
        assert_eq!(groups[1].phones[0].phone.as_str(), "b");
    }

    #[test]
    fn xphoint_roundtrips_via_display() {
        let content =
            "t \u{0015}0_110\u{0015} w \u{0015}110_220\u{0015} / b \u{0015}220_330\u{0015}";
        let groups = parse_xphoint_content(content).unwrap();
        let tier = XphointTier::new(groups);
        assert_eq!(tier.to_string(), content);
    }

    #[test]
    fn parse_xphoint_accepts_inverted_bullet() {
        // start >= end is a validation concern (E742), not a parse error.
        let groups = parse_xphoint_content("t \u{0015}60_5\u{0015}").expect("parses");
        assert_eq!(groups[0].phones[0].bullet.timing.start_ms, 60);
        assert_eq!(groups[0].phones[0].bullet.timing.end_ms, 5);
    }

    #[test]
    fn parse_xphoint_rejects_dangling_phone() {
        assert_eq!(
            parse_xphoint_content("t").unwrap_err(),
            XphointParseError::MissingBullet("t".to_string())
        );
    }

    /// Greg Hedlund's spec (§"Pauses") lists numeric inter-word pauses
    /// (`(x.x)`) as legal alongside `(.)`/`(..)`/`(...)`; a numeric pause
    /// word on a syllabification tier must classify as a pause filler, not
    /// fail `phone:CODE` tokenization.
    #[test]
    fn classify_syl_word_accepts_numeric_pause() {
        use crate::model::{PauseDuration, PauseTimedDuration};
        let kind = classify_syl_word("(1.5)").expect("numeric pause is a legal filler");
        assert_eq!(
            kind,
            SylWordKind::PauseFiller(PauseDuration::Timed(PauseTimedDuration::new("1.5")))
        );
    }

    #[test]
    fn classify_syl_word_accepts_minutes_seconds_numeric_pause() {
        let kind = classify_syl_word("(1:02.5)").expect("minutes:seconds pause is legal");
        assert!(matches!(
            kind,
            SylWordKind::PauseFiller(crate::model::PauseDuration::Timed(_))
        ));
    }

    #[test]
    fn is_pause_marker_recognizes_all_legal_forms() {
        for marker in ["(.)", "(..)", "(...)", "(1.5)", "(2)", "(1:02.5)"] {
            assert!(is_pause_marker(marker), "{marker} should be a pause marker");
        }
        for not_a_pause in ["(x)", "k:O", "kæt", "()"] {
            assert!(
                !is_pause_marker(not_a_pause),
                "{not_a_pause} should not be a pause marker"
            );
        }
    }
}
