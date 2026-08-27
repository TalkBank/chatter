# Error Spec Format Reference

**Last updated:** 2026-08-27 18:09 EDT

This document defines the exact format of `spec/errors/*.md` files. These files
are the **source of truth** for error code test cases. Generators in
`spec/tools/` read them to produce tree-sitter corpus tests, Rust tests, and
documentation.

## The format is a schema now, and the schema is the reference

**As of Phase 1b (2026-08-21) an error spec's metadata is `+++` TOML
frontmatter, deserialized by serde into a typed struct.** This document no
longer describes it field by field, and that is deliberate: a prose description
of a format is a second statement of something the code already states exactly,
and this one had drifted. It said `**Level**` "determines which parse method the
test calls", which was false, and it listed `**Error Code**` as required while
every hand-written spec omitted it and loaded fine.

**The reference is `talkbank_spec_vocabulary::frontmatter`**, whose
`SpecFrontmatter` and `ExampleFrontmatter` carry every field, its type, whether
it is required, and why. An unrecognised key is a load error, so the schema is
also the enforcement.

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

`NNN` is the three-digit code and `suffix` is typically `auto` (seeded from
corpus data) or a descriptive name. A file whose stem is not code-shaped is not
a spec, which is what `talkbank_spec_vocabulary::spec_file_paths` decides.

### Three things worth knowing that the schema cannot tell you

- **Every example carries a required `claim`**: `violates`, `legal`, or
  `subsumed_by <code(s)>`, whose negative halves (absences) are enforced by
  the runner; see `ExampleFrontmatter::claim`.
- **The newline before a `'''` block's closing delimiter is not part of the
  value**, exactly as a fenced code block's closing line was not part of it.
- (`trigger` and `expected_error_codes` were Phase 1b carryovers; R2 deleted
  both, the first as pure residue and the second in favour of the claim.)

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
| `utterance` | Utterance content (words + annotations) |
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

### Expected CST

````markdown
## Expected CST

```cst
(standalone_word
  (word_body
    (initial_word_segment)
    (word_content
      (colon)
    )
  )
)
```
````

S-expression of the expected tree-sitter parse tree. Must match the output of
`tree-sitter parse` for the wrapped input. Whitespace and indentation are
normalized during comparison.

### Metadata

```markdown
## Metadata

- **Level**: word
- **Category**: lengthening
```

| Field | Required | Values |
|-------|----------|--------|
| **Level** | Yes | `word`, `tier`, `utterance`, `header`, `file` |
| **Category** | Yes | Free text, matches directory name |

## Workflow

1. Create or edit spec in `spec/constructs/{category}/`
2. Ensure a matching template exists in `spec/tools/templates/`
3. Regenerate the affected generated artifacts:

   ```bash
   just spec-gen      # every artifact derived from spec/
   just spec-check    # or: is the committed copy current?
   ```

4. `cd grammar && tree-sitter test`, verify grammar corpus tests
5. Run the concrete local verification commands from `book/src/contributing/dev-checks.md`

---

# Tools Reference

## Generators

`just spec-gen` regenerates every committed artifact from one registry, and
`just spec-check` reports staleness without writing:

| Artifact | Built from |
|--------|-------------------|
| tree-sitter corpus tests | construct specs + every error example the snapshot observed parse-stage diagnostics for |
| Rust test bodies | construct + error specs |
| validation fixture corpus + `manifest.json` | EVERY error example; the runner checks both stages |
| `DiagnosticKind` registry | each code's `kind`, from `spec/codes/error-codes.toml` |
| the `ErrorCode` enum | `spec/codes/error-codes.toml`: variant, rustdoc, code string, enforcement |
| published error documentation (`docs/errors/`) | every error spec |

## Tooling: what kinds exist, and how to see the real list

**This page used to mirror the binary list and the mirror rotted.** The "Corpus
Tools" table named seven binaries of which FOUR no longer exist (`bootstrap` and
`bootstrap_tiers`, removed 2026-03-22 with the mining machinery; `corpus_to_specs`
and `enhance_specs`, deleted by R5 because they wrote INTO the source of truth),
and it described `fix_spec_layers` as an auto-corrector when it was inert, refused
at the door by `spec/errors/.human-authored`. The golden-generator table held ten rows
against nineteen files in that directory, so it was neither a complete mirror
nor a useful subset. Four other files carried their
own copies of the same list, and no two agreed.

So this page states the PURPOSE and lets the list be looked up:

- **`just --list`** shows every spec command that is wired, each with a one-line
  summary of the question it answers. That is the list a contributor wants.
- **`ls spec/*/src/bin crates/talkbank-parser-tests/src/bin`** shows everything
  that exists, wired or not.

The kinds, which is the part worth writing down:

- **Registry generators** write every site that names a closed vocabulary, and
  each has its own drift gate: `just symbols-gen`, `just form-markers-gen`.
- **The artifact driver** is `just spec-gen` / `just spec-check`, one registry
  owning every generated destination. Its table in the book is itself generated.
- **Reporters** answer a question and write nothing: `just spec-status`,
  `just spec-coverage`, `just spec-node-coverage`, `just spec-validate-examples`.
- **Corpus tooling** finds or makes CHAT to specify against:
  `just spec-corpus-candidates`, `just spec-perturb` (the adversarial half, which
  is how CHECK gaps are found), `just spec-ca-census`.
- **Golden generators** live in `talkbank-parser-tests` and emit the committed
  `golden_*.txt` corpora, one per tier kind.

**The golden generators are the one place `just --list` does not help.**
`crates/talkbank-parser-tests/src/bin` holds nineteen binaries and NO recipes,
so for that half the pointer is true only vacuously and `ls` there mixes
goldens with audits and bootstraps. They want recipes, not a better pointer.

**And one question this page can no longer answer, deliberately.** "Is this
rule implemented in the validator?" had an answer here until 2026-08-20, and it
was fabricated: a coverage dashboard computing percentages from a hand-written
five-entry map. Deleting it is better than keeping a wrong number, but the gap
is real. `spec-coverage` answers which codes have specs and which specs
demonstrate their own code; `spec-status` answers example counts and parity.
Neither observes the validator. The honest successor is DERIVED, not declared:
run the validator over the fixture corpus and record which codes actually fire,
which is most of what `just spec-validate-examples` already does. The
hand-declared `status` was the same mirror one layer down, copied into every
spec file for a code; R1 gave it one owner in `spec/codes/error-codes.toml`,
which removes the DUPLICATION but not the declaration: it is still authored,
and still not an observation.

