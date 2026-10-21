# tree-sitter-talkbank

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/bindings/rust/README.md)

Rust bindings for the [TalkBank CHAT](https://talkbank.org) tree-sitter grammar.
This is the binding used by `talkbank-parser` in this repository.

## Source-checkout integration

```toml
[dependencies]
tree-sitter = "0.27.1"
tree-sitter-talkbank = { path = "../chatter/grammar" }
```

Adjust the path to the full checkout. The Tree-sitter version above matches
the grammar package's current development dependency; it is not a promise about
all future versions. This source dependency does not require publishing the
grammar crate. Registry availability and release compatibility must be checked
separately; see the [publication contract](../../../book/src/contributing/crates-io-publication.md).

## Usage

```rust
use tree_sitter::Parser;

fn main() {
    let code = "@UTF8\n@Begin\n@End\n";
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_talkbank::LANGUAGE.into())
        .expect("Error loading TalkBank CHAT parser");
    let tree = parser.parse(code, None).unwrap();
    assert!(!tree.root_node().has_error());
}
```

This is a syntax-loading illustration matching the crate's
[compiled example](lib.rs), not a valid-transcript assertion. A tree without
syntax errors can still violate CHAT header, content or alignment rules.
Use [the typed parser and validation APIs](../../../crates/talkbank-parser/README.md)
when those rules are required; do not infer them from `has_error()` alone.

The crate also exposes query constants when the corresponding files are present:

- `HIGHLIGHTS_QUERY`: syntax highlighting
- `LOCALS_QUERY`: speaker scope tracking
- `TAGS_QUERY`: document symbols

See the [tree-sitter docs](https://tree-sitter.github.io/) for more details.
