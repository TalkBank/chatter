# Proposal: CHAT Span Construct for Code-Switched Regions

**Status:** Draft, design proposal, needs CHAT manual maintainer sign-off
**Last modified:** 2026-06-14 19:57 EDT
**Scope:** Propose a new CHAT main-tier construct to delimit multi-word code-switched spans, filling the gap between the existing utterance-wide `[- lang]` and single-word `@s` markers. Motivated by the L2 morphotag rollout (2026-04-15) and patterns observed in the 54-file validation corpus.

---

## 1. The gap in CHAT today

CHAT has two mechanisms for marking non-matrix-language material on
the main tier:

| Scope | Syntax | Use |
|---|---|---|
| Whole utterance | `*CHI: [- eng] hello how are you .` | The utterance is entirely in the declared non-primary language |
| Single word | `word@s` or `word@s:eng` | Exactly one word is code-switched |

There is **no scoped, multi-word construct** between these two
extremes. When a transcriber encounters a 5-word English phrase
embedded in a Welsh utterance, the CHAT manual currently requires one
of three unsatisfactory choices:

1. Tag every word with `@s`:
   ```
   *JEA: a wedes i "oh@s come@s on@s let's@s take@s this@s seriously@s now@s" .
   ```
2. Break the utterance into two utterances to use `[- lang]` on the
   English chunk (loses the fact that it's a single speaker turn and
   destroys timing alignment).
3. Under-tag, marking only the first word or only the content words,
   and hope downstream tools cope.

Option (1) is what the CHAT manual effectively prescribes and what
our eval corpora overwhelmingly contain. Option (2) is a
transcription policy mistake. Option (3) is what real transcribers do
when they get tired, which is constantly.

## 2. Evidence that the gap is load-bearing

### 2.1 Prevalence of multi-word spans in the validation set

From the 2026-04-15 aggregate evaluation (`batchalign3/book/src/reference/l2-eval-runs/2026-04-15/`):

- 16,845 total `@s` words across 54 files
- The longest contiguous `@s`-run observed was **25+ words in a
  single turn** (`fin,swe / lfsma36h.cha`):

  ```
  *INK:   jaa (.) då (.) ja måst säja att ja få (.) få tacka ...
          eh jos@s sä@s ajattelet@s tuleek@s sulle@s mieleen@s
          ajatuksia@s (.) tän@s jutun@s pohjalta@s mitä@s me@s
          ollaan@s täällä@s tänään@s tehty@s tai@s eh@s muita@s
          jotka@s liittyy@s tähän@s meidän@s hommaan@s niin@s
          kirjota@s muistiin@s ne@s .
  ```
- Typical "long quote" cases in `cym,eng` and `eng,spa` run 6-12
  contiguous `@s` words (quoted English dialogue inside a Welsh or
  Spanish narrative).
- In `eng,spa / sastre01.cha`:

  ```
  *SOF:   look@s yo I@s found@s that@s that@s that@s Saint@s Thomas@s
          started@s too@s early@s .
  ```

  Every one of those words has to be manually suffixed.

### 2.2 Known downstream cost

- **Transcriber effort.** Every word in a span requires a keystroke
  of `@s` appended. For a corpus like `cym,eng` (3,237 `@s` words
  across 3 files), a span construct would reduce the number of
  annotations the transcriber has to produce by an order of magnitude.
- **Transcriber error.** Under-tagging is the most common problem in
  real bilingual transcripts. Scanning the validation set, multiple
  files show partial `@s` runs (first word tagged, rest bare) where
  the obvious intent was a span. Per-word tagging makes
  under-tagging silent; a span construct makes it explicit (you
  either open a span or you don't).
- **Readability.** Long `@s@s@s@s` runs obscure the utterance
  structure. A reviewer reading a Welsh-English corpus spends more
  visual effort parsing the `@s` noise than the actual content.
- **Provenance in the AST.** Our current AST records each `@s` as a
  per-word attribute. "This 8-word run is one English insertion" is
  a fact the AST cannot express; it can only report 8 independent
  English-flagged words that happen to be adjacent.

### 2.3 Confusion with the L2 dispatch algorithm

A downstream per-word language-routing pipeline must group contiguous
`@s` words into spans before sending them to a secondary language
model, reconstructing at runtime what a span construct would encode at
the source level. A syntactic span construct would let the AST
carry the group directly, making dispatch simpler and making invalid
partial tagging impossible by construction.

## 3. Design goals

Any proposal should satisfy:

1. **Scoped, not utterance-wide.** The span must be able to appear
   mid-utterance and coexist with matrix-language words before and
   after it.
2. **Consistent with existing CHAT idioms.** CHAT already has a
   `<text> [annotation]` scoping pattern used for retrace, overlap,
   repetition, pause, and many others. A span construct should fit
   that family, not introduce a new delimiter class.
3. **Backward compatible.** Files without the new construct must
   remain valid. The existing `word@s` form must continue to work for
   single-word switches.
4. **Unambiguous with an explicit language when needed.** Parallel
   to `word@s:eng`, the span form must accept an optional language
   code when the utterance declares more than two languages.
5. **Parse-friendly.** Must be expressible in the tree-sitter
   grammar with minimal disruption, and must not conflict with any
   existing bracketed annotation.
6. **Nesting-safe.** Retrace markers (`[/]`, `[//]`, `[///]`),
   overlap markers (`[<]`, `[>]`, `[<N]`), and group markers must
   continue to work inside and around the span.

## 4. Design space (alternatives considered)

Five candidates, in order of how well they fit CHAT conventions:

### (A) Bracketed annotation: `<word word word> [@s]`

```
*CHI: ik weet niet <how to do it> [@s] .
*JEA: a wedes i "oh <come on let's take this seriously now> [@s]" .
```

Pros:
- Reuses the established `<...> [...]` scoping pattern. Parser and
  editor support already exist (overlap, retrace, repetition all use
  this shape).
- Natural to read: the bracketed annotation describes the scope.
- Minimal grammar change, one new annotation variant.
- `[@s:lang]` form available by parallel with `@s:lang`.

Cons:
- Two keystrokes more than just wrapping the span (you still need
  the `<>` and the `[@s]`).
- Must pick precedence when `[@s]` combines with overlap or retrace
  brackets on the same `<>`.

### (B) Explicit open/close sentinels: `@s< word word word @s>`

```
*CHI: ik weet niet @s< how to do it @s> .
```

Pros:
- Visually emphasizes that `@s` opens and closes a region.
- No dependence on existing scoping machinery.

Cons:
- Introduces a new delimiter class, breaks the "annotation always
  lives in `[...]`" rule.
- Tokenization becomes ambiguous at word boundaries (is `@s<` a
  word? a sentinel? does `how@s<` mean something?).
- Unusual for CHAT; fits programming-language conventions more than
  transcription conventions.

### (C) Trailing range: `word..word@s`

```
*CHI: ik weet niet how..it@s .
```

Pros:
- Compact.

Cons:
- Doesn't survive any editing of the span (inserting a word breaks
  it).
- Ambiguous with existing uses of `..` (pauses).
- Fails on spans containing commas, clitics, or CHAT markup.

Rejected.

### (D) XML-like tags: `<eng>word word word</eng>`

Pros:
- Precedent in other encoding standards (TEI `<foreign xml:lang=...>`).

Cons:
- CHAT doesn't use XML syntax anywhere else. Adding it here creates
  a foreign island.
- Visually heavier than `<...> [@s]`.

Rejected.

### (E) Extend `[- lang]` to scoped: `<span> [- eng]`

```
*CHI: ik weet niet <how to do it> [- eng] .
```

Pros:
- Reuses an existing marker.

Cons:
- `[- lang]` has utterance-wide semantics ("the primary language for
  this utterance is X"). Overloading it to mean "this scoped region is
  X" conflates two distinct concepts.
- Our L2 morphotag dispatcher currently interprets `[- lang]`
  differently from `@s`; the merge and routing semantics would have
  to be reconciled. Not worth the behavioral risk when `@s` already
  has the right semantics.

Rejected as overload; mentioned here because it will come up in
discussion.

## 5. Proposal

**Adopt option (A): `<span> [@s]`, with optional language code `[@s:lang]`.**

### 5.1 Syntax

```
<word [word ...]> [@s]
<word [word ...]> [@s:lang]
```

where:

- `word` is any CHAT word token (ordinary word, filler `&-`, nonword
  `&~`, etc.).
- `lang` is an ISO 639-3 code matching one of the codes declared in
  `@Languages`.
- The `<>` scope contains at least **two** word tokens. A single-word
  span degenerates to `word@s`; the validator should reject
  single-word spans and suggest the per-word form.

### 5.2 Semantics

`<w1 w2 ... wN> [@s]` is semantically equivalent to
`w1@s w2@s ... wN@s`. The AST preserves the span grouping but
all `@s`-consuming code paths (L2 dispatch, splicer, language
detection, validation) treat the contained words as if each carried
`@s`.

With an explicit language, `<w1 w2 ... wN> [@s:eng]` is equivalent to
`w1@s:eng w2@s:eng ... wN@s:eng`.

### 5.3 Examples

Bare form:

```
*CHI:   ik weet niet <how to do it> [@s] .
```

With explicit language:

```
*JEA:   a wedes i "<oh come on let's take this seriously now> [@s:eng]" .
```

Mixed in an utterance with both single-word and span code-switching:

```
*SOF:   look@s yo <I found that that that Saint Thomas started too early> [@s] .
```

Within a trilingual corpus (Catalan-Hungarian-Spanish):

```
*CHI:   <ha tirado la pelota> [@s:spa] .
```

### 5.4 Interaction with existing constructs

| Construct | Inside the span | Surrounding the span |
|---|---|---|
| Retrace `[/]` `[//]` `[///]` | Allowed inside inner `<>` as today: `<<said> [/] said something> [@s]` | Allowed |
| Overlap `[<]` `[>]` `[<N]` | Not allowed, overlap attaches to utterance-level `<>` | Allowed as separate bracket after `[@s]`: `<...> [@s] [<]` |
| Repetition `[x N]` | Allowed: `<word word> [@s] [x 2]` | Allowed |
| Best-effort guess `[?]` | Allowed: `<...> [@s] [?]` | Allowed |
| Comment `[% ...]` | Allowed: `<...> [@s] [% English insertion]` | Allowed |
| Pause `(.)` | Allowed inside: `<how to (.) do it> [@s]` | Allowed |
| Group `<...>` nested | Allowed up to one level; deeper nesting rejected |, |

Precedence rule for a `<>` with multiple annotations: **a `<>` may
carry at most one `[@s]` or `[@s:lang]` annotation**, but may carry
any number of orthogonal annotations (`[/]`, `[<]`, `[x N]`, etc.)
in any order. The span annotation applies to the whole scope
regardless of co-annotation order.

### 5.5 What the span does NOT do

- **Not an utterance-language override.** `[- lang]` still sets the
  primary language for the whole utterance. `[@s]` spans mark
  embedded material within that primary language.
- **Not a dependent-tier marker.** The span only appears on the main
  tier. Dependent tiers (`%mor`, `%gra`, `%wor`, etc.) continue to be
  word-aligned; the span has no representation on dependent tiers
  beyond the per-word effect already produced by `@s`.
- **Not a replacement for `@s`.** Single-word insertions remain
  `word@s`. The span is strictly for N ≥ 2.

## 6. Grammar and parser impact

### 6.1 Tree-sitter grammar change

The tree-sitter grammar in `grammar/` already models
`<scope> [annotation]` as a shared production family. Adding `@s`
spans is one new annotation variant under the existing scoping rule.
Estimated impact:

- `grammar.js`: one new alternative in the bracketed-annotation enum,
  matching `@s` optionally followed by `:` and a language code.
- `corpus/`: new test cases for the valid forms and invalid corner
  cases (single-word span, span with nested `[@s]`, span with
  unknown language).
- No change to tokenization, `@s` already exists as a word suffix
  and as a bare token; extending it to an annotation keyword is
  disjoint from its per-word use because the context (`[...]`)
  disambiguates.

### 6.2 Spec-driven validation

Add validation rules to `spec/`:

- `E7xx: Span must contain at least 2 words` (reject single-word `[@s]`)
- `E7xx: Span language must be declared` (reject `[@s:xyz]` when `xyz`
  not in `@Languages`)
- `E7xx: @s annotation conflicts with @s suffix` (reject
  `<word@s word@s> [@s]`, redundant and ambiguous)
- `W7xx: Span of length 1 should use word@s form` (warning, not error;
  covers single-word span for consistency)

### 6.3 Rust model impact

In `talkbank-model`, add a `CodeSwitchSpan` variant to the
`UtteranceContent` enum, parallel to `Retrace`, `Repetition`, etc.
The variant carries the inner content and the optional
`LanguageCode`.

In the content walker (`walk_words` / `walk_words_mut`), the span
variant recurses into its inner words exactly as any other grouping
variant does, with the added effect that each contained word is
treated as if `@s`-marked. Consumers that already handle per-word
`@s` get span support for free; span-aware consumers (L2 dispatcher,
serializer, validator) can inspect the span variant directly.

Estimated code impact: ~200 lines of new Rust code + tests. Low risk
given the existing scoping machinery.

### 6.4 Serializer

`to_chat_string` emits `<w1 w2 ...> [@s]` or `<w1 w2 ...> [@s:lang]`
when serializing a `CodeSwitchSpan`. Round-trip parity tests in the
existing corpus suite cover the shape.

### 6.5 L2 dispatcher simplification

Downstream L2 dispatchers currently reconstruct dispatch spans from
contiguous `@s` words at runtime.
With the new AST span variant, the reconstruction becomes trivial
for span-sourced material: the dispatch span *is* the
`CodeSwitchSpan`. Per-word `@s` and mixed cases still go through
the runtime grouping as today. Net effect: less grouping logic, not
more.

## 7. Backward compatibility

- Existing files are unaffected; they contain no `[@s]` brackets.
- Existing per-word `@s` continues to work unchanged for single-word
  switches and for transcribers who do not adopt the span construct.
- The new construct is purely additive. No change to `@Languages`,
  `@Participants`, `@ID`, or any header.
- Files authored with the new construct are forward-compatible with
  tools that don't yet know about it only if those tools fall back to
  ignoring unknown bracketed annotations (CLAN's traditional
  behavior). The span's *content* (the words inside `<>`) is
  unchanged; only the `[@s]` annotation is skipped. Consumers that
  need the L2 signal will upgrade.

## 8. Migration for existing corpora

An opt-in tool would convert contiguous per-word `@s` runs into
spans:

```bash
tb chat condense-l2 --corpus childes-biling-data/ --min-span 3
```

The tool walks the AST, finds contiguous `@s` runs of length ≥ N, and
rewrites them as `<span> [@s]`. Preserves explicit `@s:lang` tags
(promotes to `[@s:lang]` on the span). Refuses to condense across
retrace / overlap / group boundaries. Output is AST-walked, not
regex-based (per cross-repo Rule 17).

No existing corpus is forced to migrate. The condensation tool is a
convenience for transcribers who want cleaner source files.

## 9. Open questions for the CHAT manual maintainer

1. **Minimum span length.** The proposal says N ≥ 2. Is there a case
   for allowing N = 1 as a stylistic choice, or is forcing single-word
   switches to use `word@s` correct?
2. **Permitted languages inside a span.** Should a span be
   homogeneous (all words in one language) or allow sub-tagging?
   Current proposal: homogeneous. A mixed span is implausible in
   practice.
3. **CHAT manual placement.** The new construct belongs alongside
   `@s` in the manual. Which section, 8.3 (word-level language
   markers), or a new subsection covering both?
4. **Preferred bare vs explicit form.** When `@Languages: cym, eng`
   declares exactly two languages, should transcribers write `[@s]`
   (bare) or `[@s:eng]` (explicit)? The per-word convention allows
   both. Recommend: bare is canonical for two-language corpora,
   explicit is required when three or more languages are declared.
5. **`[- lang]` overlap.** If an utterance already declares
   `[- eng]`, is it meaningful to have `<...> [@s:eng]` inside it?
   Current proposal: allowed but linted (`W7xx: redundant span
   language matches utterance language`).
6. **Terminology.** The construct is called a "code-switched span"
   in this doc. Should the manual call it a "switch span", an "L2
   span", or retain the generic "`@s` span"? Not a semantic question,
   but worth settling once.

## 10. Recommended next steps

1. **Circulate this doc to the CHAT manual maintainer** for review of the semantic model
   and the open questions in §9.
2. **If accepted, add a CHAT manual section** describing the
   construct with 2-3 worked examples.
3. **Grammar change** in `grammar/` with corpus tests.
4. **Rust model + serializer + validator** in parallel with (3).
5. **L2 dispatcher simplification** once the AST variant lands.
6. **Migration tool** (`tb chat condense-l2`) to offer transcribers
   a one-shot conversion of existing files.
7. **Book pages** in `book/` and
   `batchalign3/book/src/reference/l2-morphotag.md` updated to
   reflect the new construct.

Estimated total effort, end-to-end: **4-6 engineer-weeks**,
dominated by the CHAT manual revision (manual-maintainer time) and the
book/validation-spec sweep. Code change is the smallest part.

## 11. Bottom line

The `@s`-per-word form was adequate when bilingual transcripts were
rare and short. With L2 morphotag now default-on, 16,845 `@s` words
across 19 pairs validated, and long contiguous runs the norm in
several of those pairs, the per-word form is actively harmful to
transcriber productivity, corpus readability, and error rates. A
scoped `<span> [@s]` construct fits cleanly into existing CHAT
idioms, is cheap to implement, is strictly additive, and would make
the next generation of bilingual corpora substantially easier to
produce and consume.

---

## Related documents

- `batchalign3/book/src/reference/l2-morphotag.md`: feature design
- `batchalign3/book/src/reference/l2-morphotag-ungating-decision.md`
- `docs/investigations/2026-04-21-l2-morphotag-corpus-state.md`: quality assessment of the 19-pair validation run
- `docs/investigations/2026-04-21-hindi-language-handling.md`: related language-handling report
- CHAT manual: `https://talkbank.org/0info/manuals/CHAT.html` §8.3 (word-level language markers)
