# Symbol Registry Architecture

**Status:** Current
**Last modified:** 2026-05-29 18:43 EDT

## Purpose
`spec/symbols/symbol_registry.json` is the canonical source of token/symbol classes used by
CHAT grammar tokenization policy.

## Scope
The registry currently governs:
- CA delimiter symbols,
- CA element symbols,
- word segment forbidden symbol classes,
- event segment forbidden symbol classes.

## Governance Rules
1. Symbol changes must be made only in `spec/symbols/symbol_registry.json`.
2. Registry must pass validation:
   - `node spec/symbols/validate_symbol_registry.js`
3. Grammar symbol sets must be regenerated after any registry change:
   - `just symbols-gen`
4. Generated files are read-only and must not be edited manually.

## Determinism Requirements
- Every category list in the registry must be lexicographically sorted.
- Duplicate symbols are forbidden.
- `ca_delimiter_symbols` and `ca_element_symbols` must be disjoint.

These constraints keep generated outputs stable and review diffs minimal.

## Consuming Outputs
Generated symbol constants are emitted to:
- `grammar/src/generated_symbol_sets.js`
- `crates/talkbank-model/src/generated/symbol_sets.rs`
- `spec/tools/src/generated/symbol_sets.rs`

`grammar/grammar.js` imports from this generated module to avoid manual duplication of
critical symbol policy.

## Change Workflow
1. Edit registry JSON.
2. Run registry validation.
3. Run `just symbols-gen`.
4. Run grammar generation/tests.
5. Run parser equivalence tests.
6. Commit source + generated outputs together.

## Auditability
Registry drift is caught by the checked-in generated artifacts plus the normal
local verification sweep and CI checks, so symbol changes should land together
with regenerated grammar and Rust outputs.
