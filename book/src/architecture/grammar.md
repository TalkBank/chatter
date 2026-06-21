# Grammar

**Status:** Current
**Last updated:** 2026-03-24 00:01 EDT

The CHAT grammar is defined in `grammar/grammar.js` using the [tree-sitter](https://tree-sitter.github.io/) parser generator. It produces a GLR parser that handles the full CHAT format with error recovery.

## Design Principles

### Explicit Whitespace

Unlike most tree-sitter grammars, CHAT does not use `extras` for whitespace. All whitespace is grammar-visible because CHAT's structure is whitespace-sensitive:

- Tab separates tier prefix from content
- Newline ends tiers
- Line continuation uses tab-at-start-of-line
- Space separates words and annotations

### Two-Level Structure

The grammar has two structural levels:

1. **Document level**: headers, utterances, `@Begin`/`@End`
2. **Tier level**: main tier content, dependent tier content (each with distinct rules)

### Opaque Lemmas

In the `%mor` tier rules, lemmas are parsed as opaque Unicode strings. The grammar does not attempt to decompose lemma content, that happens in the model layer. This follows the "parse, don't validate" principle.

## Key Grammar Rules

### Document Structure

```text
document → utf8_header, begin_header, lines..., end_header
line → header | utterance
utterance → main_tier, dependent_tiers...
```

### Main Tier

```text
main_tier → star, speaker, colon, tab, tier_body
tier_body → contents, utterance_end
contents → content_item, (whitespace, content_item)...
```

### MOR Tier (UD-style)

```text
mor_contents → mor_content, (whitespace, mor_content)..., terminator
mor_content → mor_word, mor_post_clitic*
mor_word → mor_pos, pipe, mor_lemma, mor_feature*
mor_post_clitic → tilde, mor_word
mor_feature → hyphen, mor_feature_value
```

POS tags are simple identifiers (no subcategories). Lemmas are opaque strings. Features are hyphen-separated values that may contain `=` for Key=Value pairs and `,` for multi-value features.

## Grammar Change Workflow

`parser.c` is generated from `grammar.js`, never edit it directly.

After any change to `grammar.js`:

1. `cd grammar && tree-sitter generate`
2. `tree-sitter test` (160 tests)
3. `cargo test -p talkbank-parser`
4. `cargo nextest run -p talkbank-parser-tests` (reference corpus equivalence, per-file)
5. Verify the 78-file reference corpus passes at 100%

## Conflict Resolution

The grammar uses tree-sitter's precedence and conflict mechanisms to handle ambiguities:

- Word tokens use `prec(5)` to win over separators
- Inline bullets use `prec(10)` for their delimiters
- CA (conversation analysis) symbols use `prec(3)` for colon disambiguation

## Generated Artifacts

Running `tree-sitter generate` produces:
- `src/parser.c`: the C parser
- `src/node-types.json`: node type metadata

The Rust crate `talkbank-parser` references `node-types.json` to generate `node_types.rs` (a generated constants file).
