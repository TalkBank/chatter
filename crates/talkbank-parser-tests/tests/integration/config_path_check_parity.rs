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

//! Pins the five checks that `ChatFile::validate_with_config` was missing
//! relative to `ChatFile::validate` until they were unified into one shared
//! check sequence (`run_validation_checks`): E544, E552, E752, E755, and the
//! `check_cross_header_consistency` family (E519/E532 in their cross-header
//! role).
//!
//! Before that unification, `talkbank-transform`'s worker only reached
//! `validate_with_config` when `--strict-linkers` was passed, so the gap was
//! invisible in ordinary use. `validate_with_config` (via
//! `validate_with_alignment_and_config`) is now the ALWAYS-taken path for
//! every `chatter validate` run (suppression joins the rule set upstream of
//! validation), so if any of these five silently stopped firing under that
//! path, nothing else would catch it: `validation_error_corpus.rs` only
//! exercises `validate_with_alignment`, never the config path.
//!
//! `E544`/`E552`/`E752`/`E755` reuse the exact fixtures the spec-generated
//! corpus commits (real spec-authored CHAT, preferred over hand-written),
//! named `<spec stem>_<example index>` since R4 made fixture identity derive
//! from the example rather than from iteration order.
//! `check_cross_header_consistency`'s two rules
//! (CHECK 122: `@ID` language not in `@Languages`; CHECK 142: `@ID` role
//! disagrees with `@Participants`) have no existing fixture anywhere in the
//! corpus (grep confirms no spec references either CHECK number), so those
//! two get minimal hand-written fixtures, isolated so each triggers through
//! ONLY the cross-header call site and not any other E519/E532 call site
//! (both codes have other, unrelated origins elsewhere in the validator).

use std::path::{Path, PathBuf};
use talkbank_model::model::FileStem;
use talkbank_model::model::TranscriptName;

use talkbank_model::{ErrorCode, ErrorCollector, ParseOutcome, RuleSelection};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;
use talkbank_spec_vocabulary::observations::ExampleId;

/// The validation corpus dir this crate's fixtures live under (shared with
/// `validation_error_corpus.rs`).
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/error_corpus/validation_errors")
}

/// Parse `content` and run `ChatFile::validate_with_config` (the path that
/// was missing these checks), returning every parse + validation code
/// produced. `rules` is a plain default `RuleSelection::new()` in every
/// caller below: the point is that the CONFIG-PATH FUNCTION ITSELF must
/// still run the full check sequence, independent of what the config
/// actually overrides.
fn codes_through_config_path(
    content: &str,
    name: TranscriptName<'_>,
) -> Result<Vec<String>, TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let parse_errors = ErrorCollector::new();
    let parse_result = parser.parse_chat_file_fragment(content, 0, &parse_errors);
    let mut codes: Vec<String> = parse_errors
        .to_vec()
        .iter()
        .map(|e| e.code.to_string())
        .collect();

    let ParseOutcome::Parsed(chat_file) = parse_result else {
        return Err(TestError::Failure(format!(
            "fixture did not parse; codes so far: {codes:?}"
        )));
    };

    let validation_errors = ErrorCollector::new();
    chat_file.validate_with_rules(RuleSelection::new(), &validation_errors, name);
    codes.extend(
        validation_errors
            .to_vec()
            .iter()
            .map(|e| e.code.to_string()),
    );
    Ok(codes)
}

/// Assert that `code` fires through the config path, using the committed
/// fixture the spec for that code generated as its `example`th (0-based)
/// example.
///
/// # Why this takes an `ErrorCode` rather than two strings
///
/// It used to take a hand-typed fixture stem beside a hand-typed expected
/// code: `("E544_media_linkage_without_timing_1", "E544")`. That states the
/// code twice and mirrors a filename the generator owns, and both mirrors
/// broke at once on 2026-08-26 when the error specs were renamed to the bare
/// `E###.md` convention and the generator's fixture names moved with them.
/// Four tests went red over a rename, in a file whose subject (does the config
/// path run the whole check sequence?) had not changed at all.
///
/// # Where the naming rule actually lives, which is NOT here
///
/// `<stem>_<1-based position>.cha` is owned by
/// [`ExampleId::fixture_name`](talkbank_spec_vocabulary::observations::ExampleId::fixture_name),
/// whose own docstring records that three generators and the snapshot key each
/// derived it independently before it had an owner. This asks that type rather
/// than becoming the fourth, so the 1-based convention in particular is
/// applied in exactly one place.
///
/// What this function DOES decide is the spec file's name, and it assumes the
/// bare `E###.md` convention. That holds for every code with a single spec
/// file, which is all four used here; a code claimed by more than one spec
/// cannot be addressed this way at all, and would need the fixture looked up
/// through `manifest.json` instead.
fn assert_fixture_code_fires_through_config_path(
    code: ErrorCode,
    example: usize,
) -> Result<(), TestError> {
    let spec_file = format!("{}.md", code.as_str());
    let fixture = ExampleId::from_enumerate(&spec_file, example).fixture_name();
    let path = corpus_dir().join(&fixture);
    let content = std::fs::read_to_string(&path)
        .map_err(|err| TestError::Failure(format!("failed to read {}: {err}", path.display())))?;
    let stem = Path::new(&fixture).with_extension("");
    let codes = codes_through_config_path(&content, TranscriptName::for_path(&stem))?;
    assert!(
        codes.iter().any(|c| c == code.as_str()),
        "{fixture}: expected {} through validate_with_config, got {codes:?}",
        code.as_str()
    );
    Ok(())
}

#[test]
fn e544_media_linkage_without_timing_fires_through_config_path() -> Result<(), TestError> {
    assert_fixture_code_fires_through_config_path(ErrorCode::MediaLinkageWithoutTiming, 0)
}

#[test]
fn e552_media_unlinked_with_timing_fires_through_config_path() -> Result<(), TestError> {
    assert_fixture_code_fires_through_config_path(ErrorCode::MediaUnlinkedWithTiming, 0)
}

#[test]
fn e752_timing_without_media_fires_through_config_path() -> Result<(), TestError> {
    assert_fixture_code_fires_through_config_path(ErrorCode::TimingWithoutMedia, 0)
}

#[test]
fn e755_undeclared_utterance_language_fires_through_config_path() -> Result<(), TestError> {
    assert_fixture_code_fires_through_config_path(ErrorCode::UndeclaredUtteranceLanguage, 0)
}

/// CHECK 122: `@ID`'s language field names a real ISO 639-3 code (`fra`,
/// French) that is simply absent from `@Languages` (which declares only
/// `eng`). A REAL (registry-valid) code is deliberately used so this fires
/// ONLY `check_cross_header_consistency`'s membership check, not the
/// unrelated ISO-registry-lookup E519 call site in
/// `model/header/codes/language.rs` (which fires on codes that are not
/// valid ISO 639-3 at all, e.g. the committed `E519_*_not_in_the_ISO_639_3_registry`
/// fixtures).
#[test]
fn cross_header_id_language_not_in_languages_fires_through_config_path() -> Result<(), TestError> {
    let content = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\tfra|corpus|CHI|||||Target_Child|||
*CHI:\tbonjour .
@End
";
    let codes = codes_through_config_path(
        content,
        TranscriptName::Named(FileStem::from_stem("cross_header_language")),
    )?;
    assert!(
        codes.iter().any(|c| c == "E519"),
        "expected E519 (CHECK 122, @ID language not in @Languages) through \
         validate_with_config, got {codes:?}"
    );
    Ok(())
}

/// CHECK 142: `@ID`'s role field (`Mother`) disagrees with the role
/// `@Participants` declared for the same speaker code (`Target_Child`).
/// Both role values are individually valid `ParticipantRole` variants (the
/// committed `E532_Invalid_participant_role.cha` fixture exercises the OTHER
/// E532 call site, an actually-invalid role string), so this fixture
/// isolates the cross-header mismatch specifically.
#[test]
fn cross_header_id_role_disagrees_with_participants_fires_through_config_path()
-> Result<(), TestError> {
    let content = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, MOT Mother
@ID:\teng|corpus|CHI|||||Mother|||
@ID:\teng|corpus|MOT|||||Mother|||
*CHI:\thello .
*MOT:\thi .
@End
";
    let codes = codes_through_config_path(
        content,
        TranscriptName::Named(FileStem::from_stem("cross_header_role")),
    )?;
    assert!(
        codes.iter().any(|c| c == "E532"),
        "expected E532 (CHECK 142, @ID role disagrees with @Participants) through \
         validate_with_config, got {codes:?}"
    );
    Ok(())
}
