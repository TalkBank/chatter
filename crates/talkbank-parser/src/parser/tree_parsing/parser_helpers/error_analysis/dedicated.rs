//! Quotation diagnostics justified by delimiter nodes in a recovery subtree.
//! Only a producer-issued unbalanced-delimiter finding can emit the diagnostic.

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
