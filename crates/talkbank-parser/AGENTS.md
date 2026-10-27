# Tree-sitter parser agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-parser/AGENTS.md)

Read the [root guidance](../../AGENTS.md) first. This file adds the Tree-sitter
backend's ownership and boundary rules; it does not create separate gates.
Grammar changes are governed by [grammar/AGENTS.md](../../grammar/AGENTS.md).

## What this crate is

The Tree-sitter CHAT backend: its grammar's CST lowered into
`talkbank-model` values. `api/` is the public surface (`TreeSitterParser`,
`ChatParser` impl, fragment entry points, `fragment.rs` rebasing); `parser/`
is the lowering (`chat_file_parser/`, `tree_parsing/`, `tier_parsers/`,
`typed_cst/`); `generated_traversal.rs` and `node_types.rs` are generated.
Architecture: [parsing](../../book/src/architecture/parsing.md),
[parser-model contracts](../../book/src/architecture/parser-model-contracts.md),
[parser backends](../../book/src/architecture/parser-backends.md).
Both parsers have equal standing in shared-model design; Tree-sitter was
prototyped first, not granted ownership of its syntax or recovery types.

## Generated typed traversal

- Production lowering consumes the generated typed traversal (`NodeSlot`,
  the `*ChildrenRule` descriptors, `SourceSlotView`). Hand-walking
  `node.kind()` and classifying ERROR-node text are banned; the remaining
  `.kind()` sites outside generated code are the typed-traversal migration's
  residue (row 9 of the correctness architecture), to be retired, not grown.
- The traversal and the C metadata bridge come from tree-sitter-grammar-utils,
  pinned at `tsgu-revision`. **When this crate needs a capability the
  generated module lacks, improve the generator**: a descriptor, a proof, a
  slot shape or an accessor belongs in tsgu, never in a hand edit of generated
  output and never in a consumer that re-derives what the generator knows.
  Commit tsgu, pin, regenerate through `just traversal-gen` and
  `just conformance-gen`, then commit the migrated consumers; the module
  header carries the generator's revision, so the order is not optional.
- `generated_traversal_is_current` hashes the grammar JSON; a passing check
  proves what it hashes and nothing else. A generator API change requires
  consumer migration even when the grammar is unchanged.

## Locations

- Every model value this crate builds is located: `node_span::span_of(node)`
  is the one adapter from a CST node to a `Span`, and the constructor takes
  that span first. This crate is held at **zero** production
  `Location::Unlocated` by the fabricated-AST gate in `talkbank-parser-tests`.
  A fallback arm that reads a location "in case" it is unlocated is a defect
  here, not caution: the parser has every node's span in hand.
- `DiagnosticLocation::Unlocated` is different and allowed where a diagnostic
  truly has nothing to point at (an unrepresentable fragment origin).
- Fragment APIs (`api/fragment.rs`) wrap the caller's input in a synthetic
  envelope, parse, then rebase. The tier's own span is clamped to the caller's
  input before the move (`DependentTier::map_location`); payload spans move by
  `SpanShift`; an empty input is located at the caller's offset, zero width;
  diagnostics are rebased through the fragment's `error_sink`. Fragment
  origins and option context must describe the actual caller input.

## Recovery and diagnostics

- The shared model's
  [no-defaults/no-sentinels principle](../../book/src/architecture/chat-model/chat-model.md#no-silent-defaults-or-sentinels)
  applies to lowering: retain explicit recovery or return failure. A fabricated
  period, empty body or other plausible placeholder can cause spurious cascades
  and silent corruption. Earlier Tree-sitter behavior does not justify it.
- Recovery is not validity. Keep the whole-tree recovery backstop beside
  per-position handling; both are load-bearing. Never drop a recovery node,
  never fabricate a model value for one, never read a `MISSING` node as the
  value it stands for.
- Diagnostics are architecture-native: derived from CST kinds, source-bound
  recovery evidence and the owning traversal's context. Matching another
  validator's code, wording or count is not a goal. A construct or parser bug
  is fixed by writing the spec first (`spec/`), then `just regen`.
- CA arrows and TCU markers remain separators, linkers remain linkers; no
  conversion pass promotes them to terminators
  ([CA terminator resolution](../../book/src/architecture/parser-and-grammar/ca-terminator-resolution.md)).

## Verification

`cargo test -p talkbank-parser --lib` and the crate's `tests/integration/`
cover lowering contracts; whole-document, fragment, recovery and backend
comparison evidence lives in `talkbank-parser-tests` and
`talkbank-parser-re2c`. A grammar or traversal change needs `just regen`
first; `tree-sitter test` alone does not detect a stale `parser.c`.
