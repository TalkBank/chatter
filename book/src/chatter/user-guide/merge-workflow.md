# Review Tools (`adjudicate`, `sanity-scan`)

**Last modified:** 2026-09-24 00:21 EDT

These experimental tools review speaker decisions and inspect existing output.
The former `merge`, `pipeline`, and `batch` commands have been
[removed](merge.md). These review tools do not replace event correspondence.
Use `speaker-id --write-pending` to prepare unresolved speaker decisions.

## `chatter adjudicate` (the operator step)

Reads the pending file a pass produced, walks the operator through the
unresolved sessions, and appends the resolved decisions to the override
file. On success the pending file is rewritten to drop the entries that
were resolved, so re-running adjudicate only ever shows what is left.

```text
chatter adjudicate <PENDING> --override-file <FILE> [--interactive | --scripted <TOML>]

ARGUMENTS:
  <PENDING>  The pending-adjudications TOML a pass wrote.

REQUIRED:
  --override-file <FILE>  Override file to append resolved decisions to
                         (created if absent). This is the same file pass 2
                         reads back.

DECISION SOURCE (one of):
  --interactive           Prompt per pending entry on stdin. See "The
                         interactive decision language" below for the
                         three decision verbs and their syntax.
  --scripted <TOML>       Pre-canned operator decisions, for replayable /
                         tested runs. Mutually exclusive with --interactive.

  --operator <NAME>       Recorded in each override entry (defaults to $USER).
```

### The interactive decision language

Each pending entry is printed with its full context (the sessions, the
suggested mapping, the engine's confidence scores and reasoning), then
one line is read from stdin. Three decision verbs are accepted:

| Verb | Form | Meaning |
|---|---|---|
| `accept` (or `a`) | `accept [note...]` | Take the suggested mapping exactly as proposed |
| `choose` | `choose SPK:CODE:TAG [SPK:CODE:TAG ...] [note...]` | Supply the speaker mapping yourself: each group maps a donor speaker to a CHAT code and role tag |
| `override` | `override SPK:CODE:TAG [SPK:CODE:TAG ...] SPK=action [SPK=action ...] [note...]` | Supply the mapping AND per-speaker actions (for example `SPK=drop` to exclude a donor speaker entirely) |

`SPK:CODE:TAG` groups are repeatable, so multi-adult sessions are
expressed naturally, one group per speaker:

```text
choose A:CHI:Target_Child B:INV:Investigator C:MOT:Mother reviewed against the recording
```

Anything after the structured arguments is recorded verbatim as the
operator's note. Every decision (verb, mapping, note, operator, and the
engine's original scores) is appended to the override file, so the audit
trail survives the session.

This is the interactive review tool the `speaker-id` page
refer to: the audit trail (who decided, the scores, any note) lands in
the override file so a later reader can see *why* a session was labeled
the way it was. The decision schema is the same override-file format
used everywhere in the workflow; see
[Merge Override File Format](../integrating/merge-overrides.md), and the
[Adjudication Workflow](../../architecture/adjudication-workflow.md)
architecture page for the design.

## `chatter sanity-scan` (post-merge QA)

A confident auto-decision can still be wrong, the runner-up was simply
even further off. `sanity-scan` re-reads the merged output and the
pass-1 audit file and flags sessions that pass an out-of-band check: the
**mean utterance word count** of the anchor speaker versus the inserted
speaker. In a typical child-language recording the adult out-talks the
child, so an anchor (child) mean that is much *higher* than the inserted
(adult) mean is suspicious, possibly the two were swapped.

```text
chatter sanity-scan <MERGED_DIR> \
  --override-file <FILE> --anchor <SPEAKER> --write-pending <FILE> [OPTIONS]

REQUIRED:
  --override-file <FILE>  The pass-1 audit file. Only auto-decided sessions
                         are scanned; explicit-mode entries are skipped (the
                         operator already signed off).
  --anchor <SPEAKER>      Anchor code in the merged files (typically CHI).
  --write-pending <FILE>  Flagged sessions are appended here as
                         sanity-scan-misclassification pending entries for
                         `chatter adjudicate`. Required.

  --threshold <F>         Flag when anchor_mean >= inserted_mean * threshold
                         (default 1.5).
```

A flag is a question, not a verdict: the session goes back into the
adjudication queue for an operator to confirm or correct. Whether to run
the scan at all is a judgment about the corpus. It assumes the typical
"adult out-talks child" shape, and is unreliable where that inverts
(e.g. a clinical-interview corpus where children out-narrate the adult);
there, review speaker identity using appropriate evidence instead.
