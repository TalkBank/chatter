# Adjudication Workflow

**Status:** Current
**Last modified:** {{git-dates:page}}

`chatter adjudicate` is an experimental review tool. It consumes pending
speaker decisions and writes override records; it does not relabel CHAT, merge
recordings, run alignment or certify a suggestion. The removed `merge`,
`pipeline` and `batch` commands are not part of this workflow.

See [Review Tools](../chatter/user-guide/merge-workflow.md) for operator usage,
[Override File Format](../chatter/integrating/merge-overrides.md) for persistence,
and [Merge Architecture](merge-architecture.md) for structural assembly.

## Supported decisions

The pending kind determines which decisions can be applied:

| Pending kind | Admitted decisions |
| --- | --- |
| `speaker-id-low-confidence` | `AcceptSuggested`, `OverrideMapping` |
| `parent-role-lookup` | `ChooseRole` |
| `sanity-scan-misclassification` | `AcceptSuggested`, `OverrideMapping` |

A role choice supplies `adult_roles` for the pending entry's existing
`speaker_mapping`; it does not replace that action map.
An override supplies both the action map and role map.
Every rename must have role data before the decision can be committed.

Defer, block, flag-only decisions, diarization-mix adjudication and
re-adjudication history are not implemented. There is no kind filter,
skip-on-error, dry-run, save-and-quit command or built-in media player.
Review recordings and other relevant evidence separately.

## The pending-adjudications artifact

`PendingAdjudications` is a version-two TOML document with an ordered
`entries` array. Each entry has `session_id`, `created_at` and a flattened
`kind` payload. Scores, margin and threshold are optional evidence; engine
defaults to deterministic and judgment provenance is optional.

The kind-specific payload is:

- Low confidence: `suggested.mapping` and `suggested.adult_roles`.
- Parent role: `donor_speaker` and `speaker_mapping`.
- Sanity scan: the suggested mapping/roles plus `reason`.

The current record has no input/reference path, transcript preview or media
location fields. Do not assume the terminal can recover that context from a
session ID. A pending file is not an archive: successful adjudication removes
resolved entries, including evidence not copied into the override record.
Preserve the original pending input when its full evidence matters.

Both pending readers reject unsupported schema versions.
`read` requires an existing file; `read_or_default` starts empty only for a
missing path. These are different contracts.

## Command and terminal behavior

```text
chatter adjudicate PENDING.toml --override-file overrides.toml \
  --scripted decisions.toml --operator operator
```

Alternatively, select `--interactive`. Exactly one decision source is required;
interactive mode is not an implicit default.
The terminal displays the session, kind, relevant mapping/roles or scan reason,
and available lexical scores/margin. It does not display all stored judgment
provenance or play audio.

Terminal input is one line per pending entry:

- `accept` (also `a` or `accept-suggested`) accepts a suggestion.
  This parser records no note for accept, even if trailing text is supplied.
- `choose SPK:CODE:TAG [SPK:CODE:TAG ...] [note...]` supplies roles.
- `override SPK:CODE:TAG [SPK:CODE:TAG ...] SPK=action [SPK=action ...] [note...]`
  supplies roles and actions (`rename` or `drop`).

Choose/override notes are reconstructed from whitespace-separated tokens,
not retained byte-for-byte. Invalid input stops the run; it does not reprompt
or silently defer the entry. Use scripted accept when an acceptance note
must be recorded.

## The scripted mode

This illustrative input is synthetic; it records no actual speaker judgment.
For a pending low-confidence entry named `session-demo`, a matching decision
file is:

```toml
schema_version = 2

[[decisions]]
session_id = "session-demo"
kind = "speaker-id-low-confidence"
choice = { kind = "accept-suggested", note = "Synthetic example only" }
```

The prompter consumes decisions in file order and checks each consumed session
ID against the next pending entry. The pending kind, not the scripted entry's
descriptive kind, determines decision compatibility. Missing decisions and
out-of-order sessions refuse. Unused trailing scripted decisions are not checked.

The scripted reader admits versions one and two with the current choice
representation. Other numeric versions, missing versions and incorrectly typed
versions refuse before a prompter is constructed. This scripted version domain
is independent of the pending/override readers, which require version two.
Successful version admission does not prove that the decisions are appropriate
for the pending entries or that every scripted decision will be consumed.

## The prompter abstraction (testability)

`Prompter::ask(&PendingEntry)` returns an `OperatorDecision` or
`AdjudicationError`. The CLI implements `TerminalPrompter`;
the library provides `ScriptedPrompter::from_decisions` and `read_toml`.
There is no separate `ScriptedTomlPrompter` or proposed context wrapper in
the shipped API.

`run_adjudication` borrows pending and override documents mutably.
Its private queue owner advances only after prompting and applying a decision
succeed. On failure it restores the failing entry and later entries in their
original order; earlier decisions remain in the in-memory override document.

A private prepared-decision state checks `to_mapping_spec` before insertion.
A missing rename role therefore refuses without consuming that pending request.
This proves conversion readiness, not full CHAT validity, complete speaker
coverage, unique target codes or correctness of the human decision.
After an application error, retry the retained queue with a newly prepared
prompter: the earlier prompter may already have consumed its decision.

## Persistence and recovery

```text
read pending + overrides
    -> adjudicate all entries in memory
    -> write overrides
    -> write remaining pending
```

The CLI writes only after the entire in-memory run succeeds. An input/decision
failure before that point writes neither document; earlier answers from that
invocation are not saved to disk. The library's in-memory resume contract
does not imply CLI save-on-quit or process-crash recovery.

Each writer uses a temporary file and rename. The pair is not a transaction:
override writing can succeed and pending writing can fail. Concurrent writers
are not coordinated. Existing entries for the same session are replaced, not
appended to a history; repeated pending session IDs can replace an earlier
decision in the same run.

Before real review, retain copies of both files and avoid concurrent writers.
After a write error, compare both destinations with those retained inputs
before retrying; do not infer that neither changed from a nonzero exit status.
Preserve scan reasons, original suggestions and judgment provenance separately
if needed: operator decisions do not copy every pending field into the override.

Exit status is zero on successful completion, one for I/O/TOML/unsupported
pending-schema errors, and two for decision or mode errors. There is no
deferred-entry exit-four path in this command.

## Evidence and limits

The finite canonical-reference workflow exercises pending TOML roundtrip,
schema refusal, explicit operator acceptance, mapping replay and the
in-memory failure/resume contract. It preserves failure suffixes and refuses
missing rename roles. These checks do not establish a transactional filesystem
or validate actual speaker identity.

The sanity-scan workflow uses mean utterance word-count asymmetry, not nearby
utterance text similarity. Its advisory suggestion needs operator evidence;
a threshold crossing is not proof of an identity swap. See
[Review Tools](../chatter/user-guide/merge-workflow.md) and
[Test Plan](merge-test-plan.md) for the relevant owners.
