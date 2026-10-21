# Error Spec Format Reference

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/docs/ERROR_SPEC_FORMAT.md)

This document defines the exact format of `spec/errors/*.md` files. These files
are the **source of truth** for error code test cases. Generators in
`spec/tools/` read them to produce tree-sitter corpus tests, Rust tests, and
documentation.

## The format is a schema now, and the schema is the reference

An error spec's metadata is `+++` TOML frontmatter, deserialized into a typed
struct. The type owns the exact field schema; this page supplies authoring
context and routes workflow to the book.

**The reference is `talkbank_spec_vocabulary::frontmatter`**, whose
`SpecFrontmatter` and `ExampleFrontmatter` carry every field, its type, whether
it is required, and why. An unrecognised key is a load error, so the schema is
also the enforcement.

`code` and `name` belong to the spec; `level`, `claim` and `chat` belong to
each example. Code-level `kind` and implementation `status` live only in
`spec/codes/error-codes.toml`. There is no `layer` field. A registered code and
well-formed frontmatter do not prove that an example's behavioral claim holds.

A spec looks like this:

```markdown
+++
code = 'E256'
name = 'Illegal curly single quote'

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	don’t .
@End
'''
+++

## Description

What is illegal here, and why.

## Expected Behavior

## CHAT Rule

## Notes
```

**Declared data goes in the frontmatter; prose goes in the body.** The
`## Description` and `## CHAT Rule` sections are still read from the body,
because they are markdown that is republished as markdown on the code's
page. Everything a generator branches on is a declared field.

### File naming

```
spec/errors/E{NNN}_{suffix}.md
```

`NNN` is the three-digit code; the suffix is optional and can describe the case.
Historical `_auto` names do not confer reviewed provenance. A file whose stem is not code-shaped is not
a spec, which is what `talkbank_spec_vocabulary::spec_file_paths` decides.

### Three things worth knowing that the schema cannot tell you

- **Every example carries a required `claim`**: `violates`, `legal`, or
  `subsumed_by <code(s)>`, whose negative halves (absences) are enforced by
  the runner; see `ExampleFrontmatter::claim`. For example,
  `claim = { subsumed_by = 'E316' }` names an alternative diagnostic, but
  selecting that claim requires a reviewed rule, not merely matching output.
- **The newline before a `'''` block's closing delimiter is not part of the
  value**, exactly as a fenced code block's closing line was not part of it.
- A `legal` claim excludes this code, not every possible diagnostic. Deferred
  implementation does not waive legal/subsumption regression obligations.

# Construct Spec Format Reference

Construct specs in `spec/constructs/` define **valid** CHAT examples with their
expected CST (Concrete Syntax Tree). These drive tree-sitter corpus test
generation.

## File Location

```
spec/constructs/{category}/{name}.md
```

Categories: `header/`, `main_tier/`, `tiers/`, `utterance/`, `word/`

## Required Sections

### H1: Name

```markdown
# mor_basic_3
```

Used as the test name in the generated tree-sitter corpus file.

### Description (paragraph)

```markdown
Basic %mor tier with adjective and noun
```

Brief description (informational).

### Input

````markdown
## Input

```standalone_word
a:
```
````

The code fence info string specifies the **template** used to wrap the fragment
into a valid CHAT file for testing.

#### Templates

Templates live in `spec/tools/templates/`. Each wraps a fragment in the minimal
CHAT structure needed for tree-sitter to parse it:

| Template | Use for |
|----------|---------|
| `standalone_word` | Single word fragments |
| `utterance` | Full utterance, including its main-tier speaker prefix |
| `main_tier` | Full main tier line (`*CHI: ...`) |
| `mor_dependent_tier` | %mor tier content |
| `gra_dependent_tier` | %gra tier content |
| `pho_dependent_tier` | %pho tier content |
| `wor_dependent_tier` | %wor tier content |
| `com_dependent_tier` | %com tier content |
| `chat` | Full CHAT file (no wrapping needed) |
| `participants_header` | @Participants header line |
| `languages_header` | @Languages header line |
| `overlap_point` | Overlap point markers |

If the info string doesn't match any template, test generation fails.
This table is a selection, not the complete template inventory; inspect the
matching file in `spec/tools/templates/` for its exact scaffolding.

### Expected CST

````markdown
## Expected CST

```cst
(standalone_word
  ...)
```
````

Record the reviewed relevant subtree as an S-expression, without byte ranges
or field labels. The ellipsis above is an illustration, not a valid complete
expectation. Inspect a current construct such as
[`lengthening.md`](../constructs/word/lengthening.md) for a complete example.

The emitter parses the wrapped input to derive a full-document CST; it does
not automatically compare that result with the authored fragment's
`Expected CST`. Review their correspondence explicitly. A passing generated
test alone cannot prove that correspondence or full semantic validity.

### Metadata

```markdown
## Metadata

- **Level**: word
- **Category**: lengthening
```

Follow the corresponding maintained construct's level and directory category.
Construct markdown metadata is distinct from the typed error-example
frontmatter; do not copy this bullet syntax into an error spec.

## Workflow

Follow [Spec Workflow](../../book/src/contributing/spec-workflow.md) for
authoring, new-code bootstrap, regeneration and focused verification. Changes
to generated inputs use the root's complete regeneration workflow;
documentation-only corrections do not require regeneration.

---

# Tools Reference

The [Spec System](../../book/src/architecture/spec-system.md) includes the
artifact table generated from the owning registry. Use that table instead of
a second hand-maintained destination inventory.

- `just spec-gen` writes registered spec artifacts; `just spec-check` checks
  currency without rewriting them.
- `just spec-status` reports derived counts and dispositions.
- `just spec-coverage` reports which codes have specs and which examples
  demonstrate their own codes.
- `just spec-validate-examples` runs live example checks; inspect observations
  against the reviewed claims.
- `just spec-node-coverage` reports reference-corpus grammar-node reach,
  not complete semantic or recovery coverage.
- `just spec-corpus-candidates`, `just spec-perturb` and
  `just spec-mutation-candidates` produce discovery material with different
  admission guarantees. Follow the canonical workflow before promotion.
- `just symbols-gen` and `just form-markers-gen` regenerate their closed
  vocabulary consumers from the respective registries.

Use `just --list` for wired recipes. Maintained golden generators are separate
binaries under `crates/talkbank-parser-tests/src/bin/`; inspect the relevant
owner before running one. Do not infer a complete regeneration workflow from
a directory's binary count.

Registry implementation status is authored metadata, not an observation.
Live checks and snapshots establish finite example behavior under selected
rules; neither counts nor artifact currency prove complete implementation.
