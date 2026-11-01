//! Parsing for `@Media` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>

use crate::generated_traversal::{AsRawNode, MediaHeaderNode, ReadableSlot, SourceBound};

use crate::error::ErrorSink;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::{
    Body, ContentSlot, HeaderSite, read_admitted_content, read_body, surface_displaced,
};
use crate::parser::typed_cst::{CstFailure, ExtractCanonical};
use talkbank_model::ParseOutcome;
use talkbank_model::model::{
    Header, MediaFilename, MediaHeader, MediaStatus, MediaType, PlainKind, StructuredKind,
};

/// The fix every `@Media` recovery suggests.
const MEDIA_FIX: &str = "Expected @Media:\tfilename, audio|video[, status]";

/// Lower a producer-bound media header without accepting a separate source.
/// Generated projections retain association through the body and optional
/// status group. Body ranges are admitted together; recovery states and
/// the validated filename constructor remain independent obligations.
pub fn parse_media_header<'tree>(
    typed: SourceBound<'tree, '_, MediaHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Header>, CstFailure> {
    let source = typed.source();
    let site = HeaderSite::bound(typed);

    // The body is optional in the grammar: an omitted one is the one
    // empty-content state, refused by validation (E553). A present body is
    // not proof of a valid filename, media kind or other nested field. An
    // ERROR where the body belongs is unparsable text, the whole-tree
    // backstop's to report, and the line is kept as an unknown header with
    // its text.
    let header_children = crate::parser::typed_cst::extract_envelope(typed)?;
    surface_displaced(
        &header_children.children().unexpected,
        "media_header",
        source,
        errors,
    );
    let words = ContentSlot {
        missing: "Missing @Media content",
        suggested_fix: Some(MEDIA_FIX),
    };
    let contents = match read_body(
        &site,
        header_children.field_child_2().slot(),
        &words,
        errors,
    ) {
        Ok(Body::Empty(separator)) => {
            return Ok(ParseOutcome::Parsed(
                PlainKind::Structured(StructuredKind::Media).empty(separator),
            ));
        }
        Ok(Body::Written { body, .. }) => body.into_inner(),
        Err(failure) => return Ok(failure.into_outcome(&site, errors)),
    };

    // Child slots retain the same source as their admitted body. Read under
    // the admitted kind proof: the filename, type and status kinds are
    // narrowed, so none of them can be MISSING.
    let admitted = contents.extract_canonical()?.admit_ranges()?;
    let contents_children = admitted.children();

    // Payload recovery reports once and retains Header::Unknown. Finite
    // fixture non-reachability is not proof that these states are impossible.
    let filename = match read_admitted_content(
        &site,
        &contents_children.child_0.slot,
        &ContentSlot {
            missing: "Missing media filename in @Media header",
            suggested_fix: Some(MEDIA_FIX),
        },
        errors,
    ) {
        Ok(text) => text,
        Err(failure) => return Ok(ParseOutcome::Parsed(failure.into_header())),
    };

    // Whitespace between the filename and the comma, recorded as provenance
    // rather than reported here: E767 is a VALIDATION rule, so it fires for
    // every parser front end instead of only this one. child_1 is
    // `optional($.whitespaces)`.
    let whitespace_before_comma = match contents_children.child_1.slot.as_ref() {
        Some(ReadableSlot::Present(space_node)) => {
            let raw = space_node.raw_node();
            Some(span_of(raw))
        }
        None | Some(ReadableSlot::Missing(_) | ReadableSlot::Error(_)) => None,
        Some(ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never)) => match *never {},
    };

    // The type is `child_4`: the position moved twice as the grammar grew
    // its whitespace positions, and every value is accepted here
    // (`MediaType::from_text`); the validator names an unsupported one.
    let media_type = match read_admitted_content(
        &site,
        &contents_children.child_4.slot,
        &ContentSlot {
            missing: "Missing media type in @Media header",
            suggested_fix: Some(MEDIA_FIX),
        },
        errors,
    ) {
        Ok(text) => match MediaType::try_new(text) {
            Ok(media_type) => media_type,
            Err(_) => {
                return Ok(blank_field(
                    &site,
                    "Blank media type in @Media header",
                    errors,
                ));
            }
        },
        Err(failure) => return Ok(ParseOutcome::Parsed(failure.into_header())),
    };

    // Preserve the existing optional-group policy. The sequence slot cannot
    // contain a Missing node; Error/Absent still mean no status here, while
    // a present group's missing/error/absent payload takes Unknown recovery.
    // The file's structural recovery scan remains independently load-bearing.
    let status_group = match contents_children.child_5.slot.as_ref() {
        Some(ReadableSlot::Present(group)) => Some(group),
        None | Some(ReadableSlot::Error(_)) => None,
        Some(
            ReadableSlot::Missing(never)
            | ReadableSlot::Unexpected(never)
            | ReadableSlot::Absent(never),
        ) => match *never {},
    };
    let status = match status_group {
        Some(group) => match read_admitted_content(
            &site,
            &group.child_2.slot,
            &ContentSlot {
                missing: "Missing media status in @Media header",
                suggested_fix: Some(MEDIA_FIX),
            },
            errors,
        ) {
            Ok(text) => match MediaStatus::try_new(text) {
                Ok(status) => Some(status),
                Err(_) => {
                    return Ok(blank_field(
                        &site,
                        "Blank media status in @Media header",
                        errors,
                    ));
                }
            },
            Err(failure) => return Ok(ParseOutcome::Parsed(failure.into_header())),
        },
        None => None,
    };
    if let Some(group) = status_group {
        for bad in &group.unexpected {
            surface_displaced(
                std::slice::from_ref(&bad.raw_node()),
                "media_contents",
                bad.source(),
                errors,
            );
        }
    }

    for bad in &contents_children.unexpected {
        surface_displaced(
            std::slice::from_ref(&bad.raw_node()),
            "media_contents",
            source,
            errors,
        );
    }

    // The grammar stops the filename at the comma, so a well-formed parse
    // always satisfies the invariant. Going through the checked constructor
    // anyway means a grammar change that broke that assumption surfaces as a
    // diagnostic here rather than as a header that serializes back wrong.
    let filename = match MediaFilename::parse(filename) {
        Ok(filename) => filename,
        Err(err) => {
            return Ok(ParseOutcome::Parsed(site.unknown(
                format!("Invalid @Media filename: {err}"),
                Some(MEDIA_FIX),
            )));
        }
    };

    let mut media_header = MediaHeader::new(filename, media_type);
    if let Some(span) = whitespace_before_comma {
        media_header = media_header.with_whitespace_before_comma(span);
    }
    if let Some(s) = status {
        media_header = media_header.with_status(s);
    }
    Ok(ParseOutcome::Parsed(Header::Media(media_header)))
}

/// A `@Media` field that is present but blank (a trailing comma with nothing
/// after it, which the grammar holds as a zero-width field rather than a
/// recovery node): a malformed header, E316 and an unknown header, exactly
/// as the re2c field combinator refuses it. Never an unsupported value named
/// `""`.
fn blank_field(
    site: &HeaderSite<'_, '_>,
    reason: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    errors.report(talkbank_model::ParseError::at_span(
        talkbank_model::ErrorCode::UnparsableContent,
        talkbank_model::Severity::Error,
        span_of(site.actual()),
        reason,
    ));
    ParseOutcome::Parsed(site.unknown(reason, Some(MEDIA_FIX)))
}
