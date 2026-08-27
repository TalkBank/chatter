//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use crate::ast;
use crate::ast::{CaDelimiterKind, CaElementKind, OverlapKind, StressKind, WordBodyItem};
use crate::source_text::SourceText;
use crate::token::Token;
use talkbank_model::Span;
use talkbank_model::annotation::AnnotatedContentAnnotations;
use talkbank_model::model::WordCompoundMarker;
use talkbank_model::model::*;

/// Convert a typed word body item to a model WordContent.
pub(crate) fn body_item_to_word_content(item: &WordBodyItem<'_>) -> WordContent {
    match item {
        WordBodyItem::Text(s) => WordContent::Text(WordText::new_unchecked(s)),
        WordBodyItem::Shortening(s) => WordContent::Shortening(WordShortening::new_unchecked(s)),
        WordBodyItem::Lengthening(count) => WordContent::Lengthening(WordLengthening {
            count: *count,
            span: None,
        }),
        WordBodyItem::CompoundMarker => WordContent::CompoundMarker(WordCompoundMarker::new()),
        WordBodyItem::Stress(StressKind::Primary) => {
            WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary))
        }
        WordBodyItem::Stress(StressKind::Secondary) => {
            WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Secondary))
        }
        WordBodyItem::SyllablePause => WordContent::SyllablePause(WordSyllablePause::new()),
        WordBodyItem::CliticBoundary => {
            WordContent::CliticBoundary(talkbank_model::model::WordCliticBoundary::new())
        }
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
            WordContent::OverlapPoint(OverlapPoint::new(model_kind, index))
        }
        WordBodyItem::CaElement(kind) => {
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
            WordContent::CAElement(CAElement::new(t))
        }
        WordBodyItem::UnderlineBegin => {
            WordContent::UnderlineBegin(talkbank_model::model::UnderlineMarker::new())
        }
        WordBodyItem::UnderlineEnd => {
            WordContent::UnderlineEnd(talkbank_model::model::UnderlineMarker::new())
        }
        WordBodyItem::CaDelimiter(kind) => {
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
            WordContent::CADelimiter(CADelimiter::new(t))
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
            WordBodyItem::CaDelimiter(CaDelimiterKind::SegmentRepetition) => {
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

/// Convert a parsed word to the model Word type.
/// `raw_text` is the word's slice of `source` on the rich-word path, so
/// `span_of` places it. A word built by `subtoken_word` reconstructs its
/// `raw_text` into a FRESH allocation, so it cannot be placed and keeps
/// `Span::DUMMY`; see the note at the span assignment below.
pub fn word_from_parsed(w: &ast::WordWithAnnotations<'_>, source: SourceText<'_>) -> Word {
    let raw = w.raw_text;
    let cleaned = compute_cleaned_text(&w.body);

    let content_items: Vec<WordContent> = w.body.iter().map(body_item_to_word_content).collect();

    // A `@u` word's content is a PHONETIC transcription (UNIBET/IPA), not
    // orthography: fold the lexed pieces into one opaque phonetic node,
    // mirroring the tree-sitter parser (option B of the 2026-07-13 UNIBET
    // design; scope is @u ONLY per the 2026-07-14 adjudication).
    // Read the marker ONCE. This used to be parsed here for the `@u` test and
    // again below for the assignment, discarding the first result, so every
    // form-marked word paid for two splits and two case folds, and every
    // `@z:label` word built a `FormType::UserDefined` purely to drop it.
    let declared = w
        .form_marker
        .map(|marker| FormType::from_payload(FormMarkerPayload::after_at(marker)));

    let is_u_form = matches!(declared, Some(Ok(FormType::U)));
    let content_items = if is_u_form {
        fold_phonetic(content_items)
    } else {
        content_items
    };

    let cleaned_for_model = if cleaned.is_empty() { raw } else { &cleaned };
    let mut word = Word::new_unchecked(raw, cleaned_for_model)
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

    // Form marker, tag-extracted content, direct to model.
    //
    // The lexer hands over the payload WITHOUT the `@`, while the tree-sitter
    // parser hands over its token WITH one. That is why the payload is a named
    // type: both sides used to pass a bare `&str` into one function that
    // accepted either shape, and each then re-derived the `@z:label` rule
    // itself, one testing for `"@z:"` and the other for `"z:"`.
    // Deliberately silent on the error path. An undeclared marker (`@zzz`, or
    // `@z` with no label) is left WITHOUT a form_type so the shared model
    // validation raises E203, matching CLAN CHECK 147 and the tree-sitter
    // parser. Setting one would mask it from that check.
    if let Some(Ok(declared)) = declared {
        word = word.with_form_type(declared);
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

    // On the rich-word path `raw_text` IS the word's slice of the source, so
    // its position is recoverable; nothing recovered it, and every re2c word
    // reached the model at `Span::DUMMY`.
    //
    // `None` has TWO causes and only one of them is a caller error. The caller
    // may have paired this word with a source it did not come from; or the
    // word came from `subtoken_word`, which rebuilds `raw_text` by leaking a
    // fresh concatenation of its tokens' display forms, so the string is a
    // different allocation and can never be placed. The second is a real gap:
    // such words keep `Span::DUMMY`, which silently disables span-keyed rules
    // and renders surviving diagnostics at byte 0 of the FILE. Fixing it needs
    // the lexer's own byte range, which `parser/mod.rs` discards; pointer
    // arithmetic cannot reach it. Leaving the span untouched is the honest
    // answer here, not a fabricated position.
    if let Some(span) = source.span_of(w.raw_text) {
        word = word.with_span(span);
    }

    word
}

// ═══════════════════════════════════════════════════════════════
// ContentItem → UtteranceContent
// ═══════════════════════════════════════════════════════════════

/// Convert a ContentItem to a model UtteranceContent.
/// Every content item type has a proper model representation.
/// Convert a linker token to a model Linker.
pub(crate) fn linker_token_to_model(tok: &Token<'_>) -> Option<Linker> {
    // Every model node this converter builds gets Span::DUMMY, because
    // `parser::tokenize` drops the lexer's spans (`lexer.map(|(tok, _span)|
    // tok)`) before the parser ever sees them. The lexer DOES produce them:
    // `Lexer::next` returns `(Token, LexerSpan)`. An earlier version of this
    // comment said the tokens "carry only the matched text slice", which is
    // false and which reached the user-facing CLI reference before it was
    // caught, where it made restoring positions look impossible rather than
    // merely unfinished.
    // Source-spacing rules (E758) are span-arithmetic gated on non-dummy
    // spans in the model path; the re2c oracle mirrors them via its own
    // token-stream scan, so a dummy span is correct here.
    let kind = match tok {
        Token::LinkerLazyOverlap(_) => LinkerKind::LazyOverlapPrecedes,
        Token::LinkerQuickUptake(_) => LinkerKind::OtherCompletion,
        Token::LinkerQuickUptakeOverlap(_) => LinkerKind::QuickUptakeOverlap,
        Token::LinkerQuotationFollows(_) => LinkerKind::QuotationFollows,
        Token::LinkerSelfCompletion(_) => LinkerKind::SelfCompletion,
        Token::CaNoBreakLinker(_) => LinkerKind::NoBreakTcuContinuation,
        Token::CaTechnicalBreakLinker(_) => LinkerKind::TcuContinuation,
        _ => return None,
    };
    Some(Linker::new(kind, Span::DUMMY))
}

pub fn content_item_to_model(
    item: &ast::ContentItem<'_>,
    source: SourceText<'_>,
) -> UtteranceContent {
    match item {
        ast::ContentItem::Word(w) => word_with_annotations_to_model(w, source),
        ast::ContentItem::Pause(kind) => UtteranceContent::Pause(Pause::new(pause_duration(kind))),
        ast::ContentItem::Event(event_text) => {
            let event_text = *event_text;
            UtteranceContent::Event(Event::new(event_text))
        }
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            let event_text = *event;
            let event_model = Event::new(event_text);
            let scoped = annotations_to_scoped(annotations);
            // The constructor's `Option` IS the bare-versus-annotated
            // question, so it replaces the `is_empty` check that used to ask it
            // separately.
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Event(event_model),
                Some(scoped) => {
                    UtteranceContent::AnnotatedEvent(Annotated::new(event_model, scoped))
                }
            }
        }
        ast::ContentItem::Separator { kind, text } => {
            UtteranceContent::Separator(separator_from_kind(*kind, source.span_of(text)))
        }
        ast::ContentItem::Freecode(text) => UtteranceContent::Freecode(Freecode::new(*text)),
        // An annotation with nothing to scope over is invalid CHAT, reported as
        // E759. It is preserved as a freecode carrying the marker text rather
        // than dropped, which is what this converter already did; the change is
        // that it is now a NAMED case instead of a `_ =>` that also swallowed
        // real freecodes. The bracketed converter used to drop the same shape,
        // so the two levels disagreed; they now agree.
        ast::ContentItem::OrphanAnnotation(annotation) => {
            UtteranceContent::Freecode(Freecode::new(annotation.chat_text()))
        }
        ast::ContentItem::Retrace(r) => {
            let kind = match r.kind {
                crate::ast::RetraceKindParsed::Partial => RetraceKind::Partial,
                crate::ast::RetraceKindParsed::Complete => RetraceKind::Full,
                crate::ast::RetraceKindParsed::Multiple => RetraceKind::Multiple,
                crate::ast::RetraceKindParsed::Reformulation => RetraceKind::Reformulation,
            };
            let content: Vec<BracketedItem> = r
                .content
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            let mut retrace = Retrace::new(BracketedContent::new(content), kind);
            if r.is_group {
                retrace = retrace.as_group();
            }
            // These are the annotations written AFTER the marker, which is
            // exactly what an `Annotated<Retrace>` wrapper means. `classify.rs`
            // splits the run at the marker's index, so the ones written before
            // it are already on the retraced word inside `r.content` and never
            // reach here.
            let scoped = annotations_to_scoped(&r.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Retrace(Box::new(retrace)),
                Some(scoped) => UtteranceContent::AnnotatedRetrace(Box::new(
                    talkbank_model::model::Annotated::new(retrace, scoped),
                )),
            }
        }
        ast::ContentItem::Group(g) => {
            let content: Vec<BracketedItem> = g
                .contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            let group = Group::new(BracketedContent::new(content));
            let scoped = annotations_to_scoped(&g.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Group(group),
                Some(scoped) => UtteranceContent::AnnotatedGroup(Annotated::new(group, scoped)),
            }
        }
        ast::ContentItem::Quotation(q) => {
            let content: Vec<BracketedItem> = q
                .contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            UtteranceContent::Quotation(Quotation::new(BracketedContent::new(content)))
        }
        ast::ContentItem::OverlapPoint { kind, index } => {
            UtteranceContent::OverlapPoint(overlap_point(*kind, *index))
        }
        ast::ContentItem::MediaBullet { start, end } => {
            UtteranceContent::InternalBullet(bullet_from_times(start, end))
        }
        ast::ContentItem::UnderlineBegin => {
            UtteranceContent::UnderlineBegin(UnderlineMarker::new())
        }
        ast::ContentItem::UnderlineEnd => UtteranceContent::UnderlineEnd(UnderlineMarker::new()),
        ast::ContentItem::LongFeatureBegin(label) => {
            // The label alone ("X", not "&{l=X"); the lexer tag-extracts it.
            UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(LongFeatureLabel::new(*label)))
        }
        ast::ContentItem::LongFeatureEnd(label) => {
            UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(LongFeatureLabel::new(*label)))
        }
        ast::ContentItem::NonvocalBegin(label) => {
            UtteranceContent::NonvocalBegin(NonvocalBegin::new(NonvocalLabel::new(*label)))
        }
        ast::ContentItem::NonvocalEnd(label) => {
            UtteranceContent::NonvocalEnd(NonvocalEnd::new(NonvocalLabel::new(*label)))
        }
        ast::ContentItem::NonvocalSimple(label) => {
            UtteranceContent::NonvocalSimple(NonvocalSimple::new(NonvocalLabel::new(*label)))
        }
        ast::ContentItem::OtherSpokenEvent { speaker, text } => {
            UtteranceContent::OtherSpokenEvent(OtherSpokenEvent::new(*speaker, *text))
        }
        ast::ContentItem::Action { annotations, .. } => {
            let scoped = annotations_to_scoped(annotations);
            // This backend had the same defect as the tree-sitter one: it
            // wrapped EVERY action, so a bare `0` became an annotated node
            // carrying nothing. It now asks the same question the three sites
            // above it were already asking.
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Action(Action::new()),
                Some(scoped) => {
                    UtteranceContent::AnnotatedAction(Annotated::new(Action::new(), scoped))
                }
            }
        }
        ast::ContentItem::PhoGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            UtteranceContent::PhoGroup(PhoGroup::new(BracketedContent::new(items)))
        }
        ast::ContentItem::SinGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            UtteranceContent::SinGroup(SinGroup::new(BracketedContent::new(items)))
        }
    }
}

/// Convert annotation tokens to model ContentAnnotation list.
pub(crate) fn annotations_to_scoped(
    annotations: &[ast::ParsedAnnotation<'_>],
) -> Vec<ContentAnnotation> {
    annotations
        .iter()
        .filter_map(|a| parsed_annotation_to_scoped(a))
        .collect()
}

/// Convert a word with annotations to the appropriate UtteranceContent variant.
/// - No annotations → Word
/// - Has [: replacement] → ReplacedWord (with any other annotations as scoped)
/// - Has other annotations → AnnotatedWord
pub(crate) fn word_with_annotations_to_model(
    w: &ast::WordWithAnnotations<'_>,
    source: SourceText<'_>,
) -> UtteranceContent {
    let word = word_from_parsed(w, source);

    // Check if there's a replacement annotation
    let replacement_idx = w
        .annotations
        .iter()
        .position(|a| matches!(a, crate::ast::ParsedAnnotation::Replacement(_)));

    if let Some(idx) = replacement_idx {
        let replacement_text = match &w.annotations[idx] {
            crate::ast::ParsedAnnotation::Replacement(text) => *text,
            _ => unreachable!(),
        };
        let replacement_words: Vec<Word> = replacement_text
            .split_whitespace()
            .map(parse_word_to_model)
            .collect();
        let replacement = Replacement::new(replacement_words);

        let scoped: Vec<ContentAnnotation> = w
            .annotations
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != idx)
            .filter_map(|(_, a)| parsed_annotation_to_scoped(a))
            .collect();

        let replaced = ReplacedWord::new(word, replacement).with_scoped_annotations(scoped);
        UtteranceContent::ReplacedWord(Box::new(replaced))
    } else {
        let scoped: Vec<ContentAnnotation> = w
            .annotations
            .iter()
            .filter_map(|a| parsed_annotation_to_scoped(a))
            .collect();
        match AnnotatedContentAnnotations::new(scoped) {
            None => UtteranceContent::Word(Box::new(word)),
            Some(scoped) => {
                // Read the span BEFORE the word moves into the wrapper.
                let placed = annotated_span(word.span, &w.annotations, source);
                let annotated = Annotated::new(word, scoped);
                let annotated = match placed {
                    Some(span) => annotated.with_span(span),
                    None => annotated,
                };
                UtteranceContent::AnnotatedWord(Box::new(annotated))
            }
        }
    }
}

/// The span to give an `Annotated` wrapper: the annotated construct, EXTENDED
/// to cover every annotation this backend can place.
///
/// # Why this exists
///
/// `Annotated::new` starts at `Span::DUMMY`, the tree-sitter parser follows it
/// with `.with_span(..)`, and this converter never did. `Span::DUMMY` is
/// `{0, 0}`, which is also a real position, so every E207 this backend
/// produced was reported at "line 1, column 1, bytes 0..0", pointing at
/// `@UTF8`. That is the same defect the separator and word spans carried until
/// 2026-08-27, in the one place the sweep did not reach.
///
/// # It EXTENDS rather than replaces, and REFUSES rather than fabricating
///
/// `span_of` refuses a slice that does not belong to this source rather than
/// inventing an offset, and only some `ParsedAnnotation` variants carry a
/// slice at all. So the construct's own span is the floor, every annotation
/// that CAN be placed widens it, and one that cannot is skipped rather than
/// answered with a zero. The result is therefore an UNDER-approximation when
/// an annotation carries no slice, never a wrong position.
///
/// `None` when there is nothing to say, rather than `Span::DUMMY`: the
/// sentinel IS a real position, and handing it back as an answer is how this
/// defect arose in the first place. The caller then leaves the wrapper's own
/// span untouched instead of overwriting it with a zero.
fn annotated_span(
    construct: Span,
    annotations: &[ast::ParsedAnnotation<'_>],
    source: SourceText<'_>,
) -> Option<Span> {
    // `merge` is a hull, so a DUMMY floor would drag every result back to
    // offset 0 and reintroduce exactly the bug this function exists to fix.
    // A dummy construct span therefore contributes NOTHING and the hull starts
    // at the first annotation that places, which is the honest reading:
    // "somewhere in these annotations" beats "the start of the file".
    let mut span = (!construct.is_dummy()).then_some(construct);
    for annotation in annotations {
        // EXHAUSTIVE, with no catch-all. A wildcard here would silently skip
        // the next slice-carrying variant somebody adds, and the only symptom
        // would be a hull that is quietly too small: no compile error, no test
        // failure, and a diagnostic pointing at less than it should. Five
        // variants were missing from the first version of this match for
        // exactly that reason.
        let slice = match annotation {
            ast::ParsedAnnotation::Unknown(inner)
            | ast::ParsedAnnotation::CodeSwitchExplicit(inner)
            | ast::ParsedAnnotation::Error(inner)
            | ast::ParsedAnnotation::OverlapPrecedes(inner)
            | ast::ParsedAnnotation::OverlapFollows(inner)
            | ast::ParsedAnnotation::Explanation(inner)
            | ast::ParsedAnnotation::Paralinguistic(inner)
            | ast::ParsedAnnotation::Alternative(inner)
            | ast::ParsedAnnotation::PercentComment(inner)
            | ast::ParsedAnnotation::Replacement(inner)
            | ast::ParsedAnnotation::Langcode(inner)
            | ast::ParsedAnnotation::Postcode(inner) => *inner,
            // These carry no slice of the source at all, so there is nothing
            // to place: the marker IS the whole annotation.
            ast::ParsedAnnotation::Retrace(_)
            | ast::ParsedAnnotation::Stressing
            | ast::ParsedAnnotation::ContrastiveStressing
            | ast::ParsedAnnotation::Uncertain
            | ast::ParsedAnnotation::Exclude
            | ast::ParsedAnnotation::CodeSwitchShortcut => continue,
        };
        let Some(placed) = source.span_of(slice) else {
            continue;
        };
        span = Some(match span {
            Some(so_far) => so_far.merge(placed),
            None => placed,
        });
    }
    span
}

/// Parse a word string through the lexer+parser and convert to model Word.
/// Used for replacement words which may have internal structure (compounds, etc.)
pub(crate) fn parse_word_to_model(text: &str) -> Word {
    if let Some(parsed) = crate::parser::parse_word(text) {
        // EVERY SPAN FROM THIS PATH IS ABSENT, and that is the honest outcome
        // rather than the intended one. `parse_word` leaks its own NUL-padded
        // copy of the input and does not hand it back, so the AST's slices
        // borrow from an allocation the caller cannot name. `text` below is a
        // DIFFERENT allocation, and `SourceText::span_of` refuses a slice that
        // does not lie inside the source it was given, so it answers `None`
        // for every one and the word arrives unplaced.
        //
        // This comment previously claimed the opposite, that passing `text`
        // "would LOOK right and place nothing, which is worse than saying so
        // here", directly above the line that passes `text`. The refusal is
        // what keeps it safe: nothing is fabricated, the spans are simply
        // missing. Giving `parse_word` a way to return its source is the fix,
        // and it is the same change the retrace spans need.
        word_from_parsed(&parsed, SourceText::new(text))
    } else {
        Word::simple(text)
    }
}

/// Convert a separator token to model Separator, at `span`.
///
/// `span` is `None` only when the caller paired the item with a source it did
/// not come from, which is a programming error rather than a property of the
/// input. `Span::DUMMY` is the honest answer for "we do not know", and it is
/// what this function used to return UNCONDITIONALLY: every separator arrived
/// at offset zero, and `comma_span()` filters that value out, so E258 and every
/// other span-keyed rule was silently unreachable under this backend.
pub(crate) fn separator_from_kind(kind: ast::SeparatorKindParsed, span: Option<Span>) -> Separator {
    let s = match span {
        Some(span) => span,
        None => Span::DUMMY,
    };
    match kind {
        ast::SeparatorKindParsed::Comma => Separator::Comma { span: s },
        ast::SeparatorKindParsed::Semicolon => Separator::Semicolon { span: s },
        ast::SeparatorKindParsed::Colon => Separator::Colon { span: s },
        ast::SeparatorKindParsed::CaContinuation => Separator::CaContinuation { span: s },
        ast::SeparatorKindParsed::Tag => Separator::Tag { span: s },
        ast::SeparatorKindParsed::Vocative => Separator::Vocative { span: s },
        ast::SeparatorKindParsed::UnmarkedEnding => Separator::UnmarkedEnding { span: s },
        ast::SeparatorKindParsed::Uptake => Separator::Uptake { span: s },
        ast::SeparatorKindParsed::CaNoBreak => Separator::CaNoBreak { span: s },
        ast::SeparatorKindParsed::CaTechnicalBreak => Separator::CaTechnicalBreak { span: s },
        ast::SeparatorKindParsed::RisingToHigh => Separator::RisingToHigh { span: s },
        ast::SeparatorKindParsed::RisingToMid => Separator::RisingToMid { span: s },
        ast::SeparatorKindParsed::Level => Separator::Level { span: s },
        ast::SeparatorKindParsed::FallingToMid => Separator::FallingToMid { span: s },
        ast::SeparatorKindParsed::FallingToLow => Separator::FallingToLow { span: s },
    }
}

/// The one place a parsed pause kind becomes a model duration.
///
/// Exhaustive, because the AST now carries the kind rather than a raw token.
/// It replaces two copies of a match that ended in `_ => PauseDuration::Short`,
/// which silently turned anything unexpected into `(.)`.
fn pause_duration(kind: &ast::PauseKindParsed<'_>) -> PauseDuration {
    match kind {
        ast::PauseKindParsed::Short => PauseDuration::Short,
        ast::PauseKindParsed::Medium => PauseDuration::Medium,
        ast::PauseKindParsed::Long => PauseDuration::Long,
        ast::PauseKindParsed::Timed(s) => PauseDuration::Timed(PauseTimedDuration::new(*s)),
    }
}

// ── Leaf mappings shared by BOTH content levels ─────────────────
//
// `content_item_to_model` (tier level) and `content_item_to_bracketed` are two
// matches over the same `ast::ContentItem`, and these three constructs carry
// real logic rather than a bare variant rename. Writing that logic twice is the
// same defect shape as the one the bracketed converter was just fixed for: two
// copies of one rule, with nothing binding them. The "second character is the
// digit" overlap-index rule in particular had been written three times in this
// file.

/// A parsed overlap kind and index as the model's `OverlapPoint`.
///
/// The kind arrives resolved from the parser, so there is no token to
/// re-inspect and no impossible case to invent an answer for.
fn overlap_point(kind: ast::OverlapKind, index: Option<u32>) -> OverlapPoint {
    let model_kind = match kind {
        ast::OverlapKind::TopBegin => OverlapPointKind::TopOverlapBegin,
        ast::OverlapKind::TopEnd => OverlapPointKind::TopOverlapEnd,
        ast::OverlapKind::BottomBegin => OverlapPointKind::BottomOverlapBegin,
        ast::OverlapKind::BottomEnd => OverlapPointKind::BottomOverlapEnd,
    };
    OverlapPoint::new(model_kind, index.map(OverlapIndex::new))
}

/// The two timestamp texts of a media bullet, as a model `Bullet`.
///
/// `unwrap_or(0)` is retained deliberately and is NOT a silent default: the
/// lexer's bullet rule matches digits only, so the parse can fail only on
/// overflow of a number wider than `u64`, and 0 is the least surprising answer
/// for a timestamp that cannot be represented. Unlike the pause default it
/// replaced, this cannot be reached by any well-formed shape.
pub(crate) fn bullet_from_times(start: &str, end: &str) -> Bullet {
    let (start_ms, end_ms) = bullet_times(start, end);
    Bullet::new(start_ms, end_ms)
}

/// The same parse for callers that need the raw pair rather than a `Bullet`.
///
/// Four sites had written this expression by hand, and only the one with a
/// `Bullet` to build could share the wrapper above, so the justification for
/// `unwrap_or(0)` documented one copy in four.
pub(crate) fn bullet_times(start: &str, end: &str) -> (u64, u64) {
    (start.parse().unwrap_or(0), end.parse().unwrap_or(0))
}

/// Convert a content item to a BracketedItem (for inside groups/quotations/retraces).
///
/// Exhaustive over `ast::ContentItem`, and denied from regaining a catch-all.
/// It HAD one, and it silently deleted ten variants from anything written
/// inside `<...>`, a retrace, a quotation or a pho/sin group: both underline
/// markers, all five scoped markers, CA markers, overlap points and media
/// bullets. The model represents every one of them in `BracketedItem`, so this
/// was pure loss, and it made the two parser backends disagree about whether a
/// valid transcript was valid.
///
/// The guarantee is rustc's own exhaustiveness check, not a lint: with the
/// catch-all gone, a new `ast::ContentItem` variant is an E0004 compile error
/// here, which fires under `cargo test` rather than only in CI's clippy pass.
/// `#[deny(clippy::wildcard_enum_match_arm)]` is deliberately NOT applied: it
/// covers nested matches too, and several arms below legitimately match on
/// `Token` (~180 variants), where enumeration buys nothing. Design rule 3 is
/// about the CONTENT enums, and the textual catch-all ratchet in
/// `talkbank-parser-tests` is what holds that line for this file.
pub(crate) fn content_item_to_bracketed(
    item: &ast::ContentItem<'_>,
    source: SourceText<'_>,
) -> BracketedItem {
    match item {
        ast::ContentItem::Word(w) => {
            let word = word_from_parsed(w, source);
            let replacement_idx = w
                .annotations
                .iter()
                .position(|a| matches!(a, crate::ast::ParsedAnnotation::Replacement(_)));

            if let Some(idx) = replacement_idx {
                let replacement_text = match &w.annotations[idx] {
                    crate::ast::ParsedAnnotation::Replacement(text) => *text,
                    _ => unreachable!(),
                };
                let replacement_words: Vec<Word> = replacement_text
                    .split_whitespace()
                    .map(parse_word_to_model)
                    .collect();
                let replacement = Replacement::new(replacement_words);
                let scoped: Vec<ContentAnnotation> = w
                    .annotations
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != idx)
                    .filter_map(|(_, a)| parsed_annotation_to_scoped(a))
                    .collect();
                let replaced = ReplacedWord::new(word, replacement).with_scoped_annotations(scoped);
                BracketedItem::ReplacedWord(Box::new(replaced))
            } else {
                let scoped = annotations_to_scoped(&w.annotations);
                match AnnotatedContentAnnotations::new(scoped) {
                    None => BracketedItem::Word(Box::new(word)),
                    Some(scoped) => {
                        BracketedItem::AnnotatedWord(Box::new(Annotated::new(word, scoped)))
                    }
                }
            }
        }
        ast::ContentItem::Pause(kind) => BracketedItem::Pause(Pause::new(pause_duration(kind))),
        ast::ContentItem::Event(event_text) => {
            let event_text = *event_text;
            BracketedItem::Event(Event::new(event_text))
        }
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            let event_text = *event;
            let event_model = Event::new(event_text);
            let scoped = annotations_to_scoped(annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => BracketedItem::Event(event_model),
                Some(scoped) => BracketedItem::AnnotatedEvent(Annotated::new(event_model, scoped)),
            }
        }
        ast::ContentItem::Action { annotations, .. } => {
            let scoped = annotations_to_scoped(annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => BracketedItem::Action(Action::new()),
                Some(scoped) => {
                    BracketedItem::AnnotatedAction(Annotated::new(Action::new(), scoped))
                }
            }
        }
        ast::ContentItem::OtherSpokenEvent { speaker, text } => {
            BracketedItem::OtherSpokenEvent(OtherSpokenEvent::new(*speaker, *text))
        }
        ast::ContentItem::Separator { kind, text } => {
            let sep = separator_from_kind(*kind, source.span_of(text));
            BracketedItem::Separator(sep)
        }
        ast::ContentItem::Group(g) => {
            let inner: Vec<BracketedItem> = g
                .contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            let group = Group::new(BracketedContent::new(inner));
            // `BracketedItem` HAS a bare `Group` now. It did not until
            // 2026-08-26, which is why this used to hand an empty annotation
            // list to `AnnotatedGroup` and explain itself in a comment.
            let scoped = annotations_to_scoped(&g.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => BracketedItem::Group(group),
                Some(scoped) => BracketedItem::AnnotatedGroup(Annotated::new(group, scoped)),
            }
        }
        ast::ContentItem::Retrace(r) => {
            let kind = match r.kind {
                crate::ast::RetraceKindParsed::Partial => RetraceKind::Partial,
                crate::ast::RetraceKindParsed::Complete => RetraceKind::Full,
                crate::ast::RetraceKindParsed::Multiple => RetraceKind::Multiple,
                crate::ast::RetraceKindParsed::Reformulation => RetraceKind::Reformulation,
            };
            let inner: Vec<BracketedItem> = r
                .content
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            let mut retrace =
                talkbank_model::model::Retrace::new(BracketedContent::new(inner), kind);
            if r.is_group {
                retrace = retrace.as_group();
            }
            // As at the tier-level site above: only the after-the-marker
            // annotations reach here.
            let scoped = annotations_to_scoped(&r.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => BracketedItem::Retrace(Box::new(retrace)),
                Some(scoped) => BracketedItem::AnnotatedRetrace(Box::new(
                    talkbank_model::model::Annotated::new(retrace, scoped),
                )),
            }
        }
        ast::ContentItem::Freecode(text) => BracketedItem::Freecode(Freecode::new(*text)),
        // Preserved, matching the tier-level arm. This used to return `None`,
        // so the same orphaned annotation survived at tier level and vanished
        // inside a group.
        ast::ContentItem::OrphanAnnotation(annotation) => {
            BracketedItem::Freecode(Freecode::new(annotation.chat_text()))
        }
        ast::ContentItem::OverlapPoint { kind, index } => {
            BracketedItem::OverlapPoint(overlap_point(*kind, *index))
        }
        ast::ContentItem::MediaBullet { start, end } => {
            BracketedItem::InternalBullet(bullet_from_times(start, end))
        }
        ast::ContentItem::UnderlineBegin => BracketedItem::UnderlineBegin(UnderlineMarker::new()),
        ast::ContentItem::UnderlineEnd => BracketedItem::UnderlineEnd(UnderlineMarker::new()),
        ast::ContentItem::LongFeatureBegin(label) => {
            BracketedItem::LongFeatureBegin(LongFeatureBegin::new(LongFeatureLabel::new(*label)))
        }
        ast::ContentItem::LongFeatureEnd(label) => {
            BracketedItem::LongFeatureEnd(LongFeatureEnd::new(LongFeatureLabel::new(*label)))
        }
        ast::ContentItem::NonvocalBegin(label) => {
            BracketedItem::NonvocalBegin(NonvocalBegin::new(NonvocalLabel::new(*label)))
        }
        ast::ContentItem::NonvocalEnd(label) => {
            BracketedItem::NonvocalEnd(NonvocalEnd::new(NonvocalLabel::new(*label)))
        }
        ast::ContentItem::NonvocalSimple(label) => {
            BracketedItem::NonvocalSimple(NonvocalSimple::new(NonvocalLabel::new(*label)))
        }
        ast::ContentItem::PhoGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            BracketedItem::PhoGroup(PhoGroup::new(BracketedContent::new(items)))
        }
        ast::ContentItem::SinGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            BracketedItem::SinGroup(SinGroup::new(BracketedContent::new(items)))
        }
        ast::ContentItem::Quotation(q) => {
            let items: Vec<BracketedItem> = q
                .contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            BracketedItem::Quotation(Quotation::new(BracketedContent::new(items)))
        }
    }
}

/// Convert a single annotation token to a ContentAnnotation.
/// All tokens carry tag-extracted content, no string manipulation needed.
/// Convert a parsed annotation to a model ContentAnnotation.
pub(crate) fn parsed_annotation_to_scoped(
    ann: &ast::ParsedAnnotation<'_>,
) -> Option<ContentAnnotation> {
    match ann {
        crate::ast::ParsedAnnotation::Retrace(_) => None, // Retraces handled at content level
        // `[@ xyz]` carries `"@ xyz"`. The MARKER is the leading run of
        // non-space characters and the TEXT is whatever follows it, matching
        // `ScopedUnknown`'s two fields; a marker with no text (`[@@@]`) keeps
        // an empty text rather than borrowing the marker into it.
        crate::ast::ParsedAnnotation::Unknown(inner) => {
            let (marker, text) = match inner.split_once(' ') {
                Some((marker, text)) => (marker, text.trim()),
                None => (*inner, ""),
            };
            Some(ContentAnnotation::Unknown(
                talkbank_model::model::ScopedUnknown {
                    marker: marker.into(),
                    text: text.into(),
                },
            ))
        }
        crate::ast::ParsedAnnotation::Stressing => Some(ContentAnnotation::Stressing),
        crate::ast::ParsedAnnotation::ContrastiveStressing => {
            Some(ContentAnnotation::ContrastiveStressing)
        }
        crate::ast::ParsedAnnotation::Uncertain => Some(ContentAnnotation::Uncertain),
        crate::ast::ParsedAnnotation::Exclude => Some(ContentAnnotation::Exclude),
        crate::ast::ParsedAnnotation::CodeSwitchShortcut => Some(ContentAnnotation::CodeSwitch(
            talkbank_model::model::CodeSwitchSpan::Shortcut,
        )),
        crate::ast::ParsedAnnotation::CodeSwitchExplicit(code) => Some(
            ContentAnnotation::CodeSwitch(talkbank_model::model::CodeSwitchSpan::Explicit(
                // Same justification as the word-level `@s:` suffix above: the
                // lexer's `[a-z]{2,4}` matches `language_code` exactly, so this
                // is defensive only.
                talkbank_model::LanguageCode::new(*code)
                    .expect("lexer-guaranteed language_code shape"),
            )),
        ),
        crate::ast::ParsedAnnotation::Error(s) => {
            let code = if s.is_empty() {
                None
            } else {
                Some((*s).into())
            };
            Some(ContentAnnotation::Error(ScopedError { code }))
        }
        crate::ast::ParsedAnnotation::OverlapPrecedes(s) => {
            let index = if s.is_empty() {
                None
            } else {
                s.parse().ok().map(OverlapMarkerIndex::new)
            };
            Some(ContentAnnotation::OverlapBegin(ScopedOverlapBegin {
                index,
            }))
        }
        crate::ast::ParsedAnnotation::OverlapFollows(s) => {
            let index = if s.is_empty() {
                None
            } else {
                s.parse().ok().map(OverlapMarkerIndex::new)
            };
            Some(ContentAnnotation::OverlapEnd(ScopedOverlapEnd { index }))
        }
        crate::ast::ParsedAnnotation::Explanation(s) => {
            Some(ContentAnnotation::Explanation(ScopedExplanation {
                text: (*s).into(),
            }))
        }
        crate::ast::ParsedAnnotation::Paralinguistic(s) => {
            Some(ContentAnnotation::Paralinguistic(ScopedParalinguistic {
                text: (*s).into(),
            }))
        }
        crate::ast::ParsedAnnotation::Alternative(s) => {
            Some(ContentAnnotation::Alternative(ScopedAlternative {
                text: (*s).into(),
            }))
        }
        crate::ast::ParsedAnnotation::PercentComment(s) => {
            Some(ContentAnnotation::PercentComment(ScopedPercentComment {
                text: (*s).into(),
            }))
        }
        crate::ast::ParsedAnnotation::Replacement(_) => None, // Handled separately in word conversion
        crate::ast::ParsedAnnotation::Langcode(_) | crate::ast::ParsedAnnotation::Postcode(_) => {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Terminator conversion
// ═══════════════════════════════════════════════════════════════

/// Convert a terminator token to a model `Terminator`.
///
/// `None` for a token that is not a terminator. It used to fall back to
/// `Terminator::Period`, which fabricated a sentence-final period out of
/// whatever arrived: the same silent-wrong-answer shape as the pause and
/// separator defaults, and here it invents PUNCTUATION that changes what the
/// utterance means. An absent terminator is a condition the model can state
/// and the validators already report.
pub fn token_to_terminator(tok: &Token<'_>) -> Option<Terminator> {
    ast::TerminatorKindParsed::from_token(tok).map(terminator_from_kind)
}

/// A resolved terminator kind as the model's `Terminator`.
///
/// Exhaustive: the kind arrives resolved, so there is no unmatched token to
/// answer for. This replaced `_ => Terminator::Period`, which invented
/// sentence-final punctuation out of whatever arrived.
fn terminator_from_kind(kind: ast::TerminatorKindParsed) -> Terminator {
    let s = Span::DUMMY;
    match kind {
        ast::TerminatorKindParsed::Period => Terminator::Period { span: s },
        ast::TerminatorKindParsed::Question => Terminator::Question { span: s },
        ast::TerminatorKindParsed::Exclamation => Terminator::Exclamation { span: s },
        ast::TerminatorKindParsed::TrailingOff => Terminator::TrailingOff { span: s },
        ast::TerminatorKindParsed::Interruption => Terminator::Interruption { span: s },
        ast::TerminatorKindParsed::SelfInterruption => Terminator::SelfInterruption { span: s },
        ast::TerminatorKindParsed::InterruptedQuestion => {
            Terminator::InterruptedQuestion { span: s }
        }
        ast::TerminatorKindParsed::BrokenQuestion => Terminator::BrokenQuestion { span: s },
        ast::TerminatorKindParsed::QuotedNewLine => Terminator::QuotedNewLine { span: s },
        ast::TerminatorKindParsed::QuotedPeriodSimple => Terminator::QuotedPeriodSimple { span: s },
        ast::TerminatorKindParsed::SelfInterruptedQuestion => {
            Terminator::SelfInterruptedQuestion { span: s }
        }
        ast::TerminatorKindParsed::TrailingOffQuestion => {
            Terminator::TrailingOffQuestion { span: s }
        }
        ast::TerminatorKindParsed::BreakForCoding => Terminator::BreakForCoding { span: s },
    }
}

// ═══════════════════════════════════════════════════════════════
// MainTier conversion
// ═══════════════════════════════════════════════════════════════
