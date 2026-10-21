# talkbank-lsp Architecture

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-lsp/ARCHITECTURE.md)

Start with the book's [crate reference](../../book/src/architecture/crate-reference.md#talkbank-lsp)
and [diagnostic contract](../../book/src/chatter/integrating/diagnostic-contract.md).
The crate-root [Rust documentation](src/lib.rs) identifies the embedding entry
points and internal module responsibilities.

[LSP Testing and Diagnosis](../../book/src/contributing/lsp-testing.md) owns the
process boundary and verification guide; [Editor Integration](../../book/src/chatter/user-guide/editor-integration.md)
owns the end-user setup tutorial. These links replace the former placeholder
architecture page, not a second architecture specification.
