//! The verbs a header's typed content slot is read through, and the
//! `Header::Unknown` recovery they build: one owner for what the simple and
//! special header families, the pre-`@Begin` headers and `@Media` had each
//! written for themselves.

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, NamedKind, Never, NoChild, ReadableSlot, SourceBindingError, SourceBound,
    SourceBoundKind, SourceField, SourceRecovery, SourceSlotView,
};
use crate::model::{Header, WarningText};
use crate::parser::typed_cst::AnyKindSlot;
use crate::parser::typed_cst::CstFailure;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Build `Header::Unknown` from malformed header input: the header's own text
/// with the reason the parser gave up. Unreadable text is a producer failure,
/// not a header whose text is the grammar node's name.
/// Shared by the simple and special header families, the pre-`@Begin`
/// headers and `@Media`, each of which had written its own copy.
pub(crate) fn unknown_header_from_node(
    header_actual: Node,
    input: &str,
    reason: impl Into<String>,
    suggested_fix: Option<&str>,
) -> Result<Header, SourceBindingError> {
    let text = input
        .get(header_actual.byte_range())
        .ok_or(SourceBindingError::InvalidRange)?;
    Ok(unknown_header_from_text(text, reason, suggested_fix))
}

fn unknown_header_from_text(
    text: &str,
    reason: impl Into<String>,
    suggested_fix: Option<&str>,
) -> Header {
    Header::Unknown {
        text: WarningText::new(text.to_owned()),
        parse_reason: Some(reason.into()),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}

#[cfg(test)]
mod unknown_header_admission_tests {
    use super::*;
    use crate::generated_traversal::{FromNodeKind, LanguagesHeaderNode};

    #[test]
    fn unknown_header_retains_admitted_text_and_refuses_unreadable_source() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/tiers/mor-gra.cha"
        ));
        let parser = crate::TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("reference");
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(header) = LanguagesHeaderNode::from_node(node) else {
                continue;
            };
            let site = HeaderSite::bound(parsed.bind_typed(header).expect("owner"));
            let observed =
                unknown_header_from_node(node, source, "boundary control", None).expect("readable");
            assert_eq!(site.unknown("boundary control", None), observed);
            let Header::Unknown { text, .. } = observed else {
                panic!("unknown header")
            };
            assert_eq!(text.to_string(), &source[node.byte_range()]);
            assert!(matches!(
                unknown_header_from_node(node, "", "boundary control", None),
                Err(SourceBindingError::InvalidRange)
            ));
            witnessed += 1;
        }
        assert!(witnessed > 0);
    }
}

/// A header node under parse, carried once: the node every recovery
/// diagnostic and every `Header::Unknown` points at, the kind the generator
/// named it, and the document text. Built from the typed node, so the node
/// kind and source are one admitted value and cannot disagree. Until 2026-09-09 every
/// verb took them as three separate arguments beside the errors sink, which
/// put two of the verbs past clippy's argument limit and let a caller pair a
/// node with another header's kind.
pub(crate) struct HeaderSite<'tree, 'src> {
    actual: Node<'tree>,
    kind: &'static str,
    input: &'src str,
    text: &'src str,
}

impl<'tree, 'src> HeaderSite<'tree, 'src> {
    /// Retain a producer-bound header's own node, kind and source together.
    pub(crate) fn bound<T: SourceBoundKind<'tree> + NamedKind>(
        typed: SourceBound<'tree, 'src, T>,
    ) -> Self {
        Self {
            actual: typed.raw_node(),
            kind: T::KIND,
            input: typed.source(),
            text: typed.text(),
        }
    }

    /// The header node itself.
    pub(crate) fn actual(&self) -> Node<'tree> {
        self.actual
    }

    /// The header node type's own `KIND`.
    pub(crate) fn kind(&self) -> &'static str {
        self.kind
    }

    /// The document text.
    pub(crate) fn input(&self) -> &'src str {
        self.input
    }

    /// The `Header::Unknown` recovery for this header, with `reason`.
    pub(crate) fn unknown(&self, reason: impl Into<String>, suggested_fix: Option<&str>) -> Header {
        unknown_header_from_text(self.text, reason, suggested_fix)
    }
}

/// A content slot that did not deliver, its diagnostic already reported:
/// the words of the slot that refused, for the caller to build the recovery
/// from. The `Result` a verb returns carries this reference rather than a
/// built `Header`, which is a quarter of a kilobyte on the error path.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Refused<'w>(&'w ContentSlot<'w>);

impl Refused<'_> {
    /// The `Header::Unknown` for the refused slot, at `site`.
    pub(crate) fn into_header(self, site: &HeaderSite<'_, '_>) -> Header {
        site.unknown(self.0.missing, self.0.suggested_fix)
    }
}

/// Structural recovery may retain an unknown header; a producer fault may not.
#[derive(Debug)]
pub(crate) enum ContentReadError<'w> {
    /// The recovery diagnostic was already reported by the slot owner.
    Recovery(Refused<'w>),
    /// Source admission failed; the dispatch boundary must report this fault.
    Producer(CstFailure),
}

impl ContentReadError<'_> {
    /// Preserve the distinction until a fallible header consumer handles it.
    pub(crate) fn into_header(self, site: &HeaderSite<'_, '_>) -> Result<Header, CstFailure> {
        match self {
            Self::Recovery(refused) => Ok(refused.into_header(site)),
            Self::Producer(failure) => Err(failure),
        }
    }

    /// Finish at a diagnostic-sink boundary without fabricating fault recovery.
    pub(crate) fn into_outcome(
        self,
        site: &HeaderSite<'_, '_>,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Header> {
        match self.into_header(site) {
            Ok(header) => ParseOutcome::parsed(header),
            Err(failure) => {
                crate::parser::typed_cst::report_cst_failure(
                    site.actual(),
                    site.input(),
                    failure,
                    errors,
                );
                ParseOutcome::rejected()
            }
        }
    }
}

/// What a header says when its content slot does not deliver: the reason
/// recorded on the `Header::Unknown` recovery, and the fix to suggest with
/// it. The node kinds the diagnostic names come from the typed nodes
/// themselves (`NamedKind::KIND`), never from a string the caller spells.
#[derive(Debug)]
pub(crate) struct ContentSlot<'a> {
    /// The `parse_reason` of the `Header::Unknown` built when the slot fails.
    pub missing: &'a str,
    /// The `suggested_fix` beside that reason, when the header has one.
    pub suggested_fix: Option<&'a str>,
}

impl<'w> ContentSlot<'w> {
    /// Report a missing/recovered payload using the family's existing policy.
    fn refuse(
        &'w self,
        site: &HeaderSite<'_, '_>,
        child_kind: &str,
        errors: &impl ErrorSink,
    ) -> Refused<'w> {
        let node = site.actual();
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(site.input(), node.byte_range(), site.kind()),
            format!("Missing expected {child_kind} node in {}", site.kind()),
        ));
        Refused(self)
    }
}

/// Read an associated payload without accepting independently selected text.
/// Range admission remains fallible, and every non-present slot retains the
/// family's recovery policy. Source association does not certify valid syntax.
///
/// Generic over the slot's `Missing` payload `M` (see [`AnyKindSlot`]): a
/// placeholder refuses like any other non-present state and is never read, so
/// one body serves a slot that can be MISSING and a narrowed one under either
/// kind proof.
pub(crate) fn read_source_content<'value, 'tree: 'value, 'source, 'w, T, M>(
    site: &HeaderSite<'tree, '_>,
    content_slot: SourceField<'value, 'tree, 'source, AnyKindSlot<'tree, T, M>>,
    words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<&'source str, ContentReadError<'w>>
where
    T: SourceBoundKind<'tree> + NamedKind,
    M: SourceRecovery<'value, 'tree, 'source>,
{
    match content_slot.view() {
        SourceSlotView::Present(content) => content
            .read()
            .map(|bound| bound.text())
            .map_err(|error| ContentReadError::Producer(error.into())),
        SourceSlotView::Missing(_) | SourceSlotView::Absent(NoChild) | SourceSlotView::Error(_) => {
            Err(ContentReadError::Recovery(words.refuse(
                site,
                T::KIND,
                errors,
            )))
        }
    }
}

/// Read a range-admitted lexical slot. Only structural recovery can refuse;
/// the producer-failure transition has already completed for this carrier.
///
/// Generic over the `Missing` payload `M` for the reason
/// [`read_source_content`] is: a placeholder only refuses.
pub(crate) fn read_admitted_content<
    'tree,
    'source,
    'w,
    T: SourceBoundKind<'tree> + NamedKind,
    M,
>(
    site: &HeaderSite<'tree, '_>,
    slot: &ReadableSlot<'tree, 'source, SourceBound<'tree, 'source, T>, M, Never, NoChild>,
    words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<&'source str, Refused<'w>> {
    match slot {
        ReadableSlot::Present(content) => Ok(content.text()),
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Missing(_) | ReadableSlot::Absent(NoChild) | ReadableSlot::Error(_) => {
            Err(words.refuse(site, T::KIND, errors))
        }
    }
}
