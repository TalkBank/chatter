# Parser Leniency Policy

**Status:** Current
**Last updated:** {{git-dates:page}}

This chapter records selected leniency rulings and explains how grammar,
validation and presentation differ. The grammar, authored specifications and
owning implementations remain authoritative for their contracts; this prose is
not an exhaustive acceptance matrix.

> **Scope**: Documentation only. This document does not implement new validation
> rules or authorize stricter profiles. Retained rulings are not reopened merely
> because a documentation description needs correction.

---

## Philosophy: Parse, Don't Validate

The grammar recognizes structure, including some forms whose validity requires
later context. Tree-sitter can also recover from syntax errors. Recovery is not
a declaration that the input belongs to valid CHAT, and rejected syntax can
still produce diagnostics even when no model can be built.

Separate three questions:

1. What structure could the parser build, and what diagnostics did it emit?
2. Does that structure and its source provenance pass the selected validation?
3. Which already-produced diagnostics should a user interface display?

An editor may retain recovery content for inspection without admitting it for
transformation. A roundtrip alone does not certify validity. See
[parser/model contracts](parser-model-contracts.md) and
[validation architecture](errors-and-validation/validation.md).

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

The earlier table mixed current behavior, approximate line numbers and obsolete
MOR rules. Use these source-owned references instead of treating an optional
grammar position as permission to omit required information:

| Boundary | Owning reference |
| --- | --- |
| Required headers and contextual header rules | [Headers](../chat-format/headers.md) and authored error specifications |
| Terminators and CA exceptions | [CA options](../chat-format/options.md) |
| MOR lexical structure, features and required terminator | [MOR](../chat-format/mor-tier.md) |
| Nested content and tier-specific projections | [Dependent tiers](../chat-format/dependent-tiers.md) |
| Optional timing versus timing validity | [Bullet validation](bullet-validation.md) |
| Diagnostic selection, presentation and cache reuse | [Validation cache](parser-and-grammar/validation-cache.md) |

MOR is currently a main morphological word plus optional post-clitics, with
POS/lemma/features and a required tier terminator. It has no separate nested
translation or fusional-suffix grammar of the form previously listed here.
Opaque retained characters are not semantic support for a legacy dialect.

The A/B/C labels classify individual decisions, not whole constructs. For
example, a bullet may be optional in one position while any supplied bullet
still has syntax, interval and media-linkage obligations.

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
- **Rationale**: CA does not waive the temporal rules. Timing selection,
  tolerance, per-speaker comparisons and the separate E704 producers are
  described in [bullet validation](bullet-validation.md). `E362` remains
  applicable too. Do not infer a current population-wide acceptance result
  from this policy.
- **Revisit**: closed. No CA-specific temporal policy is needed: no policy
  difference between CA and non-CA files exists.

### Decision 7: Ordinary warnings do not block admission

- **Behaviour**: Ordinary warning diagnostics do not alone cause failure.
  This is not the converse claim that every failure requires an error-severity
  diagnostic: internal failures, rejected parsing and incomplete validation
  have explicit failure paths.
- **Implementation**: `talkbank-transform/src/pipeline/parse.rs`.
- **Rationale**: Warnings should not block parse/transform/export pipelines.
- **Boundary**: `CompletedDiagnostics` checks internal-failure evidence before
  severity filtering. Required validation preserves incomplete-parse refusal.
  Presentation severity remapping is not admission policy.

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

### Decision 10: `%wor` word intervals must have positive duration

- **Behaviour**: A `%wor` word bullet that ends before it starts
  (`300_100`) or has zero duration (`100_100`) is `E362`
  (`check_word_interval`). Untimed words remain legal partial timing.
  Word bullets may overlap or start out of order.
- **Relation to CHECK**: CLAN CHECK `21-Sep-2026` checks no `%wor` bullets:
  it accepts both cases above, and reports its error 82 ("BEG mark of bullet
  must be smaller than END mark") only for main-tier bullets. Measured with
  the `E362` spec examples renamed to match `@Media`. Chatter goes beyond
  CHECK for both reversed and zero-duration word intervals.
- **Ruling**: the maintainer explicitly required zero-duration `%wor` bullets
  to become invalid on 2026-10-09, superseding the October 6 temporary
  compatibility allowance. A bullet that covers no time locates nothing.
  An aligner that cannot time a word must omit its bullet, not fabricate an
  instant. Existing zero-duration data now receives E362; this change neither
  rewrites corpus files nor claims they have been regenerated.
- **Consumer boundary**: `assess_wor_timing_sequence` still requires a positive
  interval for every word and names rejected slots. Validation of partial CHAT
  and admission of a complete reusable timing sequence remain distinct.

---

## Validation Gap Roadmap

This chapter does not allocate new error codes or authorize a stricter policy.
Required-header checks already exist; broad grammar acceptance is not by itself
a missing validator. A new gap needs a declared requirement, an authored
counterexample and legal controls, with parser-versus-validator ownership
adjudicated before implementation. See [Spec Workflow](../contributing/spec-workflow.md).

---

## Validation Profile Infrastructure

### What Exists

#### Two kinds of setting, two types, two crates

What the validator COMPUTES and what a reader SEES are different questions, and
conflating them is not a style matter: it decides what a cached verdict means.
They are separate types, and deliberately not in the same crate.

#### `RuleSelection` (`talkbank-model/src/errors/config.rs`)

Which optional rules run. This contributes the rule-selection fragment of cache
identity; it is not the entire key. Parser identity, source content, resolved
path and alignment coverage also matter. See the cache chapter above.

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

#### Command options and proposed profiles

Use the [validation guide](../chatter/user-guide/validation-errors.md) and the
command's help for its supported options. Do not infer a CLI profile selector
from the existence of library presentation presets. `strict()` changes displayed
severity; it does not turn on every optional validator or repair missing evidence.

Earlier suggestions for `reference-compatible`, `strict-chat`, `lint`,
serialized profiles and corpus-specific rules were proposals, not implemented
contracts or standing permission to tighten validation. A future policy needs an
explicit semantic design, source-owned specs and compatible cache identity.
Retired codes remain retired.

## Downstream Recovery Is a Separate Contract

Applications that generate or replace tiers must carry their own admission,
mutation and output guarantees. Chatter's parse-health metadata alone does not
prove that a downstream transformation preserved provenance or refused invalid
input. Inspect the exact application's producer and consumer boundaries.

An earlier downstream audit is not a current implementation inventory. This
chapter does not claim present behavior for Stanza, forced alignment, translation
or segmentation, and does not infer downstream wiring from old method names.
The [library admission guide](../chatter/integrating/library-usage.md) describes
Chatter's source-preservation and tier-replacement boundaries.

---

## Cross-References

| Source | What It Contains |
|--------|-----------------|
| Grammar governance analysis (archived) | Proposed this document; leniency matrix concept; three-tier classification |
| Permissiveness regression log (archived) | Historical rulings; the decisions above retain their specific scope |
| Python-Rust boundary audit (archived) | Historical downstream observations, not current runtime acceptance |
| `grammar/grammar.js` | Inline comments on each leniency decision (follow named rules rather than old line numbers) |
| `talkbank-model/src/errors/config.rs` | `RuleSelection` API (and the cache key derived from it) |
| `talkbank-transform/src/presentation.rs` | `PresentationPolicy` and the `ConfigurableErrorSink` adapter |
| `talkbank-model/src/validation/header/structure.rs` | Header validation: E501, E502, E503, E504-E533 |
| `talkbank-model/src/validation/temporal.rs` | Temporal constraint checks (E701, E704), run for every file |
| `talkbank-model/src/model/content/main_tier.rs` | Main-tier validation path; carries no W210/W211 spacing pass |
