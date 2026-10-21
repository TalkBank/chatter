# Merge Pipeline, Crate Architecture

**Status:** Current
**Last modified:** {{git-dates:page}}

The current structural merge is a library in `talkbank-transform`.
`chatter merge`, `chatter pipeline` and `chatter batch` are not CLI commands.
See the [merge user guide](../chatter/user-guide/merge.md) for that boundary.

Speaker identification, adjudication and sanity scanning are related tools,
not proof that a pair of recordings is admissible for merging. This chapter
describes library ownership and transitions, not a corpus execution workflow.

## Ownership

| Responsibility | Source owner |
| --- | --- |
| CHAT values and validation | `talkbank-model` |
| Parsing CHAT source | `talkbank-parser` and the chosen backend |
| Structural assembly, origins and fates | `talkbank-transform/src/transcript_merge.rs` |
| Ordered-source event placement | `transcript_merge/ordered.rs` |
| Source-bound donor selection | `transcript_merge/selection.rs` |
| Lexical identification and relabeling | `speaker_id/identify.rs`, `mapping.rs`, `apply.rs` |
| Override-file representation and I/O | `speaker_id/override_file.rs` |

Paths in the final five rows are relative to `crates/talkbank-transform/src/`
where not already qualified. These modules reuse model types; there is no
separate `talkbank-model::merge` type hierarchy.

## Assembly, validation and reporting

```text
source-bound inputs and explicit placement policy
    -> MergeDraft
    -> validate (consumes draft)
    -> Merged
    -> report (offers notices to caller's sink)
    -> Reported
    -> borrowed model or consumed editable ChatFile
```

The ordinary `merge_chat_files` and `merge_chat_files_by_source_order`
entry points assemble and validate before returning `Merged`.
The selected-donor draft API exposes the intermediate state when the caller
needs to inspect unresolved order reviews or make recorded terminal-bullet edits.

`MergeDraft::validate` applies model validation including tier alignment.
Failure returns `MergeError::InvalidOutput`, not a reportable success.
`Merged` retains output origins, source fates, placement reviews and bullet
edits. `report` makes notices available to the caller; it cannot prove that
a human read them or that the sink retained them.

`Reported::into_file` returns an editable model, not an irrevocable validity
certificate. Subsequent edits require fresh validation.

## Source order, not a flattened global sort

The merge preserves source event structure. Reference headers and selected
utterances remain related to their original source order; donor metadata has
its own admission rules. Header placement is not interchangeable with sorting
all utterances by start time and adding headers afterward.

The source-order entry point uses recorded anchors to determine cross-source
placement without adding bullets to untimed speech. Incomparable frontiers
refuse. Equal recorded starts use reference-first serialization, not a claim
of acoustic precedence; missing starts are not time ties.

An explicitly requested flagged draft policy can retain unresolved frontiers
with `DraftOrderReview` records. Those records are not generated transcript
comments, and successful model validation does not settle their ordering.
See the [user guide](../chatter/user-guide/merge.md) for this distinction.

## Admission and selection

Declared donor languages must be a subset of the reference declarations.
The ordinary merge requires retained reference speech; the selected-donor API
also supports its explicitly admitted header-only reference case.
Conflicting speaker/header situations return typed errors rather than silently
choosing a version.

`SourceBoundDonorSelection` ties selection and placement evidence to source
documents. Do not transfer an ordinal or review result to an unrelated source
because its numbers happen to fit. The source-bound interfaces are described
in [Domain Types](merge-domain-types.md).

Tier stripping is explicit policy. The library's default list contains
`wor`, `mor`, `gra` and `pho`; callers must choose the appropriate list for
their application rather than assume that default is a corpus rule.

## Limits and verification

These operations do not establish common recording identity, source authority,
speaker identity, acoustic alignment or permission to publish. Those decisions
remain outside structural assembly.

See [Test Plan](merge-test-plan.md) for actual test owners and evidence limits,
and [Domain Types](merge-domain-types.md) for the current vocabulary. Do not
restore removed CLI orchestration from examples in older design history.
