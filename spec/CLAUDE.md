# spec, CHAT Specification

**Status:** Current
**Last modified:** 2026-08-11 20:40 EDT

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
                      ├──► spec/tools generators ──► grammar/test/corpus/generated/*.txt
spec/errors/*.md      ─┤                            ──► crates/talkbank-parser-tests/tests/integration/generated/*.rs (parser tests)
                      │                            ──► crates/talkbank-parser-tests/tests/error_corpus/validation_errors/ + manifest.json (validation)
                      │                            ──► docs/errors/*.md
spec/tools/templates/ ─┘
```

This repo does **not** currently have the predecessor workspace's root
`make test-gen` wrapper. Run the relevant `spec/tools` binaries directly.

### Generated and hand-maintained tests live in SEPARATE trees

```
grammar/test/corpus/
├── generated/   owned by gen_tree_sitter_tests; DELETED IN FULL every run
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
| `spec/errors/` | Invalid CHAT examples with expected error codes |
| → `grammar/test/corpus/generated/` | Generated tree-sitter tests (wiped each run) |
| `grammar/test/corpus/manual/` | Hand-maintained tree-sitter tests (never generated) |
| → `crates/talkbank-parser-tests/tests/integration/generated/` | Generated Rust parser tests |
| → `crates/talkbank-parser-tests/tests/error_corpus/validation_errors/` | Validation fixtures + `manifest.json` (data-driven runner) |
| → `docs/errors/` | Optional locally generated error-reference pages |

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

```markdown
# E999, Description

Error for some condition.

- **Code**: E999
- **Severity**: Error
- **Layer**: parser | validation
- **Status**: implemented | not_implemented

## Example

```chat
@UTF8
@Begin
...invalid content...
@End
```

## Expected Error Codes

- E999
```

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
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_tree_sitter_tests -- \
  --output-dir grammar/test/corpus/generated \
  --template-dir spec/tools/templates

cargo run --manifest-path spec/tools/Cargo.toml --bin gen_rust_tests -- \
  --output-dir crates/talkbank-parser-tests/tests/integration/generated

cargo run --manifest-path spec/tools/Cargo.toml --bin gen_validation_corpus -- \
  --corpus-dir crates/talkbank-parser-tests/tests/error_corpus/validation_errors

cd grammar && tree-sitter test
cargo build --workspace --all-targets --locked
cargo test --workspace
```

## Key Commands

```bash
# Regenerate grammar corpus tests
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_tree_sitter_tests -- \
  --output-dir grammar/test/corpus/generated \
  --template-dir spec/tools/templates

# Regenerate generated Rust tests
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_rust_tests -- \
  --output-dir crates/talkbank-parser-tests/tests/integration/generated

cargo run --manifest-path spec/tools/Cargo.toml --bin gen_validation_corpus -- \
  --corpus-dir crates/talkbank-parser-tests/tests/error_corpus/validation_errors

# Regenerate local error docs
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_error_docs

# Verify spec format integrity
cargo run --manifest-path spec/runtime-tools/Cargo.toml --bin validate_error_specs
```

## Generator Binaries (`spec/tools/src/bin/`)

| Binary | What it generates |
|--------|-------------------|
| `gen_tree_sitter_tests` | `grammar/test/corpus/generated/*.txt` from constructs + errors |
| `gen_rust_tests` | `crates/talkbank-parser-tests/tests/integration/generated/*.rs` from constructs + errors |
| `gen_validation_corpus` | Validation fixture corpus + `manifest.json` from `spec/errors/` (one fixture per example, asserting that example's own Expected Error Codes) |
| `gen_error_docs` | `docs/errors/*.md` from errors |
| `gen_form_markers` | Every site carrying the CHAT form-marker inventory, from `spec/form_markers/form_marker_registry.json`: the model's `FormType` enum, the re2c lexer's code set, and the book's table. Run it as `just form-markers-gen`; see `spec/form_markers/README.md` for the two follow-ups it cannot do (the vendored re2c lexer and the JSON Schema). |
| `validate_spec` | Validates spec format integrity (no output) |
| `corpus_node_coverage` | Reports which grammar node types are exercised by `corpus/reference/` |
| `coverage` | Reports spec coverage and error-code coverage |
| `corpus_to_specs` | Migrates legacy `tests/error_corpus/` fixtures into spec format |

## Cross-Spec Consistency

Error spec examples can be cross-referenced, the same `.cha` content may
appear in multiple specs with different expected error codes. When changing a
grammar rule so that previously-unparsable content now parses:

1. Update the primary error spec: change `Layer: parser` → `Layer: validation`
2. Audit `E316_auto.md`: remove examples that no longer produce E316
3. Regenerate the affected outputs with the current `spec/tools` binaries
4. Run the concrete verification commands from `book/src/contributing/dev-checks.md`

## See Also
- `spec/tools/CLAUDE.md`: generator implementation details
- `grammar/CLAUDE.md`: grammar change workflow
- `book/src/contributing/testing.md`: testing strategy
