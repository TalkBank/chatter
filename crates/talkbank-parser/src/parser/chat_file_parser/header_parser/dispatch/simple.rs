//! Per-kind parsing for scalar/textual header forms.
//!
//! These headers can be decoded from a single content field without invoking
//! multi-node structural parsers. Each function here is the LEVEL-2 entry for one
//! `HeaderChoice` simple variant, and each is one call to [`simple_header`]: the
//! source-associated content slot, the words for its recovery, and the
//! constructor for the header it builds. Entry points consume `SourceBound`
//! rather than independently supplied nodes and source. Payload text is borrowed
//! after range admission. Shared recovery remains in `simple_header`; finite
//! lack of a malformed-slot witness is not a proof that recovery is impossible.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Tape_Location_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    ActivitiesHeaderNode, BckHeaderNode, DateHeaderNode, KindSlot, LocationHeaderNode, NamedKind,
    PageHeaderNode, RoomLayoutHeaderNode, SourceBound, SourceBoundKind, SourceField, THeaderNode,
    TapeLocationHeaderNode, TimeDurationHeaderNode, TimeStartHeaderNode, TranscriberHeaderNode,
    VideosHeaderNode, WarningHeaderNode,
};
use crate::model::{self, Header};
use crate::parser::tree_parsing::parser_helpers::{
    ContentSlot, HeaderSite, read_source_content, surface_displaced, unknown_header_from_node,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// One header with one content slot: read the slot, build the header from
/// its text with `build`, or hand back the recovery the slot produced, then
/// surface the carrier's own unexpected sink. The one owner of the two arms
/// every simple header used to write.
pub(super) fn simple_header<'tree, 'source, T: SourceBoundKind<'tree> + NamedKind>(
    site: &HeaderSite<'tree, '_>,
    content_slot: SourceField<'_, 'tree, 'source, KindSlot<'tree, T>>,
    unexpected: &[Node<'tree>],
    words: &ContentSlot<'_>,
    errors: &impl ErrorSink,
    build: impl FnOnce(&'source str) -> Header,
) -> ParseOutcome<Header> {
    let header = read_source_content(site, content_slot, words, errors)
        .map_or_else(|refused| refused.into_header(site), build);
    surface_displaced(unexpected, site.kind(), site.input(), errors);
    ParseOutcome::parsed(header)
}

/// `@Date` -> `Header::Date`. `date_contents` is `choice(strict_date,
/// generic_date)`; the text is kept and `ChatDate::new` classifies it, and the
/// validator reports E518 for a malformed date.
pub(super) fn date<'tree>(
    typed: SourceBound<'tree, '_, DateHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Date content",
            suggested_fix: None,
        },
        errors,
        |content| Header::Date {
            date: model::ChatDate::new(content),
        },
    )
}

/// `@Tape Location` -> `Header::TapeLocation`.
pub(super) fn tape_location<'tree>(
    typed: SourceBound<'tree, '_, TapeLocationHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Tape Location content",
            suggested_fix: None,
        },
        errors,
        |location| Header::TapeLocation {
            location: model::TapeLocationDescription::new(location),
        },
    )
}

/// `@Time Duration` -> `Header::TimeDuration`.
pub(super) fn time_duration<'tree>(
    typed: SourceBound<'tree, '_, TimeDurationHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Time Duration content",
            suggested_fix: None,
        },
        errors,
        |duration| Header::TimeDuration {
            duration: model::TimeDurationValue::new(duration),
        },
    )
}

/// `@Time Start` -> `Header::TimeStart`.
pub(super) fn time_start<'tree>(
    typed: SourceBound<'tree, '_, TimeStartHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Time Start content",
            suggested_fix: None,
        },
        errors,
        |start| Header::TimeStart {
            start: model::TimeStartValue::new(start),
        },
    )
}

/// `@Location` -> `Header::Location`.
pub(super) fn location<'tree>(
    typed: SourceBound<'tree, '_, LocationHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Location content",
            suggested_fix: None,
        },
        errors,
        |location| Header::Location {
            location: model::LocationDescription::new(location),
        },
    )
}

/// `@Room Layout` -> `Header::RoomLayout`.
pub(super) fn room_layout<'tree>(
    typed: SourceBound<'tree, '_, RoomLayoutHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Room Layout content",
            suggested_fix: None,
        },
        errors,
        |layout| Header::RoomLayout {
            layout: model::RoomLayoutDescription::new(layout),
        },
    )
}

/// `@Transcriber` -> `Header::Transcriber`.
pub(super) fn transcriber<'tree>(
    typed: SourceBound<'tree, '_, TranscriberHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Transcriber content",
            suggested_fix: None,
        },
        errors,
        |transcriber| Header::Transcriber {
            transcriber: model::TranscriberName::new(transcriber),
        },
    )
}

/// `@Warning` -> `Header::Warning`.
pub(super) fn warning<'tree>(
    typed: SourceBound<'tree, '_, WarningHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Warning content",
            suggested_fix: None,
        },
        errors,
        |text| Header::Warning {
            text: model::WarningText::new(text),
        },
    )
}

/// `@Activities` -> `Header::Activities`.
pub(super) fn activities<'tree>(
    typed: SourceBound<'tree, '_, ActivitiesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Activities content",
            suggested_fix: None,
        },
        errors,
        |activities| Header::Activities {
            activities: model::ActivitiesDescription::new(activities),
        },
    )
}

/// `@Bck` -> `Header::Bck`.
pub(super) fn bck<'tree>(
    typed: SourceBound<'tree, '_, BckHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Bck content",
            suggested_fix: None,
        },
        errors,
        |bck| Header::Bck {
            bck: model::BackgroundDescription::new(bck),
        },
    )
}

/// `@Page` -> `Header::Page`.
pub(super) fn page<'tree>(
    typed: SourceBound<'tree, '_, PageHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Page number",
            suggested_fix: None,
        },
        errors,
        |page| Header::Page {
            page: model::PageNumber::new(page),
        },
    )
}

/// `@Videos` -> `Header::Videos`.
pub(super) fn videos<'tree>(
    typed: SourceBound<'tree, '_, VideosHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Videos content",
            suggested_fix: None,
        },
        errors,
        |videos| Header::Videos {
            videos: model::VideoSpec::new(videos),
        },
    )
}

/// `@T` -> `Header::T`.
pub(super) fn t<'tree>(
    typed: SourceBound<'tree, '_, THeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @T content",
            suggested_fix: None,
        },
        errors,
        |text| Header::T {
            text: model::TDescription::new(text),
        },
    )
}

/// An `unsupported_header` node: the grammar recognised a header the model
/// does not carry, so it is kept as `Header::Unknown` with its text.
pub(super) fn unsupported(
    header_actual: Node,
    input: &str,
    _errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    ParseOutcome::parsed(unknown_header_from_node(
        header_actual,
        input,
        "Unsupported header type",
        None,
    ))
}
