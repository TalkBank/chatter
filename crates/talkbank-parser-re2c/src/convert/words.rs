//! Convert parsed word bodies and word metadata into model words.
#![allow(clippy::expect_used)]

use crate::ast;
use crate::ast::{CaDelimiterKind, CaElementKind, OverlapKind, StressKind, WordBodyItem};
use crate::source_text::SourceText;
use talkbank_model::model::*;

/// Convert a typed word body item to a model WordContent, placing each
/// marker at its lexeme in `source` (unlocated when the lexeme is not a
/// slice of this source).
pub(crate) fn body_item_to_word_content(
    item: &WordBodyItem<'_>,
    source: SourceText<'_>,
) -> WordContent {
    match item {
        WordBodyItem::Text(s) => WordContent::Text(WordText::new_unchecked(s)),
        WordBodyItem::Shortening(s) => WordContent::Shortening(WordShortening::new_unchecked(s)),
        WordBodyItem::Lengthening(run) => WordContent::Lengthening(WordLengthening::with_count(
            super::locate(source, run.text()),
            run.count(),
        )),
        WordBodyItem::CompoundMarker(s) => {
            WordContent::CompoundMarker(WordCompoundMarker::new(super::locate(source, s)))
        }
        WordBodyItem::Stress(StressKind::Primary, s) => WordContent::StressMarker(
            WordStressMarker::new(super::locate(source, s), WordStressMarkerType::Primary),
        ),
        WordBodyItem::Stress(StressKind::Secondary, s) => WordContent::StressMarker(
            WordStressMarker::new(super::locate(source, s), WordStressMarkerType::Secondary),
        ),
        WordBodyItem::SyllablePause(s) => {
            WordContent::SyllablePause(WordSyllablePause::new(super::locate(source, s)))
        }
        WordBodyItem::CliticBoundary(s) => WordContent::CliticBoundary(
            talkbank_model::model::WordCliticBoundary::new(super::locate(source, s)),
        ),
        WordBodyItem::OverlapPoint(kind, s) => {
            let model_kind = match kind {
                OverlapKind::TopBegin => OverlapPointKind::TopOverlapBegin,
                OverlapKind::TopEnd => OverlapPointKind::TopOverlapEnd,
                OverlapKind::BottomBegin => OverlapPointKind::BottomOverlapBegin,
                OverlapKind::BottomEnd => OverlapPointKind::BottomOverlapEnd,
            };
            let index = s
                .chars()
                .nth(1)
                .and_then(|c| c.to_digit(10))
                .map(OverlapIndex::new);
            WordContent::OverlapPoint(OverlapPoint::new(
                super::locate(source, s),
                model_kind,
                index,
            ))
        }
        WordBodyItem::CaElement(kind, s) => {
            let t = match kind {
                CaElementKind::BlockedSegments => CAElementType::BlockedSegments,
                CaElementKind::Constriction => CAElementType::Constriction,
                CaElementKind::Hardening => CAElementType::Hardening,
                CaElementKind::HurriedStart => CAElementType::HurriedStart,
                CaElementKind::Inhalation => CAElementType::Inhalation,
                CaElementKind::LaughInWord => CAElementType::LaughInWord,
                CaElementKind::PitchDown => CAElementType::PitchDown,
                CaElementKind::PitchReset => CAElementType::PitchReset,
                CaElementKind::PitchUp => CAElementType::PitchUp,
                CaElementKind::SuddenStop => CAElementType::SuddenStop,
            };
            WordContent::CAElement(CAElement::new(super::locate(source, s), t))
        }
        WordBodyItem::UnderlineBegin(s) => WordContent::UnderlineBegin(
            talkbank_model::model::UnderlineMarker::new(super::locate(source, s)),
        ),
        WordBodyItem::UnderlineEnd(s) => WordContent::UnderlineEnd(
            talkbank_model::model::UnderlineMarker::new(super::locate(source, s)),
        ),
        WordBodyItem::CaDelimiter(kind, s) => {
            let t = match kind {
                CaDelimiterKind::Unsure => CADelimiterType::Unsure,
                CaDelimiterKind::Precise => CADelimiterType::Precise,
                CaDelimiterKind::Creaky => CADelimiterType::Creaky,
                CaDelimiterKind::Softer => CADelimiterType::Softer,
                CaDelimiterKind::SegmentRepetition => CADelimiterType::SegmentRepetition,
                CaDelimiterKind::Faster => CADelimiterType::Faster,
                CaDelimiterKind::Slower => CADelimiterType::Slower,
                CaDelimiterKind::Whisper => CADelimiterType::Whisper,
                CaDelimiterKind::Singing => CADelimiterType::Singing,
                CaDelimiterKind::LowPitch => CADelimiterType::LowPitch,
                CaDelimiterKind::HighPitch => CADelimiterType::HighPitch,
                CaDelimiterKind::Louder => CADelimiterType::Louder,
                CaDelimiterKind::SmileVoice => CADelimiterType::SmileVoice,
                CaDelimiterKind::BreathyVoice => CADelimiterType::BreathyVoice,
                CaDelimiterKind::Yawn => CADelimiterType::Yawn,
            };
            WordContent::CADelimiter(CADelimiter::new(super::locate(source, s), t))
        }
    }
}

/// Compute cleaned_text from word body items.
/// Only Text and Shortening contribute; all markers are stripped.
///
/// `↫ ... ↫` (CA segment repetition) brackets a stuttered repeated segment that
/// is not lexical: per the CHAT manual, `↫b-b-b↫boy` is the word "boy". Text
/// between a `↫` pair is dropped. This mirrors `Word::compute_cleaned_text` in
/// `talkbank-model`; keep the two in sync.
pub(crate) fn compute_cleaned_text(body: &[WordBodyItem<'_>]) -> String {
    let mut cleaned = String::new();
    let mut in_segment_repetition = false;
    for item in body {
        match item {
            WordBodyItem::CaDelimiter(CaDelimiterKind::SegmentRepetition, _) => {
                in_segment_repetition = !in_segment_repetition;
            }
            WordBodyItem::Text(s) if !in_segment_repetition => cleaned.push_str(s),
            WordBodyItem::Shortening(s) if !in_segment_repetition => cleaned.push_str(s),
            _ => {}
        }
    }
    cleaned
}

/// Fold a `@u` word's lexed content pieces into a single phonetic node.
///
/// Serializes each piece's CHAT surface form so the fold is lossless for
/// round-tripping; falls back to the original pieces if serialization
/// yields nothing so no information is ever dropped. Kept as an
/// independent implementation of the same rule as the tree-sitter
/// parser's `fold_phonetic` (the two parsers deliberately cross-check
/// each other).
fn fold_phonetic(content_items: Vec<WordContent>) -> Vec<WordContent> {
    use talkbank_model::model::WriteChat;

    let mut phonetic = String::new();
    for item in &content_items {
        if item.write_chat(&mut phonetic).is_err() {
            return content_items;
        }
    }
    match talkbank_model::WordPhonetic::new(&phonetic) {
        Ok(form) => vec![WordContent::Phonetic(form)],
        Err(_) => content_items,
    }
}

// ═══════════════════════════════════════════════════════════════
// WordWithAnnotations → Word
// ═══════════════════════════════════════════════════════════════

/// Where a parsed word sits in `source`: `raw_text` IS the word's slice of
/// the source on both parse paths, so `None` means only that the caller
/// paired this word with a source it did not come from. One owner: the model
/// word and the hulls that contain it both ask here.
pub(crate) fn word_span(
    w: &ast::WordWithAnnotations<'_>,
    source: SourceText<'_>,
) -> Option<talkbank_model::Span> {
    source.span_of(w.raw_text)
}

/// Convert a parsed word to the model Word type, placed at `word_span`.
pub fn word_from_parsed(w: &ast::WordWithAnnotations<'_>, source: SourceText<'_>) -> Word {
    let raw = w.raw_text;
    let cleaned = compute_cleaned_text(&w.body);

    let content_items: Vec<WordContent> = w
        .body
        .iter()
        .map(|item| body_item_to_word_content(item, source))
        .collect();

    // A `@u` word's content is a PHONETIC transcription (UNIBET/IPA), not
    // orthography: fold the lexed pieces into one opaque phonetic node,
    // mirroring the tree-sitter parser (option B of the 2026-07-13 UNIBET
    // design; scope is @u ONLY per the 2026-07-14 adjudication).
    // Read the marker ONCE. This used to be parsed here for the `@u` test and
    // again below for the assignment, discarding the first result, so every
    // form-marked word paid for two splits and two case folds, and every
    // `@z:label` word built a `FormType::UserDefined` purely to drop it.
    let is_u_form = matches!(w.form_marker, Some(FormType::U));
    let content_items = if is_u_form {
        fold_phonetic(content_items)
    } else {
        content_items
    };

    let cleaned_for_model = if cleaned.is_empty() { raw } else { &cleaned };
    let mut word = Word::new_unchecked(super::located(word_span(w, source)), cleaned_for_model)
        .with_content(WordContents::new(content_items.into_iter().collect()));

    // Category from typed enum, no token scanning
    if let Some(cat) = &w.category {
        word = word.with_category(match cat {
            crate::ast::WordCategory::Omission => WordCategory::Omission,
            crate::ast::WordCategory::Filler => WordCategory::Filler,
            crate::ast::WordCategory::Nonword => WordCategory::Nonword,
            crate::ast::WordCategory::Fragment => WordCategory::PhonologicalFragment,
        });
    }

    // Admission already established the registry value. Invalid payloads must
    // not disappear while cleaned word text loses the original @ spelling.
    if let Some(declared) = &w.form_marker {
        word = word.with_form_type(declared.clone());
    }

    // Language suffix, typed enum, no string hacking. Each split piece is
    // guaranteed non-empty by the lexer's `lang_suffix` regex
    // (`[a-z]{2,3}` per `+`/`&`-separated segment, see the token catalog
    // doc), so `.expect()` is defensive only.
    if let Some(ref lang) = w.lang {
        word = match lang {
            crate::ast::ParsedLangSuffix::Shortcut => word.with_language_shortcut(),
            crate::ast::ParsedLangSuffix::Explicit(codes) if codes.contains('+') => {
                let lc: Vec<LanguageCode> = codes
                    .split('+')
                    .map(|c| LanguageCode::new(c).expect("lexer-guaranteed non-empty segment"))
                    .collect();
                word.lang = Some(WordLanguageMarker::Multiple(lc));
                word
            }
            crate::ast::ParsedLangSuffix::Explicit(codes) if codes.contains('&') => {
                let lc: Vec<LanguageCode> = codes
                    .split('&')
                    .map(|c| LanguageCode::new(c).expect("lexer-guaranteed non-empty segment"))
                    .collect();
                word.lang = Some(WordLanguageMarker::Ambiguous(lc));
                word
            }
            crate::ast::ParsedLangSuffix::Explicit(code) => word
                .with_lang(LanguageCode::new(*code).expect("lexer-guaranteed non-empty segment")),
        };
    }

    // POS tag, tag-extracted content
    if let Some(tag) = w.pos_tag {
        word = word.with_part_of_speech(tag);
    }

    word
}
