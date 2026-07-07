//! Per-kind parsing for scalar/textual header forms.
//!
//! These headers can be decoded from a single content field without invoking
//! multi-node structural parsers. Each function here is the LEVEL-2 entry for one
//! `HeaderChoice` simple variant.
//!
//! LEVEL 2: the content child is read through the NEW backend's free, typed,
//! POSITIONAL `extract_<kind>(node).child_2.slot`, replacing the pre-migration
//! `get_required_content_by_kind` / `find_child_by_kind` `node.kind()` scan (and,
//! as of Task B2, the OLD `TypedTraversal.extract_<kind>` trait-receiver call).
//!
//! The migration is BEHAVIOUR-PRESERVING: a `Present` content slot with valid
//! UTF-8 builds the same `Header::Xxx { .. }` value as before, and the diagnostic
//! plus `Header::Unknown` recovery fallback for a malformed or missing content
//! slot is byte-identical to the pre-migration `get_required_content_by_kind`
//! plus `unknown_header_from_node` behaviour (see `read_simple_content`). Each
//! carrier's own `unexpected` sink is surfaced per R2 of the migration template
//! (see `parser_helpers::surface_unexpected`).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Tape_Location_Header>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    ActivitiesHeaderNode, AsRawNode, BckHeaderNode, DateHeaderNode, LocationHeaderNode, NodeSlot,
    PageHeaderNode, RoomLayoutHeaderNode, THeaderNode, TapeLocationHeaderNode,
    TimeDurationHeaderNode, TimeStartHeaderNode, TranscriberHeaderNode, VideosHeaderNode,
    WarningHeaderNode, extract_activities_header, extract_bck_header, extract_date_header,
    extract_location_header, extract_page_header, extract_room_layout_header, extract_t_header,
    extract_tape_location_header, extract_time_duration_header, extract_time_start_header,
    extract_transcriber_header, extract_videos_header, extract_warning_header,
};
use crate::model::{self, Header};
use crate::node_types::{
    ACTIVITIES_HEADER, BCK_HEADER, DATE_CONTENTS, DATE_HEADER, FREE_TEXT, LOCATION_HEADER,
    PAGE_HEADER, PAGE_NUMBER, ROOM_LAYOUT_HEADER, T_HEADER, TAPE_LOCATION_HEADER,
    TIME_DURATION_CONTENTS, TIME_DURATION_HEADER, TIME_START_HEADER, TRANSCRIBER_HEADER,
    VIDEOS_HEADER, WARNING_HEADER,
};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Build `Header::Unknown` from malformed simple-header input.
fn unknown_header_from_node(
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
        text: model::WarningText::new(text),
        parse_reason: Some(reason.into()),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}

/// Outcome of reading a simple header's typed content slot.
///
/// This preserves, byte-for-byte, the pre-migration behaviour of
/// `get_required_content_by_kind` wrapped by each per-kind function's
/// `unknown_header_from_node` fallback: a `Present` slot with valid UTF-8 yields
/// the decoded content text (the caller builds `Header::Xxx`); every other slot
/// state has ALREADY reported the same diagnostic the old code emitted, and the
/// caller builds the `Header::Unknown` recovery fallback.
///
/// Shared across the simple-scalar family (this module) and the special family
/// (`dispatch/special.rs`), which reuses the identical content-slot reading
/// discipline for `@Number` / `@Birth of` / etc.; hence `pub(super)`.
pub(super) enum SimpleContent {
    /// `Present` content slot decoded to UTF-8 text.
    Decoded(String),
    /// Non-`Present` slot, or a `Present` slot whose bytes are not valid UTF-8:
    /// the matching diagnostic has been reported; the caller builds
    /// `Header::Unknown`.
    Fallback,
}

/// Read a simple header's content child from the typed, positional
/// `extract_<kind>(node).child_2` slot, reproducing `get_required_content_by_kind`'s
/// text + diagnostic handling EXACTLY.
///
/// `content_slot` is the `child_2` slot for the concrete header (e.g.
/// `NodeSlot<DateContentsNode>`); `header_actual` is the header node itself,
/// `content_kind`/`header_kind` are the node-type name constants used only to
/// build the preserved diagnostic message strings (NOT for `node.kind()`
/// dispatch). The `child_2` match is exhaustive over every `NodeSlot` variant;
/// there is deliberately no `_` catch-all that could silently drop a recovery
/// slot.
///
/// Shared with the special family (`dispatch/special.rs`): its single option
/// headers (`@Number`, `@Recording Quality`, `@Transcription`) and its
/// dual-content headers (`@Birth of`, `@Birthplace of`, `@L1 of`) reproduce the
/// pre-migration `get_required_content_by_kind(...).into_option()` behaviour
/// through this exact helper. Hence `pub(super)`.
pub(super) fn read_simple_content<'tree, T: AsRawNode<'tree>>(
    content_slot: NodeSlot<'tree, T>,
    header_actual: Node<'tree>,
    input: &str,
    content_kind: &str,
    header_kind: &str,
    errors: &impl ErrorSink,
) -> SimpleContent {
    match content_slot {
        NodeSlot::Present(content) => {
            // Decode through the shared `decode_present_child` helper, which reads
            // from the RAW node's `utf8_text` (NOT the wrapper's `.text()`
            // accessor, which swallows UTF-8 errors via `unwrap_or("")`),
            // reproducing `get_required_content_by_kind`'s Ok/Err handling
            // exactly. The family-specific diagnostic (context = `header_kind`,
            // the "text for {kind}" wording) is supplied here, so it stays
            // byte-identical to the pre-migration emission.
            match decode_present_child(content.raw_node(), input, errors, header_kind, |err| {
                format!("Failed to extract UTF-8 text for {}: {}", header_kind, err)
            }) {
                ParseOutcome::Parsed(text) => SimpleContent::Decoded(text),
                ParseOutcome::Rejected => SimpleContent::Fallback,
            }
        }
        // The pre-migration `find_child_by_kind` returned `None` for an
        // absent / missing / error / unexpected content child, all funnelling to
        // the SAME "missing content" diagnostic at the HEADER NODE span. Preserve
        // that exactly (NOT a position-precise diagnostic; a later task may
        // refine it).
        NodeSlot::Missing(_) | NodeSlot::Absent | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(header_actual.start_byte(), header_actual.end_byte()),
                ErrorContext::new(
                    input,
                    header_actual.start_byte()..header_actual.end_byte(),
                    header_kind,
                ),
                format!("Missing expected {} node in {}", content_kind, header_kind),
            ));
            SimpleContent::Fallback
        }
    }
}

/// `@Date` -> `Header::Date`.
pub(super) fn date(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // Grammar: date_contents = choice(strict_date, generic_date). Both
    // alternatives are wrapped in the date_contents node; we extract the text and
    // let ChatDate::new() classify or mark Unsupported. Validator reports E518 for
    // malformed dates.
    let children = extract_date_header(DateHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        DATE_CONTENTS,
        DATE_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(content) => ParseOutcome::parsed(Header::Date {
            date: model::ChatDate::new(content),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Date content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Tape Location` -> `Header::TapeLocation`.
pub(super) fn tape_location(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_tape_location_header(TapeLocationHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        TAPE_LOCATION_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(location) => ParseOutcome::parsed(Header::TapeLocation {
            location: model::TapeLocationDescription::new(location),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Tape Location content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Time Duration` -> `Header::TimeDuration`.
pub(super) fn time_duration(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // Grammar: time_duration_contents = choice(strict_time, generic_time).
    // TimeDurationValue::new() classifies or marks Unsupported. Validator reports
    // E541 for malformed durations.
    let children = extract_time_duration_header(TimeDurationHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        TIME_DURATION_CONTENTS,
        TIME_DURATION_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(duration) => ParseOutcome::parsed(Header::TimeDuration {
            duration: model::TimeDurationValue::new(duration),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Time Duration content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Time Start` -> `Header::TimeStart`.
pub(super) fn time_start(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // Same grammar pattern as TIME_DURATION_HEADER (the content node is also a
    // `time_duration_contents`). Validator reports E542 for malformed start times.
    let children = extract_time_start_header(TimeStartHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        TIME_DURATION_CONTENTS,
        TIME_START_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(start) => ParseOutcome::parsed(Header::TimeStart {
            start: model::TimeStartValue::new(start),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Time Start content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Location` -> `Header::Location`.
pub(super) fn location(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_location_header(LocationHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        LOCATION_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(location) => ParseOutcome::parsed(Header::Location {
            location: model::LocationDescription::new(location),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Location content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Room Layout` -> `Header::RoomLayout`.
pub(super) fn room_layout(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_room_layout_header(RoomLayoutHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        ROOM_LAYOUT_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(layout) => ParseOutcome::parsed(Header::RoomLayout {
            layout: model::RoomLayoutDescription::new(layout),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Room Layout content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Transcriber` -> `Header::Transcriber`.
pub(super) fn transcriber(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_transcriber_header(TranscriberHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        TRANSCRIBER_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(transcriber) => ParseOutcome::parsed(Header::Transcriber {
            transcriber: model::TranscriberName::new(transcriber),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Transcriber content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Warning` -> `Header::Warning`.
pub(super) fn warning(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_warning_header(WarningHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        WARNING_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(text) => ParseOutcome::parsed(Header::Warning {
            text: model::WarningText::new(text),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Warning content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Activities` -> `Header::Activities`.
pub(super) fn activities(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_activities_header(ActivitiesHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        ACTIVITIES_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(activities) => ParseOutcome::parsed(Header::Activities {
            activities: model::ActivitiesDescription::new(activities),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Activities content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Bck` -> `Header::Bck`.
pub(super) fn bck(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_bck_header(BckHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        BCK_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(bck) => ParseOutcome::parsed(Header::Bck {
            bck: model::BackgroundDescription::new(bck),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Bck content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Page` -> `Header::Page`.
pub(super) fn page(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_page_header(PageHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        PAGE_NUMBER,
        PAGE_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(page) => ParseOutcome::parsed(Header::Page {
            page: model::PageNumber::new(page),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Page number",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Videos` -> `Header::Videos`.
pub(super) fn videos(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_videos_header(VideosHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        VIDEOS_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(videos) => ParseOutcome::parsed(Header::Videos {
            videos: model::VideoSpec::new(videos),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Videos content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@T` -> `Header::T`.
pub(super) fn t(header_actual: Node, input: &str, errors: &impl ErrorSink) -> ParseOutcome<Header> {
    let children = extract_t_header(THeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        FREE_TEXT,
        T_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(text) => ParseOutcome::parsed(Header::T {
            text: model::TDescription::new(text),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @T content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@unsupported` catch-all -> `Header::Unknown`.
///
/// NOT migrated: unlike the 13 standard simple headers, this function does not
/// read a typed content child. It builds `Header::Unknown` from the whole header
/// node via `unknown_header_from_node` (the catch-all for unknown `@`-headers the
/// grammar matched structurally). Reading its `child_2` (`rest_of_line`) instead
/// would change the captured `text`, so the body is kept as-is, byte-identical.
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
