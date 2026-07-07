//! Regression test for the empty-replacement diagnostic, after removing the
//! dead `[:]` ERROR-text branches.
//!
//! Bug history: an empty replacement (e.g. `word [:]`) was historically also
//! classified by scanning the raw text of an ERROR node for the literal `[:]`,
//! in THREE places: `analyze_word_error` (content/errors.rs), the utterance
//! ERROR branch in `utterance_parser.rs`, and `analyze_utterance_error`
//! (error_analysis/utterance.rs). That ERROR-text classification is the banned
//! anti-pattern (root CLAUDE.md "CST Traversal Rules").
//!
//! Reality (verified by tree-sitter CST observation, 2026-06-25): `word [:]`
//! does NOT become an ERROR node at all. It PARSES into a structured
//! `replacement` node whose body is a zero-width `standalone_word` with a
//! MISSING `word_segment`. The replacement-parsing path (typed model) emits
//! E376 (`ContentAnnotationParseError`), and the MISSING recovery slot emits
//! E342 (`MissingRequiredElement`). No content-level ERROR node ever carries
//! `[:]` text (a bare `[:]` or `<group> [:]` instead becomes a FILE-level ERROR
//! routed to `analyze_error_node`, which has no `[:]` branch and yields the
//! generic E316). The three `[:]` text-scan branches were therefore DEAD, and
//! were removed. This test pins that `word [:]` still flags structurally and
//! does NOT regress to generic E316.

mod common;

/// An empty replacement `word [:]` is detected by the structural replacement
/// path (E376 `ContentAnnotationParseError`, plus the MISSING-word E342), NOT by
/// any `[:]` ERROR-text scan, and must NOT regress to a generic E316.
#[test]
fn empty_replacement_detected_structurally_not_e316() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tword [:] .\n@End\n";

    let diags = common::parse_validate_and_collect_diagnostics(input, Some("e208_regression"));
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        codes.contains(&"E376"),
        "Expected E376 (content-annotation parse error) for the empty replacement `word [:]`, got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "`word [:]` must not regress to generic E316 (unparsable content); got: {diags:#?}",
    );
}

/// A well-formed replacement `word [: corrected]` must NOT be flagged (no false
/// positive) and must not produce E316.
#[test]
fn well_formed_replacement_not_flagged() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tword [: corrected] .\n@End\n";

    let diags = common::parse_validate_and_collect_diagnostics(input, Some("e208_regression"));
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        !codes.contains(&"E376"),
        "Well-formed replacement `word [: corrected]` must NOT be flagged E376; got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "Well-formed replacement `word [: corrected]` must not produce E316; got: {diags:#?}",
    );
}
