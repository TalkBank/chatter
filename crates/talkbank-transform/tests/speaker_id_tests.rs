//! L2 tests for `talkbank_transform::speaker_id::apply_mapping`.
//!
//! These tests exercise the mapping pipeline directly (no CLI
//! subprocess). They pin the precise rewrite rules from
//! `book/src/chatter/user-guide/speaker-id.md` so future
//! refactors cannot silently widen the behavior, for instance,
//! by rewriting utterance content beyond the speaker prefix, or by
//! losing the participant `name` field in `@Participants` rewrites.
//!
//! The file also contains override-file back-compat and round-trip
//! tests (Task 4) and the `llm_entries` audit-query test (Task 5).

use std::collections::{BTreeMap, HashMap};

use chrono::Utc;
use talkbank_model::ParseValidateOptions;
use talkbank_model::{ParticipantRole, SpeakerCode};
use talkbank_transform::parse_and_validate;
use talkbank_transform::speaker_id::{
    Confidence, ConfidenceField, ConfidenceMargin, DEFAULT_CONFIDENCE_THRESHOLD, DecisionEngine,
    DonorMatchReport, EndpointUrl, InsertedRoleSpec, JaccardScore, JudgmentProvenance, MappingSpec,
    MergeOverride, ModelId, OverrideFile, PromptVersion, SpeakerAssignment, SpeakerIdError,
    apply_mapping, identify_mapping,
};

/// Build a one-entry mapping renaming `PAR1` → `INV:Investigator`.
/// Used by every test in this file; the renamed-speaker side is the
/// most invariant-heavy path (utterance content byte-stable,
/// participants rewritten, ID rewritten).
fn par1_to_inv() -> MappingSpec {
    let mut m: MappingSpec = HashMap::new();
    m.insert(
        SpeakerCode::new("PAR1"),
        SpeakerAssignment::Rename {
            code: SpeakerCode::new("INV"),
            role: ParticipantRole::new("Investigator"),
        },
    );
    m
}

/// Rich-markup fixture for the byte-stability test. PAR1 carries:
///   - a CHAT error code `[*]`
///   - a filled-pause marker `&-um`
///   - a NAK-delimited time bullet
///   - a `%com:` dependent tier
///
/// Byte-stability means every byte on the *PAR1 line except the
/// leading `*PAR1:\t` prefix must appear in the output, plus the
/// `%com:` line must be preserved verbatim.
const FIX_RICH_PAR1: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR1 Investigator
@ID:\teng|corpus|PAR1|||||Investigator|||
@Media:\trich_par1, audio
*PAR1:\t&-um my notes [*] here . \u{15}500_3500\u{15}
%com:\tnoted as fluency-relevant turn
@End
";

/// Every byte of a PAR1 utterance EXCEPT the leading `*PAR1:\t`
/// prefix appears verbatim in the relabeled output, with `*INV:\t`
/// substituted in. Markup, time bullets, error codes, filled-pause
/// markers, and dependent tiers all round-trip unchanged.
#[test]
fn apply_mapping_byte_stable_except_prefix() {
    let options = ParseValidateOptions::default();
    let mapping = par1_to_inv();
    let relabeled = apply_mapping(FIX_RICH_PAR1, &mapping, options).expect("apply_mapping ok");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== relabeled output ===\n{relabeled}=== end ===");
    }

    // The *PAR1 prefix is gone; the *INV prefix takes its place; the
    // utterance content after the prefix is preserved byte-stable
    // (including all CHAT markup and the NAK bullet).
    let renamed_line = "*INV:\t&-um my notes [*] here . \u{15}500_3500\u{15}";
    assert!(
        relabeled.contains(renamed_line),
        "relabeled output missing byte-stable INV line.\n\
         expected substring: {renamed_line:?}\n\
         output:\n{relabeled}"
    );
    assert!(
        !relabeled.contains("*PAR1:"),
        "relabeled output should not contain the old *PAR1: prefix:\n{relabeled}"
    );

    // The `%com:` dependent tier attached to the renamed utterance is
    // preserved verbatim (with no speaker-code interference).
    let com_line = "%com:\tnoted as fluency-relevant turn";
    assert!(
        relabeled.contains(com_line),
        "relabeled output missing the %com dependent tier.\n\
         expected line: {com_line:?}\n\
         output:\n{relabeled}"
    );
}

/// Fixture for the @Participants-rewrite test. PAR1's entry carries
/// all three components, code (PAR1), participant name (Adam), role
/// (Participant), so the test can assert that:
///   - the code is rewritten (PAR1 → INV);
///   - the role-tag is rewritten (Participant → Investigator);
///   - the name token is PRESERVED from the input (Adam stays).
const FIX_PARTICIPANTS_FULL_ENTRY: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR1 Adam Participant
@ID:\teng|corpus|PAR1|||||Participant|||
@Media:\tparticipants_rewrite, audio
*PAR1:\thello . \u{15}0_1000\u{15}
@End
";

/// `@Participants` rewrite: speaker code and role tag come from the
/// mapping's `Rename` payload; the participant `name` field, if
/// present in the input entry, is preserved from the input. This
/// matches the contract in `speaker-id.md`: "the @Participants
/// entry's code and role-tag tokens are rewritten; any intervening
/// tokens (corpus ID, participant name) are preserved."
#[test]
fn apply_mapping_rewrites_participants() {
    let options = ParseValidateOptions::default();
    let mapping = par1_to_inv();
    let relabeled =
        apply_mapping(FIX_PARTICIPANTS_FULL_ENTRY, &mapping, options).expect("apply_mapping ok");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== relabeled output ===\n{relabeled}=== end ===");
    }

    let participants_line = relabeled
        .lines()
        .find(|l| l.starts_with("@Participants:"))
        .expect("relabeled output missing @Participants header");

    // Old code is gone; new code is present.
    assert!(
        !participants_line.contains("PAR1"),
        "@Participants should not contain PAR1: {participants_line}"
    );
    assert!(
        participants_line.contains("INV"),
        "@Participants should contain INV: {participants_line}"
    );

    // Role tag rewritten.
    assert!(
        participants_line.contains("Investigator"),
        "@Participants should contain new role 'Investigator': {participants_line}"
    );
    assert!(
        !participants_line.contains(" Participant"),
        "@Participants should not contain old role 'Participant' \
         (the trailing role token): {participants_line}"
    );

    // Participant name preserved from input.
    assert!(
        participants_line.contains("Adam"),
        "@Participants should preserve the input's participant name 'Adam': {participants_line}"
    );

    // Ordering: code first, then name, then role.
    let inv_pos = participants_line.find("INV").expect("INV present");
    let adam_pos = participants_line.find("Adam").expect("Adam present");
    let role_pos = participants_line
        .find("Investigator")
        .expect("Investigator present");
    assert!(
        inv_pos < adam_pos && adam_pos < role_pos,
        "@Participants token order should be code/name/role: \
         INV@{inv_pos}, Adam@{adam_pos}, Investigator@{role_pos} in {participants_line}"
    );
}

/// Fixture for the @ID-rewrite test. The PAR1 @ID row populates every
/// optional field so the test can assert which fields are rewritten
/// and which are preserved:
///
/// ```text
/// @ID:  language|corpus|speaker|age|sex|group|ses|role|education|custom|
///       eng     |fluencybank|PAR1|3;06.|female|exp|low|Participant|college|note|
/// ```
///
/// Per the contract, the rewrite touches field 3 (code) and field 8
/// (role) only, every other field is preserved.
const FIX_ID_FULL_FIELDS: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR1 Participant
@ID:\teng|fluencybank|PAR1|3;06.|female|exp|low|Participant|college|note|
@Media:\tid_rewrite, audio
*PAR1:\thello . \u{15}0_1000\u{15}
@End
";

/// `@ID` rewrite: field 3 (speaker code) and field 8 (role tag) are
/// rewritten from the `Rename` payload; every other field
/// (language, corpus, age, sex, group, SES, education, custom) is
/// preserved from the input row.
#[test]
fn apply_mapping_rewrites_id() {
    let options = ParseValidateOptions::default();
    let mapping = par1_to_inv();
    let relabeled = apply_mapping(FIX_ID_FULL_FIELDS, &mapping, options).expect("apply_mapping ok");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== relabeled output ===\n{relabeled}=== end ===");
    }

    let id_line = relabeled
        .lines()
        .find(|l| l.starts_with("@ID:"))
        .expect("relabeled output missing @ID row");

    // The whole expected line, asserted as a single substring so the
    // test fails loudly if ANY non-rewritten field changes (the
    // operator gets a single diff instead of nine separate asserts).
    let expected = "@ID:\teng|fluencybank|INV|3;06.|female|exp|low|Investigator|college|note|";
    assert_eq!(
        id_line, expected,
        "relabeled @ID row should rewrite only fields 3+8 (code+role) and preserve all other \
         fields verbatim.\n  got: {id_line}\n  exp: {expected}"
    );
}

// `identify_mapping`, reference-mode text-similarity matching.

/// Reference fixture: hand-coded child transcript. The anchor speaker
/// is CHI; the lexicon ("frog", "where", "go", "jar", "fall") is
/// distinctive enough that one donor speaker will overwhelmingly
/// share it and the other will not.
const FIX_REF_CHI_FROG: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tident_clean, audio
*CHI:\twhere did the frog go . \u{15}0_2000\u{15}
*CHI:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*CHI:\twhere is my frog . \u{15}5000_6500\u{15}
@End
";

/// Donor fixture: anonymous-2-speaker ASR output of the same media.
/// PAR0 speaks the child's lexicon (frog/jar/where/go) and will
/// match CHI overwhelmingly. PAR1 speaks the clinician's lexicon
/// (good/tell/about/yes) and will not match CHI at all.
const FIX_DONOR_CLEAN_WINNER: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|frogstory|PAR0|||||Participant|||
@ID:\teng|frogstory|PAR1|||||Participant|||
@Media:\tident_clean, audio
*PAR0:\twhere did the frog go . \u{15}0_2000\u{15}
*PAR1:\ttell me about the picture . \u{15}2000_2500\u{15}
*PAR0:\tthe frog fell in the jar . \u{15}2500_4500\u{15}
*PAR1:\tyes good . \u{15}4500_5000\u{15}
*PAR0:\twhere is my frog . \u{15}5000_6500\u{15}
*PAR1:\tthat is good . \u{15}6500_7000\u{15}
@End
";

/// `identify_mapping` picks the donor speaker whose content tokens
/// best match the reference anchor's tokens by multiset Jaccard. In
/// the clean-winner fixture, PAR0 shares the child's lexicon
/// (where, frog, go, jar, fell) and PAR1 does not (tell, about,
/// picture, yes, good). The winner is the donor speaker the merge
/// pipeline will DROP (the reference authoritatively covers them).
#[test]
fn identify_mapping_clean_winner() {
    let options = ParseValidateOptions::default();
    let reference =
        parse_and_validate(FIX_REF_CHI_FROG, options.clone()).expect("reference parses");
    let donor = parse_and_validate(FIX_DONOR_CLEAN_WINNER, options).expect("donor parses");

    let anchor = SpeakerCode::new("CHI");
    let report = identify_mapping(&reference, &anchor, &donor, DEFAULT_CONFIDENCE_THRESHOLD)
        .expect("identify_mapping ok");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!(
            "=== identify_mapping report ===\nwinner: {:?}\nmargin: {}\nscores: {:?}\n=== end ===",
            report.winner, report.margin, report.scores
        );
    }

    // PAR0 (child lexicon, matches CHI) wins.
    assert_eq!(
        report.winner,
        SpeakerCode::new("PAR0"),
        "expected PAR0 to win (matches CHI lexicon), got: {:?}",
        report.winner
    );

    // Both donor speakers got scored.
    assert!(
        report.scores.contains_key(&SpeakerCode::new("PAR0")),
        "scores should include PAR0; got: {:?}",
        report.scores
    );
    assert!(
        report.scores.contains_key(&SpeakerCode::new("PAR1")),
        "scores should include PAR1; got: {:?}",
        report.scores
    );

    // PAR0's score is strictly greater than PAR1's.
    let par0_score = report.scores[&SpeakerCode::new("PAR0")];
    let par1_score = report.scores[&SpeakerCode::new("PAR1")];
    assert!(
        par0_score > par1_score,
        "PAR0 score ({par0_score}) should beat PAR1 score ({par1_score})"
    );

    // The clean-winner margin is well above the default 2.0× threshold
    //, the test would expect ≥3.0× per the validation sweep findings.
    // PAR1 may legitimately score 0 if it shares no content tokens
    // with the reference; the infinite-margin case is reported as
    // f64::INFINITY by the implementation.
    assert!(
        report.margin.0 >= 3.0,
        "clean-winner margin should be ≥3.0 (got {}); fixture content needs adjusting if not",
        report.margin
    );
}

/// Reference fixture for the borderline test. CHI describes a
/// "frog jumped in the pond" scene with enough lexical overlap that
/// both donor speakers (a clinician asking about the same scene, and
/// the child re-describing it) will partially match.
const FIX_REF_CHI_POND: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|frogstory|CHI|3;06.||||Target_Child|||
@Media:\tident_borderline, audio
*CHI:\tthe frog jumped in the pond . \u{15}0_2000\u{15}
*CHI:\tthe frog is in the pond . \u{15}2000_4000\u{15}
*CHI:\twhere is the frog . \u{15}4000_5500\u{15}
@End
";

/// Donor fixture: BOTH PAR0 (clinician) and PAR1 (child re-take)
/// share substantial vocabulary with the reference. The clinician
/// asks scene questions using the same nouns; the child re-describes
/// the same scene with the same nouns. Neither overwhelmingly wins,
/// the margin between winner and runner-up sits below the default
/// 2.0× confidence threshold, so `identify_mapping` must refuse.
const FIX_DONOR_BORDERLINE: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|frogstory|PAR0|||||Participant|||
@ID:\teng|frogstory|PAR1|||||Participant|||
@Media:\tident_borderline, audio
*PAR0:\twhere is the frog now . \u{15}0_1500\u{15}
*PAR1:\tthe frog jumped . \u{15}1500_2500\u{15}
*PAR0:\tyou see the frog . \u{15}2500_3500\u{15}
*PAR1:\tin the pond . \u{15}3500_4500\u{15}
*PAR0:\tthe frog is jumping . \u{15}4500_5500\u{15}
*PAR1:\tthe frog . \u{15}5500_6500\u{15}
@End
";

/// `identify_mapping` refuses with `SpeakerIdError::LowConfidence`
/// when the winner→runner-up margin is below the supplied confidence
/// threshold. The error carries the per-speaker scores so the
/// operator can inspect the borderline and decide whether to lower
/// the threshold, supply explicit `--mapping`, or load a saved
/// override.
#[test]
fn identify_mapping_borderline_refuses() {
    let options = ParseValidateOptions::default();
    let reference = parse_and_validate(FIX_REF_CHI_POND, options.clone()).expect("ref parses");
    let donor = parse_and_validate(FIX_DONOR_BORDERLINE, options).expect("donor parses");
    let anchor = SpeakerCode::new("CHI");

    let err = identify_mapping(&reference, &anchor, &donor, DEFAULT_CONFIDENCE_THRESHOLD)
        .expect_err("borderline fixture should fail confidence-threshold check");

    match err {
        SpeakerIdError::LowConfidence { report, threshold } => {
            if std::env::var("TB_TEST_VERBOSE").is_ok() {
                eprintln!(
                    "=== borderline report ===\nwinner: {:?}\nmargin: {}\nthreshold: {threshold}\nscores: {:?}\n=== end ===",
                    report.winner, report.margin, report.scores
                );
            }
            // Margin is strictly below the threshold (the whole point
            // of refusing).
            assert!(
                !report.margin.meets(threshold),
                "margin ({}) should not meet threshold ({threshold})",
                report.margin
            );
            // Threshold echoed verbatim from the call.
            assert_eq!(
                threshold, DEFAULT_CONFIDENCE_THRESHOLD,
                "threshold in error should echo the parameter"
            );
            // Both donor speakers scored, the operator can inspect.
            assert!(
                report.scores.contains_key(&SpeakerCode::new("PAR0"))
                    && report.scores.contains_key(&SpeakerCode::new("PAR1")),
                "scores should include both donor speakers; got: {:?}",
                report.scores
            );
            // Both got a non-zero score; they really are borderline,
            // not one zero / one positive.
            for (spk, score) in report.scores.iter() {
                assert!(
                    score.0 > 0.0,
                    "borderline speaker {spk} should have a positive score, got {score}"
                );
            }
            // A winner exists even on the refusal path, the operator
            // (or `--write-pending`) needs to know which speaker the
            // algorithm WOULD have picked.
            assert!(
                report.scores.contains_key(&report.winner),
                "winner {:?} should be one of the scored donor speakers",
                report.winner
            );
        }
        other => panic!("expected SpeakerIdError::LowConfidence, got: {other:?}"),
    }
}

// Task 4: back-compat + round-trip tests for decision provenance.

/// A pre-provenance override file (no engine/judgment fields) must
/// still read, defaulting engine=Deterministic and judgment=None.
/// schema_version stays 1, so no version bump is needed.
#[test]
fn legacy_override_without_engine_reads_as_deterministic() {
    let toml = r#"
schema_version = 1

[NF203-2]
mode = "auto"
operator = "reference-mode"
decided_at = "2026-05-01T00:00:00Z"

[NF203-2.inserted_role]
code = "INV"
tag = "Investigator"

[NF203-2.mapping]
PAR0 = "rename"
PAR1 = "drop"
"#;
    let file: OverrideFile = toml::from_str(toml).unwrap();
    let entry = file.get("NF203-2").expect("entry present");
    assert_eq!(entry.engine, DecisionEngine::Deterministic);
    assert!(entry.judgment.is_none());
}

/// An entry with an LLM judgment block round-trips through TOML.
#[test]
fn llm_judgment_block_round_trips() {
    let prov = JudgmentProvenance {
        model: ModelId("deepseek-v4-flash".to_string()),
        endpoint: EndpointUrl("http://localhost:8000/v1".to_string()),
        prompt_version: PromptVersion("v1".to_string()),
        confidence: BTreeMap::from([(ConfidenceField::Mapping, Confidence(0.9))]),
        merge_applicable: true,
        reasoning: "assumed-CHI uses adult interviewer prompts".to_string(),
    };
    let serialized = toml::to_string(&prov).unwrap();
    let back: JudgmentProvenance = toml::from_str(&serialized).unwrap();
    assert_eq!(prov, back);
}

// Task 5: llm_entries audit-query test.

/// The audit query returns only entries whose engine is Llm. On a
/// purely deterministic file it is empty.
#[test]
fn llm_entries_filters_by_engine() {
    let mut file = OverrideFile::default();

    // Build one deterministic auto-decision entry. The mapping content
    // is immaterial here (the test only checks the engine tag), so reuse
    // the shared PAR1->INV fixture.
    let mapping = par1_to_inv();
    let report = DonorMatchReport {
        winner: SpeakerCode::new("PAR0"),
        scores: HashMap::from([
            (SpeakerCode::new("PAR0"), JaccardScore(0.8)),
            (SpeakerCode::new("PAR1"), JaccardScore(0.1)),
        ]),
        margin: ConfidenceMargin(8.0),
    };
    let inserted_role = InsertedRoleSpec {
        code: "INV".to_string(),
        tag: "Investigator".to_string(),
    };
    let entry = MergeOverride::auto_decision(
        &mapping,
        &report,
        inserted_role,
        "test-operator".to_string(),
        Utc::now(),
    );
    file.upsert("det-1".to_string(), entry);
    assert_eq!(file.llm_entries().count(), 0);
}
