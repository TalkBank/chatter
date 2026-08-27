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

//! Characterization tests for the @ID header parser's INTERNAL child-access
//! (Task 2j, Level-2 structured migration).
//!
//! `parse_id_header` lives in `tree_parsing/header/id/parse.rs` and is SHARED by
//! both the line path (`header_parser/dispatch/structured.rs`) and the
//! single-line `header_dispatch/parse.rs` API. Task 2j migrates the body off the
//! flat raw-cursor walk (`find_child_by_kind` + an `idx`-cursor over raw CST
//! children, plus the `fields.rs` / `helpers.rs` cursor helpers
//! `parse_required_text_field` / `parse_optional_text_field` /
//! `parse_optional_sex_field` / `parse_optional_terminal_field` /
//! `skip_whitespace` / `get_child_or_report`) onto the generated, typed,
//! positional `extract_id_header(node).child_2` -> `extract_id_contents(...)`
//! field slots (reached through the shared `HeaderTraversal` ZST seam in
//! `tree_parsing/header/typed.rs`). It is BEHAVIOUR-PRESERVING: the produced
//! `IDHeader` payloads and every diagnostic must stay byte-identical.
//!
//! These tests pin the OBSERVABLE behaviour at the real parser boundary
//! (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics) on
//! EXISTING fixtures (reference corpus + the @ID error corpus), NOT hand-authored
//! and NOT guessed. All asserted values were captured by RUNNING the
//! pre-migration parser at HEAD 909a56a. The tests PASS on the current code and
//! MUST STAY GREEN after the child-access migration.
//!
//! Coverage:
//! - FULL valid @ID with every optional field present (manchester-anne.cha).
//! - Valid @ID with ALL optional fields ABSENT -- the key preservation case
//!   (words-markers.cha).
//! - Empty (required-but-lenient) corpus -> empty `CorpusName` (E514 fixture).
//! - Unsupported sex / ses values preserved as `Unsupported(..)` (E542 / E546).
//! - Out-of-pattern age preserved as raw text (E517).
//! - Auto-generated all-optionals-absent @ID (E518).
//! - Malformed/truncated @ID that never forms an `id_header` node: emits E505 via
//!   the error-analysis pass and yields NO `Header::ID`/`Header::Unknown`
//!   (confirms `parse_id_header` is never reached, so the migration leaves the
//!   malformed dispatch path untouched).

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// Existing reference-corpus fixtures (NOT hand-authored).
const FULL: &str = include_str!("../../../../corpus/reference/languages/manchester-anne.cha");
const ABSENT: &str = include_str!("../../../../corpus/reference/content/words-markers.cha");

// Existing @ID error-corpus fixtures (NOT hand-authored).

/// The committed spec-generated fixture for one example of one error code.
///
/// `include_str!` needs a string LITERAL, so this cannot take an `ErrorCode`
/// the way `config_path_check_parity`'s helper does. What it can remove is the
/// repetition that actually broke: the corpus path was spelled out eight
/// times, and each fixture named its code twice, once in the const and once
/// inside a filename the generator owns. When the error specs were renamed to
/// the bare `E###.md` convention on 2026-08-26 the generator's fixture names
/// moved with them, and all eight stopped compiling at once.
///
/// The naming rule (`<code>_<example index>`, R4) is now written once, here,
/// instead of once per fixture.
macro_rules! spec_fixture {
    ($code:literal, $example:literal) => {
        include_str!(concat!(
            "../../../../crates/talkbank-parser-tests/tests/error_corpus/validation_errors/",
            $code,
            "_",
            $example,
            ".cha"
        ))
    };
}

const E505_1: &str = spec_fixture!("E505", 1);
const E505_2: &str = spec_fixture!("E505", 2);
const E505_3: &str = spec_fixture!("E505", 3);
const E514: &str = spec_fixture!("E514", 1);
const E542: &str = spec_fixture!("E542", 1);
const E546: &str = spec_fixture!("E546", 1);
const E518: &str = spec_fixture!("E518", 1);
const E517: &str = spec_fixture!("E517", 1);

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every `Header::ID` / `Header::Unknown` (in document order) plus every collected
/// diagnostic as `(code, message)`.
fn id_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .as_slice()
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. }
                if matches!(&**header, Header::ID(_) | Header::Unknown { .. }) =>
            {
                Some(format!("{header:?}"))
            }
            _ => None,
        })
        .collect();
    let diags = errors
        .into_vec()
        .into_iter()
        .map(|d| (d.code.as_str().to_string(), d.message))
        .collect();
    (headers, diags)
}

/// FULL valid @ID with every optional present (languages, corpus, speaker, age,
/// sex, group, ses, role) reproduces each field's pre-migration payload exactly,
/// with zero parse diagnostics.
#[test]
fn full_valid_id_preserves_all_fields() {
    let (headers, diags) = id_headers_and_diags(FULL);
    assert_eq!(
        headers,
        vec![
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 1, months: Some(10), days: Some(7), raw: "1;10.07" }), sex: Some(Female), group: Some(GroupName("TD")), ses: Some(SesOnly(MC)), role: ParticipantRole("Target_Child"), education: None, custom_field: None })"#.to_string(),
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("MOT"), age: None, sex: Some(Female), group: None, ses: None, role: ParticipantRole("Mother"), education: None, custom_field: None })"#.to_string(),
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("INV"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Investigator"), education: None, custom_field: None })"#.to_string(),
        ],
        "full @ID must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero parse diags: {diags:?}"
    );
}

/// Valid @ID with ALL optional fields ABSENT (corpus + speaker + role present;
/// age/sex/group/ses/education/custom omitted) keeps every optional model field
/// `None` and emits zero parse diagnostics. THE key preservation case: an absent
/// optional must NOT newly error.
#[test]
fn optionals_absent_preserves_unset_fields() {
    let (headers, diags) = id_headers_and_diags(ABSENT);
    assert_eq!(
        headers,
        vec![
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#.to_string(),
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("MOT"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Mother"), education: None, custom_field: None })"#.to_string(),
        ],
        "optionals-absent @ID must keep every optional None"
    );
    assert!(
        diags.is_empty(),
        "absent optionals must not newly error: {diags:?}"
    );
}

/// Empty (blank) corpus field parses to the constructor's empty `CorpusName("")`
/// (not an error and not `Unknown`); the empty corpus is what the Validate trait
/// later flags as E514. The parse-time path emits no diagnostic.
#[test]
fn empty_corpus_parses_to_empty_corpusname() {
    let (headers, diags) = id_headers_and_diags(E514);
    assert_eq!(
        headers,
        vec![
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName(""), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Target_Child"), education: None, custom_field: None })"#.to_string(),
        ],
        "empty corpus must parse to empty CorpusName, not Unknown"
    );
    assert!(diags.is_empty(), "no parse diag expected: {diags:?}");
}

/// An unsupported sex value is preserved verbatim as `Sex::Unsupported(..)` (the
/// validator later flags E542); the sex field stays TEXT-based (`Sex::from_text`).
#[test]
fn unsupported_sex_preserved_as_unsupported_variant() {
    let (headers, diags) = id_headers_and_diags(E542);
    assert_eq!(
        headers,
        vec![
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 3, months: Some(6), days: None, raw: "3;06." }), sex: Some(Unsupported("badsex")), group: None, ses: None, role: ParticipantRole("Target_Child"), education: None, custom_field: None })"#.to_string(),
        ],
        "unsupported sex must be preserved as Unsupported(\"badsex\")"
    );
    assert!(diags.is_empty(), "no parse diag expected: {diags:?}");
}

/// An unsupported ses value is preserved verbatim as `SesValue::Unsupported(..)`
/// (the validator later flags E546); the ses field stays TEXT-based
/// (`SesValue::from_text`).
#[test]
fn unsupported_ses_preserved_as_unsupported_variant() {
    let (headers, diags) = id_headers_and_diags(E546);
    assert_eq!(
        headers,
        vec![
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 3, months: Some(6), days: None, raw: "3;06." }), sex: Some(Female), group: None, ses: Some(Unsupported("badses")), role: ParticipantRole("Target_Child"), education: None, custom_field: None })"#.to_string(),
        ],
        "unsupported ses must be preserved as Unsupported(\"badses\")"
    );
    assert!(diags.is_empty(), "no parse diag expected: {diags:?}");
}

/// An out-of-pattern age is preserved with its raw text (`AgeValue::Valid` carries
/// `raw`); the validator later flags E517. The parse produces the model, not an
/// error.
#[test]
fn out_of_pattern_age_preserved_as_raw() {
    let (headers, diags) = id_headers_and_diags(E517);
    assert_eq!(
        headers,
        vec![
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 3, months: Some(0), days: None, raw: "3;0" }), sex: None, group: None, ses: None, role: ParticipantRole("Target_Child"), education: None, custom_field: None })"#.to_string(),
        ],
        "out-of-pattern age must be preserved with raw text"
    );
    assert!(diags.is_empty(), "no parse diag expected: {diags:?}");
}

/// An all-optionals-absent @ID (E518 fixture) parses to its exact payload; the
/// only diagnostic is the unrelated E501 (content after @End) elsewhere in the
/// fixture, captured here to pin the full streaming behaviour.
#[test]
fn auto_generated_id_preserves_fields() {
    let (headers, diags) = id_headers_and_diags(E518);
    assert_eq!(
        headers,
        vec![
            r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Target_Child"), education: None, custom_field: None })"#.to_string(),
        ],
        "auto-generated @ID must reproduce its payload"
    );
    assert_eq!(
        diags,
        vec![(
            "E501".to_string(),
            "Duplicate @End header or content after @End: nothing may follow the @End line"
                .to_string(),
        )],
        "only the unrelated E501 diagnostic is expected"
    );
}

/// Malformed / truncated @ID lines that cannot form a valid `id_header` CST node
/// are caught by the error-analysis pass (E505) and never reach
/// `parse_id_header`: NO `Header::ID` or `Header::Unknown` is produced, and the
/// E505 diagnostic is emitted. This confirms the migration leaves the malformed
/// dispatch path untouched (the diagnostics below are byte-identical pre/post).
#[test]
fn malformed_truncated_id_emits_e505_no_header() {
    let expected_diags = vec![
        (
            "E505".to_string(),
            "Invalid @ID header format: structure could not be parsed".to_string(),
        ),
        (
            "E522".to_string(),
            "Speaker 'CHI' declared in @Participants but has no matching @ID header".to_string(),
        ),
    ];
    for (name, src) in [("E505_1", E505_1), ("E505_2", E505_2), ("E505_3", E505_3)] {
        let (headers, diags) = id_headers_and_diags(src);
        assert!(
            headers.is_empty(),
            "{name}: malformed @ID must not yield an ID/Unknown header, got {headers:?}"
        );
        assert_eq!(
            diags, expected_diags,
            "{name}: malformed @ID must emit the same E505 diagnostics as before"
        );
    }
}
