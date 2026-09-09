//! Per-kind parsing for scalar/textual header forms.
//!
//! These headers can be decoded from a single content field without invoking
//! multi-node structural parsers. Each function here is the LEVEL-2 entry for one
//! `HeaderChoice` simple variant, and each is one call to [`simple_header`]: the
//! typed content slot (`extract_<kind>(node).child_2`), the words for its
//! recovery, and the constructor for the header it builds. Until 2026-09-08
//! every function here matched the slot outcome itself, thirteen copies of the
//! same two arms; a whole-workspace coverage run showed the recovery arm of
//! every copy unreached, because the grammar always supplies the content child
//! and tree-sitter parks a malformed header line in a file-level ERROR node
//! rather than inside the header. The one copy left is in `simple_header`.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Tape_Location_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    ActivitiesHeaderNode, AsRawNode, BckHeaderNode, ChildSlot, DateHeaderNode, LocationHeaderNode,
    NamedKind, PageHeaderNode, RoomLayoutHeaderNode, THeaderNode, TapeLocationHeaderNode,
    TimeDurationHeaderNode, TimeStartHeaderNode, TranscriberHeaderNode, VideosHeaderNode,
    WarningHeaderNode, extract_activities_header, extract_bck_header, extract_date_header,
    extract_location_header, extract_page_header, extract_room_layout_header, extract_t_header,
    extract_tape_location_header, extract_time_duration_header, extract_time_start_header,
    extract_transcriber_header, extract_videos_header, extract_warning_header,
};
use crate::model::{self, Header};
use crate::parser::tree_parsing::parser_helpers::{
    ContentSlot, HeaderSite, read_simple_content, surface_displaced, unknown_header_from_node,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// One header with one content slot: read the slot, build the header from
/// its text with `build`, or hand back the recovery the slot produced, then
/// surface the carrier's own unexpected sink. The one owner of the two arms
/// every simple header used to write.
pub(super) fn simple_header<'tree, T: AsRawNode<'tree> + NamedKind>(
    site: &HeaderSite<'tree, '_>,
    content_slot: &ChildSlot<'tree, T>,
    unexpected: &[Node<'tree>],
    words: &ContentSlot<'_>,
    errors: &impl ErrorSink,
    build: impl FnOnce(String) -> Header,
) -> ParseOutcome<Header> {
    let header = read_simple_content(site, content_slot, words, errors)
        .map_or_else(|refused| refused.into_header(site), build);
    surface_displaced(unexpected, site.kind(), site.input(), errors);
    ParseOutcome::parsed(header)
}

/// `@Date` -> `Header::Date`. `date_contents` is `choice(strict_date,
/// generic_date)`; the text is kept and `ChatDate::new` classifies it, and the
/// validator reports E518 for a malformed date.
pub(super) fn date(
    typed: DateHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_date_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn tape_location(
    typed: TapeLocationHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_tape_location_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn time_duration(
    typed: TimeDurationHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_time_duration_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn time_start(
    typed: TimeStartHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_time_start_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn location(
    typed: LocationHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_location_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn room_layout(
    typed: RoomLayoutHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_room_layout_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn transcriber(
    typed: TranscriberHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_transcriber_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn warning(
    typed: WarningHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_warning_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn activities(
    typed: ActivitiesHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_activities_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn bck(
    typed: BckHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_bck_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn page(
    typed: PageHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_page_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn videos(
    typed: VideosHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_videos_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
pub(super) fn t(
    typed: THeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_t_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
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
