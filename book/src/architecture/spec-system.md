# Spec System

**Status:** Current
**Last modified:** {{git-dates:page}}

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
| `name` | The human-readable title used in generated error documentation. |
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

Both are properties of the CODE, not of a document about it, so a code with
several spec files has one `kind` and one `status`. They live in
[`spec/codes/error-codes.toml`](#the-code-registry), one entry per code, and a
spec reaches them through the code it names. Because there is one copy, there
is nothing to reconcile: the enum's status and the specs cannot disagree.

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
RETIRED number brought back; the load error names the retirement's own
recorded reason. Reusing a retired number such as `W210`, `W601` or `E754` is
therefore unrepresentable.

What the registry deliberately does NOT own is whether a code is DOCUMENTED.
That is a coverage question, and `error_code_specs` asks it as one: a variant
with no spec file is a coverage gap, not a vocabulary divergence.

### A field has no position

An example is one value that carries its own input and its own declared
fields, so there is no fence for a field to be on the wrong side of, and no
placement rule to remember. It is the clearest example in this system of a
type removing a rule rather than a document restating one.

### Every example carries a CLAIM, and absences are assertable

Each example declares one of:

```toml
claim = 'violates'                         # the spec's code MUST appear
claim = 'legal'                            # the spec's code MUST NOT appear
claim = { subsumed_by = 'E316' }           # E316 appears; this code does not
claim = { subsumed_by = ['E246', 'E249'] } # all listed appear; this code does not
```

The claim is REQUIRED: an example that asserts nothing is unwritable.

Extra emitted codes are fine (one malformed line legitimately raises several
diagnostics); the exact per-stage sets are the observation snapshot's
business. The NEGATIVE half is assertable: `legal` and the own-code-absent
part of `subsumed_by` assert that a code is NOT emitted. A spec whose examples
are all `subsumed_by` is the parser-specificity worklist, verifiable against
the snapshot; `coverage --errors` lists it.

Reading a spec: a `subsumed_by` claim tells you what chatter emits, not
necessarily what the rule is. Read the spec's Description and title for the
rule, and treat a mismatch between the title code and the example's codes as
an open question rather than as a specification. `subsumed_by E316`
("unparsable content") means the example does not parse, so the specific rule
is never reached: a parser gap. A specific other code is usually a wrong
fixture. When authoring, an example whose input violates the rule should
produce that rule's code; if it does not, chatter has a gap, and the gap is
the finding.

### There is no `layer` field, and the runner is total

Which stage catches a rule is not authored; it is an OBSERVATION, recorded per
example in `spec/observations/example-diagnostics.json`. Every example is a
fixture, and the fixture runner collects BOTH stages' codes against a real
file, so there is no stage a declared code can hide in (some examples' codes
are genuinely SPLIT across stages, which no per-stage harness could assert).
Error specs have no string-based per-stage tests; the fixture runner plus the
observation snapshot cover them.

Tree-sitter corpus membership is derived from the snapshot: an example joins
iff it produced parse-stage diagnostics, so there is structure to pin.

### `status` controls implementation checking, not every regression contract

| Value | Effect |
|-------|--------|
| `implemented` | Examples are verified. |
| `not_implemented` | Own-code implementation is DEFERRED and generated tests carry `#[ignore]`; explicit legal/subsumption claims remain checked separately. |
| `deprecated`, `unreachable_from_chat` | Implementation is deferred; legal/subsumption regression claims remain checked. |
| **absent** | REFUSED: `spec/codes/error-codes.toml` fails to load, naming the entry. |

Declared per CODE, in the registry, with no default: `implemented` means
someone decided it once for the code, rather than each of its spec files
claiming it separately.

Changing a spec from `not_implemented` to `implemented` un-`#[ignore]`s its
generated tests, and those tests may never have run. Regenerate and run them in
the same change.

Deferred code status is not a count of missing validation rules. Run
`cargo run --manifest-path spec/Cargo.toml --bin spec_status -- --deferred`
to see each authored claim beside its observed codes. This view distinguishes
verified legal/subsumption claims, contradicted claims and planned violation
claims using the claim's shared evaluator. The separate deferred-spec regression
test enforces legal/subsumption claims even when their code is not
implemented. A verified alternate diagnostic does not reactivate that code;
an observed emission of the deferred code instead requires status review.

### `source` names the transcript

Some CHAT rules are about the file's own name: E531 requires the `@Media`
header's filename to match the transcript's stem. The example runner therefore
names each transcript after the stem of its `source`, and an example with no
`source` is anonymous, so those rules do not run for it.

The backend-parity harness also preserves this context: its input owns both
CHAT text and the declared source path, and performs contextual validation for
either parser. A measurement regression checks mismatching, matching, and
anonymous source names through both backends.

A `legal` claim asserts absence of this spec's own code. It does not assert that
other rules accept the input or that parsing required no recovery. For example,
E758's malformed-content controls retain their content diagnostics while proving
there is no space directly after the tab. Serialization-equivalence assertions
must distinguish clean parsing from recovery.

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
it true.

One generator sits outside it, deliberately: `gen_form_markers` has its own
registry and its own drift gate (`just form-markers-gen`).

`docs/errors/*.md` is a tracked, committed registry artifact like any other:
`just spec-gen` writes it and `just spec-check` compares it.

Two registries under `spec/` own closed vocabularies and generate every site
that names them: `spec/symbols/symbol_registry.json` (`just symbols-gen`) and
`spec/form_markers/form_marker_registry.json` (`just form-markers-gen`). Each
has its own README and its own drift gate.

Shared-directory artifacts (`Ownership::NamedFiles`) retain byte-identical
outputs and delete only explicitly retired filenames. This preserves generated
Rust inputs across no-op regeneration while leaving other producers' files
alone. The generator reports the number of files actually written, not the
number it expected to produce. Whole-directory artifacts additionally require the ownership capability
described below before obsolete files may be pruned.

**Generated and hand-written tests live in separate trees.**
`grammar/test/corpus/generated/` retains unchanged files and removes obsolete
ones through `GeneratedDir`, which requires a `.generated-output-dir` marker
and refuses human ownership or symlinked entries;
`grammar/test/corpus/manual/` is never written by a generator, so a
regeneration can never destroy hand-mined corpus tests.

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

## CLAN CHECK assessment

See the generated [CHECK Assessment](errors-and-validation/check-parity-audit.md)
for the current inventory, adjudications, architectural mandate, completion
limits and rules for reopening an obligation. Its authored manifest is the
single authority; this chapter deliberately does not repeat the verdict scheme
or completion claims. `just spec-status` derives its assessment summary from
the same shared types and manifest. Neither report substitutes for a fresh
runtime observation of a specific CHECK executable.

## Related

- [Spec Workflow](../contributing/spec-workflow.md), how to make a change.
- [Testing](../contributing/testing.md), the wider test strategy.
- [Grammar Governance](grammar-governance.md), the grammar side.
