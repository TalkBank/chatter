# Panic audit: talkbank-parser

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/panic-audit/talkbank-parser.md)

See [shared policy](README.md) for enforcement and verification limits.
The package inherits the workspace panic-shape restriction lints.

CST traversal, source admission and recovery use typed failures and diagnostic
sinks. This design goal does not prove every execution path panic-free.

Allowances in [document_root.rs](../../crates/talkbank-parser/src/parser/document_root.rs)
and [typed_cst.rs](../../crates/talkbank-parser/src/parser/typed_cst.rs)
belong to `#[cfg(test)]` modules. A source search must classify the enclosing
configuration rather than count those as production exceptions. Excluding
every file containing tests would also hide production code.

The former blanket claim that any new panic fails a normal build was too
strong: ordinary compilation does not run Clippy. Generated traversal, explicit
exceptions and implicit panic paths require distinct evidence.
