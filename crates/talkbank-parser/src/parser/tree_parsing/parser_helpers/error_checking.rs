//! Recursive traversal helpers for collecting tree-sitter recovery errors.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use crate::node_types::{
    FULL_DOCUMENT, GRA_DEPENDENT_TIER, LINE, MOR_DEPENDENT_TIER, NEWLINE, PHO_DEPENDENT_TIER,
};
use tree_sitter::Node;

use super::error_analysis::analyze_dependent_tier_error_with_context;

/// Recursively walks a subtree and collects parse errors.
pub(crate) fn check_for_errors_recursive(node: Node, source: &str, errors: &mut Vec<ParseError>) {
    check_for_errors_recursive_with_context(node, source, errors, None);
}

/// Recursively walks a subtree and tracks tier context for better diagnostics.
pub(crate) fn check_for_errors_recursive_with_context(
    node: Node,
    source: &str,
    errors: &mut Vec<ParseError>,
    tier_type: Option<&str>,
) {
    // Check for ERROR nodes (tree-sitter couldn't parse this content)
    if node.is_error() {
        errors.push(analyze_dependent_tier_error_with_context(
            node, source, tier_type,
        ));
        return;
    }

    // Check for MISSING nodes (tree-sitter inserted placeholder for required element)
    if node.is_missing() {
        let tier_context = match tier_type {
            Some(t) => format!(" in {} tier", t),
            None => String::new(),
        };
        errors.push(ParseError::new(
            ErrorCode::MissingRequiredElement,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            format!(
                "Missing required '{}'{} at byte {} (tree-sitter error recovery)",
                node.kind(),
                tier_context,
                node.start_byte()
            ),
        ));
        return;
    }

    // Determine tier type from node kind
    let new_tier_type = match node.kind() {
        MOR_DEPENDENT_TIER => Some("mor"),
        GRA_DEPENDENT_TIER => Some("gra"),
        PHO_DEPENDENT_TIER => Some("pho"),
        _ if tier_type.is_some() => tier_type, // Inherit parent tier type
        _ => None,
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_for_errors_recursive_with_context(child, source, errors, new_tier_type);
    }
}

/// Walk the entire CST and COLLECT a diagnostic for every surviving
/// tree-sitter recovery node (`ERROR` and `MISSING`).
///
/// The parser deliberately recovers from malformed input by inserting these
/// nodes and continuing, so the LSP and downstream repair always get an AST.
/// But recovery is not validity: a document that needed a synthetic recovery
/// node did not conform to the grammar, so each such node is a
/// `Severity::Error`. `ERROR` -> [`ErrorCode::UnparsableContent`] (E316);
/// `MISSING` -> [`ErrorCode::MissingRequiredElement`] (E342).
///
/// This is the whole-tree BACKSTOP for the streaming lowering, which only
/// inspects recovery nodes in the specific regions its per-region handlers
/// descend into (top-level children, one level into `LINE`, dependent-tier
/// children). Recovery nodes nested elsewhere (a stray token after a matched
/// header, mid-utterance content) were silently dropped; this catch-all
/// surfaces them. It COLLECTS rather than reports so the caller can suppress
/// any node already covered by a (richer) region diagnostic before emitting.
///
/// Recursion stops at a recovery node: its subtree is accounted for by the node
/// itself. The caller owns `out` (mirroring [`check_for_errors_recursive`]) so
/// it can dedup against already-reported spans before emitting.
pub(crate) fn collect_recovery_nodes(node: Node, source: &str, out: &mut Vec<ParseError>) {
    if node.is_error() {
        // A structural-incompleteness ERROR wraps the recovered document: when a
        // top-level element is missing (no @End, or a malformed @Begin), the whole
        // `document` rule fails to complete and tree-sitter returns an ERROR node
        // AROUND the recovered headers/lines. The validation layer reports that
        // precisely (E502 missing @End, E504 missing @Begin), so reporting the
        // wrapper too would be a misleading, redundant whole-file E316. Recurse
        // into it to surface only LOCALIZED recovery nodes; do not report the
        // wrapper itself. A leaf/content ERROR (a stray token, a malformed code)
        // wraps no document structure and is reported normally below.
        if wraps_document_structure(node) {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_recovery_nodes(child, source, out);
            }
            return;
        }

        let start = node.start_byte();
        let end = node.end_byte();
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        let first_line = text.lines().next().unwrap_or(text).trim();

        // Dedicated-code classification before the generic E316 catch-all
        // (same pure rules as the region analyzers; see
        // `error_analysis::dedicated`).
        //
        // E760: the ERROR sits inside a `%mor` tier (typed ancestor check,
        // or the whole line is the ERROR and carries the prefix) and holds
        // an item with an empty part-of-speech field.
        let in_mor_tier = {
            let mut ancestor = node.parent();
            let mut found = false;
            while let Some(candidate) = ancestor {
                if candidate.kind() == MOR_DEPENDENT_TIER {
                    found = true;
                    break;
                }
                ancestor = candidate.parent();
            }
            found || text.contains("%mor:")
        };
        if in_mor_tier
            && let Some(item) = super::error_analysis::dedicated::mor_item_with_empty_pos(
                text,
                super::error_analysis::dedicated::at_item_boundary(source, start),
            )
        {
            let (item_start, item_end) = match text.find(item) {
                Some(offset) => (start + offset, start + offset + item.len()),
                None => (start, end),
            };
            out.push(
                ParseError::new(
                    ErrorCode::MorItemEmptyPos,
                    Severity::Error,
                    SourceLocation::from_offsets(item_start, item_end),
                    ErrorContext::new(source, item_start..item_end, item),
                    format!("MOR item '{item}' has an empty part-of-speech field"),
                )
                .with_suggestion(
                    "Every %mor item is pos|stem with a non-empty part of speech before the \
                     pipe (e.g., pro|we, v|go)",
                ),
            );
            return;
        }

        // E759: the ERROR is a whole main-tier line whose content begins
        // with a postfix annotation (CLAN CHECK 52); fragment-level leading
        // annotations are classified positionally in the contents loop.
        if text.starts_with('*')
            && let Some(sep) = text.find(":\t")
            && let Some(code_token) = super::error_analysis::dedicated::leading_postfix_annotation(
                text[sep + 2..].trim_start(),
            )
        {
            out.push(
                ParseError::new(
                    ErrorCode::AnnotationAtUtteranceStart,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, text),
                    format!(
                        "Annotation '{code_token}' at utterance start has no content to attach to"
                    ),
                )
                .with_suggestion(
                    "Retraces, overlap markers, replacements, and quotation codes scope over \
                     the material BEFORE them; put the annotated content first, or remove the \
                     code",
                ),
            );
            return;
        }

        out.push(
            ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, text),
                format!("Unparsable content: tree-sitter could not parse '{first_line}'"),
            )
            .with_suggestion(
                "Check the CHAT format specification for valid syntax at this position",
            ),
        );
        return;
    }

    if node.is_missing() {
        // A MISSING `newline` is a LAYOUT omission, not a content invalidity:
        // the grammar requires a newline after `@End` (and other lines), but a
        // CHAT file legitimately omits the final trailing newline at EOF, and
        // CLAN `check` accepts that. Flagging it would wrongly reject every
        // newline-less file. Only content recovery nodes (ERROR, and MISSING
        // content tokens like `retrace_complete`) are invalidity here.
        if node.kind() == NEWLINE {
            return;
        }

        let start = node.start_byte();
        let end = node.end_byte();
        out.push(
            ParseError::new(
                ErrorCode::MissingRequiredElement,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, ""),
                format!(
                    "Missing required '{}': the document is incomplete here and was only \
                     parsed via tree-sitter recovery (recovery is not validity)",
                    node.kind()
                ),
            )
            .with_suggestion("Supply the element required by the CHAT grammar at this position"),
        );
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_recovery_nodes(child, source, out);
    }
}

/// Surface a NEW-backend carrier's `unexpected` sink (spec Section 7: children
/// that filled no grammar position) using the SAME [`collect_recovery_nodes`]
/// mapping the whole-tree backstop uses (`ERROR` -> `UnparsableContent`/E316,
/// `MISSING` -> `MissingRequiredElement`/E342), reported at each offending
/// node's exact span.
///
/// This is the shared mechanism every migrated visitor-driven carrier (Task
/// B1 onward) uses to surface its own `unexpected` sink, mirroring the
/// pattern `document_lowering.rs` established for Task B1
/// (`DocumentLowering::surface_unexpected`). Because the whole-tree backstop
/// still runs today and dedups by span overlap, an exact-span emission here
/// is auto-suppressed as a backstop duplicate, so this can never introduce a
/// NEW diagnostic while the backstop is present; it is the per-carrier
/// mechanism that lets the backstop be deleted once every region surfaces its
/// own recovery (migration Task D). In practice most carriers' `unexpected`
/// sinks are empty for valid CHAT and for the recovery fixtures exercised by
/// each cluster's characterization tests, so calling this is a no-op today.
pub(crate) fn surface_unexpected(
    unexpected: &[Node],
    source: &str,
    errors: &impl crate::error::ErrorSink,
) {
    if unexpected.is_empty() {
        return;
    }
    let mut candidates = Vec::new();
    for node in unexpected {
        collect_recovery_nodes(*node, source, &mut candidates);
    }
    for candidate in candidates {
        errors.report(candidate);
    }
}

/// Whether an `ERROR` node is a structural-incompleteness WRAPPER, i.e. it
/// directly contains recovered document-structure children (`line`, a
/// `*_header`, or `full_document`). Such an ERROR appears when a top-level
/// element is missing (no `@End`/`@Begin`) and tree-sitter wraps the whole
/// recovered document in one ERROR node; the validation layer reports that
/// precisely, so the backstop recurses into it rather than reporting the wrapper.
/// A content/leaf ERROR (a stray token, a malformed inline code) wraps no such
/// structure and returns false.
fn wraps_document_structure(node: Node) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|child| {
        let kind = child.kind();
        kind == LINE || kind == FULL_DOCUMENT || kind.ends_with("_header")
    })
}
