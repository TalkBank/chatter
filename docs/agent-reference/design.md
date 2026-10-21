# Design reference

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/agent-reference/design.md)

Read the sections relevant to your task. [AGENTS.md](../../AGENTS.md)
is the canonical policy entry point and resolves workflow conflicts here.
Dated incidents, measurements, versions and paths are historical evidence;
verify current source and live artifacts before relying on them. Inline
repository paths are relative to the repository root unless stated otherwise.

## Type-oriented design is mandatory

### Architecture-native recovery and diagnostics

Error recovery and reporting must be idiomatic to the tree-sitter and typed-AST
architecture. Derive findings from CST kinds, source-bound recovery evidence
and context supplied by the owning traversal. Do not reconstruct a parallel
parser from raw line prefixes or token heuristics merely to reproduce another
validator's diagnostics. Lexical checks and inspection of genuinely unparsed
input need an explicit boundary justification and must retain their source
association; they must not invent structure or certainty.

Specific, actionable messages are valuable when the available evidence proves
the narrower fault. Otherwise retain an honest broader diagnostic. Grammar
rejection or a producer-enforced invariant may subsume another validator's
check; matching its diagnostic code, stage, count or wording is not a goal.

### Type-oriented implementation

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
  two things stay equal. Establish one owner, then remove only the redundant
  synchronization check, preserving that owner's boundary contracts.

**Decision test before writing any type:** name a wrong value it permits and
ask what would notice. If the answer is a reviewer, a comment or a doc, the
type may need stronger admission. Compiler rejection closes a caller route,
but cannot establish the semantic correctness of the admitting constructor.
After a type change, identify the forbidden state or transition it removed
and the evidence for its boundary. Counts of lines, branches or tests removed
are not correctness evidence.

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

**Improve types within the authorized scope.** Remove obsolete routes only
after their replacement invariants are established. Verify an enforcement
claim at its actual boundary, including a negative control. Reuse existing
verification owners; a new automatic gate requires workflow approval.

**Use types and boundary tests together.** Private constructors and consuming
transitions close caller bypasses; tests still establish correct admission,
wire behavior, policy and public-boundary outcomes. Retain legal and rejected
constructor cases and meaningful compile-fail controls. Remove only tests whose
specific obligation has become redundant, naming its replacement evidence.
Introducing a type does not by itself justify deleting its boundary tests.

**Record what you notice even when you are not fixing it.**

## Cross-cutting design rules

1. Types are the first layer of documentation: newtypes at stable
   boundaries, no tuple-packed seams, enums over two or more bools.
2. Domain errors via `thiserror`; streaming diagnostics via `ErrorSink`;
   explicit outcomes for parser production. Streaming `ParseOutcome` and
   whole-document `ParseProduct` have different diagnostic ownership; neither
   certifies validity. Use `Result` for fallible operations where appropriate;
   do not silently swallow failures through `.ok()` or defaults.
3. Exhaustive matches on `UtteranceContent` and `BracketedItem`: no catch-all
   that discards content; all group types recurse.
4. "Consecutive" on the main tier always means in-order recursive traversal
   (`walk_words`), never flat-index adjacency.
5. Parse, don't validate: strict plus catch-all grammar pattern for closed
   header-value sets (`grammar/AGENTS.md`).
6. **The production parser is driven by the generated typed CST traversal
   (`generated_traversal`, `NodeSlot`); hand-walking `node.kind()` and
   classifying ERROR-node text are banned.** The whole-tree recovery backstop
   is retained alongside per-position handling; both are load-bearing.
   `book/src/architecture/parsing.md`.
7. **Recovery is not validity.** A document that needed a recovery node is
   invalid; never drop a recovery node; never fabricate model values during
   recovery. **Two facts, never one:** `ParseHealth` taint answers "is
   CROSS-TIER alignment involving this domain trustworthy" and is also set for
   faults in OTHER tiers; whether a tier lost content of its own is the tier's
   business (`GraCompleteness`). Reading the first as the second withheld four
   `%gra` rules from a tier that parsed perfectly, and reading it as "these
   labels are not the author's" withheld E761 from relations that are.
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
12. Keep the book current with behaviour changes. Publication metadata uses
    Git-derived page dates or source-bound history links; it is not a content
    review certificate. Update remaining handwritten dates from real `date`
    output on edits. The date convention has one owner in
    `book/src/contributing/documentation-architecture.md`.
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
21. **Before any code that pairs, walks, counts or aligns two things, find
    the owner and travel its whole path.** One search for `single owner`,
    `canonical`, `Projection`, `Binding` in the domain, then read the owner
    to its LAST verdict, not its first. Three reviewed drafts of the `%wor`
    sanitizer (2026-09-08) each re-implemented a verdict
    `WorMainTierProjection` already held, one path step further each time
    (`bind_timing`, then `corroborate_wor_timing`). Where the route around
    an owner still type-checks, close it, and name in the commit any route
    left open: the counter and the extractor can no longer be asked a
    `%wor` count (`PositionalDomain` has no `Wor`), and the overlap
    collector walks with the projection's own leaf set.

22. **Source locations are a typed fact, never a sentinel.** `Span` is
    geometry only (`start <= end`, asserted in every profile, no `Default`).
    Every model value carries `location: Location` (`Located(Span)` or
    `Unlocated`), given to its constructor as the first argument: a parser
    passes the node's span, a builder or JSON readback writes `Unlocated`.
    There is no setter, default or placeholder span; AST JSON carries no
    locations. A consumer leaves a `Location` only through `match` or
    `span() -> Option<Span>` and states its unlocated policy at that arm.
    Diagnostics carry `DiagnosticLocation` (`Known` with an optional paired
    `LineColumn`, or `Unlocated`), with `From<Location>`. **Presence decides,
    location points:** whether a validation rule fires depends on the
    construct existing in the model, never on its having a span; the span
    only chooses where the diagnostic points. A value with no location of its
    own reports where its parent narrowed the `ValidationContext`
    (`field_diagnostic_location`), or `Unlocated`, never byte 0. The parser
    crates are held on production `Location::Unlocated` by the fabricated-AST
    gate (tree-sitter at zero). Full contract:
    `book/src/architecture/errors-and-validation/chat-core-errors.md`.

## Coding standards

`book/src/contributing/coding-standards.md` and
`coding-standards-extended.md`: newtypes, integer discipline, closed-set
enums, string-literal policy, path discipline, rustdoc as primary
documentation, file-size limits (400 recommended, 800 hard). Conventional
Commits for messages.
