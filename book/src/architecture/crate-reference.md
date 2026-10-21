# Crate Reference

**Status:** Current
**Last modified:** {{git-dates:page}}

This is a navigation guide to the main packages in Chatter. Package names are
not always directory or executable names. The manifests own exact dependencies,
features and publication metadata; this page does not certify registry
availability or stable pre-1.0 APIs.

## Shared libraries and grammar

| Package | Location | Responsibility |
| --- | --- | --- |
| `tree-sitter-talkbank` | `grammar/` | Generated grammar bindings, node metadata and queries |
| `talkbank-model` | `crates/talkbank-model/` | CHAT types, parser traits, validation, alignment, diagnostics, traversal and serialization |
| `talkbank-derive` | `crates/talkbank-derive/` | Procedural macros including semantic equality/diff, span shifting and validation metadata |
| `talkbank-parser` | `crates/talkbank-parser/` | Default tree-sitter frontend and source-bound CST-to-model lowering |
| `talkbank-parser-re2c` | `crates/talkbank-parser-re2c/` | Independent native frontend sharing the model API |
| `talkbank-transform` | `crates/talkbank-transform/` | Conversion, transformation, repair and validation-runner workflows |
| `talkbank-cache` | `crates/talkbank-cache/` | SQLite validation and roundtrip result storage |
| `talkbank-llm` | `crates/talkbank-llm/` | HTTP implementation of the transform judgment-provider interface |
| `send2clan` | `crates/send2clan/` | Platform bridge to the CLAN application |

The re2c parser is not an oracle for tree-sitter. It is selectable for supported
validation workflows, while tree-sitter remains the default and the LSP
backend. See [Parser Backends](parser-backends.md) for the 1.0 compatibility
contract and its limits.

The transform crate's default `validation-runner` feature enables its cache
and channel dependencies. Disabling that feature is an integration choice,
not a promise that every optional feature or target has been independently
qualified. Model-assisted HTTP work is explicit; review data-sharing authority
before supplying content to a configured endpoint.

## Applications

| Package | Location | Executable or surface |
| --- | --- | --- |
| `talkbank-chatter` | `crates/chatter/` | `chatter` CLI |
| `talkbank-lsp` | `crates/talkbank-lsp/` | `talkbank-lsp` server for LSP clients |
| `chatter-desktop` | `apps/chatter-desktop/src-tauri/` | Tauri application with the adjacent frontend |

The CLI exposes validation, conversion, normalization and other documented
CHAT workflows. It is not a corpus custody or publication authority.
The LSP uses primary-parser/model/grammar services directly, with source-bound
incremental analysis. Desktop and CLI share transform workflows, but their
presentation, installation and update acceptance are separate.
See the user guides for supported commands rather than deriving a command
inventory from a crate summary.

## Build, test and spec support

| Package | Location | Role |
| --- | --- | --- |
| `talkbank-build` | `crates/talkbank-build/` | Build-time deterministic source fingerprints; not cryptographic or whole-environment attestation |
| `talkbank-parser-tests` | `crates/talkbank-parser-tests/` | Shared test support, spec/reference contracts and maintenance tools |
| `xtask` | `xtask/` | Repository maintenance commands |
| `generators` | `spec/tools/` | Core spec generators |
| `spec-runtime-tools` | `spec/runtime-tools/` | Real-parser spec generation, validation and bootstrap tools |
| `talkbank-spec-vocabulary` | `spec/vocabulary/` | Shared vocabulary for spec tooling and test consumers |

Tests also live beside their owning libraries and applications; the parser-test
package is not the exclusive owner of every native equivalence, property or
roundtrip test. The spec packages form a separate Cargo workspace with deliberate
dependencies on root libraries. See
[Repository Architecture and Boundaries](repo-architecture.md) and
[Spec Workflow](../contributing/spec-workflow.md).

Typed CST generation uses the separately pinned generator described in
[Grammar Workflow](../contributing/grammar-workflow.md), not an additional
package silently vendored into this workspace. Ordinary builds consume the
committed output; regeneration and public distribution have their own
prerequisites.
