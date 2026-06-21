# Merge Command Family: Scope-Rule Classification

**Status:** Current. Verdicts ratified by the maintainer 2026-06-12.
**Last updated:** 2026-06-12 16:29 EDT

Pre-release review of the transcript-merge command family against the
repo scope rule (CLAUDE.md: every subcommand must be useful to CHAT
users in general, never specific to one corpus, one data provider, or
one workflow). Question answered per surface: does it stay in chatter,
or does it belong in a downstream project?

Surfaces reviewed: the CLI layer
(`crates/talkbank-cli/src/commands/{adjudicate,batch,merge_preflight,
pipeline,sanity_scan,transcript_merge}.rs`, `speaker_id/`) AND the
backing `talkbank-transform` modules (`adjudication.rs`,
`sanity_scan.rs`, `speaker_id/`, `transcript_merge.rs`).

## Verdicts

| Surface | Verdict | Strongest grounds |
|---------|---------|-------------------|
| `chatter merge` (transcript_merge) | STAYS, general | Pure structural AST merge of two CHAT files sharing a timeline; no ASR/inference/domain logic; `book/src/chatter/user-guide/merge.md` already documents it corpus-agnostically. |
| `chatter speaker-id` | STAYS, general | Three independent modes (explicit mapping, reference-text Jaccard, override-file replay); the LLM judgment mode consumes the free-vocabulary `--session-context` JSON seam, whose module doc states corpus-specific conversion lives outside this repository; example codes (PAR0, CHI, INV) are illustrative prompt text, not hardcoded. |
| `chatter pipeline` | STAYS, general | Thin composition of speaker-id reference mode + merge; no domain logic of its own. |
| `chatter batch` | STAYS, general | Pure subprocess orchestrator over donor/reference directory pairs (basename matching); aggregates outcomes; threads settings. |
| `chatter sanity-scan` | STAYS, general, with a documented bias | Interface (override-file integration, pending entries) fully generic; the default heuristic (anchor/inserted mean-utterance word-count asymmetry) is child-language oriented. It is optional and tunable (`--threshold`); the module doc invites replacing the signal. Follow-up: state the child-language orientation in `--help` and the book page. |
| `chatter adjudicate` | STAYS, general | Human-approval layer over generic decision kinds (speaker-id low confidence, parent-role lookup, sanity-scan flag) via an extensible Prompter trait. |
| `merge_preflight` (internal module, not a subcommand) | STAYS, general | Shared fail-closed validation gate: every merge input must pass the same checks as `chatter validate` before any merge work. |

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

## Conclusion

All seven surfaces remain in chatter. No relocations are required
before the first public release. Remaining work is hardening, not
moving (tracked in the v0.1.0 release checklist, Track A):

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
