//! Shared classification rules for dedicated recovery diagnostics.
//!
//! Unparsable regions are reported from several independent sites (the
//! file-level error analysis, the main-tier contents loop, the
//! dependent-tier analyzer, the whole-tree recovery backstop). Each site
//! calls these rules so the classification cannot drift between them; the
//! caller supplies whatever position or typed-CST gate its context affords
//! (for example, “this fragment is the first content item”).

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use crate::node_types::{
    LEFT_DOUBLE_QUOTE as LEFT_DOUBLE_QUOTE_NODE, RIGHT_DOUBLE_QUOTE as RIGHT_DOUBLE_QUOTE_NODE,
};
use std::ops::Range;
use talkbank_model::chars::{LEFT_DOUBLE_QUOTE, RIGHT_DOUBLE_QUOTE};
use tree_sitter::Node;

/// Result of scanning the structured quotation delimiters inside one recovery
/// subtree.
///
/// `Absent` and `Balanced` are deliberately separate. A matched quotation can
/// sit inside an `ERROR` caused by some other construct; treating “contains a
/// curly quote” as “unbalanced quotation” produced a false E242 on exactly
/// that shape. Only `Unbalanced` can construct the diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QuotationDelimiterScan {
    /// The recovery subtree has no structured quotation delimiter nodes.
    Absent,
    /// Every structured opening delimiter has an ordered closing partner.
    Balanced,
    /// The tree itself proves which delimiter lacks a partner.
    Unbalanced(UnbalancedQuotationDelimiter),
}

/// One quotation delimiter for which the recovery CST proves no partner.
///
/// Fields are private so callers can consume a producer-issued finding but
/// cannot manufacture one from arbitrary offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnbalancedQuotationDelimiter {
    delimiter: UnpairedQuotationDelimiter,
    span: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnpairedQuotationDelimiter {
    /// U+201D occurred without an earlier unmatched U+201C.
    Closing,
    /// U+201C remained unmatched at the end of the subtree.
    Opening,
}

impl UnbalancedQuotationDelimiter {
    /// Consume the structural finding into its E242 diagnostic.
    pub(crate) fn into_diagnostic(self, source: &str) -> ParseError {
        let (found, message, suggestion) = match self.delimiter {
            UnpairedQuotationDelimiter::Closing => (
                RIGHT_DOUBLE_QUOTE,
                "Quotation end (U+201D) without an earlier quotation begin (U+201C)",
                "Add the opening curly double quote (U+201C), or remove the unmatched closing quote",
            ),
            UnpairedQuotationDelimiter::Opening => (
                LEFT_DOUBLE_QUOTE,
                "Quotation begin (U+201C) without a later quotation end (U+201D)",
                "Close the quotation with a curly double quote (U+201D)",
            ),
        };
        let span = self.span;
        ParseError::new(
            ErrorCode::UnbalancedQuotation,
            Severity::Error,
            SourceLocation::from_offsets(span.start, span.end),
            ErrorContext::new(source, span, found),
            message,
        )
        .with_suggestion(suggestion)
    }
}

/// Scan quotation delimiters from CST structure rather than ERROR-node text.
///
/// The grammar gives U+201C and U+201D their own named nodes even when a whole
/// main tier is wrapped in a top-level `ERROR`. Walking those nodes retains the
/// one fact raw text cannot provide: whether the delimiters are actually
/// balanced and ordered.
pub(crate) fn scan_quotation_delimiters(node: Node<'_>) -> QuotationDelimiterScan {
    fn visit(
        node: Node<'_>,
        opens: &mut Vec<Range<usize>>,
        first_unmatched_close: &mut Option<Range<usize>>,
        saw_delimiter: &mut bool,
    ) {
        match node.kind() {
            LEFT_DOUBLE_QUOTE_NODE => {
                *saw_delimiter = true;
                opens.push(node.start_byte()..node.end_byte());
                return;
            }
            RIGHT_DOUBLE_QUOTE_NODE => {
                *saw_delimiter = true;
                if opens.pop().is_none() && first_unmatched_close.is_none() {
                    *first_unmatched_close = Some(node.start_byte()..node.end_byte());
                }
                return;
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            visit(child, opens, first_unmatched_close, saw_delimiter);
        }
    }

    let mut opens = Vec::new();
    let mut first_unmatched_close = None;
    let mut saw_delimiter = false;
    visit(
        node,
        &mut opens,
        &mut first_unmatched_close,
        &mut saw_delimiter,
    );

    if let Some(closing) = first_unmatched_close {
        QuotationDelimiterScan::Unbalanced(UnbalancedQuotationDelimiter {
            delimiter: UnpairedQuotationDelimiter::Closing,
            span: closing,
        })
    } else if let Some(opening) = opens.into_iter().next() {
        QuotationDelimiterScan::Unbalanced(UnbalancedQuotationDelimiter {
            delimiter: UnpairedQuotationDelimiter::Opening,
            span: opening,
        })
    } else if saw_delimiter {
        QuotationDelimiterScan::Balanced
    } else {
        QuotationDelimiterScan::Absent
    }
}

/// CHECK-52 family: a bracket code whose first inner character is one of
/// `/`, `<`, `>`, `:`, `"` (retraces, overlap markers, replacements, the
/// quotation marker). Returns the code token for the message (through
/// the first `]` when present). The caller is responsible for asserting
/// the LEADING position; this rule only recognizes the shape.
pub(crate) fn leading_postfix_annotation(content: &str) -> Option<&str> {
    let rest = content.strip_prefix('[')?;
    if rest.starts_with(['/', '<', '>', ':', '"']) {
        Some(match content.find(']') {
            Some(close) => &content[..=close],
            None => "[",
        })
    } else {
        None
    }
}

/// E760 shape: a whitespace-delimited %mor item beginning with the `|`
/// separator (`|we`): its part-of-speech field is empty. Returns the
/// offending item. The caller supplies the mor-tier gate.
///
/// The FIRST token is only an empty-POS item when the analyzed text
/// itself starts at an item boundary: tree-sitter splits malformations
/// like `n|dog|cat` (CHECK 79, two pipes) or malformed compounds
/// (CHECK 87) into a parsed head (`n|dog`) plus an ERROR fragment
/// (`|cat`) whose leading pipe is a SPLIT TAIL, not an empty POS field.
/// Callers with only a fragment must pass `starts_at_item_boundary`
/// accordingly (see [`at_item_boundary`]); tokens after the first are
/// whitespace-delimited by construction and always eligible.
pub(crate) fn mor_item_with_empty_pos(text: &str, starts_at_item_boundary: bool) -> Option<&str> {
    text.split_whitespace()
        .enumerate()
        .find(|(index, token)| {
            (starts_at_item_boundary || *index > 0) && token.starts_with('|') && token.len() > 1
        })
        .map(|(_, token)| token)
}

/// Whether byte offset `start` sits at an ITEM boundary on its line:
/// preceded by whitespace (space or tab) or at the very start of the
/// line. A fragment whose preceding byte is any other character is the
/// tail of a split item, never a free-standing item.
pub(crate) fn at_item_boundary(source: &str, start: usize) -> bool {
    let Some(prefix) = source.get(..start) else {
        return false;
    };
    matches!(prefix.chars().next_back(), None | Some(' ' | '\t' | '\n'))
}

/// Whether byte offset `start` sits at the START of a main tier's
/// content: its line begins with `*`, has the `:<tab>` separator, and
/// everything between that tab and `start` is (at most) spaces. Used to
/// distinguish a LEADING annotation fragment (E759) from one glued after
/// a word (E757/E375 territory) when the analyzer has only the fragment
/// node and no traversal context.
pub(crate) fn at_main_tier_content_start(source: &str, start: usize) -> bool {
    let Some(prefix) = source.get(..start) else {
        return false;
    };
    let line_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let line_prefix = &prefix[line_start..];
    if !line_prefix.starts_with('*') {
        return false;
    }
    match line_prefix.find(":\t") {
        Some(sep) => line_prefix[sep + 2..].chars().all(|c| c == ' '),
        None => false,
    }
}

/// Whether byte offset `start` sits on a `%mor` / `%trn` tier line. Same
/// no-context situation as [`at_main_tier_content_start`]: the analyzer
/// may hold only the fragment (`|we`), so the tier is derived from the
/// enclosing source line.
pub(crate) fn on_mor_tier_line(source: &str, start: usize) -> bool {
    let Some(prefix) = source.get(..start) else {
        return false;
    };
    let line_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let line_prefix = &prefix[line_start..];
    line_prefix.starts_with("%mor:") || line_prefix.starts_with("%trn:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_check_52_family() {
        assert_eq!(leading_postfix_annotation("[/] we go"), Some("[/]"));
        assert_eq!(leading_postfix_annotation("[//] no"), Some("[//]"));
        assert_eq!(leading_postfix_annotation("[<] hi"), Some("[<]"));
        assert_eq!(
            leading_postfix_annotation("[: because] x"),
            Some("[: because]")
        );
        assert_eq!(leading_postfix_annotation("[\"] said"), Some("[\"]"));
        // Legal leading codes and non-codes do not match.
        assert_eq!(leading_postfix_annotation("[- heb] word"), None);
        assert_eq!(leading_postfix_annotation("[^ note] word"), None);
        assert_eq!(leading_postfix_annotation("word [/] ."), None);
        // Unclosed code still names the bracket.
        assert_eq!(leading_postfix_annotation("[/ ."), Some("["));
    }

    #[test]
    fn recognizes_empty_pos_items() {
        assert_eq!(mor_item_with_empty_pos("|we v|go .", true), Some("|we"));
        assert_eq!(
            mor_item_with_empty_pos("pro|we v|go |home .", true),
            Some("|home")
        );
        assert_eq!(mor_item_with_empty_pos("pro|we v|go .", true), None);
        // A lone pipe is a different malformation, not an empty-POS item.
        assert_eq!(mor_item_with_empty_pos("| we", true), None);
        // A fragment NOT at an item boundary is a split tail (CHECK 79
        // `n|dog|cat` splits to head `n|dog` + fragment `|cat`): its
        // leading pipe must not classify as empty POS...
        assert_eq!(mor_item_with_empty_pos("|cat .", false), None);
        // ...but a genuine empty-POS item LATER in the same fragment
        // still classifies.
        assert_eq!(mor_item_with_empty_pos("|cat |we .", false), Some("|we"));
    }

    #[test]
    fn item_boundary_is_whitespace_or_line_start() {
        let source = "%mor:	n|dog|cat |we .";
        let at = |needle: &str| source.find(needle).map(|i| at_item_boundary(source, i));
        assert_eq!(at("n|dog"), Some(true), "after tab = boundary");
        assert_eq!(at("|cat"), Some(false), "mid-item split tail");
        assert_eq!(at("|we"), Some(true), "after space = boundary");
        assert!(at_item_boundary(source, 0), "line start = boundary");
    }
}
