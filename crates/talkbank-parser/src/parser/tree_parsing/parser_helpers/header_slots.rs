//! The verbs a header's typed content slot is read through, and the
//! `Header::Unknown` recovery they build: one owner for what the simple and
//! special header families, the pre-`@Begin` headers and `@Media` had each
//! written for themselves.

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, FromNodeKind, HeaderSepNode,
    KindAdmitted, L1OfHeaderNode, NamedKind, NarrowedMissing, Never, NodeSlot, ReadableSlot,
    SourceBindingError, SourceBound, SourceBoundKind, SourceField, SourceRecovery, SourceSlotView,
    extract_birth_of_header, extract_birthplace_of_header, extract_l1_of_header,
};
use crate::model::{Header, HeaderContent, HeaderText, RecoveryText, WrittenSeparator};
use crate::parser::node_span::span_of;
use crate::parser::typed_cst::CstFailure;
use crate::parser::typed_cst::{AnyKindSlot, AnySelectedKindSlot};
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
    Ok(unknown_header_from_line(
        HeaderLine::of(text),
        reason,
        suggested_fix,
    ))
}

/// An `unsupported_header` node, a header name the model does not carry, kept
/// as `Header::Unknown` with its text. Its reason is the shared
/// [`MALFORMED_HEADER_CONTENT`](crate::model::MALFORMED_HEADER_CONTENT) when
/// it holds an ERROR, the reason re2c records for a catch-all line with a
/// malformed token, and "Unsupported header type" otherwise.
pub(crate) fn unsupported_header_from_node(
    header_actual: Node,
    input: &str,
) -> Result<Header, SourceBindingError> {
    let reason = if holds_error(header_actual) {
        crate::model::MALFORMED_HEADER_CONTENT
    } else {
        "Unsupported header type"
    };
    unknown_header_from_node(header_actual, input, reason, None)
}

/// The one builder of this backend's `Header::Unknown`: only a
/// [`HeaderLine`] can supply its text, so no recovery keeps a line
/// terminator that re2c's recovery of the same line does not.
fn unknown_header_from_line(
    line: HeaderLine<'_>,
    reason: impl Into<String>,
    suggested_fix: Option<&str>,
) -> Header {
    Header::Unknown {
        text: RecoveryText::observed(line.0),
        parse_reason: Some(reason.into()),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}

/// A header's line as written, without the line terminator that ends it:
/// the text every `Header::Unknown` this backend keeps.
///
/// A header node's text ends with its `newline` token, while re2c's header
/// region stops before that token, so until this type the two backends kept
/// different text for the same malformed line, and tree-sitter's E525
/// message carried a line break. Built only by [`HeaderLine::of`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct HeaderLine<'src>(&'src str);

impl<'src> HeaderLine<'src> {
    /// The header node's text without its trailing `newline` token (the
    /// grammar's `\r\n`, `\r` or `\n`). A header that ends the input
    /// without a written terminator is its whole text. A continuation line
    /// break inside the content is content and is kept: the terminator is
    /// only the token that ends the header.
    fn of(header_text: &'src str) -> Self {
        match header_text.strip_suffix("\r\n") {
            Some(line) => Self(line),
            None => match header_text.strip_suffix(['\r', '\n']) {
                Some(line) => Self(line),
                None => Self(header_text),
            },
        }
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
            let words = ContentSlot {
                missing: "boundary control",
                suggested_fix: None,
            };
            let errors = crate::error::ErrorCollector::new();
            let refused = {
                let selected = HeaderSite::bound(parsed.bind_typed(header).expect("owner"));
                words.refuse(&selected, "payload", &errors)
            };
            // Reporting selected the source already. Recovery needs neither
            // the temporary site nor a caller-selected replacement header.
            assert_eq!(refused.into_header(), observed);
            assert_eq!(errors.into_vec().len(), 1);
            let Header::Unknown { text, .. } = observed else {
                panic!("unknown header")
            };
            // The line without its terminator, as re2c keeps it.
            let line = &source[node.byte_range()];
            assert_eq!(text.to_string(), line.strip_suffix('\n').expect("a line"));
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
    line: HeaderLine<'src>,
}

impl<'tree, 'src> HeaderSite<'tree, 'src> {
    /// Retain a producer-bound header's own node, kind and source together.
    pub(crate) fn bound<T: SourceBoundKind<'tree> + NamedKind>(
        typed: SourceBound<'tree, 'src, T>,
    ) -> Self {
        Self {
            actual: typed.raw_node(),
            kind: T::KIND.name(),
            input: typed.source(),
            line: HeaderLine::of(typed.text()),
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
        unknown_header_from_line(self.line, reason, suggested_fix)
    }

    /// Refuse content the header's own reader found malformed: report it
    /// (E316) at the header and keep the header as unknown with `reason`, the
    /// counterpart of re2c's refusal of the same line.
    pub(crate) fn refuse_content(&self, reason: &str, errors: &impl ErrorSink) -> Header {
        errors.report(ParseError::at_span(
            ErrorCode::UnparsableContent,
            Severity::Error,
            span_of(self.actual),
            reason,
        ));
        self.unknown(reason, None)
    }

    /// The header's separator, when its colon and tab were both written:
    /// the separator node's own text, admitted by the model. A colon whose
    /// tab is a recovery placeholder (zero-width, E303) is no written
    /// separator.
    pub(crate) fn written_separator(&self) -> Result<Option<WrittenSeparator>, CstFailure> {
        Ok(match find_header_separator(self.actual) {
            Some(separator) => {
                let raw = separator.raw_node();
                let text = self
                    .input
                    .get(raw.byte_range())
                    .ok_or(SourceBindingError::InvalidRange)?;
                WrittenSeparator::admit(text)
            }
            None => None,
        })
    }

    /// What a header with nothing after its separator is, once it is known
    /// to hold no unparsable text: the empty state, when the separator was
    /// written, else a malformed header.
    fn empty_body<'w>(
        &self,
        complete: CompleteBody<()>,
    ) -> Result<WrittenSeparator, ContentReadError<'src, 'w>> {
        let CompleteBody(()) = complete;
        match self.written_separator() {
            Ok(Some(separator)) => Ok(separator),
            Ok(None) => Err(ContentReadError::Unseparated),
            Err(failure) => Err(ContentReadError::Producer(failure)),
        }
    }

    /// The one decision of whether a header's body is all that was written:
    /// `body` (read from this same header) comes back proven
    /// [`CompleteBody`] when no node of the header is or holds an ERROR
    /// node, and is refused as unparsable text
    /// ([`ContentReadError::Unparsable`]) otherwise.
    ///
    /// An ERROR holds text no slot of the header does: `@Situation:` and a
    /// tab, then `at`, a NUL and `home`, parses `at` as the body and the rest
    /// as an ERROR beside it. Lowering the body would build a header from the
    /// part before the ERROR and silently drop the rest; re2c, whose lexer
    /// marks the same bytes malformed, keeps the line as an unknown header.
    /// The whole header is judged, not only what follows its separator: an
    /// ERROR between a participant and the colon, inside the separator, or
    /// after a gem's bare marker drops text just the same. Every header
    /// family's content reaches the model through this one check
    /// ([`read_body`], [`read_empty_body`], a gem's bare form and the `@Types`
    /// field group), so none can lower partial content.
    ///
    /// A MISSING placeholder is not judged here. It is zero-width, so it
    /// drops no text; the reader of the slot it stands in refuses it with
    /// that slot's own recovery and diagnostic (the whole-tree backstop does
    /// not report every placeholder, so refusing one here as unparsable
    /// would leave it unreported); and a header that ends the input without
    /// a line terminator carries one in its `newline` position, which is no
    /// defect at all.
    pub(crate) fn complete_body<'w, B>(
        &self,
        body: B,
    ) -> Result<CompleteBody<B>, ContentReadError<'src, 'w>> {
        if holds_error(self.actual) {
            Err(ContentReadError::Unparsable)
        } else {
            Ok(CompleteBody(body))
        }
    }
}

/// Whether `node` is, or has among its descendants, an ERROR node. Only a
/// subtree tree-sitter flags as holding recovery is walked, so a clean
/// subtree costs one flag read.
fn holds_error(node: Node<'_>) -> bool {
    if node.is_error() {
        return true;
    }
    if !node.has_error() {
        return false;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).any(holds_error)
}

/// A header body proven complete: nothing after the header's separator is
/// or holds an ERROR node, so the body is all the text the author wrote and
/// not the part of it before an ERROR.
///
/// Issued only by [`HeaderSite::complete_body`], the one place that decision
/// is made; its field is private to this module, so a [`Body::Written`], the
/// only state that lowers content, cannot be built without it.
pub(crate) struct CompleteBody<B>(B);

impl<B> CompleteBody<B> {
    /// The proven body.
    pub(crate) fn into_inner(self) -> B {
        self.0
    }

    /// The proof apart from the slot it was judged with, so the body read
    /// from that slot can carry it ([`CompleteBody::holding`]).
    fn split(self) -> (CompleteBody<()>, B) {
        (CompleteBody(()), self.0)
    }
}

impl CompleteBody<()> {
    /// The same header's proof, now holding the body read from it.
    fn holding<B>(self, body: B) -> CompleteBody<B> {
        CompleteBody(body)
    }
}

/// The header's present `header_sep` node, through its generated carrier for
/// the speaker-qualified headers (no raw child index assumed) and at the
/// first named child for every other header.
pub(crate) fn find_header_separator<'tree>(
    header_node: Node<'tree>,
) -> Option<HeaderSepNode<'tree>> {
    if let Some(header) = BirthOfHeaderNode::from_node(header_node) {
        present_header_separator(extract_birth_of_header(header).child_3.slot())
    } else if let Some(header) = BirthplaceOfHeaderNode::from_node(header_node) {
        present_header_separator(extract_birthplace_of_header(header).child_3.slot())
    } else if let Some(header) = L1OfHeaderNode::from_node(header_node) {
        present_header_separator(extract_l1_of_header(header).child_3.slot())
    } else {
        header_node
            .named_child(1)
            .and_then(HeaderSepNode::from_node)
    }
}

/// Only a present separator establishes whitespace geometry, regardless of
/// which proof axes its enclosing header exposes.
fn present_header_separator<'tree, M, E, U, A>(
    slot: &NodeSlot<HeaderSepNode<'tree>, M, E, U, A>,
) -> Option<HeaderSepNode<'tree>> {
    match slot {
        NodeSlot::Present(separator) => Some(*separator),
        NodeSlot::Missing(_)
        | NodeSlot::Error(_)
        | NodeSlot::Unexpected(_)
        | NodeSlot::Absent(_) => None,
    }
}

/// A content slot that did not deliver, its diagnostic already reported:
/// the header's admitted text and the words of the slot that refused.
/// Recovery consumes this evidence without selecting a header a second time.
/// The borrowed payload avoids constructing a full Header on the error path.
#[derive(Debug)]
pub(crate) struct Refused<'source, 'w> {
    line: HeaderLine<'source>,
    words: &'w ContentSlot<'w>,
}

impl Refused<'_, '_> {
    /// The `Header::Unknown` for the same header whose slot was refused.
    pub(crate) fn into_header(self) -> Header {
        unknown_header_from_line(self.line, self.words.missing, self.words.suggested_fix)
    }
}

/// Structural recovery may retain an unknown header; a producer fault may not.
#[derive(Debug)]
pub(crate) enum ContentReadError<'source, 'w> {
    /// The recovery diagnostic was already reported by the slot owner.
    Recovery(Refused<'source, 'w>),
    /// An ERROR stands where the grammar-optional content belongs, or
    /// anywhere else in the header ([`HeaderSite::complete_body`]):
    /// the text is unparsable, which the whole-tree backstop reports (E316).
    /// Neither empty content nor a header of this kind can be built from it,
    /// so the header is kept as unknown with its text, the recovery every
    /// other malformed header gets and the one re2c builds for the same line.
    Unparsable,
    /// Nothing follows a separator whose tab was not written (a colon alone,
    /// which the whole-tree pass reports as E303): a malformed header, kept
    /// as unknown with its text, never the empty state.
    Unseparated,
    /// Source admission failed; the dispatch boundary must report this fault.
    Producer(CstFailure),
}

impl ContentReadError<'_, '_> {
    /// Finish at a diagnostic-sink boundary without fabricating fault recovery.
    pub(crate) fn into_outcome(
        self,
        site: &HeaderSite<'_, '_>,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Header> {
        match self {
            Self::Recovery(refused) => ParseOutcome::parsed(refused.into_header()),
            // The reason is the model's one constant, which re2c records
            // for the same line; the diagnostic (E316) is the whole-tree
            // backstop's, which names the text.
            Self::Unparsable => {
                ParseOutcome::parsed(site.unknown(crate::model::MALFORMED_HEADER_CONTENT, None))
            }
            Self::Unseparated => {
                ParseOutcome::parsed(site.unknown("Header colon is not followed by its tab", None))
            }
            Self::Producer(failure) => {
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
/// themselves (`NamedKind::KIND.name()`), never from a string the caller spells.
#[derive(Debug)]
pub(crate) struct ContentSlot<'a> {
    /// The `parse_reason` of the `Header::Unknown` built when the slot fails.
    pub missing: &'a str,
    /// The `suggested_fix` beside that reason, when the header has one.
    pub suggested_fix: Option<&'a str>,
}

impl<'w> ContentSlot<'w> {
    /// Report a missing/recovered payload using the family's existing policy.
    fn refuse<'source>(
        &'w self,
        site: &HeaderSite<'_, 'source>,
        child_kind: &str,
        errors: &impl ErrorSink,
    ) -> Refused<'source, 'w> {
        let node = site.actual();
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(site.input(), span_of(node), site.kind()),
            format!("Missing expected {child_kind} node in {}", site.kind()),
        ));
        Refused {
            line: site.line,
            words: self,
        }
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
pub(crate) fn read_source_content<'value, 'tree: 'value, 'source, 'w, T, M, E, A>(
    site: &HeaderSite<'tree, 'source>,
    content_slot: SourceField<'value, 'tree, 'source, AnyKindSlot<T, M, E, A>>,
    words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<&'source str, ContentReadError<'source, 'w>>
where
    T: SourceBoundKind<'tree> + NamedKind,
    M: SourceRecovery<'value, 'tree, 'source>,
    E: SourceRecovery<'value, 'tree, 'source>,
    A: crate::generated_traversal::Absence,
{
    match content_slot.view() {
        SourceSlotView::Present(content) => content
            .read()
            .map(|bound| bound.text())
            .map_err(|error| ContentReadError::Producer(error.into())),
        SourceSlotView::Missing(_) | SourceSlotView::Absent(_) | SourceSlotView::Error(_) => Err(
            ContentReadError::Recovery(words.refuse(site, T::KIND.name(), errors)),
        ),
    }
}

/// What a header's grammar-optional body held, read once.
pub(crate) enum Body<'source, B> {
    /// Nothing, or only whitespace, after a written separator: the empty
    /// state, which only this evidence can make.
    Empty(WrittenSeparator),
    /// Content: the body and its text, proven nonblank, the body proven to
    /// be all that was written ([`CompleteBody`]).
    Written {
        /// The body, for a header whose content has its own grammar.
        body: CompleteBody<B>,
        /// The body's text, from the one emptiness decision.
        text: HeaderText<'source>,
    },
}

/// Read a header's grammar-optional body: the one owner, for every header
/// family, of "an omitted or blank body is empty content".
///
/// An omitted body, or a present one whose text is blank, is
/// [`Body::Empty`] when the separator was written and a malformed header
/// otherwise ([`ContentReadError::Unseparated`]). A MISSING placeholder, or
/// a blank body made of recovery placeholders, refuses with its recovery and
/// is never empty content. An ERROR anywhere in the header, where the body
/// belongs or in or beside it ([`HeaderSite::complete_body`]), is unparsable
/// text, the whole-tree backstop's to report: never content built from the
/// part of the body before it. Source-binding faults remain producer
/// failures. Generic over the slot's recovery payloads, so one body serves
/// every header family.
pub(crate) fn read_body<'value, 'tree: 'value, 'source, 'w, T, M, E>(
    site: &HeaderSite<'tree, 'source>,
    content_slot: SourceField<'value, 'tree, 'source, Option<AnySelectedKindSlot<T, M, E>>>,
    words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<Body<'source, SourceBound<'tree, 'source, T>>, ContentReadError<'source, 'w>>
where
    T: SourceBoundKind<'tree> + NamedKind,
    M: SourceRecovery<'value, 'tree, 'source>,
    E: SourceRecovery<'value, 'tree, 'source>,
{
    // The header is judged before its slot: an ERROR anywhere in it wins
    // over a MISSING body, as re2c's malformed-token refusal does.
    let (complete, content_slot) = site.complete_body(content_slot)?.split();
    let Some(slot) = content_slot.optional() else {
        return site.empty_body(complete).map(Body::Empty);
    };
    let body = match slot.view() {
        SourceSlotView::Present(content) => content
            .read()
            .map_err(|error| ContentReadError::Producer(error.into()))?,
        SourceSlotView::Missing(_) => {
            return Err(ContentReadError::Recovery(words.refuse(
                site,
                T::KIND.name(),
                errors,
            )));
        }
        SourceSlotView::Error(_) => return Err(ContentReadError::Unparsable),
        SourceSlotView::Absent(never) | SourceSlotView::Unexpected(never) => match never {},
    };
    match HeaderContent::classify(body.text()) {
        HeaderContent::Text(text) => Ok(Body::Written {
            body: complete.holding(body),
            text,
        }),
        // A body whose only content is recovery placeholders supplies no
        // text, but it is a recovery, not an empty header.
        HeaderContent::Blank if body.raw_node().has_error() => Err(ContentReadError::Recovery(
            words.refuse(site, T::KIND.name(), errors),
        )),
        HeaderContent::Blank => site.empty_body(complete).map(Body::Empty),
    }
}

/// Read the empty state of a header whose body is not a single slot (the
/// `@Types` field group): the same separator and recovery rules
/// [`read_body`] applies.
pub(crate) fn read_empty_body<'source, 'w>(
    site: &HeaderSite<'_, 'source>,
) -> Result<WrittenSeparator, ContentReadError<'source, 'w>> {
    site.complete_body(())
        .and_then(|complete| site.empty_body(complete))
}

/// Read a range-admitted lexical slot of a kind the admitted proof narrows,
/// so it cannot be MISSING. Only structural recovery (an ERROR node, or a
/// position the walk left absent) can refuse; the producer-failure
/// transition has already completed for this carrier.
pub(crate) fn read_admitted_content<
    'tree,
    'source,
    'w,
    T: SourceBoundKind<'tree> + NamedKind,
    M,
    E,
    A,
>(
    site: &HeaderSite<'tree, 'source>,
    slot: &ReadableSlot<
        SourceBound<'tree, 'source, T>,
        NarrowedMissing<KindAdmitted, M>,
        E,
        Never,
        A,
    >,
    words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<&'source str, Refused<'source, 'w>> {
    match slot {
        ReadableSlot::Present(content) => Ok(content.text()),
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Missing(NarrowedMissing(never)) => match *never {},
        ReadableSlot::Absent(_) | ReadableSlot::Error(_) => {
            Err(words.refuse(site, T::KIND.name(), errors))
        }
    }
}
