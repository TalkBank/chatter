//! The verbs a header's typed content slot is read through, and the
//! `Header::Unknown` recovery they build: one owner for what the simple and
//! special header families, the pre-`@Begin` headers and `@Media` had each
//! written for themselves.

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{AsRawNode, ChildSlot, NamedKind, NoChild, SlotView};
use crate::model::{Header, WarningText};
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
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
/// and its kind are one value and cannot disagree; until 2026-09-09 every
/// verb took them as three separate arguments beside the errors sink, which
/// put two of the verbs past clippy's argument limit and let a caller pair a
/// node with another header's kind.
pub(crate) struct HeaderSite<'tree, 'src> {
    actual: Node<'tree>,
    kind: &'static str,
    input: &'src str,
}

impl<'tree, 'src> HeaderSite<'tree, 'src> {
    /// The site of `typed`, the header node the generator classified.
    pub(crate) fn of<T: AsRawNode<'tree> + NamedKind>(typed: &T, input: &'src str) -> Self {
        Self {
            actual: typed.raw_node(),
            kind: T::KIND,
            input,
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

/// Read a header's content child from its typed, positional slot: the decoded
/// text of a `Present` slot, or the refusal carrying `words` for every other
/// state, whose diagnostic has been reported.
///
/// The `NodeSlot` match is exhaustive; there is deliberately no `_` catch-all
/// that could silently drop a recovery slot. A non-`Present` slot (absent,
/// MISSING, ERROR or displaced) reports one "missing content" diagnostic at
/// the HEADER NODE span, as the pre-migration `find_child_by_kind` route did
/// for all four states; a later change may make it position-precise.
/// The site's kind is the header node type's own `KIND`, so the diagnostic's
/// context and the content kind it names are both the generator's literals.
///
/// Shared by the simple family, the special family (`@Number`, `@Recording
/// Quality`, `@Transcription`, `@Birth of`, `@Birthplace of`, `@L1 of`) and
/// `@Media`'s three payload slots, which each read through this one verb.
pub(crate) fn read_simple_content<'tree, 'w, T: AsRawNode<'tree> + NamedKind>(
    site: &HeaderSite<'tree, '_>,
    content_slot: &ChildSlot<'tree, T>,
    words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<String, Refused<'w>> {
    let header_actual = site.actual();
    let header_kind = site.kind();
    let input = site.input();
    let fallback = || Refused(words);
    match content_slot.view() {
        SlotView::Present(content) => {
            // Decode through the shared `decode_present_child` helper, which reads
            // from the RAW node's `utf8_text` (NOT the wrapper's `.text()`
            // accessor, which swallows UTF-8 errors via `unwrap_or("")`). The
            // family-specific diagnostic (context = `header_kind`, the "text for
            // {kind}" wording) is supplied here.
            match decode_present_child(content.raw_node(), input, errors, header_kind, |err| {
                format!("Failed to extract UTF-8 text for {header_kind}: {err}")
            }) {
                ParseOutcome::Parsed(text) => Ok(text),
                ParseOutcome::Rejected => Err(fallback()),
            }
        }
        SlotView::Missing(_) | SlotView::Absent(NoChild) | SlotView::Error(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(header_actual.start_byte(), header_actual.end_byte()),
                ErrorContext::new(
                    input,
                    header_actual.start_byte()..header_actual.end_byte(),
                    header_kind,
                ),
                format!("Missing expected {} node in {header_kind}", T::KIND),
            ));
            Err(fallback())
        }
    }
}
