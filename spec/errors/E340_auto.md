+++
code = 'E340'
name = 'UnknownBaseContent'
kind = 'Invalidity'
status = 'unreachable_from_chat'
+++

## Description

Main tier content could not be classified as any known word or construct
type. This fires when a `base_content_item` CST node has a child kind
that the Rust parser doesn't recognize, indicating a grammar/parser
mismatch (the grammar produces a new node type that the parser hasn't
been updated to handle).

## Notes

- This error indicates a grammar/parser mismatch, not a CHAT input error.
- It cannot be triggered by any CHAT input with the current grammar; it
  would only fire if the grammar added a new `base_content_item` variant
  without updating the Rust parser. That is what `Status:
  unreachable_from_chat` says, and this spec asserted it in prose while
  claiming `implemented` until 2026-08-20.
- No example is possible with the current grammar. The status therefore owes a
  named out-of-corpus test instead, and until 2026-08-20 it could not pay one:
  `UnknownBaseContent` appeared exactly ONCE in the tree, at the line emitting
  it, because the decision lived inside a `match` on a `tree_sitter::Node` and
  a test would have had to fabricate a node the grammar cannot produce.
- Out-of-corpus tests: `BaseContentKind::from_node_kind` in
  `crates/talkbank-parser/src/parser/tree_parsing/main_tier/content/base/mod.rs`
  is now the trigger condition as a total function, covered by
  `unknown_kind_is_not_recognised` (an unrecognised kind classifies as `None`,
  which is what this error reports) and `every_grammar_alternative_is_recognised`
  (no alternative the grammar declares is silently dropped).
