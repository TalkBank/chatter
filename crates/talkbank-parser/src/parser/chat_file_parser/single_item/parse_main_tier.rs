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
    DiagnosticLocation, ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity,
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
    let fragment = WrappedFragment::local(&[], input, newline)?;
    let parsed = fragment.parse(parser)?;
    MainTierFragment::admit(&parsed)?.lower()
}

/// Evidence that the typed main-tier node accounts for the complete parse source.
/// The source and original input travel with it so lowering cannot accidentally
/// expose the synthetic line terminator as a caller-owned byte.
struct MainTierFragment<'tree, 'source, 'input> {
    source: SourceBound<'tree, 'source, MainTierNode<'tree>>,
    parsed: &'tree ParsedFragment<'source, 'input>,
}

impl<'tree, 'source, 'input> MainTierFragment<'tree, 'source, 'input> {
    fn admit(parsed_fragment: &'tree ParsedFragment<'source, 'input>) -> ParseResult<Self> {
        let parsed = parsed_fragment.parsed_source();
        let fragment = parsed_fragment.fragment();
        let failure = |code, message: &str| {
            ParseErrors::from(vec![fragment.whole_input_error(
                code,
                Severity::Error,
                fragment.input(),
                message,
            )])
        };
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
            || !fragment.covers_whole_source(parsed_fragment.node_span(node.raw_node()))
        {
            return Err(failure(
                ErrorCode::MissingMainTier,
                "Expected exactly one complete main-tier fragment",
            ));
        }
        Ok(Self {
            source: parsed.bind_typed(*node).map_err(|error| {
                failure(
                    ErrorCode::InternalError,
                    &format!("Main-tier node is not bound to its parse source: {error}"),
                )
            })?,
            parsed: parsed_fragment,
        })
    }

    fn lower(self) -> ParseResult<MainTier> {
        if self.source.raw_node().has_error() {
            let errors = self.collect_errors();
            if !errors.is_empty() {
                return Err(errors);
            }
        }
        let Self { source, parsed } = self;
        let fragment = parsed.fragment();
        // Convert the main_tier node to MainTier model; its spans are
        // synthetic until the wrapper projects them.
        let errors_sink = crate::error::ErrorCollector::new();
        let main_tier = parsed.lowered(convert_main_tier_node(
            source,
            &fragment.error_sink(&errors_sink),
        ));

        let tier_errors = errors_sink.into_vec();
        let has_actual_errors = tier_errors
            .iter()
            .any(|e| matches!(e.severity, Severity::Error));
        match main_tier.try_map(|lowered| lowered.ok().filter(|_| !has_actual_errors).ok_or(())) {
            Ok(main_tier) => Ok(fragment.rebase_main_tier(main_tier)),
            // A rejection carries evidence that its producer already reported
            // a diagnostic to this collector. No synthetic fallback is needed.
            Err(()) => Err(ParseErrors::from(tier_errors)),
        }
    }

    /// Recovery can walk only the admitted main tier's canonical descendants.
    /// There is no independent node, source or wrapper offset for a caller to
    /// mismatch. Only the synthetic final newline may lie beyond caller input.
    fn collect_errors(&self) -> ParseErrors {
        let fragment = self.parsed.fragment();
        let input = fragment.input();
        let mut errors = ParseErrors::new();
        for bound in self.source.descendants() {
            let bound = match bound {
                Ok(bound) => bound,
                Err(error) => {
                    errors.push(fragment.whole_input_error(
                        ErrorCode::InternalError,
                        Severity::Error,
                        input,
                        format!("Main-tier recovery source binding failed: {error}"),
                    ));
                    continue;
                }
            };
            let node = bound.raw_node();
            // The node's synthetic span, clamped into the caller's input and
            // expressed in input coordinates: the snippet below is cut from
            // the caller's text, so no synthetic byte may reach it.
            let in_input = fragment.input_span(self.parsed.node_span(node));
            let (start, end) = (in_input.start(), in_input.end());
            if node.is_missing() {
                errors.push(
                    ParseError::new(
                        ErrorCode::MissingNode,
                        Severity::Error,
                        DiagnosticLocation::from_offsets(start, start),
                        ErrorContext::new(input, talkbank_model::Span::new(start, start), ""),
                        format!("Missing '{}'", node.kind()),
                    )
                    .with_suggestion(format!("Add missing {}", node.kind())),
                );
            }
            if node.is_error() {
                match input.get(start..end) {
                    Some(found) => {
                        let mut error = analyze_bound_error_node(bound, "parse tree");
                        error.location = DiagnosticLocation::from_offsets(start, end);
                        error.context = Some(ErrorContext::new(
                            input,
                            talkbank_model::Span::new(start, end),
                            found,
                        ));
                        errors.push(error);
                    }
                    None => errors.push(fragment.whole_input_error(
                        ErrorCode::InternalError,
                        Severity::Error,
                        input,
                        "Main-tier recovery range is not a UTF-8 slice of its caller input",
                    )),
                }
            }
        }
        errors
    }
}
