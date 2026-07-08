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

//! L2 tests for `talkbank_transform::transcript_merge::merge_chats`.
//!
//! These tests exercise the merge over parsed `ChatFile` values
//! directly (no CLI subprocess). See
//! `book/src/architecture/merge-test-plan.md` for the cycle plan
//! that drives this file's incremental growth.
//!
//! Phase A cycle 2: byte-stability of retained-speaker utterances.

use talkbank_model::ParseValidateOptions;
use talkbank_model::SpeakerCode;
use talkbank_transform::transcript_merge::{MergeError, default_strip_tiers, merge_chats};

/// File 1 fixture for cycle 2. CHI carries:
///   - a CHAT error code `[*]`
///   - a filled-pause marker `&-um`
///   - a `%com:` dependent tier attached to the first utterance
///
/// Byte-stability under merge means the main tier line, its bullet,
/// and the `%com:` dependent line all appear verbatim in the output.
const FIX_REF_RICH_CHI: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@Media:\tcycle2, audio
*CHI:\t&-um my birthday was [*] yesterday . \u{15}500_3500\u{15}
%com:\tnoted as fluency-relevant sample
*CHI:\tand it was fun . \u{15}5000_6500\u{15}
@End
";

/// File 2 fixture for cycle 2. Single INV utterance positioned
/// between the two CHI utterances on the timeline.
const FIX_ASR_LABELED_RICH: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tcycle2, audio
*INV:\tthat sounds wonderful . \u{15}3500_4800\u{15}
@End
";

/// Byte-stability invariant: every retained-speaker main tier line
/// from File 1, and every dependent tier attached to those
/// utterances, appears in the merged output exactly as in File 1.
#[test]
fn merge_retained_speakers_byte_stable() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_RICH_CHI,
        FIX_ASR_LABELED_RICH,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("merge should succeed on valid inputs");

    // Print merged for inspection on failure; ignored by passing runs
    // unless TB_TEST_VERBOSE=1.
    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    // Both CHI main-tier lines preserved byte-stable, including
    // markup and bullet.
    let chi_line_1 = "*CHI:\t&-um my birthday was [*] yesterday . \u{15}500_3500\u{15}";
    let chi_line_2 = "*CHI:\tand it was fun . \u{15}5000_6500\u{15}";
    assert!(
        merged.contains(chi_line_1),
        "merged output missing byte-stable first CHI line.\n\
         expected line: {chi_line_1:?}\n\
         output:\n{merged}"
    );
    assert!(
        merged.contains(chi_line_2),
        "merged output missing byte-stable second CHI line.\n\
         expected line: {chi_line_2:?}\n\
         output:\n{merged}"
    );

    // The %com dependent tier attached to CHI's first utterance is
    // preserved verbatim.
    let com_line = "%com:\tnoted as fluency-relevant sample";
    assert!(
        merged.contains(com_line),
        "merged output missing the %com dependent tier attached to CHI.\n\
         expected line: {com_line:?}\n\
         output:\n{merged}"
    );

    // The %com line must appear in its original position relative to
    // its parent utterance: immediately after the *CHI: line, before
    // any subsequent utterance.
    let chi1_pos = merged
        .find(chi_line_1)
        .expect("CHI utterance 1 must be present");
    let com_pos = merged.find(com_line).expect("%com line must be present");
    let inv_pos = merged.find("*INV:").expect("INV utterance must be present");
    assert!(
        chi1_pos < com_pos && com_pos < inv_pos,
        "%com line not attached to its parent utterance: \
         chi1@{chi1_pos}, %com@{com_pos}, INV@{inv_pos}"
    );
}

// ============================================================================
// Phase A, cycle 3, derived-tier stripping on inserted speakers
// ============================================================================

/// File 1 fixture for cycle 3: retained CHI with a `%com:` dependent
/// tier. `%com` is NOT in the default strip set; the test confirms
/// it survives on the retained side. A header `@Comment:` line is
/// included to give cycle 8a's @Comment-concatenation test something
/// to preserve from File 1.
const FIX_REF_CHI_WITH_COM: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@Media:\tcycle3, audio
@Comment:\tSession recorded 2024-09-15 for fluency study.
*CHI:\thello there . \u{15}0_1000\u{15}
%com:\tchild waves hand
@End
";

/// File 2 fixture for cycle 3: donor INV with `%wor` AND `%mor`
/// (both in the default strip set), AND `%com` (not in strip set).
/// The contract says merge strips wor/mor/gra/pho from inserted
/// speakers; com/spa/etc. must survive.
///
/// A header `@Comment:` line carries ASR-pipeline provenance, exactly
/// the kind of donor metadata cycle 8a tests must preserve into the
/// merged output.
const FIX_ASR_INV_WITH_DERIVED: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tcycle3, audio
@Comment:\tASR engine rev. Unchecked output of ASR model.
*INV:\tgreetings friend . \u{15}1500_2800\u{15}
%wor:\tgreetings \u{15}1500_2100\u{15} friend \u{15}2300_2800\u{15} .
%mor:\tco|greetings n|friend .
%com:\tasr-generated investigator turn
@End
";

/// Default strip set (`%wor`, `%mor`, `%gra`, `%pho`) is removed
/// from INSERTED-speaker utterances. Tiers not in the strip set
/// (`%com`, `%spa`, etc.) and ALL tiers on retained-speaker
/// utterances are preserved.
#[test]
fn merge_strips_default_derived_tiers() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_CHI_WITH_COM,
        FIX_ASR_INV_WITH_DERIVED,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("merge should succeed on valid inputs");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    // Retained-side dependent tier survives.
    assert!(
        merged.contains("%com:\tchild waves hand"),
        "merged output missing retained CHI's %com line.\n{merged}"
    );

    // Inserted-side derived tiers are stripped.
    assert!(
        !merged.contains("%wor:"),
        "merged output still contains %wor (must be stripped from inserted speakers).\n{merged}"
    );
    assert!(
        !merged.contains("%mor:\tco|greetings"),
        "merged output still contains the donor's %mor row (must be stripped).\n{merged}"
    );

    // Inserted-side non-derived tier survives.
    assert!(
        merged.contains("%com:\tasr-generated investigator turn"),
        "merged output missing donor's %com line (NOT in default strip set, must survive).\n{merged}"
    );

    // Main tier of inserted speaker still present.
    assert!(
        merged.contains("*INV:\tgreetings friend . \u{15}1500_2800\u{15}"),
        "merged output missing INV main tier.\n{merged}"
    );
}

// ============================================================================
// Phase A, cycle 4, strip set is configurable
// ============================================================================

/// Passing a custom strip set replaces the default. With
/// `strip_tiers = ["com"]`:
///   - `%com` IS stripped from inserted speakers (it's in the
///     custom set);
///   - `%wor` SURVIVES on inserted speakers (it's no longer in
///     the strip set).
///
/// The fixtures from cycle 3 are reused; they already have
/// the right tier mix.
#[test]
fn merge_strip_tiers_configurable() {
    let options = ParseValidateOptions::default();
    let custom_strip = vec!["com".to_string()];
    let merged = merge_chats(
        FIX_REF_CHI_WITH_COM,
        FIX_ASR_INV_WITH_DERIVED,
        &[SpeakerCode::new("CHI")],
        &custom_strip,
        options,
    )
    .expect("merge should succeed on valid inputs");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    // Donor's %com IS now stripped; it was in the custom strip set.
    assert!(
        !merged.contains("%com:\tasr-generated investigator turn"),
        "merged output still contains donor's %com (must be stripped under custom strip set).\n{merged}"
    );

    // Donor's %wor SURVIVES; it is NOT in the custom strip set.
    assert!(
        merged.contains("%wor:"),
        "merged output missing donor's %wor (it should survive under custom strip set that excludes it).\n{merged}"
    );

    // Retained-speaker %com survives regardless, strip only applies
    // to inserted speakers.
    assert!(
        merged.contains("%com:\tchild waves hand"),
        "merged output missing retained CHI's %com (strip set must not affect retained speakers).\n{merged}"
    );
}

// ============================================================================
// Phase A, cycle 5, empty strip set preserves all dependent tiers
// ============================================================================

/// An empty `strip_tiers` slice degrades the strip behavior to a
/// no-op: every dependent tier, including `%wor`, `%mor`, `%gra`,
/// `%pho`, survives on inserted-speaker utterances. This is the
/// `--strip-tiers ''` escape hatch from the user-guide, used when a
/// caller wants the donor file's dependent tiers passed through
/// verbatim (e.g., for debugging, or when no downstream stage will
/// regenerate them).
#[test]
fn merge_strip_tiers_empty_preserves_all() {
    let options = ParseValidateOptions::default();
    let empty_strip: Vec<String> = Vec::new();
    let merged = merge_chats(
        FIX_REF_CHI_WITH_COM,
        FIX_ASR_INV_WITH_DERIVED,
        &[SpeakerCode::new("CHI")],
        &empty_strip,
        options,
    )
    .expect("merge should succeed on valid inputs");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    // All of the donor's dependent tiers survive, the strip set is
    // empty, so none are removed.
    assert!(
        merged.contains("%wor:"),
        "merged output missing donor's %wor under empty strip set.\n{merged}"
    );
    assert!(
        merged.contains("%mor:\tco|greetings n|friend ."),
        "merged output missing donor's %mor under empty strip set.\n{merged}"
    );
    assert!(
        merged.contains("%com:\tasr-generated investigator turn"),
        "merged output missing donor's %com under empty strip set.\n{merged}"
    );

    // Retained-side tier still present.
    assert!(
        merged.contains("%com:\tchild waves hand"),
        "merged output missing retained CHI's %com.\n{merged}"
    );
}

// ============================================================================
// Phase A, cycle 6, @Participants concatenation
// ============================================================================

/// Header reconciliation rule per the user-guide contract:
/// `@Participants` is the concatenation of File 1's entries followed
/// by File 2's entries for non-retained speakers, in their original
/// order within each file.
///
/// Uses the same fixtures as cycle 3 (CHI in File 1, INV in File 2);
/// the assertion is on the header rather than the body.
#[test]
fn merge_header_participants_concatenates() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_CHI_WITH_COM,
        FIX_ASR_INV_WITH_DERIVED,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("merge should succeed on valid inputs");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    // The merged @Participants line declares BOTH retained-side
    // CHI (from File 1) AND inserted-side INV (from File 2). The
    // exact form depends on parser/serializer canonicalization, so
    // we assert on the structural content rather than a verbatim
    // string.
    let participants_line = merged
        .lines()
        .find(|line| line.starts_with("@Participants:"))
        .expect("merged output missing @Participants header");
    assert!(
        participants_line.contains("CHI"),
        "@Participants missing CHI: {participants_line}"
    );
    assert!(
        participants_line.contains("Target_Child"),
        "@Participants missing CHI's role-tag Target_Child: {participants_line}"
    );
    assert!(
        participants_line.contains("INV"),
        "@Participants missing INV (inserted-side speaker): {participants_line}"
    );
    assert!(
        participants_line.contains("Investigator"),
        "@Participants missing INV's role-tag Investigator: {participants_line}"
    );

    // Ordering: File 1's entries come first.
    let chi_pos = participants_line.find("CHI").expect("CHI must be present");
    let inv_pos = participants_line.find("INV").expect("INV must be present");
    assert!(
        chi_pos < inv_pos,
        "@Participants ordering: File 1's CHI should precede File 2's INV. \
         CHI@{chi_pos}, INV@{inv_pos} in: {participants_line}"
    );
}

// ============================================================================
// Phase A, cycle 7, @ID concatenation
// ============================================================================

/// Header reconciliation rule per the user-guide contract: `@ID` rows
/// are the concatenation of File 1's rows (verbatim, in original
/// order) followed by File 2's rows for non-retained speakers (in
/// their original order within File 2).
///
/// Uses cycle 3's fixtures. The CHI @ID row should be preserved
/// verbatim from File 1; the INV @ID row should appear after it,
/// sourced from File 2.
#[test]
fn merge_header_id_concatenates() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_CHI_WITH_COM,
        FIX_ASR_INV_WITH_DERIVED,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("merge should succeed on valid inputs");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    // Collect all @ID lines from the merged output.
    let id_lines: Vec<&str> = merged
        .lines()
        .filter(|line| line.starts_with("@ID:"))
        .collect();

    // Exactly two @ID rows: one for CHI (from File 1), one for INV
    // (from File 2).
    assert_eq!(
        id_lines.len(),
        2,
        "merged should have two @ID rows (CHI + INV). got: {id_lines:?}"
    );

    // CHI's @ID row (assert structural content; canonicalization may
    // adjust trailing-pipe count).
    assert!(
        id_lines[0].contains("|CHI|"),
        "first @ID row should be CHI's. got: {}",
        id_lines[0]
    );
    assert!(
        id_lines[0].contains("|Target_Child|"),
        "CHI @ID row missing Target_Child role-tag. got: {}",
        id_lines[0]
    );

    // INV's @ID row.
    assert!(
        id_lines[1].contains("|INV|"),
        "second @ID row should be INV's. got: {}",
        id_lines[1]
    );
    assert!(
        id_lines[1].contains("|Investigator|"),
        "INV @ID row missing Investigator role-tag. got: {}",
        id_lines[1]
    );
}

// ============================================================================
// Phase A, cycle 8a, @Comment concatenation
// ============================================================================

/// Header reconciliation rule per the user-guide contract: `@Comment`
/// rows are the concatenation of File 1's rows followed by File 2's
/// rows, in their original order within each file. This preserves
/// donor provenance comments (ASR engine, run timestamp, processing
/// notes) into the merged file's audit trail.
///
/// Uses cycle 3's fixtures, now augmented with one `@Comment:` line
/// in each file. The retained-side comment (session metadata) and
/// the inserted-side comment (ASR provenance) must both appear, with
/// File 1's first.
#[test]
fn merge_header_comments_concatenate() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_CHI_WITH_COM,
        FIX_ASR_INV_WITH_DERIVED,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("merge should succeed on valid inputs");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    // Collect @Comment header lines (NOT %com dependent tier lines,
    // those start with `%` not `@`).
    let comment_lines: Vec<&str> = merged
        .lines()
        .filter(|line| line.starts_with("@Comment:"))
        .collect();

    // Expect two: File 1's session-metadata comment, then File 2's
    // ASR-provenance comment.
    assert_eq!(
        comment_lines.len(),
        2,
        "merged should have two @Comment rows. got: {comment_lines:?}"
    );

    assert!(
        comment_lines[0].contains("Session recorded 2024-09-15"),
        "first @Comment should be File 1's session metadata. got: {}",
        comment_lines[0]
    );
    assert!(
        comment_lines[1].contains("ASR engine"),
        "second @Comment should be File 2's ASR provenance. got: {}",
        comment_lines[1]
    );
}

// ============================================================================
// Phase A, cycle 8b, @Languages + @Media File-1-wins
// ============================================================================

/// Cycle-8b fixtures. The two files share `@Languages: eng` (a hard
/// precondition, language mismatch would be rejected by a later
/// cycle) but deliberately differ on the `@Media` modality field:
/// File 1 declares `video`, File 2 declares `audio`. The merge
/// contract says File 1's `@Media` wins regardless of modality, so
/// the merged output should carry `video`, not `audio`.
const FIX_REF_CYCLE8B: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@Media:\tcycle8b, video
*CHI:\thello . \u{15}0_1000\u{15}
@End
";

const FIX_ASR_CYCLE8B: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tcycle8b, audio
*INV:\thi there . \u{15}2000_3000\u{15}
@End
";

/// `@Languages` passthrough: exactly one `@Languages` line in the
/// merged output, carrying File 1's value. File 2's `@Languages` (in
/// the matching-language case) is discarded silently.
#[test]
fn merge_header_languages_passthrough() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_CYCLE8B,
        FIX_ASR_CYCLE8B,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("merge should succeed on valid inputs");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    let language_lines: Vec<&str> = merged
        .lines()
        .filter(|line| line.starts_with("@Languages:"))
        .collect();

    assert_eq!(
        language_lines.len(),
        1,
        "merged should have exactly one @Languages line. got: {language_lines:?}"
    );
    assert!(
        language_lines[0].contains("eng"),
        "@Languages should carry File 1's value 'eng'. got: {}",
        language_lines[0]
    );
}

/// `@Media` File-1-wins: exactly one `@Media` line in the merged
/// output, carrying File 1's value (including the modality field).
/// Even when File 2's `@Media` modality differs (here: audio vs
/// File 1's video), File 1's value is the one preserved.
#[test]
fn merge_header_media_file1_wins() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_CYCLE8B,
        FIX_ASR_CYCLE8B,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("merge should succeed on valid inputs");

    if std::env::var("TB_TEST_VERBOSE").is_ok() {
        eprintln!("=== merged output ===\n{merged}=== end ===");
    }

    let media_lines: Vec<&str> = merged
        .lines()
        .filter(|line| line.starts_with("@Media:"))
        .collect();

    assert_eq!(
        media_lines.len(),
        1,
        "merged should have exactly one @Media line. got: {media_lines:?}"
    );
    assert!(
        media_lines[0].contains("video"),
        "@Media should carry File 1's modality 'video', not File 2's 'audio'. got: {}",
        media_lines[0]
    );
    assert!(
        !media_lines[0].contains("audio"),
        "@Media should NOT contain File 2's 'audio' modality. got: {}",
        media_lines[0]
    );
}

// L2 sibling of the CLI-level `merge_no_retain_speakers_in_file1`:
// pins the exact `MergeError::RetainSpeakersMissing` variant + its
// `retain` payload so a CLI refactor cannot silently widen the
// failure into a generic "empty merge" arm.

/// File 1 has only `*PAR:` utterances; the retain set is `["CHI"]`.
/// `merge_chats` must refuse before any timeline / language check,
/// retain-set membership is the most fundamental shape question
/// (without any retained-speaker utterance, the entire merge is
/// ill-defined regardless of every other invariant).
const FIX_REF_PAR_ONLY_L2: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|corpus|PAR|||||Participant|||
@Media:\tprecond, audio
*PAR:\tsome utterance . \u{15}0_1000\u{15}
@End
";

/// `merge_chats` returns `MergeError::RetainSpeakersMissing { retain }`
/// when File 1 declares no utterances for any speaker in the retain
/// set. The variant's `retain` payload echoes the caller's input set
/// so an operator reading the error knows *which* set was searched
/// for.
#[test]
fn merge_no_retain_speakers_in_file1_returns_err() {
    let options = ParseValidateOptions::default();
    let retain = vec![SpeakerCode::new("CHI")];
    let err = merge_chats(
        FIX_REF_PAR_ONLY_L2,
        FIX_ASR_INV_PRECOND_L2,
        &retain,
        &default_strip_tiers(),
        options,
    )
    .expect_err("merge should refuse when File 1 has no utterances for any retain-set speaker");

    match err {
        MergeError::RetainSpeakersMissing { retain: echoed } => {
            assert_eq!(
                echoed, retain,
                "retain payload should echo the caller's input, got: {echoed:?}"
            );
        }
        other => panic!("expected MergeError::RetainSpeakersMissing, got: {other:?}"),
    }
}

// L2 sibling of `merge_no_timeline_in_file1`: pins the
// `MergeError::NoTimelineInFile1` variant against silent widening.

/// File 1 with retained-speaker utterances that lack time bullets.
/// The CHAT parser accepts unbulleted main tiers (legacy hand
/// transcripts often have them), but the merge needs a shared
/// timeline to position File 2's content against.
const FIX_REF_CHI_NO_BULLETS_L2: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@Media:\tprecond, audio
*CHI:\thello there .
*CHI:\tgoodbye .
@End
";

const FIX_ASR_INV_PRECOND_L2: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tprecond, audio
*INV:\tasr turn . \u{15}500_1500\u{15}
@End
";

/// `merge_chats` returns `MergeError::NoTimelineInFile1` when File 1
/// contains retained-speaker utterances but none of them carry a
/// time bullet. The merge has no shared timeline to anchor File 2's
/// content against, so it must refuse rather than emit a
/// meaningless start-time-less concatenation.
#[test]
fn merge_no_timeline_in_file1_returns_err() {
    let options = ParseValidateOptions::default();
    let err = merge_chats(
        FIX_REF_CHI_NO_BULLETS_L2,
        FIX_ASR_INV_PRECOND_L2,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect_err("merge should refuse when File 1 has no bulleted utterances");

    assert!(
        matches!(err, MergeError::NoTimelineInFile1),
        "expected MergeError::NoTimelineInFile1, got: {err:?}"
    );
}

// L2 sibling of `merge_language_mismatch`: pins the
// `MergeError::LanguageMismatch` variant + both files' code-list
// payloads against silent widening into a generic header-mismatch
// arm.

/// File 1 declares `@Languages: eng`. The retained speaker has a
/// time-bulleted utterance, so the timeline + retain preconditions
/// are satisfied; the only failing precondition is language equality.
const FIX_REF_CHI_ENG_L2: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@Media:\tprecond_lang, audio
*CHI:\thello there . \u{15}0_1000\u{15}
@End
";

/// File 2 declares `@Languages: yue` (Cantonese). Same media key as
/// File 1, but the language disagreement must trigger refusal.
const FIX_ASR_INV_YUE_L2: &str = "@UTF8
@Begin
@Languages:\tyue
@Participants:\tINV Investigator
@ID:\tyue|corpus|INV|||||Investigator|||
@Media:\tprecond_lang, audio
*INV:\t你好 . \u{15}500_1500\u{15}
@End
";

/// `merge_chats` returns `MergeError::LanguageMismatch` carrying both
/// files' declared `@Languages` codes when they disagree. Cross-language
/// merging would corrupt downstream language-aware stages (morphotag,
/// alignment, segmentation), so the merge refuses on disagreement
/// rather than emitting a mixed-language file.
#[test]
fn merge_language_mismatch_returns_err() {
    let options = ParseValidateOptions::default();
    let err = merge_chats(
        FIX_REF_CHI_ENG_L2,
        FIX_ASR_INV_YUE_L2,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect_err("merge should refuse when @Languages disagree");

    match err {
        MergeError::LanguageMismatch { file1, file2 } => {
            // The payload preserves each file's declared codes so the
            // operator can see *which* language pair was in conflict
            // without re-reading the inputs.
            let f1_codes: Vec<String> = file1.0.iter().map(|c| c.as_str().to_string()).collect();
            let f2_codes: Vec<String> = file2.0.iter().map(|c| c.as_str().to_string()).collect();
            assert_eq!(
                f1_codes,
                vec!["eng".to_string()],
                "file1 codes should be [eng], got {f1_codes:?}"
            );
            assert_eq!(
                f2_codes,
                vec!["yue".to_string()],
                "file2 codes should be [yue], got {f2_codes:?}"
            );
        }
        other => panic!("expected MergeError::LanguageMismatch, got: {other:?}"),
    }
}

// L2 sibling of `merge_ambiguous_speaker`: pins the
// `MergeError::AmbiguousSpeaker` variant and its `speaker` payload
// against silent widening into a generic precondition arm.

/// File 1 has both CHI (retain set) and INV (non-retained, hand-coded
/// clinician). File 2 also attributes utterances to INV. With
/// `--retain CHI`, INV is the ambiguous code: the merge cannot pick
/// between File 1's hand-coded INV and File 2's ASR INV.
const FIX_REF_CHI_PLUS_INV_L2: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, INV Investigator
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tambig, audio
*CHI:\thello there . \u{15}0_1000\u{15}
*INV:\thand-coded clinician turn . \u{15}1500_2500\u{15}
@End
";

const FIX_ASR_INV_AMBIG_L2: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tambig, audio
*INV:\tasr generated clinician turn . \u{15}3000_4000\u{15}
@End
";

/// `merge_chats` returns `MergeError::AmbiguousSpeaker { speaker }`
/// when a speaker code outside the retain set appears in both files.
/// The payload names the conflicting code so the operator can either
/// add it to `--retain` (favoring File 1's version) or rename File 2's
/// usage as a preprocessing step.
#[test]
fn merge_ambiguous_speaker_returns_err() {
    let options = ParseValidateOptions::default();
    let err = merge_chats(
        FIX_REF_CHI_PLUS_INV_L2,
        FIX_ASR_INV_AMBIG_L2,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect_err("merge should refuse when a non-retained speaker code appears in both files");

    match err {
        MergeError::AmbiguousSpeaker { speaker } => {
            assert_eq!(
                speaker,
                SpeakerCode::new("INV"),
                "expected ambiguous speaker = INV, got: {speaker:?}"
            );
        }
        other => panic!("expected MergeError::AmbiguousSpeaker, got: {other:?}"),
    }
}

// ============================================================================
// Dedupe-on-insert: file1 already declares a participant the donor also uses
// ============================================================================

/// File 1 fixture: reference transcript that vestigially declares `INV`
/// (a placeholder header row) but has zero `*INV:` utterances. Reproduces
/// the `CWNS-264-4` / `CWNS-265-4` shape from the IISRP merge.
const FIX_REF_VESTIGIAL_INV: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, INV Investigator
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tvestigial, audio
*CHI:\thello there . \u{15}0_1000\u{15}
@End
";

/// File 2 fixture: donor with real `INV` content using the same code and
/// role as file1's vestigial declaration.
const FIX_DONOR_REAL_INV: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tvestigial, audio
*INV:\thow are you today . \u{15}1000_2500\u{15}
@End
";

/// When file1 declares a participant code with zero utterances and
/// matching role, and the donor uses that same code with real content,
/// the merge must dedupe silently: exactly one `@Participants`/`@ID`
/// declaration for that code in the output, and the donor's utterances
/// merged in under it.
#[test]
fn merge_dedupes_vestigial_participant_declaration() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_VESTIGIAL_INV,
        FIX_DONOR_REAL_INV,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("merge should succeed: file1's INV declaration is vestigial and matches the donor's");

    let participants_count = merged.matches("@Participants:").count();
    assert_eq!(
        participants_count, 1,
        "expected exactly one @Participants header line; got {participants_count}\n{merged}"
    );
    let inv_entry_count = merged.matches("INV Investigator").count();
    assert_eq!(
        inv_entry_count, 1,
        "@Participants line must declare 'INV Investigator' exactly once; got:\n{merged}"
    );
    let inv_id_count = merged
        .lines()
        .filter(|l| l.starts_with("@ID:") && l.contains("|INV|"))
        .count();
    assert_eq!(
        inv_id_count, 1,
        "expected exactly one @ID row for INV; got {inv_id_count}\n{merged}"
    );
    assert!(
        merged.contains("*INV:\thow are you today . \u{15}1000_2500\u{15}"),
        "donor's INV utterance must be merged in under the shared code.\n{merged}"
    );
}

/// File 2 fixture: donor's INV entry has a DIFFERENT role than file1's
/// vestigial declaration (Investigator vs. a generic Adult), a metadata
/// conflict that must refuse rather than silently pick one side.
const FIX_DONOR_INV_ROLE_CONFLICT: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Adult
@ID:\teng|corpus|INV|||||Adult|||
@Media:\tvestigial, audio
*INV:\thow are you today . \u{15}1000_2500\u{15}
@End
";

#[test]
fn merge_refuses_on_role_conflicting_declared_participant() {
    let options = ParseValidateOptions::default();
    let err = merge_chats(
        FIX_REF_VESTIGIAL_INV,
        FIX_DONOR_INV_ROLE_CONFLICT,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect_err("merge must refuse: file1 says INV is Investigator, donor says INV is Adult");

    assert!(
        matches!(err, MergeError::ParticipantAlreadyDeclared { .. }),
        "expected ParticipantAlreadyDeclared; got: {err}"
    );
}

/// File 1 fixture: file1's vestigial `INV` declaration carries a real,
/// specific name ("Bob"), same role as file1's other vestigial fixture.
const FIX_REF_VESTIGIAL_INV_NAMED: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, INV Bob Investigator
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tvestigial, audio
*CHI:\thello there . \u{15}0_1000\u{15}
@End
";

/// File 2 fixture: donor's `INV` entry has the SAME role as file1's
/// vestigial declaration (Investigator) but a DIFFERENT specific name
/// ("Carol" vs. file1's "Bob"), a metadata conflict on the name
/// dimension alone that must refuse rather than silently pick one
/// side's name.
const FIX_DONOR_INV_NAME_CONFLICT: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Carol Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tvestigial, audio
*INV:\thow are you today . \u{15}1000_2500\u{15}
@End
";

/// Both file1 and the donor declare a name for `INV`, the roles match,
/// but the names disagree ("Bob" vs. "Carol"). Per the merge-robustness
/// design spec (Gap 1), name is part of the dedupe metadata-equality
/// check whenever both sides actually provide one; the merge must
/// refuse rather than silently keep file1's name and drop the donor's
/// conflicting one.
#[test]
fn merge_refuses_on_name_conflicting_declared_participant() {
    let options = ParseValidateOptions::default();
    let err = merge_chats(
        FIX_REF_VESTIGIAL_INV_NAMED,
        FIX_DONOR_INV_NAME_CONFLICT,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect_err("merge must refuse: file1 says INV is named Bob, donor says INV is named Carol");

    assert!(
        matches!(err, MergeError::ParticipantAlreadyDeclared { .. }),
        "expected ParticipantAlreadyDeclared; got: {err}"
    );
}

/// File 1 fixture: `INV` has REAL utterances in file1 (not vestigial) and
/// is not in `--retain`. Colliding with a donor `INV` must refuse, same
/// as the role-conflict case, even though the roles happen to match.
const FIX_REF_NONVESTIGIAL_INV: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, INV Investigator
@ID:\teng|corpus|CHI|2;06.||||Target_Child|||
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tnonvestigial, audio
*CHI:\thello there . \u{15}0_1000\u{15}
*INV:\thi yourself . \u{15}1000_2000\u{15}
@End
";

/// Donor fixture: `INV` is declared in `@Participants`/`@ID` but has ZERO
/// real utterances (the donor only utters via `SIS`). This isolates the
/// "file1 nonvestigial" branch of the new precondition: the pre-existing
/// `AmbiguousSpeaker` check only inspects UTTERANCE-bearing speakers in
/// File 2 (`unique_utterance_speakers`), so a donor `INV` with zero
/// utterances never reaches that check at all. `ParticipantAlreadyDeclared`
/// is the only check that can refuse this pairing, since it looks at
/// FILE 1's utterance count for the colliding code, not the donor's.
const FIX_DONOR_DECLARED_INV_NO_UTTERANCES: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator, SIS Sibling
@ID:\teng|corpus|INV|||||Investigator|||
@ID:\teng|corpus|SIS|||||Sibling|||
@Media:\tnonvestigial, audio
*SIS:\thi there . \u{15}1000_2000\u{15}
@End
";

#[test]
fn merge_refuses_on_nonvestigial_declared_participant() {
    let options = ParseValidateOptions::default();
    let err = merge_chats(
        FIX_REF_NONVESTIGIAL_INV,
        FIX_DONOR_DECLARED_INV_NO_UTTERANCES,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect_err(
        "merge must refuse: file1's INV is not vestigial (has real utterances), even \
         though the donor never actually utters as INV",
    );

    assert!(
        matches!(err, MergeError::ParticipantAlreadyDeclared { .. }),
        "expected ParticipantAlreadyDeclared; got: {err}"
    );
}

// ============================================================================
// @Languages subset matching: donor (ASR, monolingual) vs. reference
// (hand-coded, multilingual)
// ============================================================================

const FIX_REF_BILINGUAL: &str = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng, spa|corpus|CHI|2;06.||||Target_Child|||
@Media:\tlangs, audio
*CHI:\thello there . \u{15}0_1000\u{15}
@End
";

const FIX_DONOR_MONOLINGUAL_ENG: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tINV Investigator
@ID:\teng|corpus|INV|||||Investigator|||
@Media:\tlangs, audio
*INV:\thow are you today . \u{15}1000_2500\u{15}
@End
";

/// Reference declares [eng, spa]; donor declares [eng], a strict subset.
/// This must succeed today it does not (exact-equality check refuses).
#[test]
fn merge_succeeds_when_donor_languages_are_a_subset_of_reference() {
    let options = ParseValidateOptions::default();
    let merged = merge_chats(
        FIX_REF_BILINGUAL,
        FIX_DONOR_MONOLINGUAL_ENG,
        &[SpeakerCode::new("CHI")],
        &default_strip_tiers(),
        options,
    )
    .expect("donor's [eng] is a subset of reference's [eng, spa]; merge must succeed");
    assert!(
        merged.contains("@Languages:\teng, spa"),
        "merged output must carry file1's (reference's) @Languages verbatim.\n{merged}"
    );
}

/// Reference declares [eng] only; donor declares [eng, spa]. Donor is
/// over-claiming relative to reference; must still refuse.
#[test]
fn merge_refuses_when_donor_languages_exceed_reference() {
    let options = ParseValidateOptions::default();
    let err = merge_chats(
        FIX_DONOR_MONOLINGUAL_ENG, // reused as file1: declares only eng
        FIX_REF_BILINGUAL,         // reused as file2: declares eng, spa
        &[SpeakerCode::new("INV")],
        &default_strip_tiers(),
        options,
    )
    .expect_err("donor declaring spa when reference only declares eng must refuse");
    assert!(
        matches!(err, MergeError::LanguageMismatch { .. }),
        "expected LanguageMismatch; got: {err}"
    );
}
