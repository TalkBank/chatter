# spec, CHAT Specification

**Status:** Current
**Last modified:** 2026-08-21 13:12 EDT

## Read the book first

The CONTRIBUTOR-facing documentation is authoritative and complete:

- [`book/src/architecture/spec-system.md`](../book/src/architecture/spec-system.md),
  what every field does, what is generated, and what checks what.
- [`book/src/contributing/spec-workflow.md`](../book/src/contributing/spec-workflow.md),
  the procedure, with every command written out.

Start any spec question with `just spec-status`, which derives its answers from
the same code the gates use. This file carries only what is specific to working
here as an agent; where it and the book overlap, the book wins.

## How This Works

Specs are the **single source of truth** for all CHAT grammar tests, parser
tests, and error documentation. You never hand-edit generated test files.

```
spec/constructs/*.md  ─┐
                      ├──► spec/tools generators ──► grammar/test/corpus/generated/*.txt (membership from the snapshot)
spec/errors/*.md      ─┤                            ──► crates/.../integration/generated/*.rs (construct tests only)
                      │                            ──► crates/.../error_corpus/validation_errors/ + manifest.json (EVERY example, claim-judged)
                      │                            ──► docs/errors/*.md
spec/tools/templates/ ─┘
spec/observations/    ◄── spec-runtime-tools (regenerated FIRST; what each example emits, by stage)
```

Regenerate with `just spec-gen`, and ask `just spec-check` whether the
committed artifacts are current. One command covers every artifact; the
destinations are constants in the registry (`spec/tools/src/artifacts.rs`), not
arguments, so a generator cannot be aimed at the wrong tree.

### Generated and hand-maintained tests live in SEPARATE trees

```
grammar/test/corpus/
├── generated/   owned by the tree-sitter corpus artifact; DELETED IN FULL every run
└── manual/      hand-maintained; no generator writes here
```

`tree-sitter test` recurses, and test names come from each file's `====`
header rather than its path, so the split costs nothing and renames nothing.

This is organization, not a rule to remember. The generator wipes its own
directory wholesale, which is safe precisely because nothing else is in it,
and it **refuses to clear any directory lacking its `.generated-output-dir`
marker**. Pointing `--output-dir` at a shared or hand-maintained tree fails
loudly instead of deleting work. The marker file says the same thing in situ,
so a reader who finds the directory does not need this page.

Until 2026-07-29 both kinds of file shared one tree and were indistinguishable
by content (both are `====`-headed corpus files), so "delete stale output"
could only be implemented as "delete everything". That destroyed
`manual/word_markers/marker_density.txt`, 1,468 lines mined from wild corpus
data, twice in three days; both times it was caught only because a human read
the diff. **Never hand-edit anything under `generated/`**, and put new
hand-maintained tests in `manual/`.

## Spec Locations

| Location | Purpose |
|----------|---------|
| `spec/constructs/` | Valid CHAT examples with expected CSTs |
| `spec/errors/` | Invalid (or boundary-legal) CHAT examples, each with a CLAIM |
| → `grammar/test/corpus/generated/` | Generated tree-sitter tests (wiped each run) |
| `grammar/test/corpus/manual/` | Hand-maintained tree-sitter tests (never generated) |
| → `crates/talkbank-parser-tests/tests/integration/generated/` | Generated Rust parser tests |
| → `crates/talkbank-parser-tests/tests/error_corpus/validation_errors/` | Validation fixtures + `manifest.json` (data-driven runner) |
| → `docs/errors/` | Published error-reference pages; a registry artifact, gated |
| → `spec/observations/` | Generated observation snapshot: what each example actually emits, by stage; a diff is adjudicated like a corpus differential |

## Adding a Test

### 1. Create a spec file

Put it in the right directory under `spec/constructs/` or `spec/errors/`:

```
spec/constructs/
├── header/      # @-header examples
├── main_tier/   # *SPK: line examples
├── tiers/       # %mor, %gra, %sin, %wor etc.
├── utterance/   # Full utterance (main + dependent tiers)
└── word/        # Word-internal structure
```

### 2. Spec format (constructs)

```markdown
# example_name

Description of what this tests.

## Input

```input_type
*CHI:	hello .
```

## Expected CST

```cst
(main_tier ...)
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
```

The `input_type` in the code fence (e.g., `main_tier`, `standalone_word`,
`utterance`) tells the generator which **template** to use for wrapping the
fragment in a full CHAT document. Templates live in `spec/tools/templates/`.

### 3. Spec format (errors)

Declared data in `+++` TOML frontmatter, prose in the body:

````markdown
+++
code = 'E999'
name = 'Description of the condition'
kind = 'Invalidity'     # Invalidity | Unmodeled | Deprecation | Style
status = 'implemented'  # implemented | not_implemented | deprecated | unreachable_from_chat

[[example]]
level = 'word'          # word | utterance | tier | header | file; a fact about THIS example
claim = 'violates'
chat = '''
@UTF8
@Begin
...invalid content...
@End
'''
+++

## Description

Why this is not valid CHAT.
````

**The schema is the reference**, not this snippet:
`talkbank_spec_vocabulary::frontmatter` carries every field with its type and
its reason, and refuses an unrecognised key at load. `spec/docs/ERROR_SPEC_FORMAT.md`
has the taxonomy and the handful of rules a type cannot state.

### 4. Check templates

The `input_type` must match a `.tera` template in `spec/tools/templates/`.
If no template exists for your fragment type, create one. Templates wrap the
fragment in valid CHAT scaffolding so `tree-sitter test` can parse it.

Example (`spec/tools/templates/main_tier.tera`):
```
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
{{ input }}
@End
```

### 5. Regenerate and verify

```bash
just spec-gen        # regenerate every artifact from the specs
just spec-check      # or ask whether the committed copies are current

cd grammar && tree-sitter test
cargo build --workspace --all-targets --locked
cargo test --workspace
```

## Key Commands

```bash
# Regenerate every artifact derived from spec/ (corpus tests, Rust test bodies,
# validation fixtures + manifest, the DiagnosticKind registry)
just spec-gen

# Report staleness without writing anything. This is what the gate runs.
just spec-check

# (docs/errors/ is a spec-gen artifact; no separate command.)

# Do the error specs' examples produce the codes they declare?
just spec-validate-examples
```

## The tooling, and why this file does not list it

`just --list` names every wired spec command with a one-line summary of the
question it answers; `ls spec/*/src/bin` shows everything that exists. **This
file used to carry its own table of binaries**, and it was one of FIVE
hand-written copies across the tree that gave five different answers, one of
them naming tools deleted months earlier. `spec/docs/ERROR_SPEC_FORMAT.md`
carries the taxonomy (registry generators, artifact driver, reporters, corpus
tooling, golden generators) that is worth writing down because it is not
derivable; membership is derivable and so is not written down.

The two that matter most here, because they are what you run:

- `just spec-gen` / `just spec-check`: regenerate, or check, EVERY artifact
  derived from `spec/`, from one registry that owns each destination.
- `just spec-status`: what state the spec system is in, derived from the same
  code the gates use.

## Cross-Spec Consistency

Error spec examples can be cross-referenced, the same `.cha` content may
appear in multiple specs with different claims. When changing a
grammar rule so that previously-unparsable content now parses:

1. Regenerate: the observation snapshot records the new stage per example, and
   corpus membership follows it (there is no authored `layer` to flip since R4)
2. Audit `E316_auto.md`: remove examples that no longer produce E316
3. Run `just spec-gen` and review the diff
4. Run the concrete verification commands from `book/src/contributing/dev-checks.md`

## See Also
- `spec/tools/CLAUDE.md`: generator implementation details
- `grammar/CLAUDE.md`: grammar change workflow
- `book/src/contributing/testing.md`: testing strategy
