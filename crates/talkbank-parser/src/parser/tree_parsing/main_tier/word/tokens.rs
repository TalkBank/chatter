//! Prefix and opaque suffix decoding from the producing word's bound tokens.
//! Source ownership is retained through policy and diagnostic construction;
//! lexical/registry validity remains separate from canonical tree admission.

use super::super::content::report_tree_shape;
use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, FormMarkerNode, NamedKind, PosTagNode, RepeatedFormMarkerNode, SourceBound,
    SourceBoundKind, WordPrefixNode,
};
use talkbank_model::content::word::{FormMarkerPayload, FormType, WordCategory};

/// A closed association between a generated token kind and its leading marker.
/// Neither the source, marker nor diagnostic label is supplied independently.
trait MarkedToken<'tree>: SourceBoundKind<'tree> + NamedKind {
    const MARKER: &'static str;
    const LABEL: &'static str;
}

impl<'tree> MarkedToken<'tree> for FormMarkerNode<'tree> {
    const MARKER: &'static str = "@";
    const LABEL: &'static str = "Form marker";
}

impl<'tree> MarkedToken<'tree> for PosTagNode<'tree> {
    const MARKER: &'static str = "$";
    const LABEL: &'static str = "Part-of-speech tag";
}

fn after_marker<'tree, 'source, T: MarkedToken<'tree>>(
    token: SourceBound<'tree, 'source, T>,
    errors: &impl ErrorSink,
) -> Option<&'source str> {
    let Some(payload) = token.text().strip_prefix(T::MARKER) else {
        let node = token.raw_node();
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(token.source(), node.byte_range(), T::KIND),
            format!("{} does not begin with '{}'", T::LABEL, T::MARKER),
        ));
        return None;
    };
    Some(payload)
}

pub(super) fn word_category<'tree>(
    token: SourceBound<'tree, '_, WordPrefixNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<WordCategory> {
    match token.text() {
        "&-" => Some(WordCategory::Filler),
        "&~" => Some(WordCategory::Nonword),
        "&+" => Some(WordCategory::PhonologicalFragment),
        other => {
            report_tree_shape(
                token.raw_node(),
                format!("Unknown word prefix {other:?}"),
                token.source(),
                errors,
            );
            None
        }
    }
}

fn at_suffix_error<'tree, T: SourceBoundKind<'tree>>(
    token: SourceBound<'tree, '_, T>,
    message: String,
    suggestion: &'static str,
) -> ParseError {
    let node = token.raw_node();
    ParseError::new(
        ErrorCode::InvalidFormType,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(token.source(), node.byte_range(), ""),
        message,
    )
    .with_suggestion(suggestion)
}

/// The grammar keeps `@` and any label in one token. Registry policy owns
/// declared codes and label requirements. Undeclared payloads stay verbatim;
/// they must not become a valid `@z:label` or duplicate the model's E203.
pub(super) fn form_marker<'tree>(
    token: SourceBound<'tree, '_, FormMarkerNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<FormType> {
    let payload = after_marker(token, errors)?;
    Some(
        match FormType::from_payload(FormMarkerPayload::after_at(payload)) {
            Ok(declared) => declared,
            Err(undeclared) => {
                errors.report(at_suffix_error(
                    token,
                    format!("Undeclared form marker '@{}'", undeclared.payload()),
                    undeclared.suggestion(),
                ));
                FormType::Undeclared(undeclared.into_payload())
            }
        },
    )
}

/// E203 names the whole repeated run, not a nonexistent individual marker.
/// Retain the authored payload so serialization cannot silently repair it.
pub(super) fn repeated_form_marker<'tree>(
    token: SourceBound<'tree, '_, RepeatedFormMarkerNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<FormType> {
    let text = token.text();
    let Some(payload) = text.strip_prefix('@') else {
        let node = token.raw_node();
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(
                token.source(),
                node.byte_range(),
                RepeatedFormMarkerNode::KIND,
            ),
            format!("repeated form marker token has no leading '@': {text:?}"),
        ));
        return None;
    };
    errors.report(at_suffix_error(
        token,
        format!("A word may carry only one '@' suffix, found '{text}'"),
        "Keep one marker on the word and remove the rest",
    ));
    Some(FormType::Undeclared(payload.to_owned()))
}

/// POS labels are opaque here; marker admission does not invent a vocabulary.
pub(super) fn part_of_speech<'tree>(
    token: SourceBound<'tree, '_, PosTagNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<smol_str::SmolStr> {
    after_marker(token, errors).map(smol_str::SmolStr::from)
}
