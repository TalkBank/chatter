//! Per-kind parsing for scalar/textual header forms.
//!
//! These headers are decoded from a single content field without invoking
//! multi-node structural parsers. Each is one [`TextKind`] and one call to
//! [`content_header`]: the parser maps the node to its kind and the model
//! does the rest. The one emptiness decision ([`read_body`]) makes the empty
//! state from the kind, and the kind's own constructor ([`TextKind::header`])
//! makes the header from content, so pairing one header's empty form with
//! another header's builder is not expressible. Entry points consume
//! `SourceBound` rather than independently supplied nodes and source.
//! Shared recovery remains in `content_header`; finite lack of a
//! malformed-slot witness is not a proof that recovery is impossible.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Tape_Location_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    ActivitiesHeaderNode, BckHeaderNode, DateHeaderNode, LocationHeaderNode, NamedKind,
    PageHeaderNode, RoomLayoutHeaderNode, SituationHeaderNode, SourceBound, SourceBoundKind,
    SourceField, SourceRecovery, THeaderNode, TapeLocationHeaderNode, TimeDurationHeaderNode,
    TimeStartHeaderNode, TranscriberHeaderNode, VideosHeaderNode, WarningHeaderNode,
};
use crate::model::{Header, PlainKind, StructuredKind, TextKind};
use crate::parser::tree_parsing::parser_helpers::{
    Body, ContentSlot, HeaderSite, read_body, surface_displaced, unsupported_header_from_node,
};
use crate::parser::typed_cst::AnySelectedKindSlot;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// One text-kind header with one optional content slot: read the body once
/// ([`read_body`]), and build the empty state or the kind's header from it;
/// or hand back the recovery the slot produced. Then surface the carrier's
/// own unexpected sink. The one owner of the arms every text header used to
/// write. Generic over the content slot's recovery payloads, as
/// [`read_body`] is.
pub(crate) fn content_header<'value, 'tree: 'value, 'source, T, M, E>(
    site: &HeaderSite<'tree, 'source>,
    content_slot: SourceField<'value, 'tree, 'source, Option<AnySelectedKindSlot<T, M, E>>>,
    unexpected: &[Node<'tree>],
    words: &ContentSlot<'_>,
    errors: &impl ErrorSink,
    kind: TextKind,
) -> ParseOutcome<Header>
where
    T: SourceBoundKind<'tree> + NamedKind,
    M: SourceRecovery<'value, 'tree, 'source>,
    E: SourceRecovery<'value, 'tree, 'source>,
{
    let header = match read_body(site, content_slot, words, errors) {
        Ok(Body::Empty(separator)) => ParseOutcome::parsed(PlainKind::Text(kind).empty(separator)),
        Ok(Body::Written { text, .. }) => ParseOutcome::parsed(kind.header(text)),
        Err(failure) => failure.into_outcome(site, errors),
    };
    surface_displaced(unexpected, site.kind(), site.input(), errors);
    header
}

/// One text-kind header entry point: admit the envelope, then
/// [`content_header`] over its content slot (`child_2`) with the kind.
macro_rules! text_header {
    ($(#[$doc:meta])* $name:ident, $node:ident, $kind:expr, $missing:literal) => {
        $(#[$doc])*
        pub(super) fn $name<'tree>(
            typed: SourceBound<'tree, '_, $node<'tree>>,
            errors: &impl ErrorSink,
        ) -> ParseOutcome<Header> {
            let site = HeaderSite::bound(typed);
            let Ok(children) = crate::parser::typed_cst::report_admission(
                crate::parser::typed_cst::extract_envelope(typed),
                site.actual(),
                site.input(),
                errors,
            ) else {
                return ParseOutcome::Rejected;
            };
            content_header(
                &site,
                children.field_child_2().slot(),
                &children.children().unexpected,
                &ContentSlot {
                    missing: $missing,
                    suggested_fix: None,
                },
                errors,
                $kind,
            )
        }
    };
}

text_header!(
    /// `@Date` -> `Header::Date`. `date_contents` is `choice(strict_date,
    /// generic_date)`; the text is kept and `ChatDate` classifies it, and the
    /// validator reports E518 for a malformed date.
    date, DateHeaderNode, TextKind::Date, "Missing @Date content"
);
text_header!(
    /// `@Situation` -> `Header::Situation`.
    situation, SituationHeaderNode, TextKind::Situation, "Missing @Situation content"
);
text_header!(
    /// `@Tape Location` -> `Header::TapeLocation`.
    tape_location, TapeLocationHeaderNode, TextKind::TapeLocation, "Missing @Tape Location content"
);
text_header!(
    /// `@Time Duration` -> `Header::TimeDuration`.
    time_duration, TimeDurationHeaderNode, TextKind::TimeDuration, "Missing @Time Duration content"
);
text_header!(
    /// `@Time Start` -> `Header::TimeStart`.
    time_start, TimeStartHeaderNode, TextKind::TimeStart, "Missing @Time Start content"
);
text_header!(
    /// `@Location` -> `Header::Location`.
    location, LocationHeaderNode, TextKind::Location, "Missing @Location content"
);
text_header!(
    /// `@Room Layout` -> `Header::RoomLayout`.
    room_layout, RoomLayoutHeaderNode, TextKind::RoomLayout, "Missing @Room Layout content"
);
text_header!(
    /// `@Transcriber` -> `Header::Transcriber`.
    transcriber, TranscriberHeaderNode, TextKind::Transcriber, "Missing @Transcriber content"
);
text_header!(
    /// `@Warning` -> `Header::Warning`.
    warning, WarningHeaderNode, TextKind::Warning, "Missing @Warning content"
);
text_header!(
    /// `@Activities` -> `Header::Activities`.
    activities, ActivitiesHeaderNode, TextKind::Activities, "Missing @Activities content"
);
text_header!(
    /// `@Bck` -> `Header::Bck`.
    bck, BckHeaderNode, TextKind::Bck, "Missing @Bck content"
);
text_header!(
    /// `@Videos` -> `Header::Videos`.
    videos, VideosHeaderNode, TextKind::Videos, "Missing @Videos content"
);
text_header!(
    /// `@T` -> `Header::T`.
    t, THeaderNode, TextKind::T, "Missing @T content"
);

/// `@Page` -> `Header::Page`. Its content is a page number, admitted by
/// `PageNumber` when built; a malformed one is E316 and an unknown header.
pub(super) fn page<'tree>(
    typed: SourceBound<'tree, '_, PageHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(typed),
        site.actual(),
        site.input(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let words = ContentSlot {
        missing: "Missing @Page number",
        suggested_fix: None,
    };
    let header = match read_body(&site, children.field_child_2().slot(), &words, errors) {
        Ok(Body::Empty(separator)) => {
            ParseOutcome::parsed(PlainKind::Structured(StructuredKind::Page).empty(separator))
        }
        Ok(Body::Written { text, .. }) => match crate::model::PageNumber::new(text.as_str()) {
            Ok(page) => ParseOutcome::parsed(Header::Page { page }),
            Err(error) => ParseOutcome::parsed(site.refuse_content(&error.to_string(), errors)),
        },
        Err(failure) => failure.into_outcome(&site, errors),
    };
    surface_displaced(
        &children.children().unexpected,
        site.kind(),
        site.input(),
        errors,
    );
    header
}

/// An `unsupported_header` node: the grammar recognised a header the model
/// does not carry, so it is kept as `Header::Unknown` with its text.
pub(super) fn unsupported(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    match unsupported_header_from_node(header_actual, input) {
        Ok(header) => ParseOutcome::parsed(header),
        Err(error) => {
            crate::parser::typed_cst::report_cst_failure(header_actual, input, error, errors);
            ParseOutcome::Rejected
        }
    }
}
