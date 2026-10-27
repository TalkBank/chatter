# Adjudication Workflow

**Status:** Current
**Last modified:** {{git-dates:page}}

`chatter experimental review` is an experimental review tool. It consumes pending
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
chatter experimental review scripted PENDING.toml --override-file overrides.toml \
  --script decisions.toml --operator operator
```

Alternatively, select the `interactive` subcommand. Both modes require an
explicit `--operator`; interactive review additionally requires terminal stdin
and stderr. It is not an implicit default and cannot be driven through a pipe.
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
out-of-order sessions refuse. When the pending queue ends, any unconsumed
scripted decision also refuses, naming the first unused session. The CLI exits
2 before writing either document; check that the script belongs to this pending
population before retrying. Empty pending and empty scripted lists may complete.

The scripted reader admits versions one and two with the current choice
representation. Other numeric versions, missing versions and incorrectly typed
versions refuse before a prompter is constructed. This scripted version domain
is independent of the pending/override readers, which require version two.
Successful version admission does not prove that the decisions are appropriate
for the pending entries. Successful adjudication additionally requires consuming
all scripted decisions.

## The prompter abstraction (testability)

`Prompter::ask(&PendingEntry)` returns an `OperatorDecision` or
`AdjudicationError`. `Prompter::finish(self)` consumes the source and admits
completion; scripted sources refuse leftover decisions, while the terminal
finishes without reading an extra response. The CLI implements `TerminalPrompter`;
the library provides `ScriptedPrompter::from_decisions` and `read_toml`.
There is no separate `ScriptedTomlPrompter` or proposed context wrapper in
the shipped API.

`run_adjudication` borrows pending and override documents mutably and takes
ownership of the prompter. Its private queue owner advances only after prompting and applying a decision
succeed. On failure it restores the failing entry and later entries in their
original order; earlier decisions remain in the in-memory override document.

A private prepared-decision state checks `to_mapping_spec` before insertion.
A missing rename role therefore refuses without consuming that pending request.
This proves conversion readiness, not full CHAT validity, complete speaker
coverage, unique target codes or correctness of the human decision.
After an application error, retry the retained queue with a newly prepared
prompter. A final unused-decision refusal leaves all resolved entries in the
in-memory overrides and the pending queue empty; it does not roll back applied
decisions. Only completed runs return `AdjudicationOutcome`, whose resolved
session IDs are available through `resolved()` and `resolved_count()`.

## Persistence and recovery

```text
admit distinct pending / override / script file identities
    -> read pending + overrides
    -> adjudicate all entries in memory
    -> write overrides
    -> write remaining pending
```

The pending file, override file and optional decision script must refer to
separate files. Admission compares filesystem identities before reading the
documents or asking for decisions: identical paths, alternate path spellings,
symlinks and hard links to the same file are refused with exit status one.
An absent override file is permitted. Failure to inspect any required identity
also refuses the command. These checks do not lock the files or protect against
concurrent replacement; keep the inputs and paths stable during the operation.

The CLI writes only after the entire in-memory run succeeds. An input/decision
failure before that point writes neither document; earlier answers from that
invocation are not saved to disk. The library's in-memory resume contract
does not imply CLI save-on-quit or process-crash recovery.

Each writer creates an exclusive temporary sibling, completes and flushes the
serialized bytes, then renames that owned file. Existing predictable temporary
names are left alone, including links to other files. Publication failure or
abandonment cleans up only the owned staging file. On Unix, replacement files
use owner-only permissions; existing destination metadata is not preserved.
The pair is not a transaction:
override writing can succeed and pending writing can fail. Concurrent writers
are not coordinated. Existing entries for the same session are replaced, not
appended to a history; repeated pending session IDs can replace an earlier
decision in the same run.

Before real review, retain copies of both files and avoid concurrent writers.
After a write error, compare both destinations with those retained inputs
before retrying; do not infer that neither changed from a nonzero exit status.
Preserve scan reasons, original suggestions and judgment provenance separately
if needed: operator decisions do not copy every pending field into the override.

Exit status is zero on successful completion, one for aliased paths or I/O/TOML/unsupported
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
a threshold crossing is not proof of an identity swap. The CLI requires the
override file, directory and every selected auto-decided transcript to exist;
missing inputs fail instead of becoming an empty or partial successful scan.
Pending output must be distinct from those sources and is staged before
replacement. The scan summary is stdout; diagnostics remain stderr. Scripted
and interactive review produce files, with prompts/diagnostics on stderr and
no stdout result. See
[Review Tools](../chatter/user-guide/merge-workflow.md) and
[Test Plan](merge-test-plan.md) for the relevant owners.
