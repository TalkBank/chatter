# Error Specifications

This directory contains formal markdown specifications for CHAT format validation errors.

## Purpose

Error specifications serve multiple purposes:
1. **Documentation**: Human-readable descriptions of what each error means
2. **Test Generation**: Automated generation of validation test fixtures
3. **Consistency**: Ensures error messages and behavior are well-defined
4. **Validation**: Machine-checkable specs that can be validated for completeness

## Directory Structure

```
spec/errors/
├── README.md                    # This file
├── E241_illegal_untranscribed_marker.md
├── E522_undefined_participant.md
├── E604_gra_without_mor.md
└── ... (other error specs)
```

The detailed spec-format documentation lives at
`spec/docs/ERROR_SPEC_FORMAT.md`; this directory holds the error specs
themselves.

## Spec Format

Each error specification is `+++` TOML frontmatter, then markdown prose.

A spec DOCUMENTS a code; it does not define one. Everything true of the code
itself (its `ErrorCode` variant, its rustdoc, its `kind`, and whether the
validator enforces it) lives in `spec/codes/error-codes.toml`, one entry per
code, and this file reaches it through `code`. Writing `kind` or `status` here
is a load error naming the key.

````markdown
+++
code = 'E###'          # must be registered in spec/codes/error-codes.toml
name = 'Error title'   # THIS FILE's name; several files may document one code

[[example]]
level = 'word'         # word | utterance | tier | header | file; per EXAMPLE
claim = 'violates'
chat = '''
... example CHAT that triggers the error ...
'''
+++

## Description

Brief description of the error.

## Expected Behavior

What should happen.

## CHAT Rule

What CHAT requires here, and therefore what a maintainer must write instead.

## Notes

Additional implementation notes.
````

**The frontmatter schema is the reference**
(`talkbank_spec_vocabulary::frontmatter`): it carries every field with its type
and whether it is required, and an unrecognised key is a load error rather than
a line that silently does nothing.

See [../docs/ERROR_SPEC_FORMAT.md](../docs/ERROR_SPEC_FORMAT.md) for complete format documentation.

## Workflow

### Creating a New Error Spec

#### Option 1: Manual Creation

1. Create a new markdown file named `E###_descriptive_name.md`
2. Follow the format in ERROR_SPEC_FORMAT.md
3. Validate the spec:
   ```bash
   cargo run --bin validate_error_specs --manifest-path spec/runtime-tools/Cargo.toml -- --spec-dir spec/errors
   ```

#### Option 2: there is no longer a route from the implementation

There used to be one: `corpus_to_specs` read a fixture, ran chatter on it, and
wrote down what came out; `fix_spec_layers` decided the layer field by running
the parser; `enhance_specs` filled in descriptions and manual links. Between them
they produced 152 files, and the effect was that the SPECIFICATION was derived
from the IMPLEMENTATION, which was then tested against the specification. Every
gate passed by construction, and none of them could tell a finished rule from a
gap.

Those tools are gone (`corpus_to_specs`, `enhance_specs`) or refused
(`fix_spec_layers`, deleted with R4). `spec/errors/` carries a
`.human-authored` marker and `WritableDir::claim` refuses to write into a
directory that has one, so this is mechanical rather than a request.

**Write the spec by hand.** A spec says what CHAT requires and why; that is a
decision about the format, and running the current parser cannot make it. If a
bootstrap tool is ever wanted again it writes to `spec/proposals/`, which a
person reads, completes and moves.

**Then validate the spec**
```bash
cargo run --bin validate_error_specs --manifest-path spec/runtime-tools/Cargo.toml -- --spec-dir spec/errors
```

### Generating the Validation Corpus

Once you have error specs, generate the validation fixture corpus:

```bash
just spec-gen      # every artifact derived from spec/
just spec-check    # or: is the committed copy current?
```

This generates:
- One `.cha` fixture per example (every spec, both stages)
- `manifest.json` (each fixture's spec code + claim + status + source spec),
  consumed by the data-driven runner `validation_error_corpus.rs`

### Implementing Validators

After generating tests:

1. Run tests to verify they fail (TDD red phase):
   ```bash
   cd ..
   cargo test -p talkbank-parser-tests validation_tests
   ```

2. Implement the validator in the appropriate module:
   - **E2xx** (word errors): `talkbank-model/src/validation/word/`
   - **E3xx** (main tier): `talkbank-model/src/validation/main_tier.rs`
   - **E4xx** (dependent tier): `talkbank-model/src/validation/utterance/tiers.rs`
   - **E5xx** (header): `talkbank-model/src/validation/header/`
   - **E6xx** (alignment): `talkbank-model/src/alignment/`

3. Implement validation following existing patterns:
   ```rust
   impl Validate for MyType {
       fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
           if bad_condition {
               errors.report(ParseError::new(
                   ErrorCode::MyError,
                   Severity::Error,
                   SourceLocation::new(self.span),
                   ErrorContext::new(&self.text, self.span, &self.text),
                   "Error message",
               ).with_suggestion("How to fix"));
           }
       }
   }
   ```

4. Run tests to verify they pass (TDD green phase):
   ```bash
   cargo test -p talkbank-parser-tests validation_tests
   ```

5. Verify no regressions on reference corpus:
   ```bash
   cargo test -p talkbank-parser-re2c --test integration equivalence_reference_corpus
   cargo test -p talkbank-parser-tests --test roundtrip_reference_corpus
   ```

## Tools

### validate_error_specs

Validates that error specs follow the correct format and have proper metadata.

```bash
cargo run --bin validate_error_specs --manifest-path spec/runtime-tools/Cargo.toml -- --spec-dir spec/errors
```

Checks:
- Every example produces the codes its spec declares, by running the real
  parser and validator over it.

It does NOT check that fields are present or well formed: the frontmatter
schema does that when the file loads, which is the point of having one.

### The three tools that wrote into this directory, and why they are gone

`corpus_to_specs` and `enhance_specs` were DELETED under R5 of the spec-system
redesign; `fix_spec_layers` was refused from Phase 1b and deleted with R4.

All three decided what a spec should say by running the implementation:
`corpus_to_specs` recorded whatever chatter emitted on a fixture,
`fix_spec_layers` set the `Layer` bullet from whether the parser succeeded, and
`enhance_specs` wrote descriptions and manual links. The result is that the
specification was derived from the implementation and then used to test it, so
every gate passed by construction. 152 of the files here came from that route,
and 91 still carry the stub sentence they were born with.

There is nothing to run in their place. Deciding what CHAT requires is the work;
see the liquidation queue (R8) in
`docs/design/2026-08-15-spec-system-redesign.md`.

### Regenerating the validation corpus

Generates the validation fixture corpus + manifest from error specs.

```bash
just spec-gen      # every artifact derived from spec/
just spec-check    # or: is the committed copy current?
```

Generates:
- One `.cha` fixture per example (every spec, both stages)
- `manifest.json` (spec code + claim + status + source spec per fixture)

## Status

Every count that used to sit here was roughly four times out of date: "Total
Specs: 62 files" against 236, "Auto-generated specs: 59" against 152, and a
"Parser layer: 51 / Validation layer: 3" split that had been wrong for months.
None carried the command that produced it, which is why nobody noticed.

Ask the tools instead, all of which derive their answer from the same loader
the gates use:

```bash
just spec-status                          # counts by status, and the example tally
cargo run --bin coverage -- --errors      # which specs demonstrate their own code
```

### Error Corpus

The legacy error corpus (`tests/error_corpus/`) contains 101 test files covering ~60 unique error codes. These files use `@Comment` headers to document expected errors and were once converted by `corpus_to_specs`, which R5 DELETED for writing into the source of truth; there is nothing to run in its place.

## Contributing

When adding a new validation rule:

1. Create error spec (or generate from corpus file)
2. Validate spec format
3. Generate tests
4. Implement validator (TDD: fail → pass)
5. Verify reference corpus still passes
6. Commit spec, tests, and validator together

## See Also

- [../docs/ERROR_SPEC_FORMAT.md](../docs/ERROR_SPEC_FORMAT.md) - Detailed format specification
- [talkbank-model validation CLAUDE.md](../../crates/talkbank-model/src/validation/CLAUDE.md) - Validator implementation patterns
- [Root CLAUDE.md](../../CLAUDE.md) - TDD and testing requirements

---

Last Updated: 2026-08-21
