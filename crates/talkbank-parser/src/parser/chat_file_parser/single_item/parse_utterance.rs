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

/// The probe's decision retains the exact input it classified. A complete
/// document already owns its admitted envelope; only fragments need scaffolding.
enum UtteranceInput<'input> {
    Complete(WrappedFragment<'input>),
    NeedsWrapper {
        input: &'input str,
        newline: &'static str,
    },
}

impl<'input> UtteranceInput<'input> {
    fn admit(parser: &TreeSitterParser, input: &'input str) -> ParseResult<Self> {
        let newline = if input.ends_with('\n') { "" } else { "\n" };
        // Admission checks the complete probe capacity before allocation. A
        // failed parse propagates; it must not be mistaken for fragment shape.
        let probe = WrappedFragment::new(&[], input, newline, 0)?;
        let complete = {
            let parsed = probe.parse(parser)?;
            SourceFileNode::from_node(parsed.parsed_source().root_node()).is_some_and(|root| {
                matches!(
                    extract_source_file(root).content.slot(),
                    NodeSlot::Present(SourceFileChoice::FullDocument(_))
                )
            })
        };
        Ok(if complete {
            Self::Complete(probe)
        } else {
            Self::NeedsWrapper { input, newline }
        })
    }

    fn into_wrapped(self) -> ParseResult<WrappedFragment<'input>> {
        match self {
            Self::Complete(probe) => Ok(probe),
            Self::NeedsWrapper { input, newline } => WrappedFragment::new(
                &[MINIMAL_CHAT_PREFIX],
                input,
                &format!("{newline}{MINIMAL_CHAT_SUFFIX}"),
                0,
            ),
        }
    }
}

/// Parse one utterance fragment into `Utterance`.
///
/// This path intentionally reuses whole-file recovery so fragment parsing stays
/// aligned with normal utterance construction, including preceding headers and
/// attached dependent tiers.
pub(super) fn parse_utterance(parser: &TreeSitterParser, input: &str) -> ParseResult<Utterance> {
    let fragment = UtteranceInput::admit(parser, input)?.into_wrapped()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_notrans_document_is_not_a_single_utterance() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/core/headers-media-notrans.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        for input in [source, source.trim_end_matches('\n')] {
            let admitted = UtteranceInput::admit(&parser, input).expect("admitted document");
            assert!(matches!(admitted, UtteranceInput::Complete(_)));
            let wrapped = admitted.into_wrapped().expect("complete envelope");
            assert_eq!(wrapped.source(), source);
            let errors = parse_utterance(&parser, input)
                .expect_err("no utterance")
                .into_error_vec();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, ErrorCode::MissingMainTier);
            assert_eq!(
                errors[0].location.span,
                crate::error::Span::from_usize(0, input.len())
            );
        }
    }
}
