# Grammar agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/AGENTS.md)

The [repository guidance](../AGENTS.md) owns safety, verification and publication
policy. This file adds grammar-specific boundaries; it does not define another
gate or release workflow.

## Read before changing grammar or generated consumers

- [Grammar Workflow](../book/src/contributing/grammar-workflow.md): regeneration,
  pinned traversal producer, consumer migration and evidence limits.
- [Spec Workflow](../book/src/contributing/spec-workflow.md): authored examples,
  claims, generated fixtures and observation review.
- [Dev Checks](../book/src/contributing/dev-checks.md): focused feedback and
  final publication checks.
- [Setup](../book/src/contributing/setup.md): toolchain and Node prerequisites.

## Source and output ownership

`grammar.js` owns syntax. The symbol registry owns shared character sets.
Generated parser, node-type, traversal, bridge and inventory outputs must be
regenerated from their owners, never repaired by hand.

| Path | Owner |
| --- | --- |
| `src/parser.c`, `src/grammar.json`, `src/node-types.json` | Tree-sitter generation from `grammar.js` |
| `src/generated_symbol_sets.js` | `spec/symbols/symbol_registry.json` through `just symbols-gen` |
| `test/corpus/generated/` | Authored specs through `just spec-gen` |
| `test/corpus/manual/` | Hand-maintained grammar tests; no spec generator owns this directory |
| `../crates/talkbank-parser/src/generated_traversal.rs` and `bindings/rust/tsgu_language_metadata.c` | The pinned traversal recipe |
| `../crates/talkbank-parser-tests/src/conformance/inventory.rs` | `just conformance-gen` from the current traversal |

Whole-directory generated owners retain unchanged files and prune obsolete
ones through checked ownership. Do not clear `test/corpus/`, remove ownership
markers or put authored work in generated destinations. The spec system
requires its `.generated-output-dir` marker and refuses conflicting human
ownership and linked entries.

## Regeneration and verification

After a grammar/spec/registry implementation change, use the root workflow's
consolidated `just regen` and relevant tests. The book explains the dependency
steps; do not maintain a second hand-copied sequence here. Documentation-only
edits do not require rebuilding an unchanged parser.

Generate the parser before relying on behavior after a `grammar.js` change:
`tree-sitter test` alone does not detect stale generated parser source. Keep
the grammar, generated artifacts, typed consumers and reviewed spec changes in
one coherent change. Inspect the observation snapshot rather than treating
regeneration as approval of new output.

Traversal generation uses the existing recipe with a clean generator checkout
at `tsgu-revision`. It produces both the Rust traversal and C metadata bridge.
Use the [canonical procedure](../book/src/contributing/grammar-workflow.md#3-regenerate-the-typed-traversal),
not raw stdout redirection or an assumed build-directory executable. A generator
API change requires consumer migration even if the grammar hashes are unchanged.
Do not infer producer currency from grammar-hash checks alone.

**Improve the generator when Chatter would benefit.** The typed traversal and
its C bridge come from tree-sitter-grammar-utils (tsgu). When a consumer needs
a capability the generated module lacks (a descriptor, a proof, a slot shape,
an accessor), the fix belongs in tsgu, not in a hand edit of generated output
and not in a consumer-side workaround that re-derives what the generator
knows. Order matters, because the generator stamps its own revision into the
module header: commit tsgu, pin `tsgu-revision`, regenerate both artifacts
through the owning recipe, then commit Chatter with its migrated consumers.

Retain real spec/reference workflows, parser equivalence and roundtrip checks
for affected contracts. Passing a finite corpus is not a proof of all parser
states or complete format support. Confirm selected tests actually ran.

## Design and diagnosis

- Keep whitespace explicit. Structural word markers are distinct CST children;
  the symbol registry owns their exclusions from `word_segment`.
- Consume generated typed traversal and associated source capabilities in
  production lowering. Do not recreate a raw kind-walking parser or pair nodes
  with independently supplied source strings.
- Preserve genuine recovery states. Source identity, readable ranges and
  grammar-kind admission prove different things; none alone proves validity or
  presence of every required child.
- For closed header vocabularies, typed recognition plus an explicit unsupported
  value can preserve recoverable input for a specific diagnostic. Do not
  fabricate a default value or change a spec claim merely to match output.
- When a syntax change changes diagnostic behavior, review the affected error
  specs and snapshot together against the intended rule. A changed parser result
  does not automatically authorize deleting an example or relabeling its claim.

On failure, preserve the checkout and logs, identify the cause and repair the
coherent batch. Do not automatically revert, reset, stash or discard another
contributor's work. Reverting a reviewed change, when authorized, also requires
restoring its generated consumers consistently. Publication remains separate.

## Design references

- [Word Internals](../book/src/chat-format/word-internals.md)
- [Overlap Marker Binding](../book/src/architecture/overlap-binding.md)
- [Tokenization Rules](docs/tokenization-rules.md)
- [Precedence Decisions](docs/precedence-decisions.md)
- [Structuring Opaque Tokens](docs/structuring-opaque-tokens.md)
- [Pre-coarsening reference](docs/pre-coarsening-grammar.js.reference): historical
  grammar shape, not current source authority.
