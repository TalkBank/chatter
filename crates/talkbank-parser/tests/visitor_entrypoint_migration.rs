//! Characterization tests for the document/line entry point as it is migrated
//! onto the generated `GrammarTraversal` visitor (Task 1 of the visitor-driven
//! parser migration).
//!
//! These tests pin the OBSERVABLE behavior of the top-level `full_document` ->
//! line walk at the real parser boundary (`parse_chat_file_streaming` ->
//! `ChatFile` + collected diagnostics). The migration replaces the hand-walked
//! `match child.kind()` dispatch over `full_document` children with the
//! generated `extract_full_document` / `NodeSlot` dispatch, but it is
//! behavior-preserving: the model and the recovery diagnostics must not change.
//!
//! Both diagnostic values below were captured by RUNNING the pre-migration
//! parser (not guessed): a valid minimal file parses to a known line structure
//! with zero diagnostics, and a file with a stray top-level `@Date:` that drives
//! tree-sitter into a document-level ERROR recovery node produces exactly the
//! recovered-line structure plus the E316 backstop diagnostic and the E747
//! blank-line diagnostic, at the exact spans pinned here.
//!
//! WATCH-ITEM (double-emission): the document-level ERROR for `@Date:` is
//! recovered into a `Header::Date` line by the entry point AND flagged E316 by
//! the still-present whole-tree `collect_recovery_nodes` backstop. The migrated
//! handler must emit any document-level recovery diagnostic at the EXACT node
//! span so the backstop's span-dedup neither drops nor duplicates it; this test
//! pins that the E316 count stays exactly 1 at span (87..93).

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

/// One diagnostic: (code, span start, span end, message).
type Diag = (String, u32, u32, String);

/// A clean, minimal, valid CHAT file: required headers, one comment, one
/// utterance, `@End`. Drives every entry-point arm that exists for valid input
/// (utf8/begin/end headers, pre-begin-header-free body, header lines, utterance
/// line) with no recovery nodes.
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

/// A file with a stray top-level `@Date:` (empty value). Tree-sitter cannot
/// parse this as a date header, so it emits an `ERROR` node that is a DIRECT
/// child of `full_document`, sitting between the recovered header lines and
/// `@End`. The entry point recovers it into a `Header::Date` line, and the
/// whole-tree backstop additionally flags it E316 (recovery is not validity).
/// The trailing newline after the ERROR node becomes a `blank_line`, flagged
/// E747.
const STRAY_TOP_LEVEL_DATE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Date:
@End
";

/// A valid file with one trailing garbage line AFTER `@End`. This pins the
/// "out-of-slot ERROR" position the Task-1 review flagged: the migration's
/// positional walk visits `child_0..child_4` of `FullDocumentChildren`
/// (`@UTF8`, the pre-begin repeat, `@Begin`, the line repeat, `@End`), so the
/// concern was that an ERROR positioned AFTER `end_header` would escape the
/// document-level walk and reach only the whole-tree backstop, dropping the old
/// hand-walk's enrichment.
///
/// Empirically (verified with `tree-sitter parse`), the grammar does NOT emit a
/// direct `full_document`-child ERROR after `end_header`: the trailing garbage
/// is absorbed as an ERROR node INSIDE the `end_header` node (a descendant, so
/// `child_4` is `Present`). The OLD hand-walk only iterated DIRECT children of
/// `full_document`, so it never visited this descendant ERROR either; both the
/// old and new code surface it solely through the whole-tree
/// `collect_recovery_nodes` backstop. This fixture pins that the diagnostic set
/// is identical (a single E316 at the garbage span), confirming no enrichment
/// was lost for the trailing-after-`@End` case.
const TRAILING_AFTER_END: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
*CHI:\thello .
@End
GARBAGE TRAILING TEXT
";

/// Two `@End` lines. The FIRST `@End` is absorbed as an ERROR that IS a direct
/// `full_document` child, positioned in the line-repeat region (before the real
/// `end_header`), so the migration's `child_3` loop captures it as
/// `NodeSlot::Error` and routes it through the shared top-level error path
/// (`handle_top_level_error` -> `analyze_error_node`), exactly as the old
/// hand-walk did. This pins the captured-direct-child-ERROR enrichment path
/// (here it resolves to E501, "Duplicate @End"), the complement to the
/// backstop-only path that [`TRAILING_AFTER_END`] pins.
const DOUBLE_END: &str = "\
@UTF8
@Begin
@Languages:\teng
*CHI:\thello .
@End
@End
";

/// A stable tag for each `Header` variant, used to assert line structure without
/// depending on header payload internals.
fn header_tag(h: &Header) -> &'static str {
    match h {
        Header::Utf8 => "Utf8",
        Header::Begin => "Begin",
        Header::End => "End",
        Header::Languages { .. } => "Languages",
        Header::Participants { .. } => "Participants",
        Header::ID(_) => "ID",
        Header::Comment { .. } => "Comment",
        Header::Date { .. } => "Date",
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
fn parse_lines_and_diags(input: &str) -> (Vec<String>, Vec<Diag>) {
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

#[test]
fn valid_multiline_file_parses_to_expected_line_structure() {
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

#[test]
fn stray_top_level_error_line_emits_exact_recovery_diagnostics() {
    let (tags, diags) = parse_lines_and_diags(STRAY_TOP_LEVEL_DATE);

    // The entry point recovers the `@Date:` ERROR node into a `Header::Date`
    // line in file order, so the model still carries every header line.
    assert_eq!(
        tags,
        vec![
            "Header(Utf8)",
            "Header(Begin)",
            "Header(Languages)",
            "Header(Participants)",
            "Header(ID)",
            "Header(Date)",
            "Header(End)",
        ],
        "stray @Date: must be recovered into a Date header line in file order"
    );

    // EXACTLY two diagnostics, captured from the pre-migration parser:
    //   - E747 blank line at the trailing newline (span 93..94)
    //   - E316 unparsable content for the @Date: ERROR node (span 87..93),
    //     emitted once by the whole-tree backstop (NOT duplicated).
    let codes: Vec<&str> = diags.iter().map(|(c, _, _, _)| c.as_str()).collect();
    assert_eq!(
        codes,
        vec!["E747", "E316"],
        "stray @Date: must emit exactly E747 then E316, got: {diags:?}"
    );

    let e747 = diags
        .iter()
        .find(|(c, _, _, _)| c == "E747")
        .expect("E747 present");
    assert_eq!(
        (e747.1, e747.2),
        (93, 94),
        "E747 blank-line span must be (93..94)"
    );
    assert_eq!(e747.3, "Blank lines are not allowed");

    let e316: Vec<&(String, u32, u32, String)> =
        diags.iter().filter(|(c, _, _, _)| c == "E316").collect();
    assert_eq!(
        e316.len(),
        1,
        "the document-level ERROR must yield EXACTLY ONE E316 (no double-emission), got: {diags:?}"
    );
    let e316 = e316[0];
    assert_eq!(
        (e316.1, e316.2),
        (87, 93),
        "E316 must be at the exact @Date: ERROR node span (87..93)"
    );
    assert_eq!(
        e316.3, "Unparsable content: tree-sitter could not parse '@Date:'",
        "E316 message must match the backstop's exact wording"
    );
}

#[test]
fn trailing_garbage_after_end_emits_single_backstop_e316() {
    // Out-of-slot review case. The trailing garbage after `@End` is parsed as an
    // ERROR INSIDE the `end_header` node (a descendant of `child_4`, NOT a direct
    // `full_document` child after `end_header`; the grammar never emits the
    // latter). The positional document walk does not descend into it; the
    // whole-tree backstop surfaces it. Pinned diagnostics were captured by
    // RUNNING BOTH the pre-migration parser (worktree at c080c23) and the
    // migrated HEAD: they were byte-identical (single E316 at span 105..127),
    // so the migration preserved behavior for this case (no enrichment lost).
    let (tags, diags) = parse_lines_and_diags(TRAILING_AFTER_END);

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
        "valid lines before @End must still parse; trailing garbage adds no line"
    );

    assert_eq!(
        diags.len(),
        1,
        "trailing-after-@End garbage must yield exactly ONE diagnostic, got: {diags:?}"
    );
    let (code, start, end, _msg) = &diags[0];
    assert_eq!(
        (code.as_str(), *start, *end),
        ("E316", 105, 127),
        "the single diagnostic must be the backstop E316 at the garbage span (105..127)"
    );
}

#[test]
fn double_end_routes_captured_direct_child_error_to_e501() {
    // Complement to the backstop-only case: here the first `@End` becomes an
    // ERROR that IS a direct `full_document` child in the line-repeat region, so
    // the migration's `child_3` loop captures it as `NodeSlot::Error` and routes
    // it through `handle_top_level_error` -> `analyze_error_node`, yielding E501.
    // Pinned from the same OLD-vs-NEW comparison (byte-identical): E501 at the
    // first-@End span (43..48).
    let (tags, diags) = parse_lines_and_diags(DOUBLE_END);

    assert_eq!(
        tags,
        vec![
            "Header(Utf8)",
            "Header(Begin)",
            "Header(Languages)",
            "Utterance",
            "Header(End)",
        ],
        "the SECOND @End is the real end_header; the first becomes a captured ERROR"
    );

    assert_eq!(
        diags.len(),
        1,
        "double @End must yield exactly ONE diagnostic, got: {diags:?}"
    );
    let (code, start, end, _msg) = &diags[0];
    assert_eq!(
        (code.as_str(), *start, *end),
        ("E501", 43, 48),
        "the captured direct-child ERROR must route to E501 at the first @End span (43..48)"
    );
}
