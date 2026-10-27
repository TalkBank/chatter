//! Shared lowering for the content admitted to a marker chain.

use super::*;

pub(super) fn content_ref_to_bracketed(
    item: ast::ContentRef<'_, '_>,
    source: SourceText<'_>,
) -> BracketedItem {
    match item {
        ast::ContentRef::Item(item) => content_item_to_bracketed(item, source),
        ast::ContentRef::Chain(item) => match item {
            ast::Chain::Word(w) => word(w),
            ast::Chain::Event(e, annotations) => event(e, annotations),
            ast::Chain::Action(zero, annotations) => action(zero, annotations, source),
            ast::Chain::Group(enclosure, contents, annotations) => {
                group(*enclosure, contents, annotations, source)
            }
            ast::Chain::Quotation(q) => match ModelQuotation::from_parsed(q, source) {
                ModelQuotation::Bare(q) => BracketedItem::Quotation(q),
                ModelQuotation::Annotated(q) => BracketedItem::AnnotatedQuotation(q),
            },
            ast::Chain::Wrapped(r) => retrace(r, source),
        },
    }
}

pub(super) fn word(w: &ast::WordWithAnnotations<'_>) -> BracketedItem {
    let source = w.source().source();
    let word = word_from_parsed(w);
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
        BracketedItem::ReplacedWord(Box::new(replaced))
    } else {
        let scoped = annotations_to_scoped(&w.annotations);
        match AnnotatedContentAnnotations::new(scoped) {
            None => BracketedItem::Word(Box::new(word)),
            Some(scoped) => {
                // Read the span BEFORE the word moves into the wrapper.
                let placed = annotated_location(word.location, &w.annotations);
                BracketedItem::AnnotatedWord(Box::new(Annotated::new(placed, word, scoped)))
            }
        }
    }
}

pub(super) fn event(
    event: &Event,
    annotations: &[ast::ScopedAnnotationParsed<'_>],
) -> BracketedItem {
    let event_location = event.location;
    let event_model = event.clone();
    let scoped = annotations_to_scoped(annotations);
    match AnnotatedContentAnnotations::new(scoped) {
        None => BracketedItem::Event(event_model),
        Some(scoped) => BracketedItem::AnnotatedEvent(Annotated::new(
            annotated_location(event_location, annotations),
            event_model,
            scoped,
        )),
    }
}

pub(super) fn action(
    zero: &str,
    annotations: &[ast::ScopedAnnotationParsed<'_>],
    source: SourceText<'_>,
) -> BracketedItem {
    let scoped = annotations_to_scoped(annotations);
    let location = crate::convert::locate(source, zero);
    match AnnotatedContentAnnotations::new(scoped) {
        None => BracketedItem::Action(Action::new(location)),
        Some(scoped) => BracketedItem::AnnotatedAction(Annotated::new(
            annotated_location(location, annotations),
            Action::new(location),
            scoped,
        )),
    }
}

pub(super) fn group(
    enclosure: ast::Enclosure<'_>,
    contents: &[ast::ContentItem<'_>],
    annotations: &[ast::ScopedAnnotationParsed<'_>],
    source: SourceText<'_>,
) -> BracketedItem {
    let inner: Vec<BracketedItem> = contents
        .iter()
        .map(|i| content_item_to_bracketed(i, source))
        .collect();
    let group_location = crate::convert::located(enclosure.span_in(source));
    let group = Group::new(group_location, BracketedContent::new(inner));
    // `BracketedItem` HAS a bare `Group` now. It did not until
    // 2026-08-26, which is why this used to hand an empty annotation
    // list to `AnnotatedGroup` and explain itself in a comment.
    let scoped = annotations_to_scoped(annotations);
    match AnnotatedContentAnnotations::new(scoped) {
        None => BracketedItem::Group(group),
        Some(scoped) => BracketedItem::AnnotatedGroup(Annotated::new(
            annotated_location(group_location, annotations),
            group,
            scoped,
        )),
    }
}

pub(super) fn retrace(r: &ast::Retrace<'_>, source: SourceText<'_>) -> BracketedItem {
    let kind = match r.kind {
        crate::ast::RetraceKindParsed::Partial => RetraceKind::Partial,
        crate::ast::RetraceKindParsed::Complete => RetraceKind::Full,
        crate::ast::RetraceKindParsed::Multiple => RetraceKind::Multiple,
        crate::ast::RetraceKindParsed::Reformulation => RetraceKind::Reformulation,
    };
    let inner: Vec<BracketedItem> = r
        .body
        .items()
        .map(|i| content_ref_to_bracketed(i, source))
        .collect();
    let retrace_location = crate::convert::located(retrace_span(r, source));
    let mut retrace = talkbank_model::model::Retrace::new(
        retrace_location,
        crate::convert::locate(source, r.marker),
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
        Some(scoped) => {
            BracketedItem::AnnotatedRetrace(Box::new(talkbank_model::model::Annotated::new(
                annotated_location(retrace_location, &r.annotations),
                retrace,
                scoped,
            )))
        }
    }
}
