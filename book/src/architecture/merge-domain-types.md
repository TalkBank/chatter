# Merge Pipeline, Domain Types

**Status:** Current
**Last modified:** {{git-dates:page}}

This page describes shipped boundaries, not proposed Rust declarations.
The exact APIs live in `talkbank-transform`; the underlying CHAT vocabulary
comes from `talkbank-model`. See [Merge Architecture](merge-architecture.md)
for ownership and the [speaker-id guide](../chatter/user-guide/speaker-id.md)
for command usage.

## Where the types live

Structural merge types live in
`crates/talkbank-transform/src/transcript_merge.rs` and its submodules.
Speaker evidence, mappings and override records live in
`crates/talkbank-transform/src/speaker_id/`.
They reuse `ChatFile`, `SpeakerCode`, `ParticipantRole` and the model's
header types; they do not redefine those values.

## Designed vs shipped (quick map)

Older design names are not APIs:

| Earlier design name | Current representation |
| --- | --- |
| `RetainSet` | Merge entry points accept `&[SpeakerCode]` |
| `InsertedRole` | Override records use `InsertedRoleSpec` |
| `MappingAction` | Override records use `SpeakerAction` |
| `DecisionMode` | Override records use `OverrideMode` |
| Single shared inserted role | Schema-v2 `adult_roles` carries per-speaker roles |
| Two-state `Margin` | `ConfidenceMargin` also represents no information |
| Typed `MergeFlag` | Override flags remain `Vec<String>` |
| Universal session/operator newtypes | Override keys and operator fields remain strings |

A type with a similar name in the judgment submodule does not make every
override-file field validated through it.

## Structural merge states

`ReferenceIdx` and `DonorIdx` distinguish source utterance ordinals.
`MergeOrigin` identifies each emitted utterance's source; source fates also
account for utterances that were not emitted. Neither is a recording identity.

`SourceBoundDonorSelection` binds selection to its donor. Its relative-order,
timed gem-exterior and flagged-draft-order attachments enforce their own source
relationships. The selected-source API should retain these relationships
rather than serialize bare ordinals and reuse them with another document.

`MergeDraft` owns an assembled but unvalidated model.
Its `set_terminal_bullet` operation requires an existing terminal bullet and
records original assembled timing versus replacement timing. Restoring the
assembled timing removes that edit record; this is not an acoustic aligner.

`validate` consumes the draft and returns `Merged` only after model policy,
including tier alignment, accepts it. `Merged::report` yields `Reported`
after offering notices to the caller's sink. Consuming `Reported` into an
editable `ChatFile` relinquishes the validity boundary for future edits.
An unresolved draft-order review remains a review obligation even if the
assembled model passes validation.

## Lexical evidence and confidence

`JaccardScore` is derived internally from admitted multiset counts; it does
not expose the old proposed fallible scalar constructor.
Its display value is not a speaker-identity certificate.

`ConfidenceMargin` has three distinct states:

| Leading scores | State | Threshold outcome |
| --- | --- | --- |
| Both zero | `NoInformation` | Never meets the threshold |
| Both positive | `Finite` | Compare the ratio with the threshold |
| Positive winner, zero runner-up | `Unbounded` | Meets an admitted threshold |

`ConfidenceThreshold::new` admits finite values at least one; the default
is two. These rules avoid NaN/infinity sentinels and distinguish absence of
evidence from an uncontested positive match.

Identification requires anchor utterances in the reference and at least two
distinct donor utterance speakers. Stable score ordering preserves document
order for ties. Low confidence retains the report rather than discarding
the observed evidence. Absolute lexical support and input-admission failures
must remain distinct from a successful match report.

## Mapping syntax is not output admission

`MappingSpec` maps source speaker codes to `SpeakerAssignment::Drop` or
`Rename`; each rename carries its target and role information.
`parse_mapping_spec` rejects malformed entries and repeated source keys.
That does not establish full source coverage or unique target identities.

Currently, `apply_mapping_chat` passes speakers absent from the map through
unchanged, rewrites participant/ID headers, and serializes a newly assembled
model. It does not return a validated-output typestate. The string entry point
`apply_mapping` parses and validates its input first; that must not be confused
with a fresh validation of the transformed output.

## 6. Schema versioning policy: strict refuse with clear error

The override module's `CURRENT_SCHEMA_VERSION` is two.
`OverrideFile::read_or_default` returns a default document for a missing path,
rejects other I/O or TOML failures, and refuses unsupported schema versions.
That helper's missing-file behavior is not a universal CLI replay policy.

Schema-v2 rename entries need their own `adult_roles` entry when converted
to an in-memory mapping. Override score fields remain scalar wire values:
they are not automatically the checked `JaccardScore` type.

`OverrideFile::write` serializes to a sibling `.toml.tmp` path and renames it.
This is not a multi-writer transaction or a promise of crash-durable storage.
Session entries use a `BTreeMap` for deterministic key order.
The public record still has separate engine and optional judgment fields;
their documented coupling is not enforced by an enum at this boundary.

## Evidence limits

Reference-corpus controls and boundary tests exercise finite lexical, mapping
and serialization cases. Deterministic preparation tests do not exercise an
external judgment provider. Authored context labels do not establish participant
facts or consent, and header age fallback does not choose the intended child
in a multi-age session.

See [Test Plan](merge-test-plan.md) for test ownership. No type or local test in
this chapter establishes common-media provenance, corpus acceptance or
publication authority.
