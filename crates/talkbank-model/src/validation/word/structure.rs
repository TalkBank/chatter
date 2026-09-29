//! Structural/prosodic validators for main-tier word tokens.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#WordInternalPause_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Part_of_Speech>

use crate::model::content::word::{MarkerSpelling, UntranscribedStatus};
use crate::model::{Word, WordContent, WordMaterial, WordStressMarkerType};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Enforce character-level hygiene for the normalized word surface.
///
/// Words may NOT contain:
/// - Whitespace (spaces, tabs, newlines)
/// - Bullet markers (U+0015 / byte 0x15)
/// - Other control characters
///
/// NOTE: Validates cleaned_text, NOT raw_text. Raw text may contain formatting markers
/// (underline U+0001, U+0002, etc.) that are parsed into word content structure.
///
/// This validation catches parser bugs where word boundaries are incorrectly determined.
pub(crate) fn check_word_characters(word: &Word, errors: &impl ErrorSink) {
    let cleaned = word.cleaned_text();

    // Orthographic leaves cannot encode structure owned by typed markers.
    // Phonetic leaves have a separate alphabet and are deliberately excluded.
    for content in word.content() {
        let text = match content {
            WordContent::Text(text) => Some(text.as_ref()),
            WordContent::Shortening(text) => Some(text.as_ref()),
            WordContent::Phonetic(_)
            | WordContent::OverlapPoint(_)
            | WordContent::CAElement(_)
            | WordContent::CADelimiter(_)
            | WordContent::StressMarker(_)
            | WordContent::Lengthening(_)
            | WordContent::SyllablePause(_)
            | WordContent::UnderlineBegin(_)
            | WordContent::UnderlineEnd(_)
            | WordContent::CompoundMarker(_)
            | WordContent::CliticBoundary(_) => None,
        };
        if let Some(text) = text
            && text.contains(['@', '(', ')'])
        {
            errors.report(ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(text, word.span, text),
                "Lexical text contains a structural delimiter; use typed word markers",
            ));
        }
    }

    // Check for whitespace
    if cleaned.chars().any(|c| c.is_whitespace()) {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(cleaned, word.span, cleaned),
                "Word contains illegal whitespace characters",
            )
            .with_suggestion(
                "Words must not contain spaces, tabs, or newlines. Check word boundaries in %wor tiers and main tier.",
            ),
        );
    }

    // Check for bullet marker (unit separator U+0015)
    if cleaned.as_bytes().contains(&0x15) {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(cleaned, word.span, cleaned),
                "Word contains illegal bullet marker (U+0015)",
            )
            .with_suggestion(
                "Bullet markers should not be part of word text. This is likely a parser bug.",
            ),
        );
    }

    // Check for the %mor tier delimiter `|` (CLAN CHECK 48, bare-pipe shape;
    // spec E243_pipe_in_word.md): it has no meaning in main-tier word text.
    if cleaned.contains('|') {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(cleaned, word.span, cleaned),
                "Word contains reserved tier-delimiter character '|'",
            )
            .with_suggestion(
                "The pipe character belongs to %mor tier syntax; remove it from main-tier word text.",
            ),
        );
    }

    // A repetition annotation is typed syntax, not a slash Word. Free-text
    // comments do not enter word validation; embedded slashes are a separate
    // policy and are not rejected by this standalone-token rule.
    if cleaned == "/" {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(cleaned, word.span, cleaned),
                "Standalone slash is not a spoken word",
            )
            .with_suggestion(
                "Use [/] for repetition; a bare slash does not encode a CHAT annotation.",
            ),
        );
    }

    // Unicode punctuation is not the CHAT trailing-off terminator `+...`.
    // Keep the parsed Word and its source span intact: validation is not repair.
    if cleaned.contains('\u{2026}') {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(cleaned, word.span, cleaned),
                "Word contains Unicode ellipsis (U+2026)",
            )
            .with_suggestion(
                "Use the CHAT terminator +... for trailing off, not a Unicode ellipsis in word text.",
            ),
        );
    }

    // Check for other control characters (excluding those that are part of CHAT syntax)
    for (idx, ch) in cleaned.char_indices() {
        if ch.is_control() && ch != '\u{0015}' {
            // Already checked bullet separately
            errors.report(
                ParseError::new(
                    ErrorCode::IllegalCharactersInWord,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(cleaned, word.span, cleaned),
                    format!("Word contains illegal control character U+{:04X} at position {}", ch as u32, idx),
                )
                .with_suggestion(
                    "Words must contain only printable characters (Unicode alphabetic, numbers, and CHAT-allowed symbols).",
                ),
            );
        }
    }

    // Lexical scalar policy, independent of CHECK's encoded-byte predicate.
    // Control characters retain their separate diagnostics above.
    for (idx, ch) in cleaned.char_indices() {
        if let Some(rejected) = RejectedLexicalScalar::admit(ch) {
            let (character, category) = rejected.into_parts();
            errors.report(
                ParseError::new(
                    ErrorCode::IllegalCharactersInWord,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(cleaned, word.span, cleaned),
                    format!(
                        "Word contains a Unicode {category} U+{:04X} at position {idx}",
                        character as u32,
                    ),
                )
                .with_suggestion(
                    "Use the intended standard Unicode transcription character; do not silently delete or normalize this scalar.",
                ),
            );
        }
    }
}

/// A scalar admitted as forbidden lexical content, retaining its category.
/// Rust `char` already excludes surrogates and out-of-range integers.
/// Unicode defines these stable ranges; CHAT chooses to reject them in words.
/// https://www.unicode.org/faq/private_use.html
enum RejectedLexicalScalar {
    PrivateUse(char),
    Noncharacter(char),
}

impl RejectedLexicalScalar {
    fn admit(character: char) -> Option<Self> {
        let cp = character as u32;
        if matches!(cp, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD) {
            Some(Self::PrivateUse(character))
        } else if matches!(cp, 0xFDD0..=0xFDEF) || cp & 0xFFFF >= 0xFFFE {
            Some(Self::Noncharacter(character))
        } else {
            None
        }
    }

    fn into_parts(self) -> (char, &'static str) {
        match self {
            Self::PrivateUse(character) => (character, "private-use character"),
            Self::Noncharacter(character) => (character, "noncharacter"),
        }
    }
}

/// Progress through lexical compound parts, not raw character positions.
/// A join resets spoken-material evidence; prosodic markers cannot fill a part.
enum CompoundPartState {
    InitialEmpty,
    InitialSpoken,
    JoinedEmpty,
    JoinedSpoken,
}

impl CompoundPartState {
    fn with_spoken(self) -> Self {
        match self {
            Self::InitialEmpty | Self::InitialSpoken => Self::InitialSpoken,
            Self::JoinedEmpty | Self::JoinedSpoken => Self::JoinedSpoken,
        }
    }
}

/// Validate `+` compound marker placement within a token.
///
/// Compound markers must separate non-empty lexical segments, so leading,
/// trailing, or doubled markers are all rejected.
pub(crate) fn check_compound_markers(word: &Word, errors: &impl ErrorSink) {
    let mut state = CompoundPartState::InitialEmpty;
    for item in word.content().iter() {
        if matches!(item, WordContent::CompoundMarker(_)) {
            match state {
                CompoundPartState::InitialEmpty => errors.report(
                    ParseError::new(
                        ErrorCode::InvalidCompoundMarkerPosition,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                        "Compound marker '+' has no preceding spoken part",
                    )
                    .with_suggestion("Add spoken material before '+' or remove the marker"),
                ),
                CompoundPartState::JoinedEmpty => errors.report(
                    ParseError::new(
                        ErrorCode::EmptyCompoundPart,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                        "Compound markers '+' enclose an empty spoken part",
                    )
                    .with_suggestion("Remove one '+' or add content between compound markers"),
                ),
                CompoundPartState::InitialSpoken | CompoundPartState::JoinedSpoken => {}
            }
            state = CompoundPartState::JoinedEmpty;
        } else if is_spoken_material(item) {
            state = state.with_spoken();
        }
    }

    if matches!(state, CompoundPartState::JoinedEmpty) {
        errors.report(
            ParseError::new(
                ErrorCode::EmptyCompoundPart,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                "Compound marker '+' cannot have an empty trailing part",
            )
            .with_suggestion("Add content after '+' or remove the trailing marker"),
        );
    }
}

/// The marker a word was meant to spell, when its text is one written wrongly.
///
/// `None` means E241 has nothing to say about this word: either the spelling is
/// canonical, or the word is not a marker at all, or it is not the kind of word
/// whose letters are orthography in the first place.
///
/// # This takes the WORD, and that is the fix rather than a style choice
///
/// It used to take a `&str`, and the caller passed `cleaned_text()`. Cleaning a
/// word strips its category prefix, so `&+xx` arrived here as `xx`,
/// indistinguishable from a bare mistyped marker, and E241 fired on it. That
/// was shipped behaviour: `chatter validate` on a file containing `&+xx`
/// reported `"xx" is not legal; did you mean to use "xxx"?` against a line
/// whose word is `&+xx`, which is the standing tell that a diagnostic is the
/// tool's defect and not the data's. A phonological fragment is sound rather
/// than spelling, and its letters mean nothing to a lexical rule.
///
/// WHICH categories are exempt is [`crate::model::WordCategory::material`]'s
/// answer, not this rule's. Three other places had each written out their own
/// version of that subset, and the reason a lexical rule must not judge a
/// fragment's letters is the same reason in all four, so it has one owner.
pub(crate) fn illegal_untranscribed_marker(word: &Word) -> Option<UntranscribedStatus> {
    match word.material() {
        // The letters approximate a noise. There is no spelling to be wrong.
        WordMaterial::Sound => None,
        // Orthography, spoken or not: `0xx` and `(xx)` are ordinary words that
        // were not uttered, and the letters are still a spelling.
        WordMaterial::Orthography => MarkerSpelling::of(word.cleaned_text()).misspelled(),
    }
}

/// Word content measured for prosodic placement checks.
///
/// Construction and checking take linear time with constant-time neighbor
/// queries, plus the cost of emitted diagnostics. The former
/// per-marker prefix/suffix scans were quadratic on marker-heavy words.
/// The immutable borrow prevents mutation between measurement and checking.
///
/// Rules:
/// - E244: Multiple consecutive stress markers are invalid (ˈˌtest)
/// - E245: Stress must be before spoken material, not at word end or before another marker
/// - E246: Lengthening (colon) must be after spoken material, not at word start
/// - E247: Only one primary stress per word allowed
/// - E250: Secondary stress requires primary stress in the same word
/// - E252: Syllable pause (^) must be between spoken material
pub(crate) struct ProsodicWord<'a> {
    word: &'a Word,
    spoken: SpokenExtent,
    primary_stress_count: usize,
    secondary_stress_count: usize,
}

/// The first and last spoken segments, measured from the borrowed word.
/// Absence is distinct from a segment at index zero.
enum SpokenExtent {
    Absent,
    Present { first: usize, last: usize },
}

impl SpokenExtent {
    fn precedes(&self, index: usize) -> bool {
        matches!(self, Self::Present { first, .. } if *first < index)
    }

    fn follows(&self, index: usize) -> bool {
        matches!(self, Self::Present { last, .. } if *last > index)
    }
}

impl<'a> ProsodicWord<'a> {
    /// Measure once; private fields tie every fact to this immutable word.
    /// Each marker can then ask about both neighbors in constant time.
    pub(crate) fn of(word: &'a Word) -> Self {
        let mut spoken = SpokenExtent::Absent;
        let mut primary_stress_count = 0;
        let mut secondary_stress_count = 0;
        for (index, item) in word.content().iter().enumerate() {
            if is_spoken_material(item) {
                match &mut spoken {
                    SpokenExtent::Absent => {
                        spoken = SpokenExtent::Present {
                            first: index,
                            last: index,
                        };
                    }
                    SpokenExtent::Present { last, .. } => *last = index,
                }
            }
            if let WordContent::StressMarker(marker) = item {
                match marker.marker_type {
                    WordStressMarkerType::Primary => primary_stress_count += 1,
                    WordStressMarkerType::Secondary => secondary_stress_count += 1,
                }
            }
        }
        Self {
            word,
            spoken,
            primary_stress_count,
            secondary_stress_count,
        }
    }

    /// Consume measured evidence and emit the existing prosodic diagnostics.
    pub(crate) fn check(self, errors: &impl ErrorSink) {
        let Self {
            word,
            spoken,
            primary_stress_count,
            secondary_stress_count,
        } = self;
        let content = word.content();

        // E247: Only one primary stress per word
        if primary_stress_count > 1 {
            errors.report(
                ParseError::new(
                    ErrorCode::MultiplePrimaryStress,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                    format!(
                        "Word has {} primary stress markers, but only one is allowed",
                        primary_stress_count
                    ),
                )
                .with_suggestion("A word can have at most one primary stress (ˈ)"),
            );
        }

        // E250: Secondary stress requires primary stress
        if secondary_stress_count > 0 && primary_stress_count == 0 {
            errors.report(
                ParseError::new(
                    ErrorCode::SecondaryStressWithoutPrimary,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                    "Word has secondary stress (ˌ) but no primary stress (ˈ)",
                )
                .with_suggestion(
                    "Secondary stress only makes sense when there is also a primary stress marker",
                ),
            );
        }

        // E244 carries a word-level span, not a pair-level span. Report once
        // for that immutable word, so longer runs cannot create duplicate fixes.
        if content.windows(2).any(|pair| {
            matches!(
                pair,
                [WordContent::StressMarker(_), WordContent::StressMarker(_)]
            )
        }) {
            errors.report(
                ParseError::new(
                    ErrorCode::ConsecutiveStressMarkers,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                    "Multiple consecutive stress markers",
                )
                .with_suggestion(
                    "A syllable can only have one stress marker (primary ˈ or secondary ˌ)",
                ),
            );
        }

        for (i, item) in content.iter().enumerate() {
            if matches!(item, WordContent::StressMarker(_)) {
                // E245: Stress must be followed by spoken material
                let has_following_text = spoken.follows(i);

                if !has_following_text {
                    errors.report(
                        ParseError::new(
                            ErrorCode::StressNotBeforeSpokenMaterial,
                            Severity::Error,
                            SourceLocation::new(word.span),
                            ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                            "Stress marker not followed by spoken material",
                        )
                        .with_suggestion(
                            "Stress markers (ˈ ˌ) must precede the syllable they mark",
                        ),
                    );
                }
            }

            // E246: Lengthening must be after spoken material
            if let WordContent::Lengthening(_) = item {
                let has_preceding_text = spoken.precedes(i);

                if !has_preceding_text {
                    errors.report(
                    ParseError::new(
                        ErrorCode::LengtheningNotAfterSpokenMaterial,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Lengthening marker (:) not after spoken material",
                    )
                    .with_suggestion(
                        "Lengthening marker (:) must follow the syllable it lengthens (e.g., bana:nas)",
                    ),
                );
                }
            }

            // E252: Syllable pause must be between spoken material
            if let WordContent::SyllablePause(_) = item {
                let has_preceding_text = spoken.precedes(i);
                let has_following_text = spoken.follows(i);

                if !has_preceding_text || !has_following_text {
                    errors.report(
                        ParseError::new(
                            ErrorCode::SyllablePauseNotBetweenSpokenMaterial,
                            Severity::Error,
                            SourceLocation::new(word.span),
                            ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                            "Syllable pause marker (^) must be between spoken material",
                        )
                        .with_suggestion(
                            "Syllable pause (^) must occur between syllables (e.g., rhi^noceros)",
                        ),
                    );
                }
            }
        }
    }
}

/// Return whether a word-content item contributes spoken lexical material.
///
/// Prosodic placement checks use this to distinguish markers from segment text.
fn is_spoken_material(content: &WordContent) -> bool {
    match content {
        WordContent::Text(text) => !text.as_ref().is_empty(),
        // A @u phonetic form IS spoken material: it is the phonetic
        // transcription of what was said.
        WordContent::Phonetic(form) => !form.as_ref().is_empty(),
        _ => false,
    }
}

/// Return whether the word contains at least one spoken lexical segment.
///
/// This is used by higher-level validators that need to gate marker checks on
/// actual spoken content presence.
pub(crate) fn has_spoken_material(word: &Word) -> bool {
    word.content().iter().any(is_spoken_material)
}
