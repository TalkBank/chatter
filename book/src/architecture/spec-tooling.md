# Spec Tooling

**Status:** Current
**Last modified:** {{git-dates:page}}

What the generator crates ARE. For the spec system's contract, which is what
you need to write or change a spec, read
[Spec System](spec-system.md); for the procedure,
[Spec Workflow](../contributing/spec-workflow.md). This page covers only the
tooling and routes authoring and execution details to those owners.

## Three crates, one workspace

`spec/` is its own Cargo workspace. From the repository root, direct Cargo
commands select it with `--manifest-path spec/Cargo.toml`; the maintained
`just` recipes already do that. Commands run inside `spec/` can use its manifest
without that argument.

| Crate | Owns | Dependency boundary |
|---|---|---|
| `spec/vocabulary` | shared spec and registry schemas | Model-independent by default; `model-runtime` enables the typed rule-selection adapter |
| `spec/tools` (`generators`) | reading specs and emitting artifacts, including grammar CST extraction | Tree-sitter and the CHAT grammar; its package-selected build does not link the production model or parser |
| `spec/runtime-tools` | live model conversion, diagnostic observation and runtime validation | Yes, including `talkbank-parser` and `talkbank-model` |

The split separates live diagnostic observation from model-independent rendering.
`spec/tools` reads markdown and JSON and produces
tests, fixtures, docs and generated Rust. It also parses wrapped CHAT examples
with Tree-sitter to extract CSTs. Work needing the production typed model or
diagnostics (verifying a spec example emits its codes, for example) lives in
`spec/runtime-tools`. Grammar parsing and semantic validation are distinct.

The artifact registry is split along the same line, and for the same reason:
`generators::artifacts::BOOTSTRAP_ARTIFACTS` owns the generated `ErrorCode` and
`DiagnosticKind` sources. `just model-sources-gen` selects only `generators`,
without the vocabulary's model feature, and writes these before runtime
consumers compile. Selecting all workspace packages would unify features and
defeat that build boundary.

The remaining `ARTIFACTS` include the tree-sitter corpus, which consumes the
observation snapshot. `just spec-gen` first bootstraps, then Cargo builds the
runtime writer. That writer refuses stale bootstrap sources before writing
anything; it writes the snapshot, dependent spec artifacts and the book table,
never the model sources already compiled into itself. Direct invocation of an
old executable is not a substitute for the recipe's rebuild boundary.
The table included in [the spec-system chapter](spec-system.md) describes all
registered artifacts, including the separately executed bootstrap stage.

`just regen` also orders symbols before grammar generation, form-marker and
model sources before compiled consumers, and traversal/node types before schema
and runtime tooling. Its dependencies execute once per invocation. Artifact
currency alone does not establish independent-machine bootstrap or semantics.

## Layout of `spec/tools`

```text
src/
  bin/          one binary per generator
  spec/         markdown spec loaders (constructs, errors)
  output/       formatters (tree-sitter corpus, Rust tests, docs)
  form_markers/ the form-marker registry: typed model, renderers, drift gate
  generated/    generated symbol sets (never edited by hand)
templates/      Tera templates wrapping fragments into whole CHAT files
```

## Determinism, and what enforces it

Generation is expected to be idempotent for unchanged inputs and toolchain.
The following mechanisms support that requirement; they do not establish
correct dependency ordering or semantic correctness by themselves.

- **The shared artifact writer compares bytes before writing**, preserving
  unchanged files. Named-file and whole-directory ownership have different
  retirement/pruning rules; see the spec-system chapter. This is not an atomic
  transaction across all artifacts: a later failure can leave earlier writes.
- **Rust renderers use the shared formatting helper**, which runs `rustfmt`.
  Otherwise `just fmt` and the generator each rewrite the same bytes forever,
  both correct. Toolchain changes can affect the rendered bytes.
- **Drift checks compare registered artifacts against what the generators
  produce**, calling the real generators rather than a second description of
  their output. See [Spec System](spec-system.md) for the full list.
