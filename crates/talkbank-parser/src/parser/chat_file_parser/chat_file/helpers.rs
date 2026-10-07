//! Shared low-level routines used by CHAT file parsing.
//!
//! This layer handles line iteration, selective top-level error recovery, and
//! conversion into `Line` values before participant synthesis and normalization.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
    TeeErrorSink,
};
use crate::model::Line;
use crate::parser::CstNodeId;
use crate::parser::TreeSitterParser;
use crate::parser::document_root::DocumentRoot;
use crate::parser::tree_parsing::parser_helpers::error_checking::collect_recovery_nodes_retaining;
use tracing::{debug, info, trace};

use super::document_lowering::DocumentLowering;
use super::tier_plan::{PlanEntry, RetainAll, TierRouting};

/// Parse all lines from `input` and stream diagnostics to `errors`.
pub(super) fn parse_lines(
    parser: &TreeSitterParser,
    input: &str,
    errors: &impl ErrorSink,
) -> Vec<Line> {
    parse_lines_with_old_tree(parser, input, None, errors).0
}

/// Parse lines, optionally reusing `old_tree` for incremental updates.
/// Returns `(lines, new_tree)`.
///
/// # The size guard, and the hole it closes
///
/// Every FRAGMENT entry point admits its input through
/// `talkbank_model::FragmentSource`, whose whole purpose is to refuse a range
/// the model's 32-bit byte coordinates cannot hold. The four WHOLE-FILE entry
/// points funnel through here and admitted nothing, so the proof type had a
/// door in the wall it was built to close: a proof type is only as strong as
/// its weakest constructor, and the weakest one was no constructor at all.
///
/// What that cost, measured 2026-09-08: a 4,294,967,418-byte transcript whose
/// content after `@End` is invalid CHAT reported `Valid: 1` and exited 0,
/// while a 1,122-byte file of the same shape was correctly rejected with
/// E316. tree-sitter's coordinate space is 32 bits wide, so it saw a prefix
/// and answered about the prefix, and every stage above it reported that
/// answer as the file's.
///
/// The guard is here rather than at the four callers for the reason the
/// fragment API already demonstrates: a rule each caller must remember is a
/// rule one of them will not.
pub(super) fn parse_lines_with_old_tree(
    parser: &TreeSitterParser,
    input: &str,
    old_tree: Option<&tree_sitter::Tree>,
    errors: &impl ErrorSink,
) -> (Vec<Line>, Option<tree_sitter::Tree>) {
    let (lines, parsed) = parse_lines_with_source(parser, input, old_tree, errors);
    (
        lines,
        parsed.map(crate::generated_traversal::ParsedSource::into_tree),
    )
}

/// Retain the producing source capability instead of detaching its raw tree.
pub(super) fn parse_lines_with_source<'source>(
    parser: &TreeSitterParser,
    input: &'source str,
    old_tree: Option<&tree_sitter::Tree>,
    errors: &impl ErrorSink,
) -> (
    Vec<Line>,
    Option<crate::generated_traversal::ParsedSource<'source>>,
) {
    let (lines, lowered) = parse_lines_with_removal(
        parser,
        input,
        old_tree,
        errors,
        PlanEntry::Decided(RetainAll),
    );
    (lines, lowered.map(|(source, RetainAll)| source))
}

/// One source producer and lowering path for retained and complete documents.
///
/// Returns the decided plan beside the producing source: both exist exactly
/// when lowering ran, so a plan can never be read before its decision.
pub(super) fn parse_lines_with_removal<'source, P: TierRouting>(
    parser: &TreeSitterParser,
    input: &'source str,
    old_tree: Option<&tree_sitter::Tree>,
    errors: &impl ErrorSink,
    entry: PlanEntry<'_, P>,
) -> (
    Vec<Line>,
    Option<(crate::generated_traversal::ParsedSource<'source>, P)>,
) {
    debug!("Parsing CHAT file ({} bytes)", input.len());

    // The shared producer checks coordinate capacity and binds the exact
    // input. A separately supplied tree/source pair cannot enter lowering.
    let tree = match parser.parse_source_incremental(input, old_tree) {
        Ok(tree) => tree,
        Err(failure) => {
            for error in failure.into_error_vec() {
                errors.report(error);
            }
            return (Vec::new(), None);
        }
    };

    trace!("Tree-sitter parse completed");
    // One owner of "where is the document, and what did it turn out to be".
    let root = match DocumentRoot::classify(&tree) {
        Ok(root) => root,
        Err(error) => {
            crate::parser::typed_cst::report_cst_failure(
                tree.root_node(),
                tree.source(),
                error,
                errors,
            );
            return (Vec::new(), None);
        }
    };
    let root_node = root.node();
    let syntax_root = root.syntax_root();

    // Check if the root node itself has errors AND is empty (e.g., empty file)
    if root_node.has_error() && root_node.child_count() == 0 {
        errors.report(
            ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len().max(1)),
                ErrorContext::new(input, 0..input.len().max(1), input),
                "Unparsable content: file is empty or contains no recognizable CHAT structure",
            )
            .with_suggestion("CHAT files must contain at minimum @UTF8, @Begin, and @End headers"),
        );
        return (Vec::new(), None);
    }

    // Whether the parser RECOVERED at the document position, needed after the
    // loop to report a file from which no valid lines came back. Read off the
    // classification rather than re-derived with a second `is_error()`, which
    // had to agree with whether the recovery reconstruction was attempted.
    let root_is_error = root.recovered_at_root();

    // Tee the sink into a collector so the streaming loop's per-region
    // diagnostics are recorded as well as forwarded. The loop only inspects
    // recovery (ERROR/MISSING) nodes in the regions its handlers descend into;
    // recovery nodes nested elsewhere were silently dropped, so a file that
    // tree-sitter flagged as malformed could still validate clean. After the
    // loop, a whole-tree backstop surfaces any recovery node the handlers missed
    // (recovery is not validity), using the collected spans to avoid
    // double-reporting a node a richer region diagnostic already covered.
    // `ErrorCollector` allocates lazily, so a clean file pays nothing.
    let collector = ErrorCollector::new();
    let recording = TeeErrorSink::new(errors, &collector);
    let errors = &recording;

    // Visitor-driven document/line walk (Task 1 of the visitor-driven parser
    // migration). The hand-walked `match child.kind()` dispatch over
    // `full_document` children was replaced by `DocumentLowering`, which drives
    // the generated `extract_full_document` and processes each `NodeSlot` slot
    // exhaustively. Document-level ERROR nodes are reported without constructing
    // headers or tiers from their text. Present lines retain normal lowering.
    // `DocumentLowering` borrows the Tee'd sink so its emissions are
    // recorded for the backstop's span-dedup below.
    // A recovered document lowers exactly like a complete one: the ERROR
    // standing in for a `full_document` carries the same children, so a missing
    // `@End` still recovers every line and the absent trailer surfaces as an
    // `Absent` slot for the validator to report as E502. `None` is the file with
    // nothing document-shaped in it at all; the recovery backstop below still
    // runs over the node, and the "no valid lines recovered" path still reports
    // it.
    let (lines, plan) = DocumentLowering::lower(root, errors, entry);

    // When the root IS an ERROR node and the loop couldn't recover any valid
    // lines, the file is completely unparsable.  Report this so the strict caller
    // returns Err.  When the root is ERROR but children ARE valid structures
    // (e.g., missing @End), the loop recovers lines and the validation layer
    // can catch the missing header.
    if root_is_error && lines.is_empty() {
        errors.report(
            ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len().max(1)),
                ErrorContext::new(input, 0..input.len().max(1), input),
                "Unparsable content: file structure is not valid CHAT and no lines could be recovered",
            )
            .with_suggestion("CHAT files must contain @UTF8, @Begin, @Participants, @Languages, and @End headers"),
        );
    }

    // Whole-tree recovery-node backstop. Gated on `has_error()` so valid files
    // (the overwhelming majority) pay nothing. Every surviving ERROR/MISSING node
    // not already covered by a region diagnostic above is surfaced as invalidity.
    // ERROR uses a structurally proven dedicated code when available and E316
    // otherwise; MISSING uses E342. The parser still produced an AST; this only
    // reports, honoring lenient recovery while enforcing "recovery is not validity".
    if syntax_root.has_error() {
        let reported = collector.to_vec();
        let mut candidates = Vec::new();
        // Exclusion is by exact node identity, never diagnostic code/span.
        // The lookup is built once, only when the backstop must traverse
        // recovery: scanning every withheld tier per CST node would be
        // quadratic.
        let withheld: std::collections::HashSet<CstNodeId> = plan.withheld_nodes().collect();
        collect_recovery_nodes_retaining(syntax_root, input, &mut candidates, &|node| {
            !withheld.contains(&CstNodeId::of(node))
        });
        for candidate in candidates {
            // Widen a zero-width MISSING span to one byte so it can intersect a
            // reported span that merely touches its point. A candidate already
            // covered by a (richer) region diagnostic is suppressed.
            let span = candidate.location.span;
            let probe = Span::new(span.start, span.end.max(span.start.saturating_add(1)));
            // A region diagnostic that is itself zero-width at the same point
            // covers the candidate only when it reports the same code: the
            // typed tier dispatch's E342 for a MISSING node covers the
            // backstop's E342 for that node (until 2026-09-08 it did not, and
            // every MISSING node inside a dependent tier was reported twice),
            // while E376 for an empty replacement, zero-width at the point
            // where its MISSING word segment sits, is a different fact about a
            // different node and leaves the E342 to be reported (E208.md).
            let same_point_same_code = |e: &ParseError| {
                let r = e.location.span;
                r.start == r.end && r.start == span.start && e.code == candidate.code
            };
            if !reported
                .iter()
                .any(|e| e.location.span.overlaps(probe) || same_point_same_code(e))
            {
                errors.report(candidate);
            }
        }
    }

    info!("Parsed {} lines", lines.len());

    (lines, Some((tree, plan)))
}
