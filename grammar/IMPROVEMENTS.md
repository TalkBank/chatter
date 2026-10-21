# Grammar Improvement Ideas

**Status:** Unscheduled idea list, not current behavior or an accepted plan
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/IMPROVEMENTS.md)

Ideas for the tree-sitter CHAT grammar, first collected in May 2026 while
studying [tree-sitter-scala](https://github.com/tree-sitter/tree-sitter-scala),
a mature grammar with an external scanner, many declared conflicts and
multi-binding distribution. CHAT is a transcript format rather than a
programming language, but several patterns transfer.

Nothing here is scheduled or approved. Before acting on an idea, check the
current `grammar.js` and queries, and follow the
[Grammar Workflow](../book/src/contributing/grammar-workflow.md). Size and
count observations from the original study are omitted because they went
stale; the original wording remains in Git history. Query sketches below use
node names that exist in the current grammar, but none has been tested.

## Query File Enhancements

The query files are deliberately lean; each carries a `TODO(post-v1)` note.
`just grammar-test` checks only that they compile.

### Expand `highlights.scm` with predicate filters

**Current state:** Covers headers, speakers, words, events, linkers, a few
annotations, several dependent tiers, MOR/GRA internals and structural
punctuation. It treats all words uniformly and uses no predicates.

**What Scala does:** Uses predicate filters like `#match? @type "^[A-Z]"` to
highlight conditionally on node text.

**Ideas for CHAT:**

```scheme
; Highlight unrecognized headers differently from known ones
(unsupported_header) @warning

; Distinguish MOR POS categories by prefix pattern
(mor_pos) @type.builtin
; Could add: (#match? @type.builtin "^(n|v|adj|adv|pro)")
; to highlight open-class POS differently from closed-class

; Uncertain transcription ([?])
(scoped_uncertain) @warning

; Timing bullets
(bullet) @number

; Overlap points
(overlap_point) @punctuation.bracket

; Postcodes
(postcode) @attribute
```

**Effort:** Small; each capture is independent. Add highlight tests (below)
before relying on the result.

### Add `indents.scm` for auto-indentation

**Current state:** No `indents.scm` exists.

**What Scala does:** Uses `@indent.begin` and `@indent.end` captures so editors
such as Neovim can adjust indentation.

**Why this matters for CHAT:** A continuation line starts with a tab. When a
user breaks a long utterance or header value, an editor could insert the tab
automatically.

**Sketch:**

```scheme
; Continuation lines within free-text header values
(free_text
  (continuation) @indent.begin)
```

**Effort:** Small, but needs testing in each target editor. CHAT indentation is
a single structural tab, not nesting, so a query may be the wrong tool; editor
on-enter rules or the LSP may fit better.

### Expand `locals.scm` for richer speaker scoping

**Current state:** `source_file` is the scope, speaker codes in `participant`
entries are definitions and main-tier speakers are references.

**Ideas for CHAT:**

```scheme
; Speaker field of @ID headers. `id_speaker` is a raw text leaf,
; not a `speaker` node, so it can only be a reference by text.
(id_speaker) @local.reference
```

Dependent tiers belong implicitly to the preceding main tier's speaker; query
locals cannot express that association. Cross-tier speaker navigation and
hover belong in the LSP rather than in query locals.

**Effort:** Small for the query; cross-tier association is LSP work.

### Expand `tags.scm` for richer navigation

**Current state:** Speaker turns, participant declarations, the
`@Participants` header and the `@Begin`/`@End` anchors.

**Ideas for CHAT:**

```scheme
; Headers as section markers (for document outline)
(languages_header) @definition.module
(id_header) @definition.type
(options_header) @definition.type
(date_header) @definition.type

; Media references
(media_header) @reference.call

; Dependent tiers as sub-items of the preceding turn
(mor_dependent_tier) @reference.type
(gra_dependent_tier) @reference.type
```

**Value:** An outline showing headers as sections and speaker turns as items.

**Effort:** Small; adding capture patterns. Check how each client renders the
tag kinds before choosing them.

## Grammar-Level Improvements

### Named precedence groups

**Current state:** Precedence is numeric; the grammar has no `precedences`
array. The levels and their rationale are recorded in
[precedence decisions](docs/precedence-decisions.md).

**What Scala does:** Declares named precedence groups in a `precedences` array.

**Caveat:** Named groups order rule-level conflicts; `token(prec(N, ...))`
lexical priorities remain numeric. The two systems are complementary, so most
of this grammar's numeric levels (which are lexical) would stay. Any change
must leave the generated parser behavior unchanged, verified through the
Grammar Workflow.

**Effort:** Small if limited to documentation value; the benefit is modest.

### `inline` directive for intermediate rules

**Current state:** Not used.

**What Scala does:** Marks pure grouping rules as `inline`, removing them from
the tree.

**Idea for CHAT:** A wrapper that no consumer matches could be inlined.

**Caveat:** Inlining changes the CST shape, which changes the generated typed
traversal, node-kind constants, conformance inventory, typed lowering in
`talkbank-parser`, queries and spec-derived trees. Evaluate each candidate
against all of those consumers; this is a migration, not a local edit.

**Effort:** Medium to large per candidate.

## CI and Testing

### Real-corpus smoke test

**Current state:** CI runs `just grammar-test` (corpus tests and query
compilation) and the workspace's parser contracts, including equivalence over
the repository's reference corpus.

**What Scala does:** CI parses entire standard libraries as a smoke test.

**Idea for CHAT:** Parse a representative, redistributable sample of real
transcripts and fail on parser crashes. Real TalkBank corpora are not part of
this repository, so a sample would need to be curated and checked in.

**Caveats:**
- An `ERROR`-node count is not a correctness measure: valid recognition and
  intentional recovery both affect it, so a count threshold would encode
  neither validity nor a regression rule.
- A passing finite sample does not prove every parser state.
- Sample selection needs licensing and privacy review.

**Effort:** Medium.

### Highlight query tests

**What Scala does:** Keeps test files whose comment markers state the expected
captures; `tree-sitter test` verifies them.

**Idea for CHAT:** Add highlight tests under `test/highlight/` so changes to
`highlights.scm` are checked rather than inspected by eye. Tree-sitter writes
highlight assertions inside comments of the tested language, so first establish
how a CHAT test file can carry them.

**Value:** Turns highlight results into a checked contract.

**Effort:** Small to medium.

## Not Recommended for CHAT

These patterns were evaluated and judged inappropriate:

### External scanner (C/C++ custom lexer)

Scala needs a C scanner for significant indentation, string interpolation and
context-sensitive keywords. This grammar has no external scanner; CHAT's
line-oriented structure is handled with DSL rules, explicit whitespace and
precedence. A scanner would complicate the build, make the grammar harder to
modify and add serialization state for incremental parsing. Revisit only for a
concrete construct that the DSL cannot express.

### Dynamic precedence

`prec.dynamic()` resolves ambiguities by runtime parse state. Scala uses it for
constructs like dangling else. CHAT has not needed it; static precedence and
declared conflicts suffice for the current grammar.

### Many conflicts

Declare a conflict only for a real ambiguity that one token of lookahead cannot
resolve, with a comment explaining it, as the existing entries in `grammar.js`
do. Prefer restructuring rules over adding a conflict as a shortcut; each one
adds GLR paths.

## References

- [tree-sitter-scala grammar.js](https://github.com/tree-sitter/tree-sitter-scala/blob/master/grammar.js)
- [tree-sitter-scala scanner.c](https://github.com/tree-sitter/tree-sitter-scala/blob/master/src/scanner.c)
- [tree-sitter-scala queries/](https://github.com/tree-sitter/tree-sitter-scala/tree/master/queries)
- [tree-sitter-scala CI](https://github.com/tree-sitter/tree-sitter-scala/blob/master/.github/workflows/ci.yml)
- This grammar: `grammar.js` and the [architecture guide](GRAMMAR.md)
