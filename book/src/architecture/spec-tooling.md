# Spec Tooling

**Status:** Current
**Last modified:** 2026-08-16 12:39 EDT

What the generator crates ARE. For the spec system's contract, which is what
you need to write or change a spec, read
[Spec System](spec-system.md); for the procedure,
[Spec Workflow](../contributing/spec-workflow.md). This page covers only the
tooling, so the three do not overlap.

## Two crates, one workspace

`spec/` is its own cargo workspace, so every command needs
`--manifest-path spec/Cargo.toml`.

| Crate | Owns | Depends on the parser? |
|---|---|---|
| `spec/tools` (`generators`) | reading specs and emitting artifacts | **No** |
| `spec/runtime-tools` | anything needing the live parser or model | Yes |

That split is the point. `spec/tools` reads markdown and JSON and produces
tests, fixtures, docs and generated Rust; it never parses CHAT. Work that has to
actually run the parser (verifying a spec example emits its codes, mining the
corpus) lives in `spec/runtime-tools`.

The artifact registry is split along the same line, and for the same reason:
`generators::artifacts::ARTIFACTS` holds everything derivable from markdown
alone, and `spec_runtime_tools::artifacts::RUNTIME_ARTIFACTS` holds the one
artifact that has to enumerate the live `ErrorCode` enum. `spec_gen` runs both,
so a contributor sees one command and one list.

## Layout of `spec/tools`

```text
src/
  bin/          one binary per generator
  spec/         markdown spec loaders (constructs, errors)
  output/       formatters (tree-sitter corpus, Rust tests, docs)
  form_markers/ the form-marker registry: typed model, renderers, drift gate
  templates/    Tera templates wrapping fragments into whole CHAT files
  generated/    generated symbol sets (never edited by hand)
```

## Determinism, and what enforces it

Generation must be idempotent: a re-run with no source change produces no diff.
Three things make that true rather than hoped for.

- **Generators write only when content differs**, so a no-op run does not churn
  mtimes.
- **Rust output is formatted by the generator**, which runs `rustfmt` itself.
  Otherwise `just fmt` and the generator each rewrite the same bytes forever,
  both correct. Both registries do this.
- **Drift gates compare committed artifacts against what the generators
  produce**, calling the real generators rather than a second description of
  their output. See [Spec System](spec-system.md) for the full list.

## History, so the next reader is not misled

This page used to describe a bootstrap-era pipeline and a set of proposals. All
of it was stale by mid-2026 and some of it was actively wrong:

- It referred to `make test-gen` as the standard reaction to a parser change.
  **There is no Makefile in this repository.** Run the `spec/tools` binaries
  directly, or the `just` recipes.
- It listed as an open concern that `spec/tools` "still carries bootstrap-era
  Rust parser/model dependencies". That was resolved by the
  `spec/runtime-tools` split; `spec/tools` depends on no parser or model crate.
- It prescribed per-spec metadata (ownership, `draft`/`accepted`/`deprecated`)
  that no loader has ever read. The real metadata, and what each field does, is
  in [Spec System](spec-system.md).
- It proposed an `input`/`ir`/`emit`/`validate`/`sync` module split that was
  never implemented, and a `spec lint` binary that does not exist.

Aspirations are worth writing down, but not in a page a contributor reads as a
description of the code.
