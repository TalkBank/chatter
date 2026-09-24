//! Shared typed-CST seam for the production parser.
//!
//! One region-neutral primitive lives here so that every parser region (the
//! header parsers in `chat_file_parser/header_parser/dispatch/` and
//! `tree_parsing/header/`, and the utterance / dependent-tier / content parsers
//! in `chat_file_parser/` and `tree_parsing/main_tier/`) reaches ONE shared seam
//! instead of each module subtree reimplementing it:
//!
//! - [`decode_present_child`], the single helper that decodes a `Present`
//!   content child's bytes to text and reports the caller's family-specific
//!   diagnostic on a UTF-8 error. It replaces the duplicated decode logic that
//!   each header family carried (`read_simple_content`, `decode_child_text`,
//!   `read_types_field`, `decode_field_text`, and the inline `@Situation`
//!   decode).
//!
//! (The former `TypedTraversal` ZST receiver for the OLD `GrammarTraversal`
//! trait visitor was retired in the 2026-07 visitor migration once every region
//! flipped onto the NEW self-contained backend's free `extract_*` functions; the
//! parser no longer routes any structure through a trait receiver.)

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, SourceBindingError, SourceBound, SourceBoundKind, SourceField,
};
use std::str::Utf8Error;
use talkbank_model::ParseOutcome;

/// Report source admission failures at the boundary that still owns the node.
pub(in crate::parser) fn report_source_binding_error(
    node: tree_sitter::Node<'_>,
    source: &str,
    error: SourceBindingError,
    errors: &impl ErrorSink,
) {
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.byte_range(), node.kind()),
        error.to_string(),
    ));
}

/// Admit an associated typed field's own range; source identity is already
/// guaranteed by the generated projection. No independent text can be supplied.
pub(in crate::parser) fn read_source_field<'tree, 'source, T: SourceBoundKind<'tree>>(
    field: SourceField<'_, 'tree, 'source, T>,
    errors: &impl ErrorSink,
) -> Option<SourceBound<'tree, 'source, T>> {
    match field.read() {
        Ok(bound) => Some(bound),
        Err(error) => {
            report_source_binding_error(field.raw_node(), field.source(), error, errors);
            None
        }
    }
}

/// Failure at the transitional typed-node/source compatibility boundary.
#[derive(Debug, thiserror::Error)]
pub(in crate::parser) enum NodeTextError {
    /// The source cannot supply the node's byte range.
    #[error("node byte range is outside the supplied source")]
    OutsideSource,
    /// The range cuts across a UTF-8 code point in the supplied source.
    #[error(transparent)]
    InvalidUtf8(#[from] Utf8Error),
}

/// Admit a node range before decoding it. The result distinguishes an absent
/// byte range from a range that cuts a UTF-8 code point; success borrows only
/// the checked slice. This proves readability, not tree/source identity.
pub(in crate::parser) fn admit_node_text<'source>(
    node: tree_sitter::Node<'_>,
    source: &'source str,
) -> Result<&'source str, NodeTextError> {
    let bytes = source
        .as_bytes()
        .get(node.byte_range())
        .ok_or(NodeTextError::OutsideSource)?;
    std::str::from_utf8(bytes).map_err(NodeTextError::from)
}

/// Decode a `Present` content child's bytes to owned text, reporting the caller's
/// family-specific diagnostic on a UTF-8 error.
///
/// This is the ONE shared form of the "decode a typed content child, emit the
/// family diagnostic on a `utf8_text` `Err`" logic that the header families
/// previously reimplemented (`read_simple_content`, `decode_child_text`,
/// `read_types_field`, `decode_field_text`, and the inline `@Situation` decode).
/// The decode itself and the `Ok` / `Err` discipline are identical across
/// families; only the reported diagnostic differs, so the family supplies it:
///
/// - `context` is the [`ErrorContext`] node-kind / label string for the family
///   (e.g. the header kind, the field label, or `"id_contents"` /
///   `"situation_text"`).
/// - `make_message` builds the family's message from the [`NodeTextError`]. It
///   is invoked ONLY on the error path, so every call site reproduces its
///   pre-extraction message byte-for-byte (the families deliberately phrase it
///   differently: "text for `<kind>`", "text from `<label>`", a bare "text", and
///   the `@Situation`-specific wording).
///
/// Retains the caller's typed node until the read boundary. A checked byte range
/// precedes UTF-8 admission, so mismatched source ranges reject without indexing
/// panics or fabricated text. This does not prove tree/source identity; callers
/// with a producer-owned source slice should read that slice directly instead.
pub(in crate::parser) fn decode_present_child<'tree, T: AsRawNode<'tree>>(
    typed: &T,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
    make_message: impl FnOnce(NodeTextError) -> String,
) -> ParseOutcome<String> {
    match read_present_child(typed, source, context, make_message) {
        Ok(text) => ParseOutcome::parsed(text),
        Err(error) => {
            errors.report(error);
            ParseOutcome::rejected()
        }
    }
}

/// Decode while retaining the required failure diagnostic for a typed caller.
/// The streaming adapter above consumes the same result, so both routes share
/// range checks, UTF-8 admission and diagnostic construction.
#[allow(clippy::result_large_err)] // Transfer the diagnostic directly to ErrorSink without a heap allocation.
pub(in crate::parser) fn read_present_child<'tree, T: AsRawNode<'tree>>(
    typed: &T,
    source: &str,
    context: &str,
    make_message: impl FnOnce(NodeTextError) -> String,
) -> Result<String, ParseError> {
    let node = typed.raw_node();
    match admit_node_text(node, source) {
        Ok(text) => Ok(text.to_string()),
        Err(err) => Err(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), context),
            make_message(err),
        )),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::{FromNodeKind, LanguageCodeNode};
    use talkbank_model::ErrorCollector;

    #[test]
    fn typed_text_boundary_distinguishes_range_and_utf8_refusals() {
        let source = include_str!("../../../../corpus/reference/tiers/mor-gra.cha");
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut codes = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(code) = LanguageCodeNode::from_node(node) else {
                continue;
            };
            codes += 1;
            let errors = ErrorCollector::new();
            assert_eq!(
                decode_present_child(&code, source, &errors, "language_code", |e| e.to_string())
                    .into_option()
                    .as_deref(),
                Some("eng"),
            );
            assert!(errors.to_vec().is_empty());
            assert!(
                decode_present_child(&code, "", &errors, "language_code", |error| {
                    assert!(matches!(error, NodeTextError::OutsideSource));
                    error.to_string()
                })
                .is_none()
            );

            // Deliberately pair this real three-byte token with different
            // source bytes: its end offset now bisects a UTF-8 code point.
            // This is a source-pairing boundary test, not a CHAT fixture.
            let mut foreign = source.to_owned();
            foreign.replace_range(node.byte_range(), "λλ");
            assert!(
                decode_present_child(&code, &foreign, &errors, "language_code", |error| {
                    assert!(matches!(error, NodeTextError::InvalidUtf8(_)));
                    error.to_string()
                })
                .is_none()
            );
            let diagnostics = errors.into_vec();
            assert_eq!(diagnostics.len(), 2);
            assert!(
                diagnostics
                    .iter()
                    .all(|error| error.code == ErrorCode::TreeParsingError)
            );
        }
        // One @Languages code and one in each of the two @ID headers.
        assert_eq!(codes, 3);
    }
}
