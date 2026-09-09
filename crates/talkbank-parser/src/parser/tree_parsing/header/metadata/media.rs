//! Parsing for `@Media` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>

use crate::generated_traversal::{
    AsRawNode, MediaHeaderNode, NodeSlot, extract_media_contents, extract_media_header,
};

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::{
    ContentSlot, HeaderSite, present, read_simple_content, surface_displaced,
};
use talkbank_model::model::{Header, MediaFilename, MediaHeader, MediaStatus, MediaType};

/// The fix every `@Media` recovery suggests.
const MEDIA_FIX: &str = "Expected @Media:\tfilename, audio|video[, status]";

/// Parse Media header from tree-sitter node.
///
/// **Grammar Rule (structural children; the NEW backend does NOT skip
/// whitespace, so `comma`/`whitespaces` are real, though unused-here,
/// positions):**
/// ```javascript
/// media_header: $ => seq(
///     media_prefix,    // child_0 (structural)
///     header_sep,      // child_1 (structural)
///     media_contents,  // child_2 <-- payload (unchanged index)
///     newline          // child_3 (structural)
/// )
///
/// media_contents: $ => seq(
///     media_filename,               // typed child_0 <-- payload
///     comma,                        // typed child_1 (structural)
///     whitespaces,                  // typed child_2 (structural, NEW: not skipped)
///     media_type,                   // typed child_3 <-- payload (was child_2 pre-B2)
///     optional(seq(                 // typed child_4 (Option GROUP, was flat
///         comma,                    //   optional child_3 + child_4 pre-B2)
///         whitespaces,
///         media_status,             // group.child_2 <-- payload
///     )),
/// )
/// ```
///
/// The field-index remap from the pre-B2 (OLD-module, whitespace-skipped)
/// shape is: `media_filename` stays `child_0`; `media_type` moves from
/// `child_2` to `child_3`; `media_status` moves from a flat optional
/// `child_4` to `child_4`'s nested GROUP `child_2` (the whole
/// `comma+whitespaces+media_status` triple is what is optional at the
/// grammar level, not `media_status` alone).
pub fn parse_media_header(
    typed: MediaHeaderNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Header {
    let site = HeaderSite::of(&typed, source);
    let node = site.actual();

    // Extract media_contents via typed slot `child_2` of the media_header.
    // `extract_media_header` exposes `media_contents` as a `NodeSlot`;
    // `present_or_recover().ok()` keeps only a Present media_contents; every
    // non-Present recovery state funnels to the same "Missing media_contents"
    // diagnostic + Header::Unknown.
    let header_children = extract_media_header(typed);
    let Some(contents) = present(header_children.child_2.slot()) else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "media_header"),
            "Missing media_contents in @Media header",
        ));
        surface_displaced(&header_children.unexpected, "media_header", source, errors);
        return site.unknown("Missing media_contents in @Media header", Some(MEDIA_FIX));
    };
    surface_displaced(&header_children.unexpected, "media_header", source, errors);

    // Decompose the media_contents node into its typed child slots. The index
    // remap from the OLD (whitespace-skipped) module is documented in the
    // function doc-comment above.
    let contents_children = extract_media_contents(*contents);

    // The three payload slots read through the shared verb; every recovery
    // state of a slot reports once at the header node and hands back the
    // `Header::Unknown` for it. None of those states is reachable from CHAT:
    // a malformed `@Media` line is a file-level ERROR node and never reaches
    // this parser, which the 2026-09-08 coverage run showed for every header
    // family. Until that day this file wrote its own three matches.
    let filename = match read_simple_content(
        &site,
        contents_children.child_0.slot(),
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
    let whitespace_before_comma = match contents_children.child_1.slot() {
        Some(NodeSlot::Present(space_node)) => {
            let raw = space_node.raw_node();
            Some(crate::error::Span::new(
                raw.start_byte() as u32,
                raw.end_byte() as u32,
            ))
        }
        _ => None,
    };

    // The type is `child_4`: the position moved twice as the grammar grew
    // its whitespace positions, and every value is accepted here
    // (`MediaType::from_text`); the validator names an unsupported one.
    let media_type = match read_simple_content(
        &site,
        contents_children.child_4.slot(),
        &ContentSlot {
            missing: "Missing media type in @Media header",
            suggested_fix: Some(MEDIA_FIX),
        },
        errors,
    ) {
        Ok(text) => MediaType::from_text(&text),
        Err(refused) => return refused.into_header(&site),
    };

    // Extract optional status from typed child_5, a GROUP
    // (`Option<NodeSlot<MediaContentsChild4Children>>`) around the whole
    // `comma + whitespaces + media_status` triple (the OLD module's flat
    // `Option<NodeSlot<MediaStatusNode>>` no longer applies since whitespace is
    // not skipped: the group, not media_status alone, is what is optional at the
    // grammar level). Descending one level (`group.child_2.slot`) to the group's
    // own media_status member and collapsing every non-Present state at EITHER
    // level (outer None = group grammar-absent; inner non-Present = malformed
    // group) to `None` reproduces the OLD `and_then(NodeSlot::into_ok)` collapse
    // exactly for the VALID path.
    let status_group = contents_children
        .child_5
        .slot()
        .as_ref()
        .and_then(|slot| present(slot));
    let status = match status_group {
        Some(group) => match read_simple_content(
            &site,
            group.child_2.slot(),
            &ContentSlot {
                missing: "Missing media status in @Media header",
                suggested_fix: Some(MEDIA_FIX),
            },
            errors,
        ) {
            Ok(text) => Some(MediaStatus::from_text(&text)),
            Err(refused) => return refused.into_header(&site),
        },
        None => None,
    };
    if let Some(group) = status_group {
        surface_displaced(&group.unexpected, "media_contents", source, errors);
    }

    surface_displaced(
        &contents_children.unexpected,
        "media_contents",
        source,
        errors,
    );

    // The grammar stops the filename at the comma, so a well-formed parse
    // always satisfies the invariant. Going through the checked constructor
    // anyway means a grammar change that broke that assumption surfaces as a
    // diagnostic here rather than as a header that serializes back wrong.
    let filename = match MediaFilename::parse(&filename) {
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
