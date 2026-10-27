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
The Rust support policy is current stable, with no older-compiler or fixed-MSRV
promise. Record the exact compiler and verify the candidate on that toolchain;
dependency minimum-version declarations are not Chatter compatibility evidence.
Supported platforms and post-1.0 API compatibility still need their separate
candidate-bound acceptance.

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

The CLI's long-version build stamp combines package version, its own `src/`
fingerprint and build-script time. It works from an extracted package without
Git and cannot borrow an enclosing repository's revision. Missing/unreadable
source or a failed clock read stops the build. This stamp is diagnostic, not a
cryptographic executable hash or a record of all dependency revisions; retain
artifact checksums and the source/toolchain receipt separately.

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
| `.github/workflows/crates-io-foundation.yml` | Runs the check on matching push/PR paths or manual dispatch; not every publication-package edit triggers it |

The workflow's current path filters omit direct edits under `talkbank-build`,
`send2clan`, `talkbank-llm`, `talkbank-lsp` and the CLI's `crates/chatter`
directories. A change confined to those paths can therefore skip this workflow.
Verify a successful run for the exact publication commit; use the existing manual
dispatch when needed rather than treating a missing run as success. Changing
workflow coverage is separate from this documented limitation.

The `just` recipe passes `--allow-dirty`; the bare script does not. That option
permits packaging uncommitted content, so a local success alone does not bind
the checked bytes to a commit. Before publication, reconcile the actual package
contents with the reviewed candidate and preserve unrelated work. Do not use
the option as a substitute for candidate review.

The readiness check enforces:

- required crates.io metadata (`repository`, `homepage`, `keywords`,
  `categories`, `readme`)
- readme-file existence
- package file enumeration for every selected crate via `cargo package --list`
- the selected runtime and build dependency graph
- unversioned path dependencies for selected crates' workspace dev-dependencies,
  avoiding retained registry edges to unpublished test helpers or test-only cycles
- `publish = false` guards on every workspace crate outside the publication set
- real `cargo publish --dry-run` checks for the standalone `talkbank-build`
  and `tree-sitter-talkbank` crates

The metadata-only mode uses locked Cargo metadata and reads README paths. It
does not validate assembled package contents or registry resolution and cannot
replace the full pre-publication check.

The transform package must contain `schema/chat-file.schema.json`, which its
runtime JSON validator embeds. `just schema-gen` writes that packaged artifact
and the workspace schema from one model-derived byte sequence; the currency test
checks both files and the embedded value. The package preflight refuses a missing
embedded artifact. No build may reach outside an extracted package to obtain it.

Package enumeration is not a build or a review of every packaged byte. Only the
two standalone packages receive dry-run build verification in this script;
later crates still need the checks described below. The metadata check also
does not establish registry-name ownership, credential authority, compiler compatibility
or successful installation.

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
  dependency checks before publication. These do not replace candidate builds
  and installation checks on current stable Rust. Publication does not introduce
  a fixed MSRV or older-compiler support promise.
- As each prerequisite crate lands on crates.io, rerun targeted
  `cargo publish --dry-run -p <crate>` checks for the later crates before
  publishing them.

This is a real limitation of the initial bootstrap wave, not a missing script.
If we later want full registry-resolution rehearsal before publication, that
requires a staging registry/local index strategy, not just another shell loop.

## Publication procedure

Before publishing anything, obtain maintainer authorization for the exact
candidate and publication scope. Passing checks or possessing credentials does
not supply that authorization.

1. Verify name availability for new packages and authorized ownership of existing
   packages, with the intended registry/account. A name lookup does not reserve
   a name or prove permission to publish it.
2. Run `just crates-io-foundation-check`.
3. Ensure `.github/workflows/crates-io-foundation.yml` and the main CI workflow are green on the commit you intend to publish.
4. Publish in the order in **Scope** above, waiting for the crates.io index to observe each crate before moving to the next. That list is the single documented order; do not omit build-only or CLI runtime dependencies.
5. After each prerequisite becomes visible on crates.io, rerun any newly-unblocked `cargo publish --dry-run -p <crate>` checks before the next publish step.

Example command shape:

```bash
cargo publish -p tree-sitter-talkbank --locked
```

Retain each package/version and its publication result. If Cargo times out while
waiting for index visibility, check whether that exact version was uploaded
before retrying or advancing the wave. The timeout does not undo the upload;
see [Cargo's publication sequence](https://doc.rust-lang.org/cargo/commands/cargo-publish.html#description).
After the wave, perform the published CLI/LSP installation and runtime checks
above. Publication success alone is not installation or release acceptance.

## Tagging policy

Do **not** use version tags to drive crates.io publication from this repo.
`.github/workflows/release.yml` is reserved for cargo-dist GitHub Releases of
dist-enabled artifacts. Crates.io publication remains a deliberate manual
maintainer flow.
