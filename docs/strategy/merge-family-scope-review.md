# Merge Command Family: Scope-Rule Classification

**Status:** Historical scope review with subsequent command removals
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/strategy/merge-family-scope-review.md)

The June ratification below is historical, not the current command inventory.
Later removal of `merge`, `pipeline`, `batch` and the internal CLI preflight
supersedes the original all-seven-stay conclusion. Current usage and boundaries
belong to [Removed Commands](../../book/src/chatter/user-guide/merge.md),
[Review Tools](../../book/src/chatter/user-guide/merge-workflow.md) and
[Adjudication Workflow](../../book/src/architecture/adjudication-workflow.md).
The historical source paths, search observations and unchecked tasks below are
not current implementation or verification receipts.

Pre-release review of the transcript-merge command family against the
repo scope rule (AGENTS.md: every subcommand must be useful to CHAT
users in general, never specific to one corpus, one data provider, or
one workflow). Question answered per surface: does it stay in chatter,
or does it belong in a downstream project?

Surfaces reviewed: the CLI layer
(`crates/chatter/src/commands/{adjudicate,batch,merge_preflight,
pipeline,sanity_scan,transcript_merge}.rs`, `speaker_id/`) AND the
backing `talkbank-transform` modules (`adjudication.rs`,
`sanity_scan.rs`, `speaker_id/`, `transcript_merge.rs`).

## Recorded dispositions (including later removals)

| Surface | Verdict | Strongest grounds |
|---------|---------|-------------------|
| `chatter merge` (transcript_merge) | CLI REMOVED; library retained | Structural assembly is not fuzzy speech-event reconciliation. The public command was misleading; typed library APIs remain for callers with resolved source selection. |
| `chatter speaker-id` | STAYS, general | Three independent modes (explicit mapping, reference-text Jaccard, override-file replay); the LLM judgment mode consumes the free-vocabulary `--session-context` JSON seam, whose module doc states corpus-specific conversion lives outside this repository; example codes (PAR0, CHI, INV) are illustrative prompt text, not hardcoded. |
| `chatter pipeline` | REMOVED | Speaker mapping plus structural assembly did not provide event correspondence. |
| `chatter batch` | REMOVED | The directory driver exposed the same removed pipeline. |
| `chatter sanity-scan` | STAYS, general, with a documented bias | Interface (override-file integration, pending entries) fully generic; the default heuristic (anchor/inserted mean-utterance word-count asymmetry) is child-language oriented. It is optional and tunable (`--threshold`); the module doc invites replacing the signal. Follow-up: state the child-language orientation in `--help` and the book page. |
| `chatter adjudicate` | STAYS, general | Human-approval layer over generic decision kinds (speaker-id low confidence, parent-role lookup, sanity-scan flag) via an extensible Prompter trait. |
| `merge_preflight` (internal module, not a subcommand) | REMOVED | Unused CLI preflight removed with pipeline and batch; library validation remains. |

## Cross-cutting findings

1. **No corpus/provider/project hardcoding found.** Greps across the
   seven surfaces for provider names, contributor concepts, profile
   names, and spreadsheet/datasheet logic return nothing. The one
   historical violation (an xlsx datasheet converter and a
   contributor-specific flag) was removed on 2026-06-11 and replaced by
   the corpus-agnostic `--session-context` JSON seam.
2. **`--session-context` is the designed boundary.** Free-vocabulary
   labels, all fields optional, consumed only by the LLM prompt;
   producing that JSON from any particular records system is explicitly
   a downstream concern.
3. **Speaker codes are never a closed set.** PAR0/PAR1/CHI/INV appear
   only as documentation and prompt examples.

## Original June conclusion and follow-up list (superseded)

The June review concluded that all seven surfaces should remain and that no
relocations were required before the first release. That conclusion no longer
describes the CLI; the subsequent removals are retained in the table above.
The following checklist is historical planning, not an instruction to restore
removed commands or rerun obsolete test targets:

- [ ] Per-command `--help` accuracy pass.
- [ ] Book page accuracy pass (merge.md, speaker-id.md,
      batch-workflows.md, adjudication-workflow.md) verified by
      literally running the documented commands.
- [ ] Documented exit-code contract per command.
- [ ] Subprocess-level integration test present per command (most
      already exist: speaker_id_tests.rs, batch_tests.rs,
      pipeline_tests.rs, sanity_scan_tests.rs,
      holistic_pipeline_batch_cli.rs; verify coverage of `merge` and
      `adjudicate` at the CLI seam).
- [ ] sanity-scan child-language-orientation note in `--help` and book.

- [x] **Maintainer ratification of these verdicts**: ratified
      2026-06-12 (all seven stay).
