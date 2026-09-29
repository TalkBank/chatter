# Crates.io Publication

**Status:** Current
**Last updated:** 2026-09-28 20:59 EDT

## Scope

The 1.0 publication contract includes foundation libraries and registry-installable
CLI/LSP binaries. Publication is a deliberate maintainer action, not a
tag-triggered release path. The existing foundation-named check now covers the
complete dependency closure below; enabling publication is not proof of readiness.

The publication order is:

1. `talkbank-build`
2. `tree-sitter-talkbank`
3. `talkbank-derive`
4. `talkbank-model`
5. `talkbank-cache`
6. `talkbank-parser`
7. `talkbank-parser-re2c`
8. `talkbank-transform`
9. `send2clan`
10. `talkbank-llm`
11. `talkbank-lsp`
12. `chatter`

`talkbank-build` is build-only support for the model and parser source
fingerprints and must be published before those consumers.

`talkbank-parser-re2c` is included because
`talkbank-transform` has a **runtime dependency** on it. Holding it back would
make `talkbank-transform` unpublishable. Inclusion in the dependency closure
does not promise parser equivalence: re2c remains experimental.

The CLI additionally requires `send2clan` and `talkbank-llm` at runtime. They
must be published before it; optional Cargo dependencies would still require
registry resolution and are not a way to hide unpublished packages.

Every workspace package outside this publication set must be explicitly marked
`publish = false`. The check derives this complement from Cargo metadata,
so a newly added crate cannot silently escape the publication decision.
Internal test, vocabulary, desktop and task-runner packages remain held back;
the script prints the complete current set. CLI/LSP binary releases and desktop
installers remain required alongside registry installation.

Before declaring registry installation supported, verify the actual published
candidate with `cargo install chatter --locked` and
`cargo install talkbank-lsp --locked`, including CLI validation and LSP protocol
smoke tests. These are acceptance targets, not a claim that current registry
versions are available. MSRV, supported platforms and post-1.0 compatibility
policy still require explicit decisions and candidate-bound verification.

## What the repo now automates

The existing foundation-named entry points cover the full publication set:

| Surface | Purpose |
|---------|---------|
| `just crates-io-foundation-check` | Local preflight for crates.io readiness |
| `bash scripts/release/check-foundation-publication-readiness.sh --metadata-only` | Fast manifest, dependency and hold-back review without packaging or registry access |
| `.github/workflows/crates-io-foundation.yml` | CI enforcement for metadata, package surfaces, hold-backs, and publish order |

The readiness check enforces:

- required crates.io metadata (`repository`, `homepage`, `keywords`,
  `categories`, `readme`)
- readme-file existence
- package file enumeration for every selected crate via `cargo package --list`
- the selected runtime and build dependency graph
- `publish = false` guards on every workspace crate outside the publication set
- real `cargo publish --dry-run` checks for the standalone `talkbank-build`
  and `tree-sitter-talkbank` crates

The metadata-only mode uses locked Cargo metadata and reads README paths. It
does not validate assembled package contents or registry resolution and cannot
replace the full pre-publication check.

## Important limitation: Cargo cannot fully dry-run the bootstrap wave

For the first publication of an interdependent workspace, `cargo publish
--dry-run` is **not** a complete CI gate for every crate. Cargo rewrites path
dependencies to registry dependencies while preparing the package. That means a
crate such as `talkbank-model` cannot complete a registry-style dry-run until
its prerequisite `talkbank-derive` already exists on crates.io.

So the current automation is intentionally honest:

- `talkbank-build` and `tree-sitter-talkbank` get real crates.io dry-runs
  because neither depends on an unpublished workspace crate.
- The remaining selected crates are validated by metadata, readme, and
  dependency checks before publication. (No MSRV is declared yet; set a
  deliberate `rust-version` and re-add an MSRV check when publication is
  actually pursued.)
- As each prerequisite crate lands on crates.io, rerun targeted
  `cargo publish --dry-run -p <crate>` checks for the later crates before
  publishing them.

This is a real limitation of the initial bootstrap wave, not a missing script.
If we later want full registry-resolution rehearsal before publication, that
requires a staging registry/local index strategy, not just another shell loop.

## Publication procedure

Before publishing anything:

1. Verify crates.io name availability for every selected package.
2. Run `just crates-io-foundation-check`.
3. Ensure `.github/workflows/crates-io-foundation.yml` and the main CI workflow are green on the commit you intend to publish.
4. Publish in the order in **Scope** above, waiting for the crates.io index to observe each crate before moving to the next. That list is the single documented order; do not omit build-only or CLI runtime dependencies.
5. After each prerequisite becomes visible on crates.io, rerun any newly-unblocked `cargo publish --dry-run -p <crate>` checks before the next publish step.

Example command shape:

```bash
cargo publish -p tree-sitter-talkbank --locked
```

## Tagging policy

Do **not** use version tags to drive crates.io publication from this repo.
`.github/workflows/release.yml` is reserved for cargo-dist GitHub Releases of
dist-enabled artifacts. Crates.io publication remains a deliberate manual
maintainer flow.
