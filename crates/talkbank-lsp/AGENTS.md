# Language-server contributor guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-lsp/AGENTS.md)

Read the [root guidance](../../AGENTS.md) first. This file adds LSP-specific
ownership and boundary requirements; it does not create separate gates.
For diagnosis and protocol exercises, use
[LSP Testing and Diagnosis](../../book/src/contributing/lsp-testing.md).

## Ownership

The LSP is a protocol and presentation adapter over the parser, model and
transform crates. It owns request routing, document-session state, editor
coordinates and feature presentation. It does not own CHAT parsing rules,
validation semantics, serialization or cross-tier alignment algorithms.

The production Tree-sitter parser uses generated typed CST traversal. Consume
its results and recovery evidence; do not add an LSP-side CHAT parser or
classify raw ERROR text to reconstruct missing structure.

## Consume model-owned relationships

Before changing hover, highlights, dependency graphs or alignment sidecars,
trace the shared model producer and consume its final result. In particular:

- Use `MorTier::chunks()` and `chunk_at()` for MOR chunks, including clitics
  and terminators. `MorChunkKind` owns classification; serialized punctuation
  is not a substitute. Use `host_item()` when moving from a chunk to its item.
- Read the utterance's `AlignmentSet` for cross-tier positions. A missing
  alignment is not permission to synthesize a one-to-one mapping.
- Use the GRA alignment pair's `mor_chunk_index` for the aligned chunk.
  A relation's authored index remains useful as authored data, not as a
  replacement for the model's computed relationship.
- Phon alignment pairs carry independent model and actual source positions.
  One-sided pauses consume only their own source slot; the hovered alignment
  word's position is not an index into both source tiers.
- `%wor` is a timing sidecar, not a count-validity alignment. Consume the
  model's projection and timing-admission APIs. Do not reconstruct timing
  trust from counts or zip drifted timing with main-tier words.

Keep index spaces distinct using the types in
[`alignment/indices.rs`](../talkbank-model/src/alignment/indices.rs).
MOR items, MOR chunks, main words, phone items and authored one-based GRA
indices are not interchangeable. `GraHeadRef` distinguishes ROOT from a
word reference. Use existing conversions rather than subtracting from a
possibly zero authored index. If a needed model primitive is absent, add it
at the shared owner instead of duplicating an algorithm here.

## Protocol and document boundaries

- Coordinates use UTF-16 code units. Use `position_to_offset` for incoming
  coordinates and `backend::utils::LineIndex` for repeated outgoing
  conversions. Retain non-BMP and line-ending controls; byte offsets are not
  editor columns.
- Respect parse-health and recovery evidence. Withhold untrustworthy
  cross-tier results; local information may remain useful without implying
  that alignment was admitted. Never fabricate missing semantic values.
- Document changes must refresh affected results. Exercise edits as well as
  initial open. A correct fallback is allowed; do not promise a particular
  incremental fast path without its preconditions.
- Keep request handlers panic-free. Preserve typed errors and graceful
  unavailable results. Custom RPC payloads need typed request/response
  models, not unstructured JSON passed through feature logic.
- Keep stdout reserved for stdio protocol messages. Diagnostic logs may
  contain document text and paths; use synthetic/reference inputs and review
  logs before sharing them.
- Shutdown behavior belongs to the protocol lifecycle owner and standalone
  entrypoint. Library tests cannot prove child-process termination; retain
  bounded real-stdio tests and cleanup. Embedders own their runtime teardown.

## Verification and navigation

Select existing per-feature tests for the changed contract. Use shared
`test_fixtures` helpers when a test requires validated CHAT and computed
alignments; a parse-only helper does not establish validity. Reuse spec and
reference fixtures. MOR/GRA changes need clitic controls; Phon position
changes need one-sided-pause controls. Keep missing-metadata behavior tested.

Use the root workflow for formatting, scoped lint, focused tests and final
publication checks. Shared-model or parser changes require their affected
contracts too, but an LSP-only presentation change does not mandate rerunning
every corpus suite. Discover current test names from the selected target;
zero selected tests and ignored tests are not verification.

Feature handlers live in `src/backend/features/`, document ownership in
`src/backend/documents.rs`, and source-bound analysis in
`src/backend/diagnostics/analysis.rs`. Alignment presentation lives in
`src/alignment/`, graph rendering in `src/graph/`, and semantic tokens in
`src/semantic_tokens.rs`. Integration tests live in `tests/`; the
`position_conversion_tests` target includes real stdio lifecycle contracts.
See [alignment architecture](../../book/src/architecture/alignment.md) for
the shared relationship model and
[editor integration](../../book/src/chatter/user-guide/editor-integration.md)
for user-facing setup and support boundaries.
