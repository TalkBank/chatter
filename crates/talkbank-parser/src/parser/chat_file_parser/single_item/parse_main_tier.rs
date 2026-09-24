//! Parse isolated main-tier lines via multi-root grammar.
//!
//! With multi-root, `*CHI:\thello .` is parsed directly as a main_tier
//! fragment, no synthetic full-document wrapper needed.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::TreeSitterParser;
use crate::api::fragment::{ParsedFragment, WrappedFragment};
use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, MainTierNode, NodeSlot, SourceBound, SourceFileChoice, SourceFileNode,
    extract_source_file,
};
use crate::model::MainTier;
use crate::parser::tree_parsing::helpers::analyze_bound_error_node;
use crate::parser::tree_parsing::main_tier::structure::convert_main_tier_node;

/// Parse one main tier line into `MainTier`.
///
/// With multi-root grammar, the input (e.g., `*CHI:\thello .`) is parsed
/// directly as a main_tier fragment. Admission requires the typed main tier
/// to account for the entire `source_file`.
pub(super) fn parse_main_tier(parser: &TreeSitterParser, input: &str) -> ParseResult<MainTier> {
    // Multi-root needs only a line terminator, never document scaffolding.
    let newline = if input.ends_with('\n') { "" } else { "\n" };
    let fragment = WrappedFragment::new(&[], input, newline, 0)?;
    let parsed = fragment.parse(parser)?;
    MainTierFragment::admit(&parsed)?.lower()
}

/// Evidence that the typed main-tier node accounts for the complete parse source.
/// The source and original input travel with it so lowering cannot accidentally
/// expose the synthetic line terminator as a caller-owned byte.
struct MainTierFragment<'tree, 'source> {
    source: SourceBound<'tree, 'source, MainTierNode<'tree>>,
    input: &'source str,
}

impl<'tree, 'source> MainTierFragment<'tree, 'source> {
    fn admit(parsed_fragment: &'tree ParsedFragment<'source, '_>) -> ParseResult<Self> {
        let parsed = parsed_fragment.parsed_source();
        let input = parsed_fragment.fragment().input();
        let failure = |code, message| {
            ParseErrors::from(vec![ParseError::new(
                code,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                message,
            )])
        };
        let source = parsed.source();
        let root = SourceFileNode::from_node(parsed.root_node())
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
            source: parsed.bind_typed(*node).map_err(|_| {
                failure(
                    ErrorCode::TreeParsingError,
                    "Main-tier node is not bound to its parse source",
                )
            })?,
            input,
        })
    }

    fn lower(self) -> ParseResult<MainTier> {
        if self.source.raw_node().has_error() {
            let errors = self.collect_errors();
            if !errors.is_empty() {
                return Err(errors);
            }
        }
        let Self { source, input } = self;
        let main_tier_node = source.node();
        let to_parse = source.source();
        // Convert the main_tier node to MainTier model
        let errors_sink = crate::error::ErrorCollector::new();
        let main_tier = convert_main_tier_node(main_tier_node, to_parse, input, &errors_sink);

        let tier_errors = errors_sink.into_vec();
        let has_actual_errors = tier_errors
            .iter()
            .any(|e| matches!(e.severity, Severity::Error));
        let mut main_tier = match main_tier {
            Ok(main_tier) if !has_actual_errors => main_tier,
            // A rejection carries evidence that its producer already reported
            // a diagnostic to this collector. No synthetic fallback is needed.
            Ok(_) | Err(_) => return Err(ParseErrors::from(tier_errors)),
        };
        let input_end = input.len() as u32;
        main_tier.span.end = main_tier.span.end.min(input_end);
        if let Some(span) = &mut main_tier.content.content_span {
            span.end = span.end.min(input_end);
        }
        Ok(main_tier)
    }

    /// Recovery can walk only the admitted main tier's canonical descendants.
    /// There is no independent node, source or wrapper offset for a caller to
    /// mismatch. Only the synthetic final newline may lie beyond caller input.
    fn collect_errors(&self) -> ParseErrors {
        let mut errors = ParseErrors::new();
        for bound in self.source.descendants() {
            let bound = match bound {
                Ok(bound) => bound,
                Err(error) => {
                    errors.push(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(0, self.input.len()),
                        ErrorContext::new(self.input, 0..self.input.len(), self.input),
                        format!("Main-tier recovery source binding failed: {error}"),
                    ));
                    continue;
                }
            };
            let node = bound.raw_node();
            let start = node.start_byte().min(self.input.len());
            let end = node.end_byte().min(self.input.len());
            if node.is_missing() {
                errors.push(
                    ParseError::new(
                        ErrorCode::MissingNode,
                        Severity::Error,
                        SourceLocation::from_offsets(start, start),
                        ErrorContext::new(self.input, start..start, ""),
                        format!("Missing '{}'", node.kind()),
                    )
                    .with_suggestion(format!("Add missing {}", node.kind())),
                );
            }
            if node.is_error() {
                match self.input.get(start..end) {
                    Some(found) => {
                        let mut error = analyze_bound_error_node(bound, "parse tree");
                        error.location = SourceLocation::from_offsets(start, end);
                        error.context = Some(ErrorContext::new(self.input, start..end, found));
                        errors.push(error);
                    }
                    None => errors.push(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(0, self.input.len()),
                        ErrorContext::new(self.input, 0..self.input.len(), self.input),
                        "Main-tier recovery range is not a UTF-8 slice of its caller input",
                    )),
                }
            }
        }
        errors
    }
}
