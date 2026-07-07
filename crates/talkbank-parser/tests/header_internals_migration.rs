//! Characterization tests for the header-internals layer (Task B2b of the
//! visitor-driven parser migration): `tree_parsing/header/{id/parse,
//! participants,metadata/{media,languages,situation,types}}.rs`.
//!
//! These tests pin the OBSERVABLE behavior of each header's LEVEL-2 field
//! decoding at the real parser boundary (`parse_chat_file_streaming` ->
//! `ChatFile` + collected diagnostics). The migration replaces
//! `TypedTraversal.extract_<kind>_header` / `extract_id_contents` (the OLD
//! `generated_traversal` trait receiver) with the NEW backend's free
//! `extract_<kind>_header` / `extract_id_contents` functions -- which, because
//! the NEW backend does not skip whitespace, model several EXTRA structural
//! positions (interstitial `whitespaces` nodes) that shift the `child_N`
//! field indices for `@ID`/`@Media`/`@Types`. The migration is
//! behavior-preserving: the model and the recovery diagnostics must not
//! change.
//!
//! The VALID case is covered by `headers-speaker-info.cha` (multi-language
//! `@ID` codes, several optional `@ID` fields genuinely absent), read
//! verbatim from `corpus/reference/core/` like the B2a dispatch fixtures.
//! Every MALFORMED fixture below is a minimal hand-written snippet (not a new
//! `.cha` file). Every diagnostic and model value pinned here was captured by
//! RUNNING the pre-migration (OLD-API) parser (`cargo nextest run -p
//! talkbank-parser --test header_internals_migration --no-capture
//! --no-fail-fast`), not guessed. Several of the malformed fixtures turned
//! out empirically to make the WHOLE header fail tree-sitter's structural
//! parse (becoming a document-level ERROR handled by the already-migrated
//! Task B1 entry point, not a clean header node with an internally-Missing
//! field), rather than exercising the specific `required_field`/"missing
//! contents" diagnostic arms inside these files; that is itself a pinned,
//! real fact about current behavior worth locking down, not a test-design
//! defect, and `participants_missing_role` DOES reach the intended internal
//! (`parse_participant_entry`'s "role cannot be empty" rejection).

use std::path::{Path, PathBuf};

use talkbank_model::ErrorCollector;
use talkbank_model::model::Line;
use talkbank_parser::TreeSitterParser;

/// One diagnostic: (code, span start, span end, message).
type Diag = (String, u32, u32, String);

/// Read a fixture verbatim from `corpus/reference/<relative>`.
fn read_corpus_fixture(relative: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/reference")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read corpus fixture {}: {}", path.display(), e))
}

/// Render one line as a stable structural string: the full `{:?}` of the
/// `Header` payload for a header line, or `"Utterance"` for an utterance line.
fn line_repr(line: &Line) -> String {
    match line {
        Line::Header { header, .. } => format!("{header:?}"),
        Line::Utterance(_) => "Utterance".to_string(),
    }
}

fn line_reprs(lines: &[Line]) -> Vec<String> {
    lines.iter().map(line_repr).collect()
}

/// Parse `input` at the real streaming boundary and return the line
/// representations plus every collected diagnostic as `(code, start, end,
/// message)` tuples.
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
    (line_reprs(&chat.lines.0), diags)
}

#[test]
fn headers_speaker_info_fixture_parses_with_zero_diagnostics() {
    let input = read_corpus_fixture("core/headers-speaker-info.cha");
    let (reprs, diags) = parse_lines_and_diags(&input);

    assert!(
        diags.is_empty(),
        "headers-speaker-info.cha must produce zero diagnostics, got: {diags:?}"
    );

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng"), LanguageCode("ara")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }, ParticipantEntry { speaker_code: SpeakerCode("MOT"), name: None, role: ParticipantRole("Mother") }, ParticipantEntry { speaker_code: SpeakerCode("F_A_T"), name: None, role: ParticipantRole("Father") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng"), LanguageCode("ara")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 1, months: Some(8), days: Some(2), raw: "1;08.02" }), sex: Some(Female), group: Some(GroupName("normal")), ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng"), LanguageCode("ara")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("MOT"), age: None, sex: Some(Female), group: None, ses: None, role: ParticipantRole("Mother"), education: None, custom_field: None })"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng"), LanguageCode("ara")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("F_A_T"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Father"), education: None, custom_field: None })"#,
        r#"Birth { participant: SpeakerCode("CHI"), date: Valid { day: 28, month: Jun, year: 2001, raw: "28-JUN-2001" } }"#,
        r#"Birth { participant: SpeakerCode("MOT"), date: Valid { day: 15, month: Mar, year: 1975, raw: "15-MAR-1975" } }"#,
        r#"Birthplace { participant: SpeakerCode("MOT"), place: BirthplaceDescription("Taipei, Taiwan") }"#,
        r#"L1Of { participant: SpeakerCode("F_A_T"), language: LanguageName("ara") }"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Speaker info headers: @Birth of, @Birthplace of, @L1 of" })]) } }"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: birth_of_header, birthplace_of_header, l1_of_header," }), Continuation, Text(BulletContentText { text: "age_format, multiple @Languages codes, participant with special chars" })]) } }"#,
        r#"Utterance"#,
        r#"Utterance"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(
        reprs, expected,
        "headers-speaker-info.cha model output changed"
    );
}

/// `@ID` with every field after `speaker` entirely absent (no pipes at all
/// past the third field). Empirically this makes the WHOLE `@ID` line fail
/// tree-sitter's structural parse (an ERROR at the document level, handled by
/// the already-migrated Task B1 entry point via
/// `report_top_level_dependent_tier_error`), not a clean `id_header` node
/// with an internally-`Absent` role; `parse_id_header`'s own
/// `required_field`/`optional_field` diagnostic arms are not reached by this
/// fixture (they are documented as malformed-only / effectively unreachable
/// from a structurally-recovered `id_header`, consistent with what is
/// observed here).
const ID_MISSING_TAIL: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI
*CHI:\thello .
@End
";

/// `@Participants` with a participant that has a speaker code but no
/// following role word: exercises the participant-entry "role cannot be
/// empty" rejection (`parse_participant_entry`, the ONE inner malformed case
/// among these fixtures that reaches the intended per-file logic, since the
/// OUTER `extract_participants_header` content read still succeeds).
const PARTICIPANTS_MISSING_ROLE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI
@ID:\teng|corpus|CHI|||||Child|||
*CHI:\thello .
@End
";

/// `@Media` with empty contents (nothing after the tab). Empirically this
/// makes the WHOLE `@Media` line fail tree-sitter's structural parse (an
/// ERROR at the document level producing `EmptyMediaHeader`/E509), not a
/// clean `media_header` node with an internally-Missing `media_contents`.
const MEDIA_EMPTY_CONTENTS: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Media:\t
*CHI:\thello .
@End
";

/// `@Languages` with a digit-only token (the `language_code` grammar rule is
/// `/[a-z]{2,4}/`, so `123` cannot match it). Empirically this makes the
/// WHOLE `@Languages` line fail tree-sitter's structural parse (a
/// document-level `E316`), not a clean `languages_header` node reaching
/// `parse_languages_header`'s internals.
const LANGUAGES_DIGIT_TOKEN: &str = "\
@UTF8
@Begin
@Languages:\t123
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
*CHI:\thello .
@End
";

/// `@Languages` with entirely EMPTY contents (nothing between the tab and the
/// newline). Unlike `LANGUAGES_DIGIT_TOKEN` above, tree-sitter's structural
/// parse of the SURROUNDING `languages_header` succeeds here: empirically
/// (`tree-sitter parse`, `-c` CST dump) this produces a PRESENT, zero-width
/// `languages_contents` node whose only child is a zero-width MISSING
/// `language_code` placeholder at the first (non-repeated) position. That is
/// the pre-existing bug fixture: `parse_languages_header`'s un-migrated inner
/// loop used to read the MISSING placeholder's (empty) `utf8_text` straight
/// into `LanguageCode::new`, which panics on empty input (see the B2 ledger
/// "PRE-EXISTING BUG DISCOVERED" entry and the languages-header-panic
/// follow-up fix entry).
const LANGUAGES_EMPTY_CONTENTS: &str = "\
@UTF8
@Begin
@Languages:\t
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
*CHI:\thello .
@End
";

/// `@Situation` with empty contents. Empirically `free_text` CAN match
/// zero-width, so this reaches `parse_situation_header`'s `Present` decode
/// path with an empty string (NOT the "missing situation text" diagnostic
/// arm), plus an unrelated zero-width `MissingRequiredElement`/E342 for a
/// `continuation` node from the whole-tree backstop.
const SITUATION_EMPTY_CONTENTS: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Situation:\t
*CHI:\thello .
@End
";

/// `@Types` with only the design field present (activity and group absent).
/// Empirically this makes the WHOLE `@Types` line fail tree-sitter's
/// structural parse (a document-level `E316`), not a clean `types_header`
/// node reaching `parse_types_header`'s "missing activity field"
/// short-circuit.
const TYPES_MISSING_ACTIVITY_AND_GROUP: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Types:\tlong
*CHI:\thello .
@End
";

#[test]
fn id_missing_tail_yields_e505_via_document_level_recovery() {
    let (reprs, diags) = parse_lines_and_diags(ID_MISSING_TAIL);

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }]) }"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(reprs, expected, "id_missing_tail model output changed");

    assert_eq!(
        diags,
        vec![
            (
                "E505".to_string(),
                54,
                73,
                "Invalid @ID header format: structure could not be parsed".to_string(),
            ),
            (
                "E747".to_string(),
                73,
                74,
                "Blank lines are not allowed".to_string(),
            ),
            (
                "E522".to_string(),
                29,
                54,
                "Speaker 'CHI' declared in @Participants but has no matching @ID header"
                    .to_string(),
            ),
        ],
        "id_missing_tail diagnostics changed, got: {diags:?}"
    );
}

#[test]
fn participants_missing_role_rejects_entry_with_e513() {
    let (reprs, diags) = parse_lines_and_diags(PARTICIPANTS_MISSING_ROLE);

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(
        reprs, expected,
        "participants_missing_role model output changed"
    );

    assert_eq!(
        diags,
        vec![
            (
                "E513".to_string(),
                44,
                47,
                "Participant role cannot be empty".to_string(),
            ),
            (
                "E523".to_string(),
                48,
                81,
                "@ID header for 'CHI' but speaker not in @Participants".to_string(),
            ),
        ],
        "participants_missing_role diagnostics changed, got: {diags:?}"
    );
}

#[test]
fn media_empty_contents_yields_e509_via_document_level_recovery() {
    let (reprs, diags) = parse_lines_and_diags(MEDIA_EMPTY_CONTENTS);

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(reprs, expected, "media_empty_contents model output changed");

    assert_eq!(
        diags,
        vec![
            (
                "E509".to_string(),
                87,
                95,
                "@Media header cannot be empty".to_string(),
            ),
            (
                "E747".to_string(),
                95,
                96,
                "Blank lines are not allowed".to_string(),
            ),
        ],
        "media_empty_contents diagnostics changed, got: {diags:?}"
    );
}

#[test]
fn languages_digit_token_yields_e316_via_document_level_recovery() {
    let (reprs, diags) = parse_lines_and_diags(LANGUAGES_DIGIT_TOKEN);

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(
        reprs, expected,
        "languages_digit_token model output changed"
    );

    assert_eq!(
        diags,
        vec![
            (
                "E316".to_string(),
                13,
                28,
                "Unparsable content at file level: '@Languages:\t123'".to_string(),
            ),
            (
                "E747".to_string(),
                28,
                29,
                "Blank lines are not allowed".to_string(),
            ),
        ],
        "languages_digit_token diagnostics changed, got: {diags:?}"
    );
}

/// Regression pin for the pre-existing panic bug (see `LANGUAGES_EMPTY_CONTENTS`'s
/// doc comment): a MISSING `language_code` at the required first position of
/// `languages_contents` used to reach `LanguageCode::new` with empty text and
/// panic. After the fix, `parse_languages_header`'s inner loop is migrated to
/// the typed `NodeSlot`-exhaustive template, so the MISSING placeholder is
/// type-distinct from Present and is reported as a diagnostic instead.
#[test]
fn languages_empty_contents_reports_missing_language_code_not_panic() {
    let (reprs, diags) = parse_lines_and_diags(LANGUAGES_EMPTY_CONTENTS);

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(
        reprs, expected,
        "languages_empty_contents model output changed"
    );

    // TWO E342s at the same zero-width span: the region-level
    // `check_not_missing` diagnostic (this fix) plus the whole-tree backstop's
    // own independent diagnostic for the same MISSING node. This is the
    // documented "zero-width-MISSING dedup asymmetry" TASK-D AWARENESS caveat
    // from the B1/B2 migration ledger ("region-surfaced raw spans could
    // double-emit if a MISSING ever lands in a sink -- inert today, cover when
    // Task D reworks dedup"): dedup suppression keys off spans the backstop
    // itself widens by 1 byte, which a genuinely zero-width MISSING span does
    // not trigger. Not a regression introduced here (the panic path never
    // reached either diagnostic before this fix); pinned as-is, matching the
    // ledger's own guidance that this is Task D's to resolve, not this fix's.
    assert_eq!(
        diags,
        vec![
            (
                "E342".to_string(),
                25,
                25,
                "Tree-sitter error recovery: MISSING 'language_code' node inserted in languages_contents".to_string(),
            ),
            (
                "E342".to_string(),
                25,
                25,
                "Missing required 'language_code': the document is incomplete here and was only parsed via tree-sitter recovery (recovery is not validity)".to_string(),
            ),
        ],
        "languages_empty_contents diagnostics changed, got: {diags:?}"
    );
}

#[test]
fn situation_empty_contents_decodes_empty_text() {
    let (reprs, diags) = parse_lines_and_diags(SITUATION_EMPTY_CONTENTS);

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"Situation { text: SituationDescription("") }"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(
        reprs, expected,
        "situation_empty_contents model output changed"
    );

    assert_eq!(
        diags,
        vec![(
            "E342".to_string(),
            99,
            99,
            "Missing required 'continuation': the document is incomplete here and was only parsed via tree-sitter recovery (recovery is not validity)".to_string(),
        )],
        "situation_empty_contents diagnostics changed, got: {diags:?}"
    );
}

#[test]
fn types_missing_activity_and_group_yields_e316_via_document_level_recovery() {
    let (reprs, diags) = parse_lines_and_diags(TYPES_MISSING_ACTIVITY_AND_GROUP);

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(
        reprs, expected,
        "types_missing_activity_and_group model output changed"
    );

    assert_eq!(
        diags,
        vec![
            (
                "E316".to_string(),
                87,
                99,
                "Unparsable content at file level: '@Types:\tlong'".to_string(),
            ),
            (
                "E747".to_string(),
                99,
                100,
                "Blank lines are not allowed".to_string(),
            ),
        ],
        "types_missing_activity_and_group diagnostics changed, got: {diags:?}"
    );
}
