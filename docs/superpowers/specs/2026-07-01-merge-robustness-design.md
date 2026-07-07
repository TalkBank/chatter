# Merge Robustness: Dedupe-on-Insert, @Languages Subset Matching, Multi-Adult-Speaker

**Status:** Draft
**Last modified:** 2026-07-01 13:51 EDT

## Context

The IISRP corpus merge (donor ASR re-transcription merged into 805 reference
sessions, `docs/investigations/2026-06-24-iisrp-brian-merge-request-assessment.md`
in the meta-repo) surfaced three genuine gaps in `chatter`'s merge/speaker-id
pipeline, not one-off bugs specific to that corpus. This spec designs the
long-term, principled fix for each. There is no deadline; the goal is the
correct general mechanism, not a patch scoped to IISRP's specific data.

All three gaps were traced to exact file/line locations in this repo (not
inferred from the investigation doc's summary), and all three approaches
below were worked out with Franklin question-by-question before being
written up here.

## Gap 1: Dedupe-on-insert for already-declared participants

**Location:** `crates/talkbank-transform/src/transcript_merge.rs`, the
`inserted_participants`/`inserted_id_lines` collection (current lines
216-247 of `merge_chats`).

**Current behavior.** These two `Vec`s are built by filtering file2's
`@Participants`/`@ID` rows to "speaker code not in `--retain`", with no
check against whether that code is *already declared* in file1. When
file1 vestigially declares a participant (e.g. `INV`, zero utterances,
a placeholder header row) and the donor also uses that code with real
content, the merge emits two `@Participants`/`@ID` declarations for the
same code, invalid CHAT (E549 "speaker declared more than once").
Observed on `CWNS-264-4` and `CWNS-265-4`.

**New behavior.** Before assembling the two `Vec`s, for each donor
participant code not in `--retain`, look up whether that code is
already declared in file1 (via file1's own `@Participants`/`@ID`
headers):

1. **Not declared in file1 at all:** insert normally (today's
   behavior, unchanged).
2. **Declared in file1, zero utterances for that code in file1, AND
   file1's declared role/name metadata matches what the donor's entry
   would carry:** silent dedupe. Skip inserting the donor's duplicate
   `@Participants`/`@ID` rows; keep file1's row as-is; still merge the
   donor's utterances in under that code (utterance insertion is
   already independent of header insertion in this function, so no
   change needed there).
3. **Declared in file1 with nonzero utterances for that code, OR
   file1's metadata conflicts with the donor's entry for that code:**
   refuse. New error variant:

   ```rust,ignore
   #[error(
       "speaker {speaker} is already declared in File 1 (role {file1_role}) \
        and also appears in File 2's non-retained participants (role \
        {donor_role}); this is ambiguous, resolve by adding {speaker} to \
        --retain or renaming it in File 2"
   )]
   ParticipantAlreadyDeclared {
       speaker: SpeakerCode,
       file1_role: ParticipantRole,
       donor_role: ParticipantRole,
   },
   ```

   Symmetric in spirit with the existing `AmbiguousSpeaker` variant,
   which catches the analogous case for *utterance*-bearing speakers;
   this variant catches it for *header-declared* speakers.

**Implementation notes.**

- Need a helper `fn utterance_count_for(chat: &ChatFile, code: &SpeakerCode) -> usize`
  (or reuse/extend an existing utterance-counting helper if one exists
  in `talkbank-model`) to determine "zero utterances" for file1's
  declared code.
- Need a metadata-equality check between file1's `ParticipantEntry`/`IDHeader`
  for the code and what the donor's corresponding entry declares (role
  tag at minimum; name if both are present and non-placeholder).
- This only changes the *header*-insertion path; the utterance-insertion
  loop (current lines 319-335) is untouched, it already inserts
  donor utterances for any code not in `--retain`, independent of
  whether that code's header was newly inserted or already present.

**TDD (write first, red/green).** Top-level test = a real `merge_chats()`
call on two fixture strings reproducing the `CWNS-264-4` shape:

- file1 has `@Participants: ... INV Investigator ...` and zero `*INV:`
  utterances; file2 has real `*INV:` utterances and its own `@Participants`/`@ID`
  rows for `INV`. Assert: merged output has exactly one `INV`
  declaration, contains the donor's `INV` utterances, and validates
  clean via `chatter validate`.
- A metadata-conflict fixture (file1's `INV` role differs from what
  the donor implies) → assert `MergeError::ParticipantAlreadyDeclared`.
- A nonzero-file1-utterance fixture (file1 has real `INV` utterances,
  not retained) → assert refusal (this may already be partially
  covered by `AmbiguousSpeaker`'s existing utterance-based check;
  confirm during implementation whether this is a genuinely new path
  or already caught, and only add the new variant if it isn't).

## Gap 2: `@Languages` subset matching

**Location:** Same file, `MergeError::LanguageMismatch` precondition
(current lines 149-160 of `merge_chats`).

**Current behavior.** `f1_langs != f2_langs`, an exact-equality check
over the full declared `@Languages` code list. Refuses whenever the
two lists differ in any way.

**Empirical grounding (from the IISRP run, not guessed).** All 8
observed mismatches have the same shape: donor (ASR, run in a fixed
monolingual mode) declares a strict subset of reference's (typically
hand-coded, multilingual) set, e.g. donor `[eng]` vs. reference
`[eng, spa]`. The donor never over-claims relative to the reference in
the observed data.

**New behavior.** Replace the equality check with a subset check:
refuse only when file2 (donor) declares a language code **not present**
in file1 (reference)'s set. Exact-match pairs keep working unchanged
(equality is a special case of subset). `MergeError::LanguageMismatch`
keeps its name; message text updates to state the actual failure
direction ("File 2 declares language(s) not present in File 1's
@Languages: {extra}").

```rust,ignore
// Was: if f1_langs != f2_langs { ... }
let donor_extra: Vec<_> = f2_langs.iter().filter(|c| !f1_langs.contains(c)).collect();
if !donor_extra.is_empty() {
    return Err(MergeError::LanguageMismatch { file1: f1_langs, file2: f2_langs });
}
```

(Exact field/method names depend on `LanguageCodes`' actual API;
confirm during implementation whether it already exposes a `contains`/
set-difference method or needs one added.)

**TDD.**

- `file1=[eng,spa], file2=[eng]` → merge now succeeds (today it fails).
  This is the regression test that directly encodes the fix.
- `file1=[eng], file2=[eng,spa]` → still refuses (donor over-claims
  relative to reference; this is the case that should still be
  treated as suspicious).
- Identical sets on both sides → still succeeds (confirms the
  exact-match case, i.e. today's only passing case, keeps passing).

## Gap 3: Multi-adult-speaker representation

**Locations:**
- `crates/talkbank-transform/src/speaker_id/judgment/consume.rs`
  (`judgment_to_pending`, `ConsumeError::MultipleAdults`)
- `crates/talkbank-transform/src/adjudication.rs`
  (`SuggestedSpeakerIdMapping`, `PendingAdjudications::schema_version`)
- `crates/talkbank-transform/src/speaker_id/override_file.rs`
  (`MergeOverride`, `InsertedRoleSpec`, `to_mapping_spec`)
- `crates/chatter/src/commands/adjudicate.rs` (operator-facing rendering)

**Current behavior and why it's mostly plumbing, not new CHAT
semantics.** The LLM's own judgment schema already supports assigning
*different* roles to *different* donor speakers
(`HolisticJudgment.adult_roles: BTreeMap<DonorCode, AdultRole>`,
`crates/talkbank-transform/src/speaker_id/judgment/output.rs`), and the
low-level apply mechanism already supports per-speaker roles too
(`MappingSpec = HashMap<SpeakerCode, SpeakerAssignment>` with
`SpeakerAssignment::Rename { code, role }` per entry, applied fully
independently per speaker in `apply_mapping_chat`,
`crates/talkbank-transform/src/speaker_id/apply.rs`, confirmed by
direct read: no shared/global role assumption there). The actual
bottleneck is narrower: `judgment_to_pending` deliberately refuses
(`ConsumeError::MultipleAdults`, "single-adult first cut... future
extension") the moment it sees a second `Adult` verdict, because
`SuggestedSpeakerIdMapping` (the pending-entry payload,
`adjudication.rs`) and `MergeOverride` (the on-disk override/replay
format, `override_file.rs`) both carry exactly one
`inserted_role: InsertedRoleSpec` field, applied uniformly to every
speaker whose action is `Rename`. There is a real, live passing test
(`multiple_adults_is_error`) pinning today's refusal as intentional;
it will need to change as part of this work, with Franklin's
awareness (this spec *is* that awareness).

**New behavior.**

1. **`SuggestedSpeakerIdMapping`** (`adjudication.rs`): replace
   `inserted_role: InsertedRoleSpec` with
   `adult_roles: BTreeMap<String, InsertedRoleSpec>` (donor speaker
   code → role). `PendingAdjudications::schema_version` bumps `1` → `2`;
   reading a `schema_version: 1` file is a typed refusal (matching this
   codebase's established "strict refuse, no silent migration" policy
   for schema changes), not a silent reinterpretation.
2. **`MergeOverride`** (`override_file.rs`): the same
   `inserted_role: InsertedRoleSpec` → `adult_roles: BTreeMap<String, InsertedRoleSpec>`
   change, since `to_mapping_spec()` currently builds one
   `inserted_code`/`inserted_role` pair outside its per-speaker loop
   and applies it to every `Rename` action (lines 163-180 as read).
   This is the on-disk *replay* format (used when re-running a merge
   with `--override-file`), so it must move in lockstep with the
   pending schema, not as a follow-up.
3. **`judgment_to_pending`** (`consume.rs`): delete
   `ConsumeError::MultipleAdults` and the single-`adult: Option<AdultRole>`
   tracking variable; instead, for every `SpeakerVerdict::Adult` code,
   look up its `adult_roles` entry and insert `(code, role)` into the
   new map directly. `ConsumeError::AdultRoleMissing` and
   `ConsumeError::NoAdultButMergeApplicable` stay as-is (still real
   failure modes, unrelated to the single-vs-multi-adult question).
4. **Same-role collision** (two distinct donor speakers both assigned
   the same `AdultRole` variant, e.g. both `Inv`): auto-disambiguate.
   Verified directly against the CHAT manual
   (https://talkbank.org/0info/manuals/CHAT.html, fetched and read this
   session, not inferred): the manual's own precedent for "two people,
   same role" numbers **both** speaker codes (its `CHI1`/`CHI2` example
   for twins/siblings-as-target-children) and gives each a
   distinguishing specific-role label while keeping the shared standard
   role unchanged (`SI1 First_Sibling Sibling, SI2 Second_Sibling Sibling`).
   Applied here: `INV1`/`INV2` (not a bare `INV` + `INV2`), with
   `@Participants` specific-role labels `First_Investigator`/`Second_Investigator`
   and the shared standard role `Investigator` for both. Needs: a
   collision-detection pass over one judgment's `adult_roles` values
   (group by `AdultRole` variant, any group with 2+ entries collides),
   and a renderer producing the numbered-code + specific-role-label
   pair per the manual's convention.
5. **`chatter adjudicate`** (`adjudicate.rs`): render a per-speaker
   role table (donor code → assigned role) instead of a single role
   line; accept per-speaker overrides at accept time (an operator
   should be able to correct one speaker's role without needing to
   re-specify all of them).

**Backward compatibility, decided explicitly.** The live 345-session
`pending.toml` from the overnight IISRP run uses the old
single-`inserted_role` schema and has had **zero** entries adjudicated
yet (`chatter adjudicate` has not been run against it). Given nothing
would be lost, this is a clean breaking change: bump
`schema_version` to `2`, change both types' shapes directly, and
regenerate `pending.toml` by re-running `chatter batch`'s judgment pass
after the code lands. No migration code, no dual-shape support carried
forward.

**TDD.**

- `judgment_to_pending` fixture: two `Adult` verdicts with two distinct
  `adult_roles` entries (e.g. `Inv` and `Fat`) → produces a
  `PendingEntry` whose `adult_roles` map has both entries, no error
  (today: `ConsumeError::MultipleAdults`).
- Same-role fixture: two `Adult` verdicts both mapped to `Inv` →
  auto-disambiguates to `INV1`/`INV2` with the manual-verified label
  convention (assert on the actual rendered `@Participants` string,
  not just the internal map).
- `MergeOverride::to_mapping_spec()` round-trip test: a multi-adult
  override entry (2+ entries in `adult_roles`) → `apply_mapping_chat`
  produces a `ChatFile` with each donor speaker renamed to its own
  distinct code/role, not all collapsed to one.
- Regression: the existing `multiple_adults_is_error` test is deleted
  (its assertion describes exactly the behavior this spec removes);
  confirm with a targeted look whether any other test depends on the
  single-adult refusal before deleting.

## Open items deliberately left alone (out of scope for this spec)

- The 3 mono-speaker-donor sessions (diarization limitation, no donor
  fix available) are not a merge-tooling gap; nothing in this spec
  addresses them.
- The 288 pending holistic-judgment suggestions themselves (accepting/
  overriding via `chatter adjudicate`) are a human-review workload, not
  a design gap; this spec changes the *shape* `chatter adjudicate`
  will present for multi-adult sessions once they stop being refused,
  but does not itself review any suggestions.
- `max_group_ms` / FA window tuning, the Wave2Vec→Whisper default
  change, and any other unrelated batchalign3 work are untouched.

## Side note: doc-reference drift found in passing

`judgment/output.rs`'s module doc comment cites
`docs/superpowers/specs/2026-06-04-llm-in-the-loop-merge-design.md` as
the design spec for the holistic judgment call's JSON contract. That
file does not exist anywhere in this repo or the broader workspace
(checked directly, not inferred). Either it was written and later
lost, or the comment was aspirational and the doc was never created.
Not investigated further here since it's tangential to this spec;
flagged for awareness, not fixed as part of this work.
