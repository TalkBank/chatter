# Removed commands: `merge`, `pipeline`, and `batch`

**Last modified:** 2026-09-28 20:59 EDT

The experimental `chatter merge`, `chatter pipeline`, and `chatter batch`
commands have been removed from the CLI.
Earlier releases exposed structural interleaving of two CHAT transcripts under
this name. That operation assumed the caller had already resolved which speech
events and speakers to retain. It did not match uncertain transcriptions to
recorded speech, reconcile different segmentation, or correct speaker attribution.
The command name encouraged a broader interpretation than the operation supported.

There is no drop-in CLI replacement for fuzzy transcript-to-recording matching.
The former `pipeline` and `batch` commands composed speaker mapping and
structural assembly; they did not supply that missing event-matching step.

For applications that have already resolved correspondence and source selection,
the typed structural APIs remain in `talkbank_transform::transcript_merge`.
They preserve their validated input, draft, and reported-output transitions.
Removing this CLI does not remove or change those library contracts.

## Explicit review drafts in the library

Structural assembly normally refuses unresolved cross-source ordering. A caller
can explicitly opt in through
`SourceBoundDonorSelection::with_flagged_draft_order`, bound to the exact
reference document. This uses reference-first serialization at unresolved
frontiers and preserves both source sequences. Uncertainty is returned only as
structured `DraftOrderReview` records; no generated review `@Comment` lines are
inserted. Contributor comments remain unchanged. Callers present review information
outside the transcript using `MergeDraft::draft_order_reviews()` before validation
or `Merged::draft_order_reviews()` afterward.

Each record carries an `OutputUtteranceBoundary`, not a CHAT line index. Its
`utterances_before()` count excludes all headers and comments; zero means before
the first utterance. The reason distinguishes competing utterances, a section
against an utterance, and competing sections. Optional section navigation bounds
are milliseconds from neighboring recorded speech, not inferred section times.

That convention does **not** establish chronology or task membership. It does
not authorize speech deletion, invented timestamps, or overriding contradictory
timing evidence. Model validation remains a separate required transition;
validation success does not adjudicate the recorded ordering uncertainties.
The canonical untimed conversation and disjoint-timing reference tests exercise
these distinctions without changing their source transcripts.
