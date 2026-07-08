# Merge Pipeline, Test Plan

**Status:** Draft
**Last modified:** 2026-07-07 21:17 EDT

This page is the test-coverage roadmap for the new merge pipeline
(`chatter speaker-id` + `chatter merge` + `chatter adjudicate` +
the override-file format + the underlying
`talkbank-transform::speaker_id` types). It exists because, per this
repo's root `CLAUDE.md`
red/green TDD rule, every new feature starts with failing tests
at the highest level the feature lives at, and we want to
enumerate those tests *before* writing the implementation, so
coverage is designed, not discovered.

This is a **plan**, not yet code. When the implementation work
begins, every test case below becomes a real test; the doc then
flips to a coverage matrix that gets kept honest by CI.

## TDD discipline, what "strict red/green" means here

Every cycle of impl-phase work is:

1. **RED.** Write ONE failing test at the highest layer the
   feature lives at. The test exercises a real user-observable
   behavior, not an internal helper. Commit the failing test
   alone (or stage it before any code change), verify it fails
   for the *right* reason (the missing behavior), not for a
   compile error or a typo.
2. **GREEN.** Write the *smallest* code change that makes the
   test pass. No anticipating future tests, no scaffolding for
   tests that don't yet exist. The codebase should compile and
   pass tests at this point.
3. **REFACTOR.** With the green test as the safety net, tighten
   the implementation: extract helpers, rename for clarity,
   replace primitives with newtypes, document tricky parts.
   Tests stay green throughout.
4. **DRILL DOWN if needed.** If the L3 (or L2) test passes but
   pinned the behavior less precisely than the contract
   requires (e.g., the L3 test asserts "exit 2 with some error"
   but the contract says "the specific MergeError variant must
   match"), add an L2 (or L1) test next that drills into the
   precise path. The drilled test FAILS at first against the
   green-but-imprecise impl, motivating the tighter impl.

Cycles must be **atomic**: one RED → one GREEN → optional
REFACTOR → optional drill-down. Do not stack multiple tests on
top of a single impl change; do not write impl ahead of tests.
The discipline matters because the bug bar of this pipeline is
high (CHAT-data byte-stable preservation, audit-trail
reproducibility) and TDD is the cheapest way to catch
regressions before they ship.

## Three test layers + the adjudication layer

The merge pipeline's behavior spans four substrates with
different testing mechanisms.

| Layer | Substrate | Why tests live here |
|---|---|---|
| **L1, Spec / fragment** | `spec/constructs/speaker-id/` → current `spec/tools` generators | Token-cleaner behavior on CHAT *fragments* (markup strip for Jaccard scoring). Same mechanism that pins parser/grammar tests; regenerated regression. |
| **L2, Transform / AST** | `crates/talkbank-transform/tests/` | Pure-Rust tests over parsed `ChatFile` values. `identify_mapping`, `apply_mapping`, `merge`, `run_adjudication` semantics on hand-built or parsed CHAT inputs. No process boundary. |
| **L3, CLI / subprocess** | `crates/chatter/tests/merge_tests.rs` (new) | End-to-end behavior of `chatter speaker-id`, `chatter merge`, and `chatter adjudicate` invoked as subprocesses (`assert_cmd` + `predicates`). Exit codes, flag parsing, file I/O, stderr formats. |
| **L4, Scripted adjudication** | `crates/talkbank-transform/tests/adjudication_tests.rs` + scripted prompter | Operator-decision paths in `chatter adjudicate`. Uses `ScriptedPrompter` injecting synthetic operator choices. See [Adjudication Workflow](./adjudication-workflow.md) for the prompter abstraction. |

L1 ⊂ L2 ⊂ L3 in terms of failure-mode coverage: a failing L1 test
implies a failing L2 test which implies a failing L3 test. So
when the same invariant could be tested at multiple layers, the
*starter* test is the highest layer and lower-layer tests are
supplements that pin the precise internal path. L4 sits beside
L2/L3, same crate/file conventions but a dedicated layer
because the prompter-injection pattern is specific to
adjudication.

## L1, Spec / fragment tests

Lives in `spec/constructs/speaker-id/`. Three subdirectories:

- `token-cleaner/`: what the Jaccard tokenizer strips and keeps
- `jaccard-scoring/`: fixed-input → fixed-score golden tests
- `mapping-application/`: header rewrite rules on real fragments

### L1.1, Token cleaner

Each spec is a CHAT main-tier fragment + the expected token list
after cleaning. Behavior pinned: bracket markup stripped,
angle-bracket retracing unwrapped, terminator variants
discarded, `&-...` / `&+...` discarded, `xxx`/`yyy`/`www`
discarded, `0` discarded, `@l` / `@n` / `@c` suffix dropped,
`_`-compound split to spaces, punctuation stripped, lowercased,
≥2-char alpha filter, NAK bullets stripped.

| Spec | Input fragment | Expected tokens |
|---|---|---|
| `clean-plain-utterance` | `*CHI:\thello world .` | `["hello", "world"]` |
| `clean-strip-bracket-codes` | `*CHI:\thello [*] [/] world [//] .` | `["hello", "world"]` |
| `clean-unwrap-angle-retrace` | `*CHI:\t<two of the> [//] three of the presents .` | `["two", "of", "the", "three", "of", "the", "presents"]` |
| `clean-strip-fillers` | `*CHI:\t&-um &+pre something &-uh .` | `["something"]` |
| `clean-strip-zero-and-paralinguistic` | `*CHI:\t0 [=! nodding] .` | `[]` |
| `clean-strip-unintelligible` | `*CHI:\txxx and yyy and www .` | `["and", "and"]` |
| `clean-strip-bullets` | `*CHI:\thello world . \x150_1234\x15` | `["hello", "world"]` |
| `clean-special-form-suffix` | `*CHI:\tnaming l@l u@l l@l u@l .` | `["naming"]` |
| `clean-compound-underscore` | `*CHI:\tValentine's_Day and Fruit_Loops .` | `["valentine", "day", "and", "fruit", "loops"]` |
| `clean-terminator-variants` | `*CHI:\thello +//. world +... again +/. last !` | `["hello", "world", "again", "last"]` |
| `clean-overlap-markers` | `*CHI:\t↫here↫ and there .` | `["here", "and", "there"]` |
| `clean-lowercase-filter` | `*CHI:\tHello World A I am .` | `["hello", "world", "am"]` |

Each spec file in `spec/constructs/speaker-id/token-cleaner/` has
the standard `# name`, `## Input`, `## Expected tokens`, and
`## Metadata` sections per the spec authoring template at
`spec/CLAUDE.md` in the workspace root (outside the book).

### L1.2, Jaccard scoring

Fixed bag-of-tokens pairs with known multiset Jaccard. These
guard against off-by-one errors in the `sum_w min` / `sum_w max`
implementation and against any future "optimizations" that
silently change scoring.

| Spec | Bag A | Bag B | Expected `J(A,B)` |
|---|---|---|---|
| `jaccard-identical` | `{hello:2, world:1}` | `{hello:2, world:1}` | `1.0` |
| `jaccard-disjoint` | `{hello:1}` | `{world:1}` | `0.0` |
| `jaccard-empty-empty` | `{}` | `{}` | `0.0` |
| `jaccard-empty-nonempty` | `{}` | `{x:1}` | `0.0` |
| `jaccard-multiset-counts` | `{a:3, b:1}` | `{a:1, b:1}` | `2/4 = 0.5` |
| `jaccard-partial-overlap` | `{a:1, b:1, c:1}` | `{b:1, c:1, d:1}` | `2/4 = 0.5` |

### L1.3, Mapping application on fragments

Header-rewrite micro-tests. Each spec gives an input
`@Participants:` or `@ID:` row and a small mapping; the expected
output row is the rewritten form.

| Spec | Input row | Mapping | Expected output row |
|---|---|---|---|
| `participants-rewrite-rename` | `@Participants:\tPAR0 Participant, PAR1 Participant` | `PAR0→INV:Investigator, PAR1→drop` | `@Participants:\tINV Investigator` |
| `participants-preserve-name-token` | `@Participants:\tCHI Alex Target_Child, PAR0 Participant` | `PAR0→INV:Investigator` | `@Participants:\tCHI Alex Target_Child, INV Investigator` |
| `id-rewrite-rename` | `@ID:\teng\|corpus_name\|PAR0\|\|\|\|\|Participant\|\|\|` | `PAR0→INV:Investigator` | `@ID:\teng\|corpus_name\|INV\|\|\|\|\|Investigator\|\|\|` |
| `id-drop-removes-row` | `@ID:\teng\|...\|PAR1\|\|\|\|\|Participant\|\|\|` | `PAR1→drop` | *(row removed)* |
| `id-preserves-other-fields` | `@ID:\teng\|2\|CHI\|6;01.\|female\|NF\|\|Target_Child\|\|\|` | `(no-op for CHI)` | identical to input |

## L2, Transform / AST tests

Lives in `crates/talkbank-transform/tests/`. Three test files:

- `speaker_id_tests.rs`
- `transcript_merge_tests.rs`
- `override_file_tests.rs`

Each tests behavior over parsed `talkbank-model::ChatFile` values,
using inline synthetic CHAT strings parsed via
`talkbank_parser::parse_chat_file` (no subprocess overhead).

### L2.1, `identify_mapping` (reference mode)

| Test | Scenario | Assertion |
|---|---|---|
| `identify_mapping_clean_winner` | Reference has CHI saying content X; donor has PAR0 saying X verbatim and PAR1 saying unrelated content | Returns `SpeakerMapping { drop: {PAR0}, rename: {PAR1: INV} }`, margin >> 2.0 |
| `identify_mapping_borderline_refuses` | Reference and both donor speakers share substantial vocabulary (margin < 2.0) | Returns `Err(SpeakerIdError::LowConfidence { scores, threshold, margin })` |
| `identify_mapping_anchor_missing` | Reference has no utterances tagged with anchor speaker | Returns `Err(SpeakerIdError::AnchorMissingInReference { anchor: CHI })` |
| `identify_mapping_single_speaker_donor` | Donor has only one speaker | Returns `Err(SpeakerIdError::InsufficientSpeakers { n: 1 })` |
| `identify_mapping_threshold_at_exact_value` | Constructed donor where margin = 2.0 exactly with threshold 2.0 | Returns `Ok(_)` (≥ comparison, not strict >) |
| `identify_mapping_threshold_below_exact_value` | Margin = 1.9999 with threshold 2.0 | Returns `Err(SpeakerIdError::LowConfidence)` |
| `identify_mapping_unbounded_margin` | Donor PAR1 has Jaccard 0 against reference; PAR0 > 0 | Returns `Ok(_)` with `margin = Margin::Unbounded` |
| `identify_mapping_deterministic` | Same inputs, repeated call | Identical `SpeakerMapping` byte-for-byte (BTreeMap ordering) |

### L2.2, `apply_mapping`

| Test | Scenario | Assertion |
|---|---|---|
| `apply_mapping_renames_main_tier` | Donor has `*PAR0:\t...` and `*PAR1:\t...`; mapping renames PAR0→INV, drops PAR1 | Output has `*INV:\t...` for original PAR0 utts; PAR1 utts absent |
| `apply_mapping_byte_stable_except_prefix` | Donor has rich CHAT markup, %wor, %com on every utt | Every retained utt is byte-identical except the `*CODE:\t` prefix; dependent tiers preserved exactly |
| `apply_mapping_rewrites_participants` | Donor `@Participants:` has PAR0+PAR1 entries | Output has only INV entry (after PAR1 drop) |
| `apply_mapping_rewrites_id` | Donor `@ID:` rows for PAR0+PAR1 | PAR0 row rewritten to INV with role tag; PAR1 row removed |
| `apply_mapping_speaker_not_in_input` | Mapping references PAR9 which isn't in donor | Returns `Err(SpeakerIdError::MappingSpeakerNotInInput { speaker: PAR9 })` |
| `apply_mapping_speaker_not_in_mapping` | Donor has PAR0+PAR1+PAR2 but mapping only covers PAR0+PAR1 | Returns `Err(SpeakerIdError::SpeakerNotInMapping { speaker: PAR2 })` |
| `apply_mapping_preserves_other_headers` | Donor has `@Languages`, `@Media`, `@Comment` | All non-Participants/non-ID headers pass through verbatim |
| `apply_mapping_idempotent_on_rerun` | Apply mapping, parse output, apply identity mapping | Output unchanged (byte-stable) |

### L2.3, `merge` (core invariants)

These mirror the user-guide's "What the merged output guarantees"
section directly. Each invariant from that section maps to one
or more L2 tests; the L3 tests then re-exercise the same
invariant through the CLI.

| Test | Invariant from user-guide | Assertion |
|---|---|---|
| `merge_retained_speakers_byte_stable` | "Retained speakers are byte-stable" | Every `*CHI:` block from File 1 (main tier + all dependent tiers, including `%com`) appears in the output byte-identical, in original order |
| `merge_strips_default_derived_tiers` | "Inserted speakers' downstream-generated tiers are stripped" | Output has no `%wor`, `%mor`, `%gra`, `%pho` on inserted-speaker utts; other dependent tiers preserved |
| `merge_strip_tiers_configurable` | "configurable via `--strip-tiers`" | Custom `strip_tiers=[com]` removes `%com` instead of the defaults |
| `merge_strip_tiers_empty_preserves_all` | empty strip set | Inserted utts retain `%wor`, `%mor`, `%gra`, `%pho` from File 2 verbatim |
| `merge_utterance_order_by_start_time` | "Utterance order is timeline order" | Output utterances sorted by start_ms ascending |
| `merge_stable_tiebreak_file1_first` | "first-file utterance comes first" | When File 1 and File 2 each have an utterance starting at exactly t, the File 1 one appears first in the output |
| `merge_bullets_pass_through` | "Time bullets are pass-through" | Every bullet in the output is exactly the bullet from its source utterance, merge does not recompute, smooth, or refresh |
| `merge_bullet_lift_from_wor` | "If main tier lacks bullet, lift from %wor" | Donor utt with no end-of-line bullet but a `%wor` row gets a derived `\x15<first>_<last>\x15` appended; original `%wor` then stripped per the tier policy |
| `merge_no_overlap_markers_injected` | "Overlap markup is NOT injected" | Even when inserted utt's bullet overlaps a retained utt's bullet by 500ms, no `[>]`/`[<]` tokens appear anywhere in the output that weren't in the original retained file |
| `merge_preserves_existing_overlap_markers` | retained file already has `[>]` somewhere | The original `[>]` is preserved byte-stable on the retained utt |
| `merge_header_languages_passthrough` | Header reconciliation rule | Output `@Languages` matches File 1's |
| `merge_header_media_file1_wins` | Header reconciliation rule | File 1 says `video`, File 2 says `audio` → output says `video` (no warning emitted for modality only) |
| `merge_header_participants_concatenates` | Header reconciliation rule | Output `@Participants:` is File 1's entries + File 2's non-retained entries, in that order, with **dedupe-on-insert**: a File 2 entry whose speaker code File 1 already declares is skipped rather than inserted twice (legal only when File 1's declaration is vestigial: zero utterances, matching role/name metadata; otherwise the merge refuses with `MergeError::ParticipantAlreadyDeclared`) |
| `merge_header_id_concatenates` | Header reconciliation rule | Output `@ID:` rows are File 1's + File 2's non-retained, original order within each file; the same dedupe-on-insert set also filters File 2's `@ID` rows, so a deduped participant contributes no duplicate `@ID` row |
| `merge_header_comments_concatenate` | Header reconciliation rule | Output `@Comment` rows are File 1's + File 2's, in original order (ASR provenance preserved) |
| `merge_preconditions_retain_missing` | exit code 2 precondition | File 1 declares no CHI; merge with `retain={CHI}` returns `Err(MergeError::RetainSpeakersMissing)` |
| `merge_preconditions_no_timeline` | exit code 2 precondition | File 1 has no utterances with bullets → `Err(MergeError::NoTimelineInFile1)` |
| `merge_preconditions_language_mismatch` | exit code 2 precondition | File 1 `@Languages: eng`, File 2 `@Languages: yue` → `Err(MergeError::LanguageMismatch)` |
| `merge_preconditions_ambiguous_speaker` | exit code 2 precondition | Both files have INV utterances and retain={CHI} (INV not in retain) → `Err(MergeError::AmbiguousSpeaker { speaker: INV })` |
| `merge_warns_on_backward_bullet_drift` | "small backward-time bullets ... proceeds" | File with `utt1: 100_200`, `utt2: 190_300`, succeeds, emits a warning |

### L2.4, Override file I/O

| Test | Scenario | Assertion |
|---|---|---|
| `override_file_round_trip` | Construct `OverrideFile` with one entry, write, read back | Re-read value `==` original |
| `override_file_refuses_missing_schema_version` | TOML with no `schema_version` | `Err(OverrideFileError::UnsupportedSchemaVersion { found: None, supported: 2 })` (the shipped `found` is an `Option<u32>`, so absence reports as `None`, not a sentinel value) |
| `override_file_refuses_wrong_schema_version` | `schema_version = 99` (any value other than the current `2`; a pre-bump `schema_version = 1` file is refused the same way, per the v1-to-v2 migration note in [merge-overrides.md](../chatter/integrating/merge-overrides.md)) | `Err(UnsupportedSchemaVersion { found: Some(99), supported: 2 })` |
| `override_file_rejects_unknown_field` | Entry has an extraneous field `extra = "x"` | `Err(OverrideFileError::Parse)` |
| `override_file_rejects_malformed_mode` | `mode = "guess"` | `Err(Parse)` (only `auto`/`explicit`/`override` accepted) |
| `override_file_atomic_write` | Write to a path that already exists | Original file is replaced atomically; no `<path>.tmp` left behind |
| `override_file_deterministic_serialization` | Same struct, write twice | Bytes on disk are byte-identical between writes |
| `override_file_omits_empty_optionals` | Entry has empty `scores`, no `margin`, empty `flags` | TOML output does not contain those keys |
| `override_file_preserves_margin_unbounded` | Entry has `margin = Margin::Unbounded` | TOML on disk has `margin = "unbounded"`; reads back as `Unbounded` |
| `override_file_preserves_margin_finite` | Entry has `margin = Margin::Finite(3.81)` | TOML on disk has `margin = 3.81`; reads back equal |
| `override_file_read_or_default_missing` | Path does not exist | Returns empty `OverrideFile` with current schema version |
| `override_file_get_returns_entry` | File has one entry under SessionId X | `get(X)` returns Some; `get(Y)` returns None |

### L2.5, Domain-type unit tests

Smaller per-type tests. Each in its module's `#[cfg(test)] mod
tests` section.

Note on type names: several of the designed types referenced below
shipped under different names or shapes (`InsertedRole` is
`InsertedRoleSpec`; `Margin` is `ConfidenceMargin(f64)` with
`f64::INFINITY` for the unbounded case; `RetainSet` and `MergeFlag`
never shipped as newtypes). See the designed-vs-shipped table in
[Domain Types](./merge-domain-types.md#designed-vs-shipped-quick-map)
before writing any still-pending test from this table against the
current code.

| Test | Type | Assertion |
|---|---|---|
| `jaccard_score_new_in_range` | `JaccardScore` | `new(0.5)` → `Ok`; `new(-0.1)` and `new(1.1)` → `Err`; `new(NaN)` → `Err` |
| `jaccard_score_serde_round_trip` | `JaccardScore` | Serializes to `0.5` (bare float in JSON/TOML); deserializes back identically; out-of-range deserialize → error |
| `confidence_threshold_default_is_2_0` | `ConfidenceThreshold` | `Default::default().value() == 2.0` |
| `confidence_threshold_rejects_below_1` | `ConfidenceThreshold` | `new(0.5)` → `Err` |
| `margin_from_scores_zero_loser` | `Margin` | `from_scores(JaccardScore::new(0.7), JaccardScore::zero()) == Margin::Unbounded` |
| `margin_from_scores_zero_zero` | `Margin` | `from_scores(zero, zero) == Margin::Finite(0.0)` or explicit "degenerate" representation (decide and document) |
| `margin_meets_threshold` | `Margin` | `Finite(3.81).meets(threshold=2.0) == true`; `Finite(1.5).meets(2.0) == false`; `Unbounded.meets(threshold) == true` for any threshold |
| `retain_set_parse` | `RetainSet` | `"CHI".parse() == Ok({CHI})`; `"CHI,SI2".parse() == Ok({CHI, SI2})`; `"".parse() == Err`; `"CHI,,SI2".parse() == Err` |
| `inserted_role_parse` | `InsertedRole` | `"INV:Investigator".parse() == Ok(_)`; `"INV".parse() == Err`; `":Investigator".parse() == Err` |
| `mapping_spec_parse_simple` | `parse_mapping_spec` | `"PAR0=drop,PAR1=INV:Investigator"` parses to a complete `MappingSpec` with `Drop` for PAR0 and a `Rename` carrying PAR1's own code + role |
| `mapping_spec_parse_drop_only` | `parse_mapping_spec` | `"PAR0=drop"` parses; a drop-only mapping is legal in isolation, since roles are per-speaker and a mapping with no `Rename` needs no role at all |
| `mapping_spec_parse_multiple_roles` | `parse_mapping_spec` | `"PAR0=INV:Investigator,PAR1=MOT:Mother"` parses, with each speaker's `Rename` carrying its own role. (The original plan named this `mapping_spec_parse_conflicting_roles` and expected an error because the designed v1 schema allowed only one shared inserted role; the shipped schema-v2 per-speaker `adult_roles` map makes multiple distinct roles a supported case, and two adults assigned the *same* role auto-disambiguate to numbered codes `INV1`/`INV2` with `First_`/`Second_` specific-role labels.) |
| `merge_flag_serde_known_variants` | `MergeFlag` | `DiarizationMixed` serializes as `"diarization-mixed"` (kebab-case); deserializes the same |
| `merge_flag_serde_custom` | `MergeFlag` | Unknown string deserializes as `Custom("unknown-flag")`; serializes verbatim |

## L3, CLI / subprocess tests

Lives in `crates/chatter/tests/merge_tests.rs` (new file).
Uses the same `assert_cmd` + `predicates` + `tempfile` pattern
as the existing `integration_tests.rs`. Each test invokes
`chatter speaker-id` or `chatter merge` as a subprocess against
files written to a `tempdir()`.

### L3.1, `chatter merge`, success paths

| Test | Invariants exercised |
|---|---|
| `merge_basic_clinician_pattern` | E2E happy path: small hand-coded child-only file + small ASR-labeled file → exit 0, output exists, retained CHI byte-stable, inserted INV present with derived tiers stripped. Single-invocation smoke test. |
| `merge_writes_to_stdout_by_default` | No `-o` flag → output goes to stdout, exit 0 |
| `merge_writes_to_output_path` | `-o merged.cha` → file created with correct content; nothing on stdout |
| `merge_retain_multi_speaker` | `--retain CHI,SI2` keeps both CHI and SI2 byte-stable; everything else from File 2 |
| `merge_strip_tiers_custom` | `--strip-tiers com,act` removes `%com` and `%act` instead of default set |
| `merge_strip_tiers_empty` | `--strip-tiers ''` preserves `%wor` from File 2 in output |

### L3.2, `chatter merge`, error paths

| Test | Asserted exit code | Asserted stderr |
|---|---|---|
| `merge_missing_file1` | 1 | "No such file" or equivalent typed message |
| `merge_unparseable_file1` | 1 | parser diagnostic |
| `merge_missing_retain_flag` | 2 (clap) | clap usage message |
| `merge_retain_empty_value` | 2 | typed error from `RetainSet::from_str` |
| `merge_no_retain_speakers_in_file1` | 2 | `RetainSpeakersMissing` rendered |
| `merge_no_timeline_in_file1` | 2 | `NoTimelineInFile1` rendered |
| `merge_language_mismatch` | 2 | `LanguageMismatch { file1: eng, file2: yue }` rendered |
| `merge_ambiguous_speaker` | 2 | `AmbiguousSpeaker { speaker: ... }` rendered with hint to use --retain |

### L3.3, `chatter speaker-id`, reference mode

| Test | Scenario | Assertion |
|---|---|---|
| `speaker_id_reference_auto_clean_winner` | Reference + donor where margin >> 2.0 | Exit 0; output has expected renamed/dropped speakers |
| `speaker_id_reference_writes_override` | With `--write-override path.toml` | File created; entry has `mode = "auto"`, scores, margin, decided_at, operator |
| `speaker_id_reference_appends_to_existing_override` | `--write-override path.toml` where file already has another session | New session added; existing session preserved |
| `speaker_id_reference_low_confidence_exits_4` | Margin < threshold | Exit 4; stderr contains per-speaker scores |
| `speaker_id_reference_anchor_missing_exits_2` | Reference has no anchor speaker utterances | Exit 2; typed error in stderr |
| `speaker_id_reference_threshold_override` | `--confidence-threshold 1.5` on a margin-1.7 case | Exit 0 (would have refused at default 2.0) |
| `speaker_id_reference_anchor_required` | `--reference` without `--anchor` | Exit 2 (clap or our own); usage error |

### L3.4, `chatter speaker-id`, explicit-mapping mode

| Test | Scenario | Assertion |
|---|---|---|
| `speaker_id_explicit_basic` | `--mapping "PAR0=drop,PAR1=INV:Investigator"` | Exit 0; output renames PAR1→INV, drops PAR0 |
| `speaker_id_explicit_mapping_speaker_not_in_input` | `--mapping` references PAR9 not in input | Exit 2; typed error |
| `speaker_id_explicit_speaker_missing_from_mapping` | Input has PAR0+PAR1+PAR2; mapping only covers PAR0+PAR1 | Exit 2; typed error naming PAR2 |
| `speaker_id_explicit_with_note_records_in_override` | `--mapping` + `--write-override` + `--note "verified by listening"` | TOML entry has `note = "verified by listening"` and `mode = "explicit"` |

### L3.5, `chatter speaker-id`, override-file mode

| Test | Scenario | Assertion |
|---|---|---|
| `speaker_id_override_file_replay` | Override file has entry for session-X | Reading override + applying produces same output as the original auto/explicit run |
| `speaker_id_override_file_missing_entry` | Override file has no entry for the requested session | Exit 2; `OverrideEntryMissing` in stderr |
| `speaker_id_override_file_missing_file` | `--override-file path.toml` where file doesn't exist | Exit 1; `NotFound` in stderr |
| `speaker_id_override_file_wrong_schema_version` | File has `schema_version = 99` | Exit 1; `UnsupportedSchemaVersion` in stderr |
| `speaker_id_override_file_mutually_exclusive_modes` | `--reference` AND `--mapping` both set | Exit 2 (clap or our own); only one operation mode allowed |

### L3.6, Pipeline composition

These exercise `chatter speaker-id` → `chatter merge` composed
end-to-end through the file system, simulating the orchestrator
workflow.

| Test | Scenario | Assertion |
|---|---|---|
| `pipeline_speaker_id_then_merge` | Run speaker-id on anonymous ASR file; run merge on the result + hand-coded file | Final merged file passes all merge invariants (retained byte-stable, etc.) |
| `pipeline_replay_via_override_file` | Run once with auto; capture override file; delete intermediates; replay via `--override-file`; merge again | Final merged file is byte-identical to the original run (audit-trail-reproducibility property) |
| `pipeline_low_confidence_then_explicit` | Run speaker-id; gets exit 4; capture scores from stderr; run again with `--mapping` matching what the operator would decide; record via `--write-override`; merge | All steps succeed; override file has `mode = "explicit"` with prior scores recorded |

## L4, Scripted adjudication tests

Lives in `crates/talkbank-transform/tests/adjudication_tests.rs`.
Uses the `Prompter` trait and `ScriptedPrompter` documented in
[Adjudication Workflow §The prompter abstraction](./adjudication-workflow.md#the-prompter-abstraction-testability).
Each test constructs a pending-adjudications input, scripts the
operator's decisions, runs `run_adjudication`, and asserts on
the resulting override file plus the residual pending file.

### L4.1, Speaker-id adjudication paths

| Test | Scripted decision | Assertion |
|---|---|---|
| `adjudicate_speaker_id_accepts_suggested` | `AcceptSuggested { note: None }` for one pending entry | Override file entry has `mode = "explicit"`, mapping matches suggested, pending file emptied |
| `adjudicate_speaker_id_override_mapping` | `OverrideMapping { mapping: { PAR0=rename, PAR1=drop }, note: Some("verified by listening") }` (opposite of suggested) | Override file mapping matches operator's choice; note recorded |
| `adjudicate_speaker_id_defer` | `Defer { reason: "need to listen to audio" }` | Pending entry untouched; override file unchanged; tool exits 4 (deferred) |
| `adjudicate_speaker_id_block` | `Block { reason: "reference file missing bullets" }` | Pending entry tagged as blocked; override file unchanged |
| `adjudicate_speaker_id_kind_mismatch_rejected` | `OverrideInsertedRole { ... }` against a `speaker-id-low-confidence` entry | Returns `Err(AdjudicationError::DecisionKindMismatch)`; nothing written |

### L4.2, Parent-role-lookup adjudication paths

| Test | Scripted decision | Assertion |
|---|---|---|
| `adjudicate_parent_role_accepts_default_inv` | `AcceptSuggested` | Override entry uses `INV:Investigator` (the safe default) |
| `adjudicate_parent_role_overrides_to_mother` | `OverrideInsertedRole { code: "MOT", tag: "Mother" }` | Override entry uses MOT; note recorded |
| `adjudicate_parent_role_overrides_to_father` | `OverrideInsertedRole { code: "FAT", tag: "Father" }` | Override entry uses FAT |
| `adjudicate_parent_role_invalid_code_rejected` | `OverrideInsertedRole { code: "", tag: "Mother" }` | Returns `Err`; with `--skip-on-error`, logs and proceeds |

### L4.3, Diarization-mix and sanity-scan paths

| Test | Scripted decision | Assertion |
|---|---|---|
| `adjudicate_diarization_mix_flag_only` | `Flag { flags: [DiarizationMixed], note: "PAR0 mixes clinician+parent" }` | Existing override entry gets flag added; mapping unchanged |
| `adjudicate_sanity_scan_swap_mapping` | `OverrideMapping { ... }` reversing original speaker-id | Override entry updated; `mode = "explicit"`; original mapping preserved in `history` |
| `adjudicate_sanity_scan_confirms_real_overlap` | `Flag { flags: [Custom("real-overlap-confirmed")] }` | Override entry gets custom flag; mapping unchanged |

### L4.4, Workflow plumbing

| Test | Scenario | Assertion |
|---|---|---|
| `adjudicate_empty_pending_file_noop` | Pending file has empty `entries` array | Exit 0; nothing changes |
| `adjudicate_resumption_skips_decided_entries` | Pending file has 3 entries; first 2 already decided in override; only 3rd has no override entry | Prompter is called exactly once, for the 3rd entry |
| `adjudicate_re_adjudicate_preserves_history` | Existing override entry; `--re-adjudicate` with new decision | New decision saved; prior decision preserved in `history` array |
| `adjudicate_kind_filter_processes_only_matching` | Pending file has mixed kinds; `--kind parent-role-lookup` flag set | Prompter only called for parent-role-lookup entries; other kinds untouched |
| `adjudicate_dry_run_writes_nothing` | Any pending input + any decision; `--dry-run` set | Override file unchanged; pending file unchanged |
| `adjudicate_scripted_mode_unknown_session_aborts` | Scripted decisions reference session-X but pending has only session-Y | Returns `Err(AdjudicationError::ScriptedDecisionWithoutPendingEntry)`; tool exits 2 |
| `adjudicate_scripted_mode_extra_pending_aborts` | Pending has session-X and session-Y; scripted decisions cover only session-X | Returns `Err(AdjudicationError::PendingEntryWithoutScriptedDecision)`; tool exits 2 |
| `adjudicate_mutually_exclusive_modes` | `--interactive` + `--scripted` both set | Returns `Err`; tool exits 2 (clap or our own validator) |

### L4.5, Prompter contract conformance

These tests pin the contract that any `Prompter` impl must
satisfy, so future UI backends (VS Code, web) can be developed
against the same invariants.

| Test | Scenario | Assertion |
|---|---|---|
| `prompter_terminal_round_trip_decision` | `TerminalPrompter` reading a scripted stdin | Returns the expected `OperatorDecision` parsed from the operator's typed input |
| `prompter_scripted_returns_decisions_in_order` | `ScriptedPrompter::from_decisions([d1, d2, d3])` | Three consecutive `ask()` calls return d1, d2, d3 in order |
| `prompter_scripted_panics_on_unscripted_session` | `ScriptedPrompter` has decisions for session A; tool asks for session B | `ask()` returns `Err(PrompterError::NoDecisionFor(SessionId))` |
| `prompter_scripted_toml_round_trips` | Write a scripted-decisions TOML, read with `ScriptedTomlPrompter`, run | Same `OperatorDecision` sequence as a `ScriptedPrompter::from_decisions` with equivalent contents |

## Fixture catalog

These are the synthetic CHAT pairs that the tests above
consume. Each is small (≤20 utterances), exercises a precise
invariant, and is fully fictional (no real corpus content).

The fixtures live as inline `const FIX_*: &str` blocks in the
respective test modules, following the precedent in
`chatter/tests/integration_tests.rs` (which has
`const VALID_CHAT: &str = r#"..."#` etc.).

### `FIX_REF_TWO_UTT_NO_MARKUP`

The **smallest possible** valid CHAT pair input. Two `*CHI:`
utterances, no markup beyond a simple terminator, time bullets
on both. Used by cycle 1's smoke test where the impl must
work without yet handling any markup edge cases.

### `FIX_ASR_LABELED_TWO_UTT`

The matching donor for `FIX_REF_TWO_UTT_NO_MARKUP`: two
`*INV:` utterances at different time positions. Used by
cycle 1.

### `FIX_REF_CHILD_ONLY_SIMPLE`

A 6-utterance child-only hand transcript with rich CHAT markup
(error code, retracing, filled pause, special-form letter, zero
realization with paralinguistic). Used by every L2/L3 merge
test from cycle 2 onward as the canonical "File 1", the
reference / authoritative file. Has time bullets on every
utterance.

### `FIX_ASR_ANON_2SPEAKER_SIMPLE`

The matching ASR-output file with anonymous `PAR0` (clinician,
asks questions) and `PAR1` (child, says what `FIX_REF_*` shows
plus some extra). Has `%wor` on every utterance. Used by every
speaker-id test where auto-mode is expected to succeed cleanly
(margin >> 2.0).

### `FIX_ASR_LABELED_INV_SIMPLE`

`FIX_ASR_ANON_2SPEAKER_SIMPLE` after speaker-id has run with
`PAR1→drop, PAR0→INV:Investigator`. Used by merge tests where
we want to skip the speaker-id step and test merge alone.

### `FIX_ASR_BORDERLINE_VOCABULARY`

ASR file where both speakers describe the same picture-book
content (margin 1.6-1.9 against reference). Used by
low-confidence tests.

### `FIX_REF_NO_BULLETS`

A reference file with no time bullets at all. Used to test
`NoTimelineInFile1` precondition.

### `FIX_REF_LANG_ENG` / `FIX_ASR_LANG_YUE`

Two files with conflicting `@Languages`. Used to test
`LanguageMismatch`.

### `FIX_AMBIGUOUS_INV`

Two files both containing `*INV:` utterances, with
`--retain CHI` (INV not in retain set). Used to test
`AmbiguousSpeaker`.

### `FIX_REF_MULTI_RETAIN`

Reference file containing `*CHI:` and `*SI2:` utterances (sibling
target). Used to test `--retain CHI,SI2`.

### `FIX_ASR_NO_MAIN_BULLET`

Donor file where some utterances have no main-tier bullet, only
`%wor`. Used to test bullet-lift behavior in normalization.

### `FIX_OVERRIDE_VALID` / `FIX_OVERRIDE_WRONG_SCHEMA` / `FIX_OVERRIDE_MALFORMED`

Override files in valid, schema-rejected, and parse-rejected
shapes. Used by override-file I/O tests.

### `FIX_PENDING_SPEAKER_ID` / `FIX_PENDING_PARENT_ROLE` / `FIX_PENDING_MIXED_KINDS`

Pending-adjudications files exercising one kind, another kind,
and a mix. Used by L4 adjudication tests.

### `FIX_SCRIPTED_ACCEPT_ALL` / `FIX_SCRIPTED_OVERRIDE_FIRST_DEFER_SECOND`

Scripted-decisions TOML files for `ScriptedTomlPrompter`.
Cover the canonical accept-suggested case and a mixed
override+defer case.

The exact bytes of each fixture are pinned in their respective
test modules when the implementation lands; this plan doesn't
freeze them yet, only their *purpose*. Drafting the actual
bytes is the first step of impl-phase work.

## Coverage matrix

Cross-checking that every behavioral invariant from the four
design docs has at least one test:

| Invariant source | Invariant | First-failing layer | Test name |
|---|---|---|---|
| merge user-guide | Retained byte-stable | L3 → L2 | `merge_basic_clinician_pattern` + `merge_retained_speakers_byte_stable` |
| merge user-guide | Derived tiers stripped | L3 → L2 | `merge_strip_tiers_custom` + `merge_strips_default_derived_tiers` |
| merge user-guide | Order by start_ms | L2 | `merge_utterance_order_by_start_time` |
| merge user-guide | Tiebreak File1 first | L2 | `merge_stable_tiebreak_file1_first` |
| merge user-guide | Bullets pass-through | L2 | `merge_bullets_pass_through` |
| merge user-guide | Bullet lift from %wor | L2 | `merge_bullet_lift_from_wor` |
| merge user-guide | Header reconciliation (all rows, including @Participants / @ID dedupe-on-insert of donor codes File 1 already vestigially declares) | L2 | `merge_header_*` series |
| merge user-guide + memory | No overlap markers injected | L2 | `merge_no_overlap_markers_injected` + `merge_preserves_existing_overlap_markers` |
| merge user-guide | Each precondition → exit 2 | L3 | `merge_*_exits_2` series in L3.2 |
| merge user-guide | Warns on bullet drift | L2 | `merge_warns_on_backward_bullet_drift` |
| speaker-id user-guide | Reference mode auto | L3 | `speaker_id_reference_auto_clean_winner` |
| speaker-id user-guide | Explicit mode | L3 | `speaker_id_explicit_basic` |
| speaker-id user-guide | Override-file mode | L3 | `speaker_id_override_file_replay` |
| speaker-id user-guide | Confidence threshold (exit 4) | L3 → L2 | `speaker_id_reference_low_confidence_exits_4` + `identify_mapping_borderline_refuses` |
| speaker-id user-guide | Byte-stable except prefix | L2 | `apply_mapping_byte_stable_except_prefix` |
| speaker-id user-guide | Header rewrites | L2 + L1 | `apply_mapping_rewrites_*` + `participants-rewrite-*` specs |
| speaker-id user-guide | Provenance captured | L3 | `speaker_id_reference_writes_override` |
| speaker-id user-guide | Each precondition → typed error | L3 → L2 | various `*_exits_2` and `apply_mapping_*` tests |
| speaker-id user-guide | Token cleaner spec | L1 | `clean-*` specs |
| speaker-id user-guide | Multiset Jaccard formula | L1 | `jaccard-*` specs |
| override-file ref | Schema-version refusal | L2 | `override_file_refuses_*` tests |
| override-file ref | Round-trip fidelity | L2 | `override_file_round_trip` |
| override-file ref | Deterministic serialization | L2 | `override_file_deterministic_serialization` |
| override-file ref | Atomic write | L2 | `override_file_atomic_write` |
| override-file ref | margin `"unbounded"` form | L2 | `override_file_preserves_margin_unbounded` |
| domain types | `JaccardScore` range | L2 | `jaccard_score_new_in_range` |
| domain types | `ConfidenceThreshold ≥ 1` | L2 | `confidence_threshold_*` |
| domain types | `Margin` semantics | L2 | `margin_*` |
| domain types | `RetainSet::from_str` | L2 | `retain_set_parse` |
| domain types | `InsertedRole::from_str` | L2 | `inserted_role_parse` |
| domain types | `parse_mapping_spec` | L2 | `mapping_spec_parse_*` |
| domain types | `MergeFlag` serde | L2 | `merge_flag_serde_*` |
| domain types | Pipeline reproducibility | L3 | `pipeline_replay_via_override_file` |

Every invariant has at least one named test; many have multiple
across layers. When the impl phase begins, the first commit
should produce the fixtures, the second commit the highest-layer
failing test for the simplest invariant, then drill down per
the standard TDD progression.

## What this plan does NOT cover

- **Performance / scaling tests.** Until the pipeline shows up
  on a measured workload, no targeted perf assertions. The
  reference corpus's existing round-trip benchmarks remain the
  baseline.
- **Fuzz testing.** This repository now has a local `fuzz/`
  workspace for parser/validation fuzzing. If the merge crate
  stabilizes enough to justify dedicated fuzzing, adding a
  merge-specific target for random parseable CHAT-pair inputs is a
  follow-up, not a v1 blocker.
- **Cross-platform CI checks.** Windows / Linux / macOS each
  build the workspace; the merge module rides the existing CI.
  No platform-specific tests needed (the merge operates on
  parsed AST and writes UTF-8; no path-or-line-ending quirks).
- **Real-corpus regression sweeps.** Once impl lands, running
  `chatter merge` over a curated subset of the reference
  corpus and snapshotting outputs is a smart follow-up. Lives
  in a separate `tests/golden/` style mechanism if added; not
  designed here.

## TDD authoring sequence

Each numbered item is one full **RED → GREEN → REFACTOR** cycle.
Cycles must run in order; do not start cycle N+1 until cycle N
is green and committed. Numbers are designed so the first
working pipeline (cycle 8) emerges from the absolute minimum
set of types + algorithms, then each later cycle extends.

The **starter test for cycle 1** is intentionally tiny: a 2-utterance
fixture pair with no markup, one retain speaker. The smoke test
exercises every layer (parser, transform, CLI) but with the
simplest possible CHAT bytes, so the first impl is small enough
to land in one cycle.

### Phase A, minimal end-to-end pipeline (cycles 1-8)

These cycles produce the simplest possible `chatter merge`
working end-to-end with synthetic fixtures.

| # | RED (failing test) | GREEN (smallest impl that passes) |
|---|---|---|
| 1 | `merge_basic_smoke`, L3 subprocess test against the **tiniest** fixture pair (`FIX_REF_TWO_UTT_NO_MARKUP` + `FIX_ASR_LABELED_TWO_UTT`), retain={CHI}, asserts exit 0 and "merged file exists" | Stub `chatter merge` subcommand wiring; introduce minimal `talkbank-transform::transcript_merge::merge` that interleaves utterances by start_ms and emits parser→serializer round-trip. No tier-stripping, no header-reconcile, no validation. Just: parse, sort, serialize. |
| 2 | `merge_retained_speakers_byte_stable`, L2 over the smoke fixture, asserts every CHI block byte-identical | Implement byte-stable handling for retained utterances (preserve `main_raw_lines` + dependent tiers exactly). |
| 3 | `merge_strips_default_derived_tiers`, L2 against a fixture where the donor has `%wor` rows | Implement `tier_strip` per the per-tier policy; drop `%wor`/`%mor`/`%gra`/`%pho` from inserted-speaker utts. |
| 4 | `merge_utterance_order_by_start_time`, L2 with a fixture where File 1 and File 2 utterances interleave | Implement `timeline` sort key (start_ms primary; source-order tiebreak). |
| 5 | `merge_header_participants_concatenates`, L2 | Implement `header_reconcile::participants_merge`. |
| 6 | `merge_header_id_concatenates`, L2 | Extend `header_reconcile` for @ID rows. |
| 7 | `merge_header_languages_passthrough` + `merge_header_media_file1_wins` + `merge_header_comments_concatenate`, L2 | Extend `header_reconcile` for remaining headers per the contract table. |
| 8 | `merge_preconditions_retain_missing` + `merge_preconditions_no_timeline` + `merge_preconditions_language_mismatch` + `merge_preconditions_ambiguous_speaker`, L3, each asserting exit code 2 with a specific stderr message | Implement `preconditions` module + map `MergeError` to exit codes in the CLI. |

#### Phase A, actual cycle log

The four-precondition cycle 8 was deliberately split into four
single-variant cycles (9a / 9b / 9c / 9d) so each `MergeError`
variant lands with its own RED→GREEN cycle and L2 + L3 sibling
tests. The numbering here is therefore finer-grained than the
plan table above; the table records the *shape* of Phase A, the
log records what was actually committed.

| # | Test(s) | Layer | Status |
|---|---------|-------|--------|
| 1 | `merge_basic_smoke` | L3 | done |
| 2 | `merge_retained_speakers_byte_stable` | L2 | done |
| 3 | `merge_strips_default_derived_tiers` | L2 | done |
| 4 | `merge_strip_tiers_configurable` | L2 | done |
| 5 | `merge_strip_tiers_empty_preserves_all` | L2 | done |
| 6 | `merge_header_participants_concatenates` | L2 | done |
| 7 | `merge_header_id_concatenates` | L2 | done |
| 8a | `merge_header_comments_concatenate` | L2 | done |
| 8b | `merge_header_languages_passthrough` + `merge_header_media_file1_wins` | L2 | done |
| 9a | `merge_no_retain_speakers_in_file1` + `_returns_err` | L3 + L2 | done (L2 sibling backfilled in 9c) |
| 9b | `merge_no_timeline_in_file1` + `_returns_err` | L3 + L2 | done |
| 9c | `merge_language_mismatch` + `_returns_err` | L3 + L2 | done |
| 9d | `merge_ambiguous_speaker` + `_returns_err` | L3 + L2 | done |

End of Phase A: `chatter merge` works on simple fixtures with
all four preconditions (retain / timeline / language / ambiguous
speaker) enforced. The pipeline is publishable as v0.

#### Phase B, actual cycle log

Phase B picks up at cycle 10 in the cycle log (Phase A used 9a-9d for
the precondition split).

| # | Test(s) | Layer | Status |
|---|---------|-------|--------|
| 10 | `speaker_id_explicit_basic` | L3 | done |
| 11 | `apply_mapping_byte_stable_except_prefix` + `apply_mapping_rewrites_participants` + `apply_mapping_rewrites_id` | L2 | done (regression-guards) |
| 12 | `identify_mapping_clean_winner` | L2 | done |
| 13 | `identify_mapping_borderline_refuses` | L2 | done |
| 14 | `speaker_id_reference_low_confidence_exits_4` | L3 | done |
| 15 | `speaker_id_reference_writes_override` (+ `OverrideFile` data model) | L3 | done |
| 16 | `speaker_id_override_file_replay` (+ `OverrideFile::get`) | L3 | done |
| 17 | `adjudicate_speaker_id_accepts_suggested` (+ adjudication core) | L4 | done |
| 18 | `adjudicate_scripted_accepts_suggested` (+ `chatter adjudicate` CLI + scripted-TOML I/O) | L3 | done |
| 19 | `speaker_id_reference_writes_pending_on_low_confidence` (+ `--write-pending` flag + `LowConfidence` carries `DonorMatchReport`) | L3 | done |
| 20 | `adjudicate_speaker_id_override_mapping` (+ `OperatorDecision::OverrideMapping` variant + scripted-TOML `override-mapping` shape) | L4 | done |
| 21 | `adjudicate_interactive_accepts_suggested` (+ `TerminalPrompter` + `--interactive` flag) | L3 | done |
| 22 | `adjudicate_parent_role_lookup_chooses_role` (+ `PendingKindData` promotion + `ParentRoleLookup` kind + `ChooseRole` decision) | L4 | done |
| 23 | `adjudicate_interactive_chooses_role` (+ `parse_operator_response` + kind-aware prompt hint) | L3 | done |
| 24 | `adjudicate_interactive_override_mapping` (+ `parse_override_mapping` + `parse_speaker_assignment`) | L3 | done |
| 25 | `pipeline_clean_winner_end_to_end` (+ `chatter pipeline` subcommand) | L3 | done |
| 26 | `batch_pass1_single_session` (+ `chatter batch` subcommand, subprocess driver) | L3 | done |
| 27 | `batch_mixed_outcomes` (regression-guard: clean+borderline aggregation) | L3 | done |
| 28 | `batch_pass2_replay` (+ `--override-file` on `pipeline` + `batch`; per-session auto-detection) | L3 | done |
| 29 | `batch_skip_existing` (+ `--skip-existing` flag on `batch` for idempotent re-runs) | L3 | done |
| 30 | refactor, `PipelineArgs` + `BatchArgs` structs retire three `#[allow(clippy::too_many_arguments)]` markers |, | done (true-no-op refactor; covered by cycles 25-29 regression suite) |
| 31 | refactor, split `commands/speaker_id.rs` (472 lines) into `speaker_id/{mod,modes,writes,support}.rs` (158 + 196 + 103 + 86 lines); retire 4 stale `#[allow(dead_code)]` markers on `ReferenceModeOutcome` (fields are read by `write_override_entry`) |, | done (true-no-op refactor; covered by cycles 10-29 regression suite) |
| 32 | `adjudicate_sanity_scan_accept_suggested` (+ `AdjudicationKind::SanityScanMisclassification` variant, `PendingKindData::SanityScanMisclassification { suggested, reason }` variant, two apply-decision arms mirroring `SpeakerIdLowConfidence`, terminal prompter render + prompt-hint arm) | L4 | done, adjudication kind end-to-end; the post-merge scan detector itself (heuristic + auto-pending-write) is a separate cycle 33 |
| 33 | `sanity_scan_flags_inverted_mlu` (+ `talkbank_transform::sanity_scan::scan_session` + `chatter sanity-scan` subcommand; mean-utterance-word-count asymmetry heuristic, default 1.5×, binary-mapping only) | L3 | done, detector + CLI end-to-end; multi-rename support, batch integration, and alternative heuristics deferred |
| 34 | `batch_writes_override_for_auto_decisions` (+ `--write-override` on both `chatter pipeline` and `chatter batch`; threaded through `PipelineArgs.write_override_path` + `BatchArgs.write_override_path`; reference-mode auto-decisions audit-trailed for sanity-scan + future re-runs) | L3 | done |
| 35 | `batch_with_sanity_scan_flag_flags_inverted_mlu` (+ `--sanity-scan` + `--sanity-scan-threshold` on `chatter batch`; post-loop subprocess driver for `chatter sanity-scan`; precondition validation requiring `--write-override` + `--write-pending`) | L3 | done |
| 36 | refactor, split `cli/args/core.rs` (984 → 747 lines): extract `DebugCommands` → `debug_commands.rs`, `CacheCommands` → `cache_commands.rs`, config enums (`LogFormat`, `TuiMode`, `OutputFormat`, `ParserBackend`, `AlignmentTier`) → `cli_types.rs`, unit-test module → `core_tests.rs` (via `#[path]`); satisfies the 800-line hard limit |, | done (true-no-op refactor; covered by full regression suite + 110 bin/integration tests) |
| 37+ | sanity-scan multi-rename support; diarization-mix-review kind (operator workflow design needed); newtype threading at struct seams (deferred simplify finding); `apply_decision` arm dedup + per-kind `OperatorDecision` sub-enums | L3 + L4 | pending |

### Phase B, speaker-id pipeline (cycles 9-16)

These cycles add `chatter speaker-id` and its three modes.

| # | RED | GREEN |
|---|---|---|
| 9 | `speaker_id_explicit_basic`, L3 against an anonymous-2-speaker donor with `--mapping "PAR0=drop,PAR1=INV:Investigator"`, asserts output has only INV utts | Stub `chatter speaker-id` subcommand. Implement `parse_mapping_spec` + `apply_mapping`. Reference mode and override-file mode return `unimplemented!()` for now. |
| 10 | `apply_mapping_byte_stable_except_prefix` + `apply_mapping_rewrites_participants` + `apply_mapping_rewrites_id`, L2 | Tighten `apply_mapping` per header rewrite rules. |
| 11 | `identify_mapping_clean_winner`, L2 with a fixture where one donor speaker overwhelmingly matches the reference | Implement `text_cleaner` + `jaccard` modules. Implement `identify_mapping` using them. Reference mode in CLI now works. |
| 12 | `identify_mapping_borderline_refuses`, L2 with a borderline fixture | Add `ConfidenceThreshold` check + `LowConfidence` error path. |
| 13 | `speaker_id_reference_low_confidence_exits_4`, L3 against borderline fixture | Map `LowConfidence` to exit code 4 in the CLI; print scores to stderr. |
| 14 | `speaker_id_reference_writes_override`, L3 with `--write-override` | Implement `OverrideFile::read_or_default` + `OverrideFile::write`. |
| 15 | `speaker_id_override_file_replay`, L3 with `--override-file` + `--session-id` | Implement override-file mode in CLI (`OverrideFile::get` + apply). |
| 16 | Token-cleaner L1 specs (a handful of representative `clean-*` specs from L1.1) + current `spec/tools` generators | Move the regex-and-string cleaner into a spec-test-covered implementation. Specs become the regression net. |

End of Phase B: full `chatter speaker-id` + `chatter merge`
pipeline works auto + explicit + override modes.

### Phase C, adjudication (cycles 17-22)

These cycles add the `chatter adjudicate` tool and its
prompter-injection testability.

| # | RED | GREEN |
|---|---|---|
| 17 | `adjudicate_empty_pending_file_noop`, L4 against an empty pending file, asserts exit 0 + no changes | Stub `chatter adjudicate` subcommand. Implement `PendingAdjudications::read` + `run_adjudication` core skeleton with a no-op `Prompter` trait. |
| 18 | `prompter_scripted_returns_decisions_in_order`, L4 | Implement `ScriptedPrompter::from_decisions` (in-memory) per the `Prompter` trait. |
| 19 | `adjudicate_speaker_id_accepts_suggested`, L4 against `FIX_PENDING_SPEAKER_ID` with one `AcceptSuggested` decision | Implement `apply_decision` for the speaker-id-low-confidence kind. Override file now gets the decision; pending entry removed. |
| 20 | `adjudicate_speaker_id_override_mapping`, L4 with `OverrideMapping` decision | Extend `apply_decision` for the override-mapping variant. |
| 21 | `adjudicate_speaker_id_kind_mismatch_rejected`, L4 with a `OverrideInsertedRole` against a speaker-id pending entry | Implement kind→variants validation in `apply_decision`. |
| 22 | `adjudicate_scripted_mode_unknown_session_aborts` + `adjudicate_scripted_mode_extra_pending_aborts`, L4 | Tighten scripted-mode validation; assert 1:1 mapping between pending entries and scripted decisions. |

End of Phase C: scripted adjudication tested end-to-end with
synthetic operator inputs. Interactive terminal UX still
unimplemented (next phase).

### Phase D, interactive UX (cycles 23-25)

| # | RED | GREEN |
|---|---|---|
| 23 | `prompter_terminal_round_trip_decision`, L4 with mocked stdin/stdout | Implement `TerminalPrompter` parsing `[a]/[o]/[f]/...` keys + optional follow-up prompts. |
| 24 | `adjudicate_resumption_skips_decided_entries`, L4 with a partially-decided override file + full pending list | Implement skip-already-decided logic in `run_adjudication`. |
| 25 | Manual smoke test (NOT automated), run `chatter adjudicate --interactive` against the test fixtures; visually confirm the operator UX matches the doc's mock-up | Polish terminal output: ANSI formatting, fixed-width alignment, the `[m] Show more context` action, the `[p] Play media` action. |

End of Phase D: full v1 pipeline complete.

### Phase E, non-speaker-id adjudication kinds (cycles 26-29)

Each adjudication kind gets its own RED→GREEN cycle.

| # | RED | GREEN |
|---|---|---|
| 26 | `adjudicate_parent_role_overrides_to_mother` + `adjudicate_parent_role_overrides_to_father`, L4 | Implement `parent-role-lookup` kind end-to-end (pending schema, prompter context, decision application). |
| 27 | `adjudicate_diarization_mix_flag_only`, L4 | Implement `diarization-mix-review` kind end-to-end. |
| 28 | `adjudicate_sanity_scan_swap_mapping`, L4 | Implement `sanity-scan-misclassification` kind end-to-end. |
| 29 | `adjudicate_re_adjudicate_preserves_history`, L4 | Implement `--re-adjudicate` flag; add `history` field to `MergeOverride`. |

### Phase F, breadth pass (cycles 30+)

Fill in every remaining test from L1-L4 that hasn't been
written yet. These are coverage-deepening tests, not behavior
adders. The impl from Phases A-E should pass them with at
most minor refactoring; if a test fails meaningfully, that's a
gap in the impl that this cycle closes.

The breadth pass is the only phase where multiple cycles can
proceed in parallel (different contributors take different
test groups). Phases A-E are strictly serial.

### Hard rules during impl phase

- **No test stubs.** Every test in this plan, when written,
  must FAIL before its impl exists and PASS after. Skipped or
  `#[ignore]`-marked tests are not allowed in the regression
  net (use `#[ignore]` only for genuinely slow or
  environment-dependent tests, not for "not implemented yet").
- **No test deletion to make CI green.** If a test that was
  passing starts failing after a refactor, the refactor is
  wrong. Investigate; do not delete the test.
- **Three cycle archetypes, distinguish them.** A cycle is one
  of:
  - **bug-fix**: RED motivates new impl code (cycle N-1's
    impl truly cannot satisfy the new test).
  - **regression-guard**: RED pins an invariant the impl
    inherits from upstream infrastructure (e.g. parse→serialize
    byte-stability inherited from `talkbank-parser`). The test
    passes against cycle N-1's impl, but the cycle is valuable
    because it locks in the invariant against future
    "optimizations" that might break it. Verbose-output the
    actual behavior on first run to confirm the invariant holds
    for the *right reasons*, not by accident.
  - **true no-op**: RED tests something already pinned
    elsewhere. These ARE unnecessary; drop the cycle or
    sharpen the test.
  The difference between regression-guard and true no-op is
  whether the invariant is *named explicitly* anywhere else.
  If yes (e.g., the parser crate already has a roundtrip
  test that covers it), the cycle is true-no-op. If no, the
  cycle is a regression-guard and worth keeping.
