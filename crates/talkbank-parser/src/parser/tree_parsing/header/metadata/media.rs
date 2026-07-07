//! Parsing for `@Media` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>

use crate::generated_traversal::{
    AsRawNode, MediaContentsNode, MediaHeaderNode, NodeSlot, extract_media_contents,
    extract_media_header,
};
use crate::node_types::MEDIA_HEADER;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, MediaHeader, MediaStatus, MediaType, WarningText};

/// Build `Header::Unknown` for malformed `@Media` input.
fn unknown_media_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@Media".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @Media:\tfilename, audio|video[, status]".to_string()),
    }
}

/// Decode UTF-8 child text for media header fields, delegating to the shared
/// `decode_present_child` helper.
///
/// The per-field diagnostic (context = the `media_*` field label, message
/// "Failed to extract UTF-8 text from `<context>`: `<err>`") is supplied here, so
/// it stays byte-identical to the pre-extraction emission.
fn decode_child_text(
    child: Node,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> ParseOutcome<String> {
    decode_present_child(child, source, errors, context, |err| {
        format!("Failed to extract UTF-8 text from {}: {}", context, err)
    })
}

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
pub fn parse_media_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is a media_header node.
    if node.kind() != MEDIA_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected media_header node, got: {}", node.kind()),
        ));
        return unknown_media_header(node, source, "Media header CST node had unexpected kind");
    }

    // Extract media_contents via typed slot `child_2` of the media_header.
    // `extract_media_header` exposes `media_contents` as a `NodeSlot`;
    // `present_or_recover().ok()` keeps only a Present media_contents; every
    // non-Present recovery state funnels to the same "Missing media_contents"
    // diagnostic + Header::Unknown.
    let header_children = extract_media_header(MediaHeaderNode(node));
    let Some(contents) = header_children.child_2.slot.present_or_recover().ok() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "media_header"),
            "Missing media_contents in @Media header",
        ));
        surface_unexpected(&header_children.unexpected, source, errors);
        return unknown_media_header(node, source, "Missing media_contents in @Media header");
    };
    let contents_raw = contents.raw_node();
    surface_unexpected(&header_children.unexpected, source, errors);

    // Decompose the media_contents node into its typed child slots. The index
    // remap from the OLD (whitespace-skipped) module is documented in the
    // function doc-comment above.
    let contents_children = extract_media_contents(MediaContentsNode(contents_raw));

    // Extract filename from typed child_0 (unchanged index).
    // All values accepted via decode_child_text(); the validator flags semantic issues.
    let filename = match contents_children.child_0.slot {
        // Happy path: correct node kind, decode its UTF-8 text via raw_node().
        NodeSlot::Present(filename_node) => {
            match decode_child_text(filename_node.raw_node(), source, errors, "media_filename") {
                ParseOutcome::Parsed(text) => text,
                ParseOutcome::Rejected => {
                    return unknown_media_header(node, source, "Could not decode @Media filename");
                }
            }
        }
        // Wrong-kind node at this position: reproduce the "got {kind}" diagnostic
        // from the pre-migration `child.kind() != MEDIA_FILENAME` branch.
        NodeSlot::Unexpected(child) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(
                    source,
                    child.start_byte()..child.end_byte(),
                    "media_filename",
                ),
                format!(
                    "Expected media_filename node at @Media content position 0, got {}",
                    child.kind()
                ),
            ));
            return unknown_media_header(node, source, "Missing media filename in @Media header");
        }
        // No usable node at this position: reproduce the "Missing media_filename"
        // diagnostic from the pre-migration `contents.child(0u32)` None branch.
        NodeSlot::Missing(_) | NodeSlot::Absent | NodeSlot::Error(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(contents_raw.start_byte(), contents_raw.end_byte()),
                ErrorContext::new(
                    source,
                    contents_raw.start_byte()..contents_raw.end_byte(),
                    "media_contents",
                ),
                "Missing media_filename node in @Media header",
            ));
            return unknown_media_header(node, source, "Missing media filename in @Media header");
        }
    };

    // Extract media_type from typed child_3 (moved from child_2 pre-B2: the NEW
    // backend models the `whitespaces` between the comma and media_type as its
    // own child_2 position). All values accepted via MediaType::from_text();
    // unsupported ones are flagged by the validator.
    let media_type = match contents_children.child_3.slot {
        // Happy path: correct node kind, decode its UTF-8 text.
        NodeSlot::Present(type_node) => {
            let ParseOutcome::Parsed(type_text) =
                decode_child_text(type_node.raw_node(), source, errors, "media_type")
            else {
                return unknown_media_header(node, source, "Could not decode @Media type");
            };
            MediaType::from_text(&type_text)
        }
        // Wrong-kind node: reproduce the "got {kind}" diagnostic.
        NodeSlot::Unexpected(child) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), "media_type"),
                format!(
                    "Expected media_type node at @Media content position 3, got {}",
                    child.kind()
                ),
            ));
            return unknown_media_header(node, source, "Missing media type in @Media header");
        }
        // No usable node: reproduce the "Missing media_type" diagnostic.
        NodeSlot::Missing(_) | NodeSlot::Absent | NodeSlot::Error(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(contents_raw.start_byte(), contents_raw.end_byte()),
                ErrorContext::new(
                    source,
                    contents_raw.start_byte()..contents_raw.end_byte(),
                    "media_contents",
                ),
                "Missing media_type node in @Media header",
            ));
            return unknown_media_header(node, source, "Missing media type in @Media header");
        }
    };

    // Extract optional status from typed child_4, now a GROUP
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
        .child_4
        .slot
        .and_then(|s| s.present_or_recover().ok());
    let status = match status_group
        .as_ref()
        .and_then(|group| group.child_2.slot.clone().present_or_recover().ok())
    {
        Some(status_node) => {
            let ParseOutcome::Parsed(status_text) =
                decode_child_text(status_node.raw_node(), source, errors, "media_status")
            else {
                return unknown_media_header(node, source, "Could not decode @Media status");
            };
            Some(MediaStatus::from_text(&status_text))
        }
        None => None,
    };
    if let Some(group) = &status_group {
        surface_unexpected(&group.unexpected, source, errors);
    }

    surface_unexpected(&contents_children.unexpected, source, errors);

    let mut media_header = MediaHeader::new(filename, media_type);
    if let Some(s) = status {
        media_header = media_header.with_status(s);
    }
    Header::Media(media_header)
}
