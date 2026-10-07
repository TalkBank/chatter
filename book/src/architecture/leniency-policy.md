# Parser Leniency Policy

**Status:** Current
**Last updated:** {{git-dates:page}}

This document is the single source of truth for how the tree-sitter grammar,
Rust validation layer, and CLI tooling divide responsibility for enforcing the
CHAT specification. It consolidates decisions scattered across `grammar.js`
comments, analysis documents, and code.

> **Scope**: Documentation only. This document does not implement new validation
> rules; it records what exists, what is intentionally absent, and proposes a
> roadmap for closing gaps.

---

## Philosophy: Parse, Don't Validate

The tree-sitter grammar intentionally accepts a **superset** of valid CHAT. The
rationale:

1. **Maximise parse coverage**: Real-world `.cha` files contain legacy patterns,
   whitespace variations, and edge cases. A grammar that rejects them produces no
   AST and therefore no diagnostics. Accepting them gives the validation layer
   something to work with.

2. **Separate syntax from semantics**: The grammar captures structure (headers,
   utterances, tiers, annotations). The Rust validation layer enforces semantic
   rules (required headers, participant declarations, alignment counts).

3. **Enable configurable strictness**: Different consumers need different
   policies. A roundtrip pipeline can be strict; an editor providing live
   diagnostics should be lenient. Validation profiles (see
   [Validation Profile Infrastructure](#validation-profile-infrastructure)) make
   this possible.

### Three-Tier Classification

Every intentional leniency decision falls into one of three tiers:

| Tier | Label | Meaning |
|------|-------|---------|
| **A** | Parse-lenient + validate-strict | Grammar accepts it; validation **rejects** it as an error |
| **B** | Parse-lenient + validate-warning | Grammar accepts it; validation emits a **warning** |
| **C** | Parse-lenient only | Grammar accepts it; **no validation needed**: the construct is genuinely optional or the broad acceptance is by design |

This classification was proposed in an earlier grammar governance analysis and is
formalised here.

---

## Leniency Matrix

Master table of every documented leniency decision in the grammar. The
**Status** column indicates whether downstream validation compensates for the
grammar's permissiveness.

| # | Grammar Construct | Spec Requirement | Grammar Behavior | Tier | Validation | Error Code | Status |
|---|---|---|---|---|---|---|---|
| 1 | `@UTF8` header | Required, must be first line | Optional (not enforced) | A | Validated | E503 | OK |
| 2 | `@Begin` header | Required | Optional (`grammar.js` ~L104) | A | Validated | E504 | OK |
| 3 | `@End` header | Required | Optional (`grammar.js` ~L106) | A | Validated | E502 | OK |
| 4 | Pre-first-utterance header order | No enforced order (matches CLAN CHECK) | `choice()`, any order (`grammar.js` ~L122-135) | C | N/A (by design) |, | OK |
| 5 | Headers after utterances | Allowed (e.g. `@Bg`, `@Eg`, `@G`, `@Comment`) | Interleaved freely | C | N/A (by design) |, | OK |
| 6 | Content type context restrictions | Unified across contexts | Unified `base_content_item` (`grammar.js` ~L731-738) | C | N/A (by design); specific semantic rules (E371, E372) exist separately |, | OK |
| 7 | Terminator presence | Required (except CA mode) | Optional (`grammar.js` ~L691-692) | A | Validated | E305 | OK |
| 8 | Bare shortening as word | CA mode only | Accepted anywhere | A | Validated | E2xx | OK |
| 9 | Trailing whitespace in annotations | Not specified | Optional trailing space (`grammar.js` ~L957, 966, 975, 1004, 1013) | C | N/A |, | OK |
| 10 | MOR segment Unicode | Very permissive (broad language support) | Exclusion-based regex (`grammar.js` ~L1909-1915) | C | N/A (by design) |, | OK |
| 11 | MOR fusional suffixes with hyphens | ALNUM + IPA only | Allows hyphens (`grammar.js` ~L1942-1945) | C | N/A (by design) |, | OK |
| 12 | MOR nested translations | No nested structures | Allows `()` and `[]` nesting (`grammar.js` ~L1954-1966) | C | N/A (by design) |, | OK |
| 13 | Linkers / language codes | Truly optional | Optional | C | N/A |, | OK |
| 14 | Word annotations | Truly optional | Optional | C | N/A |, | OK |
| 15 | Media bullet | Truly optional | Optional | C | N/A |, | OK |
| 16 | Group whitespace (leading/trailing) | No whitespace inside `<` `>` | Optional (`grammar.js` ~L1097, 1099) | C | N/A |, | OK |
| 17 | Long feature label characters | Limited character set | `/[A-Za-z0-9@%_-]+/` (`grammar.js` ~L1327) | C | N/A |, | OK |
| 18 | Catch-all headers (`$.anything`) | Structured content for some headers | `/[^\r\n]+/` for ~19 header types | C | N/A (content is opaque) |, | OK |
| 19 | Header gap whitespace | Single space/tab | `repeat1(choice(space, tab))` (`grammar.js` ~L467, 477, 489) | C | N/A |, | OK |
| 20 | `@Types` header whitespace | No spaces around commas | Optional whitespace around commas (`grammar.js` ~L584-592) | C | N/A |, | OK |

---

## Permissiveness Regression Decisions

Several validation rules are deliberately permissive because stricter forms
produce false positives against the reference corpus. Each decision is
summarised here with its ruling and rationale (the permissiveness regression
log, archived, holds the full record).

### Decision 1: `[*]` bare annotation, E214 retired

- **Behaviour**: Bare `[*]` (an empty `ContentAnnotation::Error`) is accepted
  without error; no code is emitted for it.
- **Rationale**: Reference files (`errormarkers.cha`, `compound.cha`) use bare
  `[*]` as valid CHAT.
- **Why the number stays retired.** A code that names two different rules
  (the bare-`[*]` rule and "the scoped-annotation LIST is empty") leaves its
  documentation and its implementation free to disagree, and nothing detects
  the drift when neither rule can fire. `AnnotatedContentAnnotations` is
  non-empty by construction, so the empty-list rule is unrepresentable rather
  than merely unimplemented, and the bare-`[*]` rule stays retired on this
  decision's own reasoning.
- **Revisit**: If coded error annotations become required, that is a NEW code
  against the `ContentAnnotation::Error` payload, behind an explicit strict
  profile. Do not revive `E214`: it has meant two things already.

### Decision 2: `@t` without `@s:<lang>`, E248 disabled

- **Behaviour**: `@t` is accepted without requiring `@s:<lang>`; `E248` (the
  code for a `@t` marker lacking an explicit language marker) is not emitted.
- **Implementation**: `talkbank-model/src/validation/word/structure.rs` carries
  no such check.
- **Rationale**: Reference file `formmarkers.cha` contains `a@t` and is expected
  to be valid.
- **Revisit**: Scope to explicit strict validation mode if desired.

### Decision 3: Undeclared inline language codes, E254 retired

- **Behaviour**: An explicit word-level `@s:LANG` marker carries no
  requirement to be declared in `@Languages` (reference file `lang-marker.cha`
  stays valid). `E254` (`UndeclaredExplicitWordLanguage`) is a retired code and
  is not emitted.
- **Rationale**: `@Languages` declares the transcript's substantial
  languages; a one-word insertion is not substantial presence. CLAN CHECK
  imposes no `@s` declaration requirement either.
- **Neighbouring rules**: `E255`
  (`WholeUtteranceLanguageSwitchShouldUsePrecode`) covers whole-utterance
  `@s` runs that should use `[- lang]` precodes, and a `[- lang]` utterance
  precode whose language is absent from `@Languages` is `E755`.
- **Revisit**: The number is retired and not reused.

### Decision 4: Mixed-language digit legality, permissive-any rule

- **Behaviour**: For mixed/ambiguous markers, digits are accepted if legal in
  **at least one** applicable language (not required to be legal in all).
- **Implementation**: an `any()` over the applicable languages in
  `talkbank-model/src/validation/word/language/digits.rs`.
- **Rationale**: Prevents false positives in mixed-language reference examples.
- **Revisit**: Confirm spec intent for mixed/ambiguous validation semantics.

### Decision 5: `@Bg` nesting, same-label only

- **Behaviour**: `E529` only fires when nesting the **same label** (or
  same unlabeled scope key). Different labels may nest hierarchically.
- **Implementation**: a `same_scope_open` test (not "any scope open") in
  `talkbank-model/src/validation/header/structure.rs`.
- **Rationale**: Avoids false positives on hierarchical markup patterns (e.g.,
  HSLLD corpus).
- **Revisit**: Decide whether nesting policy should be global or per-label.

### Decision 6: Temporal bullets in CA mode, checked for every file

- **Behaviour**: `E701`/`E704` run for every file, CA included; there is no
  CA-mode skip.
- **Rationale**: the temporal rules carry a 500 ms tolerance and per-speaker
  semantics, and with them the CA reference files and all 994 kept
  CA-declared files validate clean (full-population measurement). A CA-mode
  skip has no CLAN CHECK counterpart and would be internally incoherent:
  `E362` bullet monotonicity runs on CA files, so `E701`/`E704` must too.
- **Revisit**: closed. No CA-specific temporal policy is needed: no policy
  difference between CA and non-CA files exists.

### Decision 7: Pipeline severity threshold, errors only

- **Behaviour**: Pipeline returns failure only if at least one diagnostic
  has `Severity::Error`.
- **Implementation**: `talkbank-transform/src/pipeline/parse.rs`.
- **Rationale**: Warnings should not block parse/transform/export pipelines.
- **Revisit**: Keep as default; add explicit `--strict` flag/profile if needed.

### Decision 8: Spacing warnings W210/W211, retired

- **Behaviour**: No style-level spacing warning runs around terminators and
  overlap markers; the core main-tier validation path in
  `talkbank-model/src/model/content/main_tier.rs` has no such pass.
- **Rationale**: Such warnings produce unexpected diagnostics on files treated
  as valid in the reference workflow.
- **Revisit**: CLOSED (maintainer ruling). Real CLAN CHECK accepts the W210
  construct (glued terminator), overlap markers hug their content by design
  so W211's shape is valid CA notation, and no production code emits either.
  The numbers are retired and not reused; no lint profile will reintroduce
  them. The living spacing rules are E243, E749, E750, E751, E757, and E758.

### Decision 9: Repeated `@Date` headers, accepted

- **Behaviour**: A file may carry any number of `@Date` headers, anywhere
  headers are allowed, with the same or different values, adjacent or
  separated. Each is validated on its own (`E516` empty, `E518`
  malformed); no rule relates one `@Date` to another.
- **Parity**: CLAN CHECK accepts the same three shapes: two identical
  `@Date` lines together, two different ones together, and a later
  `@Date` after utterances begin. Measured against CLAN `V 21-Sep-2026
  11:00` and chatter 0.27.0 with three minimal files, all accepted by both.
- **Why so permissive**: Repeated dates are legitimate CHAT in real
  corpora. Episode-structured transcripts put an `@Date` before each
  recording session, so the same date repeats whenever several episodes
  share a day. Diary corpora open each day with its own `@Date`, including
  days that produced no utterances. Sessions recorded over two days carry
  both dates. A narrower rule ("two adjacent `@Date` lines are an error")
  was proposed in 2026-10 for identical adjacent duplicates introduced by
  an export tool. The maintainer ruling is that chatter does not add
  `@Date` rules ahead of CLAN CHECK.
- **Revisit**: Only when CLAN CHECK adds a repeated-`@Date` rule. Chatter
  then follows it for parity, under a new code.

### Decision 10: `%wor` word intervals, reversed rejected, zero-duration legal

- **Behaviour**: A `%wor` word bullet that ends before it starts
  (`300_100`) is `E362` (`check_word_interval`). A zero-duration word bullet
  (`100_100`) is legal, and word bullets may overlap or start out of order.
- **Relation to CHECK**: CLAN CHECK `21-Sep-2026` checks no `%wor` bullets:
  it accepts both cases above, and reports its error 82 ("BEG mark of bullet
  must be smaller than END mark") only for main-tier bullets. Measured with
  the `E362` spec examples renamed to match `@Media`. Chatter goes beyond
  CHECK for the reversed case only.
- **Why**: the maintainer, 2026-10-06: "do what actually makes sense. CHECK is not
  God and neither are we." A reversed interval contradicts itself and has no
  reading as timing; no corpus file was found with one, so rejecting it costs
  nothing. A zero-duration interval places a word at an instant; at least
  185,540 such bullets in 1,102 corpus files pass today, and rejecting them
  would invalidate those files for no reader's benefit. A consumer that needs
  a positive interval for each word refuses it at its own boundary
  (`assess_wor_timing_sequence` yields `Rejected` with the slot named).
- **Planned**: zero-duration word bullets in existing files are aligner
  artifacts (words that were not located, recorded as instants). Once the
  affected files are regenerated, a zero-duration word bullet becomes `E362`
  as well, as a zero-duration main-tier bullet already is: a bullet that
  covers no time locates nothing.

---

## Validation Gap Roadmap

Concrete items where the grammar is lenient but no validation compensates.
Each proposes a new error code and priority.

### ~~Priority 1: `@UTF8` Presence (E503)~~, DONE

- **Grammar**: `@UTF8` is optional.
- **Spec**: Required, must be the first line.
- **Implemented**: `E503` (`MissingUTF8Header`) added to `check_headers()` in
  `talkbank-model/src/validation/header/structure.rs`.
- **Severity**: Error.
- **Note**: All 340 reference corpus files contain `@UTF8`, zero roundtrip
  impact.

### ~~Priority 2: Pre-First-Utterance Header Order (proposed E534)~~, Not a Gap

- **Grammar**: `choice()` accepts headers in any order between `@Begin` and the
  first utterance.
- **Assessment**: CLAN CHECK does not enforce any ordering for post-`@Begin`
  headers; it validates presence and format only. Our grammar's flexible
  ordering matches CHECK's behavior.
- **Status**: Reclassified from Tier B (GAP) to Tier C (by design).

### ~~Priority 3: Content Type Context Validation~~, Not a Gap

- **Grammar**: Unified `base_content_item` accepts any content type in any
  context.
- **Assessment**: The unified rule is correct by design. Nested groups are legal
  CHAT (e.g., `<the <dag> [: dog]> [= something]`). The two specific semantic
  restrictions that do exist (no pauses in pho groups, E371; no nested
  quotations, E372) are already validated.
- **Status**: Reclassified from Tier A (PARTIAL) to Tier C (by design).

---

## Validation Profile Infrastructure

### What Exists

#### Two kinds of setting, two types, two crates

What the validator COMPUTES and what a reader SEES are different questions, and
conflating them is not a style matter: it decides what a cached verdict means.
They are separate types, and deliberately not in the same crate.

#### `RuleSelection` (`talkbank-model/src/errors/config.rs`)

Which rules run. Every field here changes the diagnostics that exist, which is
why this type, and only this type, derives the validation cache key.

```rust,ignore
let rules = RuleSelection::new().with_strict_linkers(); // turns on the [Opt-in] codes
```

- `new()`: every always-on check, no opt-in check
- `with_strict_linkers()`: run the cross-utterance linker checks (chainable)
- `strict_linkers_enabled() -> bool`: query
- `cache_key_fragment() -> String`: the canonical text folded into
  `talkbank_cache::RulesVersion::current_with_rule_selection`. Destructures
  `Self` with no `..` rest pattern, so a new field is a compile error until
  someone folds it in.

#### `PresentationPolicy` (`talkbank-transform/src/presentation.rs`)

What a reader is shown, and at what severity, applied to diagnostics the
validator has ALREADY produced. `--suppress` lands here.

```rust,ignore
let policy = PresentationPolicy::new()
    .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
    .disable(ErrorCode::InvalidOverlapIndex)
    .upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
```

**API**: `new()`, `downgrade(code, severity)`, `disable(code)`,
`upgrade(code, severity)`, `set_severity(code, Option<Severity>)`,
`effective_severity(code, original) -> Option<Severity>`,
`is_disabled(code) -> bool`, `shows_everything() -> bool`,
`apply(diagnostic) -> Option<ParseError>`, `apply_all(Vec<ParseError>)`.

**Pre-built profiles**:
- `lenient()`: shows `IllegalUntranscribed` and `InvalidOverlapIndex` as
  warnings. For gradual migration of legacy corpora.
- `strict()`: shows unmapped warnings as errors. Explicit per-code overrides
  still take precedence, so a caller can opt a specific code back to
  `Severity::Warning`.

**Why the crate split.** `talkbank-transform` depends on `talkbank-cache`, so
the cache crate cannot name `PresentationPolicy`. Folding a display preference
into the cache key is therefore a dependency cycle rather than a judgement call.
Were a display preference part of the cache key, `--suppress` would partition
the cache: two runs differing only in what they printed would share no
entries, and a second pass over a large corpus would re-validate all of it
from cold.

**What this makes true of a cache row.** The stored fact is "this file produced
no diagnostics at all under this rule selection". No presentation policy can
change that, which is what lets one cache serve suppressed and unsuppressed runs
alike.

#### `ConfigurableErrorSink` (`talkbank-transform/src/presentation.rs`)

Wrapper that applies a `PresentationPolicy` to diagnostics on their way to an
inner `ErrorSink`, for surfaces that stream to a reader as they arrive.

```rust,ignore
let inner = ErrorCollector::new();
let sink = ConfigurableErrorSink::new(&inner, policy);
```

It must never wrap a sink whose output feeds a cache write or a run tally: those
consume the complete diagnostic set.

#### Runner-Level Flags (`talkbank-transform`, `chatter`)

| Flag | Effect |
|------|--------|
| `--skip-alignment` | Skip tier alignment validation |
| `--roundtrip` | Test serialization idempotency after validation |
| `--force` | Clear cache for path and revalidate |
| `--max-errors N` | Stop once N errors (never warnings) are found (N is at least 1) |

### What Is Missing

| Gap | Description | Effort |
|-----|-------------|--------|
| No `--profile` CLI flag | Users cannot select `strict` / `lenient` / `lint` from the command line | Medium |
| No profile serialization | Cannot load profiles from TOML/JSON config files | Medium |
| No corpus-specific profiles | E.g., HSLLD-specific rules | Future |

### Proposed Profiles

From the permissiveness regression log:

| Profile | Purpose | Behaviour |
|---------|---------|-----------|
| `reference-compatible` | Current permissive baseline | Default, matches current validation behaviour |
| `strict-chat` | Full spec enforcement | Re-enable selected tightenings (E248, etc.; E254 and E214 are retired codes and are not candidates) |

The roundtrip gate should be pinned to an agreed profile to prevent future
ambiguity about what "pass" means.

---

## Silent Recovery Points (NLP Pipelines)

An earlier Python-Rust boundary audit identified several
places where `batchalign-core` silently massages data without diagnostics. These
are related to leniency because they represent permissive acceptance without
transparency.

| Pipeline | Recovery Mechanism | Diagnostics? |
|----------|-------------------|-------------|
| Stanza morphosyntax | `retokenize.rs` DP alignment; `Word::new_unchecked` fallback | **No** |
| Whisper/Wave2Vec FA | `forced_alignment.rs` DP "best fit" | **No** |
| Google Translate | Imported verbatim into `%xtra` | **No filtering** |
| Stanza segmentation | Silent abort on assignment mismatch | **No** |

**Key infrastructure gap**: `ParseHealth` exists in `talkbank-model` (per-utterance
tier cleanliness flags with `taint()`, `is_clean()`, `can_align_main_to_mor()`
methods). It is used by the tree-sitter and direct parsers during parsing.
However, `batchalign-core` does **not** read, write, or propagate `ParseHealth`
during any mutation (morphosyntax injection, FA injection, retokenisation). The
infrastructure exists in the model layer but is not connected to the pipeline
layer.

---

## Cross-References

| Source | What It Contains |
|--------|-----------------|
| Grammar governance analysis (archived) | Proposed this document; leniency matrix concept; three-tier classification |
| Permissiveness regression log (archived) | 8 permissiveness regression decisions with rationale |
| Python-Rust boundary audit (archived) | Silent recovery points; ParseHealth gap; NLP pipeline audit |
| `grammar/grammar.js` | Inline comments on each leniency decision (line references in matrix above) |
| `talkbank-model/src/errors/config.rs` | `RuleSelection` API (and the cache key derived from it) |
| `talkbank-transform/src/presentation.rs` | `PresentationPolicy` and the `ConfigurableErrorSink` adapter |
| `talkbank-model/src/validation/header/structure.rs` | Header validation: E501, E502, E503, E504-E533 |
| `talkbank-model/src/validation/temporal.rs` | Temporal constraint checks (E701, E704), run for every file |
| `talkbank-model/src/model/content/main_tier.rs` | Main-tier validation path; carries no W210/W211 spacing pass |
