# tree-sitter-talkbank

Rust bindings for the [TalkBank CHAT](https://talkbank.org) tree-sitter grammar.
This is the primary supported downstream binding for v0.1.

## Installation

```sh
cargo add tree-sitter tree-sitter-talkbank
```

## Usage

```rust
use tree_sitter::Parser;

fn main() {
    let code = "@UTF8\n@Begin\n@Participants:\tMOT Mother\n*MOT:\thello .\n@End\n";
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_talkbank::LANGUAGE.into())
        .expect("Error loading TalkBank CHAT parser");
    let tree = parser.parse(code, None).unwrap();
    assert!(!tree.root_node().has_error());
}
```

The crate also exposes query constants when the corresponding files are present:

- `HIGHLIGHTS_QUERY`: syntax highlighting
- `LOCALS_QUERY`: speaker scope tracking
- `TAGS_QUERY`: document symbols

See the [tree-sitter docs](https://tree-sitter.github.io/) for more details.
