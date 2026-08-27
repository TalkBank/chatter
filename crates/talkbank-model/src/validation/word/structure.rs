//! Structural/prosodic validators for main-tier word tokens.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#WordInternalPause_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Part_of_Speech>

use std::ops::RangeInclusive;

use crate::model::content::word::{MarkerSpelling, UntranscribedStatus};
use crate::model::{FormType, Word, WordContent, WordMaterial, WordStressMarkerType};
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

    // Reject Private-Use-Area and other non-standard high-BMP code points (CLAN
    // CHECK error 86); the rejected range is named once in
    // `is_nonstandard_unicode_word_char`.
    for (idx, ch) in cleaned.char_indices() {
        let cp = ch as u32;
        if is_nonstandard_unicode_word_char(cp) {
            errors.report(
                ParseError::new(
                    ErrorCode::IllegalCharactersInWord,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(cleaned, word.span, cleaned),
                    format!(
                        "Word contains a non-standard Unicode character U+{cp:04X} at position {idx} (private-use or compatibility area)"
                    ),
                )
                .with_suggestion(
                    "Replace private-use and compatibility-area characters with their standard Unicode equivalents; CHAT requires standard Unicode.",
                ),
            );
        }
    }
}

/// Whether `cp` is a non-standard high-BMP word code point that CLAN CHECK
/// rejects (error 86, `isIllegalASCII` in OSX-CLAN `check.cpp:2880`). CLAN flags
/// the 3-byte UTF-8 block U+E000..=U+FFFF as non-standard EXCEPT two ranges it
/// whitelists for internal use: U+F170..=U+F264 (CLAN-internal markup) and the
/// fullwidth ASCII forms U+FF01..=U+FF5E. (U+00B7, the allowed middle dot, sits
/// below this block.) CHAT requires standard Unicode, so private-use and
/// compatibility-area code points are invalid in word text; this mirrors CLAN's
/// range exactly for CHECK parity.
fn is_nonstandard_unicode_word_char(cp: u32) -> bool {
    // The 3-byte high-BMP block CLAN treats as non-standard.
    const NONSTANDARD_BLOCK: RangeInclusive<u32> = 0xE000..=0xFFFF;
    // CLAN-internal markup, whitelisted inside the block.
    const CLAN_INTERNAL_MARKUP: RangeInclusive<u32> = 0xF170..=0xF264;
    // Fullwidth ASCII forms, whitelisted inside the block.
    const FULLWIDTH_ASCII: RangeInclusive<u32> = 0xFF01..=0xFF5E;
    NONSTANDARD_BLOCK.contains(&cp)
        && !CLAN_INTERNAL_MARKUP.contains(&cp)
        && !FULLWIDTH_ASCII.contains(&cp)
}

/// Validate that shortening markers use properly nested parentheses.
///
/// Uses stack-based validation to ensure proper pairing, not just counting.
pub(crate) fn check_shortening_balance(word: &Word, errors: &impl ErrorSink) {
    let mut depth = 0i32;

    // Use raw_text to preserve parser-recovered boundary information.
    for ch in word.raw_text.chars() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth < 0 {
                errors.report(
                    ParseError::new(
                        ErrorCode::UnbalancedShortening,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Closing parenthesis ')' without corresponding opening '('",
                    )
                    .with_suggestion(
                        "Ensure each closing ')' has a matching opening '(' before it",
                    ),
                );
                // Reset depth to prevent cascading errors
                depth = 0;
            }
        }
    }

    // Check for unclosed parentheses
    if depth > 0 {
        errors.report(
            ParseError::new(
                ErrorCode::UnbalancedShortening,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                format!(
                    "Unbalanced shortening markers: {} unclosed opening '('",
                    depth
                ),
            )
            .with_suggestion("Ensure each opening '(' has a matching closing ')'"),
        );
    }
}

/// Validate `+` compound marker placement within a token.
///
/// Compound markers must separate non-empty lexical segments, so leading,
/// trailing, or doubled markers are all rejected.
pub(crate) fn check_compound_markers(word: &Word, errors: &impl ErrorSink) {
    if matches!(word.content.first(), Some(WordContent::CompoundMarker(_))) {
        errors.report(
            ParseError::new(
                ErrorCode::InvalidCompoundMarkerPosition,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                "Compound marker '+' cannot start a word",
            )
            .with_suggestion("Remove the leading '+' or attach it to the previous word"),
        );
    }

    if matches!(word.content.last(), Some(WordContent::CompoundMarker(_))) {
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

    if word.content.windows(2).any(|window| {
        matches!(
            window,
            [
                WordContent::CompoundMarker(_),
                WordContent::CompoundMarker(_)
            ]
        )
    }) {
        errors.report(
            ParseError::new(
                ErrorCode::EmptyCompoundPart,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                "Compound marker '+' cannot have empty parts (++)",
            )
            .with_suggestion("Remove one '+' or add content between compound markers"),
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

/// Return whether a stress marker is primary (`ˈ`).
///
/// Small helper to keep pattern checks readable in prosodic validation.
fn is_primary_stress(marker_type: WordStressMarkerType) -> bool {
    matches!(marker_type, WordStressMarkerType::Primary)
}

/// Return whether a stress marker is secondary (`ˌ`).
///
/// Small helper to keep pattern checks readable in prosodic validation.
fn is_secondary_stress(marker_type: WordStressMarkerType) -> bool {
    matches!(marker_type, WordStressMarkerType::Secondary)
}

/// Validate prosodic marker placement in word content.
///
/// Rules:
/// - E244: Multiple consecutive stress markers are invalid (ˈˌtest)
/// - E245: Stress must be before spoken material, not at word end or before another marker
/// - E246: Lengthening (colon) must be after spoken material, not at word start
/// - E247: Only one primary stress per word allowed
/// - E250: Secondary stress requires primary stress in the same word
/// - E252: Syllable pause (^) must be between spoken material
pub(crate) fn check_prosodic_markers(word: &Word, errors: &impl ErrorSink) {
    let content = &word.content;

    // Count stress markers for E247 and E250
    let mut primary_stress_count = 0;
    let mut secondary_stress_count = 0;

    for item in content.iter() {
        if let WordContent::StressMarker(marker) = item {
            if is_primary_stress(marker.marker_type) {
                primary_stress_count += 1;
            } else if is_secondary_stress(marker.marker_type) {
                secondary_stress_count += 1;
            }
        }
    }

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

    for (i, item) in content.iter().enumerate() {
        // E244: Check for consecutive stress markers
        if matches!(item, WordContent::StressMarker(_)) {
            if matches!(content.get(i + 1), Some(WordContent::StressMarker(_))) {
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

            // E245: Stress must be followed by spoken material
            let has_following_text = content[i + 1..].iter().any(is_spoken_material);

            if !has_following_text {
                errors.report(
                    ParseError::new(
                        ErrorCode::StressNotBeforeSpokenMaterial,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Stress marker not followed by spoken material",
                    )
                    .with_suggestion("Stress markers (ˈ ˌ) must precede the syllable they mark"),
                );
            }
        }

        // E246: Lengthening must be after spoken material
        if let WordContent::Lengthening(_) = item {
            let has_preceding_text = content[..i].iter().any(is_spoken_material);

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
            let has_preceding_text = content[..i].iter().any(is_spoken_material);
            let has_following_text = content[i + 1..].iter().any(is_spoken_material);

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
    word.content.iter().any(is_spoken_material)
}

/// Validate inline `@...` marker integrity from raw text.
///
/// # Both branches were unreachable until 2026-08-27
///
/// The comment below used to say this "catches parser-recovery cases where
/// malformed marker suffixes are split into standalone ERROR nodes". It did
/// not: a word whose `@` suffix was malformed never FORMED, so neither branch
/// could see one. Measured before the grammar change, `hello@` reported E202
/// from tree-sitter classifying an ERROR node's TEXT, and `hello@@c` reported
/// a generic E316; this function's E202 and E203 branches fired on neither.
///
/// The grammar admits the repeated shape now (`repeated_form_marker`), so such
/// a word forms and the `at_count` branch is live. There is NO
/// `dangling_form_marker`: this comment named one until 2026-08-27 and the
/// grammar never had it, because `@` is the header sigil and admitting a single
/// one in word position moved the diagnostic for a doubled `@End`. `hello@`
/// still parses as a word body plus a sibling ERROR, so the E202 branch below
/// remains UNREACHABLE and is not evidence of anything.
///
/// The dangling branch reported E243 "illegal characters in word", which is
/// what an unreachable rule's code drifts to. `spec/errors/E202_missing_form_type.md`
/// carries `*CHI:\thello@ .` and E202's own summary is "Missing form type on
/// special word", so E202 is the code and it is used here now.
///
/// STILL OWED: this reads `word.raw_text` and counts `@` bytes, which is the
/// text-scanning this project bans. The grammar knows the answer structurally,
/// in the node kind; the model cannot see it because `Word` has no slot for
/// "the form marker was malformed, and how". That slot is the next change.
pub(crate) fn check_inline_at_markers(word: &Word, errors: &impl ErrorSink) {
    let at_count = word
        .raw_text
        .as_bytes()
        .iter()
        .filter(|&&b| b == b'@')
        .count();
    if at_count == 0 {
        return;
    }

    // A word whose marker the PARSER already refused is not reported twice,
    // and that covers BOTH branches below.
    //
    // The suppression existed for the `at_count > 1` branch only, so `word@@`
    // and `word@c@` reported the parser's specific E203 ("a word may carry
    // only one '@' suffix, found '@@'") AND this function's generic E202
    // ("dangling '@' marker"), for one defect: the specific diagnostic buried
    // under the generic one, which is the shape the re2c doubling was fixed
    // for hours earlier on the other side of the same release.
    //
    // `FormType::Undeclared` means the form-marker text names no declared
    // code, which the parser says at parse time with the text in hand. A bare
    // trailing `@` (`hello@`) carries no form type at all, so it is not
    // suppressed and keeps its E202, which is the case this branch exists for.
    if matches!(word.form_type, Some(FormType::Undeclared(_))) {
        return;
    }

    if word.raw_text.ends_with('@') {
        errors.report(
            ParseError::new(
                ErrorCode::MissingFormType,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                "Dangling '@' marker in word",
            )
            .with_suggestion("Remove '@' or provide a valid marker suffix"),
        );
    }

    // A word whose marker the PARSER already refused is not reported twice.
    //
    // `FormType::Undeclared` means the form-marker text names no declared code,
    // and the parser says so at parse time with the text in hand:
    // `Undeclared form marker '@@c'`. This branch would then add
    // `Malformed form marker suffix` for the same word, because a repeated `@`
    // is BOTH. Two E203s for one defect, the specific one buried under the
    // generic one, which is the shape the re2c doubling was fixed for hours
    // earlier on the other side.
    //
    // WHAT THIS BRANCH IS FOR, now that the ruling is made: the re2c backend.
    // Its lexer has no repeated-suffix rule, so `word@k@s:spa` arrives there as
    // a form marker plus a language suffix and two `@` in `raw_text`, and this
    // count is the only thing that refuses it. On tree-sitter the grammar
    // swallows the whole run into one node the parser names directly, so the
    // suppression below is what stops the same defect being reported twice.
    //
    // This paragraph said the opposite for a few hours on 2026-08-27, that
    // `word@k@s:spa` carries `FormType::K`, reaches the report, and that
    // whether it should was a "live adjudication". It was ruled that day (a
    // word may carry at most ONE `@` suffix, `spec/errors/E203.md`), and under
    // that ruling's grammar such a word carries `FormType::Undeclared` and is
    // suppressed here. Corrected in place, because a reader deciding whether
    // this branch may be deleted looks exactly here.
    if at_count > 1 {
        errors.report(
            ParseError::new(
                ErrorCode::InvalidFormType,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                "Malformed form marker suffix",
            )
            // Deliberately says nothing about WHICH markers exist: this is a
            // shape complaint (more than one `@`), and an inventory example
            // here would be a copy that goes stale.
            .with_suggestion("Use exactly one form marker"),
        );
        return;
    }

    if let Some(form_type) = &word.form_type {
        let marker = format!("@{}", form_type.to_chat_marker());
        if let Some(marker_pos) = word.raw_text.rfind(&marker) {
            let trailing = &word.raw_text[marker_pos + marker.len()..];
            if !trailing.is_empty() && !trailing.starts_with('@') && !trailing.starts_with('$') {
                errors.report(
                    ParseError::new(
                        ErrorCode::InvalidFormType,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Invalid characters after form marker",
                    )
                    // Shape, not inventory: see the suggestion above.
                    .with_suggestion("Use a marker suffix only, with nothing after it"),
                );
            }
        }
    }

    if word.form_type.is_none() && word.lang.is_none() && !word.raw_text.ends_with('@') {
        errors.report(
            ParseError::new(
                ErrorCode::InvalidFormType,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                "Unknown '@' marker suffix",
            )
            // The inventory is the registry's, not this function's: this site
            // used to name three markers by hand, so retiring one left it
            // advertised in a user-facing string. `@s:` is the language suffix,
            // a separate construct, and is named separately.
            .with_suggestion(crate::model::FormType::DECLARED_MARKERS_SUGGESTION),
        );
    }
}
