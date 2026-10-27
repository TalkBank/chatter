//! Parse isolated utterances via whole-file recovery plus extraction.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::TreeSitterParser;
use super::helpers::{MINIMAL_CHAT_PREFIX, MINIMAL_CHAT_SUFFIX};
use crate::api::fragment::WrappedFragment;
use crate::error::{ErrorCode, ErrorCollector, ParseErrors, ParseResult, Severity};
use crate::generated_traversal::{
    FromNodeKind, NodeSlot, SourceFileChoice, SourceFileNode, extract_source_file,
};
use talkbank_model::{SingleUtterance, UtteranceCardinalityError};

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
        let probe = WrappedFragment::local(&[], input, newline)?;
        let complete = {
            let parsed = probe.parse(parser)?;
            match SourceFileNode::from_node(parsed.parsed_source().root_node()) {
                Some(root) => {
                    let children = extract_source_file(root);
                    matches!(
                        children.content.slot(),
                        NodeSlot::Present(SourceFileChoice::FullDocument(_))
                    )
                }
                None => false,
            }
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
            Self::NeedsWrapper { input, newline } => WrappedFragment::local(
                &[MINIMAL_CHAT_PREFIX],
                input,
                &format!("{newline}{MINIMAL_CHAT_SUFFIX}"),
            ),
        }
    }
}

/// Parse one utterance fragment into `Utterance`.
///
/// This path intentionally reuses whole-file recovery so fragment parsing stays
/// aligned with normal utterance construction, including preceding headers and
/// attached dependent tiers.
pub(super) fn parse_utterance(
    parser: &TreeSitterParser,
    input: &str,
) -> ParseResult<SingleUtterance> {
    let fragment = UtteranceInput::admit(parser, input)?.into_wrapped()?;
    let errors_sink = ErrorCollector::new();
    let file = fragment.parse_file(parser, &errors_sink);
    let parse_errors = errors_sink.into_vec();
    if !parse_errors.is_empty() {
        return Err(ParseErrors::from(parse_errors));
    }

    let utterance = file.try_map(SingleUtterance::admit).map_err(|reason| {
        let code = match reason {
            UtteranceCardinalityError::Missing => ErrorCode::MissingMainTier,
            UtteranceCardinalityError::Multiple => ErrorCode::UnparsableContent,
        };
        ParseErrors::from(vec![fragment.whole_input_error(
            code,
            Severity::Error,
            input,
            reason.to_string(),
        )])
    })?;

    fragment.rebase_utterance(utterance)
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
            assert_eq!(wrapped.synthetic_source(), source);
            let errors = parse_utterance(&parser, input)
                .expect_err("no utterance")
                .into_error_vec();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, ErrorCode::MissingMainTier);
            assert_eq!(
                errors[0].location.span().expect("located diagnostic"),
                crate::error::Span::new(0, input.len())
            );
        }
    }
}
