//! File-level `ERROR` analysis and fallback diagnostic routing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{AsRawNode, SourceSlice};

/// Report top-level recovery without inventing header or utterance structure
/// from its text. Delimiter diagnostics require actual parsed delimiter nodes.
pub(crate) fn analyze_error_node(bound: SourceSlice<'_, '_>, errors: &impl ErrorSink) {
    let node = bound.raw_node();
    let source = bound.source();
    let error_text = bound.text();
    let start = node.start_byte();
    let end = node.end_byte();

    if let super::dedicated::QuotationDelimiterScan::Unbalanced(finding) =
        super::dedicated::scan_quotation_delimiters(node)
    {
        errors.report(finding.into_diagnostic(source));
        return;
    }

    // Generic file-level error
    errors.report(
        ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            format!(
                "Unparsable content at file level: '{}'",
                match error_text.lines().next() {
                    Some(line) => line,
                    None => error_text,
                }
            ),
        )
        .with_suggestion("Check CHAT format specification for valid syntax at this position"),
    );
}
