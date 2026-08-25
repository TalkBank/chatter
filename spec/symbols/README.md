# Symbol Registry

**Last modified:** 2026-08-25 14:14 EDT

`symbol_registry.json` is the single owner of what each CHAT symbol MEANS and
how it parses. Everything downstream is generated from it.

## Two kinds of entry, deliberately different shapes

- **`symbols` are ENTITIES.** Each carries an identity (`codepoint`), a name
  (`id`), a `gloss`, a `parse_role`, a `notation_family` and a runnable
  `example`, and each maps 1:1 to a Rust enum variant.
- **`character_classes` are SETS.** Bags of characters with no individual
  identity, used to build the grammar's word and event regexes.

The `paired_stretch_symbols` and `word_attached_symbols` arrays the grammar and
the model consume are **derived** from `parse_role` and appear nowhere in the
file. They are named for the role they are derived FROM: until 2026-08-25 they
were `ca_delimiter_symbols` and `ca_element_symbols`, which named provenance on
a value holding a parse role.

**`parse_role` is not provenance.** It says what the grammar does with a symbol;
`notation_family` says where the symbol comes from. Two of the 25 are disfluency
marks rather than Conversation Analysis notation, and CLAN names them
`NOTCA_CROSSED_EQUAL` and `NOTCA_LEFT_ARROW_CIRCLE`. Never read provenance off
the name of a `ca_*` array.

## Where the checking lives

`registry.js` is the one reader, and it validates on load: required fields,
snake_case unique ids, well-formed unique codepoints, `parse_role` and
`notation_family` from their closed sets, every example containing its own
symbol, and single-scalar non-duplicate character classes. A malformed registry
therefore cannot reach a generator whether or not anyone runs a script.

`validate_symbol_registry.js` is a **report**, not a gate:

```bash
node spec/symbols/validate_symbol_registry.js
```

Two things this README claimed until 2026-08-20 and that were never or are no
longer true: lexicographic ordering is **not** enforced in any category (and the
architecture page explains why forcing it buys nothing), and the CA
delimiter/element disjointness check is **gone**, because both sets derive from
one `parse_role` field and a symbol in both is unrepresentable.

## Generated output

- `grammar/src/generated_symbol_sets.js`
- `crates/talkbank-model/src/generated/symbol_sets.rs`
- `spec/tools/src/generated/symbol_sets.rs`
- `crates/talkbank-model/src/generated/ca_symbols.rs`
- `book/src/chat-format/generated/ca-symbols.md`

Every symbol's `example` is additionally parsed and validated by
`crates/talkbank-parser/tests/integration/symbol_registry_examples.rs`, so a
documented usage that stops being valid CHAT fails the build.

## Regeneration

```bash
just symbols-gen
```

Do not edit generated files manually. The drift gate
(`generated_symbol_sets_are_current`) runs every generator in `--check` mode.
