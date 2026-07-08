// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Regression test for E203 invalid-form-marker, re-homed off ERROR-text scanning.
//!
//! Bug history: an unknown form marker (e.g. `word@zz`) was historically also
//! classified by scanning the raw text of an ERROR node, via
//! `find_invalid_form_marker_offset` in `analyze_word_error` (and a sibling `@`
//! text-scan in `analyze_utterance_error`). That ERROR-text classification is the
//! banned anti-pattern.
//!
//! Re-home: `word@zz` PARSES into a structured word with a `form_marker` child
//! (`@zz`). The parser's typed dispatch reads that parsed `form_marker` node's
//! own text and, because the base marker `zz` is not in the valid set
//! (a,b,c,d,f,fp,g,i,k,l,ls,n,o,p,q,sas,si,sl,t,u,wp,x,z, and `@z:label` for
//! user-defined), emits E203 (`InvalidFormType`). Reading a parsed node's own
//! content for validation is typed-model work, NOT raw-CHAT / ERROR-text scanning.
//! The redundant ERROR-text branches are removed; this test pins that the typed
//! path still flags `@zz`, does NOT regress to generic E316, and does NOT
//! false-positive on valid markers (`@i`, `@s:eng`).

mod common;

/// An unknown form marker `word@zz` must be flagged E203 via the typed
/// `form_marker` dispatch, and must NOT regress to a generic E316.
#[test]
fn unknown_form_marker_emits_e203_not_e316() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tword@zz .\n@End\n";

    let diags = common::parse_validate_and_collect_diagnostics(input, Some("e203_regression"));
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        codes.contains(&"E203"),
        "Expected E203 (invalid form type) for `word@zz`, got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "`word@zz` must not regress to generic E316 (unparsable content); got: {diags:#?}",
    );
}

/// A valid built-in form marker `hello@i` (interjection) must NOT be flagged
/// E203 (no false positive) and must not produce E316.
#[test]
fn valid_builtin_form_marker_not_flagged() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello@i .\n@End\n";

    let diags = common::parse_validate_and_collect_diagnostics(input, Some("e203_regression"));
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        !codes.contains(&"E203"),
        "Valid marker `hello@i` must NOT be flagged E203; got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "Valid marker `hello@i` must not produce E316; got: {diags:#?}",
    );
}

/// A valid language suffix `hello@s:eng` must NOT be flagged E203 (its base `s`
/// is a language tag, not a form marker) and must not produce E316.
#[test]
fn valid_language_suffix_not_flagged() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello@s:eng .\n@End\n";

    let diags = common::parse_validate_and_collect_diagnostics(input, Some("e203_regression"));
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        !codes.contains(&"E203"),
        "Valid language suffix `hello@s:eng` must NOT be flagged E203; got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "Valid language suffix `hello@s:eng` must not produce E316; got: {diags:#?}",
    );
}
