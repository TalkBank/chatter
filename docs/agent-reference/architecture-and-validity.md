# Architecture and validity reference

**Last modified:** 2026-09-10 00:45 EDT

Read the sections relevant to your task. [AGENTS.md](../../AGENTS.md)
is the canonical policy entry point and resolves workflow conflicts here.
Dated incidents, measurements, versions and paths are historical evidence;
verify current source and live artifacts before relying on them. Inline
repository paths are relative to the repository root unless stated otherwise.

## Repo positioning

This repository is the canonical home of the TalkBank CHAT format authority
and the `chatter` tool family, and the source of truth for the `chatter`
binary. The CHAT core is self-contained: it builds and runs with no external
repository, and downstream consumers depend on its crates directly.

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

**`chatter validate` is the authority on whether a byte sequence is valid
CHAT.** When it rejects a file, the file is invalid and the response is to
clean the data, not weaken the parser.

**That is a conclusion, never a default action, and chatter is never assumed
correct.** Before any data is touched, every diagnostic is adjudicated: is
this a chatter defect or a data defect? The working assumption is that
chatter is wrong unless the data is certainly at fault. The test is whether
the rejected construct actually fails to make sense. Signs the fault is
chatter's: a message that does not match its input; a generic "unparsable"
code standing in for a specific rule; an auto-generated error spec with no
description; behaviour no spec justifies.

**Authority ordering:** the CHAT manual is dated background. When it and this
project diverge, trust `spec/`, the grammar, and above all real corpus data.
Never reintroduce a legacy construct that has been removed from the data and
from chatter, whatever the manual or CLAN still say. **A permissive grammar
is not a validity claim**: this grammar deliberately admits invalid
constructs so the model can name them precisely, so grammar tolerance is
weaker evidence of legality than CLAN CHECK's silence, not stronger.

**A word may carry at most one `@` suffix** (maintainer ruling; a documented
divergence from CLAN CHECK, recorded in `spec/errors/E203.md`).

## Architecture (index)

Data flows: **spec** (source of truth) → **grammar** → **crates** (parsers,
model, transform, cli, lsp).

| Crate | Purpose |
|-------|---------|
| `talkbank-model` | Typed CHAT AST, WriteChat, validation, alignment, `walk_words`, `ChatParser` trait |
| `talkbank-derive` | SemanticEq / SpanShift / error-code proc macros |
| `talkbank-cache` | SQLite validation/roundtrip cache |
| `talkbank-parser` | Canonical tree-sitter parser |
| `talkbank-parser-re2c` | Independent re2c parser: spec oracle and wasm-clean backend |
| `talkbank-parser-tests` | Equivalence, roundtrip, golden, property tests |
| `talkbank-transform` | Pipelines, CHAT↔JSON, normalize, merge |
| `chatter` | The CLI |
| `talkbank-lsp` | LSP server (tree-sitter only) |
| `send2clan` | CLAN app bridge bindings |
| `chatter-desktop` | Tauri v2 desktop app |

Two Cargo workspaces: the root, and `spec/` (`spec/tools`,
`spec/runtime-tools`). Parser-backend selection and the oracle workflow:
`book/src/architecture/parser-backends.md`,
`crates/talkbank-parser-re2c/CLAUDE.md`.

**"Parity" names two unrelated programmes; always say which.** **CHECK
adjudication** asks, per CLAN CHECK code, whether the rejected construct
fails to make sense, and records a divergence or closes the gap; it is about
CHAT validity and every entry in `tests/check_parity/manifest.json` carries a
terminal verdict. **Backend parity** asks whether the two parsers answer
identically; it is about our implementations, says nothing about CHAT, and
is open, tracked per spec case in `KNOWN_DIVERGENCES`.

**The reference corpus** (`corpus/reference/`) is a regression signal, not a
validity authority: when a change rejects a reference file, adjudicate the
file against the real authorities and fix the data (or move it to
`spec/errors/`) rather than weaken the parser.

## Cache policy

The validation cache lives in the OS cache directory; `--force` refreshes
specific paths; `TALKBANK_CHAT_CACHE_DIR` relocates the root. Integration
tests isolate the cache through `CliHarness`, never `HOME` tricks.
Initialization is concurrency-safe across threads and processes. Never delete
a user's cache without an explicit request.

## LSP reliability

Backend init failures surface as diagnostics, not panics; handlers degrade
gracefully; diagnostics align with parse-health semantics.
`crates/talkbank-lsp/CLAUDE.md`.

## Sub-project CLAUDE.md files

| File | Scope |
|------|-------|
| `grammar/CLAUDE.md` | Grammar design, verification sequence, strict+catch-all |
| `spec/CLAUDE.md` + `spec/tools/CLAUDE.md` | Spec structure, generators, regeneration |
| `crates/talkbank-lsp/CLAUDE.md` | LSP: model-owned alignment; index spaces; reliability |
| `crates/talkbank-parser-re2c/CLAUDE.md` | Re2c parser and oracle workflow |
| `apps/chatter-desktop/CLAUDE.md` | Desktop app; TUI parity mandate |

## Relationship to batchalign

This repo contains no Batchalign code; the ML pipeline consumes chatter's
crates from its own repository.
