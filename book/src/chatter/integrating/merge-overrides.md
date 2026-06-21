# Merge Override File Format

**Status:** Draft
**Last updated:** 2026-06-15 15:00 EDT

The merge override file is the typed, human-readable record of
operator decisions in the `chatter speaker-id` →  `chatter merge`
pipeline. It serves three purposes:

1. **Persistence**: operator adjudications made for one batch can
   be replayed on later runs without re-prompting (`chatter
   speaker-id --override-file <FILE> --session-id <ID>`).
2. **Audit trail**: each entry records who decided what, when,
   and on the basis of which Jaccard scores. Years later, a
   researcher can answer "why was PAR0 labeled INV in this
   session?" by reading the file.
3. **Interchange**: an adjudication UI (CLI, future web app) and the
   batch pipeline share the same file format; UI tools can be added
   or replaced without changing the on-disk contract.

This page is the authoritative reference for the file's schema.
For the *usage* contract (which commands read/write it, when, why),
see [`chatter speaker-id`](../user-guide/speaker-id.md).

## File location and naming

The file's location is **caller-chosen**. The convention is one
file per donor batch, named for the batch:

```text
batch-2026-05-27-childes-eng.overrides.toml
batch-2026-06-15-fluency-pilot.overrides.toml
batch-2026-08-22-aphasiabank-bilingual.overrides.toml
```

Pipeline operators pass the path explicitly via `--override-file`;
no implicit search of a default location.

## File format

UTF-8 TOML. The file has exactly one top-level key,
`schema_version`, followed by zero or more session entries, each
keyed by a session ID.

```toml
schema_version = 1

[<session_id_1>]
mode = "auto"
# ... fields per entry ...

[<session_id_2>]
mode = "explicit"
# ... fields per entry ...
```

The session ID is the table name. It is a free-form stable string,
typically the basename stem of the CHAT file the entry applies to
(`s12-t1`, `Corpus2024-session-07`, etc.). The TOML parser treats it
as a key; CHAT-conformant identifiers fit the unquoted-key grammar
and need no escaping, but any string is permitted if it conforms
to TOML key syntax (use quoted keys like `"unusual_session-id"`
if the ID contains non-bare-key characters).

## Top-level fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `schema_version` | unsigned integer | yes | The schema version this file conforms to. Currently `1`. Readers refuse files with any other value. |

The reader **refuses** files with `schema_version` absent or
unknown, returning a typed error
(`OverrideFileError::UnsupportedSchemaVersion`). There is no
implicit version, no fallback, no auto-migration. Operators of a
file written by a newer version of `chatter` must upgrade their
binary; operators of a file written by an older version that the
current binary no longer supports must re-adjudicate. This policy
is documented in
[`architecture/merge-domain-types.md` §6](../../architecture/merge-domain-types.md#6-schema-versioning-policy-strict-refuse-with-clear-error);
its rationale is to keep the schema honest and avoid premature
migration code that might silently misinterpret old data.

## Per-session entry fields

Each `[<session_id>]` table contains the fields below. Required
fields must be present and well-typed; optional fields may be
omitted; unknown fields cause a parse error.

### Required fields

| Field | Type | Meaning |
|---|---|---|
| `mode` | string enum | One of `"auto"`, `"explicit"`, `"override"`. How the decision was made; see "Mode semantics" below. |
| `inserted_role` | inline table | The CHAT identity assigned to every speaker whose `mapping` action is `"rename"`. Fields: `code` (string, CHAT speaker code), `tag` (string, CHAT role-tag). |
| `mapping` | inline table | Map from input speaker codes to actions. Keys are speaker codes; values are `"rename"` or `"drop"`. Every speaker that exists in the input CHAT file must appear in `mapping`. |
| `operator` | string | Free-form identifier of the person who created the entry (username, initials, email prefix). Recorded as audit trail. |
| `decided_at` | RFC 3339 datetime | When the decision was made. Must include a time zone (UTC recommended). |

### Optional fields

| Field | Type | Default | Meaning |
|---|---|---|---|
| `scores` | inline table | `{}` | Per-speaker Jaccard scores recorded at decision time. Keys are speaker codes; values are floats in `[0.0, 1.0]`. Populated when the decision was based on a reference-mode auto attempt (even if the final mode is `"explicit"` because the operator overrode a low-confidence result). |
| `margin` | float or string | absent | The decisive margin (winner-score / loser-score). Finite values serialize as numbers; the divide-by-zero case (loser score = 0) serializes as the string `"unbounded"`. |
| `note` | string | `""` | Free-text operator note. **Strongly recommended** for `"explicit"` and `"override"` modes, captures *why* the operator made the call. |
| `flags` | array of strings | `[]` | Operator-supplied flags marking unusual situations. Known values listed in "Flag vocabulary" below; unknown strings are preserved verbatim (treated as `Custom`). |
| `engine` | string enum | `"deterministic"` | Which engine produced the decision. Always written on new entries; absent only in pre-provenance files, which read as `"deterministic"`. One of `"deterministic"` (Jaccard reference-mode, spreadsheet, or operator adjudication) or `"llm"` (language-model judgment). |
| `judgment` | inline table | absent | LLM audit trail. Present only when `engine = "llm"`; omitted for deterministic decisions. Sub-fields documented below. |

#### `judgment` sub-table fields

The `judgment` inline table records the audit trail for LLM-produced
decisions. It is present if and only if `engine = "llm"`.

| Field | Type | Required | Meaning |
|---|---|---|---|
| `model` | string | yes | Model identifier used for the judgment (e.g. `"deepseek-v4-flash"`). |
| `endpoint` | string | yes | OpenAI-compatible base URL the judgment was made against. |
| `prompt_version` | string | yes | Prompt-template version tag (e.g. `"v1"`). Bumping this marks older entries as produced by a prior template. |
| `confidence` | inline table | no (omitted when empty) | Per-field model confidence in `[0.0, 1.0]`. Keys are decision field names (e.g. `"mapping"`, `"roles"`, `"merge_applicable"`). Omitted entirely when no confidence values were reported. |
| `reasoning` | string | yes | One or two sentence model rationale for the decision. |

## Mode semantics

The `mode` field records how the decision was made and is
informational only at read time, every mode applies the same
`mapping` deterministically. Distinguishing modes matters for
audit purposes.

| Mode | Set when | Operator confidence |
|---|---|---|
| `"auto"` | `chatter speaker-id` ran in reference mode, Jaccard margin was at or above `--confidence-threshold`, and the operator did not intervene. | High; the algorithm picked. |
| `"explicit"` | The operator supplied `--mapping` directly, typically after a prior reference-mode attempt failed at the confidence threshold. | Operator made the call; confidence depends on what evidence they used (listening to audio, contributor data sheet, prior knowledge). |
| `"override"` | The entry was created by reading a prior override file (replay). | Inherited from whichever prior decision the entry was first stamped with. The `mode` is updated to `"override"` whenever a replay re-writes the entry. |

The reader does not enforce mode → field correlations (e.g., it
does not require `scores` to be present when `mode = "auto"`). The
writer follows these conventions:

- `"auto"` entries always include `scores` and `margin`.
- `"explicit"` entries include `scores` and `margin` IFF a prior
  reference-mode attempt produced them; otherwise they are absent.
- `"override"` entries preserve whatever `scores`, `margin`, and
  `note` were in the source file.

## Mapping semantics

Each entry in `mapping` is one of:

- `"rename"`: the speaker is renamed to
  `inserted_role.code` with role tag `inserted_role.tag` in the
  output CHAT file. Every utterance for this speaker has its
  `*CODE:` prefix rewritten; the `@Participants` entry for this
  speaker has its code + role-tag rewritten (preserving any
  intervening name); the `@ID` row's code (field 3) and role
  (field 8) are rewritten.
- `"drop"`: the speaker's utterances are removed from the
  output entirely. The speaker's `@Participants` entry and `@ID`
  row are also removed.

**Precondition.** Every speaker that appears in the input CHAT
file must appear in `mapping`. There is no defaulting; omission
is rejected with
`SpeakerIdError::SpeakerNotInMapping { speaker }`. This is by
design: every decision must be explicit, so a future reader
knows that no speaker was silently passed through.

The reader rejects:

- Mapping entries whose key is not a speaker present in the input
  (`SpeakerIdError::MappingSpeakerNotInInput`).
- Mapping values other than `"rename"` or `"drop"` (TOML parse
  error from the typed deserializer).

## Flag vocabulary

The `flags` array contains zero or more string values. The
following are recognized vocabulary; consumers MAY treat them
specially:

| Flag | Meaning |
|---|---|
| `"diarization-mixed"` | The ASR diarization label being renamed actually contains multiple real-world speakers (e.g., clinician + parent collapsed). The rename is the best available approximation; downstream consumers should know the output is imperfect. |
| `"best-guess"` | The operator could not confidently determine which speaker is which (e.g., from audio alone). The mapping is recorded as best-guess and merits review by a domain expert before publication. |

Any other string is preserved verbatim as a contributor-specific
flag (`Custom(String)` in the Rust type). Consumers SHOULD NOT
crash on unknown flags but MAY surface them in audit-trail
displays.

The order of flags within an entry is not semantically meaningful;
duplicates are tolerated but considered noise. Tooling that
modifies the list SHOULD deduplicate.

## Reader semantics

`OverrideFile::read(path)` is the canonical reader. Its behavior:

1. Open `path` UTF-8.
2. Parse via `toml`.
3. Refuse if `schema_version` is absent or not equal to the
   binary's `CURRENT_SCHEMA_VERSION` (currently `1`). Error:
   `OverrideFileError::UnsupportedSchemaVersion { found, supported }`.
4. Parse all `[<session_id>]` tables into `MergeOverride` values;
   reject unknown fields.
5. Return `OverrideFile { schema_version, entries }`.

`OverrideFile::read_or_default(path)` is the variant used by
`chatter speaker-id --write-override`: if the file does not
exist, returns `OverrideFile::default()` (empty, current schema
version); otherwise behaves as `read`.

`OverrideFile::get(&session_id)` retrieves a single entry;
returns `None` if absent.

## Writer semantics

`OverrideFile::write(path)` serializes the file deterministically:

- Top-level field order: `schema_version` first.
- Entries ordered by session ID alphabetically (`BTreeMap`
  default).
- Per-entry field order: `mode`, `inserted_role`, `mapping`,
  `scores`, `margin`, `operator`, `decided_at`, `note`, `flags`,
  `engine`, `judgment`.
- Optional fields omitted when empty / absent.
- Atomic replace: writes to `<path>.tmp` then renames over
  `<path>` to avoid leaving a partial file on crash.

`chatter speaker-id --write-override <path>` appends a single
entry: it reads the file (or starts empty), inserts/updates the
entry for the current session, and writes back. The session ID
defaults to the input CHAT file's basename stem unless
overridden via `--session-id`.

## Example: minimal auto-mode entry

```toml
schema_version = 1

[session-101-t1]
mode = "auto"
inserted_role = { code = "INV", tag = "Investigator" }
mapping = { PAR0 = "rename", PAR1 = "drop" }
scores = { PAR0 = 0.1931, PAR1 = 0.7347 }
margin = 3.81
operator = "alice"
decided_at = 2026-05-27T08:41:00-04:00
```

The reader reconstructs: child speaker was `PAR1` (high Jaccard
match with reference's `CHI`); auto-decide succeeded with margin
3.81×; `PAR0` becomes `INV:Investigator` in the output.

## Example: operator-adjudicated entry

After a low-confidence refusal, the operator listened to the
audio, confirmed the call, and re-ran with `--mapping`:

```toml
[session-102-t1]
mode = "explicit"
inserted_role = { code = "INV", tag = "Investigator" }
mapping = { PAR0 = "drop", PAR1 = "rename" }
scores = { PAR0 = 0.6286, PAR1 = 0.3457 }
margin = 1.82
operator = "alice"
decided_at = 2026-05-27T11:15:00-04:00
note = "Auto refused at 2.0× threshold. Listened to first 60 seconds; PAR0 produces child-content matching the hand transcript. PAR1 introduces herself as the clinician."
```

The scores from the prior auto attempt are preserved; the note
captures *why* the operator was confident in the call despite
the close margin. Years later, a researcher can verify by
listening to the same 60 seconds and confirming the operator's
observation, the audit trail is reproducible.

## Example: diarization-mixed parent sample

```toml
[session-103-t1-parent]
mode = "explicit"
inserted_role = { code = "MOT", tag = "Mother" }
mapping = { PAR0 = "rename", PAR1 = "drop" }
scores = { PAR0 = 0.3727, PAR1 = 0.6940 }
margin = 1.86
operator = "alice"
decided_at = 2026-05-27T11:22:00-04:00
note = "Parent sample. Per contributor data sheet: mother. PAR0 contains clinician intro + parent mixed (Batchalign diarization limitation)."
flags = ["diarization-mixed"]
```

The `flags = ["diarization-mixed"]` warns downstream consumers
that the renamed `MOT` speaker is not a clean parent-only stream
the first ~15 seconds were the clinician giving setup
instructions before leaving the room. The `note` captures the
specifics for future review.

## Example: replayed entry

The same file run on a different day from the override file:

```toml
[session-102-t1]
mode = "override"
inserted_role = { code = "INV", tag = "Investigator" }
mapping = { PAR0 = "drop", PAR1 = "rename" }
scores = { PAR0 = 0.6286, PAR1 = 0.3457 }
margin = 1.82
operator = "alice"
decided_at = 2026-05-27T11:15:00-04:00
note = "Auto refused at 2.0× threshold. Listened to first 60 seconds; PAR0 produces child-content matching the hand transcript. PAR1 introduces herself as the clinician."
```

`mode` becomes `"override"` whenever the entry is re-applied by
reading the file. The other fields (including the original
`operator` and `decided_at`) are preserved, the override file
is the audit trail of the *original* decision, not of the
replay.

## TOML grammar reference

For consumers writing the file by hand or generating it from
other tools, the grammar is standard TOML 1.0
([toml.io](https://toml.io/en/v1.0.0)) with the following
domain-specific conventions:

- Datetimes use RFC 3339 with explicit time zone. UTC offset
  `Z` and offsets like `-04:00` are both accepted.
- Floats: standard TOML float syntax. The `margin` field accepts
  either a float or the string `"unbounded"`.
- Tables vs inline tables: top-level `[<session_id>]` tables
  may use either standard or inline syntax; the writer emits
  standard tables for readability.
- Comments: TOML `#` line comments are permitted anywhere; the
  reader ignores them. The writer does not preserve comments
  across read-modify-write cycles (`toml`, not `toml_edit`);
  hand-edited comments may be lost on subsequent
  `--write-override` runs. If preserving comments becomes
  important, the writer can be swapped for `toml_edit` in a
  future release.

## Future schema changes

Schema version increments will appear here under "Migration" with
the version-to-version diff and migration instructions. Until
then, this is the only schema; the policy is strict
refuse-with-clear-error on any other `schema_version` value.

### 2026-06 additive fields: `engine` and `judgment` (no version bump)

The `engine` and `judgment` fields were added in 2026-06 to record
decision provenance (deterministic vs LLM). This addition did NOT
increment `schema_version` because both fields are backward
compatible in both directions:

- **Old reader, new file:** TOML `deny_unknown_fields` is not set
  globally; older binaries that parse a file containing `engine` and
  `judgment` will silently ignore the unknown keys. The decision
  itself (mode, mapping, inserted_role) is unaffected.
- **New reader, old file:** `engine` has `#[serde(default)]` and
  defaults to `"deterministic"`; `judgment` has
  `skip_serializing_if = "Option::is_none"` and is absent, which
  deserializes as `None`. Pre-provenance files are therefore readable
  without error and are treated as deterministic decisions.

A future version bump would be warranted only if a change makes old
files unreadable or misinterpretable, neither of which applies here.

## Relationship to JSON Schema

The Rust `OverrideFile` type is implemented (in
`talkbank-transform`, `src/speaker_id/override_file.rs`) and drives
the override-file replay workflow today. What is not yet built is its
JSON Schema export: `OverrideFile` does not yet derive
`schemars::JsonSchema`, so no schema is generated, and the canonical
URL `https://talkbank.org/schemas/v0.1/merge-overrides.json` is
reserved but not yet published. Exposing it follows the same
`schemars`-based generator pattern documented in
[JSON Schema](./json-schema.md).

The TOML form is the on-disk format; JSON Schema is the
machine-readable spec for external tooling. Both describe the
same `OverrideFile` Rust type.
