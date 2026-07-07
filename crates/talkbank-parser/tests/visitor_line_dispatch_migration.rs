//! Characterization tests for `DocumentLowering::dispatch_line` migrated to
//! the generated `extract_line` typed visitor (Task 2a).
//!
//! These tests pin the OBSERVABLE behaviour of `dispatch_line` at the real
//! parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). The migration replaces the hand-walked `line_child.kind()`
//! string-dispatch loop with exhaustive typed `NodeSlot<LineChoice>` matching,
//! but it is BEHAVIOUR-PRESERVING: the model and recovery diagnostics must not
//! change.
//!
//! All diagnostic values were captured by RUNNING the pre-migration parser
//! (not guessed). The test PASSES on the current code and MUST STAY GREEN
//! after the `dispatch_line` rewrite.
//!
//! # Coverage
//!
//! - Case (a): a valid CHAT file with header lines and utterance lines ->
//!   exact line structure, ZERO diagnostics.
//! - Case (b1): a file containing a blank line -> E747 `BlankLineNotAllowed`
//!   at the exact byte span, with the pinned message.
//! - Case (b2): a file containing an unsupported line -> E326
//!   `UnexpectedLineType` "Unsupported line skipped: ..." at the exact byte
//!   span.
//! - The `Error(error_node)` arm of `NodeSlot<LineChoice>` maps to the same
//!   `analyze_line_error(error_node, line_node, ...)` call as the old
//!   `is_error()` branch (identical arguments, identical path). That code
//!   path is structurally equivalent between old and new; divergence would
//!   surface in the gate tests (`parser_equivalence`, `roundtrip_reference_corpus`,
//!   `chatter_matches_check`) which run against the full error corpus.
//! - The `Missing(_)` and `Absent` arms map to the old `is_missing() ->
//!   continue` path (no diagnostic), which is also covered by the gate tests.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// ---------------------------------------------------------------------------
// Inline CHAT sources (exact byte positions annotated for span assertions)
// ---------------------------------------------------------------------------

/// A minimal valid CHAT file. Every header and utterance line exercises the
/// `Present(Header(_))` and `Present(Utterance(_))` arms. Zero diagnostics
/// expected.
///
/// Byte layout (for documentation; not needed for this case's assertions):
/// ```
/// [0..6)   @UTF8\n
/// [6..13)  @Begin\n
/// [13..29) @Languages:\teng\n
/// [29..54) @Participants:\tCHI Child\n
/// [54..87) @ID:\teng|corpus|CHI|||||Child|||\n
/// [87..101) @Comment:\tMinimal\n    (17 bytes: @Comment:\tMinimal\n)
/// ...
/// ```
const VALID_MINIMAL: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Comment:\tMinimal valid file
*CHI:\thello .
@End
";

/// A file with a blank line between two utterances. The blank line is an empty
/// `\n` that tree-sitter classifies as `blank_line` (grammar rule
/// `blank_line: $ => $.newline`). `dispatch_line` routes the `line(blank_line)`
/// node to the `BlankLineNotAllowed` diagnostic (E747) at the blank_line
/// node's span.
///
/// Byte layout:
/// ```
/// [0..6)   @UTF8\n
/// [6..13)  @Begin\n
/// [13..29) @Languages:\teng\n
/// [29..54) @Participants:\tCHI Child\n
/// [54..87) @ID:\teng|corpus|CHI|||||Child|||\n
/// [87..98) *CHI:\thi .\n
/// [98..99) \n  <- blank_line node, E747 expected here
/// [99..111) *CHI:\tbye .\n
/// [111..116) @End\n
/// ```
const WITH_BLANK_LINE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
*CHI:\thi .
\n*CHI:\tbye .
@End
";

/// A file with an unsupported line (content not starting with `@`, `*`, or
/// `%`). The grammar's `unsupported_line` rule matches it as a catch-all.
/// `dispatch_line` routes the `line(unsupported_line)` node to E326
/// `UnexpectedLineType` "Unsupported line skipped: ...".
///
/// Byte layout:
/// ```
/// [0..6)   @UTF8\n
/// [6..13)  @Begin\n
/// [13..29) @Languages:\teng\n
/// [29..54) @Participants:\tCHI Child\n
/// [54..87) @ID:\teng|corpus|CHI|||||Child|||\n
/// [87..101) *CHI:\thello .\n
/// [101..116) junk line here\n  <- unsupported_line node, E326 expected here
/// [116..121) @End\n
/// ```
const WITH_UNSUPPORTED_LINE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
*CHI:\thello .
junk line here
@End
";

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// A stable tag for each `Header` variant, used to assert line structure
/// without depending on header-payload internals.
fn header_tag(h: &Header) -> &'static str {
    match h {
        Header::Utf8 => "Utf8",
        Header::Begin => "Begin",
        Header::End => "End",
        Header::Languages { .. } => "Languages",
        Header::Participants { .. } => "Participants",
        Header::ID(_) => "ID",
        Header::Comment { .. } => "Comment",
        _ => "Other",
    }
}

/// Render each line as a stable structural tag for order-sensitive assertions.
fn line_tags(chat_lines: &[Line]) -> Vec<String> {
    chat_lines
        .iter()
        .map(|l| match l {
            Line::Header { header, .. } => format!("Header({})", header_tag(header)),
            Line::Utterance(_) => "Utterance".to_string(),
        })
        .collect()
}

/// Parse `input` at the real streaming boundary and return the line tags plus
/// every collected diagnostic as `(code, start, end, message)` tuples.
fn parse_lines_and_diags(input: &str) -> (Vec<String>, Vec<(String, u32, u32, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let diags = errors
        .into_vec()
        .into_iter()
        .map(|d| {
            (
                d.code.as_str().to_string(),
                d.location.span.start,
                d.location.span.end,
                d.message,
            )
        })
        .collect();
    (line_tags(&chat.lines.0), diags)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Case (a): A valid file with headers and utterances should produce the exact
/// expected line order and ZERO diagnostics. This pins the
/// `Present(Header(_))` and `Present(Utterance(_))` arms of the new dispatch.
#[test]
fn valid_file_parses_to_expected_line_structure_and_zero_diagnostics() {
    let (tags, diags) = parse_lines_and_diags(VALID_MINIMAL);

    assert_eq!(
        tags,
        vec![
            "Header(Utf8)",
            "Header(Begin)",
            "Header(Languages)",
            "Header(Participants)",
            "Header(ID)",
            "Header(Comment)",
            "Utterance",
            "Header(End)",
        ],
        "valid minimal file must parse to the exact header/utterance line order"
    );
    assert!(
        diags.is_empty(),
        "valid minimal file must produce zero diagnostics, got: {diags:?}"
    );
}

/// Case (b1): A blank line between utterances must produce E747
/// `BlankLineNotAllowed` at the exact blank_line node span (98..99), with
/// the pinned message. This pins the `Present(BlankLine(_))` arm.
///
/// Diagnostic captured by running the pre-migration parser on `WITH_BLANK_LINE`.
#[test]
fn blank_line_between_utterances_produces_e747() {
    let (tags, diags) = parse_lines_and_diags(WITH_BLANK_LINE);

    // The blank line contributes NO line to the model (no Line pushed for it).
    assert_eq!(
        tags,
        vec![
            "Header(Utf8)",
            "Header(Begin)",
            "Header(Languages)",
            "Header(Participants)",
            "Header(ID)",
            "Utterance",
            "Utterance",
            "Header(End)",
        ],
        "blank line must not contribute a model Line; both utterances must appear"
    );

    let e747: Vec<&(String, u32, u32, String)> =
        diags.iter().filter(|(c, _, _, _)| c == "E747").collect();
    assert_eq!(
        e747.len(),
        1,
        "exactly one E747 expected for a single blank line, got: {diags:?}"
    );
    let (code, start, end, msg) = e747[0];
    assert_eq!(code.as_str(), "E747");
    assert_eq!(
        (*start, *end),
        (98, 99),
        "E747 blank-line span must be (98..99): one byte for the lone newline"
    );
    assert_eq!(
        msg.as_str(),
        "Blank lines are not allowed",
        "E747 message must be exactly 'Blank lines are not allowed'"
    );
}

/// Case (b2): An unsupported line (not starting with `@`, `*`, or `%`) must
/// produce E326 `UnexpectedLineType` "Unsupported line skipped: ..." at the
/// exact unsupported_line node span (101..116), including the trailing newline.
/// This pins the `Present(UnsupportedLine(_))` arm.
///
/// Diagnostic captured by running the pre-migration parser on
/// `WITH_UNSUPPORTED_LINE`.
#[test]
fn unsupported_line_produces_e326() {
    let (tags, diags) = parse_lines_and_diags(WITH_UNSUPPORTED_LINE);

    // The unsupported line is skipped; no Line pushed for it.
    assert_eq!(
        tags,
        vec![
            "Header(Utf8)",
            "Header(Begin)",
            "Header(Languages)",
            "Header(Participants)",
            "Header(ID)",
            "Utterance",
            "Header(End)",
        ],
        "unsupported line must not contribute a model Line"
    );

    let e326: Vec<&(String, u32, u32, String)> =
        diags.iter().filter(|(c, _, _, _)| c == "E326").collect();
    assert_eq!(
        e326.len(),
        1,
        "exactly one E326 expected for a single unsupported line, got: {diags:?}"
    );
    let (code, start, end, msg) = e326[0];
    assert_eq!(code.as_str(), "E326");
    assert_eq!(
        (*start, *end),
        (101, 116),
        "E326 span must be (101..116): the full unsupported_line node including newline"
    );
    assert_eq!(
        msg.as_str(),
        "Unsupported line skipped: junk line here",
        "E326 message must include the trimmed line text"
    );
}
