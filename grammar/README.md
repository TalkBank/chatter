# tree-sitter-talkbank

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/README.md)

[![CI](https://github.com/TalkBank/chatter/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/TalkBank/chatter/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/tree-sitter-talkbank)](https://www.npmjs.com/package/tree-sitter-talkbank)
[![crates.io](https://img.shields.io/crates/v/tree-sitter-talkbank)](https://crates.io/crates/tree-sitter-talkbank)

A [tree-sitter](https://tree-sitter.github.io/tree-sitter/) grammar for the [CHAT transcription format](https://talkbank.org/0info/manuals/CHAT.html) used by [TalkBank](https://talkbank.org).

CHAT (Codes for the Human Analysis of Transcripts) is a standardized format for transcribing and annotating conversational interactions, widely used in language acquisition research, aphasia studies, and other areas of linguistic analysis.

### The CHAT Manual

The CHAT manual is the external format reference. Chatter's authored specs,
validation policies and recorded compatibility decisions define its tested
implementation contract; successful grammar recognition alone does not prove
that contract or complete format support.

- **Word** (authoritative): [talkbank.org/0info/manuals/CHAT.docx](https://talkbank.org/0info/manuals/CHAT.docx)
- **HTML** (generated): [talkbank.org/0info/manuals/CHAT.html](https://talkbank.org/0info/manuals/CHAT.html)
- **PDF** (generated): [talkbank.org/0info/manuals/CHAT.pdf](https://talkbank.org/0info/manuals/CHAT.pdf)

### Syntax and validation

The grammar recognizes headers, main and dependent tiers, annotations, overlap
markers and Conversation Analysis notation, with recovery for malformed input.
The actual root is `source_file`, whose alternatives include `full_document`
and selected fragments. A complete-document parse nests its headers and
utterances under `full_document`; it does not produce a root named `document`.

Use an authored [reference transcript](../corpus/reference/core/basic-conversation.cha)
to inspect the current CST rather than copying a handwritten tree into tests.
From this directory, after the development prerequisites are installed:

```sh
npx --no-install tree-sitter parse ../corpus/reference/core/basic-conversation.cha
```

Inspect the output for its root, selected structure and recovery nodes.
This command does not run Chatter's model validation. Whitespace is explicit
in the grammar (`extras: []`), but not every source byte has a distinct named
node. Keep the original source alongside the CST. The companion
[`talkbank-parser`](../crates/talkbank-parser/README.md) lowers that evidence
into a typed model; see the [grammar architecture](GRAMMAR.md) and
[parser/model contracts](../book/src/architecture/parser-model-contracts.md).

## Bindings

The actively supported downstream binding is Rust (used by `talkbank-parser`).

Other language bindings in `bindings/` remain scaffold-level; their presence
does not establish packaging, editor integration or platform acceptance.

For Rust source-checkout usage, see the [binding guide](bindings/rust/README.md).

## Editor support

The `queries/` directory provides tree-sitter queries for:

- **Syntax highlighting** (`highlights.scm`)
- **Injections** (`injections.scm`)
- **Local scope symbols** (`locals.scm`)
- **Symbol tagging** (`tags.scm`)

These are the minimal boilerplate query files used by the default language bindings.

Additional editor query families and native-client behavior need separate
implementation and verification; query files are not an editor acceptance test.

## Documentation

- [Grammar architecture guide](https://github.com/TalkBank/chatter/blob/main/grammar/GRAMMAR.md): design patterns, precedence strategy, node type taxonomy, how to add new constructs
- [Grammar contributing guide](https://github.com/TalkBank/chatter/blob/main/grammar/CONTRIBUTING.md): development guide, spec-driven test generation, release steps
- [CHAT Manual](https://talkbank.org/0info/manuals/CHAT.docx): official CHAT format specification ([HTML](https://talkbank.org/0info/manuals/CHAT.html), [PDF](https://talkbank.org/0info/manuals/CHAT.pdf))

## Upstream sources of truth

This grammar lives inside the broader [`TalkBank/chatter`](https://github.com/TalkBank/chatter) repository.

- CHAT format specification: [talkbank.org/0info/manuals/CHAT.docx](https://talkbank.org/0info/manuals/CHAT.docx)
- Construct, error, and symbol specs: [`spec/`](https://github.com/TalkBank/chatter/tree/main/spec)

## Development

```bash
npm ci                         # Install dependencies (locked)
npx --no-install tree-sitter test           # Grammar corpus only
npx --no-install tree-sitter parse file.cha # Inspect syntax, not model validity
npm start                      # Launch the web playground
```

See the repository's [grammar contributing guide](https://github.com/TalkBank/chatter/blob/main/grammar/CONTRIBUTING.md) for the full development guide, including spec-driven test generation and release steps.

## Generated artifacts

Several files in this repository are generated and should not be edited by hand:

- **`src/parser.c`**, **`src/grammar.json`**, **`src/node-types.json`**: generated by `tree-sitter generate` from `grammar.js`
- **`src/generated_symbol_sets.js`**: symbol constants imported by `grammar.js`, generated from the [CHAT spec symbol registry](https://github.com/TalkBank/chatter/tree/main/spec/symbols)
- **`test/corpus/`**: test cases generated from CHAT format [specifications](https://github.com/TalkBank/chatter/tree/main/spec) via the `spec/tools` generator binaries

Grammar edits also affect generated node types, traversal consumers and other
derived artifacts. Use the consolidated `just regen` recipe from the repository
root, with prerequisites and review steps from
[Grammar Workflow](../book/src/contributing/grammar-workflow.md) and
[Spec Workflow](../book/src/contributing/spec-workflow.md). Then verify the
affected grammar/parser/model contracts. Running only `tree-sitter generate`
and `tree-sitter test` does not establish consumer currency or release readiness.

## License

MIT.
