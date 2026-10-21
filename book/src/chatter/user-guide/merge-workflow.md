# Review Tools (`adjudicate`, `sanity-scan`)

**Last modified:** {{git-dates:page}}

These experimental tools review speaker decisions and inspect existing CHAT.
They do not establish event correspondence or recording identity.
`merge`, `pipeline` and `batch` are not commands; see the
[removal notice](merge.md).

## Prepare for review

Reference-mode [speaker-id](speaker-id.md) can write unresolved decisions with
`--write-pending`. Keep the original pending file and any existing override
file before reviewing. Associate each session ID with the correct source and
external evidence yourself; the pending record does not contain a playable
recording or transcript preview.

Use separate working copies and avoid simultaneous writers. There is no
dry-run flag or automatic decision history.

## `chatter adjudicate`

Choose one of these forms, using your own retained inputs:

```bash
chatter adjudicate pending.toml --override-file overrides.toml \
  --scripted decisions.toml --operator operator
```

```bash
chatter adjudicate pending.toml --override-file overrides.toml \
  --interactive --operator operator
```

On success, decisions replace/insert session entries in the override file,
then the pending file is rewritten without resolved entries.
Review the resulting mappings and notes before applying them to CHAT.

The terminal supports:

| Input | Meaning |
| --- | --- |
| `accept` or `a` | Accept a low-confidence or sanity-scan suggestion; no terminal acceptance note is recorded |
| `choose SPK:CODE:TAG ... [note...]` | Supply roles for a parent-role entry's existing mapping |
| `override SPK:CODE:TAG ... SPK=action ... [note...]` | Replace a suggestion with roles and rename/drop actions |

Choose/override notes are reconstructed from tokens, not stored verbatim.
Use a scripted decision to retain an acceptance note. Unsupported input fails
rather than deferring or reprompting. There is no media player, defer, block,
save-and-quit or flag-only action.

See [Adjudication Workflow](../../architecture/adjudication-workflow.md) for
the scripted format, admitted decision/kind pairs and in-memory retry contract.

## Failure and recovery

Decision failure occurs before either CLI file write: answers already supplied
during that invocation are not persisted. On a write failure, however, the
override file may already have changed while the pending file remains old.
The two writes are not transactional.

Compare both files with the retained originals before retrying. Do not simply
assume exit failure means no change, or that replay creates a history record.
See [Override File Format](../integrating/merge-overrides.md).

## `chatter sanity-scan`

The scan flags a mean-utterance-word-count asymmetry for operator review.
It is not a nearby-text-similarity detector or a speaker-identity oracle.

```text
chatter sanity-scan MERGED_DIR \
  --override-file overrides.toml --anchor CHI --write-pending pending.toml
```

The CLI examines auto-decided entries; explicit decisions are skipped.
The default threshold is 1.5: the heuristic flags when anchor mean is at least
inserted mean times the threshold. Unsupported mapping shapes and absent
observed speaker populations do not supply a swap suggestion.

The threshold must be finite and positive. Invalid values (including NaN,
infinity, zero and negatives) are usage errors before input/output access.
Positive factors below one remain allowed; choosing a meaningful sensitivity
requires evidence for the task, not merely successful numeric admission.

A suggestion is not automatically applied to the transcript or existing
override. The heuristic may be inappropriate for a task where the anchor
naturally produces longer turns. Inspect the evidence, then adjudicate only
if a decision is justified. Successful parsing or a threshold crossing is
not proof that speakers were swapped.
