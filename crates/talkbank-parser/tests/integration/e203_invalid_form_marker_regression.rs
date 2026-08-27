// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
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
//! own text and hands it to `FormType::from_payload`, which owns the question
//! of which markers exist; `zz` is not one of them, so E203 (`InvalidFormType`)
//! is emitted. Reading a parsed node's own content for validation is
//! typed-model work, NOT raw-CHAT / ERROR-text scanning.
//!
//! This comment used to enumerate the valid set, and by 2026-08-11 it was the
//! last place in the repository still advertising `@a`, one commit after `@a`
//! was retired. The set has one owner now,
//! `spec/form_markers/form_marker_registry.json`; a copy here could only ever
//! go stale again, because nothing reads a comment.
//! The redundant ERROR-text branches are removed; this test pins that the typed
//! path still flags `@zz`, does NOT regress to generic E316, and does NOT
//! false-positive on valid markers (`@i`, `@s:eng`).

use talkbank_model::ErrorCollector;
use talkbank_model::model::FileStem;
use talkbank_model::model::TranscriptName;
use talkbank_model::model::WriteChat;
use talkbank_parser::TreeSitterParser;

/// The four shapes the at-most-one-`@`-suffix rule refuses, together.
///
/// A form marker then a language suffix (the case the 2026-08-27 ruling
/// actually decided), a second run that is not a language suffix, a doubled
/// sigil, and two form markers. One list, so a shape added for the round-trip
/// property is automatically checked for the refusal property too; they were
/// two identical literals until a review pointed out they could drift.
const TWO_AT_SUFFIX_WORDS: [&str; 4] = ["bebe@k@s:spa", "bebe@k@st", "hello@@c", "hello@c@d"];

/// A minimal document carrying `body` as its one main tier.
///
/// The header block was written out at six call sites in this file, one of
/// them as an unwrapped 200-column literal, differing only in the `*CHI:` line.
fn document(body: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\tspa, eng\n@Participants:\tCHI Target_Child\n\
         @ID:\tspa|corpus|CHI|||||Target_Child|||\n*CHI:\t{body}\n@End\n"
    )
}

/// An unknown form marker `word@zz` must be flagged E203 via the typed
/// `form_marker` dispatch, and must NOT regress to a generic E316.
#[test]
fn unknown_form_marker_emits_e203_not_e316() {
    let input = document("word@zz .");

    let diags = crate::common::parse_validate_and_collect_diagnostics(
        &input,
        TranscriptName::Named(FileStem::from_stem("e203_regression")),
    );
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
    let input = document("hello@i .");

    let diags = crate::common::parse_validate_and_collect_diagnostics(
        &input,
        TranscriptName::Named(FileStem::from_stem("e203_regression")),
    );
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
    let input = document("hello@s:eng .");

    let diags = crate::common::parse_validate_and_collect_diagnostics(
        &input,
        TranscriptName::Named(FileStem::from_stem("e203_regression")),
    );
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

/// An undeclared marker survives a parse-and-serialize round trip byte for
/// byte.
///
/// # Why this exists
///
/// The recovery path used to store `FormType::UserDefined(payload)` for
/// `word@zz`, which asserts that the word carries the `@z` user-defined marker
/// with label `zz`. `Word::write_chat` rebuilds the marker from `form_type`
/// rather than from the raw text, so it wrote `word@z:zz`: a silent
/// corruption, unobservable only because every command that serializes aborts
/// on E203 first. That is a latent trap, not a safe state, and the re2c parser
/// stored nothing at all for the same input, so the two parsers disagreed.
///
/// A ROUNDTRIP test rather than an assertion about which variant is stored,
/// deliberately: this pins the property that matters (the transcript comes
/// back unchanged), which no type signature can state, and it still holds if
/// the recovery representation changes again.
#[test]
fn an_undeclared_form_marker_round_trips_unchanged() {
    let input = document("word@zz .");

    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let chat_file = parser.parse_chat_file_streaming(&input, &errors);

    let mut serialized = String::new();
    chat_file.write_chat(&mut serialized).expect("serialize");

    assert_eq!(
        serialized, input,
        "an undeclared form marker must serialize back verbatim; storing it as \
         a DECLARED marker rewrote `word@zz` as `word@z:zz`"
    );
}

/// A word carrying TWO `@` suffixes round-trips byte for byte.
///
/// # The rule
///
/// A word may carry at most ONE `@` suffix. Ruled by the maintainer on
/// 2026-08-27, asked directly because CLAN CHECK accepts these and chatter did
/// not: "Multiple suffixes might make logical sense, but it is computationally
/// messy. So, let's disallow that." So `word@k@s:spa` is invalid even though
/// `word@k` and `word@s:spa` are each fine.
///
/// # Why a ROUND-TRIP test for an invalid construct
///
/// Because being invalid is not a licence to rewrite it. On 2026-08-27 the
/// grammar gained a `repeated_form_marker` node with no lowering arm, so its
/// text reached no `WordContent`, `form_type` stayed `None`, and serialization
/// rebuilt the word without it: `chatter normalize` wrote `the bebe here .`
/// for `the bebe@k@s:spa here .` and EXITED 0. Silent data loss on a write
/// path reporting success.
///
/// This pins the OUTCOME rather than the mechanism, which is what a round-trip
/// test is for: it still holds if the storage representation changes again.
/// The shape is attested only in `%com` and `%exp` free text
/// (`action@man@s:eng`, Serbian SCECL Milos 031003); main-tier words carrying
/// two `@` runs number ZERO across ~106,000 corpus files.
#[test]
fn a_word_with_two_at_suffixes_round_trips_unchanged() {
    for word in TWO_AT_SUFFIX_WORDS {
        let input = document(&format!("the {word} here ."));

        let parser = TreeSitterParser::new().expect("parser");
        let errors = ErrorCollector::new();
        let chat_file = parser.parse_chat_file_streaming(&input, &errors);

        let mut serialized = String::new();
        chat_file.write_chat(&mut serialized).expect("serialize");
        assert_eq!(
            serialized, input,
            "`{word}` must serialize back verbatim; with no lowering arm its \
             suffix was dropped and `normalize` wrote a shorter word, exit 0"
        );
    }
}

/// The same words are REFUSED, exactly once, by a diagnostic that names the
/// actual defect.
///
/// Three things are pinned here and each was wrong at some point on
/// 2026-08-27:
///
/// 1. **E203, not a generic E316.** Before the grammar formed these words the
///    utterance fell to error recovery and said only "content could not be
///    parsed".
/// 2. **Exactly ONCE.** The parser names the run and the model's `at_count > 1`
///    branch names the same word, so E203 arrived TWICE. `contains` cannot see
///    that, which is why this counts.
/// 3. **The message names the RUN.** `@c` in `@c@s:spa` is a real marker, so
///    "Undeclared form marker '@c@s:spa'" would send a transcriber hunting for
///    a marker that does not exist rather than deleting one of the two that do.
///
/// SURVIVES as a test rather than a type: it pins a POLICY, the maintainer's
/// ruling, which has a real alternative (CLAN CHECK still accepts these).
#[test]
fn a_word_with_two_at_suffixes_is_refused_once_and_named() {
    for word in TWO_AT_SUFFIX_WORDS {
        let input = document(&format!("the {word} here ."));

        let diags = crate::common::parse_validate_and_collect_diagnostics(
            &input,
            TranscriptName::Named(FileStem::from_stem("e203_regression")),
        );
        let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

        let e203s = codes.iter().filter(|code| **code == "E203").count();
        assert_eq!(
            e203s, 1,
            "`{word}` must be refused E203 exactly ONCE, got {e203s}: {diags:#?}",
        );
        assert!(
            !codes.contains(&"E316"),
            "`{word}` must not degrade to generic E316: {diags:#?}",
        );
        assert!(
            diags
                .iter()
                .any(|(_, message)| message.contains("only one '@' suffix")),
            "the diagnostic for `{word}` must name the multiple-suffix rule \
             rather than calling the whole run an undeclared marker: {diags:#?}",
        );
    }
}
