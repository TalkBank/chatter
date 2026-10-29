# Architecture and validity reference

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/agent-reference/architecture-and-validity.md)

Read the sections relevant to your task. [AGENTS.md](../../AGENTS.md)
is the canonical policy entry point and resolves workflow conflicts here.
Dated incidents, measurements, versions and paths are historical evidence;
verify current source and live artifacts before relying on them. Inline
repository paths are relative to the repository root unless stated otherwise.

## Repo positioning

This repository owns the CHAT implementation and the `chatter` tool family.
Runtime consumers use its crates; contributor regeneration also has documented
generator prerequisites. See the contributor setup and generation instructions
rather than inferring build or public-package availability from a source checkout.

**Scope: general-purpose CHAT tooling only.** Nothing specific to one corpus,
one data provider or one workflow. Where a general capability needs
per-corpus input it takes a documented corpus-agnostic form (the
`--session-context` JSON seam); producing that input is a downstream concern.

**Git hygiene.** Before each push, squash unpublished commits since the last
push into one reviewed commit. Push rarely and preserve published history.
Never `git push --force`, never `--no-verify`, never push to
the `archive` remote, never change visibility or push a shared branch without
the maintainer's sign-off. `CONTRIBUTING.md` covers content hygiene.

## CHAT-validity authority

`chatter validate` implements the maintained validity rules; its observed
behavior is not an independent definition of CHAT. Before changing data,
adjudicate each diagnostic against the specifications and recorded semantic
rulings: a rejection may expose a tool defect rather than invalid input.
Unsupported messages, unexplained specs and misleading source locations need
investigation, not automatic transcript edits or weakened checks.

Use `spec/`, the CHAT documentation, source examples and recorded rulings
together to resolve intended behavior. Corpus occurrence is evidence, not
proof of legality. Do not revive a deliberately retired construct from an
older manual or another validator's behavior alone. A permissive recovery
grammar is not a validity claim, and CLAN CHECK's silence is not one either.
Ask when these sources leave the semantic decision unresolved.

**A word may carry at most one `@` suffix** (maintainer ruling; a documented
divergence from CLAN CHECK, recorded in `spec/errors/E203.md`).

## Architecture (index)

Authored specs record constructs and expected behavior; the grammar has its own
maintained source and generated artifacts. Registry generators share vocabulary
with consumers. Do not infer that every grammar rule is generated from a spec
example, or that parser acceptance establishes model validity. The parser/model
boundary and downstream validation have distinct contracts.

| Crate | Purpose |
|-------|---------|
| `talkbank-model` | Typed CHAT AST, WriteChat, validation, alignment, `walk_words`, `ChatParser` trait |
| `talkbank-derive` | SemanticEq / SpanShift / error-code proc macros |
| `talkbank-build` | Build-time fingerprints of selected packaged source trees |
| `talkbank-cache` | SQLite validation/roundtrip cache |
| `talkbank-parser` | Tree-sitter parser backend |
| `talkbank-parser-re2c` | Independent re2c backend; shared compatibility contract and wasm compilation checks |
| `talkbank-parser-tests` | Equivalence, roundtrip, golden, property tests |
| `talkbank-transform` | Pipelines, CHAT↔JSON, normalize, merge |
| `talkbank-llm` | HTTP judgment-provider implementation for explicitly requested model-assisted work |
| `talkbank-chatter` | The CLI package, in `crates/chatter`, producing `chatter` |
| `talkbank-lsp` | LSP server (tree-sitter only) |
| `send2clan` | CLAN app bridge bindings |
| `chatter-desktop` | Tauri v2 desktop app |

Two Cargo workspaces: the root, and `spec/` (`spec/tools`, the core
generators independent of the live model; `spec/runtime-tools`, generators and
censuses that need the live model and parsers; `spec/vocabulary`, the shared
authored schemas and diagnostic vocabulary). Parser-backend selection and comparison contracts:
`book/src/architecture/parser-backends.md`,
`crates/talkbank-parser-re2c/AGENTS.md`.

**"Parity" names two unrelated programmes; always say which.** **CHECK
adjudication** asks, per CLAN CHECK code, whether the rejected construct
fails to make sense, and records a divergence or closes the gap; it is about
CHAT validity and every entry in `crates/talkbank-parser-tests/tests/check_parity/manifest.json` carries a
terminal verdict. **Backend comparison** measures our implementations under the
[compatibility contract](../../book/src/architecture/parser-backends.md#backend-compatibility-mandate):
supported valid input needs semantic agreement, and every tree-sitter error
requires some re2c error for 1.0. Diagnostic/recovery identity is not required;
code-set measurements do not define CHAT policy or waive missed errors.

**The reference corpus** (`corpus/reference/`) is a regression signal, not a
validity authority: when a change rejects a reference file, adjudicate the
file against the specifications and rulings. Fix a demonstrated implementation
defect, or correct/reclassify an invalid fixture with evidence. Do not weaken
rules or alter fixtures merely to obtain a passing test.

## Cache policy

The validation cache lives in the OS cache directory; `--refresh-cache` refreshes
specific paths; `TALKBANK_CHAT_CACHE_DIR` relocates the root. Integration
tests isolate the cache through `CliHarness`, never `HOME` tricks.
Treat cross-thread/process initialization, contention and cleanup as explicit
test obligations; one successful cache open is not a concurrency proof. Never
delete a user's cache without an explicit request.

## LSP reliability

Backend init failures must surface as diagnostics, not panics; handlers must
preserve typed failure outcomes and diagnostics must respect parse health.
These are boundary obligations, not a claim that every failure path has been
qualified on every platform. See
`crates/talkbank-lsp/AGENTS.md`.

## Sub-project AGENTS.md files

The root [AGENTS.md](../../AGENTS.md#nested-guidance) owns the one table of
nested guidance files and what each adds; it is not duplicated here.

## Relationship to batchalign

This repo contains no Batchalign code; the ML pipeline consumes chatter's
crates from its own repository.
