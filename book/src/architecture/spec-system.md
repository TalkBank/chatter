# Spec System

**Status:** Current
**Last modified:** 2026-08-27 13:44 EDT

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

Invalid CHAT, and the codes it must produce. Everything declared lives in
`+++` TOML frontmatter; everything published as prose lives in the body.

````markdown
+++
code = 'E207'
name = 'Unknown scoped annotation marker'

[[example]]
source = 'E2xx_word_errors/E207_multiple_form_types.cha'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	word@zz .
@End
'''
+++

## Description

Unknown scoped annotation marker.
````

## What every field actually does

The fields are not decoration. Each one changes what is checked. The
authoritative list, with types, is `talkbank_spec_vocabulary::frontmatter`,
which refuses an unrecognised key at load; this table says what each field
DOES, which a type cannot.

| Field | Effect |
|-------|--------|
| `code` | The code the spec DOCUMENTS; names the generated tests, and is resolved against `spec/codes/error-codes.toml` at load, so a spec naming an unregistered code does not load. |
| `name` | THIS FILE's short name, published as the page's title. Per file, not per code: `E241`'s two specs and `E519`'s three all differ, legitimately. |
| `status_note` | A human's adjudication of the code's current state. Prose, published nowhere, read by people. |
| `example.chat` | The input itself, a whole CHAT file. Required: an example without one is not an example. |
| `example.source` | The fixture the example came from. **Its stem NAMES the transcript**, see below. |
| `example.title`, `example.notes` | Prose about this example, read by people. |
| `example.claim` | What the example asserts: `violates`, `legal`, or `subsumed_by <code(s)>`. REQUIRED, and both halves are enforced (absences included), see below. |
| `example.level` | Where THIS example's fault is (`word`, `tier`, `utterance`, `header`, `file`). Required per example: a code like E519 is violated at header level in one example and at utterance level in another, so the fault site is a fact about the example, not the code. The page's Level line renders the distinct set. |

Two prose sections are published as well as read by humans:

| Section | Effect |
|---------|--------|
| `## Description` | Published verbatim, markdown and paragraph breaks intact, as the page's description. Required. |
| `## CHAT Rule` | Published verbatim as the page's `## CHAT Rule` section: what CHAT requires, and therefore what a maintainer must write instead. Optional; a spec without one publishes no such section. |
| `## Expected Behavior`, `## Notes` | Prose for whoever opens the spec file. Read by no tool. |

Write the RULE in `## CHAT Rule`, not a bare manual link. The pages exist so a
data maintainer can fix a file without reading the validator's source.

### `kind` and `status` are facts about a CODE, and live in the registry

Until R1 (2026-08-26) every spec declared `kind` and `status`. Both are
properties of the CODE, so each of a code's spec files carried a copy, and
eleven codes have two or three files. Nothing made them agree; a generator
checked, and refused to run on disagreement.

They live in [`spec/codes/error-codes.toml`](#the-code-registry) now, one entry
per code, and a spec reaches them through the code it names. Three things went
with the move: the `spec_status` gate that reconciled `#[status(planned)]` on
the enum against the specs in both directions, the `spec/errors <-> ErrorCode`
divergence check the `DiagnosticKind` generator ran, and the per-code `kind`
agreement loop beside it.

## The code registry

`spec/codes/error-codes.toml` is the source of truth for everything true of a
CODE, as opposed to true of a document about one:

```toml
[[code]]
code    = 'E202'
variant = 'MissingFormType'   # the ErrorCode variant it compiles to
summary = 'Missing form type on special word.'   # the variant's rustdoc
kind    = 'Invalidity'
status  = 'implemented'

[[retired]]
code   = 'W601'
reason = 'renumbered to E756 on 2026-07-16; the warning prefix was the bug'
```

`crates/talkbank-model/src/errors/codes/generated_error_code.rs` (the
`ErrorCode` enum) and `generated_diagnostic_kind.rs` are both GENERATED from
it, and both are under the currency gate, so the enum cannot disagree with the
specs about which checks run.

The schema, with a reason per field, is
`talkbank_spec_vocabulary::registry`. It refuses, at load: an unrecognised key,
a code registered twice, two codes compiling to one Rust identifier, and a
RETIRED number brought back. That last one was a twenty-line comment in the
enum asking readers not to reuse `W210`, `W601`, `E754` and eight others; it
is a load error now, and it names the retirement's own recorded reason.

What the registry deliberately does NOT own is whether a code is DOCUMENTED.
That used to be entangled with the vocabulary question, since "this variant has
no spec file" read as a divergence. It is a coverage question, and
`error_code_specs` asks it as one.

### There is no longer a rule about where a field sits

This section used to say `Expected Error Codes` **must precede the fence**,
because the loader read the content before the ```` ```chat ```` block and a
spec that put the line below it declared nothing while reading, to a human, as
fully specified. Two of E757's examples did exactly that, and the loader grew a
guard that refused the placement.

Phase 1b deleted the rule and the guard together: an example is one value that
carries its own input, so there is no fence for a field to be on the wrong side
of. It is recorded here because it is the clearest example in this system of a
type removing a rule rather than a document restating one.

### Every example carries a CLAIM, and absences are assertable

Since R2 (2026-08-21) each example declares one of:

```toml
claim = 'violates'                         # the spec's code MUST appear
claim = 'legal'                            # the spec's code MUST NOT appear
claim = { subsumed_by = 'E316' }           # E316 appears; this code does not
claim = { subsumed_by = ['E246', 'E249'] } # all listed appear; this code does not
```

The claim is REQUIRED: an example that asserts nothing is unwritable, which
retired the self-demonstration gate and its 36-entry baseline outright, plus
the zero-ratchet test whose own docstring had named exactly this retirement
("nothing in a type stops the next spec omitting it").

Extra emitted codes are still fine (one malformed line legitimately raises
several diagnostics); the exact per-stage sets are the observation snapshot's
business. What changed is the NEGATIVE half: `legal` and the own-code-absent
part of `subsumed_by` are assertions the old subset check could not express at
all, and this page used to say so ("a spec cannot be used to assert that a
code is NOT emitted"). A spec whose examples are all `subsumed_by` is the
parser-specificity worklist, verifiable against the snapshot rather than
merely recorded; `coverage --errors` lists it.

### There is no `layer` field, and the runner is total

Until R4 (2026-08-21) every spec declared `layer = 'parser' | 'validation'`,
and the field decided what a generated test could SEE: a parser-layer spec got
a string-based test inspecting parse diagnostics only, a validation-layer spec
got a fixture. Declaring a validation-layer code in a parser-layer spec
therefore produced a test that could never see it, which the E342/E390 case
demonstrated in production.

R4 deleted the field and the failure mode together, in three moves:

- **every example is a fixture**, and the fixture runner has always collected
  BOTH stages' codes against a real file, so there is no stage a declared code
  can hide in (five examples' codes are genuinely SPLIT across stages, which
  no per-stage harness could assert);
- **the string-based error tests are gone**, being strictly weaker than the
  fixture runner plus the observation snapshot;
- **which stage catches a rule is an OBSERVATION**, recorded per example in
  `spec/observations/example-diagnostics.json`. The authored field disagreed
  with the observation on 17 examples on the day it was measured.

Tree-sitter corpus membership, which the field used to route, is derived from
the snapshot instead: an example joins iff it produced parse-stage
diagnostics, so there is structure to pin.

### `status` decides whether an example is checked at all

| Value | Effect |
|-------|--------|
| `implemented` | Examples are verified. |
| `not_implemented` | Examples are DEFERRED, not checked, and generated tests carry `#[ignore]`. |
| `deprecated`, `unreachable_from_chat` | Deferred, same as above. |
| **absent** | REFUSED: `spec/codes/error-codes.toml` fails to load, naming the entry. |

Declared per CODE, in the registry, since R1. It was a per-FILE field, and
before 2026-08-11 it defaulted to `implemented` when absent, so the file said
nothing and the loader invented an answer: on that date it was true of **104 of
238 specs**. The default went first and the duplication went second, so
`implemented` now means one person decided it once for the code, rather than
each of its spec files claiming it separately.

Changing a spec from `not_implemented` to `implemented` un-`#[ignore]`s its
generated tests, and those tests may never have run. Regenerate and run them in
the same change.

### `source` names the transcript

Some CHAT rules are about the file's own name: E531 requires the `@Media`
header's filename to match the transcript's stem. The example runner therefore
names each transcript after the stem of its `source`, and an example with no
`source` is anonymous, so those rules do not run for it.

This field was parsed by nothing until 2026-08-11, which is why E531's spec
could not be verified and was reported as failing rather than as untestable.

## The observation snapshot

`spec/observations/example-diagnostics.json` (generated, gated) records, for
every example of every spec, the exact diagnostic codes the current binary
produces, split by the stage (parse or validation) that emitted them. It
covers every spec regardless of `status`, because an observation is not an
assertion: for an unimplemented rule the honest record is "nothing fires".

It is the regression instrument for the spec suite: **a diff in this file
is a review event**, and every changed entry is adjudicated INTENDED (the
behaviour change was the point; commit the regenerated snapshot in the same
change) or UNINTENDED (a regression; fix the code, never the snapshot). It is
also what makes a `subsumed by` claim verifiable and what the layer-of-capture
question is answered from, observed rather than authored.

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
