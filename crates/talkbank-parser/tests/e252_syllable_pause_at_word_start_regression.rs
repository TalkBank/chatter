//! Regression test for E252 syllable-pause-at-word-start, re-homed off text scanning.
//!
//! Bug history: a syllable pause `^` at the START of a word (e.g. `^banana`) made
//! the whole main tier fail to parse into a structured word. The grammar's
//! `word_body` rejected a leading `syllable_pause`, so the tier became a single
//! line-level `ERROR` node and the spoken text was dropped. The E252 diagnostic
//! (`SyllablePauseNotBetweenSpokenMaterial`) was produced by scanning the raw
//! ERROR text in `analyze_word_error` (`error_text.starts_with('^')`), which is
//! the banned ERROR-text-classification anti-pattern.
//!
//! Expected behavior (the re-home): `^banana` PARSES into a structured word whose
//! `word_body` begins with a `syllable_pause` child, and the typed-model
//! validation (`check_prosodic_markers`, reading `WordContent::SyllablePause`)
//! emits E252 because the pause has no preceding spoken material. No raw-text or
//! ERROR-text scan is involved. The word must NOT become a generic E316.

mod common;

/// A leading syllable pause `^banana` must be flagged with E252 via typed-model
/// validation, and must NOT regress to a generic unparsable-content E316.
///
/// This exercises the full parse-then-validate boundary: the parser must produce
/// a structured word (no ERROR / no E316), and the typed-model validator
/// (`check_prosodic_markers`, reading `WordContent::SyllablePause`) must emit E252
/// because the pause precedes all spoken material.
#[test]
fn leading_syllable_pause_emits_e252_not_e316() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t^banana .\n@End\n";

    let diags = common::parse_validate_and_collect_diagnostics(input, Some("e252_regression"));
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        codes.contains(&"E252"),
        "Expected E252 (syllable pause not between spoken material) for `^banana`, got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "`^banana` must not regress to generic E316 (unparsable content); got: {diags:#?}",
    );
}
