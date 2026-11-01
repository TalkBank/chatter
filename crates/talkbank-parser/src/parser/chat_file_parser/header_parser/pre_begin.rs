//! Parsing for headers permitted before `@Begin`, over the generated typed
//! traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#PID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Font_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, FullDocumentChild1Choice, FullDocumentChild1ChoiceBoundView, SourceBound,
};
use crate::model::{Header, Line, TextKind};
use talkbank_model::ParseOutcome;

use super::dispatch::content_header;
use super::helpers::header_separator;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::{ContentSlot, HeaderSite};

/// Parse and append one pre-`@Begin` header line.
///
/// The document's pre-begin repeat is typed as the four-way
/// `FullDocumentChild1Choice` (`@PID`, `@Window`, `@Color words`, `@Font`),
/// and this matches it exhaustively; until 2026-09-09 the lowering handed
/// over the raw node and this matched `node.kind()` with a catch-all
/// reporting "unknown pre-begin header type", which the choice makes
/// unreachable. All four share one shape,
/// `seq(prefix, header_sep, optional(free_text), newline)`, and one reader,
/// the text-header reader every other text header uses.
pub fn handle_pre_begin_header<'tree>(
    choice: SourceBound<'tree, '_, FullDocumentChild1Choice<'tree>>,
    errors: &impl ErrorSink,
    append: impl FnOnce(Line),
) {
    let input = choice.source();
    let span = span_of(choice.raw_node());
    let header = match parse_pre_begin_header(choice, errors) {
        Ok(ParseOutcome::Parsed(header)) => header,
        // A source-admission fault, already reported by the reader. (An
        // unparsable body is not a rejection: it is kept as an unknown
        // header with its text.)
        Ok(ParseOutcome::Rejected) => return,
        Err(failure) => {
            crate::parser::typed_cst::report_cst_failure(choice.raw_node(), input, failure, errors);
            return;
        }
    };
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        header_separator(choice.raw_node()),
        choice.raw_node(),
        input,
        errors,
    ) else {
        return;
    };
    append(Line::header_with_separator(span, header, separator));
}

/// Decode the generated pre-begin choice for either document or fragment APIs.
/// Both consumers share this exhaustive dispatch; neither keeps a kind subset.
pub(crate) fn parse_pre_begin_header<'tree>(
    choice: SourceBound<'tree, '_, FullDocumentChild1Choice<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Header>, crate::parser::typed_cst::CstFailure> {
    // One pre-`@Begin` text header: admit its envelope, then the shared text
    // reader over its `free_text` slot.
    macro_rules! text_header {
        ($typed:expr, $missing:literal, $kind:expr) => {{
            let typed = $typed;
            let site = HeaderSite::bound(typed);
            let associated = crate::parser::typed_cst::extract_envelope(typed)?;
            content_header(
                &site,
                associated.field_child_2().slot(),
                &associated.children().unexpected,
                &ContentSlot {
                    missing: $missing,
                    suggested_fix: None,
                },
                errors,
                $kind,
            )
        }};
    }
    Ok(match choice.view() {
        FullDocumentChild1ChoiceBoundView::PidHeader(pid) => {
            text_header!(pid, "Missing PID value in @PID header", TextKind::Pid)
        }
        FullDocumentChild1ChoiceBoundView::WindowHeader(window) => text_header!(
            window,
            "Missing or invalid @Window geometry",
            TextKind::Window
        ),
        FullDocumentChild1ChoiceBoundView::ColorWordsHeader(color_words) => text_header!(
            color_words,
            "Missing or invalid @Color words content",
            TextKind::ColorWords
        ),
        FullDocumentChild1ChoiceBoundView::FontHeader(font) => {
            text_header!(font, "Missing or invalid @Font content", TextKind::Font)
        }
    })
}
