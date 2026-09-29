# Chatter agent guidance

**Last modified:** 2026-09-28 20:59 EDT

Canonical guidance for all coding agents. Read applicable nested guidance and
the task-relevant references below.

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

`just test` is the inner loop. Changes to grammar, spec, registries or generated
inputs require `just regen`, then tests. Review the final diff for reuse,
simplicity, efficiency, abstraction boundaries and typestate before committing.
Preserve the production-Rust evidence and breaking-changelog hooks.

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

<!-- graft:start -->
## Graft — repo context graph

This repo is indexed in `graft/`: small linked markdown nodes that explain each
system and carry exact file:line spans, kept in sync with the code through git.

For ANY task here — understanding how something works, finding where code lives,
or scoping a change — get context from the graph before grepping or opening
source files. Re-ask freely (it's cheap) and reuse literal identifiers you
already have (symbol, error string, file name) as the query. New to this repo?
Run `graft map` first — a token-budgeted orientation (dir clusters, hubs,
hotspots), no LLM, no key.

- Run `graft ask "<your question>" --source` → ranked nodes with the relevant
  code spans inlined (each hit's ≤8-line crux by default; `--full` for whole
  definitions when the crux isn't enough). Match the tool to the task shape:
  for understanding or editing, the top node IS the answer — cite its
  `covers:` file:line spans and edit straight from `--source`. For
  exhaustive tasks ("every occurrence / every caller of this pattern"), ranked
  results are top-N, not complete — run `graft grep "<literal>"` instead
  (exhaustive over indexed files, grouped by enclosing symbol), falling back
  to raw `grep -rn` only for unindexed files.
- `graft skeleton <file>` → every definition's signature + span, ~10× cheaper
  than reading the file; use it to skim an API surface.
- `graft callers <symbol>` gives precomputed, exact edges — who calls this.
  Add `--direction out` for what it calls, or `--depth N` to walk
  transitively for the full blast radius. For structural questions, skip
  ranking and use this directly.
- Or browse: `graft/INDEX.md` lists every node; follow the links.
- Monorepos and folders of multiple repos rank fairly across sub-projects —
  hits carry `[scope/]` labels naming which one they're from. Narrow with
  `graft ask "<task>" --in <scope>/` once you know where you're working.

If a returned span is truncated ("+N more lines"), open the file at that exact
range before finalizing. Only open source files when a node genuinely lacks a
needed detail, and then at the exact file:line the node points to — never
re-read whole files.

After big code changes, refresh the graph with `graft build` (deterministic,
no API key, $0).
<!-- graft:end -->
