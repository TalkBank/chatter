# Merge Override File Format

**Status:** Current
**Last modified:** {{git-dates:page}}

Override files are caller-selected UTF-8 TOML documents used by
[`speaker-id`](../user-guide/speaker-id.md) and
[adjudication](../../architecture/adjudication-workflow.md).
They preserve selected decision fields, not an append-only history or a complete
copy of the pending evidence. The Rust owner is
`crates/talkbank-transform/src/speaker_id/override_file.rs`.

## File format

The document contains `schema_version = 2` and session-keyed tables.
Session IDs are strings, not bound recording identities. Quote a TOML key
when it contains characters requiring quoting.

This complete synthetic example demonstrates the wire shape, not an actual
speaker decision:

```toml
schema_version = 2

[session-demo]
mode = "explicit"
adult_roles = { PAR1 = { code = "INV", tag = "Investigator" } }
mapping = { PAR0 = "drop", PAR1 = "rename" }
operator = "operator"
decided_at = "2026-01-01T12:00:00-05:00"
note = "Synthetic example only"
```

## Per-session entry fields

| Field | Representation and boundary |
| --- | --- |
| `mode` | Required: `auto`, `explicit` or `override`; informational when applying a mapping |
| `adult_roles` | Required map keyed by donor code; each role has string `code`, `tag`, optional `specific_role` |
| `mapping` | Required map from donor code to `rename` or `drop` |
| `operator` | Required free-form string, not authenticated identity |
| `decided_at` | Required offset-bearing RFC 3339 timestamp; writer uses quoted New York time at whole seconds |
| `scores` | Optional scalar score map; empty by default |
| `margin` | Optional numeric value; numeric infinity can represent positive winner/zero runner-up |
| `note` | Optional string; absent and explicitly empty remain distinct |
| `flags` | String array, empty by default; not a typed flag enum |
| `engine` | Defaults to `deterministic`; alternative `llm` |
| `judgment` | Optional judgment provenance; engine/provenance coupling is not type-enforced here |

Score and margin fields are raw wire numbers, not the checked lexical-evidence
types. The margin reader does not accept a string `"unbounded"`.
The typed JSON match report uses different tagged margin states; see
[Speaker-ID](../user-guide/speaker-id.md#match-evidence-report).

A role's `specific_role` supplies the participant name/specific-role field
during mapping. It is optional; the wire type does not enforce a particular
multi-adult naming convention.

## Mapping semantics

Conversion through `MergeOverride::to_mapping_spec` requires role data for
every rename. Applying that mapping removes dropped speakers' utterances and
participant/ID entries, and rewrites renamed speakers and corresponding headers.

Unmapped speakers currently pass through unchanged. Neither the file reader
nor mapping application proves that every input speaker is covered, that every
mapping key occurs in the input, or that targets are unique. The transformed
model is serialized without a new output-validity proof. Validate and review
the result before further use; lexical admission is not identity adjudication.

Flags are preserved as strings. Labels such as `diarization-mixed` or
`best-guess` do not invoke a built-in safety policy or authorize publication.

## Reader semantics

`OverrideFile::read_or_default` returns an empty default only when the path
is missing. Existing unreadable/malformed files fail. A missing version is a
TOML deserialization error; an unsupported numeric version yields
`UnsupportedSchemaVersion`. There is no automatic migration.

Unknown fields in session records are not globally rejected. Do not rely on
successful reading to detect misspelled optional fields or to enforce
mode/score and engine/judgment relationships. Conversion to a mapping is a
separate boundary after reading.

In CLI replay, a missing file produces an empty document and therefore a
missing-session refusal; it does not produce a usable default mapping.

## Writer semantics

The writer serializes session entries in `BTreeMap` order, writes and flushes
an exclusively created temporary sibling, then renames it over the destination.
Pending-adjudication files use the same writer. An existing `*.toml.tmp` file,
hard link or symlink is never reused or removed as staging space. Abandoning a
prepared write or failing to publish removes only its owned temporary file.

This is replacement, not append-only storage, a multi-file transaction,
concurrent-writer protection or a crash-durability guarantee. Replacement uses
the new file's permissions rather than preserving the destination's metadata;
on Unix, the staging file is created with owner-only permissions.
Read/modify/write does not preserve TOML comments or original formatting.

`upsert` replaces an existing session entry. Reference-mode
`speaker-id --write-override` derives the key from the input basename stem,
records the environment's operator name (or `unknown`), and writes before
the relabeled CHAT output is written. A later output failure can therefore
leave the decision file changed.

Explicit mapping rejects `--write-override`. Use adjudication to record an
operator decision. Override replay reads and applies an entry; it does not
rewrite its mode or timestamp merely because it was replayed.

Adjudication records explicit operator decisions and retains selected lexical
scores/margin and the chosen note. It does not retain every pending field,
original suggestion, scan reason or judgment provenance. Preserve the pending
input separately when that evidence is needed.

## Future schema changes

Version one is unsupported; version two uses per-speaker `adult_roles`.
Do not automatically convert older decisions by inventing role assignments.
Retain the original and arrange an explicit migration or fresh review appropriate
to its evidence.

Backward-compatible defaults do not establish universal cross-version
compatibility. Verify each reader/writer pair before claiming it. The override
type does not currently export a generated JSON Schema; the CHAT model's
[JSON Schema](json-schema.md) is not an override-file validator.

The pending format has its own versioned reader. The scripted decision reader
has a different limitation; see
[Adjudication Workflow](../../architecture/adjudication-workflow.md#the-scripted-mode).
