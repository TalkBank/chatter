//! Parse isolated main-tier lines via multi-root grammar.
//!
//! With multi-root, `*CHI:\thello .` is parsed directly as a main_tier
//! fragment, no synthetic full-document wrapper needed.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::TreeSitterParser;
use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, MainTierNode, NodeSlot, SourceFileChoice, SourceFileNode,
    extract_source_file,
};
use crate::model::MainTier;
use crate::parser::tree_parsing::main_tier::structure::{
    collect_main_tier_errors, convert_main_tier_node,
};

/// Parse one main tier line into `MainTier`.
///
/// With multi-root grammar, the input (e.g., `*CHI:\thello .`) is parsed
/// directly as a main_tier fragment. Admission requires the typed main tier
/// to account for the entire `source_file`.
pub(super) fn parse_main_tier(parser: &TreeSitterParser, input: &str) -> ParseResult<MainTier> {
    // Multi-root: parse directly, no wrapper
    let to_parse = if input.ends_with('\n') {
        std::borrow::Cow::Borrowed(input)
    } else {
        std::borrow::Cow::Owned(format!("{input}\n"))
    };

    let tree = {
        let mut ts_parser = parser.parser.borrow_mut();
        ts_parser.parse(to_parse.as_bytes(), None).ok_or_else(|| {
            let mut errors = ParseErrors::new();
            errors.push(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "Tree-sitter parse returned None",
            ));
            errors
        })?
    };

    MainTierFragment::admit(tree.root_node(), &to_parse, input)?.lower()
}

/// Evidence that the typed main-tier node accounts for the complete parse source.
/// The source and original input travel with it so lowering cannot accidentally
/// expose the synthetic line terminator as a caller-owned byte.
struct MainTierFragment<'tree, 'source> {
    node: MainTierNode<'tree>,
    source: &'source str,
    input: &'source str,
}

impl<'tree, 'source> MainTierFragment<'tree, 'source> {
    fn admit(
        root: tree_sitter::Node<'tree>,
        source: &'source str,
        input: &'source str,
    ) -> ParseResult<Self> {
        let failure = |code, message| {
            ParseErrors::from(vec![ParseError::new(
                code,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                message,
            )])
        };
        let root = SourceFileNode::from_node(root)
            .ok_or_else(|| failure(ErrorCode::TreeParsingError, "Expected source_file root"))?;
        let children = extract_source_file(root);
        let NodeSlot::Present(SourceFileChoice::MainTier(node)) = children.content.slot() else {
            return Err(failure(
                ErrorCode::MissingMainTier,
                "Expected exactly one complete main-tier fragment",
            ));
        };
        if !children.unexpected.is_empty()
            || !children.trailing_extras.is_empty()
            || !children.content.leading_extras().is_empty()
            || node.raw_node().byte_range() != (0..source.len())
        {
            return Err(failure(
                ErrorCode::MissingMainTier,
                "Expected exactly one complete main-tier fragment",
            ));
        }
        Ok(Self {
            node: *node,
            source,
            input,
        })
    }

    fn lower(self) -> ParseResult<MainTier> {
        let Self {
            node: main_tier_node,
            source: to_parse,
            input,
        } = self;
        // Check for parse errors
        if main_tier_node.raw_node().has_error() {
            let mut errors = ParseErrors::new();
            collect_main_tier_errors(main_tier_node.raw_node(), to_parse, input, 0, &mut errors);
            if !errors.is_empty() {
                return Err(errors);
            }
        }

        // Convert the main_tier node to MainTier model
        let errors_sink = crate::error::ErrorCollector::new();
        let main_tier =
            convert_main_tier_node(main_tier_node, to_parse, input, &errors_sink).into_option();

        let tier_errors = errors_sink.into_vec();
        let has_actual_errors = tier_errors
            .iter()
            .any(|e| matches!(e.severity, Severity::Error));
        if has_actual_errors {
            return Err(ParseErrors::from(tier_errors));
        }
        let mut main_tier = main_tier.ok_or_else(|| {
            let mut errors = ParseErrors::from(tier_errors);
            if errors.is_empty() {
                errors.push(ParseError::new(
                    ErrorCode::MissingMainTier,
                    Severity::Error,
                    SourceLocation::from_offsets(0, input.len()),
                    ErrorContext::new(input, 0..input.len(), input),
                    "Failed to build main tier from parse tree",
                ));
            }
            errors
        })?;
        let input_end = input.len() as u32;
        main_tier.span.end = main_tier.span.end.min(input_end);
        if let Some(span) = &mut main_tier.content.content_span {
            span.end = span.end.min(input_end);
        }
        Ok(main_tier)
    }
}
