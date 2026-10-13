# Crates.io Publication

**Status:** Current
**Last updated:** {{git-dates:page}}

## Scope

The 1.0 publication contract includes foundation libraries and registry-installable
CLI/LSP binaries. Publication is a deliberate maintainer action, not a
tag-triggered release path. The foundation-named check covers the
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
12. `talkbank-chatter`

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

The registry already contains an unrelated
[`chatter` 0.1.0 package](https://docs.rs/crate/chatter/0.1.0/source/Cargo.toml.orig),
which translates dialogue scripts into Rust. `cargo install chatter` is therefore
not an installation instruction for the TalkBank CLI. Our workspace package is
`talkbank-chatter`; its executable remains `chatter`. This package-name choice
does not reserve the registry name or authorize bootstrap publication.

Before declaring registry installation supported, verify the actual published
candidate with `cargo install talkbank-chatter --version <candidate> --locked`,
including CLI validation and LSP protocol smoke tests. The LSP target
remains `cargo install talkbank-lsp --version <candidate> --locked`. These are
acceptance targets, not a claim that current registry versions are available.
MSRV, supported platforms and post-1.0 compatibility policy still require
explicit decisions and candidate-bound verification.

Local source-install verification can reuse the existing process contracts
against a selected installed executable, without changing their default
Cargo-produced program or CI selection:

```bash
TALKBANK_CHATTER_TEST_BINARY=/path/to/test-install/bin/chatter \
  cargo test -p talkbank-chatter --test integration cache_identity_recovery:: -- --nocapture
TALKBANK_LSP_TEST_BINARY=/path/to/test-install/bin/talkbank-lsp \
  cargo test -p talkbank-lsp --test position_conversion_tests stdio_lifecycle:: -- --nocapture
```

Explicit paths must be absolute regular files; invalid or missing selections
fail rather than falling back to a workspace binary. The operating system still
checks executability at spawn. Record the checkout, build profile/toolchain,
installed-program hashes and actual successful runtime names. These local
contracts cover scoped cache recovery and LSP lifecycle/diagnostic/edit behavior,
not registry installation, released upgrades or every CLI/LSP feature. They do
not replace the published-candidate installation requirement above.

Binary distribution follows the new package identity: cargo-dist produces
`talkbank-chatter-installer.sh` and `.ps1` and records `talkbank-chatter` receipts.
The publication prerequisite retains `chatter-installer.sh` and `.ps1` wrappers
for existing download links and older self-updaters. They preserve the generated
installer payload and translate legacy `CHATTER_*` settings using cargo-dist's
environment mapping; explicit `TALKBANK_CHATTER_*` settings take precedence.
Uploaded wrappers are checked against the payload and mapping before announcement.
New binaries load current receipts first, fall back to legacy `chatter` receipts
only when the current receipt is absent, and target the new release identity. Receipts for
another executable or release source are refused. Actual released upgrade
acceptance remains required; local compatibility checks do not certify a release.

The local boundary checks are:

```bash
cargo test -p talkbank-chatter --test integration update_command_tests
node --test scripts/release/cli-installer-compatibility.test.mjs scripts/release/verify-desktop-candidate.test.mjs
```

They cover receipt refusal, legacy shell settings and wrapper/payload integrity;
native Windows execution
and actual released upgrades remain separate acceptance obligations.

## What the repo automates

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
