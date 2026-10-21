# Contributing to tree-sitter-talkbank

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/CONTRIBUTING.md)

This guide covers modifying and testing the CHAT grammar: fixing a bug, adding
a CHAT construct or improving editor queries. The grammar lives inside the
Chatter workspace, so the repository's own workflow applies. The canonical
procedure is the book's [Grammar Workflow](../book/src/contributing/grammar-workflow.md);
[grammar agent guidance](AGENTS.md) records the same ownership rules for
automated contributors. This page is the short route through them.

## Prerequisites

- **Node.js** at the version in `grammar/.nvmrc` (`package.json` declares the
  supported range). `npm ci` in `grammar/` installs the locked
  `tree-sitter-cli`; make `grammar/node_modules/.bin` available to recipes
  that invoke `tree-sitter`.
- **Rust** (current stable) and **`just`**, for the repository recipes and the
  Rust binding.
- **Typed traversal regeneration** additionally needs Python 3.11 or later and
  a clean maintainer-provided `tree-sitter-grammar-utils` checkout at the
  revision pinned in `tsgu-revision`. Its source is not yet publicly
  available; see [Setup](../book/src/contributing/setup.md) and the Grammar
  Workflow.

## Quick start

```bash
cd grammar && npm ci          # Locked tree-sitter-cli and dependencies
npx tree-sitter parse file.cha  # Parse a CHAT file and inspect the tree
npm start                     # Launch the web playground
```

## Grammar modification workflow

1. **Edit `grammar.js`**, the single source of the executable syntax.
2. **Regenerate**, from the repository root:
   ```bash
   just regen
   ```
   This regenerates the parser (`src/parser.c`, `src/grammar.json`,
   `src/node-types.json`) and every artifact derived from it, in dependency
   order: node-kind constants, typed traversal and C metadata bridge,
   conformance inventory, spec-derived tests and audits. Running
   `tree-sitter generate` alone leaves those consumers stale.
3. **Test the grammar**:
   ```bash
   just grammar-test
   ```
   This runs `tree-sitter test` over `test/corpus/` and checks that every
   editor query compiles. `tree-sitter test` cannot detect a stale
   `src/parser.c`; `just grammar-generate-check` regenerates in staging and
   reports stale grammar artifacts without rewriting them.
4. **Review the generated changes** and run the affected parser, equivalence
   and roundtrip contracts, as the Grammar Workflow describes. Parse real files
   to see the effect on real data:
   ```bash
   npx tree-sitter parse path/to/file.cha
   ```

## Generated files: what not to edit

Never hand-edit a generated file; change its owner and regenerate. The owners
are:

| File | Owner |
|------|-------|
| `src/parser.c`, `src/grammar.json`, `src/node-types.json` | `grammar.js`, through tree-sitter generation in `just regen` |
| `src/generated_symbol_sets.js` | `spec/symbols/symbol_registry.json`, through `just symbols-gen` |
| `test/corpus/generated/` | authored specs under `spec/constructs/` and `spec/errors/`, through `just spec-gen` |
| typed traversal and `bindings/rust/tsgu_language_metadata.c` | the pinned traversal recipe (`just traversal-gen`) |

[Grammar agent guidance](AGENTS.md#source-and-output-ownership) lists the
remaining generated consumers outside `grammar/`.

## Testing

### Test corpus structure

```
test/corpus/
├── generated/      # Generated from spec/ by `just spec-gen`; do not edit
│   ├── errors/
│   ├── header/
│   ├── main_tier/
│   ├── tiers/
│   ├── utterance/
│   └── word/
└── manual/         # Hand-maintained grammar tests; no generator owns these
```

Each `.txt` file uses tree-sitter's corpus format. A generated header test,
for example:

```
================================================================================
activities_simple - @Activities header with simple free-text content.
================================================================================
@Activities:	play

--------------------------------------------------------------------------------

(source_file (activities_header (activities_prefix) (header_sep (colon) (tab)) (free_text (rest_of_line)) (newline)))
```

The root is always `source_file`; a complete file appears under
`(source_file (full_document ...))`.

### Spec-driven tests (generated)

Most corpus tests are generated from the repository's specifications:

- **Construct specs** (`spec/constructs/`) define valid CHAT examples and
  their expected trees.
- **Error specs** (`spec/errors/`) define invalid examples with claims.

```bash
just spec-gen      # Regenerate every artifact derived from spec/
just spec-check    # Report whether the committed copies are current
```

The [Spec Workflow](../book/src/contributing/spec-workflow.md) explains
authoring and observation review. Regeneration is not approval of new output:
review changed observations against the intended rule.

### Adding test cases manually

For grammar behavior no spec covers yet, add a test under `test/corpus/manual/`.
Never put authored work under `test/corpus/generated/`; the spec generator owns
that directory and prunes files it did not produce. If the behavior is a
lasting CHAT contract, add a spec as well.

### Running specific tests

```bash
npx tree-sitter test --include "header"      # Cases whose name matches a regex
npx tree-sitter test --file-name media_filename.txt  # Cases from one corpus file
```

### Rust binding

```bash
cargo test -p tree-sitter-talkbank
```

This checks the local Rust binding. It is not registry, platform or other
binding qualification.

## Editor queries

The `queries/` directory provides the minimal query set expected by the
language bindings:

| File | Purpose |
|------|---------|
| `highlights.scm` | Syntax highlighting |
| `injections.scm` | Language injection hooks |
| `locals.scm` | Speaker definitions and references |
| `tags.scm` | Outline and navigation symbols |

`just grammar-test` checks that each query compiles; it does not verify
highlight results. Inspect highlighting in the playground (`npm start`). When a
rule is added, removed or renamed, check whether a query names it. Additional
query families (folds, indents, text objects) are post-v1 work.

## Consumer impact

Adding, removing or renaming a node type, field or child position changes the
generated traversal, the node-kind constants, the conformance inventory and the
typed lowering in `talkbank-parser` that consumes them. Run `just regen`, then
migrate the consumers in the same change. A change to precedence or conflicts
can also change which tree a given input produces even when the node types are
unchanged, so review the spec observations and parser contracts either way.

## Symbol sets

`src/generated_symbol_sets.js` exports the character classes `grammar.js`
uses: paired stretch symbols, word-attached markers, and the characters that
cannot start or continue a word segment or an event segment. They are named
for the parse role they hold, not for provenance; some word-attached marks are
disfluency notation rather than CA notation, while the `ca_element` and
`ca_delimiter` node names are unchanged. The file is generated from
`spec/symbols/symbol_registry.json`:

```bash
just symbols-gen
```

## Releases

The grammar is published as the `tree-sitter-talkbank` crate in the workspace's
crates.io publication order. Publication is a deliberate maintainer action with
its own authority, not part of grammar development; follow
[Crates.io Publication](../book/src/contributing/crates-io-publication.md).
The book has no maintained npm publication procedure; do not publish the npm
package from this guide.

## Code style

- `grammar.js` uses section comments with CHAT manual references.
- Reference links use the format `https://talkbank.org/0info/manuals/CHAT.html#Main_Line`.
- Prefer concrete construct anchors over broad aliases (for example
  `#File_Headers` over `#Headers`).
- Use `token()` for lexer-level rules and `prec()` for disambiguation; record a
  non-obvious choice in [precedence decisions](docs/precedence-decisions.md).
- Keep `extras: []`; CHAT whitespace is always explicit.
