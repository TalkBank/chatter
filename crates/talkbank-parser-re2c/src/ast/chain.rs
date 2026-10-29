//! Content admitted to ordered annotation and retrace marker chains.

use super::*;

/// What a marker fold has built so far.
///
/// One accumulator for the seeds a marker run can start from (a word, an
/// event, a bracketed group or quotation), rather than folding over `ContentItem`, whose
/// other variants a marker run can never reach. That keeps `annotated_with`
/// total with no catch-all.
///
/// Shared by the fold sites in this crate. They were separate
/// algorithms: the word path folded, the event path partitioned markers out
/// and so lost the annotation/marker interleaving, and the group path still
/// split at the FIRST marker and dropped the rest, which is the exact bug the
/// word path had been fixed for. Covered by `equivalence_marker_chain`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum Chain<'a> {
    /// A word, with whatever annotations have attached so far.
    Word(WordWithAnnotations<'a>),
    /// An event, with its annotations.
    Event(super::ParsedEvent<'a>, Vec<ScopedAnnotationParsed<'a>>),
    /// A standalone `0` (an action), with its annotations. It joins the fold
    /// so a retrace marker after it wraps it, as the tree-sitter backend
    /// does; until 2026-10-19 the action kept its whole run as annotations
    /// and the lowering, which reads only scoped ones, dropped the marker.
    Action(super::ZeroAction<'a>, Vec<ScopedAnnotationParsed<'a>>),
    /// A bracketed group (its brackets, its contents), with its annotations.
    Group(
        Enclosure<'a>,
        Vec<ContentItem<'a>>,
        Vec<ScopedAnnotationParsed<'a>>,
    ),
    /// A quotation, with its annotations.
    Quotation(Quotation<'a>),
    /// At least one retrace marker has wrapped the chain.
    Wrapped(Retrace<'a>),
}

impl<'a> Chain<'a> {
    /// Fold an ordered marker run onto this seed, one wrapper per marker.
    pub(crate) fn fold(self, markers: impl IntoIterator<Item = BaseAnnotation<'a>>) -> Self {
        markers
            .into_iter()
            .fold(self, |chain, marker| match marker {
                BaseAnnotation::Retrace(kind, lexeme) => chain.retraced(kind, lexeme),
                BaseAnnotation::Scoped(scoped) => chain.annotated_with(scoped),
            })
    }

    /// Attach a non-retrace annotation to whatever the chain currently is.
    fn annotated_with(self, annotation: ScopedAnnotationParsed<'a>) -> Self {
        match self {
            Chain::Word(mut word) => {
                word.annotations.push(annotation);
                Chain::Word(word)
            }
            Chain::Event(event, mut annotations) => {
                annotations.push(annotation);
                Chain::Event(event, annotations)
            }
            Chain::Action(zero, mut annotations) => {
                annotations.push(annotation);
                Chain::Action(zero, annotations)
            }
            Chain::Group(enclosure, contents, mut annotations) => {
                annotations.push(annotation);
                Chain::Group(enclosure, contents, annotations)
            }
            Chain::Quotation(mut quotation) => {
                quotation.annotations.push(annotation);
                Chain::Quotation(quotation)
            }
            Chain::Wrapped(mut retrace) => {
                retrace.annotations.push(annotation);
                Chain::Wrapped(retrace)
            }
        }
    }

    /// Wrap the chain in a retrace whose marker was written as `marker`.
    fn retraced(self, kind: RetraceKindParsed, marker: super::RetraceMarker<'a>) -> Self {
        // A group with no annotations of its own hands its brackets to the
        // retrace, which is what `enclosure` records. Nesting it instead would
        // serialize `<<a b>> [/]`, a bracket pair nobody wrote.
        //
        // SHARED SEMANTICS, written twice. `talkbank-parser`'s
        // `content::marker_chain::retrace` decides the same thing on the model
        // type, so no constructor can own it for both. Drift here is silent and
        // corrupts output, so the enforcement is the cross-parser test
        // `equivalence_marker_chain`; change one of these two and run it.
        if let Chain::Group(enclosure, contents, annotations) = self {
            if annotations.is_empty() {
                return Chain::Wrapped(Retrace {
                    body: RetraceBody::Enclosed(enclosure, contents),
                    kind,
                    marker,
                    annotations: Vec::new(),
                });
            }
            return Chain::Wrapped(Retrace {
                body: RetraceBody::Single(Box::new(Chain::Group(enclosure, contents, annotations))),
                kind,
                marker,
                annotations: Vec::new(),
            });
        }
        Chain::Wrapped(Retrace {
            body: RetraceBody::Single(Box::new(self)),
            kind,
            marker,
            annotations: Vec::new(),
        })
    }

    pub(crate) fn into_content_item(self) -> ContentItem<'a> {
        match self {
            Chain::Word(word) => ContentItem::Word(word),
            Chain::Event(event, annotations) => ContentItem::Event(event, annotations),
            Chain::Action(zero, annotations) => ContentItem::Action { zero, annotations },
            Chain::Group(enclosure, contents, annotations) => ContentItem::Group(Group {
                enclosure,
                contents,
                annotations,
            }),
            Chain::Quotation(quotation) => ContentItem::Quotation(quotation),
            Chain::Wrapped(retrace) => ContentItem::Retrace(retrace),
        }
    }
}

/// A marker admitted by `base_annotations`, not a word replacement or a
/// tier-level code. Keep rejected tokens in the stream for normal recovery.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BaseAnnotation<'a> {
    Scoped(ScopedAnnotationParsed<'a>),
    Retrace(RetraceKindParsed, super::RetraceMarker<'a>),
}

impl<'a> BaseAnnotation<'a> {
    pub(crate) fn admit(annotation: ParsedAnnotation<'a>) -> Option<Self> {
        match annotation {
            ParsedAnnotation::Replacement(_)
            | ParsedAnnotation::Langcode(_)
            | ParsedAnnotation::Postcode(_) => None,
            ParsedAnnotation::Scoped(scoped) => match scoped.annotation() {
                talkbank_model::ContentAnnotation::Unknown(_) => None,
                _ => Some(Self::Scoped(scoped)),
            },
            ParsedAnnotation::Retrace(kind, marker) => Some(Self::Retrace(kind, marker)),
        }
    }
}
