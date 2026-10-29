//! Prefix and opaque suffix decoding from the producing word's bound tokens.
//! Source ownership is retained through policy and diagnostic construction;
//! lexical/registry validity remains separate from canonical tree admission.

use super::super::content::report_tree_shape;
use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AsRawNode, FormMarkerNode, NamedKind, PosTagNode, RepeatedFormMarkerNode, SourceBound,
    SourceBoundKind, WordPrefixNode,
};
use crate::parser::node_span::span_of;
use talkbank_model::content::word::{FormMarkerPayload, FormType, WordCategory, WordPosTag};

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
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(token.source(), span_of(node), T::KIND.name()),
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

/// The grammar keeps `@` and any label in one token. Registry policy owns
/// declared codes and label requirements. Undeclared payloads stay verbatim;
/// model validation owns E203 for this explicit recovery state.
pub(super) fn form_marker<'tree>(
    token: SourceBound<'tree, '_, FormMarkerNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<FormType> {
    let payload = after_marker(token, errors)?;
    Some(
        match FormType::from_payload(FormMarkerPayload::after_at(payload)) {
            Ok(declared) => declared,
            Err(undeclared) => FormType::Undeclared(undeclared.into_payload()),
        },
    )
}

/// Retain the authored repeated run for model-owned E203 and lossless rendering.
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
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(
                token.source(),
                span_of(node),
                RepeatedFormMarkerNode::KIND.name(),
            ),
            format!("repeated form marker token has no leading '@': {text:?}"),
        ));
        return None;
    };
    Some(FormType::Undeclared(payload.to_owned()))
}

/// POS labels are opaque here; marker admission does not invent a vocabulary.
pub(super) fn part_of_speech<'tree>(
    token: SourceBound<'tree, '_, PosTagNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<WordPosTag> {
    let payload = after_marker(token, errors)?;
    match WordPosTag::new(payload) {
        Ok(tag) => Some(tag),
        Err(error) => {
            report_tree_shape(token.raw_node(), error.to_string(), token.source(), errors);
            None
        }
    }
}
