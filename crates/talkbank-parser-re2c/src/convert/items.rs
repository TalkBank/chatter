//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use crate::ast;
use crate::ast::{CaDelimiterKind, CaElementKind, OverlapKind, StressKind, WordBodyItem};
use crate::token::Token;
use talkbank_model::Span;
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

// ═══════════════════════════════════════════════════════════════
// WordWithAnnotations → Word
// ═══════════════════════════════════════════════════════════════

/// Convert a parsed word to the model Word type.
/// Uses the word's self-contained `raw_text`, no external `source` needed.
pub fn word_from_parsed(w: &ast::WordWithAnnotations<'_>) -> Word {
    let raw = w.raw_text;
    let cleaned = compute_cleaned_text(&w.body);

    let content_items: Vec<WordContent> = w.body.iter().map(body_item_to_word_content).collect();

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

    // Form marker, tag-extracted content, direct to model
    if let Some(marker) = w.form_marker {
        if let Some(ft) = FormType::parse(marker) {
            word = word.with_form_type(ft);
        } else if marker.starts_with("z") {
            // User-defined form: z or z:label
            let label = marker.strip_prefix("z").unwrap_or("");
            let label = label.strip_prefix(':').unwrap_or(label);
            word = word.with_form_type(FormType::UserDefined(label.to_string()));
        }
    }

    // Language suffix, typed enum, no string hacking
    if let Some(ref lang) = w.lang {
        word = match lang {
            crate::ast::ParsedLangSuffix::Shortcut => word.with_language_shortcut(),
            crate::ast::ParsedLangSuffix::Explicit(codes) if codes.contains('+') => {
                let lc: Vec<LanguageCode> = codes.split('+').map(LanguageCode::new).collect();
                word.lang = Some(WordLanguageMarker::Multiple(lc));
                word
            }
            crate::ast::ParsedLangSuffix::Explicit(codes) if codes.contains('&') => {
                let lc: Vec<LanguageCode> = codes.split('&').map(LanguageCode::new).collect();
                word.lang = Some(WordLanguageMarker::Ambiguous(lc));
                word
            }
            crate::ast::ParsedLangSuffix::Explicit(code) => {
                word.with_lang(LanguageCode::new(*code))
            }
        };
    }

    // POS tag, tag-extracted content
    if let Some(tag) = w.pos_tag {
        word = word.with_part_of_speech(tag);
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
    match tok {
        Token::LinkerLazyOverlap(_) => Some(Linker::LazyOverlapPrecedes),
        Token::LinkerQuickUptake(_) => Some(Linker::OtherCompletion),
        Token::LinkerQuickUptakeOverlap(_) => Some(Linker::QuickUptakeOverlap),
        Token::LinkerQuotationFollows(_) => Some(Linker::QuotationFollows),
        Token::LinkerSelfCompletion(_) => Some(Linker::SelfCompletion),
        Token::CaNoBreakLinker(_) => Some(Linker::NoBreakTcuContinuation),
        Token::CaTechnicalBreakLinker(_) => Some(Linker::TcuContinuation),
        _ => None,
    }
}

pub fn content_item_to_model(item: &ast::ContentItem<'_>) -> UtteranceContent {
    match item {
        ast::ContentItem::Word(w) => word_with_annotations_to_model(w),
        ast::ContentItem::Pause(tok) => {
            let duration = match tok {
                Token::PauseShort(_) => PauseDuration::Short,
                Token::PauseMedium(_) => PauseDuration::Medium,
                Token::PauseLong(_) => PauseDuration::Long,
                Token::PauseTimed(s) => PauseDuration::Timed(PauseTimedDuration::new(*s)),
                _ => PauseDuration::Short,
            };
            UtteranceContent::Pause(Pause::new(duration))
        }
        ast::ContentItem::Event(toks) => {
            // The lexer emits a single Event token with the description text.
            let event_text = toks.first().map(|t| t.text()).unwrap_or("");
            UtteranceContent::Event(Event::new(event_text))
        }
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            let event_text = event.text();
            let event_model = Event::new(event_text);
            let scoped = annotations_to_scoped(annotations);
            if scoped.is_empty() {
                UtteranceContent::Event(event_model)
            } else {
                UtteranceContent::AnnotatedEvent(
                    Annotated::new(event_model).with_scoped_annotations(scoped),
                )
            }
        }
        ast::ContentItem::Separator(tok) => {
            UtteranceContent::Separator(separator_token_to_model(tok))
        }
        ast::ContentItem::Annotation(tok) => {
            match tok {
                Token::Freecode(s) => {
                    // Token carries tag-extracted content directly
                    UtteranceContent::Freecode(Freecode::new(*s))
                }
                _ => UtteranceContent::Freecode(Freecode::new(tok.text())),
            }
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
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            let mut retrace = Retrace::new(BracketedContent::new(content), kind);
            if r.is_group {
                retrace = retrace.as_group();
            }
            // Move non-retrace annotations from the AST to model retrace
            let scoped = annotations_to_scoped(&r.annotations);
            if !scoped.is_empty() {
                retrace = retrace.with_annotations(scoped);
            }
            UtteranceContent::Retrace(Box::new(retrace))
        }
        ast::ContentItem::Group(g) => {
            let content: Vec<BracketedItem> = g
                .contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            let group = Group::new(BracketedContent::new(content));
            let scoped = annotations_to_scoped(&g.annotations);
            if scoped.is_empty() {
                UtteranceContent::Group(group)
            } else {
                let annotated = Annotated::new(group).with_scoped_annotations(scoped);
                UtteranceContent::AnnotatedGroup(annotated)
            }
        }
        ast::ContentItem::Quotation(q) => {
            let content: Vec<BracketedItem> = q
                .contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            UtteranceContent::Quotation(Quotation::new(BracketedContent::new(content)))
        }
        ast::ContentItem::OverlapPoint(tok) => {
            let raw = tok.text();
            let kind = match tok {
                Token::OverlapTopBegin(_) => OverlapPointKind::TopOverlapBegin,
                Token::OverlapTopEnd(_) => OverlapPointKind::TopOverlapEnd,
                Token::OverlapBottomBegin(_) => OverlapPointKind::BottomOverlapBegin,
                Token::OverlapBottomEnd(_) => OverlapPointKind::BottomOverlapEnd,
                _ => unreachable!(),
            };
            let index = raw
                .chars()
                .nth(1)
                .and_then(|c| c.to_digit(10))
                .map(OverlapIndex::new);
            UtteranceContent::OverlapPoint(OverlapPoint::new(kind, index))
        }
        ast::ContentItem::MediaBullet(tok) => match tok {
            Token::MediaBullet {
                start_time,
                end_time,
                ..
            } => {
                let start_ms: u64 = start_time.parse().unwrap_or(0);
                let end_ms: u64 = end_time.parse().unwrap_or(0);
                UtteranceContent::InternalBullet(Bullet::new(start_ms, end_ms))
            }
            _ => unreachable!(),
        },
        ast::ContentItem::UnderlineBegin(_) => {
            UtteranceContent::UnderlineBegin(UnderlineMarker::new())
        }
        ast::ContentItem::UnderlineEnd(_) => UtteranceContent::UnderlineEnd(UnderlineMarker::new()),
        ast::ContentItem::CaMarker(tok) => {
            let raw = tok.text();
            // CA markers at content level are wrapped as Word in the model
            UtteranceContent::Word(Box::new(Word::new_unchecked(raw, raw).with_content(
                WordContents::new(smallvec::smallvec![WordContent::Text(
                    WordText::new_unchecked(raw)
                )]),
            )))
        }
        ast::ContentItem::LongFeatureBegin(tok) => {
            // Token carries tag-extracted label directly (e.g., "X" not "&{l=X")
            UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(LongFeatureLabel::new(
                tok.text(),
            )))
        }
        ast::ContentItem::LongFeatureEnd(tok) => {
            UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(LongFeatureLabel::new(tok.text())))
        }
        ast::ContentItem::NonvocalBegin(tok) => {
            UtteranceContent::NonvocalBegin(NonvocalBegin::new(NonvocalLabel::new(tok.text())))
        }
        ast::ContentItem::NonvocalEnd(tok) => {
            UtteranceContent::NonvocalEnd(NonvocalEnd::new(NonvocalLabel::new(tok.text())))
        }
        ast::ContentItem::NonvocalSimple(tok) => {
            UtteranceContent::NonvocalSimple(NonvocalSimple::new(NonvocalLabel::new(tok.text())))
        }
        ast::ContentItem::OtherSpokenEvent(tok) => match tok {
            Token::OtherSpokenEvent { speaker, text } => {
                UtteranceContent::OtherSpokenEvent(OtherSpokenEvent::new(*speaker, *text))
            }
            _ => unreachable!("OtherSpokenEvent content item must carry OtherSpokenEvent token"),
        },
        ast::ContentItem::Action { annotations, .. } => {
            let scoped = annotations_to_scoped(annotations);
            let annotated = Annotated::new(Action::new()).with_scoped_annotations(scoped);
            UtteranceContent::AnnotatedAction(annotated)
        }
        ast::ContentItem::PhoGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            UtteranceContent::PhoGroup(PhoGroup::new(BracketedContent::new(items)))
        }
        ast::ContentItem::SinGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
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
pub(crate) fn word_with_annotations_to_model(w: &ast::WordWithAnnotations<'_>) -> UtteranceContent {
    let word = word_from_parsed(w);

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
        if scoped.is_empty() {
            UtteranceContent::Word(Box::new(word))
        } else {
            let annotated = Annotated::new(word).with_scoped_annotations(scoped);
            UtteranceContent::AnnotatedWord(Box::new(annotated))
        }
    }
}

/// Parse a word string through the lexer+parser and convert to model Word.
/// Used for replacement words which may have internal structure (compounds, etc.)
pub(crate) fn parse_word_to_model(text: &str) -> Word {
    if let Some(parsed) = crate::parser::parse_word(text) {
        word_from_parsed(&parsed)
    } else {
        Word::simple(text)
    }
}

/// Convert a separator token to model Separator.
pub(crate) fn separator_token_to_model(tok: &Token<'_>) -> Separator {
    let s = Span::DUMMY;
    match tok {
        Token::Comma(_) => Separator::Comma { span: s },
        Token::Semicolon(_) => Separator::Semicolon { span: s },
        Token::Colon(_) => Separator::Colon { span: s },
        Token::CaContinuationMarker(_) => Separator::CaContinuation { span: s },
        Token::TagMarker(_) => Separator::Tag { span: s },
        Token::VocativeMarker(_) => Separator::Vocative { span: s },
        Token::UnmarkedEnding(_) => Separator::UnmarkedEnding { span: s },
        Token::UptakeSymbol(_) => Separator::Uptake { span: s },
        Token::CaNoBreak(_) => Separator::CaNoBreak { span: s },
        Token::CaTechnicalBreak(_) => Separator::CaTechnicalBreak { span: s },
        Token::RisingToHigh(_) => Separator::RisingToHigh { span: s },
        Token::RisingToMid(_) => Separator::RisingToMid { span: s },
        Token::LevelPitch(_) => Separator::Level { span: s },
        Token::FallingToMid(_) => Separator::FallingToMid { span: s },
        Token::FallingToLow(_) => Separator::FallingToLow { span: s },
        Token::Lengthening(_) => Separator::Colon { span: s },
        _ => Separator::Comma { span: s },
    }
}

/// Convert a content item to a BracketedItem (for inside groups/quotations/retraces).
pub(crate) fn content_item_to_bracketed(item: &ast::ContentItem<'_>) -> Option<BracketedItem> {
    match item {
        ast::ContentItem::Word(w) => {
            let word = word_from_parsed(w);
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
                Some(BracketedItem::ReplacedWord(Box::new(replaced)))
            } else {
                let scoped = annotations_to_scoped(&w.annotations);
                if scoped.is_empty() {
                    Some(BracketedItem::Word(Box::new(word)))
                } else {
                    let annotated = Annotated::new(word).with_scoped_annotations(scoped);
                    Some(BracketedItem::AnnotatedWord(Box::new(annotated)))
                }
            }
        }
        ast::ContentItem::Pause(tok) => {
            let duration = match tok {
                Token::PauseShort(_) => PauseDuration::Short,
                Token::PauseMedium(_) => PauseDuration::Medium,
                Token::PauseLong(_) => PauseDuration::Long,
                Token::PauseTimed(s) => PauseDuration::Timed(PauseTimedDuration::new(*s)),
                _ => PauseDuration::Short,
            };
            Some(BracketedItem::Pause(Pause::new(duration)))
        }
        ast::ContentItem::Event(toks) => {
            let event_text = toks.first().map(|t| t.text()).unwrap_or("");
            Some(BracketedItem::Event(Event::new(event_text)))
        }
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            let event_text = event.text();
            let event_model = Event::new(event_text);
            let scoped = annotations_to_scoped(annotations);
            if scoped.is_empty() {
                Some(BracketedItem::Event(event_model))
            } else {
                Some(BracketedItem::AnnotatedEvent(
                    Annotated::new(event_model).with_scoped_annotations(scoped),
                ))
            }
        }
        ast::ContentItem::Action { annotations, .. } => {
            let scoped = annotations_to_scoped(annotations);
            let annotated = Annotated::new(Action::new()).with_scoped_annotations(scoped);
            Some(BracketedItem::AnnotatedAction(annotated))
        }
        ast::ContentItem::OtherSpokenEvent(tok) => match tok {
            Token::OtherSpokenEvent { speaker, text } => Some(BracketedItem::OtherSpokenEvent(
                OtherSpokenEvent::new(*speaker, *text),
            )),
            _ => unreachable!(),
        },
        ast::ContentItem::Separator(tok) => {
            let sep = separator_token_to_model(tok);
            Some(BracketedItem::Separator(sep))
        }
        ast::ContentItem::Group(g) => {
            let inner: Vec<BracketedItem> = g
                .contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            let group = Group::new(BracketedContent::new(inner));
            let scoped = annotations_to_scoped(&g.annotations);
            if scoped.is_empty() {
                // Bare nested group, not directly representable as BracketedItem,
                // but AnnotatedGroup with empty annotations works
                Some(BracketedItem::AnnotatedGroup(
                    Annotated::new(group).with_scoped_annotations(scoped),
                ))
            } else {
                Some(BracketedItem::AnnotatedGroup(
                    Annotated::new(group).with_scoped_annotations(scoped),
                ))
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
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            let mut retrace =
                talkbank_model::model::Retrace::new(BracketedContent::new(inner), kind);
            if r.is_group {
                retrace = retrace.as_group();
            }
            let scoped = annotations_to_scoped(&r.annotations);
            if !scoped.is_empty() {
                retrace = retrace.with_annotations(scoped);
            }
            Some(BracketedItem::Retrace(Box::new(retrace)))
        }
        ast::ContentItem::Annotation(Token::Freecode(s)) => {
            Some(BracketedItem::Freecode(Freecode::new(*s)))
        }
        ast::ContentItem::Annotation(_) => None,
        ast::ContentItem::PhoGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            Some(BracketedItem::PhoGroup(PhoGroup::new(
                BracketedContent::new(items),
            )))
        }
        ast::ContentItem::SinGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            Some(BracketedItem::SinGroup(SinGroup::new(
                BracketedContent::new(items),
            )))
        }
        ast::ContentItem::Quotation(q) => {
            let items: Vec<BracketedItem> = q
                .contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            Some(BracketedItem::Quotation(Quotation::new(
                BracketedContent::new(items),
            )))
        }
        _ => None,
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
        crate::ast::ParsedAnnotation::Stressing => Some(ContentAnnotation::Stressing),
        crate::ast::ParsedAnnotation::ContrastiveStressing => {
            Some(ContentAnnotation::ContrastiveStressing)
        }
        crate::ast::ParsedAnnotation::Uncertain => Some(ContentAnnotation::Uncertain),
        crate::ast::ParsedAnnotation::Exclude => Some(ContentAnnotation::Exclude),
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

/// Convert a terminator token to model Terminator.
pub fn token_to_terminator(tok: &Token<'_>) -> Terminator {
    let s = Span::DUMMY;
    match tok {
        Token::Period(_) => Terminator::Period { span: s },
        Token::Question(_) => Terminator::Question { span: s },
        Token::Exclamation(_) => Terminator::Exclamation { span: s },
        Token::TrailingOff(_) => Terminator::TrailingOff { span: s },
        Token::Interruption(_) => Terminator::Interruption { span: s },
        Token::SelfInterruption(_) => Terminator::SelfInterruption { span: s },
        Token::InterruptedQuestion(_) => Terminator::InterruptedQuestion { span: s },
        Token::BrokenQuestion(_) => Terminator::BrokenQuestion { span: s },
        Token::QuotedNewLine(_) => Terminator::QuotedNewLine { span: s },
        Token::QuotedPeriodSimple(_) => Terminator::QuotedPeriodSimple { span: s },
        Token::SelfInterruptedQuestion(_) => Terminator::SelfInterruptedQuestion { span: s },
        Token::TrailingOffQuestion(_) => Terminator::TrailingOffQuestion { span: s },
        Token::BreakForCoding(_) => Terminator::BreakForCoding { span: s },
        _ => Terminator::Period { span: s },
    }
}

// ═══════════════════════════════════════════════════════════════
// MainTier conversion
// ═══════════════════════════════════════════════════════════════
