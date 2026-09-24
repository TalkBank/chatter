//! Digit-policy validation for words under resolved language context.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>

use crate::model::Word;
use crate::model::content::word::WordCategory;
use crate::validation::context::contains_digits;
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

use super::helpers::mixed_language_allows_numbers;
use super::resolve::LanguageResolution;

/// A numeral and an embedded tone/homonym digit have different admission rules.
#[derive(Clone, Copy)]
enum DigitUse {
    Numeral,
    WordComponent,
}

impl DigitUse {
    fn classify(text: &str) -> Option<Self> {
        if !contains_digits(text) {
            None
        } else if text.bytes().all(|byte| byte.is_ascii_digit()) {
            Some(Self::Numeral)
        } else {
            Some(Self::WordComponent)
        }
    }
}

/// Validate whether digits are allowed for a word under resolved language context.
///
/// Current policy is permissive for mixed/ambiguous codes: if any candidate
/// language allows embedded digits, that use is accepted. Bare numerals must
/// be written out; tone-language admission cannot license them.
///
/// # Behavior
/// - Skips validation for Omission words (0 prefix is valid CHAT)
/// - If word contains digits, checks candidate languages via `resolution`
/// - Emits `E220` for numerals or when no candidate permits embedded digits
pub(crate) fn check_word_digits_multi(
    word: &Word,
    resolution: &LanguageResolution,
    errors: &impl ErrorSink,
) {
    // Skip validation for Omission words (0word pattern) - the 0 prefix is valid CHAT
    if word.category == Some(WordCategory::Omission) {
        return;
    }

    let Some(digit_use) = DigitUse::classify(word.cleaned_text()) else {
        return;
    };

    // UNKNOWN IS NOT DISALLOWED. An unresolved language means we do not know
    // which rules apply, and reporting "digits are not legal in language X"
    // without an X is a guess that reads as a finding. `E763` already states
    // this policy for the prefix marker; the two rules are meant to agree, and
    // this one did not.
    //
    // It was invisible while `Word::validate` gated the whole block on the FILE
    // declaring a language, because then an unresolved word never reached here.
    // Removing that gate (an explicit `<...> [@s:eng]` names a language with no
    // `@Languages` header, and nothing was checking it) exposed the difference
    // immediately: `test_e220_no_language_context` went red, which is the test
    // doing its job.
    if matches!(resolution, LanguageResolution::Unresolved) {
        return;
    }

    // For mixed/ambiguous language markers, allow digits if at least one candidate
    // language allows them. This matches permissive CHAT usage in reference data.
    let allows_digits = match digit_use {
        DigitUse::Numeral => false,
        DigitUse::WordComponent => resolution
            .languages()
            .iter()
            .any(|lang| mixed_language_allows_numbers(lang.as_str())),
    };

    if !allows_digits {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalDigits,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                format!(
                    "\"{}\" is not a legal word in language(s) \"{}\": {}",
                    word.cleaned_text(),
                    resolution.as_display_string(),
                    match digit_use {
                        DigitUse::Numeral => "bare numerals must be written out in words",
                        DigitUse::WordComponent => "numeric digits not allowed",
                    }
                ),
            )
            .with_suggestion(
                "Write numbers according to their pronunciation; use embedded tone or homonym digits only in language contexts that permit them.",
            ),
        );
    }
}
