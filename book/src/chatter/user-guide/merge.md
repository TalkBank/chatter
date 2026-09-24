# Removed commands: `merge`, `pipeline`, and `batch`

**Last modified:** 2026-09-24 00:21 EDT

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
