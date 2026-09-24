//! Parsing for `@Media` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>

use crate::generated_traversal::{
    AsRawNode, MediaHeaderNode, NoChild, SourceBound, SourceSlotView,
};

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::{
    ContentSlot, HeaderSite, read_source_content, surface_displaced,
};
use crate::parser::typed_cst::read_source_field;
use talkbank_model::model::{Header, MediaFilename, MediaHeader, MediaStatus, MediaType};

/// The fix every `@Media` recovery suggests.
const MEDIA_FIX: &str = "Expected @Media:\tfilename, audio|video[, status]";

/// Lower a producer-bound media header without accepting a separate source.
/// Generated projections retain association through the body and optional
/// status group. Each payload still admits its own range; recovery states and
/// the validated filename constructor remain independent obligations.
pub fn parse_media_header<'tree>(
    typed: SourceBound<'tree, '_, MediaHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Header {
    let source = typed.source();
    let site = HeaderSite::bound(typed);
    let node = site.actual();

    // Only a present body enters lowering; missing/error/absent body states
    // retain the existing header-level diagnostic and Unknown recovery.
    let header_children = typed.extract();
    let contents = match header_children.field_child_2().slot().view() {
        SourceSlotView::Present(contents) => match read_source_field(contents, errors) {
            Some(contents) => Some(contents),
            None => {
                surface_displaced(
                    &header_children.children().unexpected,
                    "media_header",
                    source,
                    errors,
                );
                return site.unknown(
                    "Unreadable media_contents in @Media header",
                    Some(MEDIA_FIX),
                );
            }
        },
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => {
            None
        }
    };
    let Some(contents) = contents else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "media_header"),
            "Missing media_contents in @Media header",
        ));
        surface_displaced(
            &header_children.children().unexpected,
            "media_header",
            source,
            errors,
        );
        return site.unknown("Missing media_contents in @Media header", Some(MEDIA_FIX));
    };
    surface_displaced(
        &header_children.children().unexpected,
        "media_header",
        source,
        errors,
    );

    // Child slots retain the same source as their admitted body.
    let contents_children = contents.extract();

    // Payload recovery reports once and retains Header::Unknown. Finite
    // fixture non-reachability is not proof that these states are impossible.
    let filename = match read_source_content(
        &site,
        contents_children.field_child_0().slot(),
        &ContentSlot {
            missing: "Missing media filename in @Media header",
            suggested_fix: Some(MEDIA_FIX),
        },
        errors,
    ) {
        Ok(text) => text,
        Err(refused) => return refused.into_header(&site),
    };

    // Whitespace between the filename and the comma, recorded as provenance
    // rather than reported here: E767 is a VALIDATION rule, so it fires for
    // every parser front end instead of only this one. child_1 is
    // `optional($.whitespaces)`.
    let whitespace_before_comma = match contents_children
        .field_child_1()
        .slot()
        .optional()
        .map(|slot| slot.view())
    {
        Some(SourceSlotView::Present(space_node)) => {
            let raw = space_node.raw_node();
            Some(crate::error::Span::new(
                raw.start_byte() as u32,
                raw.end_byte() as u32,
            ))
        }
        None
        | Some(
            SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild),
        ) => None,
    };

    // The type is `child_4`: the position moved twice as the grammar grew
    // its whitespace positions, and every value is accepted here
    // (`MediaType::from_text`); the validator names an unsupported one.
    let media_type = match read_source_content(
        &site,
        contents_children.field_child_4().slot(),
        &ContentSlot {
            missing: "Missing media type in @Media header",
            suggested_fix: Some(MEDIA_FIX),
        },
        errors,
    ) {
        Ok(text) => MediaType::from_text(text),
        Err(refused) => return refused.into_header(&site),
    };

    // Preserve the existing optional-group policy. The sequence slot cannot
    // contain a Missing node; Error/Absent still mean no status here, while
    // a present group's missing/error/absent payload takes Unknown recovery.
    // The file's structural recovery scan remains independently load-bearing.
    let status_group = match contents_children
        .field_child_5()
        .slot()
        .optional()
        .map(|slot| slot.view())
    {
        Some(SourceSlotView::Present(group)) => Some(group),
        None | Some(SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild)) => None,
    };
    let status = match status_group {
        Some(group) => match read_source_content(
            &site,
            group.field_child_2().slot(),
            &ContentSlot {
                missing: "Missing media status in @Media header",
                suggested_fix: Some(MEDIA_FIX),
            },
            errors,
        ) {
            Ok(text) => Some(MediaStatus::from_text(text)),
            Err(refused) => return refused.into_header(&site),
        },
        None => None,
    };
    if let Some(group) = status_group {
        for bad in group.field_unexpected().iter() {
            surface_displaced(
                std::slice::from_ref(&bad.raw_node()),
                "media_contents",
                bad.source(),
                errors,
            );
        }
    }

    surface_displaced(
        &contents_children.children().unexpected,
        "media_contents",
        source,
        errors,
    );

    // The grammar stops the filename at the comma, so a well-formed parse
    // always satisfies the invariant. Going through the checked constructor
    // anyway means a grammar change that broke that assumption surfaces as a
    // diagnostic here rather than as a header that serializes back wrong.
    let filename = match MediaFilename::parse(filename) {
        Ok(filename) => filename,
        Err(err) => {
            return site.unknown(format!("Invalid @Media filename: {err}"), Some(MEDIA_FIX));
        }
    };

    let mut media_header = MediaHeader::new(filename, media_type);
    if let Some(span) = whitespace_before_comma {
        media_header = media_header.with_whitespace_before_comma(span);
    }
    if let Some(s) = status {
        media_header = media_header.with_status(s);
    }
    Header::Media(media_header)
}
