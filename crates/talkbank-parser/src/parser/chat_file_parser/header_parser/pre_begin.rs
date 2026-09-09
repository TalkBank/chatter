//! Parsing for headers permitted before `@Begin`, over the generated typed
//! traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#PID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Font_Header>

use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AsRawNode, ChildSlot, ColorWordsHeaderNode, FontHeaderNode, FreeTextNode,
    FullDocumentChild1Choice, NamedKind, NoChild, SlotView, WindowHeaderNode,
    extract_color_words_header, extract_font_header, extract_window_header,
};
use crate::model::{self, Header, Line};
use tree_sitter::Node;

use super::helpers::header_separator;
use crate::parser::tree_parsing::header::parse_pid_header;
use crate::parser::tree_parsing::parser_helpers::{surface_displaced, unknown_header_from_node};

/// Parse and append one pre-`@Begin` header line.
///
/// The document's pre-begin repeat is typed as the four-way
/// `FullDocumentChild1Choice` (`@PID`, `@Window`, `@Color words`, `@Font`),
/// and this matches it exhaustively; until 2026-09-09 the lowering handed
/// over the raw node and this matched `node.kind()` with a catch-all
/// reporting "unknown pre-begin header type", which the choice makes
/// unreachable. `@PID` has a parser of its own; the other three share one
/// shape, `seq(prefix, header_sep, free_text, newline)`, and one reader.
pub fn handle_pre_begin_header(
    choice: &FullDocumentChild1Choice<'_>,
    span: Span,
    input: &str,
    errors: &impl ErrorSink,
    lines: &mut Vec<Line>,
) {
    let header = match choice {
        FullDocumentChild1Choice::PidHeader(pid) => {
            let header_errors = ErrorCollector::new();
            let header = parse_pid_header(*pid, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            header
        }
        FullDocumentChild1Choice::WindowHeader(window) => {
            let children = extract_window_header(*window);
            surface_displaced(&children.unexpected, "window_header", input, errors);
            free_text_header(
                children.child_2.slot(),
                window.raw_node(),
                FreeText {
                    kind: WindowHeaderNode::KIND,
                    missing: "Missing or invalid @Window geometry",
                    malformed: "Malformed @Window header",
                },
                input,
                errors,
                |geometry| Header::Window {
                    geometry: model::WindowGeometry::new(geometry),
                },
            )
        }
        FullDocumentChild1Choice::ColorWordsHeader(color_words) => {
            let children = extract_color_words_header(*color_words);
            surface_displaced(&children.unexpected, "color_words_header", input, errors);
            free_text_header(
                children.child_2.slot(),
                color_words.raw_node(),
                FreeText {
                    kind: ColorWordsHeaderNode::KIND,
                    missing: "Missing or invalid @Color words content",
                    malformed: "Malformed @Color words header",
                },
                input,
                errors,
                |colors| Header::ColorWords {
                    colors: model::ColorWordList::new(colors),
                },
            )
        }
        FullDocumentChild1Choice::FontHeader(font) => {
            let children = extract_font_header(*font);
            surface_displaced(&children.unexpected, "font_header", input, errors);
            free_text_header(
                children.child_2.slot(),
                font.raw_node(),
                FreeText {
                    kind: FontHeaderNode::KIND,
                    missing: "Missing or invalid @Font content",
                    malformed: "Malformed @Font header",
                },
                input,
                errors,
                |font| Header::Font {
                    font: model::FontSpec::new(font),
                },
            )
        }
    };
    let separator = header_separator(choice.raw_node());
    lines.push(Line::header_with_separator(header, span, separator));
}

/// What a free-text header says when its text position does not deliver.
struct FreeText {
    /// The header node's own kind, for the diagnostic's context.
    kind: &'static str,
    /// The diagnostic's message.
    missing: &'static str,
    /// The `parse_reason` of the `Header::Unknown` built in its place.
    malformed: &'static str,
}

/// The header a free-text position builds: `build` over the text of a
/// present position; for any other state (a MISSING placeholder, an ERROR,
/// or text that does not decode) the header is reported malformed and
/// lowered as `Header::Unknown`, as the positional `child(2)` read did.
fn free_text_header(
    text: &ChildSlot<'_, FreeTextNode<'_>>,
    header_node: Node,
    words: FreeText,
    input: &str,
    errors: &impl ErrorSink,
    build: impl FnOnce(String) -> Header,
) -> Header {
    // What the position holds. An EMPTY text is no value: a line with
    // nothing after the tab leaves the position present with zero-width
    // text (the placeholder tree-sitter inserts is elsewhere on the line),
    // and until 2026-09-09 that empty text became `Header::Window {
    // geometry: "" }`, a value nobody wrote. It lowers as `Header::Unknown`
    // with no report of its own: the placeholder is the whole-tree pass's
    // (E342) and the malformed header is validation's (E525), and an empty
    // value is neither an internal traversal failure nor a decode error.
    enum Held {
        Value(String),
        Empty,
        Nothing,
    }
    let held = match text.view() {
        SlotView::Present(text) => match text.raw_node().utf8_text(input.as_bytes()) {
            Ok("") => Held::Empty,
            Ok(value) => Held::Value(value.to_string()),
            Err(_) => Held::Nothing,
        },
        SlotView::Missing(_) | SlotView::Error(_) | SlotView::Absent(NoChild) => Held::Nothing,
    };
    match held {
        Held::Value(value) => build(value),
        Held::Empty => unknown_header_from_node(header_node, input, words.malformed, None),
        Held::Nothing => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(header_node.start_byte(), header_node.end_byte()),
                ErrorContext::new(
                    input,
                    header_node.start_byte()..header_node.end_byte(),
                    words.kind,
                ),
                words.missing,
            ));
            unknown_header_from_node(header_node, input, words.malformed, None)
        }
    }
}
