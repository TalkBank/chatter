//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use super::words::word_from_parsed;

mod chain;
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
pub(crate) fn content_item_to_model(
    item: &ast::ContentItem<'_>,
    source: SourceText<'_>,
) -> UtteranceContent {
    match item {
        ast::ContentItem::Word(w) => word_with_annotations_to_model(w),
        ast::ContentItem::Pause(pause) => UtteranceContent::Pause(pause.clone()),
        ast::ContentItem::Event(event, annotations) => {
            let event_location = event.location;
            let event_model = event.clone();
            let scoped = annotations_to_scoped(annotations);
            // The constructor's `Option` IS the bare-versus-annotated
            // question, so it replaces the `is_empty` check that used to ask it
            // separately.
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Event(event_model),
                Some(scoped) => UtteranceContent::AnnotatedEvent(Annotated::new(
                    annotated_location(event_location, annotations),
                    event_model,
                    scoped,
                )),
            }
        }
        ast::ContentItem::Separator(separator) => UtteranceContent::Separator(separator.clone()),
        ast::ContentItem::Freecode(text) => UtteranceContent::Freecode(Freecode::from_lexed(
            super::locate(source, text.lexeme()),
            text.text(),
            text.lexeme(),
        )),
        // Preserve rejected spelling and invalidity without inventing a freecode.
        ast::ContentItem::OrphanAnnotation(annotation) => {
            UtteranceContent::OrphanAnnotation(OrphanAnnotation::new(
                super::located(annotation.span_in(source)),
                annotation.chat_text(),
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
                .map(|i| chain::content_ref_to_bracketed(i, source))
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
                        annotated_location(retrace_location, &r.annotations),
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
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            let group_location = super::located(g.enclosure.span_in(source));
            let group = Group::new(group_location, BracketedContent::new(content));
            let scoped = annotations_to_scoped(&g.annotations);
            match AnnotatedContentAnnotations::new(scoped) {
                None => UtteranceContent::Group(group),
                Some(scoped) => UtteranceContent::AnnotatedGroup(Annotated::new(
                    annotated_location(group_location, &g.annotations),
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
            UtteranceContent::InternalBullet(bullet_from_token(*bullet, source))
        }
        ast::ContentItem::UnderlineBegin(text) => {
            UtteranceContent::UnderlineBegin(UnderlineMarker::new(super::locate(source, text)))
        }
        ast::ContentItem::UnderlineEnd(text) => {
            UtteranceContent::UnderlineEnd(UnderlineMarker::new(super::locate(source, text)))
        }
        ast::ContentItem::LongFeatureBegin(marker) => {
            UtteranceContent::LongFeatureBegin(marker.clone())
        }
        ast::ContentItem::LongFeatureEnd(marker) => {
            UtteranceContent::LongFeatureEnd(marker.clone())
        }
        ast::ContentItem::NonvocalBegin(marker) => UtteranceContent::NonvocalBegin(marker.clone()),
        ast::ContentItem::NonvocalEnd(marker) => UtteranceContent::NonvocalEnd(marker.clone()),
        ast::ContentItem::NonvocalSimple(marker) => {
            UtteranceContent::NonvocalSimple(marker.clone())
        }

        ast::ContentItem::OtherSpokenEvent(event) => {
            UtteranceContent::OtherSpokenEvent(event.clone())
        }
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
                    annotated_location(location, annotations),
                    Action::new(location),
                    scoped,
                )),
            }
        }
        ast::ContentItem::PhoGroup(group) => {
            let items: Vec<BracketedItem> = group
                .contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            UtteranceContent::PhoGroup(PhoGroup::new(
                super::located(group.enclosure.span_in(source)),
                BracketedContent::new(items),
            ))
        }
        ast::ContentItem::SinGroup(group) => {
            let items: Vec<BracketedItem> = group
                .contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            UtteranceContent::SinGroup(SinGroup::new(
                super::located(group.enclosure.span_in(source)),
                BracketedContent::new(items),
            ))
        }
    }
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
            .map(|item| content_item_to_bracketed(item, source))
            .collect();
        let quotation_location = super::located(quotation.enclosure.span_in(source));
        let model = Quotation::new(quotation_location, BracketedContent::new(content));
        match AnnotatedContentAnnotations::new(annotations_to_scoped(&quotation.annotations)) {
            None => Self::Bare(model),
            Some(annotations) => Self::Annotated(Annotated::new(
                annotated_location(quotation_location, &quotation.annotations),
                model,
                annotations,
            )),
        }
    }
}

/// Convert annotation tokens to model ContentAnnotation list.
pub(crate) fn annotations_to_scoped(
    annotations: &[ast::ScopedAnnotationParsed<'_>],
) -> Vec<ContentAnnotation> {
    annotations
        .iter()
        .map(|scoped| scoped.annotation().clone())
        .collect()
}

/// Convert a word with annotations to the appropriate UtteranceContent variant.
/// - No annotations → Word
/// - Has [: replacement] → ReplacedWord (with any other annotations as scoped)
/// - Has other annotations → AnnotatedWord
pub(crate) fn word_with_annotations_to_model(w: &ast::WordWithAnnotations<'_>) -> UtteranceContent {
    let source = w.source().source();
    let word = word_from_parsed(w);

    // Check if there's a replacement annotation
    let parsed_replacement = w.replacement();

    if let Some(parsed_replacement) = parsed_replacement {
        let replacement = replacement_to_model(parsed_replacement);

        let scoped = annotations_to_scoped(&w.annotations);

        let replaced = ReplacedWord::new(
            replaced_word_location(&word, parsed_replacement, &w.annotations, source),
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
                let placed = annotated_location(word.location, &w.annotations);
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
        ast::RetraceBody::Single(item) => chain_span(item, source)?,
    };
    Some(content.merge(source.span_of(retrace.marker)?))
}

/// The extent of an admitted chain through its annotations. Failure means
/// source provenance could not place a part, never an unsupported item kind.
fn chain_span(item: &ast::Chain<'_>, source: SourceText<'_>) -> Option<Span> {
    Some(match item {
        ast::Chain::Word(word) => {
            let start = match word.replacement() {
                Some(replacement) => word.source().span().merge(replacement.span_in(source)?),
                None => word.source().span(),
            };
            annotations_hull(start, &word.annotations)
        }
        ast::Chain::Event(event, annotations) => {
            annotations_hull(event.location.span()?, annotations)
        }
        ast::Chain::Group(enclosure, _, annotations) => {
            annotations_hull(enclosure.span_in(source)?, annotations)
        }
        ast::Chain::Quotation(quotation) => {
            annotations_hull(quotation.enclosure.span_in(source)?, &quotation.annotations)
        }
        ast::Chain::Wrapped(retrace) => {
            annotations_hull(retrace_span(retrace, source)?, &retrace.annotations)
        }
        ast::Chain::Action(zero, annotations) => {
            annotations_hull(source.span_of(zero)?, annotations)
        }
    })
}

/// `start` widened through every annotation's admitted source span.
fn annotations_hull(start: Span, annotations: &[ast::ScopedAnnotationParsed<'_>]) -> Span {
    annotations
        .iter()
        .fold(start, |hull, annotation| hull.merge(annotation.span()))
}

/// A replaced word runs from the word through its last annotation: the
/// replacement's closing `]` and any scoped annotation written after it, as
/// the model's wrapper and the tree-sitter node both measure it.
fn replaced_word_location(
    word: &Word,
    replacement: &ast::ReplacementParsed<'_>,
    annotations: &[ast::ScopedAnnotationParsed<'_>],
    source: SourceText<'_>,
) -> talkbank_model::Location {
    super::located(word.location.span().and_then(|word| {
        Some(annotations_hull(
            word.merge(replacement.span_in(source)?),
            annotations,
        ))
    }))
}

/// The location of an `Annotated` wrapper: its construct through its last
/// annotation. An unlocated base remains unlocated; scoped annotations already
/// carry their source positions and cannot lose them during lowering.
fn annotated_location(
    location: talkbank_model::Location,
    annotations: &[ast::ScopedAnnotationParsed<'_>],
) -> talkbank_model::Location {
    super::located(
        location
            .span()
            .map(|start| annotations_hull(start, annotations)),
    )
}

/// Convert words already admitted by the grammar, retaining original source spans.
fn replacement_to_model(parsed: &ast::ReplacementParsed<'_>) -> Replacement {
    let mut replacement = Replacement::from_word(word_from_parsed(parsed.first()));
    for word in parsed.rest() {
        replacement.words.push(word_from_parsed(word));
    }
    replacement
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
    kind: OverlapPointKind,
    index: Option<u32>,
    location: talkbank_model::Location,
) -> OverlapPoint {
    OverlapPoint::new(location, kind, index.map(OverlapIndex::new))
}

/// Retain every observed bullet as parsed timing or explicit rejected source.
pub(crate) fn bullet_from_token(
    bullet: crate::token::MediaBulletToken<'_>,
    source: SourceText<'_>,
) -> BulletState {
    let location = super::locate(source, bullet.text());
    match bullet.times() {
        Some(times) => {
            let (start_ms, end_ms) = times.pair();
            Bullet::new(location, start_ms, end_ms).into()
        }
        None => UnparsedBullet::new(location, bullet.text()).into(),
    }
}

/// Convert a content item to a BracketedItem (for inside groups/quotations/retraces).
/// Every item has a model representation, including rejected numeric bullets.
/// Lexer diagnostics do not authorize source omission.
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
        ast::ContentItem::Word(w) => chain::word(w),
        ast::ContentItem::Pause(pause) => BracketedItem::Pause(pause.clone()),
        ast::ContentItem::Event(event, annotations) => chain::event(event, annotations),
        ast::ContentItem::Action { zero, annotations } => chain::action(zero, annotations, source),
        ast::ContentItem::OtherSpokenEvent(event) => BracketedItem::OtherSpokenEvent(event.clone()),
        ast::ContentItem::Separator(separator) => BracketedItem::Separator(separator.clone()),
        ast::ContentItem::Group(g) => {
            chain::group(g.enclosure, &g.contents, &g.annotations, source)
        }
        ast::ContentItem::Retrace(r) => chain::retrace(r, source),
        ast::ContentItem::Freecode(text) => BracketedItem::Freecode(Freecode::from_lexed(
            super::locate(source, text.lexeme()),
            text.text(),
            text.lexeme(),
        )),
        // Preserved, matching the tier-level arm. This used to return `None`,
        // so the same orphaned annotation survived at tier level and vanished
        // inside a group.
        ast::ContentItem::OrphanAnnotation(annotation) => {
            BracketedItem::OrphanAnnotation(OrphanAnnotation::new(
                super::located(annotation.span_in(source)),
                annotation.chat_text(),
            ))
        }
        ast::ContentItem::OverlapPoint { kind, index, text } => {
            BracketedItem::OverlapPoint(overlap_point(*kind, *index, super::locate(source, text)))
        }
        ast::ContentItem::MediaBullet { bullet } => {
            BracketedItem::InternalBullet(bullet_from_token(*bullet, source))
        }
        ast::ContentItem::UnderlineBegin(text) => {
            BracketedItem::UnderlineBegin(UnderlineMarker::new(super::locate(source, text)))
        }
        ast::ContentItem::UnderlineEnd(text) => {
            BracketedItem::UnderlineEnd(UnderlineMarker::new(super::locate(source, text)))
        }
        ast::ContentItem::LongFeatureBegin(marker) => {
            BracketedItem::LongFeatureBegin(marker.clone())
        }
        ast::ContentItem::LongFeatureEnd(marker) => BracketedItem::LongFeatureEnd(marker.clone()),
        ast::ContentItem::NonvocalBegin(marker) => BracketedItem::NonvocalBegin(marker.clone()),
        ast::ContentItem::NonvocalEnd(marker) => BracketedItem::NonvocalEnd(marker.clone()),
        ast::ContentItem::NonvocalSimple(marker) => BracketedItem::NonvocalSimple(marker.clone()),

        ast::ContentItem::PhoGroup(group) => {
            let items: Vec<BracketedItem> = group
                .contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            BracketedItem::PhoGroup(PhoGroup::new(
                super::located(group.enclosure.span_in(source)),
                BracketedContent::new(items),
            ))
        }
        ast::ContentItem::SinGroup(group) => {
            let items: Vec<BracketedItem> = group
                .contents
                .iter()
                .map(|i| content_item_to_bracketed(i, source))
                .collect();
            BracketedItem::SinGroup(SinGroup::new(
                super::located(group.enclosure.span_in(source)),
                BracketedContent::new(items),
            ))
        }
        ast::ContentItem::Quotation(q) => match ModelQuotation::from_parsed(q, source) {
            ModelQuotation::Bare(quotation) => BracketedItem::Quotation(quotation),
            ModelQuotation::Annotated(quotation) => BracketedItem::AnnotatedQuotation(quotation),
        },
    }
}

// ═══════════════════════════════════════════════════════════════
// Terminator conversion
// ═══════════════════════════════════════════════════════════════

/// Convert a terminator token to a model `Terminator`.
///
/// `None` for a non-terminator or a token outside the supplied source.
/// No unlocated model value is minted on failed source binding. It used to fall back to
/// `Terminator::Period`, which fabricated a sentence-final period out of
/// whatever arrived: the same silent-wrong-answer shape as the pause and
/// separator defaults, and here it invents PUNCTUATION that changes what the
/// utterance means. An absent terminator is a condition the model can state
/// and the validators already report.
pub(crate) fn token_to_terminator(tok: &Token<'_>, source: SourceText<'_>) -> Option<Terminator> {
    let construct = crate::parser::classify::terminator_constructor(tok.into())?;
    let span = source.span_of(tok.text())?;
    Some(construct(span))
}

// ═══════════════════════════════════════════════════════════════
// MainTier conversion
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    #[test]
    fn terminator_conversion_refuses_foreign_source_instead_of_unlocated() {
        use crate::{convert::token_to_terminator, source_text::SourceText, token::Token};
        let source = String::from(".");
        let other = String::from(".");
        let token = Token::Period(&other);
        assert!(token_to_terminator(&token, SourceText::new(&source)).is_none());
        let admitted = token_to_terminator(&token, SourceText::new(&other)).unwrap();
        assert_eq!(
            admitted.location().span(),
            Some(talkbank_model::Span::new(0, 1))
        );
    }
}
