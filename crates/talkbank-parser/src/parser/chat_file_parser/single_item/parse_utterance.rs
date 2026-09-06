//! Parse isolated utterances via whole-file recovery plus extraction.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::TreeSitterParser;
use super::helpers::{MINIMAL_CHAT_PREFIX, MINIMAL_CHAT_SUFFIX};
use crate::api::fragment::WrappedFragment;
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ParseError, ParseErrors, ParseResult, Severity,
    SourceLocation,
};
use crate::generated_traversal::{
    FromNodeKind, NodeSlot, SourceFileChoice, SourceFileNode, extract_source_file,
};
use crate::model::Line;
use crate::model::Utterance;

/// Parse one utterance fragment into `Utterance`.
///
/// This path intentionally reuses whole-file recovery so fragment parsing stays
/// aligned with normal utterance construction, including preceding headers and
/// attached dependent tiers.
pub(super) fn parse_utterance(parser: &TreeSitterParser, input: &str) -> ParseResult<Utterance> {
    let input_with_newline = if input.as_bytes().last().is_some_and(|b| *b == b'\n') {
        input.to_string()
    } else {
        format!("{}\n", input)
    };

    let is_full_chat = parser
        .parser
        .borrow_mut()
        .parse(&input_with_newline, None)
        .is_some_and(|tree| {
            SourceFileNode::from_node(tree.root_node()).is_some_and(|root| {
                matches!(
                    extract_source_file(root).content.slot(),
                    NodeSlot::Present(SourceFileChoice::FullDocument(_))
                )
            })
        });

    let newline = if input.ends_with('\n') { "" } else { "\n" };
    let suffix = if is_full_chat {
        newline.to_owned()
    } else {
        format!("{newline}{MINIMAL_CHAT_SUFFIX}")
    };
    let prefixes: &[&str] = if is_full_chat {
        &[]
    } else {
        &[MINIMAL_CHAT_PREFIX]
    };
    let fragment = WrappedFragment::new(prefixes, input, &suffix, 0);
    let errors_sink = ErrorCollector::new();
    let file =
        parser.parse_chat_file_streaming(fragment.source(), &fragment.error_sink(&errors_sink));
    let parse_errors = errors_sink.into_vec();
    if !parse_errors.is_empty() {
        return Err(ParseErrors::from(parse_errors));
    }

    let utterance = file
        .lines
        .into_iter()
        .find_map(|line| match line {
            Line::Utterance(utterance) => Some(*utterance),
            _ => None,
        })
        .ok_or_else(|| {
            let mut errors = ParseErrors::new();
            errors.push(ParseError::new(
                ErrorCode::MissingMainTier,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "No utterance found in parsed output",
            ));
            errors
        })?;

    Ok(fragment.rebase(utterance))
}
