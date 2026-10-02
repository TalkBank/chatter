# Symbol Registry Architecture

**Status:** Current
**Last modified:** {{git-dates:page}}

## Purpose

`spec/symbols/symbol_registry.json` is the canonical source of the token and
symbol classes CHAT tokenization policy depends on. It is one of two closed
vocabularies owned under `spec/`; the other is
[the form-marker registry](https://github.com/TalkBank/chatter/blob/main/spec/form_markers/README.md).

## Scope

The registry holds two kinds of entry, and they are deliberately different
shapes.

**`symbols` are ENTITIES.** Each has an identity (a codepoint), a name, a
meaning, a parse role, a notation family and a runnable example, and each maps
1:1 to a Rust enum variant. These are the 25 word-attached and paired-stretch
symbols.

**`character_classes` are SETS.** Bags of characters with no individual
identity, used to build the grammar's word and event regexes: word-segment
forbidden (start, rest, common) and event-segment forbidden (base, common).

Storing a set as a list of records, or an entity as a bare character, would be
the same category error in opposite directions.

The `paired_stretch_symbols` and `word_attached_symbols` arrays the grammar and
the model consume are **derived** from `parse_role` and appear nowhere in the
file, and they are NAMED for the role they are derived from, not for a notation family:
a name asserting provenance on a value holding a parse role would be the
collapse the next section warns about.

## parse_role is not provenance

`parse_role` says what the GRAMMAR does with a symbol. `notation_family` says
where it comes from. They are independent, and collapsing them would file two
disfluency marks (`≠` blocking, `↫` segment repetition) as
Conversation Analysis notation; CLAN names them `NOTCA_CROSSED_EQUAL` and
`NOTCA_LEFT_ARROW_CIRCLE`. Code that needs to know "is this CA" calls
`notation_family()`; never infer it from the name of a `ca_*` array.

## Rules

1. Symbols change in `spec/symbols/symbol_registry.json` and nowhere else.
2. Regenerate after any change: `just symbols-gen`, which validates the
   registry and then runs both generators.
3. Generated files are never edited by hand.

## What is enforced, and where

Most structural checking happens in `spec/symbols/registry.js`, which every
generator reads the registry through, so a malformed registry cannot reach a
generator even if nobody runs the validator: required fields present, ids
snake_case and unique, codepoints well-formed and unique, `parse_role` and
`notation_family` from their closed sets, and every example containing its own
symbol. `validate_symbol_registry.js` adds the character-class checks (single
Unicode scalar values, no duplicates) and prints the report.

There is no disjointness check. The two derived arrays come from a single
`parse_role` field, so a symbol in both is unrepresentable and there is
nothing to assert.

Every example is additionally PARSED AND VALIDATED by
`crates/talkbank-parser/tests/integration/symbol_registry_examples.rs`, so a
documented usage that stops being valid CHAT fails the build. The uniform
example template is valid for 24 of the 25 symbols and invalid for `↫`, which
needs a stem outside its brackets.

**Lexicographic ordering is NOT required**, and no category is sorted. Nothing
enforces it, and the validator says in its own comment that semantic grouping
is more useful than forced ordering. Reordering would make a large diff that
buys nothing.

## Generated outputs

| Output | Consumer |
|---|---|
| `grammar/src/generated_symbol_sets.js` | imported by `grammar/grammar.js` |
| `crates/talkbank-model/src/generated/symbol_sets.rs` | model and validation |
| `spec/tools/src/generated/symbol_sets.rs` | spec tooling |
| `crates/talkbank-model/src/generated/ca_symbols.rs` | `CAElementType`, `CADelimiterType`, `NotationFamily` |
| `book/src/chat-format/generated/ca-symbols.md` | included by the book's symbols page |

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

**The generator list is DISCOVERED, not written down.** A gate that lists what
it covers stops covering things silently, so the gate globs
`spec/symbols/generate_*.js` and refuses to report at all if the glob finds
fewer than two.

A hand-edit to a generated symbol set is therefore detected by the gate, which
runs in CI.

## Change workflow

1. Edit the registry JSON.
2. `just symbols-gen`.
3. Regenerate the parser if the grammar's tokenization changed:
   see [Grammar Workflow](../contributing/grammar-workflow.md).
4. Run the gates: `cargo test --manifest-path spec/Cargo.toml --workspace`
   and `just test`.
5. Commit the registry and every regenerated output together.
