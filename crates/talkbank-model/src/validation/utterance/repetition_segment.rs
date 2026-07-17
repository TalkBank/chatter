//! Validation of segment-repetition delimiters within a single word.
//!
//! `↫...↫` (U+21AB) marks a repeated segment OF a word (`↫p↫parents` =
//! "p-, parents"): the notation presumes a host word, so a word whose
//! entire spoken material sits inside the delimiters asserts a
//! repetition of nothing and is invalid (E753). Adopted from CLAN
//! CHECK error 151 by maintainer ruling (2026-07-15); only the GUI
//! CLAN build enforces the original, so this is a chatter-authority
//! rule, not a parity obligation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters>
//! - Spec: `spec/errors/E753_word_only_repetition_segments.md`

use crate::alignment::helpers::{WordItem, walk_words};
use crate::model::{CADelimiterType, Utterance, Word, WordContent};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// How a word's material relates to its segment-repetition spans.
///
/// Only words that carry at least one segment-repetition delimiter get a
/// shape; plain words are out of scope for this rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RepetitionShape {
    /// Some spoken material (text, phonetic content, or a shortening)
    /// lies outside every repetition span: the repeated segment has a
    /// host word, the valid and overwhelmingly common case.
    HasStem,
    /// Every material part sits inside repetition spans: the word is
    /// nothing but a repetition segment, the invalid case (E753).
    OnlyRepetition,
}

/// Classify one word's content parts against its repetition spans.
///
/// Walks the typed content in order, toggling an inside-span flag on each
/// `SegmentRepetition` delimiter. Material parts (`Text`, `Phonetic`,
/// `Shortening`) outside a span constitute a stem; prosody modifiers
/// (CA elements, stress, lengthening, syllable pauses, underlines,
/// compound markers, overlap points) do not, because they modify material
/// rather than being material. Returns `None` for words with no
/// repetition delimiter at all.
fn classify_repetition_shape(word: &Word) -> Option<RepetitionShape> {
    let mut inside_span = false;
    let mut saw_delimiter = false;
    let mut stem_outside = false;
    for part in word.content.iter() {
        match part {
            WordContent::CADelimiter(delimiter)
                if delimiter.delimiter_type == CADelimiterType::SegmentRepetition =>
            {
                inside_span = !inside_span;
                saw_delimiter = true;
            }
            WordContent::Text(_) | WordContent::Phonetic(_) | WordContent::Shortening(_)
                if !inside_span =>
            {
                stem_outside = true;
            }
            // Prosody/structure modifiers are not word material; an
            // unbalanced-delimiter situation is E-coded separately by the
            // CA-delimiter balance check, so no special handling here.
            _ => {}
        }
    }
    if !saw_delimiter {
        return None;
    }
    Some(if stem_outside {
        RepetitionShape::HasStem
    } else {
        RepetitionShape::OnlyRepetition
    })
}

/// E753: reject words consisting only of repetition segments.
///
/// A word-category prefix marker (`0` omission, `&-` filler, `&~`
/// nonword, ...) is material outside the arrows, so categorized words are
/// exempt: this matches GUI CLAN CHECK's character-level scan (any
/// character outside the delimiters counts as a stem) and keeps the wild
/// corpus's `&-↫w-w-w↫`-style filler tokens valid, per the 2026-07-15
/// ruling.
pub(crate) fn check_repetition_segment_has_stem(utterance: &Utterance, errors: &impl ErrorSink) {
    walk_words(&utterance.main.content.content, None, &mut |item| {
        let word = match item {
            WordItem::Word(word) => word,
            WordItem::ReplacedWord(replaced) => &replaced.word,
            WordItem::Separator(_) => return,
        };
        if word.category.is_some() {
            // The category prefix marker is material outside the span.
            return;
        }
        if classify_repetition_shape(word) == Some(RepetitionShape::OnlyRepetition) {
            let word_text = word.raw_text().to_string();
            errors.report(
                ParseError::new(
                    ErrorCode::WordOnlyRepetitionSegments,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(word_text.clone(), word.span, &word_text),
                    "word consists only of a repetition segment (all material inside ↫...↫); the notation marks a repeated segment OF a word and needs a stem outside the delimiters",
                )
                .with_suggestion(
                    "attach the repeated segment to its host word (e.g. `↫p↫parents`), or transcribe the fragment as a filler/nonword form if no host word was spoken",
                ),
            );
        }
    });
}
