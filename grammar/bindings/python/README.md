# tree-sitter-talkbank

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/bindings/python/README.md)

Python binding scaffold for the [TalkBank CHAT](https://talkbank.org) tree-sitter grammar.

This binding is not the primary maintained integration.
The actively supported downstream integration is Rust (used by `talkbank-parser`).

For the maintained source integration, see the [Rust binding](../rust/README.md).
Binding availability does not establish CHAT validity, published-package
availability or Python/platform qualification. Chatter's typed parsing and
validation APIs have their own [consumer contract](../../../crates/talkbank-parser/README.md).
