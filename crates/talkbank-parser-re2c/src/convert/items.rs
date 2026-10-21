//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use super::words::word_from_parsed;
use crate::ast;
use crate::source_text::SourceText;
use crate::token::Token;
use talkbank_model::Span;
use talkbank_model::annotation::AnnotatedContentAnnotations;
use talkbank_model::model::*;

// ═══════════════════════════════════════════════════════════════
// ContentItem → UtteranceContent
// ═══════════════════════════════════════════════════════════════

/// Convert a ContentItem to a model UtteranceContent.
/// Every content item type has a proper model representation.
/// Convert a linker token to a model Linker.
pub(crate) fn linker_token_to_model(tok: &Token<'_>, source: SourceText<'_>) -> Option<Linker> {
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
    Some(Linker::new(super::locate(source, tok.text()), kind))
}

pub fn content_item_to_model(
    item: &ast::ContentItem<'_>,
    source: SourceText<'_>,
) -> Option<UtteranceContent> {
    Some(match item {
        ast::ContentItem::Word(w) => word_with_annotations_to_model(w, source),
        ast::ContentItem::Pause(kind) => UtteranceContent::Pause(pause_from_parsed(kind)),
        ast::ContentItem::Event(event) => UtteranceContent::Event(Event::new(
            super::locate(source, event.lexeme()),
            event.text(),
        )),
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            let event_location = super::locate(source, event.lexeme());
            let event_model = Event::new(event_location, event.text());
            let scoped = annotations_to_scoped(annotations);
            // The constructor's `Option` IS the bare-versus-annotated
            // question, so it replaces the `is_empty` check that used to ask it
            // separately.
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Event(event_model),
                Some(scoped) => UtteranceContent::AnnotatedEvent(Annotated::new(
                    annotated_location(event_location, annotations, source),
                    event_model,
                    scoped,
                )),
            }
        }
        ast::ContentItem::Separator { kind, text } => {
            UtteranceContent::Separator(separator_from_kind(*kind, source.span_of(text)))
        }
        ast::ContentItem::Freecode(text) => UtteranceContent::Freecode(Freecode::new(
            super::locate(source, text.lexeme()),
            text.text(),
        )),
        // An annotation with nothing to scope over is invalid CHAT, reported as
        // E759. It is preserved as a freecode carrying the marker text rather
        // than dropped, which is what this converter already did; the change is
        // that it is now a NAMED case instead of a `_ =>` that also swallowed
        // real freecodes. The bracketed converter used to drop the same shape,
        // so the two levels disagreed; they now agree.
        ast::ContentItem::OrphanAnnotation(annotation) => {
            UtteranceContent::Freecode(Freecode::new(
                super::located(annotation.span_in(source)),
                orphan_text(annotation, source),
            ))
        }
        ast::ContentItem::Retrace(r) => {
            let kind = match r.kind {
                crate::ast::RetraceKindParsed::Partial => RetraceKind::Partial,
                crate::ast::RetraceKindParsed::Complete => RetraceKind::Full,
                crate::ast::RetraceKindParsed::Multiple => RetraceKind::Multiple,
                crate::ast::RetraceKindParsed::Reformulation => RetraceKind::Reformulation,
            };
            let content: Vec<BracketedItem> = r
                .body
                .items()
                .iter()
                .filter_map(|i| content_item_to_bracketed(i, source))
                .collect();
            let retrace_location = super::located(retrace_span(r, source));
            let mut retrace = Retrace::new(
                retrace_location,
                super::locate(source, r.marker),
                BracketedContent::new(content),
                kind,
            );
            if r.is_group() {
                retrace = retrace.as_group();
            }
            // These are the annotations written AFTER the marker, which is
            // exactly what an `Annotated<Retrace>` wrapper means. `classify.rs`
            // splits the run at the marker's index, so the ones written before
            // it are already on the retraced word inside `r.body` and never
            // reach here.
            let scoped = annotations_to_scoped(&r.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Retrace(Box::new(retrace)),
                Some(scoped) => UtteranceContent::AnnotatedRetrace(Box::new(
                    talkbank_model::model::Annotated::new(
                        annotated_location(retrace_location, &r.annotations, source),
                        retrace,
                        scoped,
                    ),
                )),
            }
        }
        ast::ContentItem::Group(g) => {
            let content: Vec<BracketedItem> = g
                .contents
                .iter()
                .filter_map(|i| content_item_to_bracketed(i, source))
                .collect();
            let group_location = super::located(g.enclosure.span_in(source));
            let group = Group::new(group_location, BracketedContent::new(content));
            let scoped = annotations_to_scoped(&g.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Group(group),
                Some(scoped) => UtteranceContent::AnnotatedGroup(Annotated::new(
                    annotated_location(group_location, &g.annotations, source),
                    group,
                    scoped,
                )),
            }
        }
        ast::ContentItem::Quotation(q) => match ModelQuotation::from_parsed(q, source) {
            ModelQuotation::Bare(quotation) => UtteranceContent::Quotation(quotation),
            ModelQuotation::Annotated(quotation) => UtteranceContent::AnnotatedQuotation(quotation),
        },
        ast::ContentItem::OverlapPoint { kind, index, text } => UtteranceContent::OverlapPoint(
            overlap_point(*kind, *index, super::locate(source, text)),
        ),
        ast::ContentItem::MediaBullet { bullet } => {
            UtteranceContent::InternalBullet(bullet_from_token(*bullet, source)?)
        }
        ast::ContentItem::UnderlineBegin(text) => {
            UtteranceContent::UnderlineBegin(UnderlineMarker::new(super::locate(source, text)))
        }
        ast::ContentItem::UnderlineEnd(text) => {
            UtteranceContent::UnderlineEnd(UnderlineMarker::new(super::locate(source, text)))
        }
        ast::ContentItem::LongFeatureBegin(label) => {
            // The label alone ("X", not "&{l=X"); the lexer tag-extracts it.
            UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(
                super::locate(source, label.lexeme()),
                LongFeatureLabel::new(label.text()),
            ))
        }
        ast::ContentItem::LongFeatureEnd(label) => {
            UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(
                super::locate(source, label.lexeme()),
                LongFeatureLabel::new(label.text()),
            ))
        }
        ast::ContentItem::NonvocalBegin(label) => {
            UtteranceContent::NonvocalBegin(NonvocalBegin::new(
                super::locate(source, label.lexeme()),
                NonvocalLabel::new(label.text()),
            ))
        }
        ast::ContentItem::NonvocalEnd(label) => UtteranceContent::NonvocalEnd(NonvocalEnd::new(
            super::locate(source, label.lexeme()),
            NonvocalLabel::new(label.text()),
        )),
        ast::ContentItem::NonvocalSimple(label) => {
            UtteranceContent::NonvocalSimple(NonvocalSimple::new(
                super::locate(source, label.lexeme()),
                NonvocalLabel::new(label.text()),
            ))
        }
        ast::ContentItem::OtherSpokenEvent {
            speaker,
            text,
            lexeme,
        } => UtteranceContent::OtherSpokenEvent(OtherSpokenEvent::new(
            super::locate(source, lexeme),
            *speaker,
            *text,
        )),
        ast::ContentItem::Action { zero, annotations } => {
            let scoped = annotations_to_scoped(annotations);
            let location = super::locate(source, zero);
            // This backend had the same defect as the tree-sitter one: it
            // wrapped EVERY action, so a bare `0` became an annotated node
            // carrying nothing. It now asks the same question the three sites
            // above it were already asking.
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Action(Action::new(location)),
                Some(scoped) => UtteranceContent::AnnotatedAction(Annotated::new(
                    annotated_location(location, annotations, source),
                    Action::new(location),
                    scoped,
                )),
            }
        }
        ast::ContentItem::PhoGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|i| content_item_to_bracketed(i, source))
                .collect();
            UtteranceContent::PhoGroup(PhoGroup::new(BracketedContent::new(items)))
        }
        ast::ContentItem::SinGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|i| content_item_to_bracketed(i, source))
                .collect();
            UtteranceContent::SinGroup(SinGroup::new(BracketedContent::new(items)))
        }
    })
}

/// One conversion owner for bare/annotated quotations in either container.
/// Nonempty annotation admission happens once, before choosing the enclosing
/// content representation; bracketed conversion cannot silently discard it.
enum ModelQuotation {
    Bare(Quotation),
    Annotated(Annotated<Quotation>),
}

impl ModelQuotation {
    fn from_parsed(quotation: &ast::Quotation<'_>, source: SourceText<'_>) -> Self {
        let content = quotation
            .contents
            .iter()
            .filter_map(|item| content_item_to_bracketed(item, source))
            .collect();
        let quotation_location = super::located(quotation.enclosure.span_in(source));
        let model = Quotation::new(quotation_location, BracketedContent::new(content));
        match AnnotatedContentAnnotations::new(annotations_to_scoped(&quotation.annotations)) {
            None => Self::Bare(model),
            Some(annotations) => Self::Annotated(Annotated::new(
                annotated_location(quotation_location, &quotation.annotations, source),
                model,
                annotations,
            )),
        }
    }
}

/// Convert annotation tokens to model ContentAnnotation list.
pub(crate) fn annotations_to_scoped(
    annotations: &[ast::ParsedAnnotation<'_>],
) -> Vec<ContentAnnotation> {
    annotations
        .iter()
        .filter_map(ast::ParsedAnnotation::scoped)
        .map(scoped_annotation_to_model)
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
    let parsed_replacement = w
        .annotations
        .iter()
        .find_map(ast::ParsedAnnotation::replacement);

    if let Some(parsed_replacement) = parsed_replacement {
        let replacement = replacement_to_model(parsed_replacement, source);

        let scoped = annotations_to_scoped(&w.annotations);

        let replaced = ReplacedWord::new(
            replaced_word_location(&word, &w.annotations, source),
            word,
            replacement,
        )
        .with_scoped_annotations(scoped);
        UtteranceContent::ReplacedWord(Box::new(replaced))
    } else {
        let scoped = annotations_to_scoped(&w.annotations);
        match AnnotatedContentAnnotations::new(scoped) {
            None => UtteranceContent::Word(Box::new(word)),
            Some(scoped) => {
                // Read the span BEFORE the word moves into the wrapper.
                let placed = annotated_location(word.location, &w.annotations, source);
                let annotated = Annotated::new(placed, word, scoped);
                UtteranceContent::AnnotatedWord(Box::new(annotated))
            }
        }
    }
}

/// The extent of a retrace: its content (the group's brackets when the
/// retrace took them, otherwise the one item it wraps) through its marker.
/// `None` when any part cannot be placed, rather than a span that stops
/// short of the construct.
fn retrace_span(retrace: &ast::Retrace<'_>, source: SourceText<'_>) -> Option<Span> {
    let content = match &retrace.body {
        ast::RetraceBody::Enclosed(enclosure, _) => enclosure.span_in(source)?,
        ast::RetraceBody::Single(item) => content_item_span(item, source)?,
    };
    Some(content.merge(source.span_of(retrace.marker)?))
}

/// The extent of a content item that can stand inside a retrace: the item
/// itself through its last annotation. The kinds the chain fold never wraps
/// answer `None`, as does any part that cannot be placed.
fn content_item_span(item: &ast::ContentItem<'_>, source: SourceText<'_>) -> Option<Span> {
    match item {
        ast::ContentItem::Word(word) => {
            annotations_hull(super::word_span(word, source)?, &word.annotations, source)
        }
        ast::ContentItem::Event(event) => source.span_of(event.lexeme()),
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            annotations_hull(source.span_of(event.lexeme())?, annotations, source)
        }
        ast::ContentItem::Group(group) => {
            annotations_hull(group.enclosure.span_in(source)?, &group.annotations, source)
        }
        ast::ContentItem::Quotation(quotation) => annotations_hull(
            quotation.enclosure.span_in(source)?,
            &quotation.annotations,
            source,
        ),
        ast::ContentItem::Retrace(retrace) => {
            annotations_hull(retrace_span(retrace, source)?, &retrace.annotations, source)
        }
        // The chain fold never wraps these in a retrace, so a retrace holding
        // one is not a shape this backend produces; listed rather than caught
        // so a new item kind has to answer here (design rule 3).
        ast::ContentItem::Pause(_)
        | ast::ContentItem::Freecode(_)
        | ast::ContentItem::OrphanAnnotation(_)
        | ast::ContentItem::Separator { .. }
        | ast::ContentItem::OverlapPoint { .. }
        | ast::ContentItem::MediaBullet { .. }
        | ast::ContentItem::UnderlineBegin(_)
        | ast::ContentItem::UnderlineEnd(_)
        | ast::ContentItem::OtherSpokenEvent { .. }
        | ast::ContentItem::PhoGroup(_)
        | ast::ContentItem::SinGroup(_)
        | ast::ContentItem::LongFeatureBegin(_)
        | ast::ContentItem::LongFeatureEnd(_)
        | ast::ContentItem::NonvocalBegin(_)
        | ast::ContentItem::NonvocalEnd(_)
        | ast::ContentItem::NonvocalSimple(_) => None,
        ast::ContentItem::Action { zero, annotations } => {
            annotations_hull(source.span_of(zero)?, annotations, source)
        }
    }
}

/// `start` widened to cover every annotation; `None` if one cannot be placed.
fn annotations_hull(
    start: Span,
    annotations: &[ast::ParsedAnnotation<'_>],
    source: SourceText<'_>,
) -> Option<Span> {
    annotations.iter().try_fold(start, |hull, annotation| {
        Some(hull.merge(annotation.span_in(source)?))
    })
}

/// A replaced word runs from the word through its last annotation: the
/// replacement's closing `]` and any scoped annotation written after it, as
/// the model's wrapper and the tree-sitter node both measure it.
fn replaced_word_location(
    word: &Word,
    annotations: &[ast::ParsedAnnotation<'_>],
    source: SourceText<'_>,
) -> talkbank_model::Location {
    super::located(
        word.location
            .span()
            .and_then(|word| annotations_hull(word, annotations, source)),
    )
}

/// An orphan annotation's text for the freecode it becomes: its exact bytes
/// when it can be placed in `source`, else the spelling rebuilt from its kind.
fn orphan_text(annotation: &ast::ParsedAnnotation<'_>, source: SourceText<'_>) -> String {
    annotation
        .source_text(source)
        .map_or_else(|| annotation.chat_text(), str::to_owned)
}

/// The location of an `Annotated` wrapper: its construct through its last
/// annotation, or `Unlocated` when the construct or any annotation cannot be
/// placed in `source` (a programming error pairing an AST with the wrong
/// source), never a span that stops short of the construct.
fn annotated_location(
    location: talkbank_model::Location,
    annotations: &[ast::ParsedAnnotation<'_>],
    source: SourceText<'_>,
) -> talkbank_model::Location {
    super::located(
        location
            .span()
            .and_then(|start| annotations_hull(start, annotations, source)),
    )
}

/// Convert words already admitted by the grammar, retaining original source spans.
fn replacement_to_model(
    parsed: &ast::ReplacementParsed<'_>,
    source: SourceText<'_>,
) -> Replacement {
    let mut replacement = Replacement::from_word(word_from_parsed(parsed.first(), source));
    for word in parsed.rest() {
        replacement.words.push(word_from_parsed(word, source));
    }
    replacement
}

/// Convert a separator token to model Separator, at `span`.
///
/// `span` is `None` only when the caller paired the item with a source it did
/// not come from, which is a programming error rather than a property of the
/// input; the separator is then unlocated. Until the lexer's offsets reached
/// here, every separator was unlocated, so E258 and every other span-keyed
/// rule was silently unreachable under this backend.
pub(crate) fn separator_from_kind(kind: ast::SeparatorKindParsed, span: Option<Span>) -> Separator {
    let s = span.map_or(
        talkbank_model::Location::Unlocated,
        talkbank_model::Location::Located,
    );
    match kind {
        ast::SeparatorKindParsed::Comma => Separator::Comma { location: s },
        ast::SeparatorKindParsed::Semicolon => Separator::Semicolon { location: s },
        ast::SeparatorKindParsed::Colon => Separator::Colon { location: s },
        ast::SeparatorKindParsed::CaContinuation => Separator::CaContinuation { location: s },
        ast::SeparatorKindParsed::Tag => Separator::Tag { location: s },
        ast::SeparatorKindParsed::Vocative => Separator::Vocative { location: s },
        ast::SeparatorKindParsed::UnmarkedEnding => Separator::UnmarkedEnding { location: s },
        ast::SeparatorKindParsed::Uptake => Separator::Uptake { location: s },
        ast::SeparatorKindParsed::CaNoBreak => Separator::CaNoBreak { location: s },
        ast::SeparatorKindParsed::CaTechnicalBreak => Separator::CaTechnicalBreak { location: s },
        ast::SeparatorKindParsed::RisingToHigh => Separator::RisingToHigh { location: s },
        ast::SeparatorKindParsed::RisingToMid => Separator::RisingToMid { location: s },
        ast::SeparatorKindParsed::Level => Separator::Level { location: s },
        ast::SeparatorKindParsed::FallingToMid => Separator::FallingToMid { location: s },
        ast::SeparatorKindParsed::FallingToLow => Separator::FallingToLow { location: s },
    }
}

/// Lower the admitted kind and its lexer-owned extent together at both content levels.
fn pause_from_parsed(kind: &ast::PauseKindParsed<'_>) -> Pause {
    let (duration, lexeme) = match kind {
        ast::PauseKindParsed::Short(lexeme) => (PauseDuration::Short, lexeme),
        ast::PauseKindParsed::Medium(lexeme) => (PauseDuration::Medium, lexeme),
        ast::PauseKindParsed::Long(lexeme) => (PauseDuration::Long, lexeme),
        ast::PauseKindParsed::Timed(lexeme) => (
            PauseDuration::Timed(PauseTimedDuration::new(lexeme.text())),
            lexeme,
        ),
    };
    Pause::new(lexeme.span(), duration)
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
fn overlap_point(
    kind: ast::OverlapKind,
    index: Option<u32>,
    location: talkbank_model::Location,
) -> OverlapPoint {
    let model_kind = match kind {
        ast::OverlapKind::TopBegin => OverlapPointKind::TopOverlapBegin,
        ast::OverlapKind::TopEnd => OverlapPointKind::TopOverlapEnd,
        ast::OverlapKind::BottomBegin => OverlapPointKind::BottomOverlapBegin,
        ast::OverlapKind::BottomEnd => OverlapPointKind::BottomOverlapEnd,
    };
    OverlapPoint::new(location, model_kind, index.map(OverlapIndex::new))
}

/// Lower only lexer-admitted numeric values, placed at the bullet's own
/// bytes. Never reparse or substitute zero: a bullet whose numbers did not
/// admit is `None`, an occupied slot the caller leaves empty.
pub(crate) fn bullet_from_token(
    bullet: crate::token::MediaBulletToken<'_>,
    source: SourceText<'_>,
) -> Option<Bullet> {
    let (start_ms, end_ms) = bullet.times()?.pair();
    Some(Bullet::new(
        super::locate(source, bullet.text()),
        start_ms,
        end_ms,
    ))
}

/// Convert a content item to a BracketedItem (for inside groups/quotations/retraces).
/// Rejected numeric bullets contribute no model item; their lexer-owned
/// diagnostics are emitted before lowering. Other content remains exhaustive.
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
) -> Option<BracketedItem> {
    Some(match item {
        ast::ContentItem::Word(w) => {
            let word = word_from_parsed(w, source);
            let parsed_replacement = w
                .annotations
                .iter()
                .find_map(ast::ParsedAnnotation::replacement);

            if let Some(parsed_replacement) = parsed_replacement {
                let replacement = replacement_to_model(parsed_replacement, source);
                let scoped = annotations_to_scoped(&w.annotations);
                let replaced = ReplacedWord::new(
                    replaced_word_location(&word, &w.annotations, source),
                    word,
                    replacement,
                )
                .with_scoped_annotations(scoped);
                BracketedItem::ReplacedWord(Box::new(replaced))
            } else {
                let scoped = annotations_to_scoped(&w.annotations);
                match AnnotatedContentAnnotations::new(scoped) {
                    None => BracketedItem::Word(Box::new(word)),
                    Some(scoped) => {
                        // Read the span BEFORE the word moves into the wrapper.
                        let placed = annotated_location(word.location, &w.annotations, source);
                        BracketedItem::AnnotatedWord(Box::new(Annotated::new(placed, word, scoped)))
                    }
                }
            }
        }
        ast::ContentItem::Pause(kind) => BracketedItem::Pause(pause_from_parsed(kind)),
        ast::ContentItem::Event(event) => BracketedItem::Event(Event::new(
            super::locate(source, event.lexeme()),
            event.text(),
        )),
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            let event_location = super::locate(source, event.lexeme());
            let event_model = Event::new(event_location, event.text());
            let scoped = annotations_to_scoped(annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => BracketedItem::Event(event_model),
                Some(scoped) => BracketedItem::AnnotatedEvent(Annotated::new(
                    annotated_location(event_location, annotations, source),
                    event_model,
                    scoped,
                )),
            }
        }
        ast::ContentItem::Action { zero, annotations } => {
            let scoped = annotations_to_scoped(annotations);
            let location = super::locate(source, zero);
            match AnnotatedContentAnnotations::new(scoped) {
                None => BracketedItem::Action(Action::new(location)),
                Some(scoped) => BracketedItem::AnnotatedAction(Annotated::new(
                    annotated_location(location, annotations, source),
                    Action::new(location),
                    scoped,
                )),
            }
        }
        ast::ContentItem::OtherSpokenEvent {
            speaker,
            text,
            lexeme,
        } => BracketedItem::OtherSpokenEvent(OtherSpokenEvent::new(
            super::locate(source, lexeme),
            *speaker,
            *text,
        )),
        ast::ContentItem::Separator { kind, text } => {
            let sep = separator_from_kind(*kind, source.span_of(text));
            BracketedItem::Separator(sep)
        }
        ast::ContentItem::Group(g) => {
            let inner: Vec<BracketedItem> = g
                .contents
                .iter()
                .filter_map(|i| content_item_to_bracketed(i, source))
                .collect();
            let group_location = super::located(g.enclosure.span_in(source));
            let group = Group::new(group_location, BracketedContent::new(inner));
            // `BracketedItem` HAS a bare `Group` now. It did not until
            // 2026-08-26, which is why this used to hand an empty annotation
            // list to `AnnotatedGroup` and explain itself in a comment.
            let scoped = annotations_to_scoped(&g.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => BracketedItem::Group(group),
                Some(scoped) => BracketedItem::AnnotatedGroup(Annotated::new(
                    annotated_location(group_location, &g.annotations, source),
                    group,
                    scoped,
                )),
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
                .body
                .items()
                .iter()
                .filter_map(|i| content_item_to_bracketed(i, source))
                .collect();
            let retrace_location = super::located(retrace_span(r, source));
            let mut retrace = talkbank_model::model::Retrace::new(
                retrace_location,
                super::locate(source, r.marker),
                BracketedContent::new(inner),
                kind,
            );
            if r.is_group() {
                retrace = retrace.as_group();
            }
            // As at the tier-level site above: only the after-the-marker
            // annotations reach here.
            let scoped = annotations_to_scoped(&r.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => BracketedItem::Retrace(Box::new(retrace)),
                Some(scoped) => BracketedItem::AnnotatedRetrace(Box::new(
                    talkbank_model::model::Annotated::new(
                        annotated_location(retrace_location, &r.annotations, source),
                        retrace,
                        scoped,
                    ),
                )),
            }
        }
        ast::ContentItem::Freecode(text) => BracketedItem::Freecode(Freecode::new(
            super::locate(source, text.lexeme()),
            text.text(),
        )),
        // Preserved, matching the tier-level arm. This used to return `None`,
        // so the same orphaned annotation survived at tier level and vanished
        // inside a group.
        ast::ContentItem::OrphanAnnotation(annotation) => BracketedItem::Freecode(Freecode::new(
            super::located(annotation.span_in(source)),
            orphan_text(annotation, source),
        )),
        ast::ContentItem::OverlapPoint { kind, index, text } => {
            BracketedItem::OverlapPoint(overlap_point(*kind, *index, super::locate(source, text)))
        }
        ast::ContentItem::MediaBullet { bullet } => {
            BracketedItem::InternalBullet(bullet_from_token(*bullet, source)?)
        }
        ast::ContentItem::UnderlineBegin(text) => {
            BracketedItem::UnderlineBegin(UnderlineMarker::new(super::locate(source, text)))
        }
        ast::ContentItem::UnderlineEnd(text) => {
            BracketedItem::UnderlineEnd(UnderlineMarker::new(super::locate(source, text)))
        }
        ast::ContentItem::LongFeatureBegin(label) => {
            BracketedItem::LongFeatureBegin(LongFeatureBegin::new(
                super::locate(source, label.lexeme()),
                LongFeatureLabel::new(label.text()),
            ))
        }
        ast::ContentItem::LongFeatureEnd(label) => {
            BracketedItem::LongFeatureEnd(LongFeatureEnd::new(
                super::locate(source, label.lexeme()),
                LongFeatureLabel::new(label.text()),
            ))
        }
        ast::ContentItem::NonvocalBegin(label) => BracketedItem::NonvocalBegin(NonvocalBegin::new(
            super::locate(source, label.lexeme()),
            NonvocalLabel::new(label.text()),
        )),
        ast::ContentItem::NonvocalEnd(label) => BracketedItem::NonvocalEnd(NonvocalEnd::new(
            super::locate(source, label.lexeme()),
            NonvocalLabel::new(label.text()),
        )),
        ast::ContentItem::NonvocalSimple(label) => {
            BracketedItem::NonvocalSimple(NonvocalSimple::new(
                super::locate(source, label.lexeme()),
                NonvocalLabel::new(label.text()),
            ))
        }
        ast::ContentItem::PhoGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|i| content_item_to_bracketed(i, source))
                .collect();
            BracketedItem::PhoGroup(PhoGroup::new(BracketedContent::new(items)))
        }
        ast::ContentItem::SinGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|i| content_item_to_bracketed(i, source))
                .collect();
            BracketedItem::SinGroup(SinGroup::new(BracketedContent::new(items)))
        }
        ast::ContentItem::Quotation(q) => match ModelQuotation::from_parsed(q, source) {
            ModelQuotation::Bare(quotation) => BracketedItem::Quotation(quotation),
            ModelQuotation::Annotated(quotation) => BracketedItem::AnnotatedQuotation(quotation),
        },
    })
}

/// Every scoped annotation has a model representation. Structural markers
/// cannot enter this conversion; the producer classified them separately.
pub(crate) fn scoped_annotation_to_model(
    ann: &ast::ScopedAnnotationParsed<'_>,
) -> ContentAnnotation {
    match ann {
        // `[@ xyz]` carries `"@ xyz"`. The MARKER is the leading run of
        // non-space characters and the TEXT is whatever follows it, matching
        // `ScopedUnknown`'s two fields; a marker with no text (`[@@@]`) keeps
        // an empty text rather than borrowing the marker into it.
        ast::ScopedAnnotationParsed::Unknown(inner) => {
            let inner = inner.text();
            let (marker, text) = match inner.split_once(' ') {
                Some((marker, text)) => (marker, text.trim()),
                None => (inner, ""),
            };
            ContentAnnotation::Unknown(talkbank_model::model::ScopedUnknown {
                marker: marker.into(),
                text: text.into(),
            })
        }
        ast::ScopedAnnotationParsed::Stressing(_) => ContentAnnotation::Stressing,
        ast::ScopedAnnotationParsed::ContrastiveStressing(_) => {
            ContentAnnotation::ContrastiveStressing
        }
        ast::ScopedAnnotationParsed::Uncertain(_) => ContentAnnotation::Uncertain,
        ast::ScopedAnnotationParsed::Exclude(_) => ContentAnnotation::Exclude,
        ast::ScopedAnnotationParsed::CodeSwitchShortcut(_) => {
            ContentAnnotation::CodeSwitch(talkbank_model::model::CodeSwitchSpan::Shortcut)
        }
        ast::ScopedAnnotationParsed::CodeSwitchExplicit(code) => {
            ContentAnnotation::CodeSwitch(talkbank_model::model::CodeSwitchSpan::Explicit(
                // Same justification as the word-level `@s:` suffix above: the
                // lexer's `[a-z]{2,4}` matches `language_code` exactly, so this
                // is defensive only.
                talkbank_model::LanguageCode::new(code.text())
                    .expect("lexer-guaranteed language_code shape"),
            ))
        }
        ast::ScopedAnnotationParsed::Error(s) => {
            let code = if s.text().is_empty() {
                None
            } else {
                Some(s.text().into())
            };
            ContentAnnotation::Error(ScopedError { code })
        }
        ast::ScopedAnnotationParsed::OverlapPrecedes(index) => {
            ContentAnnotation::OverlapBegin(ScopedOverlapBegin {
                index: index.index(),
            })
        }
        ast::ScopedAnnotationParsed::OverlapFollows(index) => {
            ContentAnnotation::OverlapEnd(ScopedOverlapEnd {
                index: index.index(),
            })
        }
        ast::ScopedAnnotationParsed::Explanation(s) => {
            ContentAnnotation::Explanation(ScopedExplanation {
                text: s.text().into(),
            })
        }
        ast::ScopedAnnotationParsed::Paralinguistic(s) => {
            ContentAnnotation::Paralinguistic(ScopedParalinguistic {
                text: s.text().into(),
            })
        }
        ast::ScopedAnnotationParsed::Alternative(s) => {
            ContentAnnotation::Alternative(ScopedAlternative {
                text: s.text().into(),
            })
        }
        ast::ScopedAnnotationParsed::PercentComment(s) => {
            ContentAnnotation::PercentComment(ScopedPercentComment {
                text: s.text().into(),
            })
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
pub fn token_to_terminator(tok: &Token<'_>, source: SourceText<'_>) -> Option<Terminator> {
    ast::TerminatorKindParsed::from_token(tok)
        .map(|kind| terminator_from_kind(kind, super::locate(source, tok.text())))
}

/// A resolved terminator kind as the model's `Terminator`.
///
/// Exhaustive: the kind arrives resolved, so there is no unmatched token to
/// answer for. This replaced `_ => Terminator::Period`, which invented
/// sentence-final punctuation out of whatever arrived.
fn terminator_from_kind(
    kind: ast::TerminatorKindParsed,
    s: talkbank_model::Location,
) -> Terminator {
    match kind {
        ast::TerminatorKindParsed::Period => Terminator::Period { location: s },
        ast::TerminatorKindParsed::Question => Terminator::Question { location: s },
        ast::TerminatorKindParsed::Exclamation => Terminator::Exclamation { location: s },
        ast::TerminatorKindParsed::TrailingOff => Terminator::TrailingOff { location: s },
        ast::TerminatorKindParsed::Interruption => Terminator::Interruption { location: s },
        ast::TerminatorKindParsed::SelfInterruption => Terminator::SelfInterruption { location: s },
        ast::TerminatorKindParsed::InterruptedQuestion => {
            Terminator::InterruptedQuestion { location: s }
        }
        ast::TerminatorKindParsed::BrokenQuestion => Terminator::BrokenQuestion { location: s },
        ast::TerminatorKindParsed::QuotedNewLine => Terminator::QuotedNewLine { location: s },
        ast::TerminatorKindParsed::QuotedPeriodSimple => {
            Terminator::QuotedPeriodSimple { location: s }
        }
        ast::TerminatorKindParsed::SelfInterruptedQuestion => {
            Terminator::SelfInterruptedQuestion { location: s }
        }
        ast::TerminatorKindParsed::TrailingOffQuestion => {
            Terminator::TrailingOffQuestion { location: s }
        }
        ast::TerminatorKindParsed::BreakForCoding => Terminator::BreakForCoding { location: s },
    }
}

// ═══════════════════════════════════════════════════════════════
// MainTier conversion
// ═══════════════════════════════════════════════════════════════
