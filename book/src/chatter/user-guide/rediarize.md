# Rediarize (`chatter rediarize`)

**Status:** Draft
**Last updated:** 2026-08-30 16:26 EDT

`chatter rediarize` re-attributes utterance speakers in a CHAT file
from an external diarization. Given a transcript whose utterances
carry media time bullets and a JSON file of timestamped speaker turns
produced by a dedicated diarizer (for example pyannote), it reassigns
each utterance's main-tier speaker to the diarization track that
covers the utterance's time span the most, keeping the utterance
content (the words) byte-stable.

The command exists for a specific, common failure shape: ASR systems
with bundled diarization (Rev.AI and others) auto-detect the speaker
count and can under-count on hard material such as child-adult
overlap, collapsing three or four real voices into two tracks. The
ASR *words* are usually fine; the *attribution* is what is wrong. A
dedicated diarizer recounts the voices correctly, and `rediarize`
reconciles its turns with the existing transcript so you keep the
good words and replace only the bad attribution.

The command is **structural and audio-free**: it never touches the
recording. The diarizer runs elsewhere (any tool, any model) and
hands its result across a documented JSON boundary.

## Pipeline position

```mermaid
flowchart LR
    Media["recording\n(audio)"] --> Diarizer["external diarizer\n(e.g. pyannote)"]
    Diarizer --> Turns["turns.json\n(documented format below)"]
    Media --> Asr["ASR with bundled\ndiarization"]
    Asr --> AsrCha["asr.cha\ngood words,\nsuspect speaker tracks"]
    AsrCha --> Rediarize["chatter rediarize\n(this page)"]
    Turns --> Rediarize
    Rediarize --> Fixed["rediarized.cha\nPAR0..PARn correctly\nseparated tracks"]
    Fixed --> SpkId["chatter speaker-id\n(assign real roles)"]
```

`rediarize` fixes WHICH anonymous track owns each utterance; it does
not decide who each track *is*. Role assignment (child, mother,
investigator, ...) is [`chatter speaker-id`](speaker-id.md)'s job,
downstream.

## Usage

```bash
chatter rediarize INPUT.cha --turns TURNS.json -o OUTPUT.cha
```

Omitting `-o` prints the rewritten CHAT to stdout.

A summary is reported on stderr after the rewrite (stderr so that a
stdout CHAT stream stays clean when `-o` is omitted):

```text
rediarize: 214 reassigned, 671 unchanged, 7 flagged
```

Flagged utterances (see below) are listed individually with their
utterance index, kept speaker, and reason.

`--contested-at SHARE` additionally reports utterances whose time is
split between tracks; see [Contested utterances](#contested-utterances-contested-at).

## Machine-readable summary (`--summary-json`)

Batch drivers looping `rediarize` over a corpus should not scrape the
stderr text. `--summary-json PATH` additionally writes the outcome as
JSON:

```bash
chatter rediarize INPUT.cha --turns TURNS.json \
    -o OUTPUT.cha --summary-json SUMMARY.json
```

```json
{
  "source": "pyannote/speaker-diarization-community-1",
  "reassigned": 747,
  "unchanged": 145,
  "flagged": [
    {"utterance_index": 12, "kept_speaker": "PAR1",
     "reason": "no_overlapping_turn"}
  ],
  "contested": [
    {"utterance_index": 41, "assigned": "PAR2",
     "ownership": {"shares": [["PAR2", 600], ["PAR1", 400]],
                   "total_ms": 1000}}
  ]
}
```

- `source`: the turns file's provenance, passed through (`null` if the
  turns file carried none).
- `reassigned` / `unchanged`: utterance counts. `unchanged` includes
  flagged utterances (they kept their speaker), so the file's total
  bulleted-tier utterance count is `reassigned + unchanged`.
- `flagged`: every declined reattribution, **never truncated** (the
  stderr listing caps at 20 detail lines; this list is complete).
  `utterance_index` is the 0-based position among main-tier lines;
  `reason` is `"no_bullet"` or `"no_overlapping_turn"`.

- `contested`: utterances whose time was meaningfully split between
  tracks, empty unless `--contested-at` was given. These were still
  reattributed, to `assigned`, so they are NOT in `flagged`, which means
  "declined". `ownership.shares` is every overlapping track with its
  union-held milliseconds, descending; overlapping turns for the SAME
  track count their shared interval once. `ownership.total_ms` is the sum
  of those per-track values and is the denominator. Simultaneous DIFFERENT
  tracks each retain the shared interval, so `total_ms` can exceed the
  utterance bullet's duration. The whole distribution is emitted rather
  than a winner and a runner-up, because that narrower shape cannot tell a
  55/45 split from 55/23/22 and the difference is the point.

Field names and the `reason` strings are a stable output contract.
The summary is written only on exit 0, after the CHAT output.

## Contested utterances (`--contested-at`)

An utterance's bullet can overlap turns from more than one track. The
tool assigns it to the track holding the most of it, which is the best
available answer, but "most" can mean 95% or 34%, and those are
different situations that the output otherwise reports identically.

```bash
chatter rediarize INPUT.cha --turns TURNS.json --contested-at 0.25
```

Reports an utterance as contested when the RUNNER-UP track holds at
least that share of the total track-held time. Two rivals at 20% each
is a different situation from one at 40%, and this is the latter
question. Same-track duplicate coverage never inflates the denominator;
cross-track overlap remains evidence for both simultaneous speakers.

**There is no default, deliberately.** Omit the flag and nothing is
reported as contested. What share makes an utterance genuinely mixed
has not been measured against human listening, so shipping a number
here would hand every user a constant wearing this tool's authority.
Supply one you can defend, or none.

The flag changes **reporting only**: placement is byte-identical with
and without it. A value outside `0.0` to `1.0`, or `NaN`, fails the
command before any file is read, rather than silently meaning "nothing
is ever contested".

Known limitation: per-track union-held totals cannot distinguish a speaker
change INSIDE an utterance (one track holds the first half, the other the
second) from crosstalk (both across the whole), and those want opposite
remedies. Contested says ownership is divided, not how it is arranged on
the timeline.

## The turns JSON format

The `--turns` file is the corpus-agnostic seam between the diarizer
and chatter. Producing it from any given diarizer's native output is
the caller's concern; the format is:

```json
{
  "source": "pyannote/speaker-diarization-community-1",
  "turns": [
    {"track": "PAR0", "start_ms": 12063, "end_ms": 17024},
    {"track": "PAR1", "start_ms": 13379, "end_ms": 14375}
  ]
}
```

- `source` (optional): free-form provenance, typically the diarizer
  model name. Not interpreted, but useful in audit trails.
- `turns` (required): the timestamped segments. Each has:
  - `track`: the anonymous CHAT speaker code this segment belongs to
    (`PAR0`, `PAR1`, ...). The producer chooses the codes; a
    deterministic mapping from diarizer-native labels (for example
    pyannote's `SPEAKER_00`) is recommended.
  - `start_ms` / `end_ms`: the segment's media time span in integer
    milliseconds, half-open `[start_ms, end_ms)`, with
    `end_ms >= start_ms`.

Turns MAY overlap each other (diarizers that permit overlapping speech
produce such turns). Overlap between turns for the same track is unioned;
overlap between different tracks is retained for each track as crosstalk
evidence. Input order is immaterial: chatter admits the turns to a typed,
start-ordered timeline before attribution. Unknown fields anywhere in the
file are rejected, so a misspelled field fails loudly instead of being
silently ignored.

## Behavior contract

- Every utterance with a time bullet is assigned to the track with the
  greatest union of millisecond coverage against the bullet's span.
  An utterance already on its max-overlap track counts as
  `unchanged`.
- An utterance with **no bullet**, or whose bullet **overlaps no
  turn at all**, keeps its existing speaker and is **flagged** in
  the summary. Ambiguity is surfaced, never silently guessed.
- `@Participants` and `@ID` headers are reconciled to declare
  exactly the set of tracks the output actually uses: new tracks get
  entries cloned from an existing participant (same role),
  declarations for tracks no longer used are dropped.
- Utterance content, dependent tiers, and all other headers are
  preserved as-is.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Rewrite completed and output written. Flagged utterances do not fail the command; check the summary. |
| 1 | Invalid input: unreadable file, CHAT parse failure, malformed turns JSON. |
| 2 | Precondition violation: the turns JSON parsed but is semantically defective (for example a turn with `end_ms < start_ms`). |

On any non-zero exit, no output file is written.

## Worked example

A recording of one child and two parents, transcribed by an ASR
whose bundled diarization auto-detected two speakers (the two adults
were merged into one track). A dedicated diarizer found three voices
and produced `turns.json` with `PAR0`/`PAR1`/`PAR2`. Then:

```bash
chatter rediarize session.cha --turns turns.json -o session-3spk.cha
chatter validate session-3spk.cha
```

splits the merged adult track by time, declares `PAR2` in the
headers, and leaves every word as the ASR wrote it. The output then
flows into `chatter speaker-id` (or the merge workflow) to name the
three tracks.
