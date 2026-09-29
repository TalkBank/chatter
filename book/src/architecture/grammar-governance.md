# Grammar System and Token Governance

**Status:** Current
**Last modified:** 2026-09-28 13:01 EDT

## Current Reality
`grammar/grammar.js` encodes substantial implicit language knowledge directly in regex exclusions,
reserved symbol lists, and leniency decisions. Example areas:
- word segment forbidden start/rest classes,
- CA delimiter/element symbol groups,
- event segment exclusions,
- hand-maintained coupling between comments and token rules.

This is currently powerful but fragile.

## Primary Failure Modes
1. New symbolic token added in one place but not in exclusion sets.
2. Parser behavior changes silently due to regex class edits.
3. Generated node types drift from assumptions in spec tooling.
4. Lenient parsing choices become undocumented policy.

## Current Design
The generated symbol registry is the single source of token constraints.
The pipeline has shipped, `just symbols-gen` rebuilds it.

### Registry Artifacts
- `spec/symbols/symbol_registry.json` (human-authored intent):
  - symbol string
  - category (delimiter, continuation, overlap, punctuation, etc.)
  - contexts where reserved/allowed
  - parse role and precedence notes
- Generated outputs:
  - `grammar/src/generated_symbol_sets.js`
  - `crates/talkbank-model/src/generated/symbol_sets.rs`
  - `spec/tools/src/generated/symbol_sets.rs`
  - docs: [Symbol Registry](symbol-registry.md)

## Grammar Refactor Requirements
1. Replace large manual regex strings with generated character classes.
2. Keep final grammar readable by preserving semantic names in generated constants.
3. Distinguish clearly between:
  - syntax permissiveness,
  - semantic validation restrictions.
4. Add comments only for design rationale, not for duplicating manual references.

## Node Type Drift Controls
- Enforce regeneration and consistency checks:
  - grammar source change must regenerate parser and node types,
  - node type constants consumed by `spec/tools` and parser code must compile,
  - CI fails if generated files differ from committed state.

## Leniency Policy
Explicitly classify every lenient parse behavior:
- Parse-lenient + validate-strict.
- Parse-lenient + validate-warning.
- Parse-strict (hard fail).

Document this matrix in the [Leniency Policy](leniency-policy.md).

### Recognized but unsupported headers

Grammar recognition does not guarantee a supported model representation.
`@Thumbnail` is recognized structurally but remains unsupported: source-bound
lowering reports E525 over the declaration and refuses to admit that header.
Recovery preserves following speech, but the recovered document is not a
strictly accepted document or a lossless serialization of the input. The E525
spec pairs this refusal with an otherwise identical supported `@Comment`
control. This is a Chatter support boundary, not a claim that thumbnail headers
are forbidden by the wider CHAT language.

### Date and time token selection

Date/time headers declare strict lexical alternatives and whole-line fallbacks.
Their selection must respect both complete values and malformed suffixes.
Higher lexical precedence is not a harmless tie-break: it can select a strict
prefix before a longer malformed value. Rule-order changes must also preserve
empty-header recovery, not merely improve selection on valid examples. Follow
Tree-sitter's [conflicting-token rules](https://tree-sitter.github.io/tree-sitter/creating-parsers/3-writing-the-grammar.html#conflicting-tokens)
and review the complete diagnostic snapshot after any such change.

These CST types prove lexical shape, not calendar or clock validity. In
particular, `strict_time` includes the shared digit/separator alphabet; the
checked model and header-specific validator still decide whether a value is
supported and in range. Selection tests need complete reference values, suffix
error specimens and the established empty-header policies. A declared strict
alternative alone does not prove that the compiled lexer ever selects it.

Empty fields have header-specific policy: an empty `@Date` is E516, whereas
empty birth/start/duration values retain the existing omission policy. Their
spec controls require the header and following speech to survive parsing and
JSON replay with byte-exact CHAT output. Nonempty malformed suffixes remain
E518/E540/E541. Do not infer midnight from the legacy start-time model's zero
components when its preserved value is empty; that representation is not
evidence of a known time.

## Grammar Test Strategy
1. Keep corpus tests generated from `spec/constructs`.
2. Add targeted hand-authored edge tests for symbol boundary interactions.
3. Add mutation-style tests for forbidden-character regressions.
4. Add parser equivalence tests for tokenizer-sensitive cases.

## Acceptance Criteria
- No manual reserved-symbol duplication in `grammar.js`.
- Symbol registry is generated to all required consumers.
- Grammar modifications cannot land with stale generated artifacts.
- Every special token category has explicit policy documentation.
