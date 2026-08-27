# CLAUDE.md

**Last modified:** 2026-08-27 14:09 EDT

Guidance for Claude Code in `TalkBank/chatter`. This file carries the rules
and an index; procedures live in the book (`book/src/`) and in per-module
CLAUDE.md files. Everything here is public: no private paths, hosts,
operators or data.

## Repo positioning

This repository is the canonical home of the TalkBank CHAT format authority
and the `chatter` tool family, and the source of truth for the `chatter`
binary. The CHAT core is self-contained: it builds and runs with no external
repository, and downstream consumers depend on its crates directly.

**Scope: general-purpose CHAT tooling only.** Nothing specific to one corpus,
one data provider or one workflow. Where a general capability needs
per-corpus input it takes a documented corpus-agnostic form (the
`--session-context` JSON seam); producing that input is a downstream concern.

**Git hygiene.** Never `git push --force`, never `--no-verify`, never push to
the `archive` remote, never change visibility or push a shared branch without
the maintainer's sign-off. `CONTRIBUTING.md` covers content hygiene.

## THE LOOP

The development process. Every rule is enforced by the thing named beside
it; a rule with no enforcement is a wish.

1. **Inner loop: `just test`.** Red first (a type change or a failing test),
   then green. Nothing else runs per edit. **The red must be VISIBLE before
   the implementing edit**: the failing test's output, or the compiler
   refusing the old call sites. A red nobody watched is a red nobody ran, and
   that is what the skipped steps looked like from outside. Enforced by
   `.githooks/commit-msg`, which refuses a commit changing production Rust
   with no test, spec, corpus or fixture staged beside it unless the message
   names what was red in a `Red:` trailer
   (`scripts/lint/production-rust-needs-evidence.sh`; `just
   evidence-gate-test` proves both directions).
2. **Touched `grammar/`, `spec/` or a registry: `just regen`, then `just
   test`, once.** Every derived artifact, one command, dependency order.
   Enforced by each artifact's currency test, which fails the gate when it is
   stale.
3. **One review, on the final diff, before the commit.** Five angles: reuse,
   simplification, efficiency, altitude, typestate. Not after every edit.
4. **Every commit reduces the bug count and says what it removed.** A commit
   that adds a defect is negative progress: it costs the fix, the retraction
   and a squash. Write slowly and correctly, and be adversarial toward your
   own tests: a test written to pass covers only the states its author
   listed.
5. **Before push: `just gate`, once.** Static checks plus every test CI
   runs, a few minutes. It mirrors per-push CI exactly, so CI confirms and
   never discovers; a red CI is a process failure. Enforced by
   `.githooks/pre-push`, which refuses a push without the stamp. The stamp is
   a hash of tree content (`scripts/tree-stamp.sh`), so gating a dirty tree
   and then committing the same bytes is valid. Clippy and the feature-off
   build are `just release-lint`, run before a release, never per push.
6. **No push without the maintainer's word, ever.** No hook can know this; it
   is the standing rule and has no exception.
7. **Release: `just fmt`, `just release-lint`, `just gate`, then squash every
   commit since the last tag into one release commit whose message is the
   CHANGELOG section**, gate once more on the squashed tree (the content stamp
   survives a squash), push on the maintainer's word, CI, `just release-tag
   X.Y.Z`. Public history is one commit per release.

**The spec system is the test corpus.** A construct with no spec example is
the gap to fix. Nothing on this path needs data outside the repository, so
every step can be run by any contributor.

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

## Danger rules

1. **`just gate` is the pre-push gate**, and it is the only one. `just test`
   is `--tests` only and cannot see doctests; never substitute it for the
   gate. **The justfile owns every command CI runs**;
   `scripts/check_ci_gate_sync.py` fails if a workflow runs anything else or
   if a recipe CI invokes is missing from the gate.
2. **Never run two cargo-family commands concurrently against one workspace**
   (the root or `spec/`). The target-dir lock serializes them silently.
3. **`grammar/src/parser.c`, `node_types.rs`, `generated_traversal.rs`, the
   conformance inventory, the spec fixtures and snapshot, the symbol and
   form-marker sites, and `schema/chat-file.schema.json` are all generated.**
   Never hand-edit any of them; `just regen` rebuilds every one, and a
   currency test refuses each stale one. `tree-sitter test` does not detect a
   stale `parser.c`.
4. **`release.yml` is generated by cargo-dist; never hand-edit.** Change
   `dist-workspace.toml` and regenerate.
5. **Test failures are bugs until proven otherwise: stop and ask.** Never
   update an expectation to match new behaviour without explicit approval.
   Doubly so for `cleaned_text()`, overlap markers, CA notation, lengthening,
   zero-words, and any grammar change that alters CST structure.
6. **No panics in long-lived code**: typed errors, no `unwrap`/`expect`. The
   workspace `[lints.clippy]` table denies the panic family, and every crate
   must declare `[lints] workspace = true` or it is silently exempt. Test
   code relaxes the family at its crate root.
7. **Never create ad hoc `.cha` test files.** Use `corpus/reference/` or the
   spec system. Every error code tests through `spec/errors/`.
8. **Program stack discipline:** `main()` spawns the program onto a 16 MiB
   thread; never move program logic onto the bare OS main thread. Gate:
   `crates/chatter/tests/stack_limit_tests.rs`.
9. **`.githooks/commit-msg` runs two gates, neither with a bypass flag.** A
   commit marked breaking must touch `CHANGELOG.md`, or a `type(scope)!:`
   subject is refused; `just breaking-changelog [ref]` is the release-time
   report. And production Rust must arrive with evidence of a red, or with a
   `Red:` trailer naming one; see THE LOOP step 1.
10. **`clippy` is release-time, not inner-loop and not per push.**
    `just release-lint` runs it with `-D warnings` over both workspaces plus
    the feature-off build; `release-lint.yml` runs the same on every tag. A
    finding you intend to keep gets a scoped `#[allow]` naming the reason.
    Never widen the gate.

## Type-oriented design is mandatory

**Every change makes illegal states unrepresentable and makes transitions
between well-defined states explicit.** It governs new code, old code, and
the design notes that precede either. Not a licence to rewrite: apply it to
what you touch and to new design.

**An affordance beats a rule.** If a rule here keeps being broken, the first
question is which type offers the wrong path, not how to word the rule
louder.

**Four shapes to recognise before writing the bug:**

- **A value proxies for a richer fact, and the two drift.** Derive it.
- **A sentinel is also a legal value.** A variant, an `Option`, or no
  `Default`.
- **A total function silently discards information.** Return it; make the
  lossy path the explicit one. Its commonest disguise is a log line.
- **Knowledge duplicated with no owner**, held together by a test asserting
  two things stay equal. One owner, then delete the test.

**Decision test before writing any type:** name a wrong value it permits and
ask what would notice. If the answer is a reviewer, a comment or a doc, the
type is wrong; if the compiler, it is right. **After any type change, count
what it removed**: lines, variants, checks, tests, branches. Nothing means
you relocated a defect rather than eliminating one.

**A single type is a point fix; the technique is a graph of them.** A node
per distinct meaning, an edge per transition, each edge `fn(Previous) ->
Next`, so the illegal state is unreachable rather than rejected. The tell
that a graph is missing: several values of the same primitive type flowing
through one pipeline, each meaning something different. Four steps: name
every space; make transitions the only route between them; construct only at
the boundary; make each sentinel a node. **A graph nobody travels is worth
nothing**: building it is half the work, deleting every route around it in
the same commit is the other half. `talkbank-model/src/alignment/indices.rs`
is the worked example.

**Hunt for typestate on every read and in every review.** Whenever a comment,
test name or docstring says "X happens only after Y", that is a sequence
maintained by convention and exactly what a phase type expresses. Do not
accept "no type expresses this" quickly. Two real limits: a scenario about
the outside world (a subprocess, a clock, the generated grammar) still needs
a test; and type-parameter typestate fights a collection of mixed-state
values.

**Fabricated values are banned, in new code and old.** A fabricated value is
one the code invents because a total function had nothing true to return:
`_ => Separator::Comma`, `unwrap_or("")` for an event, a struct literal with
placeholder fields. The cure is always to record the fact where it is known,
and to say "unknown" or "invalid" with a variant or a `Result`. The
enforcement is `clippy::wildcard_enum_match_arm`, denied per file as each is
cleaned; `talkbank-parser-tests/src/content_catch_alls.rs` is the inventory
of what is not yet protected, with a both-directions ratchet. Shapes the lint
cannot see are on you: `unwrap_or(<literal>)`, `unwrap_or_default()`, a
`Default` on a type whose wrong value is invisible, a sentinel that is also a
legal value.

**Every touch leaves the types better than it found them**, and deletes
whatever the improved type made impossible to fail. A ratchet that only
`main()` performs is not a ratchet: write a gate, run it, then break it on
purpose and watch it fail before believing any claim that rests on it.

**Remove tests by making illegal states unrepresentable.** A test guarding an
invariant is a standing admission that nothing enforces it. What legitimately
survives: wire formats, roundtrips between two separate functions,
measurements, policy choices with real alternatives, and behaviour a
signature cannot describe. A surviving test says which of those it is.

**Record what you notice even when you are not fixing it.**

## Cross-cutting design rules

1. Types are the first layer of documentation: newtypes at stable
   boundaries, no tuple-packed seams, enums over two or more bools.
2. Domain errors via `thiserror`; streaming diagnostics via `ErrorSink`;
   `ParseOutcome` for parse results; no silent swallowing (`.ok()`,
   `.unwrap_or_default()`).
3. Exhaustive matches on `UtteranceContent` and `BracketedItem`: no catch-all
   that discards content; all group types recurse.
4. "Consecutive" on the main tier always means in-order recursive traversal
   (`walk_words`), never flat-index adjacency.
5. Parse, don't validate: strict plus catch-all grammar pattern for closed
   header-value sets (`grammar/CLAUDE.md`).
6. **The production parser is driven by the generated typed CST traversal
   (`generated_traversal`, `NodeSlot`); hand-walking `node.kind()` and
   classifying ERROR-node text are banned.** The whole-tree recovery backstop
   is retained alongside per-position handling; both are load-bearing.
   `book/src/architecture/parsing.md`.
7. **Recovery is not validity.** A document that needed a recovery node is
   invalid; never drop a recovery node; never fabricate model values during
   recovery; parse taint (`ParseHealth`) gates alignment and validation.
8. Fix root causes, never symptoms. "Pragmatic" is banned as a justification
   for a band-aid.
9. **Types first, then red/green TDD top-down for what is left.** Before a
   failing test, ask what type change makes the defect unrepresentable, and
   prefer it; a change that introduces a type deletes the tests it obsoletes.
   For what a type cannot hold, the first failing test is at the bug's real
   boundary (CLI subprocess, real fragment through `parse_*`, `.cha` through
   validate, LSP request). **A construct or parser bug is fixed by writing the
   spec first**: the spec file is the failing test, `just regen` turns it into
   fixtures, and the expected CST is derived by parsing, never hand-written. A
   re2c-versus-tree-sitter divergence is a missing spec by definition.
10. **Performance regressions get a red test first, asserting counted work,
    never elapsed time.** `ValidationStatsSnapshot` exposes `cache_hits` and
    `cache_misses`; prefer an invariant that holds at every scale. A timing
    test is acceptable only as a coarse hang check with an order-of-magnitude
    ceiling, and says so.
11. **%mor is UD-only.** Legacy `&` fusional suffixes are unsupported.
12. Touched docs update `Last modified` from real `date` output; the book is
    kept current in the same commit as any behaviour change.
13. Architecture docs use Mermaid:
    `book/src/contributing/documentation-architecture.md`.
14. Structured `@Options` names are `CA` and `NoAlign` only.
15. **`From` is infallible; use `TryFrom` when construction can fail.**
16. **Never hand-parse CHAT with regex or string slicing.** CHAT content is
    read through the typed model and the fragment parsers; never fabricate
    `@UTF8`/`@Begin`/`@End` scaffolding to make a fragment parse; never
    re-parse text this codebase just serialized.
17. **Prefer `Result` to `Option` for anything that can fail.**
18. **Do not materialize intermediate collections**; prefer iterators and
    sinks. This code runs over six-figure corpora.
19. **Never revert a deliberate dependency bump to make a build pass.**
20. **Closed vocabularies have one owner in `spec/`, and every site that
    names them is generated.** Symbols from
    `spec/symbols/symbol_registry.json`; form markers from
    `spec/form_markers/form_marker_registry.json`. Before adding a list of a
    closed set anywhere, including a comment, ask whether it can be generated
    or linked instead.

## Building, testing, releasing

- Dev commands: the `justfile` and `book/src/contributing/dev-checks.md`.
- **Parser and model regression gates**, all in `just test`: the parity
  oracle `equivalence_reference_corpus` (both backends over the reference
  corpus, compared with `SemanticEq`); `reference_corpus_parses`;
  `roundtrip_reference_corpus`; the spec observation snapshot
  (`spec/observations/example-diagnostics.json`), which records for every
  spec example the codes each stage emitted and whether the parsed model
  serializes back byte-exact; and `KNOWN_DIVERGENCES`, the backend-parity
  baseline per spec case. A diff in the snapshot or the baseline is
  adjudicated in the commit as intended or unintended, never regenerated
  blindly.
- Releases: cargo-dist on `vX.Y.Z` tags, via two commands and never a hand
  bump or raw `git tag`: `just release-bump X.Y.Z`, then after CI is green on
  the pushed squash commit, `just release-tag X.Y.Z` (fail-closed on drift, a
  missing CHANGELOG section, or CI not green on the exact commit).
  `book/src/contributing/ci-and-release.md`. The desktop version inherits the
  workspace version; `tauri.conf.json` carries no `version` key.

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

## Coding standards

`book/src/contributing/coding-standards.md` and
`coding-standards-extended.md`: newtypes, integer discipline, closed-set
enums, string-literal policy, path discipline, rustdoc as primary
documentation, file-size limits (400 recommended, 800 hard). Conventional
Commits for messages.

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

## The book

`book/` is the canonical documentation, built in CI with a link check
(`just book` locally). One top-level README.md; everything else is a book
chapter. Release notes are `CHANGELOG.md` via include, never a copy.

## Relationship to batchalign

This repo contains no Batchalign code; the ML pipeline consumes chatter's
crates from its own repository.
