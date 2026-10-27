//! Convert parsed word bodies and word metadata into model words.

use crate::ast;
use crate::ast::WordBodyItem;
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
        WordBodyItem::Text(text) => WordContent::Text(text.clone()),
        WordBodyItem::Shortening(text) => WordContent::Shortening(text.clone()),
        WordBodyItem::Lengthening(run) => WordContent::Lengthening(WordLengthening::with_count(
            super::locate(source, run.text()),
            run.count(),
        )),
        WordBodyItem::CompoundMarker(s) => {
            WordContent::CompoundMarker(WordCompoundMarker::new(super::locate(source, s)))
        }
        WordBodyItem::Stress(kind, s) => {
            WordContent::StressMarker(WordStressMarker::new(super::locate(source, s), *kind))
        }
        WordBodyItem::SyllablePause(s) => {
            WordContent::SyllablePause(WordSyllablePause::new(super::locate(source, s)))
        }
        WordBodyItem::CliticBoundary(s) => WordContent::CliticBoundary(
            talkbank_model::model::WordCliticBoundary::new(super::locate(source, s)),
        ),
        WordBodyItem::OverlapPoint(kind, s) => {
            let index = s
                .chars()
                .nth(1)
                .and_then(|c| c.to_digit(10))
                .map(OverlapIndex::new);
            WordContent::OverlapPoint(OverlapPoint::new(super::locate(source, s), *kind, index))
        }
        WordBodyItem::CaElement(kind, s) => {
            WordContent::CAElement(CAElement::new(super::locate(source, s), *kind))
        }
        WordBodyItem::UnderlineBegin(s) => WordContent::UnderlineBegin(
            talkbank_model::model::UnderlineMarker::new(super::locate(source, s)),
        ),
        WordBodyItem::UnderlineEnd(s) => WordContent::UnderlineEnd(
            talkbank_model::model::UnderlineMarker::new(super::locate(source, s)),
        ),
        WordBodyItem::CaDelimiter(kind, s) => {
            WordContent::CADelimiter(CADelimiter::new(super::locate(source, s), *kind))
        }
    }
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

    let phonetic = talkbank_model::ChatString::render(|sink| {
        for item in &content_items {
            item.write_chat(sink)?;
        }
        Ok(())
    });
    match talkbank_model::WordPhonetic::new(&phonetic) {
        Ok(form) => vec![WordContent::Phonetic(form)],
        Err(_) => content_items,
    }
}

// ═══════════════════════════════════════════════════════════════
// WordWithAnnotations → Word
// ═══════════════════════════════════════════════════════════════

/// Convert a parsed word using the source admitted with its spelling.
///
/// Conversion accepts no replacement source:
///
/// ```compile_fail
/// use talkbank_parser_re2c::{ast::WordWithAnnotations, convert::word_from_parsed};
/// use talkbank_parser_re2c::source_text::SourceText;
/// fn replace_source(word: &WordWithAnnotations<'_>, foreign: &str) {
///     word_from_parsed(word, SourceText::new(foreign));
/// }
/// ```
pub fn word_from_parsed(w: &ast::WordWithAnnotations<'_>) -> Word {
    let source = w.source().source();
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

    let mut word = Word::from_content(
        talkbank_model::Location::Located(w.source().span()),
        WordContents::new(content_items.into_iter().collect()),
    );

    // Both grammar paths retain the shared semantic category directly.
    word.category = w.category.clone();

    // Admission already established the registry value. Invalid payloads must
    // not disappear while cleaned word text loses the original @ spelling.
    if let Some(declared) = &w.form_marker {
        word = word.with_form_type(declared.clone());
    }

    // Both grammar paths retain the admitted shared value; lowering cannot
    // reinterpret unchecked language strings or panic on an empty segment.
    word.lang = w.lang.clone();

    // POS tag, tag-extracted content
    if let Some(tag) = w.pos_tag {
        word = word.with_part_of_speech(tag);
    }

    word
}
