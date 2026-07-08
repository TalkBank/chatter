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

//! Characterization tests for the `main_tier` structure/prefix/tier-body
//! conversion as it is migrated onto the NEW-backend `extract_main_tier` /
//! `extract_tier_body` free functions (chatter visitor-migration Task B3a).
//!
//! These tests pin the OBSERVABLE behavior of `convert_main_tier_node`
//! (`tree_parsing/main_tier/structure/convert/{mod,prefix,body}.rs`) at the
//! real parser boundary (`parse_chat_file_streaming` -> `ChatFile` +
//! collected diagnostics). The migration is behavior-preserving: the parsed
//! speaker, language code, linkers, and diagnostics must not change for any
//! reachable input.
//!
//! Every expectation below was captured by RUNNING the pre-migration
//! (OLD-API) parser, not guessed. Two malformed-prefix candidates were probed
//! empirically before writing this file (`tree-sitter parse` on the raw
//! grammar plus a temporary Rust probe, since removed) and found
//! UNREACHABLE through realistic recovery, so they are not pinned here (see
//! the B3 report in the migration ledger for the empirical trace):
//!
//! - A missing colon after the speaker (`*CHI\thello .`) never reaches
//!   `parse_prefix`: tree-sitter recovers the WHOLE main-tier line as a
//!   document-level `ERROR`, routed by the (already-migrated) Task-B1
//!   top-level-error path, not by `prefix.rs`.
//! - An invalid language-code payload (`[- xx99]`) still parses as a
//!   structurally `Present` `langcode` node (tree-sitter isolates the
//!   trailing garbage as a NESTED `ERROR` inside the langcode subtree, not at
//!   the `tier_body` position), and `LanguageCode::new` accepts any
//!   non-empty string, so `parse_optional_langcode`'s "Malformed language
//!   code" arm never fires; the nested ERROR is instead picked up by the
//!   whole-tree recovery backstop as E316. `body.rs`'s existing doc comment
//!   already documents the sibling "Missing terminator in tier_body" arm as
//!   unreachable on valid input for the same reason (the `ending` slot always
//!   recovers Present/MISSING); this migration keeps both arms as exhaustive
//!   defensive code, not as a covered path.
//!
//! The one REACHABLE prefix-level recovery diagnostic (`MissingSpeaker` on a
//! zero-width `*:` speaker) is ALREADY pinned by
//! `test_parse_health_recovery.rs::missing_speaker_does_not_create_empty_speaker_utterance`
//! (R5: reused, not duplicated here).

use talkbank_model::model::Linker;

mod common;
use common::parse_utterances_and_diags;

fn read_fixture(relative: &str) -> String {
    let path = format!(
        "{}/../../corpus/reference/{}",
        env!("CARGO_MANIFEST_DIR"),
        relative
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn valid_langcode_switch_attaches_language_code_with_zero_diagnostics() {
    let input = read_fixture("content/language-switching.cha");
    let (utterances, diags) = parse_utterances_and_diags(&input);

    assert!(
        diags.is_empty(),
        "valid language-switching file must produce zero diagnostics, got: {diags:?}"
    );

    // First utterance is `*CHI:\t[- zho] 我 要 吃 .`: the `[- zho]` inline
    // language-code switch attaches to `main.content.language_code`.
    let first = &utterances[0];
    assert_eq!(first.main.speaker.as_str(), "CHI");
    assert_eq!(
        first
            .main
            .content
            .language_code
            .as_ref()
            .map(|lc| lc.as_str()),
        Some("zho"),
        "the [- zho] language-code switch must attach to the main tier"
    );
}

#[test]
fn valid_single_linkers_decode_to_expected_variants() {
    let input = read_fixture("content/linkers.cha");
    let (utterances, diags) = parse_utterances_and_diags(&input);

    assert!(
        diags.is_empty(),
        "valid linkers file must produce zero diagnostics, got: {diags:?}"
    );

    // Indexed by DOCUMENT ORDER (not text search: "and then she +..." at
    // index 3 has no linker at all and would falsely match a substring
    // search for "and" against index 8's "+< and then what ."). Covers
    // self-completion (+,), other-completion (++), quick uptake overlap
    // (+^), and lazy overlap (+<) in the fixture's literal line order:
    //   0 *CHI: so after the tower +/.
    //   1 *EXP: yeah .
    //   2 *CHI: +, I go straight ahead .
    //   3 *CHI: and then she +...
    //   4 *MOT: ++ went to the store .
    //   5 *CHI: the dog ran .
    //   6 *MOT: +^ yes he did .
    //   7 *CHI: so I went home .
    //   8 *MOT: +< and then what .
    let expectations: &[(usize, Linker)] = &[
        (2, Linker::SelfCompletion),
        (4, Linker::OtherCompletion),
        (6, Linker::QuickUptakeOverlap),
        (8, Linker::LazyOverlapPrecedes),
    ];

    for (index, expected_linker) in expectations {
        let utt = utterances
            .get(*index)
            .unwrap_or_else(|| panic!("utterance index {index} present"));
        assert_eq!(
            utt.main.content.linkers.as_slice(),
            std::slice::from_ref(expected_linker),
            "utterance at index {index} must decode to exactly one {expected_linker:?} linker"
        );
    }
}

#[test]
fn valid_multiple_linkers_on_one_utterance_preserve_document_order() {
    let input = read_fixture("content/linkers-multiple.cha");
    let (utterances, diags) = parse_utterances_and_diags(&input);

    assert!(
        diags.is_empty(),
        "valid multi-linker file must produce zero diagnostics, got: {diags:?}"
    );

    // `*CHI:\t+< +, the dog ran away .`: two linkers in document order.
    let chi = utterances
        .iter()
        .find(|u| u.main.speaker.as_str() == "CHI")
        .expect("CHI utterance present");
    assert_eq!(
        chi.main.content.linkers.as_slice(),
        &[Linker::LazyOverlapPrecedes, Linker::SelfCompletion],
        "two linkers on one utterance must decode in document order"
    );

    // `*SIS:\t+, ++ in the park !`
    let sis = utterances
        .iter()
        .find(|u| u.main.speaker.as_str() == "SIS")
        .expect("SIS utterance present");
    assert_eq!(
        sis.main.content.linkers.as_slice(),
        &[Linker::SelfCompletion, Linker::OtherCompletion],
        "two linkers on one utterance must decode in document order"
    );
}
