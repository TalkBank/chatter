# Algorithms and Data Structures

**Status:** Current
**Last modified:** {{git-dates:page}}

This chapter is a map of algorithm boundaries, not a second specification of
every enum variant or parser method. The linked chapters own the detailed
contracts; generated API documentation owns exact signatures.

## Owned CHAT model

A `ChatFile` contains ordered header and utterance lines. Each utterance carries
main-tier content and dependent tiers. Nested groups make main-tier content a
tree, so counting top-level items is not the same as counting words.

See [Memory and Ownership](memory-and-ownership.md) for owned values, interned
strings, source spans and revision-bound analysis. Enum variant counts and
allocation measurements are not architectural invariants.

## Parsing and admission

The Tree-sitter backend converts a concrete syntax tree to the typed model.
Grammar acceptance, conversion, recovery and model validation are separate
boundaries. A recovered model is not evidence that its input was valid.

[Parsing](parsing.md) describes the backends and public parsing surface.
Use the current parser trait/API for fragments rather than inventing a
method name by appending `_fragment`. Keep generated grammar artifacts with
their generating inputs; see [Repository Architecture](repo-architecture.md).

## Traversal and structural alignment

Traversal must follow the selected domain's treatment of words, groups,
retracing and replacement content. A generic recursive walk is not a substitute
for tier-specific eligibility.

[Alignment](alignment.md) owns those rules and the structural alignment
algorithms. Structural alignment relates main-tier units to dependent-tier
items; it does not establish acoustic timing.

`%wor` consumers instead use the checked timing transitions described in
[`%wor` Timing Semantics](wor-timing.md). Matching counts alone cannot establish
word identity or acoustic correctness.

## Structural merge and lexical evidence

[Merge Architecture](merge-architecture.md) describes ordered-source assembly,
source-bound selection and the validation/reporting transitions. It is not a
global sort of independently flattened utterances.

[Merge Domain Types](merge-domain-types.md) distinguishes lexical scores,
confidence states, mapping syntax and output validation. Lexical similarity
can support a speaker decision but cannot prove recording identity.

## Cache and deterministic output

[Validation Cache](parser-and-grammar/validation-cache.md) owns the CHAT
validation-cache contract. Cache reuse must preserve the relevant input and
policy identity; a cache hit does not create new semantic evidence.

Choose collection order at each boundary: source order matters for transcript
events, while a wire format may define sorted keys. Neither all hash maps nor
all ordered maps are a universal correctness rule. Likewise, cache expensive
regex construction where its actual call pattern warrants it; a blanket claim
that every regex in the workspace has the same lifetime is not a contract.
