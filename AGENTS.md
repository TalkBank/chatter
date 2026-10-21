# Chatter agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/AGENTS.md)

Canonical guidance for all coding agents. Read applicable nested guidance and
the task-relevant references below.

## First principle: typestate

Typestate and type-driven design are a firm mandate for every change, in every
language and every repository: new code, old code, fixes, refactors, tests and
test harnesses alike. Nothing below overrides this. Every project here is
long-term, never a one-off: a type is paid for once, while a bug the
architecture permits recurs for the life of the project.

- Before writing code or a test, ask which type change makes the defect
  unrepresentable, and make it. The compiler is the first test.
- Give each distinct meaning its own type and each step a transition
  `fn(Previous) -> Result<Next>` that consumes the earlier state. Put every
  check on the edge that is the only way to make the next state; consumers
  never re-check it.
- No sentinels, bare flags or catch-alls. A closed set is an enum matched
  exhaustively; a flag that chooses between behaviors is an enum; a field that
  only some states use belongs to those states.
- After each type change, count what it removed (checks, branches, panics,
  tests). If it removed nothing, the defect was moved, not eliminated.
- Every review checks for typestate opportunities explicitly. A fix that could
  have been a type is not finished.

## Scope and safety

- This is the canonical, self-contained CHAT format implementation. Keep
  grammar, spec, parsing, validation and generic transforms here. Corpus-specific
  workflow belongs downstream; use documented corpus-agnostic input seams.
- All content is public. Follow `CONTRIBUTING.md`; do not introduce private
  identities, paths, hosts, operational records or data.
- Keep the existing checkout and unrelated changes. Push/release needs explicit
  maintainer authorization, which persists within the stated scope. Squash
  unpublished commits since the last push; preserve published history. Never
  force-push, bypass hooks, push to `archive` or change repository visibility.
- Adjudicate diagnostics before changing data: distinguish tool defects from
  invalid input using the spec and real evidence. Recovery grammar acceptance
  is not validity. Do not change expectations merely to make a test pass;
  ask only when a semantic decision lacks a specification or ruling.
- Generated parser, traversal, registry, fixtures, schema and audit artifacts
  must be regenerated from their sources. Schema doc comments are inputs too.
  `release.yml` comes from cargo-dist; change its source configuration.
- No panics in long-lived code. Preserve the 16 MiB program thread and workspace
  lint inheritance. Use spec examples/reference fixtures, not ad hoc CHAT files.

## Development workflow

Authorized work includes implementation, investigation, review and verification;
no repeated design approval is needed. Use types to express invariants and
transitions. Validating constructors still need tests; keep policy, serialization
and external-boundary tests. Remove only tests whose checks are truly redundant.
Documentation/configuration edits receive proportional verification.
Publication metadata is Git-derived, not a claim of content review. Use the
book's page-date placeholder or a source-bound Git-history link; handwritten
dates still require real `date` output on edits. See
[documentation architecture](book/src/contributing/documentation-architecture.md).

`just test` is the inner loop. Changes to grammar, spec, registries or generated
inputs require `just regen`, then tests. Review the final diff for reuse,
simplicity, efficiency, abstraction boundaries and typestate before committing.
Review production-change evidence and breaking-change notes explicitly.
Commit hooks only chain to optional contributor-owned local hooks; do not
reintroduce automatic date, evidence or changelog commit gates.

Before an authorized push, run `just gate` on final content. Reuse its receipt
only while the checked inputs/tooling remain valid. `just test` omits doctests
and cannot replace the gate. Never run concurrent cargo-family commands in the
same workspace. Clippy and feature-off builds belong to `just release-lint`,
not an expanded per-push gate. Release sequence: `just fmt`,
`just release-lint`, `just gate`, reviewed squash of unpublished work, authorized
push, verified CI, then `just release-tag X.Y.Z`.

## Task references

| Task | Read |
| --- | --- |
| Build, generated files, commit/release gates and book | [Development](docs/agent-reference/development.md) |
| Typestate, AST ownership, cache keys and coding rules | [Design](docs/agent-reference/design.md) |
| Validity, architecture, cache policy, LSP and nested guidance | [Architecture and validity](docs/agent-reference/architecture-and-validity.md) |

This entry point resolves workflow conflicts in the references. Dated examples
are evidence, not current version or deployment claims. Generic skills do not
create extra approval gates or authorize publishing.

## Nested guidance

Each file below adds the ownership and boundary rules of its tree. None
defines another gate, approval step or release workflow; this file and the
references above own those. Read the one for the tree you are editing.

| Tree | Guidance | What it adds |
| --- | --- | --- |
| `grammar/` | [grammar/AGENTS.md](grammar/AGENTS.md) | Grammar source and generated-output ownership, strict plus catch-all |
| `spec/`, `spec/tools/` | [spec/AGENTS.md](spec/AGENTS.md), [spec/tools/AGENTS.md](spec/tools/AGENTS.md) | Authored inputs, generated artifacts, claims, generator boundaries |
| `crates/talkbank-model/` | [AGENTS.md](crates/talkbank-model/AGENTS.md) | The typed AST, locations, validation context, alignment owners, what cannot be tested here |
| `crates/talkbank-parser/` | [AGENTS.md](crates/talkbank-parser/AGENTS.md) | Typed CST traversal, `span_of`, fragment rebasing, zero unlocated values |
| `crates/talkbank-parser-re2c/` | [AGENTS.md](crates/talkbank-parser-re2c/AGENTS.md) | Second backend, compatibility mandate, located and unlocated items |
| `crates/talkbank-parser-tests/` | [AGENTS.md](crates/talkbank-parser-tests/AGENTS.md) | Where tests live, the gates and ratchets, how a ceiling is lowered |
| `crates/talkbank-transform/` | [AGENTS.md](crates/talkbank-transform/AGENTS.md) | Pipelines, checked rewrites, splice, pseudonymizer, shared rendering |
| `crates/chatter/` | [AGENTS.md](crates/chatter/AGENTS.md) | CLI surface, exit codes, stdout, the program thread, the TUI as reference |
| `crates/talkbank-lsp/` | [AGENTS.md](crates/talkbank-lsp/AGENTS.md) | Protocol adapter over model-owned relationships; index spaces |
| `apps/chatter-desktop/` | [AGENTS.md](apps/chatter-desktop/AGENTS.md) | Desktop app; TUI parity mandate; no desktop-local domain logic |

Two rules cut across every tree and are stated once, in the design reference:
a model value's `Location` is set at construction and never defaulted, and a
validation rule fires on a construct's presence while its location only
decides where the diagnostic points. The ratchet gates in
`crates/talkbank-parser-tests` (fabrications, parser-minted unlocated values,
content catch-alls, undemonstrated codes) may only fall, and a ceiling is
lowered in the commit that earned it.

<!-- trail:start -->
## Trail: repo context graph

This repo is indexed in `.trail/cache/`: small linked markdown nodes that explain each
system and carry exact file:line spans, kept in sync with the code through git.

For ANY task here, understanding how something works, finding where code lives,
or scoping a change, get context from the graph before grepping or opening
source files. Re-ask freely (it's cheap) and reuse literal identifiers you
already have (symbol, error string, file name) as the query. New to this repo?
Run `trail map` first: a token-budgeted orientation (dir clusters, hubs,
hotspots), no LLM, no key.

- Run `trail ask "<your question>" --source` → ranked nodes with the relevant
  code spans inlined (each hit's ≤8-line crux by default; `--full` for whole
  definitions when the crux isn't enough). Match the tool to the task shape:
  for understanding or editing, the top node IS the answer; cite its
  `covers:` file:line spans and edit straight from `--source`. For
  exhaustive tasks ("every occurrence / every caller of this pattern"), ranked
  results are top-N, not complete; run `trail grep "<literal>"` instead
  (exhaustive over indexed files, grouped by enclosing symbol), falling back
  to raw `grep -rn` only for unindexed files.
- `trail skeleton <file>` → every definition's signature + span, ~10× cheaper
  than reading the file; use it to skim an API surface.
- `trail callers <symbol>` gives precomputed, exact edges: who calls this.
  Add `--direction out` for what it calls, or `--depth N` to walk
  transitively for the full blast radius. For structural questions, skip
  ranking and use this directly.
- Or browse: `.trail/cache/INDEX.md` lists every node; follow the links.
- Monorepos and folders of multiple repos rank fairly across sub-projects;
  hits carry `[scope/]` labels naming which one they're from. Narrow with
  `trail ask "<task>" --in <scope>/` once you know where you're working.

If a returned span is truncated ("+N more lines"), open the file at that exact
range before finalizing. Only open source files when a node genuinely lacks a
needed detail, and then at the exact file:line the node points to; never
re-read whole files.

After big code changes, refresh the graph with `trail build` (deterministic,
no API key, $0).
<!-- trail:end -->
