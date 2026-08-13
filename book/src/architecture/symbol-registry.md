# Symbol Registry Architecture

**Status:** Current
**Last modified:** 2026-08-12 19:05 EDT

## Purpose

`spec/symbols/symbol_registry.json` is the canonical source of the token and
symbol classes CHAT tokenization policy depends on. It is one of two closed
vocabularies owned under `spec/`; the other is
[the form-marker registry](https://github.com/TalkBank/chatter/blob/main/spec/form_markers/README.md).

## Scope

The registry governs:

- CA delimiter symbols,
- CA element symbols,
- word-segment forbidden symbol classes (start, rest, common),
- event-segment forbidden symbol classes (base, common).

## Rules

1. Symbols change in `spec/symbols/symbol_registry.json` and nowhere else.
2. Regenerate after any change: `just symbols-gen`, which validates the
   registry and then runs both generators.
3. Generated files are never edited by hand.

## What the validator actually enforces

- every entry is a single Unicode scalar value, and non-empty;
- no duplicates within a category;
- `ca_delimiter_symbols` and `ca_element_symbols` are disjoint;
- every required category is present.

**Lexicographic ordering is NOT required**, and no category is sorted. This
page previously said it was, which was wrong in both directions: nothing
enforces it, and the validator says in its own comment that semantic grouping
is more useful than forced ordering. A contributor who "fixed" the ordering
would be making a large diff that buys nothing.

## Generated outputs

| Output | Consumer |
|---|---|
| `grammar/src/generated_symbol_sets.js` | imported by `grammar/grammar.js` |
| `crates/talkbank-model/src/generated/symbol_sets.rs` | model and validation |
| `spec/tools/src/generated/symbol_sets.rs` | spec tooling |

The Rust outputs are formatted by the generator itself, which runs `rustfmt`
before writing. That is not tidiness: without it `just fmt` re-wraps the const
arrays, re-running the generator un-wraps them, and the two rewrite the same
bytes forever with both sides correct. Generating and formatting have to be one
state. The form-marker generator does the same, for the same reason.

## The drift gate

`generated_symbol_sets_are_current`, in
`spec/tools/src/form_markers/mod.rs`, runs each generator in `--check` mode
(render, compare, write nothing, exit non-zero on drift) and fails if any
committed output disagrees with the registry. It runs in CI under
`cargo test --manifest-path spec/Cargo.toml --workspace`.

It runs the REAL generators rather than re-describing their output, so there is
no second description to drift. They are JavaScript, so the gate shells out to
`node`.

**Before 2026-08-12 there was no gate at all.** Nothing compared any of the
three outputs against the registry, and neither `just symbols-gen` nor the
validator ran in CI, so a hand-edit to a generated symbol set was undetectable.
This page claimed drift was "caught by the checked-in generated artifacts plus
the normal local verification sweep and CI checks"; none of that was true. The
gate found real drift on its first run: two Rust outputs were rustfmt-wrapped in
the tree and unwrapped by the generator.

## Change workflow

1. Edit the registry JSON.
2. `just symbols-gen`.
3. Regenerate the parser if the grammar's tokenization changed:
   see [Grammar Workflow](../contributing/grammar-workflow.md).
4. Run the gates: `cargo test --manifest-path spec/Cargo.toml --workspace`
   and `just test`.
5. Commit the registry and every regenerated output together.
