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
- Never replace missing, malformed or failed model data with silent defaults
  or plausible sentinels. They fabricate facts, cause spurious diagnostic
  cascades and can silently corrupt serialized/transformed data. Preserve
  explicit recovery states or propagate rejection. See the
  [design principle](book/src/architecture/chat-model/chat-model.md#no-silent-defaults-or-sentinels).
- After each type change, count what it removed (checks, branches, panics,
  tests). If it removed nothing, the defect was moved, not eliminated.
- Every review checks for typestate opportunities explicitly. A fix that could
  have been a type is not finished.

## Scope and safety

- Tree-sitter and re2c are peers. Both can drive changes to the shared model,
  including malformed/recovery states; follow
  [shared model ownership](book/src/architecture/parser-backends.md#equal-backends-one-shared-model).
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

## Optional: a local trail index

Some maintainers index this repository locally with trail (the npm package
`@trailhq/trail-code`). Its index lives in `.trail/`, which is gitignored, so
a fresh clone has none and nothing in this repository depends on it. When
`trail` is installed and `.trail/cache/` exists, use it before searching
blindly:

- `trail ask "<question>" --source`: ranked code spans that answer a
  question, each with its file and line range.
- `trail grep "<literal>"`: every occurrence, grouped by enclosing symbol.
  Use it whenever completeness matters, since `ask` ranks only the top hits.
- `trail skeleton <file>`: a file's signatures without reading the file.
- `trail callers <symbol>`: exact call edges; `--direction out` for callees
  and `--depth N` for the blast radius of a change.
- `trail build`: refresh the index after large changes.

Without trail, use `rg` and the nested AGENTS.md files above.
