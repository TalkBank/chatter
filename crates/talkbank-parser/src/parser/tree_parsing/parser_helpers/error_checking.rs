//! Recursive traversal helpers for collecting tree-sitter recovery errors.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use crate::node_types::{GRA_DEPENDENT_TIER, MOR_DEPENDENT_TIER, NEWLINE, PHO_DEPENDENT_TIER};
use tree_sitter::Node;

use super::error_analysis::analyze_dependent_tier_error_with_context;

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
/// `Severity::Error`. An `ERROR` becomes a dedicated code when its structure
/// proves one, otherwise [`ErrorCode::UnparsableContent`] (E316); a `MISSING`
/// node becomes [`ErrorCode::MissingRequiredElement`] (E342).
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
/// itself. The caller owns `out` (mirroring [`check_for_errors_recursive_with_context`]) so
/// it can dedup against already-reported spans before emitting.
pub(crate) fn collect_recovery_nodes(node: Node, source: &str, out: &mut Vec<ParseError>) {
    if node.is_error() {
        // A structural-incompleteness ERROR wraps the recovered document: when a
        // top-level element is missing (for example no @End), the whole
        // `document` rule fails to complete and tree-sitter returns an ERROR node
        // AROUND the recovered headers/lines. The validation layer reports that
        // precisely (for example E502 missing @End), so reporting the
        // wrapper too would be a misleading, redundant whole-file E316. Recurse
        // into it to surface only LOCALIZED recovery nodes; do not report the
        // wrapper itself. A leaf/content ERROR (a stray token, a malformed code)
        // wraps no document structure and is reported normally below.
        if let Some(wrapper) = DocumentRecoveryWrapper::admit(node, source) {
            wrapper.collect_nested(source, out);
            return;
        }

        // E242: quotation balance is recoverable from typed delimiter nodes
        // even when tree-sitter wraps the complete main tier in one ERROR.
        // A matched pair inside an otherwise malformed region is explicitly
        // not this diagnostic.
        if let super::error_analysis::dedicated::QuotationDelimiterScan::Unbalanced(finding) =
            super::error_analysis::dedicated::scan_quotation_delimiters(node)
        {
            out.push(finding.into_diagnostic(source));
            return;
        }

        let start = node.start_byte();
        let end = node.end_byte();
        // The node's text drives every classification below; a node whose
        // bytes are not UTF-8 is reported as that fact rather than classified
        // over an empty text the analyzer invented.
        let text = match node.utf8_text(source.as_bytes()) {
            Ok(text) => text,
            Err(error) => {
                out.push(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, ""),
                    format!("UTF-8 decoding error in recovery node: {error}"),
                ));
                return;
            }
        };
        // The first line as `str::lines` would cut it (at the first `\n`, its
        // `\r` going with the trim), without an empty text invented for a
        // text that has no line at all.
        let first_line = match text.split_once('\n') {
            Some((first, _)) => first,
            None => text,
        }
        .trim();

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
                super::error_analysis::dedicated::annotation_at_utterance_start(
                    code_token,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, text),
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

/// Surface a carrier's `unexpected` sink WITH its displaced nodes.
///
/// Replaced `surface_unexpected` on 2026-09-09: that verb reported only what
/// [`collect_recovery_nodes`] reports (ERROR and MISSING nodes) and nothing
/// else, so a WELL-FORMED node that filled no grammar position (a displaced
/// sibling that itself parsed cleanly) was dropped without a diagnostic,
/// where the hand walks this traversal replaces named every child they did
/// not expect. Here such a node is reported as unexpected at `context`, and a
/// node carrying recovery goes to the classifier as before (dedicated
/// recovery code or generic `UnparsableContent`/E316 for `ERROR`;
/// `MissingRequiredElement`/E342 for `MISSING`), reported at the most
/// specific structurally proven span.
///
/// This is the shared mechanism every migrated visitor-driven carrier (Task
/// B1 onward) uses to surface its own `unexpected` sink. Because the
/// whole-tree backstop still runs today and dedups by span overlap, a
/// recovery-node emission here is auto-suppressed as a backstop duplicate, so
/// that half can never introduce a NEW diagnostic while the backstop is
/// present; it is the per-carrier mechanism that lets the backstop be deleted
/// once every region surfaces its own recovery (migration Task D). In
/// practice most carriers' `unexpected` sinks are empty for valid CHAT and
/// for the recovery fixtures exercised by each cluster's characterization
/// tests, so calling this is usually a no-op.
pub(crate) fn surface_displaced(
    unexpected: &[Node],
    context: &str,
    source: &str,
    errors: &impl crate::error::ErrorSink,
) {
    for node in unexpected {
        if node.is_error() || node.is_missing() || node.has_error() {
            let mut candidates = Vec::new();
            collect_recovery_nodes(*node, source, &mut candidates);
            for candidate in candidates {
                errors.report(candidate);
            }
        } else {
            errors.report(crate::parser::tree_parsing::helpers::unexpected_node_error(
                *node, source, context,
            ));
        }
    }
}

/// An ERROR whose children are accounted for by the document grammar.
/// Only this admitted wrapper may defer its own diagnostic to missing-header
/// validation. A recognizable header beside unconsumed malformed text is not
/// enough: suppressing that region would erase its only syntax diagnostic.
struct DocumentRecoveryWrapper<'tree>(Node<'tree>);

impl<'tree> DocumentRecoveryWrapper<'tree> {
    fn admit(node: Node<'tree>, source: &str) -> Option<Self> {
        use crate::generated_traversal::{
            BeginHeaderNode, EndHeaderNode, FromNodeKind, FullDocumentNode, HeaderChoice, LineNode,
            MainTierNode, PreBeginHeaderChoice, Utf8HeaderNode, UtteranceNode,
        };
        if !node.is_error() {
            return None;
        }
        // Missing-header validation can replace a wrapper diagnostic only at
        // the document position. A stray header beside a complete document is
        // an error in its own right, even if every child looks structural.
        if let Some(parent) = node.parent()
            && (parent.kind() != crate::node_types::SOURCE_FILE || parent.child_count() != 1)
        {
            return None;
        }
        let mut has_structure = false;
        let mut cursor = node.walk();
        let mut children = node.children(&mut cursor);
        while let Some(child) = children.next() {
            // Nested recovery remains visible through collect_nested. Extras
            // are grammar-owned trivia, not unconsumed header/body tokens.
            if child.is_error() || child.is_missing() || child.is_extra() {
                continue;
            }
            // Recovery can leave the last main tier unwrapped when @End is
            // absent. Classify complete constructs through generated types;
            // a prefix/contents token alone cannot certify a complete header.
            let structural = FullDocumentNode::from_node(child).is_some()
                || LineNode::from_node(child).is_some()
                || MainTierNode::from_node(child).is_some()
                || UtteranceNode::from_node(child).is_some()
                || HeaderChoice::from_node(child).is_some()
                || PreBeginHeaderChoice::from_node(child).is_some()
                || Utf8HeaderNode::from_node(child).is_some()
                || BeginHeaderNode::from_node(child).is_some()
                || EndHeaderNode::from_node(child).is_some();
            if !structural {
                // Without the final newline, recovery may also flatten the
                // final main tier. Admit only a complete terminal sequence,
                // never arbitrary body tokens beside a recognizable header.
                return (has_structure
                    && node.end_byte() == source.len()
                    && crate::parser::terminal_main_tier::TerminalMainTier::admit(
                        child, children, source,
                    )
                    .is_some())
                .then_some(Self(node));
            }
            has_structure = true;
        }
        has_structure.then_some(Self(node))
    }

    fn collect_nested(self, source: &str, out: &mut Vec<ParseError>) {
        let mut cursor = self.0.walk();
        for child in self.0.children(&mut cursor) {
            collect_recovery_nodes(child, source, out);
        }
    }
}
