# Spec System

**Status:** Current
**Last modified:** 2026-08-16 12:39 EDT

`spec/` is the source of truth for what CHAT is and for what chatter rejects.
Tests, fixtures and error documentation are GENERATED from it. You change the
spec; you do not hand-edit what it produces.

This chapter is the reference: what the spec files contain, what each field
does, and what checks them. To make a change, follow
[Spec Workflow](../contributing/spec-workflow.md).

## Start here: ask the system

Before reading further, run:

```bash
just spec-status
```

It reports, derived from the same code the gates use rather than from prose:
how many specs exist and what status they declare, how many examples are
verified, how many are deferred, **how many assert nothing at all**, the state
of CLAN CHECK parity, and which gate checks which artifact. If this page and
that command ever disagree, the command is right.

## The two kinds of spec

### Construct specs, `spec/constructs/`

A valid CHAT fragment and the tree it must parse to.

````markdown
# languages_single

@Languages header with single language code

## Input

```languages_header
@Languages:	eng
```

## Expected CST

```cst
(languages_header
  (languages_prefix)
  ...
)
```

## Metadata

- **Level**: header
- **Category**: header
````

The `Input` fence label (`languages_header`, `main_tier`, `utterance`,
`standalone_word`, ...) names a **template** in `spec/tools/templates/` that
wraps the fragment in a complete CHAT file, because tree-sitter parses
documents rather than fragments. A label with no matching `.tera` template is
an error; add the template.

### Error specs, `spec/errors/`

Invalid CHAT, and the codes it must produce.

````markdown
# E207: Unknown scoped annotation marker

## Description

Unknown scoped annotation marker.

## Metadata

- **Error Code**: E207
- **Category**: Word validation
- **Level**: word
- **Layer**: parser
- **Kind**: Invalidity
- **Status**: implemented

## Example 1

**Source**: `E2xx_word_errors/E207_multiple_form_types.cha`
**Trigger**: Scoped annotation with unrecognized marker
**Expected Error Codes**: E207

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	word@zz .
@End
```
````

## What every field actually does

The fields are not decoration. Each one changes what is checked.

| Field | Effect |
|-------|--------|
| `**Error Code**` | The code the spec is about; names the generated tests. |
| `**Layer**` | `parser` or `validation`. **Decides what a generated test can SEE**, see below. |
| `**Kind**` | The `DiagnosticKind` axis. Required; a spec without it fails to load. |
| `**Status**` | Whether examples are verified or deferred, see below. |
| `**Category**`, `**Level**` | Documentation and grouping only. |
| `**Source**` | The fixture the example came from. **Its stem NAMES the transcript**, see below. |
| `**Trigger**` | Prose. Not read by any tool. |
| `**Expected Error Codes**` | The codes the example must emit. **The only field that makes an example assert anything**, and it must appear BEFORE the code fence. |

### `Expected Error Codes` must precede the fence

The loader reads it from the content BEFORE the ```chat block. A spec that puts
the line after the fence declares nothing while reading, to a human, as fully
specified; two of E757's examples did exactly that. The loader now REFUSES that
placement rather than silently accepting an example that cannot fail.

### `Expected Error Codes` is a SUBSET check

An example passes when every code it declares was emitted. Emitting **extra**
codes is fine, because one malformed line legitimately raises several
diagnostics.

Two consequences worth stating plainly:

- Declaring fewer codes is always safe, so a spec cannot be used to assert that
  a code is NOT emitted.
- **An example declaring no `Expected Error Codes` can never fail** in this
  runner. It is parsed and nothing more. `just spec-status` counts them, and a
  test holds the count at ZERO, so a new one fails CI.

  This matters more than it looks, because the validation-corpus builder falls back
  to the spec's TITLE code when an example declares none. An undeclared example
  was therefore asserted by the corpus runner and ignored by this one: the same
  question with two answers. On 2026-08-11 all 22 undeclared examples were given
  the code they were MEASURED to emit, which in every case was the title code
  the corpus generator had been assuming, so the two runners now agree by
  construction.

### `Layer` decides what a generated test can see

A `parser`-layer test inspects PARSE diagnostics only; a `validation`-layer
test runs validation as well. So declaring a validation-layer code in a
parser-layer spec produces a test that can never see it.

This is not hypothetical: `E342_auto.md`'s first example declared E390
(`ReplacementContainsOmission`, a validation-layer code) in a parser-layer
spec. The input genuinely raises both E342 and E390 in production, but the
generated test only ever sees E342, and the mismatch sat undetected because
the spec was ALSO marked `not_implemented`, so the test was `#[ignore]`d.

### `Status` decides whether an example is checked at all

| Value | Effect |
|-------|--------|
| `implemented` | Examples are verified. |
| `not_implemented` | Examples are DEFERRED, not checked, and generated tests carry `#[ignore]`. |
| `deprecated`, `unreachable_from_chat` | Deferred, same as above. |
| **absent** | REFUSED: the spec fails to load, naming the file. |

`Status` used to default to `implemented` when the bullet was missing, so the
file said nothing and the loader invented an answer. On 2026-08-11 that was
true of **104 of 238 specs**. All of them now declare it explicitly and the
default is gone, so `implemented` in a spec file means somebody decided it.

Changing a spec from `not_implemented` to `implemented` un-`#[ignore]`s its
generated tests, and those tests may never have run. Regenerate and run them in
the same change.

### `Source` names the transcript

Some CHAT rules are about the file's own name: E531 requires the `@Media`
header's filename to match the transcript's stem. The example runner therefore
names each transcript after the stem of its `**Source**` path, and an example
with no `**Source**` is anonymous, so those rules do not run for it.

This field was parsed by nothing until 2026-08-11, which is why E531's spec
could not be verified and was reported as failing rather than as untestable.

## What is generated, and by what

One command regenerates everything committed: `just spec-gen`. Its registry
(`spec/tools/src/artifacts.rs`, plus the half in `spec/runtime-tools` that needs
the live `ErrorCode` enum) is the only place a destination is written down, and
the same list drives writing, checking and the gate.

{{#include generated/spec-artifacts.md}}

That table is itself generated from the registry, and the currency gate keeps
it true. The hand-written one it replaced listed five generators when the tree
held eighteen binaries, and named four separate commands that had by then become
one.

One generator sits outside it, deliberately: `gen_form_markers` has its own
registry and its own drift gate (`just form-markers-gen`).

`docs/errors/*.md` used to be described here as "an optional local reference
nothing commits". That was false when written: the directory has been tracked
since 2026-06-23, 226 files of it. It is now a registry artifact like any other,
so `just spec-gen` writes it and `just spec-check` compares it, and the
standalone `gen_error_docs` binary that wrote it outside the gate is deleted.

Two registries under `spec/` own closed vocabularies and generate every site
that names them: `spec/symbols/symbol_registry.json` (`just symbols-gen`) and
`spec/form_markers/form_marker_registry.json` (`just form-markers-gen`). Each
has its own README and its own drift gate.

**Generated and hand-written tests live in separate trees.**
`grammar/test/corpus/generated/` is wiped in full on every run and refuses to
clear a directory lacking its `.generated-output-dir` marker;
`grammar/test/corpus/manual/` is never written by a generator. Both were once
one tree, which destroyed 1,468 lines of hand-mined corpus tests twice in three
days.

## What checks what

| Gate | Checks | Needs |
|---|---|---|
| `every_generated_artifact_is_current` | every committed generated artifact against what the specs produce now | |
| `error_spec_codes` | every example emits the codes it declares | |
| `manifest_agrees_with_clan_reference` | parity manifest against `check.cpp` | |
| `generated_form_marker_sites_are_current` | form-marker outputs against the registry | |
| `generated_symbol_sets_are_current` | symbol-set outputs against the registry | `node` |
| `clan_check_grounding` | fixtures against the REAL CLAN binary | CLAN, `CHATTER_CLAN_RUN` |

The first four run in CI under
`cargo test --manifest-path spec/Cargo.toml --workspace`. `clan_check_grounding`
is `#[ignore]`d and catches UPSTREAM drift; `refresh-unix-clan.sh` runs it after
a successful CLAN sync, which is the moment it matters.

## CLAN CHECK parity

CHECK is a decades-old approximation and a QUESTION LIST, never a
specification. For each of its error codes the question is whether the
construct it rejects actually fails to make sense, answered against `spec/`,
the grammar and real corpus data.

Every code carries a verdict in
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`:

- **parity**, chatter rejects it too;
- **divergence**, chatter deliberately accepts it, with the reason recorded;
- **no_obligation**, CLAN cannot emit it (commented out, no emission path,
  unreachable in file mode, or GUI-only), with the reason as a typed value.

`just spec-status` prints the current counts. CHECK's silence is not authority:
when upstream retired error 76 in the 2026-08-07 bundle, chatter KEPT its rule,
because the changelog showed enforcement being abandoned rather than a
linguistic question being decided.

## Related

- [Why the Spec System Looks Like That](spec-system-history.md), which answers
  the questions this page raises and does not settle: what `_auto` means, why an
  E202 spec can carry an example expecting E316, and why eleven codes have two
  spec files. Read it before concluding that a spec file means what it appears
  to mean.
- [Spec Workflow](../contributing/spec-workflow.md), how to make a change.
- [Testing](../contributing/testing.md), the wider test strategy.
- [Grammar Governance](grammar-governance.md), the grammar side.
