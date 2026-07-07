# Merge Robustness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix three independent gaps in chatter's merge/speaker-id pipeline found via the IISRP corpus merge: silent-dedupe an already-declared participant on merge insert, relax `@Languages` matching to a subset check, and let the LLM holistic-judgment path represent multiple distinct adult speakers instead of refusing.

**Architecture:** Three independently committable phases, in ascending order of blast radius. Phase 1 and 2 are precondition/algorithm changes confined to one function in `crates/talkbank-transform/src/transcript_merge.rs`. Phase 3 threads a new `adult_roles: BTreeMap<String, InsertedRoleSpec>` shape through six files (judgment consumption, the pending-adjudication schema, the on-disk override/replay format, the CLI's operator-decision types, the writers, and the interactive/scripted rendering), replacing the current single `inserted_role: InsertedRoleSpec` field everywhere it appears.

**Tech Stack:** Rust 2024, `thiserror` for error types, `serde`/`toml` for on-disk formats, `cargo nextest` for the test runner.

**Approved design:** `docs/superpowers/specs/2026-07-01-merge-robustness-design.md` (this repo, commit `7fa0b49`). Read it before starting Phase 3 for the "why," not just the "what."

## Global Constraints

- Rust 2024 edition; `cargo fmt` before every commit; `cargo clippy --all-targets -- -D warnings` must stay clean.
- No panics (`unwrap`/`expect`) in library code (`talkbank-transform`, `talkbank-model`); test code may unwrap fixtures by convention.
- Every new/changed public type needs a `thiserror`-based error variant, never a stringly failure.
- File-size targets: ≤400 lines recommended, ≤800 hard limit per file. None of the files touched here are near the hard limit; do not split unless a task explicitly says to.
- **Mandatory regression gate** for any change touching the data model (`talkbank-model`) or transform pipeline (`talkbank-transform`), run before every phase's final commit:
  ```bash
    cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
  cargo nextest run -p talkbank-parser-tests --test roundtrip_reference_corpus
  ```
- Every book page touched in a phase updates its `**Last modified:**` header via `date '+%Y-%m-%d %H:%M %Z'` (never guessed/hardcoded), in the same commit as the code change.
- Test-first, red before green, for every step marked "Write the failing test."
- Use `cargo nextest run -p talkbank-transform --lib` / `-p talkbank-transform --test <name>` / `-p chatter --test <name>` for fast, scoped iteration; do not run bare `cargo test` (blocked by a pre-exec hook workspace-wide) or the full workspace suite mid-task.
- Never git push; local commits only, one per task, Franklin pushes when ready.

---

## Phase 1: Dedupe-on-insert for already-declared participants

**File:** `crates/talkbank-transform/src/transcript_merge.rs`
**Test file:** `crates/talkbank-transform/tests/transcript_merge_tests.rs`

### Task 1: Add `MergeError::ParticipantAlreadyDeclared` and the vestigial-dedupe success path

**Files:**
- Modify: `crates/talkbank-transform/src/transcript_merge.rs` (imports at top; `MergeError` enum, lines 38-98; `merge_chats`, lines 139-353)
- Test: `crates/talkbank-transform/tests/transcript_merge_tests.rs` (append at end)

**Interfaces:**
- Consumes: existing `MergeError`, `merge_chats(file1: &str, file2: &str, retain: &[SpeakerCode], strip_tiers: &[String], options: ParseValidateOptions) -> Result<String, MergeError>`.
- Produces: new error variant `MergeError::ParticipantAlreadyDeclared { speaker: SpeakerCode, file1_role: ParticipantRole, donor_role: ParticipantRole }`, consumed by Task 2's refusal test and by any future CLI error-rendering code (none exists yet for this variant; `Display` via `thiserror` is enough for now, matching every other `MergeError` variant).

- [ ] **Step 1: Write the failing test (vestigial-dedupe success case)**

Append to `crates/talkbank-transform/tests/transcript_merge_tests.rs`:

```rust
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
    let inv_participant_count = merged
        .lines()
        .filter(|l| l.starts_with("@Participants:") && l.contains("INV"))
        .count();
    assert_eq!(
        inv_participant_count, 1,
        "@Participants line must declare INV exactly once; got:\n{merged}"
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
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p talkbank-transform --test transcript_merge_tests -E 'test(merge_dedupes_vestigial_participant_declaration)'
```

Expected: FAIL. Today's code has no dedupe check, so file1's `INV` `@Participants` row and the donor's `INV` `@Participants` row are both emitted, and `participants_count` will still be 1 (both are concatenated into the SAME `@Participants:` header line since File 1's header gets rewritten with `combined` entries per lines 284-293 of the current file). The actual failure surfaces on `inv_id_count`: today emits 2 `@ID:` lines for INV (one from file1, one appended from file2), so that assertion fails with `2 != 1`. Confirm the actual failure message names `inv_id_count`.

- [ ] **Step 3: Implement the dedupe logic**

In `crates/talkbank-transform/src/transcript_merge.rs`, add the import (with the other `talkbank_model` imports at the top):

```rust
use talkbank_model::ParticipantRole;
```

Add a new `MergeError` variant, after `AmbiguousSpeaker` (around line 93, before the `Parse` variant):

```rust
    /// A participant code the donor uses (outside `--retain`) is already
    /// declared in File 1 with either real utterances or metadata that
    /// disagrees with the donor's declaration for that code. Silently
    /// keeping one side's declaration would either discard real content
    /// or paper over a genuine identity mismatch, so the merge refuses.
    #[error(
        "speaker {speaker} is already declared in File 1 (role {file1_role}) and also appears \
         in File 2's non-retained participants (role {donor_role}); this is ambiguous, resolve \
         by adding {speaker} to --retain or renaming it in File 2"
    )]
    ParticipantAlreadyDeclared {
        /// The colliding speaker code.
        speaker: SpeakerCode,
        /// File 1's declared role for this code.
        file1_role: ParticipantRole,
        /// The role the donor's entry for this code declares.
        donor_role: ParticipantRole,
    },
```

Add two helper functions after `last_header_index` (end of file, before the closing brace of the module, i.e. after line 408):

```rust
/// Speaker codes declared in `chat_file`'s `@Participants` header,
/// mapped to their full entry. Empty if the file has no
/// `@Participants` header line (CHAT expects exactly one; this stays
/// defensive rather than assuming).
fn declared_participants(
    chat_file: &ChatFile,
) -> std::collections::HashMap<SpeakerCode, ParticipantEntry> {
    chat_file
        .lines
        .0
        .iter()
        .filter_map(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::Participants { entries } => Some(entries),
                _ => None,
            },
            _ => None,
        })
        .flat_map(|entries| entries.iter().cloned())
        .map(|entry| (entry.speaker_code.clone(), entry))
        .collect()
}

/// Number of main-tier utterances in `chat_file` whose speaker is `code`.
fn utterance_count_for(chat_file: &ChatFile, code: &SpeakerCode) -> usize {
    chat_file
        .lines
        .0
        .iter()
        .filter(|line| matches!(line, Line::Utterance(u) if &u.main.speaker == code))
        .count()
}
```

Now change `merge_chats`. Insert this precondition right before the "Collect File 2's participant entries" comment (before current line 216), and change the two `.filter(...)` closures on `inserted_participants` and `inserted_id_lines` to also exclude deduped codes:

```rust
    // Precondition: a donor participant code (outside `--retain`) that
    // File 1 already declares must either be a safe silent dedupe
    // (File 1's declaration is vestigial: zero utterances, matching
    // role) or a refusal (File 1 has real content under that code, or
    // the two declarations disagree). Build the dedupe set up front so
    // the insertion filters below can consult it.
    let f1_declared = declared_participants(&f1);
    let mut dedupe_codes: std::collections::HashSet<SpeakerCode> = std::collections::HashSet::new();
    for line in f2.lines.0.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Participants { entries } = header.as_ref()
        {
            for donor_entry in entries.iter() {
                if in_retain(&donor_entry.speaker_code) {
                    continue;
                }
                if let Some(f1_entry) = f1_declared.get(&donor_entry.speaker_code) {
                    let vestigial = utterance_count_for(&f1, &donor_entry.speaker_code) == 0;
                    let roles_match = f1_entry.role == donor_entry.role;
                    if !vestigial || !roles_match {
                        return Err(MergeError::ParticipantAlreadyDeclared {
                            speaker: donor_entry.speaker_code.clone(),
                            file1_role: f1_entry.role.clone(),
                            donor_role: donor_entry.role.clone(),
                        });
                    }
                    dedupe_codes.insert(donor_entry.speaker_code.clone());
                }
            }
        }
    }

```

Then change the existing `inserted_participants` filter (current line 230) from:

```rust
        .filter(|entry| !in_retain(&entry.speaker_code))
```

to:

```rust
        .filter(|entry| !in_retain(&entry.speaker_code) && !dedupe_codes.contains(&entry.speaker_code))
```

And the existing `inserted_id_lines` filter (current lines 239-245), from:

```rust
        .filter(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::ID(id) => !in_retain(&id.speaker),
                _ => false,
            },
            _ => false,
        })
```

to:

```rust
        .filter(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::ID(id) => !in_retain(&id.speaker) && !dedupe_codes.contains(&id.speaker),
                _ => false,
            },
            _ => false,
        })
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p talkbank-transform --test transcript_merge_tests -E 'test(merge_dedupes_vestigial_participant_declaration)'
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/transcript_merge.rs crates/talkbank-transform/tests/transcript_merge_tests.rs
git commit -m "feat(merge): dedupe already-declared vestigial participants on insert"
```

### Task 2: Refusal path for non-vestigial or metadata-conflicting collisions

**Files:**
- Test: `crates/talkbank-transform/tests/transcript_merge_tests.rs` (append)

**Interfaces:**
- Consumes: `MergeError::ParticipantAlreadyDeclared` from Task 1.
- Produces: nothing new; this task is regression coverage for the refusal branch Task 1 already implemented.

- [ ] **Step 1: Write the failing test (should already pass; this step proves it, per TDD discipline of not trusting untested branches)**

Append to `crates/talkbank-transform/tests/transcript_merge_tests.rs`:

```rust
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
```

**Why this fixture pairing, and not `FIX_DONOR_REAL_INV` from Task 1:** pairing `FIX_REF_NONVESTIGIAL_INV` with `FIX_DONOR_REAL_INV` (both files having real `INV` utterances) is structurally caught by the pre-existing `AmbiguousSpeaker` precondition, which runs earlier in `merge_chats` and refuses first with a different error variant, since it independently guards "a non-retained speaker with utterances in both files." That pairing would never reach `ParticipantAlreadyDeclared` at all. This is expected, not a bug in Task 1: the two preconditions are complementary (one triggers on the *utterance* dimension, the other on the *header-declaration* dimension), and the brief places the new check after the existing one. Use `FIX_DONOR_DECLARED_INV_NO_UTTERANCES` above instead, which side-steps `AmbiguousSpeaker` by construction.

- [ ] **Step 2: Run both tests**

```bash
cargo nextest run -p talkbank-transform --test transcript_merge_tests -E 'test(merge_refuses_on_role_conflicting_declared_participant) or test(merge_refuses_on_nonvestigial_declared_participant)'
```

Expected: both PASS immediately (Task 1's implementation already covers both branches). If either fails, the Task 1 implementation has a bug, fix it there before proceeding, do not weaken these tests to match.

- [ ] **Step 3: Commit**

```bash
git add crates/talkbank-transform/tests/transcript_merge_tests.rs
git commit -m "test(merge): cover refusal paths for conflicting declared participants"
```

### Task 3: Regression gate and book update for Phase 1

**Files:**
- Modify: `book/src/architecture/merge-domain-types.md` (if it documents `MergeError` variants; otherwise skip the doc edit for this file and note why in the commit)
- Check: `book/src/chatter/user-guide/merge.md` for an error-list section

- [ ] **Step 1: Check whether `merge.md` documents `MergeError` variants**

```bash
grep -n "AmbiguousSpeaker\|RetainSpeakersMissing\|LanguageMismatch" book/src/chatter/user-guide/merge.md
```

If this returns matches, add a row for `ParticipantAlreadyDeclared` immediately after the `AmbiguousSpeaker` row, matching the existing table's exact column format (read the matched lines first to copy the format precisely). If it returns nothing, this book page doesn't enumerate error variants at the CLI level; skip the edit and note that in the commit message.

- [ ] **Step 2: Update the `Last modified` header on any book file touched**

```bash
date '+%Y-%m-%d %H:%M %Z'
```

Use the exact output to update the touched file's `**Last modified:**` line.

- [ ] **Step 3: Run the mandatory regression gate**

```bash
cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
cargo nextest run -p talkbank-parser-tests --test roundtrip_reference_corpus
cargo fmt -p talkbank-transform -- --check
cargo clippy -p talkbank-transform --all-targets -- -D warnings
```

Expected: all green. This phase touches `transcript_merge.rs` only (no grammar/parser/model change), so these gates are expected to pass unaffected; running them confirms no incidental breakage.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "docs(merge): document ParticipantAlreadyDeclared in the merge user guide"
```

(If Step 1 found nothing to change, skip this commit; Phase 1 is done as of Task 2's commit.)

---

## Phase 2: `@Languages` subset matching

**File:** `crates/talkbank-transform/src/transcript_merge.rs`

### Task 4: Subset-matching implementation with the regression-preserving test first

**Files:**
- Modify: `crates/talkbank-transform/src/transcript_merge.rs` (the `@Languages` precondition, current lines 149-160)
- Test: `crates/talkbank-transform/tests/transcript_merge_tests.rs`

**Interfaces:**
- Consumes: `MergeError::LanguageMismatch { file1: LanguageCodes, file2: LanguageCodes }` (unchanged shape; only the triggering condition and message change).
- Produces: nothing new for later tasks; Phase 2 is self-contained.

- [ ] **Step 1: Write the failing test (donor-subset succeeds)**

Append to `crates/talkbank-transform/tests/transcript_merge_tests.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p talkbank-transform --test transcript_merge_tests -E 'test(merge_succeeds_when_donor_languages_are_a_subset_of_reference)'
```

Expected: FAIL with `LanguageMismatch` (today's exact-equality check rejects `[eng, spa] != [eng]`).

- [ ] **Step 3: Implement the subset check**

In `crates/talkbank-transform/src/transcript_merge.rs`, replace the current precondition (lines 149-160):

```rust
    let f1_langs = extract_languages(&f1);
    let f2_langs = extract_languages(&f2);
    if f1_langs != f2_langs {
        return Err(MergeError::LanguageMismatch {
            file1: f1_langs,
            file2: f2_langs,
        });
    }
```

with:

```rust
    // Precondition: donor (File 2) must not declare a language reference
    // (File 1) doesn't have. Donor under-claiming (ASR run in a fixed
    // language mode) is expected and fine; donor over-claiming is
    // suspicious enough to refuse (a wrong-file pairing, or a language
    // the annotator missed either way needs a human look, not a silent
    // merge). Exact-equality is the special case where both sets match.
    let f1_langs = extract_languages(&f1);
    let f2_langs = extract_languages(&f2);
    let donor_over_claims = f2_langs.0.iter().any(|code| !f1_langs.0.contains(code));
    if donor_over_claims {
        return Err(MergeError::LanguageMismatch {
            file1: f1_langs,
            file2: f2_langs,
        });
    }
```

Update the `LanguageMismatch` variant's `#[error(...)]` message (current lines 65-69) to describe the actual failure direction:

```rust
    #[error(
        "File 2 declares language(s) not present in File 1's @Languages; \
         File 1 = {f1} ; File 2 = {f2}",
        f1 = file1.to_chat_string(),
        f2 = file2.to_chat_string(),
    )]
```

- [ ] **Step 4: Run test to verify it passes, plus the pre-existing exact-match test still passes**

```bash
cargo nextest run -p talkbank-transform --test transcript_merge_tests -E 'test(merge_succeeds_when_donor_languages_are_a_subset_of_reference)'
cargo nextest run -p talkbank-transform --test transcript_merge_tests -E 'test(merge_retained_speakers_byte_stable)'
```

Expected: both PASS (the second test uses matching `eng`/`eng` on both sides, exercised as a regression check that exact-match still works under the new subset logic).

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/transcript_merge.rs crates/talkbank-transform/tests/transcript_merge_tests.rs
git commit -m "feat(merge): relax @Languages matching to donor-subset-of-reference"
```

### Task 5: Regression test for donor over-claiming, plus book update and gate

**Files:**
- Test: `crates/talkbank-transform/tests/transcript_merge_tests.rs`
- Modify: `book/src/architecture/merge-domain-types.md` or `merge.md` (whichever documents `LanguageMismatch`'s trigger condition; grep first, same as Task 3)

- [ ] **Step 1: Write and run the over-claim regression test**

Append to `crates/talkbank-transform/tests/transcript_merge_tests.rs`:

```rust
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
```

Note: this reuses `FIX_DONOR_MONOLINGUAL_ENG` as file1 (retain `INV`, its only speaker) and `FIX_REF_BILINGUAL` as file2, purely to get an `[eng]` vs `[eng, spa]` pairing without a fifth fixture; the speaker/retain choice is incidental to what's under test.

```bash
cargo nextest run -p talkbank-transform --test transcript_merge_tests -E 'test(merge_refuses_when_donor_languages_exceed_reference)'
```

Expected: PASS immediately (Task 4's implementation already covers this branch).

- [ ] **Step 2: Grep and update book docs if they document the trigger condition**

```bash
grep -n "LanguageMismatch\|@Languages.*match\|exact.*language" book/src/architecture/merge-domain-types.md book/src/chatter/user-guide/merge.md
```

Update any matched prose describing "exact match required" to describe the subset rule instead. Stamp `**Last modified:**` via `date '+%Y-%m-%d %H:%M %Z'` on any file edited.

- [ ] **Step 3: Regression gate**

```bash
cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
cargo nextest run -p talkbank-parser-tests --test roundtrip_reference_corpus
cargo fmt -p talkbank-transform -- --check
cargo clippy -p talkbank-transform --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test(merge): cover donor-over-claims-languages refusal; update docs"
```

Phase 2 complete.

---

## Phase 3: Multi-adult-speaker representation

This phase threads a per-speaker role map through six files. Tasks are ordered bottom-up: foundational type changes first (Tasks 6-7, each independently compilable and tested), then Task 8, which threads `adult_roles` through the remaining five files (the producer `judgment_to_pending`, the on-disk replay format, the CLI-facing operator-decision types and apply logic, and the writers/interactive rendering) as six internal parts, since that rename ripples through all of them atomically and no intermediate slice compiles on its own. Task 9 adds subprocess-level test coverage; Task 10 updates the book docs.

**A note on scope the spec didn't spell out at the code level (flagging for your review, not a silent decision):** giving two same-role adults distinct `@Participants` entries per the CHAT manual's `CHI1`/`CHI2` convention requires a *specific-role label* (`First_Investigator`/`Second_Investigator`) that goes in `ParticipantEntry.name` and has no equivalent slot in today's `InsertedRoleSpec`/`SpeakerAssignment::Rename` types (they only carry `code` + standard `role`). Tasks 6-7 add an optional `specific_role` field to carry this. If you'd rather defer the same-role collision case entirely (Task 8 Part 2 is where it's implemented) and ship the "two adults, two different roles" case alone, that's a clean cut point, say so before Task 8 and we'll skip it.

### Task 6: Add `specific_role` to `SpeakerAssignment::Rename` and wire it through `apply_mapping_chat`

**Files:**
- Modify: `crates/talkbank-transform/src/speaker_id/mapping.rs`
- Modify: `crates/talkbank-transform/src/speaker_id/apply.rs`
- Test: `crates/talkbank-transform/src/speaker_id/apply.rs` (inline `#[cfg(test)]`, new module, since none exists there yet)

**Interfaces:**
- Consumes: `talkbank_model::{ParticipantRole, SpeakerCode}`, new `talkbank_model::ParticipantName` (already exists, used by `ParticipantEntry.name`).
- Produces: `SpeakerAssignment::Rename { code: SpeakerCode, role: ParticipantRole, specific_role: Option<ParticipantName> }` (was `{ code, role }`, this is a breaking change to the enum's field list; every construction site in this crate must be updated in this task or the crate won't compile, which is the point, the compiler finds every call site).

- [ ] **Step 1: Write the failing test**

Add to `crates/talkbank-transform/src/speaker_id/apply.rs`, at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use talkbank_model::ParticipantName;

    use super::*;

    const FIX_TWO_DONOR_SPEAKERS: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Adult, PAR1 Adult
@ID:\teng|corpus|PAR0||||Adult|||
@ID:\teng|corpus|PAR1||||Adult|||
@Media:\tspecific-role, audio
*PAR0:\thello there . \u{15}0_1000\u{15}
*PAR1:\thi yourself . \u{15}1000_2000\u{15}
@End
";

    /// `Rename` with `specific_role: Some(...)` must use it as the
    /// `@Participants` name/specific-role field, overriding whatever the
    /// donor's original entry carried (nothing, here).
    #[test]
    fn rename_with_specific_role_sets_participant_name() {
        let mut mapping = MappingSpec::new();
        mapping.insert(
            SpeakerCode::new("PAR0"),
            SpeakerAssignment::Rename {
                code: SpeakerCode::new("INV1"),
                role: ParticipantRole::new("Investigator"),
                specific_role: Some(ParticipantName::new("First_Investigator")),
            },
        );
        mapping.insert(
            SpeakerCode::new("PAR1"),
            SpeakerAssignment::Rename {
                code: SpeakerCode::new("INV2"),
                role: ParticipantRole::new("Investigator"),
                specific_role: Some(ParticipantName::new("Second_Investigator")),
            },
        );
        let result = apply_mapping(
            FIX_TWO_DONOR_SPEAKERS,
            &mapping,
            talkbank_model::ParseValidateOptions::default(),
        )
        .expect("apply_mapping should succeed");

        assert!(
            result.contains("INV1 First_Investigator Investigator"),
            "expected INV1's @Participants entry to carry the specific-role label.\n{result}"
        );
        assert!(
            result.contains("INV2 Second_Investigator Investigator"),
            "expected INV2's @Participants entry to carry the specific-role label.\n{result}"
        );
    }

    /// `Rename` with `specific_role: None` must fall back to the donor's
    /// original `@Participants` name field (today, `None`), matching the
    /// pre-existing single-role behavior exactly.
    #[test]
    fn rename_without_specific_role_preserves_donor_name() {
        let mut mapping = MappingSpec::new();
        mapping.insert(
            SpeakerCode::new("PAR0"),
            SpeakerAssignment::Rename {
                code: SpeakerCode::new("INV"),
                role: ParticipantRole::new("Investigator"),
                specific_role: None,
            },
        );
        mapping.insert(SpeakerCode::new("PAR1"), SpeakerAssignment::Drop);
        let result = apply_mapping(
            FIX_TWO_DONOR_SPEAKERS,
            &mapping,
            talkbank_model::ParseValidateOptions::default(),
        )
        .expect("apply_mapping should succeed");

        assert!(
            result.contains("INV Investigator") && !result.contains("INV_"),
            "expected plain 'INV Investigator' with no specific-role label.\n{result}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(rename_with_specific_role_sets_participant_name)'
```

Expected: FAIL to compile (`SpeakerAssignment::Rename` has no `specific_role` field yet).

- [ ] **Step 3: Add the field and wire it through**

In `crates/talkbank-transform/src/speaker_id/mapping.rs`, change the `Rename` variant (current lines 23-28):

```rust
    Rename {
        /// Replacement speaker code (e.g. `INV`, `CHI`).
        code: SpeakerCode,
        /// Replacement role tag (e.g. `Investigator`, `Target_Child`).
        role: ParticipantRole,
    },
```

to:

```rust
    Rename {
        /// Replacement speaker code (e.g. `INV`, `CHI`).
        code: SpeakerCode,
        /// Replacement role tag (e.g. `Investigator`, `Target_Child`).
        role: ParticipantRole,
        /// Specific-role label to use in `@Participants`' name/specific-role
        /// slot (CHAT manual convention: `First_Investigator`, matching
        /// `First_Sibling`/`Second_Sibling` for two people sharing a
        /// standard role). `None` preserves the donor's original entry's
        /// `name` field verbatim, matching pre-existing single-role
        /// behavior.
        specific_role: Option<talkbank_model::ParticipantName>,
    },
```

Update the doctest and `parse_mapping_spec`'s two construction sites in the same file (the doctest at the top, and the `SpeakerAssignment::Rename { code, role }` construction around line 73-76) to add `specific_role: None` (explicit CLI `--mapping`/`OLD=CODE:ROLE` syntax has no slot for a specific-role label; that's fine, it's an advanced case reachable only via the LLM judgment path for now):

```rust
            SpeakerAssignment::Rename {
                code: SpeakerCode::new(code.trim()),
                role: ParticipantRole::new(role.trim()),
                specific_role: None,
            }
```

And the doctest at the top of the file references `SpeakerAssignment::Drop` only, so it needs no change; confirm by re-reading it before editing anything else.

In `crates/talkbank-transform/src/speaker_id/apply.rs`, update `rewrite_header`'s two match arms (current lines 91-97 and 107-111):

```rust
                    Some(SpeakerAssignment::Rename {
                        code,
                        role,
                        specific_role,
                    }) => {
                        new_entries.push(ParticipantEntry {
                            speaker_code: code.clone(),
                            name: specific_role.clone().or_else(|| entry.name.clone()),
                            role: role.clone(),
                        });
                    }
```

```rust
            Some(SpeakerAssignment::Rename { code, role, .. }) => {
                let mut new_id = id.clone();
                new_id.speaker = code.clone();
                new_id.role = role.clone();
                HeaderRewrite::Keep(Header::ID(new_id))
            }
```

(`@ID`'s role field has no specific-role slot per the CHAT manual's pipe-delimited format, so `specific_role` is intentionally ignored there via `..`.)

Also update `apply_mapping_chat`'s utterance-rewrite match arm (current line 45, `Some(SpeakerAssignment::Rename { code, .. }) => {`), which already uses `..` and needs no change.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(rename_with_specific_role_sets_participant_name) or test(rename_without_specific_role_preserves_donor_name)'
```

Expected: PASS. Also re-run the full `speaker_id` module's existing tests to confirm the field addition didn't break anything else:

```bash
cargo nextest run -p talkbank-transform --lib -E 'package(talkbank-transform)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/speaker_id/mapping.rs crates/talkbank-transform/src/speaker_id/apply.rs
git commit -m "feat(speaker-id): add specific_role to SpeakerAssignment::Rename for same-role disambiguation"
```

### Task 7: Add `specific_role` to `InsertedRoleSpec`

**Files:**
- Modify: `crates/talkbank-transform/src/speaker_id/override_file.rs` (lines 80-99)

**Interfaces:**
- Consumes: nothing new.
- Produces: `InsertedRoleSpec { code: String, tag: String, specific_role: Option<String> }`, consumed by Task 8 Part 2's `judgment_to_pending` changes and Task 8 Part 4's `to_mapping_spec`.

- [ ] **Step 1: Write the failing test**

Add to `crates/talkbank-transform/src/speaker_id/override_file.rs`. Check first whether a `#[cfg(test)] mod tests` block already exists in this file (it wasn't in the portions read so far); if one exists, append to it, otherwise add a new one at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// `specific_role` must round-trip through TOML and default to `None`
    /// when absent (so pre-existing on-disk entries with just `code`/`tag`
    /// still parse once this field is added, since the schema bump in
    /// Task 8 covers the `adult_roles` shape, not this field).
    #[test]
    fn inserted_role_spec_specific_role_defaults_to_none_and_round_trips() {
        let without: InsertedRoleSpec =
            toml::from_str(r#"code = "INV"
tag = "Investigator""#)
            .expect("must parse without specific_role");
        assert_eq!(without.specific_role, None);

        let with = InsertedRoleSpec {
            code: "INV1".to_string(),
            tag: "Investigator".to_string(),
            specific_role: Some("First_Investigator".to_string()),
        };
        let toml_str = toml::to_string(&with).expect("must serialize");
        let back: InsertedRoleSpec = toml::from_str(&toml_str).expect("must parse back");
        assert_eq!(back, with);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(inserted_role_spec_specific_role_defaults_to_none_and_round_trips)'
```

Expected: FAIL to compile (`specific_role` field doesn't exist; also `InsertedRoleSpec` needs `PartialEq` for the `assert_eq!`, confirm it already derives it, current derive list is `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`, so this is already present).

- [ ] **Step 3: Add the field**

Change `InsertedRoleSpec` (current lines 83-89):

```rust
pub struct InsertedRoleSpec {
    /// CHAT speaker code (e.g. `INV`).
    pub code: String,
    /// CHAT role tag (e.g. `Investigator`).
    pub tag: String,
}
```

to:

```rust
pub struct InsertedRoleSpec {
    /// CHAT speaker code (e.g. `INV`, or `INV1` when disambiguated from
    /// a same-role collision).
    pub code: String,
    /// CHAT standard role tag (e.g. `Investigator`).
    pub tag: String,
    /// Specific-role label for `@Participants`' name/specific-role slot
    /// (e.g. `First_Investigator`), set only when two adults in the same
    /// judgment share `tag` and need the CHAT manual's `CHI1`/`CHI2`-style
    /// disambiguation. `None` for the ordinary single-adult-per-role case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specific_role: Option<String>,
}
```

Update `InsertedRoleSpec::new` (current lines 92-98) to keep taking two arguments and default the new field to `None` (all existing call sites construct via `::new`, so they need no change):

```rust
impl InsertedRoleSpec {
    /// Build from the typed CHAT primitives, with no specific-role label.
    pub fn new(code: &SpeakerCode, tag: &ParticipantRole) -> Self {
        Self {
            code: code.as_str().to_string(),
            tag: tag.as_str().to_string(),
            specific_role: None,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes, plus the whole crate still builds**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(inserted_role_spec_specific_role_defaults_to_none_and_round_trips)'
cargo check -p talkbank-transform -p chatter --all-targets
```

Expected: test PASSES; `cargo check` must also pass since every existing `InsertedRoleSpec { code, tag }` struct-literal construction site (if any exist beyond `::new`) would fail to compile without the new field, this step is where any such site surfaces. If `cargo check` fails, find the literal construction (likely in `adjudicate.rs`'s `parse_operator_response`/`parse_override_mapping`, which build `InsertedRoleSpec { code: ..., tag: ... }` directly, current lines 228-231 and 316-319) and add `specific_role: None` to each.

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/speaker_id/override_file.rs crates/chatter/src/commands/adjudicate.rs
git commit -m "feat(speaker-id): add specific_role to InsertedRoleSpec"
```

### Task 8: Thread the `adult_roles` map through the speaker-id/adjudication pipeline

This task merges what was originally planned as six separate tasks (`SuggestedSpeakerIdMapping`, `judgment_to_pending`, the `PendingAdjudications` schema bump, `MergeOverride`, `OperatorDecision`/`ScriptedChoice`/`apply_decision`, and the `writes.rs`/`adjudicate.rs` call sites) into one task with six internal parts. The field rename ripples through all seven files atomically: no intermediate part leaves the crate in a state worth an independent review gate, since `cargo check` only turns green again once every part below is done. Parts 1-5 may each end with an intermediate commit (git history stays readable), but this task is only complete, and only gets reviewed, once Part 6's final verification passes.

**Files:** all of `crates/talkbank-transform/src/adjudication.rs`, `crates/talkbank-transform/src/speaker_id/judgment/consume.rs`, `crates/talkbank-transform/src/speaker_id/override_file.rs`, `crates/chatter/src/commands/speaker_id/writes.rs`, `crates/chatter/src/commands/adjudicate.rs`.

**Interfaces:** consumes `InsertedRoleSpec` (Task 7) and `SpeakerAssignment::Rename { code, role, specific_role }` (Task 6). Produces a compiling, fully-tested crate where `adult_roles: BTreeMap<String, InsertedRoleSpec>` has replaced the old single `inserted_role: InsertedRoleSpec` field everywhere it appeared, consumed by Task 9's subprocess tests.


**Files:**
- Modify: `crates/talkbank-transform/src/adjudication.rs` (`SuggestedSpeakerIdMapping`, lines 109-117; the two inline test fixtures at lines ~685-686 and ~726-729)

**Interfaces:**
- Consumes: `InsertedRoleSpec` from Task 7.
- Produces: `SuggestedSpeakerIdMapping { mapping: BTreeMap<String, SpeakerAction>, adult_roles: BTreeMap<String, InsertedRoleSpec> }` (donor code → role), consumed by Part 2 (`judgment_to_pending`), Part 5 (`apply_decision`), Part 6 (`writes.rs` and `adjudicate.rs` rendering).

- [ ] **Step 1: Write the failing test**

This task's compile-time breakage IS the test: every existing construction/read site of `SuggestedSpeakerIdMapping.inserted_role` fails to compile once the field is renamed to a map. Run the build first to enumerate them precisely rather than guessing:

```bash
cargo check -p talkbank-transform -p chatter --all-targets 2>&1 | grep -A2 "inserted_role"
```

Expected at this point (before any change): clean build, no output. This step is a baseline; Step 3's rebuild is where the real "red" signal appears.

- [ ] **Step 2: Change the type**

In `crates/talkbank-transform/src/adjudication.rs`, change `SuggestedSpeakerIdMapping` (current lines 109-117):

```rust
pub struct SuggestedSpeakerIdMapping {
    /// Per-speaker action map the algorithm would have applied had
    /// the confidence threshold been lower.
    pub mapping: BTreeMap<String, SpeakerAction>,
    /// The inserted-role spec the algorithm would have paired with
    /// the mapping (typically from the CLI's `--inserted-role`).
    pub inserted_role: InsertedRoleSpec,
}
```

to:

```rust
pub struct SuggestedSpeakerIdMapping {
    /// Per-speaker action map the algorithm would have applied had
    /// the confidence threshold been lower.
    pub mapping: BTreeMap<String, SpeakerAction>,
    /// Per-donor-speaker-code role assignment, for every speaker whose
    /// `mapping` action is `Rename`. Empty when the mapping is all-Drop
    /// (no adult to assign a role to; there is no placeholder entry, an
    /// empty map is the correct representation of "no adult").
    pub adult_roles: BTreeMap<String, InsertedRoleSpec>,
}
```

- [ ] **Step 3: Rebuild to enumerate every broken call site**

```bash
cargo check -p talkbank-transform -p chatter --all-targets 2>&1 | grep -B2 "inserted_role\|no field\|adult_roles"
```

Expected: compile errors in `judgment/consume.rs` (`judgment_to_pending`), `adjudication.rs`'s own tests (`deterministic_entry_omits_engine_field_on_serialize`, `legacy_pending_toml_without_engine_defaults_to_deterministic`), `crates/chatter/src/commands/speaker_id/writes.rs` (`write_pending_entry`), `crates/chatter/src/commands/adjudicate.rs` (`apply_decision`'s match arms, `TerminalPrompter::ask`'s two rendering blocks). Do not fix these here, that's Parts 2-6; this step is purely to confirm the compiler's full list matches this plan's task list (if it finds a site not covered by an upcoming task, add a task for it before continuing).

- [ ] **Step 4: Fix ONLY this file's own inline tests, to keep this task isolated and compilable on its own where possible**

`adjudication.rs`'s two tests (`deterministic_entry_omits_engine_field_on_serialize` at current lines ~714-753, and `legacy_pending_toml_without_engine_defaults_to_deterministic` at lines ~671-710) construct/parse `SuggestedSpeakerIdMapping`/raw TOML directly. Update `deterministic_entry_omits_engine_field_on_serialize`'s fixture:

```rust
                suggested: SuggestedSpeakerIdMapping {
                    mapping: {
                        let mut m = std::collections::BTreeMap::new();
                        m.insert("PAR0".to_string(), SpeakerAction::Drop);
                        m
                    },
                    adult_roles: std::collections::BTreeMap::new(),
                },
```

(This test's point is the `engine`/`judgment` omission, not the role shape, so an empty map is the simplest correct fixture; it has a Drop-only mapping, consistent with "no adult" per the new empty-map convention from Step 2.)

`legacy_pending_toml_without_engine_defaults_to_deterministic` is deferred to Part 3 (it also needs the `schema_version` refusal logic Part 3 adds); leave it broken for now, note in the commit message that it's intentionally left red pending Part 3.

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/adjudication.rs
git commit -m "refactor(adjudication): SuggestedSpeakerIdMapping.inserted_role -> adult_roles map

Intermediate commit within this task; Part 2 onward continues the same field change."
```

#### Part 2: `judgment_to_pending` builds the multi-entry map, drops `MultipleAdults`, drops the INV placeholder sentinel

**Files:**
- Modify: `crates/talkbank-transform/src/speaker_id/judgment/consume.rs`

**Interfaces:**
- Consumes: `SuggestedSpeakerIdMapping.adult_roles` from Task 8, `HolisticJudgment.adult_roles: BTreeMap<DonorCode, AdultRole>` (unchanged, already multi-entry-capable), `AdultRole::inserted_role_spec()` (unchanged).
- Produces: `judgment_to_pending` with no `MultipleAdults` error variant; same-role auto-disambiguation (numbered codes + specific-role labels).

- [ ] **Step 1: Write the failing tests**

Replace the existing `multiple_adults_is_error` test (current lines 273-297) with two new tests, and update the two tests that assert on `inserted_role` (`adult_verdict_becomes_rename_child_and_drop_become_drop`, current lines 184-218, and `merge_not_applicable_with_no_adult_uses_placeholder_ok`, current lines 386-424):

Delete `multiple_adults_is_error` entirely and replace with:

```rust
    /// Two adult verdicts with two DIFFERENT roles must both appear in
    /// `adult_roles`, no error (this is the behavior change from the
    /// prior `MultipleAdults` refusal).
    #[test]
    fn two_adults_with_different_roles_both_map_to_rename() {
        let judgment = HolisticJudgment {
            speaker_mapping: BTreeMap::from([
                (DonorCode("PAR0".to_string()), SpeakerVerdict::Adult),
                (DonorCode("PAR1".to_string()), SpeakerVerdict::Adult),
            ]),
            adult_roles: BTreeMap::from([
                (DonorCode("PAR0".to_string()), AdultRole::Inv),
                (DonorCode("PAR1".to_string()), AdultRole::Fat),
            ]),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: true,
            confidence: BTreeMap::new(),
            reasoning: "two adults, different roles".to_string(),
        };

        let entry = judgment_to_pending("sess-two-adults", &judgment, &test_meta(), fixed_ts())
            .expect("two adults with distinct roles must succeed, not MultipleAdults");

        let PendingKindData::SpeakerIdLowConfidence { suggested } = &entry.data else {
            panic!("expected SpeakerIdLowConfidence; got: {:?}", entry.data.kind());
        };
        assert_eq!(suggested.mapping.get("PAR0"), Some(&SpeakerAction::Rename));
        assert_eq!(suggested.mapping.get("PAR1"), Some(&SpeakerAction::Rename));
        assert_eq!(
            suggested.adult_roles.get("PAR0").map(|r| (r.code.as_str(), r.tag.as_str())),
            Some(("INV", "Investigator")),
        );
        assert_eq!(
            suggested.adult_roles.get("PAR1").map(|r| (r.code.as_str(), r.tag.as_str())),
            Some(("FAT", "Father")),
        );
        assert!(
            suggested.adult_roles.get("PAR0").unwrap().specific_role.is_none(),
            "distinct-role adults need no specific-role disambiguation label"
        );
    }

    /// Two adult verdicts assigned the SAME role must auto-disambiguate:
    /// numbered codes (INV1/INV2) and specific-role labels
    /// (First_Investigator/Second_Investigator), per the CHAT manual's
    /// CHI1/CHI2 convention. Disambiguation order follows BTreeMap
    /// iteration order over the donor codes (alphabetical: PAR0 before
    /// PAR1), so PAR0 becomes "First".
    #[test]
    fn two_adults_with_same_role_auto_disambiguate() {
        let judgment = HolisticJudgment {
            speaker_mapping: BTreeMap::from([
                (DonorCode("PAR0".to_string()), SpeakerVerdict::Adult),
                (DonorCode("PAR1".to_string()), SpeakerVerdict::Adult),
            ]),
            adult_roles: BTreeMap::from([
                (DonorCode("PAR0".to_string()), AdultRole::Inv),
                (DonorCode("PAR1".to_string()), AdultRole::Inv),
            ]),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: true,
            confidence: BTreeMap::new(),
            reasoning: "two adults, same role".to_string(),
        };

        let entry = judgment_to_pending("sess-same-role", &judgment, &test_meta(), fixed_ts())
            .expect("two adults with the same role must succeed via auto-disambiguation");

        let PendingKindData::SpeakerIdLowConfidence { suggested } = &entry.data else {
            panic!("expected SpeakerIdLowConfidence; got: {:?}", entry.data.kind());
        };
        let par0 = suggested.adult_roles.get("PAR0").expect("PAR0 must have a role");
        let par1 = suggested.adult_roles.get("PAR1").expect("PAR1 must have a role");
        assert_eq!(par0.code, "INV1");
        assert_eq!(par1.code, "INV2");
        assert_eq!(par0.tag, "Investigator");
        assert_eq!(par1.tag, "Investigator");
        assert_eq!(par0.specific_role.as_deref(), Some("First_Investigator"));
        assert_eq!(par1.specific_role.as_deref(), Some("Second_Investigator"));
    }
```

Update `adult_verdict_becomes_rename_child_and_drop_become_drop`'s two `inserted_role`-asserting lines (current lines 210-217):

```rust
        assert_eq!(
            suggested.inserted_role.code, "INV",
            "inserted_role code must be INV for AdultRole::Inv"
        );
        assert_eq!(
            suggested.inserted_role.tag, "Investigator",
            "inserted_role tag must be Investigator for AdultRole::Inv"
        );
```

to:

```rust
        let role = suggested
            .adult_roles
            .get("PAR1")
            .expect("PAR1 (the sole adult) must have a role entry");
        assert_eq!(role.code, "INV", "role code must be INV for AdultRole::Inv");
        assert_eq!(role.tag, "Investigator", "role tag must be Investigator for AdultRole::Inv");
        assert!(role.specific_role.is_none(), "a single adult needs no disambiguation label");
```

Update `merge_not_applicable_with_no_adult_uses_placeholder_ok`'s final assertion block (current lines 419-423) and its doc comment (current line 384-385), removing the placeholder concept entirely:

```rust
        // No adult was named, so adult_roles must be empty, not a
        // placeholder entry (there is no speaker to assign a role to;
        // mapping is all-Drop).
        assert!(
            suggested.adult_roles.is_empty(),
            "adult_roles must be empty when there is no adult, not a placeholder entry"
        );
```

Rename the test itself from `merge_not_applicable_with_no_adult_uses_placeholder_ok` to `merge_not_applicable_with_no_adult_yields_empty_adult_roles` (update the `#[test] fn` line), since the placeholder no longer exists.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(two_adults_with_different_roles_both_map_to_rename) or test(two_adults_with_same_role_auto_disambiguate) or test(adult_verdict_becomes_rename_child_and_drop_become_drop) or test(merge_not_applicable_with_no_adult_yields_empty_adult_roles)'
```

Expected: FAIL to compile (`judgment_to_pending` still builds the old single `inserted_role` field and still has the `MultipleAdults` refusal).

- [ ] **Step 3: Implement**

Replace `judgment_to_pending`'s body (current lines 58-124) with:

```rust
pub fn judgment_to_pending(
    session_id: &str,
    judgment: &HolisticJudgment,
    meta: &ProvenanceMeta,
    created_at: DateTime<Utc>,
) -> Result<PendingEntry, ConsumeError> {
    let mut mapping: BTreeMap<String, SpeakerAction> = BTreeMap::new();
    let mut adults: Vec<(String, AdultRole)> = Vec::new();

    for (code, verdict) in &judgment.speaker_mapping {
        let action = match verdict {
            SpeakerVerdict::Adult => {
                let role = judgment
                    .adult_roles
                    .get(code)
                    .ok_or_else(|| ConsumeError::AdultRoleMissing(code.0.clone()))?;
                adults.push((code.0.clone(), *role));
                SpeakerAction::Rename
            }
            SpeakerVerdict::Child | SpeakerVerdict::Drop => SpeakerAction::Drop,
        };
        mapping.insert(code.0.clone(), action);
    }

    if judgment.merge_applicable && adults.is_empty() {
        return Err(ConsumeError::NoAdultButMergeApplicable);
    }

    let adult_roles = disambiguate_adult_roles(adults);

    let suggested = SuggestedSpeakerIdMapping {
        mapping,
        adult_roles,
    };

    let provenance = JudgmentProvenance {
        model: meta.model.clone(),
        endpoint: meta.endpoint.clone(),
        prompt_version: meta.prompt_version.clone(),
        confidence: judgment.confidence.clone(),
        merge_applicable: judgment.merge_applicable,
        reasoning: judgment.reasoning.clone(),
    };

    Ok(PendingEntry {
        session_id: session_id.to_string(),
        created_at,
        data: PendingKindData::SpeakerIdLowConfidence { suggested },
        scores: BTreeMap::new(),
        margin: None,
        threshold_used: None,
        engine: DecisionEngine::Llm,
        judgment: Some(provenance),
    })
}

/// Build the on-disk `adult_roles` map from `(donor_code, AdultRole)`
/// pairs, auto-disambiguating any `AdultRole` shared by 2+ donor codes
/// per the CHAT manual's `CHI1`/`CHI2` convention (numbered speaker
/// codes, a `First_`/`Second_`/... specific-role label, the shared
/// standard role tag unchanged). Codes within a colliding group are
/// ordered by their existing (already-alphabetical, from the BTreeMap
/// traversal in `judgment_to_pending`) input order.
fn disambiguate_adult_roles(
    adults: Vec<(String, AdultRole)>,
) -> BTreeMap<String, InsertedRoleSpec> {
    const ORDINALS: &[&str] = &["First", "Second", "Third", "Fourth"];

    let mut by_role: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for (code, role) in &adults {
        by_role.entry(role.as_code()).or_default().push(code.clone());
    }

    let mut result: BTreeMap<String, InsertedRoleSpec> = BTreeMap::new();
    for (donor_code, role) in adults {
        let siblings = &by_role[role.as_code()];
        if siblings.len() < 2 {
            result.insert(
                donor_code,
                InsertedRoleSpec {
                    code: role.as_code().to_string(),
                    tag: role.inserted_role_spec().tag,
                    specific_role: None,
                },
            );
            continue;
        }
        let position = siblings
            .iter()
            .position(|c| c == &donor_code)
            .expect("donor_code is a member of its own siblings group by construction");
        let ordinal = ORDINALS
            .get(position)
            .unwrap_or_else(|| panic!("more than {} donor speakers share role {}; extend ORDINALS", ORDINALS.len(), role.as_code()));
        let tag = role.inserted_role_spec().tag;
        result.insert(
            donor_code,
            InsertedRoleSpec {
                code: format!("{}{}", role.as_code(), position + 1),
                tag: tag.clone(),
                specific_role: Some(format!("{ordinal}_{tag}")),
            },
        );
    }
    result
}
```

Remove the now-unused `ConsumeError::MultipleAdults` variant (current lines 31-35 of the `ConsumeError` enum). Check whether `ConsumeError` is `#[non_exhaustive]` or matched exhaustively anywhere else in the crate before deleting; grep first:

```bash
grep -rn "ConsumeError::" crates/ --include="*.rs" | grep -v "consume.rs"
```

If any match outside `consume.rs` references `MultipleAdults`, update it in this step too.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(two_adults_with_different_roles_both_map_to_rename) or test(two_adults_with_same_role_auto_disambiguate) or test(adult_verdict_becomes_rename_child_and_drop_become_drop) or test(merge_not_applicable_with_no_adult_yields_empty_adult_roles) or test(adult_without_role_is_error) or test(merge_applicable_true_without_adult_is_error)'
```

Expected: all PASS (the last two are pre-existing tests unaffected by this change; confirm they still pass since `NoAdultButMergeApplicable`'s check moved from "adult.is_none()" to "adults.is_empty()", equivalent condition).

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/speaker_id/judgment/consume.rs
git commit -m "feat(speaker-id): judgment_to_pending supports multiple adults, auto-disambiguates same-role collisions

Removes ConsumeError::MultipleAdults. HolisticJudgment already let the
LLM assign distinct roles per adult speaker; this was the one place
that discarded that information. Same-role collisions (two adults
both e.g. Investigator) get numbered codes + specific-role labels per
the CHAT manual's CHI1/CHI2 convention."
```

#### Part 3: `PendingAdjudications` schema-version enforcement, bump to 2

**Files:**
- Modify: `crates/talkbank-transform/src/adjudication.rs` (`PendingAdjudications`, current lines 232-299; the deferred `legacy_pending_toml_without_engine_defaults_to_deterministic` test from Task 8)

**Interfaces:**
- Consumes: nothing new.
- Produces: `PendingAdjudications::CURRENT_SCHEMA_VERSION: u32 = 2`; `read()`/`read_or_default()` refuse any other value with a new `AdjudicationError::UnsupportedSchemaVersion { found, supported }` variant, mirroring `OverrideFileError::UnsupportedSchemaVersion`'s existing pattern in `override_file.rs`.

**Important finding this task corrects:** `PendingAdjudications::read()` today does **not** check `schema_version` at all (unlike `OverrideFile::read_or_default`, which does); the design spec's phrasing assumed this enforcement already existed system-wide. This task adds it, it isn't already there.

- [ ] **Step 1: Write the failing tests**

Add a new `AdjudicationError` variant test and fix the deferred legacy test. First, the new refusal test, appended to `adjudication.rs`'s test module:

```rust
    /// A pending file whose schema_version is not CURRENT_SCHEMA_VERSION
    /// must be refused with a typed error, not silently accepted (today
    /// there is no check at all; this test is the "before" proof).
    #[test]
    fn read_refuses_wrong_schema_version() {
        let dir = std::env::temp_dir().join(format!(
            "pending-schema-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("pending.toml");
        std::fs::write(&path, "schema_version = 1\n").expect("write fixture");

        let err = PendingAdjudications::read(&path).expect_err("schema_version 1 must be refused");
        assert!(
            matches!(
                err,
                AdjudicationError::UnsupportedSchemaVersion {
                    found: Some(1),
                    supported: PendingAdjudications::CURRENT_SCHEMA_VERSION,
                }
            ),
            "expected UnsupportedSchemaVersion{{found: Some(1), supported: 2}}; got: {err}"
        );
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }
```

Now fix `legacy_pending_toml_without_engine_defaults_to_deterministic` (currently broken from this task's field rename). Its actual intent, per its doc comment, is "engine/judgment fields default correctly when absent"; that's still a real, valid thing to test, but it must now use `schema_version = 2` (the current version under this task's change) and the new `adult_roles` map shape instead of `inserted_role`:

```rust
    #[test]
    fn legacy_pending_toml_without_engine_defaults_to_deterministic() {
        // Minimal valid pending TOML at the current schema version, with
        // no `engine` or `judgment` keys (simulating a file written before
        // those fields existed, which predates this schema version but
        // shares its shape otherwise). The `adult_roles` map is an inline
        // table of inline tables; the speaker-id-low-confidence kind tag
        // must be present for the flattened enum to parse.
        let toml = r#"
schema_version = 2

[[entries]]
session_id = "legacy-session-1"
created_at = "2026-01-01T00:00:00Z"
kind = "speaker-id-low-confidence"

[entries.suggested]

[entries.suggested.mapping]
PAR0 = "drop"
PAR1 = "rename"

[entries.suggested.adult_roles]
PAR1 = { code = "INV", tag = "Investigator" }
"#;

        let parsed: PendingAdjudications =
            PendingAdjudications::read_from_str_for_test(toml).expect("legacy-shape TOML must parse");

        assert_eq!(parsed.entries.len(), 1, "must have exactly one entry");
        let entry = &parsed.entries[0];
        assert_eq!(entry.engine, DecisionEngine::Deterministic);
        assert!(entry.judgment.is_none());
        assert_eq!(entry.session_id, "legacy-session-1");
    }
```

This introduces a test-only helper `PendingAdjudications::read_from_str_for_test` (since the production `read()` takes a `&Path`, and this test has no file on disk); add it as a `#[cfg(test)]`-gated associated function right after `read()` in the `impl PendingAdjudications` block:

```rust
    #[cfg(test)]
    fn read_from_str_for_test(s: &str) -> Result<Self, AdjudicationError> {
        let parsed: PendingAdjudications =
            toml::from_str(s).map_err(|e| AdjudicationError::Toml(e.to_string()))?;
        if parsed.schema_version != Self::CURRENT_SCHEMA_VERSION {
            return Err(AdjudicationError::UnsupportedSchemaVersion {
                found: Some(parsed.schema_version),
                supported: Self::CURRENT_SCHEMA_VERSION,
            });
        }
        Ok(parsed)
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(read_refuses_wrong_schema_version) or test(legacy_pending_toml_without_engine_defaults_to_deterministic)'
```

Expected: FAIL to compile (`AdjudicationError::UnsupportedSchemaVersion` doesn't exist; `CURRENT_SCHEMA_VERSION` const doesn't exist; `read_from_str_for_test` doesn't exist).

- [ ] **Step 3: Implement**

Add the const and error variant. In `crates/talkbank-transform/src/adjudication.rs`, add to the `AdjudicationError` enum (after `Toml`, current line 80):

```rust
    /// The file's `schema_version` is missing or not equal to
    /// [`PendingAdjudications::CURRENT_SCHEMA_VERSION`]. Refuses rather
    /// than risk silently reinterpreting an old-shape file under the new
    /// types.
    #[error("unsupported pending-adjudications schema_version {found:?}; this binary supports {supported}")]
    UnsupportedSchemaVersion {
        /// The schema version as read from the file.
        found: Option<u32>,
        /// The schema version this binary supports.
        supported: u32,
    },
```

Add the const and update `read()`/`read_or_default()` in `impl PendingAdjudications`:

```rust
impl PendingAdjudications {
    /// Current schema version supported by this binary. Bumped from 1 to
    /// 2 for the `adult_roles` map (was `inserted_role`, a single field).
    /// Readers refuse any other value.
    pub const CURRENT_SCHEMA_VERSION: u32 = 2;

    /// Read a pending-adjudications file from disk. Refuses unknown
    /// schema versions; uses `default()` only when the caller supplies a
    /// path that does not exist via [`Self::read_or_default`].
    pub fn read(path: &Path) -> Result<Self, AdjudicationError> {
        let bytes = fs::read_to_string(path).map_err(|e| AdjudicationError::FileIo {
            path: path.to_path_buf(),
            source: e,
        })?;
        let parsed: PendingAdjudications =
            toml::from_str(&bytes).map_err(|e| AdjudicationError::Toml(e.to_string()))?;
        if parsed.schema_version != Self::CURRENT_SCHEMA_VERSION {
            return Err(AdjudicationError::UnsupportedSchemaVersion {
                found: Some(parsed.schema_version),
                supported: Self::CURRENT_SCHEMA_VERSION,
            });
        }
        Ok(parsed)
    }

    /// Read the file or return an empty default if the path doesn't
    /// exist. Matches the `OverrideFile::read_or_default` ergonomics so
    /// first-run batches don't need a pre-created file.
    pub fn read_or_default(path: &Path) -> Result<Self, AdjudicationError> {
        match fs::read_to_string(path) {
            Ok(s) => {
                let parsed: PendingAdjudications =
                    toml::from_str(&s).map_err(|e| AdjudicationError::Toml(e.to_string()))?;
                if parsed.schema_version != Self::CURRENT_SCHEMA_VERSION {
                    return Err(AdjudicationError::UnsupportedSchemaVersion {
                        found: Some(parsed.schema_version),
                        supported: Self::CURRENT_SCHEMA_VERSION,
                    });
                }
                Ok(parsed)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(AdjudicationError::FileIo {
                path: path.to_path_buf(),
                source: e,
            }),
        }
    }
```

(rest of `impl PendingAdjudications` unchanged) and add the `read_from_str_for_test` helper from Step 1 right after `read()`.

Update `Default for PendingAdjudications` (current lines 241-248) to use the const instead of the literal `1`:

```rust
impl Default for PendingAdjudications {
    fn default() -> Self {
        Self {
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}
```

Update the exit-code mapping in `crates/chatter/src/commands/adjudicate.rs`'s `exit_with_error` (current lines 359-369) to route the new variant to the same exit code as the existing file-level errors:

```rust
    let code = match e {
        AdjudicationError::FileIo { .. }
        | AdjudicationError::TerminalIo(_)
        | AdjudicationError::Toml(_)
        | AdjudicationError::UnsupportedSchemaVersion { .. } => EXIT_INPUT_ERROR,
        AdjudicationError::PrompterFailed { .. }
        | AdjudicationError::DecisionKindMismatch { .. } => EXIT_PRECONDITION,
    };
```

Update the other test in this module that hardcodes `schema_version: 1` (the `round_trip_pending_adjudications_contains_llm_engine_and_reasoning` test in `judgment/consume.rs`, and `deterministic_entry_omits_engine_field_on_serialize` in `adjudication.rs`) to use `PendingAdjudications::CURRENT_SCHEMA_VERSION` instead of a literal, so they track the const:

```bash
grep -rn "schema_version: 1" crates/talkbank-transform/src/
```

Replace each match's `schema_version: 1` with `schema_version: PendingAdjudications::CURRENT_SCHEMA_VERSION`.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(read_refuses_wrong_schema_version) or test(legacy_pending_toml_without_engine_defaults_to_deterministic) or test(deterministic_entry_omits_engine_field_on_serialize)'
cargo nextest run -p talkbank-transform --lib -E 'package(talkbank-transform)'
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/adjudication.rs crates/talkbank-transform/src/speaker_id/judgment/consume.rs crates/chatter/src/commands/adjudicate.rs
git commit -m "feat(adjudication): enforce PendingAdjudications schema_version, bump to 2

PendingAdjudications::read() previously had no schema_version check at
all (unlike OverrideFile). Adds the same strict-refuse pattern, and
bumps to 2 for the adult_roles map shape from the prior two commits."
```

#### Part 4: `MergeOverride`/`to_mapping_spec()` → `adult_roles` map, bump `CURRENT_SCHEMA_VERSION` to 2

**Files:**
- Modify: `crates/talkbank-transform/src/speaker_id/override_file.rs`

**Interfaces:**
- Consumes: `InsertedRoleSpec` (Task 7), `SpeakerAssignment::Rename { code, role, specific_role }` (Task 6).
- Produces: `MergeOverride.adult_roles: BTreeMap<String, InsertedRoleSpec>` (was `inserted_role`); `MergeOverride::auto_decision`/`operator_decision` take `adult_roles` instead of a single `inserted_role`; `to_mapping_spec()` looks up each `Rename`-action speaker's own entry in the map instead of applying one shared value; `OverrideFile::CURRENT_SCHEMA_VERSION` bumps 1 → 2.

- [ ] **Step 1: Write the failing test**

Add to `override_file.rs`'s test module (from Task 7):

```rust
    /// A `MergeOverride` with two Rename actions, each looked up in its
    /// own `adult_roles` entry, must produce a `MappingSpec` where each
    /// speaker gets its OWN code/role/specific_role, not one shared value.
    #[test]
    fn to_mapping_spec_applies_distinct_roles_per_speaker() {
        let mut mapping = BTreeMap::new();
        mapping.insert("PAR0".to_string(), SpeakerAction::Rename);
        mapping.insert("PAR1".to_string(), SpeakerAction::Rename);
        let mut adult_roles = BTreeMap::new();
        adult_roles.insert(
            "PAR0".to_string(),
            InsertedRoleSpec {
                code: "INV".to_string(),
                tag: "Investigator".to_string(),
                specific_role: None,
            },
        );
        adult_roles.insert(
            "PAR1".to_string(),
            InsertedRoleSpec {
                code: "FAT".to_string(),
                tag: "Father".to_string(),
                specific_role: None,
            },
        );
        let entry = MergeOverride {
            mode: OverrideMode::Explicit,
            adult_roles,
            mapping,
            scores: BTreeMap::new(),
            margin: None,
            operator: "test".to_string(),
            decided_at: Utc::now(),
            note: None,
            flags: Vec::new(),
            engine: DecisionEngine::Deterministic,
            judgment: None,
        };

        let spec = entry.to_mapping_spec();
        match spec.get(&SpeakerCode::new("PAR0")) {
            Some(SpeakerAssignment::Rename { code, role, .. }) => {
                assert_eq!(code.as_str(), "INV");
                assert_eq!(role.as_str(), "Investigator");
            }
            other => panic!("expected Rename for PAR0; got {other:?}"),
        }
        match spec.get(&SpeakerCode::new("PAR1")) {
            Some(SpeakerAssignment::Rename { code, role, .. }) => {
                assert_eq!(code.as_str(), "FAT");
                assert_eq!(role.as_str(), "Father");
            }
            other => panic!("expected Rename for PAR1; got {other:?}"),
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(to_mapping_spec_applies_distinct_roles_per_speaker)'
```

Expected: FAIL to compile (`MergeOverride.adult_roles` doesn't exist yet; `to_mapping_spec()` still applies one shared role).

- [ ] **Step 3: Implement**

Bump the schema constant (current line 32):

```rust
pub const CURRENT_SCHEMA_VERSION: u32 = 2;
```

Change `MergeOverride`'s field (current lines 208-210):

```rust
    /// The CHAT identity assigned to every speaker whose `mapping`
    /// action is `Rename`.
    pub inserted_role: InsertedRoleSpec,
```

to:

```rust
    /// Per-donor-speaker-code role assignment, for every speaker whose
    /// `mapping` action is `Rename`.
    pub adult_roles: BTreeMap<String, InsertedRoleSpec>,
```

Update `to_mapping_spec()` (current lines 163-180):

```rust
    /// Translate this entry's recorded decision into the in-memory
    /// [`MappingSpec`] consumed by `apply_mapping`. Every recorded
    /// `Rename` action looks up its OWN entry in `adult_roles`; every
    /// `Drop` becomes `SpeakerAssignment::Drop`.
    pub fn to_mapping_spec(&self) -> MappingSpec {
        self.mapping
            .iter()
            .map(|(spk, action)| {
                let speaker = SpeakerCode::new(spk);
                let assignment = match action {
                    SpeakerAction::Drop => SpeakerAssignment::Drop,
                    SpeakerAction::Rename => {
                        let role_spec = self
                            .adult_roles
                            .get(spk)
                            .unwrap_or_else(|| panic!(
                                "MergeOverride invariant violated: speaker {spk} has Rename \
                                 action but no adult_roles entry"
                            ));
                        SpeakerAssignment::Rename {
                            code: SpeakerCode::new(&role_spec.code),
                            role: ParticipantRole::new(&role_spec.tag),
                            specific_role: role_spec
                                .specific_role
                                .as_deref()
                                .map(talkbank_model::ParticipantName::new),
                        }
                    }
                };
                (speaker, assignment)
            })
            .collect()
    }
```

(This uses `panic!` for the invariant-violation case, matching this crate's existing pattern in `disambiguate_adult_roles`'s `unwrap_or_else`, since it represents a data corruption bug rather than a recoverable user error, no CLI ever constructs a `MergeOverride` with a `Rename` action and no matching `adult_roles` entry through the sanctioned writer paths this plan updates.)

Update the two constructors (`auto_decision`, current lines 108-128, and `operator_decision`, current lines 134-156) to take `adult_roles: BTreeMap<String, InsertedRoleSpec>` instead of `inserted_role: InsertedRoleSpec`:

```rust
    pub fn auto_decision(
        mapping: &MappingSpec,
        report: &DonorMatchReport,
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        operator: String,
        decided_at: DateTime<Utc>,
    ) -> Self {
        Self {
            mode: OverrideMode::Auto,
            adult_roles,
            mapping: mapping_to_serializable(mapping),
            scores: report.scores_to_serializable(),
            margin: report.margin_to_serializable(),
            operator,
            decided_at,
            note: None,
            flags: Vec::new(),
            engine: DecisionEngine::Deterministic,
            judgment: None,
        }
    }

    pub fn operator_decision(
        mapping: BTreeMap<String, SpeakerAction>,
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        scores: BTreeMap<String, f64>,
        margin: Option<f64>,
        operator: String,
        decided_at: DateTime<Utc>,
        note: Option<String>,
    ) -> Self {
        Self {
            mode: OverrideMode::Explicit,
            adult_roles,
            mapping,
            scores,
            margin,
            operator,
            decided_at,
            note,
            flags: Vec::new(),
            engine: DecisionEngine::Deterministic,
            judgment: None,
        }
    }
```

- [ ] **Step 4: Run test to verify it passes; confirm remaining call sites are left broken for later tasks**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(to_mapping_spec_applies_distinct_roles_per_speaker)'
cargo check -p talkbank-transform -p chatter --all-targets 2>&1 | grep -c error
```

Expected: the target test PASSES; `cargo check` still reports errors (call sites in `adjudication.rs`'s `apply_decision`, `writes.rs`, `adjudicate.rs`), fixed in Tasks 12-13. Confirm the error count only comes from those known files:

```bash
cargo check -p talkbank-transform -p chatter --all-targets 2>&1 | grep "error\[" | sort -u
```

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/speaker_id/override_file.rs
git commit -m "refactor(speaker-id): MergeOverride.inserted_role -> adult_roles map; bump schema to 2

Intermediate commit within this task; Part 5 onward continues the same field change."
```

#### Part 5: `OperatorDecision`/`ScriptedChoice` → `adult_roles`; fix `apply_decision`

**Files:**
- Modify: `crates/talkbank-transform/src/adjudication.rs` (`OperatorDecision`, lines 304-347; `ScriptedChoice` + its `From` impl, lines 429-476; `apply_decision`, lines 561-661)

**Interfaces:**
- Consumes: `MergeOverride::operator_decision(mapping, adult_roles, ...)` from Part 4.
- Produces: `OperatorDecision::OverrideMapping { mapping, adult_roles, note }`, `OperatorDecision::ChooseRole { adult_roles, note }` (both were `inserted_role: InsertedRoleSpec`, singular); same shape change on `ScriptedChoice`.

- [ ] **Step 1: Write the failing test**

Add to `adjudication.rs`'s test module:

```rust
    /// apply_decision on AcceptSuggested must carry the pending entry's
    /// FULL adult_roles map into the resulting MergeOverride, not a
    /// single value, so a multi-adult suggestion survives acceptance.
    #[test]
    fn apply_decision_accept_suggested_preserves_full_adult_roles_map() {
        let mut adult_roles = BTreeMap::new();
        adult_roles.insert(
            "PAR0".to_string(),
            InsertedRoleSpec { code: "INV".to_string(), tag: "Investigator".to_string(), specific_role: None },
        );
        adult_roles.insert(
            "PAR1".to_string(),
            InsertedRoleSpec { code: "FAT".to_string(), tag: "Father".to_string(), specific_role: None },
        );
        let mut mapping = BTreeMap::new();
        mapping.insert("PAR0".to_string(), SpeakerAction::Rename);
        mapping.insert("PAR1".to_string(), SpeakerAction::Rename);
        let entry = PendingEntry {
            session_id: "sess-accept-multi".to_string(),
            created_at: Utc::now(),
            data: PendingKindData::SpeakerIdLowConfidence {
                suggested: SuggestedSpeakerIdMapping { mapping, adult_roles },
            },
            scores: BTreeMap::new(),
            margin: None,
            threshold_used: None,
            engine: DecisionEngine::Llm,
            judgment: None,
        };
        let mut overrides = OverrideFile::default();
        apply_decision(
            &entry,
            &OperatorDecision::AcceptSuggested { note: None },
            "tester",
            &mut overrides,
        )
        .expect("apply_decision must succeed");

        let recorded = overrides.get("sess-accept-multi").expect("entry must be recorded");
        assert_eq!(recorded.adult_roles.len(), 2, "both adults must survive into the override entry");
        assert_eq!(recorded.adult_roles.get("PAR0").unwrap().code, "INV");
        assert_eq!(recorded.adult_roles.get("PAR1").unwrap().code, "FAT");
    }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(apply_decision_accept_suggested_preserves_full_adult_roles_map)'
```

Expected: FAIL to compile (`apply_decision` is private to `adjudication.rs`, confirm the test module is inside the same file with `use super::*` so it has access; `OperatorDecision`/the match arms still reference `inserted_role`).

- [ ] **Step 3: Implement**

Change `OperatorDecision`'s two variants (current lines 319-346):

```rust
    OverrideMapping {
        /// Operator-supplied per-speaker actions. Must cover every
        /// speaker the merge stage will see in the donor file (same
        /// rule as the algorithm's mapping).
        mapping: BTreeMap<String, SpeakerAction>,
        /// Operator-supplied per-speaker role assignments. May differ
        /// from the pending entry's suggested roles.
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        /// Operator note explaining the override. Strongly
        /// recommended on this path, captures the *why* a future
        /// reader would want.
        note: Option<String>,
    },
    /// Parent-role-lookup decision: operator picks a role for an
    /// already-identified parent speaker. The mapping comes from the
    /// pending entry's `speaker_mapping` field; only `adult_roles` is
    /// operator-supplied.
    ChooseRole {
        /// Operator-supplied role assignment, keyed by the single donor
        /// speaker code this pending entry names (`ParentRoleLookup`'s
        /// `donor_speaker` field).
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        /// Operator note explaining the choice.
        note: Option<String>,
    },
```

Change `ScriptedChoice`'s matching variants and `From` impl (current lines 439-476) the same way:

```rust
    OverrideMapping {
        mapping: BTreeMap<String, SpeakerAction>,
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    ChooseRole {
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
```

```rust
impl From<ScriptedChoice> for OperatorDecision {
    fn from(choice: ScriptedChoice) -> Self {
        match choice {
            ScriptedChoice::AcceptSuggested { note } => OperatorDecision::AcceptSuggested { note },
            ScriptedChoice::OverrideMapping {
                mapping,
                adult_roles,
                note,
            } => OperatorDecision::OverrideMapping {
                mapping,
                adult_roles,
                note,
            },
            ScriptedChoice::ChooseRole { adult_roles, note } => {
                OperatorDecision::ChooseRole { adult_roles, note }
            }
        }
    }
}
```

Update `apply_decision`'s five match arms (current lines 568-658), replacing every `suggested.inserted_role.clone()` / `inserted_role.clone()` argument with `suggested.adult_roles.clone()` / `adult_roles.clone()`:

```rust
    let merge_override = match (&entry.data, decision) {
        (
            PendingKindData::SpeakerIdLowConfidence { suggested },
            OperatorDecision::AcceptSuggested { note },
        ) => MergeOverride::operator_decision(
            suggested.mapping.clone(),
            suggested.adult_roles.clone(),
            entry.scores.clone(),
            entry.margin,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (
            PendingKindData::SpeakerIdLowConfidence { .. },
            OperatorDecision::OverrideMapping {
                mapping,
                adult_roles,
                note,
            },
        ) => MergeOverride::operator_decision(
            mapping.clone(),
            adult_roles.clone(),
            entry.scores.clone(),
            entry.margin,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (
            PendingKindData::ParentRoleLookup {
                speaker_mapping, ..
            },
            OperatorDecision::ChooseRole { adult_roles, note },
        ) => MergeOverride::operator_decision(
            speaker_mapping.clone(),
            adult_roles.clone(),
            BTreeMap::new(),
            None,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (
            PendingKindData::SanityScanMisclassification { suggested, .. },
            OperatorDecision::AcceptSuggested { note },
        ) => MergeOverride::operator_decision(
            suggested.mapping.clone(),
            suggested.adult_roles.clone(),
            entry.scores.clone(),
            entry.margin,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (
            PendingKindData::SanityScanMisclassification { .. },
            OperatorDecision::OverrideMapping {
                mapping,
                adult_roles,
                note,
            },
        ) => MergeOverride::operator_decision(
            mapping.clone(),
            adult_roles.clone(),
            entry.scores.clone(),
            entry.margin,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (kind_data, decision) => {
            return Err(AdjudicationError::DecisionKindMismatch {
                session_id: entry.session_id.clone(),
                pending: kind_data.kind(),
                decision: format!("{decision:?}"),
            });
        }
    };
```

Make `apply_decision` visible to the test module (it's currently a private `fn`, already reachable from `#[cfg(test)] mod tests { use super::*; ... }` in the same file, since Rust privacy allows a child module to see its parent's private items, so no visibility change is needed, confirm this compiles rather than assuming).

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p talkbank-transform --lib -E 'test(apply_decision_accept_suggested_preserves_full_adult_roles_map)'
cargo check -p talkbank-transform -p chatter --all-targets 2>&1 | grep "error\[" | sort -u
```

Expected: target test PASSES; remaining errors confined to `writes.rs` and `adjudicate.rs` (Part 6).

- [ ] **Step 5: Commit**

```bash
git add crates/talkbank-transform/src/adjudication.rs
git commit -m "refactor(adjudication): OperatorDecision/ScriptedChoice/apply_decision carry adult_roles map"
```

#### Part 6: Fix `writes.rs` and `adjudicate.rs` call sites

**Files:**
- Modify: `crates/chatter/src/commands/speaker_id/writes.rs`
- Modify: `crates/chatter/src/commands/adjudicate.rs`

**Interfaces:**
- Consumes: everything from Tasks 8-12.
- Produces: a compiling `chatter` binary; no new public interface, this is pure call-site repair plus the rendering/parsing upgrade for multi-role display and input.

- [ ] **Step 1: Fix `writes.rs`'s two call sites**

`write_override_entry` (current lines 28-54): change the `MergeOverride::auto_decision` call to wrap the single role in a one-entry map (reference mode only ever has one non-anchor role by construction, this is not a behavior change for that path):

```rust
    let entry = MergeOverride::auto_decision(
        &outcome.mapping,
        &outcome.report,
        {
            let mut m = std::collections::BTreeMap::new();
            for (spk, action) in mapping_to_actions_for_test_visibility(&outcome.mapping) {
                if action == talkbank_transform::speaker_id::SpeakerAction::Rename {
                    m.insert(
                        spk,
                        InsertedRoleSpec::new(&outcome.inserted_code, &outcome.inserted_role_tag),
                    );
                }
            }
            m
        },
        operator,
        Utc::now(),
    );
```

This references a helper `mapping_to_actions_for_test_visibility` that doesn't exist and shouldn't, simplify instead: `outcome.mapping` is already a `MappingSpec` (`HashMap<SpeakerCode, SpeakerAssignment>`), so the every-Rename-speaker-gets-the-same-role loop is direct:

```rust
    let mut adult_roles = std::collections::BTreeMap::new();
    for (spk, action) in outcome.mapping.iter() {
        if matches!(action, talkbank_transform::speaker_id::SpeakerAssignment::Rename { .. }) {
            adult_roles.insert(
                spk.as_str().to_string(),
                InsertedRoleSpec::new(&outcome.inserted_code, &outcome.inserted_role_tag),
            );
        }
    }
    let entry = MergeOverride::auto_decision(
        &outcome.mapping,
        &outcome.report,
        adult_roles,
        operator,
        Utc::now(),
    );
```

(Place the `adult_roles` construction immediately before the `MergeOverride::auto_decision` call, replacing the old three-argument call. Check `ReferenceModeOutcome`'s `mapping` field type first, confirm it's `MappingSpec`, via `grep -n "struct ReferenceModeOutcome" -A 15 crates/chatter/src/commands/speaker_id/modes.rs`, before finalizing the iteration; adjust the exact accessor if the field is named differently.)

`write_pending_entry` (current lines 63-97): change the `SuggestedSpeakerIdMapping` construction:

```rust
    let mut suggested_mapping: std::collections::BTreeMap<String, SpeakerAction> =
        std::collections::BTreeMap::new();
    suggested_mapping.insert(report.winner.as_str().to_string(), SpeakerAction::Drop);
    let mut adult_roles: std::collections::BTreeMap<String, InsertedRoleSpec> =
        std::collections::BTreeMap::new();
    for spk in donor_chat.unique_utterance_speakers() {
        if spk != report.winner {
            suggested_mapping.insert(spk.as_str().to_string(), SpeakerAction::Rename);
            adult_roles.insert(
                spk.as_str().to_string(),
                InsertedRoleSpec::new(inserted_code, inserted_role_tag),
            );
        }
    }
    let entry = PendingEntry {
        session_id: session_id.clone(),
        created_at: Utc::now(),
        data: PendingKindData::SpeakerIdLowConfidence {
            suggested: SuggestedSpeakerIdMapping {
                mapping: suggested_mapping,
                adult_roles,
            },
        },
        scores: report.scores_to_serializable(),
        margin: report.margin_to_serializable(),
        threshold_used: Some(threshold.0),
        engine: talkbank_transform::speaker_id::DecisionEngine::Deterministic,
        judgment: None,
    };
```

- [ ] **Step 2: Fix `adjudicate.rs`'s rendering and parsing**

`TerminalPrompter::ask`'s two `inserted_role` rendering blocks (current lines 137-141 and 163-167) become a per-speaker table:

```rust
                writeln!(out, "Suggested roles:")?;
                for (spk, role) in &suggested.adult_roles {
                    match &role.specific_role {
                        Some(label) => writeln!(out, "  {spk} -> {} ({label}, {})", role.code, role.tag)?,
                        None => writeln!(out, "  {spk} -> {} ({})", role.code, role.tag)?,
                    }
                }
```

(apply this replacement in both the `SpeakerIdLowConfidence` and `SanityScanMisclassification` match arms, current lines ~137-141 and ~163-167.)

`parse_operator_response`'s `"choose"` arm and `parse_override_mapping` currently parse exactly one `CODE TAG` pair. Extend the syntax to accept one or more `SPK:CODE:TAG` groups (colon-separated, space-delimited between groups), which is unambiguous and consistent with the existing `SPK=action` token shape used later in the same command line:

```rust
        Some("choose") => match tokens.as_slice() {
            [_, rest @ ..] if !rest.is_empty() => {
                let (adult_roles, note_start) = parse_role_groups(rest, entry)?;
                let note = if note_start >= rest.len() {
                    None
                } else {
                    Some(rest[note_start..].join(" "))
                };
                Ok(OperatorDecision::ChooseRole { adult_roles, note })
            }
            _ => Err(AdjudicationError::PrompterFailed {
                session_id: entry.session_id.clone(),
                detail: "choose decision requires at least one SPK:CODE:TAG group (e.g., \"choose PAR1:MOT:Mother\")".to_string(),
            }),
        },
```

Add the shared helper `parse_role_groups` (used by both `choose` and `override`) right before `parse_override_mapping`:

```rust
/// Parse a run of leading `SPK:CODE:TAG` tokens into an `adult_roles`
/// map, stopping at the first token that doesn't match that shape (the
/// caller treats everything from there on as the optional trailing
/// note). Returns the map and the index into `tokens` where parsing
/// stopped.
fn parse_role_groups(
    tokens: &[&str],
    entry: &PendingEntry,
) -> Result<(BTreeMap<String, InsertedRoleSpec>, usize), AdjudicationError> {
    let mut adult_roles: BTreeMap<String, InsertedRoleSpec> = BTreeMap::new();
    let mut i = 0;
    while i < tokens.len() {
        let parts: Vec<&str> = tokens[i].splitn(3, ':').collect();
        match parts.as_slice() {
            [spk, code, tag] if !spk.is_empty() && !code.is_empty() && !tag.is_empty() => {
                adult_roles.insert(
                    (*spk).to_string(),
                    InsertedRoleSpec {
                        code: (*code).to_string(),
                        tag: (*tag).to_string(),
                        specific_role: None,
                    },
                );
                i += 1;
            }
            _ => break,
        }
    }
    if adult_roles.is_empty() {
        return Err(AdjudicationError::PrompterFailed {
            session_id: entry.session_id.clone(),
            detail: "expected at least one SPK:CODE:TAG group (e.g., PAR1:INV:Investigator)".to_string(),
        });
    }
    Ok((adult_roles, i))
}
```

Update `parse_override_mapping` (current lines 256-322) to use `parse_role_groups` instead of the old two-token `CODE TAG` parse:

```rust
fn parse_override_mapping(
    tokens: &[&str],
    entry: &PendingEntry,
) -> Result<OperatorDecision, AdjudicationError> {
    let [_, rest @ ..] = tokens else {
        return Err(AdjudicationError::PrompterFailed {
            session_id: entry.session_id.clone(),
            detail: "override decision requires at least one SPK:CODE:TAG group (e.g., \"override PAR1:INV:Investigator PAR0=drop\")".to_string(),
        });
    };
    let (adult_roles, role_end) = parse_role_groups(rest, entry)?;
    let assignment_and_note = &rest[role_end..];

    let mut mapping: BTreeMap<String, SpeakerAction> = BTreeMap::new();
    let mut split_idx = assignment_and_note.len();
    for (i, token) in assignment_and_note.iter().enumerate() {
        match parse_speaker_assignment(token) {
            AssignmentParse::Valid(spk, action) => {
                mapping.insert(spk, action);
            }
            AssignmentParse::Malformed => {
                return Err(AdjudicationError::PrompterFailed {
                    session_id: entry.session_id.clone(),
                    detail: format!("malformed assignment {token:?}; expected SPK=rename or SPK=drop"),
                });
            }
            AssignmentParse::NotAnAssignment => {
                split_idx = i;
                break;
            }
        }
    }
    if mapping.is_empty() {
        return Err(AdjudicationError::PrompterFailed {
            session_id: entry.session_id.clone(),
            detail: "override decision requires at least one SPK=action assignment (e.g., PAR0=rename)".to_string(),
        });
    }
    let note_words = &assignment_and_note[split_idx..];
    let note = if note_words.is_empty() {
        None
    } else {
        Some(note_words.join(" "))
    };

    Ok(OperatorDecision::OverrideMapping {
        mapping,
        adult_roles,
        note,
    })
}
```

Update the `prompt_hint` strings in `parse_operator_response`'s caller (current lines 181-192) to describe the new syntax:

```rust
        let prompt_hint = match &entry.data {
            PendingKindData::SpeakerIdLowConfidence { .. } => {
                "Decision [accept | override SPK:CODE:TAG [SPK:CODE:TAG ...] SPK=action [SPK=action ...] [note...]]: "
            }
            PendingKindData::ParentRoleLookup { .. } => {
                "Decision [choose SPK:CODE:TAG [SPK:CODE:TAG ...] [note...]]: "
            }
            PendingKindData::SanityScanMisclassification { .. } => {
                "Decision [accept | override SPK:CODE:TAG [SPK:CODE:TAG ...] SPK=action [SPK=action ...] [note...]]: "
            }
        };
```

- [ ] **Step 3: Build and run the full crate's tests**

```bash
cargo check -p talkbank-transform -p chatter --all-targets
```

Expected: clean build, zero errors. If any remain, they are call sites this plan missed, grep for `inserted_role` crate-wide to confirm none remain outside intentionally-kept doc comments:

```bash
grep -rn "\.inserted_role\b\|inserted_role:" crates/ --include="*.rs" | grep -v "adult_roles\|target/"
```

Fix any surviving hit before proceeding.

- [ ] **Step 4: Run the full speaker-id/adjudication test suite**

```bash
cargo nextest run -p talkbank-transform --lib -E 'package(talkbank-transform)'
cargo nextest run -p chatter --lib -E 'package(chatter)'
```

Expected: all green.

- [ ] **Step 3b: Full-crate integration check (this is the task's real green gate)**

```bash
cargo check -p talkbank-transform -p chatter --all-targets
cargo nextest run -p talkbank-transform --lib -E 'package(talkbank-transform)'
cargo nextest run -p chatter --lib -E 'package(chatter)'
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Expected: all green. This is the point where Parts 1-5's intermediate red states finally resolve; only after this passes is Task 8 considered done and ready for its task review.

- [ ] **Step 5: Commit**

```bash
git add crates/chatter/src/commands/speaker_id/writes.rs crates/chatter/src/commands/adjudicate.rs
git commit -m "feat(adjudicate): render and parse per-speaker role tables (SPK:CODE:TAG syntax)

Extends the interactive 'choose'/'override' commands from a single
CODE TAG pair to one-or-more SPK:CODE:TAG groups, so an operator can
correct a multi-adult suggestion without re-specifying every speaker.
This CLI syntax choice was not reviewed with Franklin at the spec
level; flag for adjustment if a different shape is preferred."
```

### Task 9: L3/L4 subprocess and adjudication-flow tests for the multi-adult path

**Files:**
- Test: `crates/chatter/tests/merge_tests.rs` or `crates/chatter/tests/adjudication_tests.rs` (check which exists and covers `chatter adjudicate`; both are listed in the repo per earlier grep)

- [ ] **Step 1: Check existing L3/L4 coverage for `chatter adjudicate`**

```bash
grep -n "fn test_\|fn " crates/chatter/tests/adjudication_tests.rs | head -30
```

Read the matched test names and the file's fixture-construction helper (likely a `run_chatter(&["adjudicate", ...])` subprocess helper) before writing the new test, to match its exact convention rather than guessing.

- [ ] **Step 2: Write a failing top-level test**

Add a subprocess-level test (using whatever harness helper the existing file uses) that: writes a `pending.toml` with `schema_version = 2` and two `Adult` verdicts with distinct roles in one entry's `adult_roles` map, runs `chatter adjudicate --scripted <accept-suggested.toml>`, and asserts the resulting `override.toml` contains both roles. Model this directly on whatever the closest existing test in the file already does for the single-adult `AcceptSuggested` case, adapting only the fixture content, since I have not read this file's exact helper signatures. **This step requires reading `adjudication_tests.rs` in full before writing the fixture** (Step 1 grep names the functions; open the file, copy its harness pattern exactly).

- [ ] **Step 3: Run the new test, confirm it passes (or fails correctly first if a genuine red step is possible given the harness)**

```bash
cargo nextest run -p chatter --test adjudication_tests -E 'test(<new_test_name>)'
```

- [ ] **Step 4: Commit**

```bash
git add crates/chatter/tests/adjudication_tests.rs
git commit -m "test(adjudicate): subprocess coverage for multi-adult accept-suggested flow"
```

### Task 10: Book docs, regenerate the live `pending.toml`, final Phase 3 regression gate

**Files:**
- Modify: `book/src/chatter/integrating/merge-overrides.md` (lines 94, 151, 223, 243, 263, 283, 306, 360, per the earlier grep)
- Modify: `book/src/chatter/user-guide/speaker-id.md` (lines 323, 342-343)
- Modify: `book/src/architecture/adjudication-workflow.md` (lines 79-80, 177, 288, 364)

- [ ] **Step 1: Update `merge-overrides.md`**

Read the file in full first (it's the authoritative on-disk format spec, referenced from `override_file.rs`'s own doc comment, so accuracy here matters more than the other two). Replace every `inserted_role = { code = "...", tag = "..." }` example with the `adult_roles` map form:

```toml
[entries.SESSION_ID.adult_roles]
PAR1 = { code = "INV", tag = "Investigator" }
```

Update the field-description table row (current line 94) from describing a single inline table to describing a map of donor-code to inline table, and update the per-entry field-order line (current line 223) to list `adult_roles` instead of `inserted_role`. Bump the schema-version examples from `1` to `2` wherever a full document example is shown. Update `**Last modified:**` via `date '+%Y-%m-%d %H:%M %Z'`.

- [ ] **Step 2: Update `speaker-id.md`**

Same `inserted_role` → `adult_roles` swap at the two matched locations (lines 323, 342-343). Update `**Last modified:**`.

- [ ] **Step 3: Update `adjudication-workflow.md`**

Same swap at lines 79-80, 177, 288; line 364 (`inserted_role: InsertedRole,` inside a Rust code block excerpt) update to match the new field. Update `**Last modified:**`.

- [ ] **Step 4: Note the schema bump for any downstream consumer**

Any existing `pending.toml` written by a pre-Task-8-Part-3 binary (`schema_version = 1`) will now be refused on read. Regenerating a specific corpus's pending set (re-running whatever `chatter batch`/judgment invocation produced it) is downstream operational follow-up outside this plan's scope, tracked wherever that corpus's own workflow is tracked, not here. This repo's own tests (Task 8 Parts 2-6, and Task 9) are the only "pending.toml" this plan needs to produce or regenerate.

- [ ] **Step 5: Full Phase 3 regression gate**

```bash
cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
cargo nextest run -p talkbank-parser-tests --test roundtrip_reference_corpus
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo nextest run -p talkbank-transform -p chatter --lib
just book   # or: cd book && mdbook build && mdbook-mermaid install .
```

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add book/src/chatter/integrating/merge-overrides.md book/src/chatter/user-guide/speaker-id.md book/src/architecture/adjudication-workflow.md
git commit -m "docs(merge): update override-file/adjudication-workflow docs for adult_roles map"
```

Phase 3 complete. All three spec gaps are now implemented, tested, and documented.

---

## Self-Review Notes (completed during plan authoring, not a separate pass)

- **Spec coverage:** Gap 1 → Tasks 1-3. Gap 2 → Tasks 4-5. Gap 3 → Tasks 6-15 (the spec's five numbered sub-points each map to a task: `SuggestedSpeakerIdMapping`→Task 8 Part 1, `MergeOverride`→Task 8 Part 4, `judgment_to_pending`→Task 8 Part 2, same-role collision→Task 8 Part 2's `disambiguate_adult_roles`, `chatter adjudicate` rendering→Task 8 Part 6).
- **Placeholder scan:** No TBD/TODO. Part 6's `writes.rs` fix and Task 9 both explicitly say "read the file before writing the fixture" rather than inventing one, because I have not read `modes.rs`'s `ReferenceModeOutcome` struct or `adjudication_tests.rs` in full; these are the two honest gaps in an otherwise fully-specified plan, flagged as such rather than papered over with invented code that might not compile.
- **Type consistency:** `adult_roles: BTreeMap<String, InsertedRoleSpec>` is the same shape in every task from 8 onward (`SuggestedSpeakerIdMapping`, `MergeOverride`, `OperatorDecision`, `ScriptedChoice`). `SpeakerAssignment::Rename`'s new `specific_role: Option<ParticipantName>` field (Task 6) and `InsertedRoleSpec`'s new `specific_role: Option<String>` field (Task 7) are deliberately different types (one is the typed CHAT primitive, one is the on-disk string form), consistent with every other field in these two types (`code`/`role` vs `code: String`/`tag: String`).
