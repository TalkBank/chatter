# Parser, Model, and API Contracts

**Status:** Current
**Last updated:** 2026-05-19 16:54 EDT

## Single-handle parser API
`talkbank-parser` provides `TreeSitterParser` as the canonical API
handle for all parsing, full-file and fragment methods live directly
on the struct. Callers create one instance and pass
`&TreeSitterParser` everywhere. The alternate `talkbank-parser-re2c`
is opt-in (specification oracle and high-throughput batch parsing)
and produces the same `ChatFile` model.

## Contract for Batchalign

The Batchalign runtime (the `batchalign` crate) consumes these
guarantees from the talkbank-* core crates:

- parsing produces a typed `ChatFile` or an explicit parse-status signal
- parse-health taint is visible to alignment consumers
- alignment helpers operate on semantic model types, not raw text hacks
- recovery never fabricates valid-looking placeholder semantics for malformed input

The parser/model boundary stays honest enough for downstream
workflows, `align`, `compare`, `benchmark`, morphotagging, to make
their own validity decisions.

## Canonical Contract Model

### Public Contract Layers
1. Parse API Contract:
  - stable function signatures,
  - deterministic parse result envelope,
  - clear partial-success semantics.
2. Semantic Model Contract:
  - stable core model fields,
  - explicit unstable/internal fields policy.
3. Diagnostic Contract:
  - stable error code IDs and severity semantics,
  - best-effort message text compatibility.
4. Serialization Contract:
  - deterministic output constraints,
  - normalized formatting policy.

### Required Types
- `ParseOutcome<T>`
  - `value: T | omitted-by-status`
  - `diagnostics: Vec<Diagnostic>`
  - `status: Success | Partial | Failed`
- `Diagnostic`
  - `code`, `severity`, `category`, `message`, `location`, `context`, `suggestion`

## Parser Role
- `talkbank-parser`: the sole parser, used by CLI/LSP/API/batchalign3. `TreeSitterParser` is the only API handle, callers create one and pass `&TreeSitterParser` everywhere.
- Tree-sitter GLR provides error recovery; the Rust traversal code converts CST to typed model.
- Full-file methods: `parser.parse_chat_file()`, `parser.parse_chat_file_streaming()`.
- Fragment methods: `parser.parse_word_fragment()`, `parser.parse_main_tier_fragment()`, etc.

## Invariants
1. Parsing with offset must shift all spans consistently.
2. Parse-level and validation-level diagnostics must remain distinguishable.
3. Serialization should preserve semantic equivalence and documented formatting rules.
4. Roundtrip behavior must be testable per parser implementation.
5. Parser functions that accept `ErrorSink` should not return `Option<T>` for fallible parse state.

## API Versioning Policy (Pre-Release but Strict)
- Three intended contract levels:
  - Stable-for-integrators
  - Stable-internal
  - Experimental
- Mark every public function/type by contract level.

This classification is not yet codified in a separate manifest file;
the levels above are the working policy. Integrators should treat any
unmarked surface as Experimental until contract levels are formally
published.

## Acceptance Criteria
- Single canonical parse outcome envelope exposed for integrators.
- Parser implementations conform to shared contract tests.
- Contract-level annotations exist for all public API surfaces.
- Documentation for parse/validate/serialize lifecycle is centralized and current.

## Recovery Contract: No Fabricated Semantic Values
The parser contract must forbid sentinel semantic values during error recovery.

Disallowed recovery behavior:
- returning arbitrary enum variants as fallback for unknown/missing nodes,
- returning empty strings as stand-ins for required fields,
- constructing fake words/chunks like `"missing"`, `"error"`, or other placeholders.

Required recovery behavior:
1. Emit structured diagnostic with precise span and expected node kind.
2. Return an explicit parse-status signal (`Partial`/`Failed`) through `ParseOutcome`.
3. Omit invalid semantic node OR store it in explicit recovery metadata, never as a valid semantic value.

Current enforcement:
- CI guardrail script tracks and blocks introduction of new `ErrorSink + Option` signatures.
- See `scripts/check-errorsink-option-signatures.sh` and `scripts/errorsink_option_allowlist.txt`.

Rationale:
- fabricated semantic values create secondary, misleading diagnostics against synthetic data,
- downstream tools cannot distinguish real user content from parser-generated placeholders,
- equivalence and regression tests become noisy and non-actionable.

For `batchalign3`, this is especially important because alignment workflows
must be able to tell the difference between:

- a malformed input that should taint or block alignment
- a recoverable input where raw text can be preserved
- a clean input that should proceed through the align/compare pipeline

## String Storage Policy

The model uses three string storage strategies:
- **`Arc<str>` interning** (`interned_newtype!`): For high-frequency repeated values (POS tags, stems, speaker codes). Global interner avoids redundant allocations.
- **`SmolStr`** (`string_newtype!`): For short strings (median 10-15 chars) that benefit from inline storage. O(1) clone, no heap allocation for strings ≤23 bytes.
- **`String`**: Only for utility types outside the core model (e.g., `semantic_diff/`).
