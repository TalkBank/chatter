//! The verbs a header's typed content slot is read through, and the
//! `Header::Unknown` recovery they build: one owner for what the simple and
//! special header families, the pre-`@Begin` headers and `@Media` had each
//! written for themselves.

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, KindSlot, NamedKind, NoChild, SourceBound, SourceBoundKind, SourceField,
    SourceSlotView,
};
use crate::model::{Header, WarningText};
use tree_sitter::Node;

/// Build `Header::Unknown` from malformed header input: the header's own text
/// when it has any, else its node kind, with the reason the parser gave up.
/// Shared by the simple and special header families, the pre-`@Begin`
/// headers and `@Media`, each of which had written its own copy.
pub(crate) fn unknown_header_from_node(
    header_actual: Node,
    input: &str,
    reason: impl Into<String>,
    suggested_fix: Option<&str>,
) -> Header {
    let text = match header_actual.utf8_text(input.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => header_actual.kind().to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(reason.into()),
        suggested_fix: suggested_fix.map(str::to_string),
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
        unknown_header_from_node(self.actual, self.input, reason, suggested_fix)
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
pub(crate) fn read_source_content<'tree, 'source, 'w, T: SourceBoundKind<'tree> + NamedKind>(
    site: &HeaderSite<'tree, '_>,
    content_slot: SourceField<'_, 'tree, 'source, KindSlot<'tree, T>>,
    words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<&'source str, Refused<'w>> {
    match content_slot.view() {
        SourceSlotView::Present(content) => {
            crate::parser::typed_cst::read_source_field(content, errors)
                .map(|bound| bound.text())
                .ok_or(Refused(words))
        }
        SourceSlotView::Missing(_) | SourceSlotView::Absent(NoChild) | SourceSlotView::Error(_) => {
            Err(words.refuse(site, T::KIND, errors))
        }
    }
}
