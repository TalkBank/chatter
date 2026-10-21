# Transform crate agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-transform/AGENTS.md)

Read the [root guidance](../../AGENTS.md) first. This file adds the transform
crate's ownership and boundary rules; it does not create separate gates.

## What this crate is

The pipelines that compose parsing, validation and transformation over CHAT
files, and the one place their results are rendered: parse and validate
(`parse.rs`, `validate.rs`, `validation_runner/`, `worker_pool.rs`), CHAT to
JSON and back (`json.rs`, `serialize.rs`, the embedded schema), checked
rewrites (`splice/`, `build_chat/`, `fix_s.rs`, `retag_language.rs`,
`join_retrace.rs`, `utterance_split/`, `dependent_tiers.rs`), merge
(`transcript_merge/`, `adjudication.rs`), the name pseudonymizer
(`pseudonymize/`), redaction and sanity scanning, and diagnostic rendering
(`rendering.rs`, `presentation.rs`). Architecture:
[transform pipeline](../../book/src/architecture/transform-pipeline.md),
[merge domain types](../../book/src/architecture/merge-domain-types.md),
[validation cache](../../book/src/architecture/parser-and-grammar/validation-cache.md).

## Boundaries

- This crate is generic CHAT tooling. Nothing specific to one corpus, provider
  or workflow; per-corpus input arrives through documented corpus-agnostic
  seams (`--session-context` JSON), and producing it is downstream.
- A returned model, JSON string or `Rewrite` does not authorize replacing the
  input. `normalize_chat` returns a checked `Rewrite` whose source-loss check
  may refuse serialization of valid CHAT; that refusal is the contract, not a
  bug to route around.
- Content-only entry points use anonymous transcript identity, which silently
  omits the name-dependent checks (E531). Input that came from a file goes
  through the path-aware or named entry point.
- Parsing failure, internal fault, model-validation failure and incomplete
  provenance are distinct outcomes. Never fold them into one success, never
  discard diagnostics, never `.ok()` a `Result` into a default.
- The pseudonymizer refuses an unlocated word (`RefusedWord::source`): a
  rewrite that cannot name where it edits does not edit. The splice engine
  diagnoses once (`splice/diagnosis`) and refuses an unlocated diagnostic
  (`CatalogIneligibility::Unlocated`) rather than guessing a byte.

## Shared with the CLI and the desktop app

`render_diagnostics()` (`rendering.rs`) is the single orchestration of
enhancement and miette rendering; the CLI and the desktop bridge both call it
and neither keeps a local render path. `validate_files_streaming` and
`validate_directory_streaming` are the only validation loops; the desktop's
single-file path once bypassed them and silently diverged on caching, E531
and stats. Policies (`presentation.rs`) downgrade or hide by code; they never
alter the retained producer diagnostic.

## Verification

Integration tests live in `tests/integration/`; `render_parity.rs` is the
shared-rendering control and `generate_schema` fails when
`schema/chat-file.schema.json` is stale relative to the model's doc comments
(`just schema-gen` regenerates it). Spec-derived corpora reach this crate
through `talkbank-parser-tests`' transform corpus modules; a new rewrite or
policy needs a spec example and a reference-corpus control, not an ad hoc
`.cha`. The validation cache is isolated in tests through its owned temporary
location, never a shared user cache.
