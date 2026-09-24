//! Parse isolated CHAT words via multi-root grammar.
//!
//! With `standalone_word` in the source_file union, a bare word like `hello`
//! or `&-uh` is parsed directly, no synthetic wrapper needed.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>

use super::TreeSitterParser;
use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, NodeSlot, ParsedSource, SourceBound, SourceFileChoice, SourceFileNode,
    StandaloneWordNode, extract_source_file,
};
use crate::model::Word;
use crate::parser::tree_parsing::main_tier::word::convert_word_node;
use talkbank_model::ParseOutcome;

/// Parse a single word token directly via the multi-root grammar.
///
/// The input (e.g., `hello`, `&-uh`, `hel(lo)`) is parsed directly as a
/// `standalone_word` fragment. No synthetic `*CHI:\t... .` wrapper is needed.
pub(super) fn parse_word(parser: &TreeSitterParser, input: &str) -> ParseResult<Word> {
    if input.is_empty() {
        let mut errors = ParseErrors::new();
        errors.push(ParseError::new(
            ErrorCode::InvalidWordFormat,
            Severity::Error,
            SourceLocation::from_offsets(0, 0),
            ErrorContext::new(input, 0..0, input),
            "Empty word input",
        ));
        return Err(errors);
    }

    let tree = parser.parse_source_incremental(input, None)?;

    WordFragment::admit(&tree)?.lower()
}

/// A complete standalone-word selection and the source owned by its producer.
/// Only admission constructs this phase; lowering accepts no independent bytes.
struct WordFragment<'tree, 'source> {
    source: SourceBound<'tree, 'source, StandaloneWordNode<'tree>>,
}

impl<'tree, 'source> WordFragment<'tree, 'source> {
    fn admit(parsed: &'tree ParsedSource<'source>) -> ParseResult<Self> {
        let input = parsed.source();
        let failure = |code, message: String| {
            ParseErrors::from(vec![ParseError::new(
                code,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                message,
            )])
        };
        let raw_root = parsed.root_node();
        let root = SourceFileNode::from_node(raw_root).ok_or_else(|| {
            failure(
                ErrorCode::TreeParsingError,
                format!("Expected source_file root, got '{}'", raw_root.kind()),
            )
        })?;
        let children = extract_source_file(root);
        let NodeSlot::Present(SourceFileChoice::StandaloneWord(node)) = children.content.slot()
        else {
            // Raw shape is used only to describe the refused input. Admission
            // and lowering dispatch through the generated typed selection.
            let found = match raw_root.child(0) {
                Some(child) => format!("got '{}'", child.kind()),
                None => "got no node at all".to_owned(),
            };
            return Err(failure(
                ErrorCode::InvalidWordFormat,
                format!("Expected standalone_word fragment, {found}"),
            ));
        };
        if !children.unexpected.is_empty()
            || !children.trailing_extras.is_empty()
            || !children.content.leading_extras().is_empty()
            || node.raw_node().has_error()
        {
            return Err(failure(
                ErrorCode::InvalidWordFormat,
                "Word input contains unparsable content".to_owned(),
            ));
        }
        Ok(Self {
            source: parsed.bind_typed(*node).map_err(|error| {
                failure(
                    ErrorCode::TreeParsingError,
                    format!("Word node is not bound to its parse source: {error}"),
                )
            })?,
        })
    }

    fn lower(self) -> ParseResult<Word> {
        let input = self.source.source();

        let errors_sink = crate::error::ErrorCollector::new();
        let outcome = convert_word_node(self.source.node(), input, &errors_sink);
        let tier_errors = errors_sink.into_vec();
        if tier_errors
            .iter()
            .any(|e| matches!(e.severity, Severity::Error))
        {
            return Err(ParseErrors::from(tier_errors));
        }

        // The producer's successful outcome cannot enter a rejection fallback.
        match outcome {
            ParseOutcome::Parsed(word) => Ok(word),
            ParseOutcome::Rejected => {
                let mut errors = ParseErrors::from(tier_errors);
                if errors.is_empty() {
                    errors.push(ParseError::new(
                        ErrorCode::InvalidWordFormat,
                        Severity::Error,
                        SourceLocation::from_offsets(0, input.len()),
                        ErrorContext::new(input, 0..input.len(), input),
                        "Failed to build word from parse tree",
                    ));
                }
                Err(errors)
            }
        }
    }
}
