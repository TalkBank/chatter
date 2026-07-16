//! Validation of special-form marker usage on main-tier words.
//!
//! Currently one rule lives here: the `@l` letter form marks a single
//! spoken letter (`b@l`), so a stem of more than one character is a
//! mis-marked form; sequences belong under `@k` (letter sequence) or
//! `@ls` (letter plural). Replicates CLAN CHECK error 76
//! (`check_isOneLetter`) per maintainer ruling 2026-07-14; the deeper
//! "is a digraph one letter" question is deliberately NOT decided here
//! (logged for the corpus authority).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Special_Form_Markers>
//! - Spec: `spec/errors/E754_letter_form_multiple_letters.md`

use crate::alignment::helpers::{WordItem, walk_words};
use crate::model::{CADelimiterType, FormType, Utterance, Word, WordContent};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Count the stem characters of a word: the Unicode scalars of its plain
/// text parts OUTSIDE any segment-repetition span. Prosody modifiers,
/// delimiters, the form marker itself, and repeated-segment material
/// (`↫b^↫` in the stuttered letter `↫b^↫b@l`) are not stem material:
/// real CLAN CHECK likewise accepts the stuttered form (verified in
/// file mode), and the wild fluency data relies on it. Counts
/// characters, not bytes, matching CHECK's UTF-8-aware scan.
fn stem_char_count(word: &Word) -> usize {
    let mut inside_repetition = false;
    let mut count = 0usize;
    for part in word.content.iter() {
        match part {
            WordContent::CADelimiter(delimiter)
                if delimiter.delimiter_type == CADelimiterType::SegmentRepetition =>
            {
                inside_repetition = !inside_repetition;
            }
            WordContent::Text(text) if !inside_repetition => {
                count += text.chars().count();
            }
            _ => {}
        }
    }
    count
}

/// E754: the `@l` letter form must carry exactly one letter of stem.
///
/// Scope is main-tier words (matching CLAN CHECK 76): multi-letter `@l`
/// inside dependent tiers or `[= ...]` gloss payloads is outside this
/// walk, which the wild data relies on (the corpus's ~97 multi-letter
/// `@l` tokens all live in those contexts).
pub(crate) fn check_letter_form_single_letter(utterance: &Utterance, errors: &impl ErrorSink) {
    walk_words(&utterance.main.content.content, None, &mut |item| {
        let word = match item {
            WordItem::Word(word) => word,
            WordItem::ReplacedWord(replaced) => &replaced.word,
            WordItem::Separator(_) => return,
        };
        if word.form_type != Some(FormType::L) {
            return;
        }
        if stem_char_count(word) > 1 {
            let word_text = word.raw_text().to_string();
            errors.report(
                ParseError::new(
                    ErrorCode::LetterFormMultipleLetters,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(word_text.clone(), word.span, &word_text),
                    "the @l letter form marks a single letter; this word has more than one character of stem",
                )
                .with_suggestion(
                    "use @k (letter sequence) or @ls (letter plural) for multi-letter content, or split into one @l word per letter",
                ),
            );
        }
    });
}
