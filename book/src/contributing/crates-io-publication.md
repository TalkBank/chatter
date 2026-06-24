# Crates.io Publication

**Status:** Current
**Last updated:** 2026-06-21 21:33 EDT

## Scope

The crates.io automation in this repo currently targets the **Wave 1A foundation
crates only**. crates.io publication is a deliberate maintainer action, not a
tag-triggered release path.

Wave 1A is:

1. `tree-sitter-talkbank`
2. `talkbank-derive`
3. `talkbank-model`
4. `talkbank-cache`
5. `talkbank-parser`
6. `talkbank-parser-re2c`
7. `talkbank-transform`

`talkbank-parser-re2c` is part of the first wave because
`talkbank-transform` has a **runtime dependency** on it. Holding it back would
make `talkbank-transform` unpublishable.

The current Wave 1B hold-backs are explicitly marked `publish = false`:

- `send2clan`
- `chatter`
- `talkbank-lsp`

They stay blocked until their support contract, install story, and user-facing
docs are ready.

## What the repo now automates

Two repo-native entry points cover the first-wave foundations:

| Surface | Purpose |
|---------|---------|
| `just crates-io-foundation-check` | Local preflight for first-wave crates.io readiness |
| `.github/workflows/crates-io-foundation.yml` | CI enforcement for first-wave metadata, package surfaces, hold-backs, and publish order |

The readiness check enforces:

- required crates.io metadata (`repository`, `homepage`, `keywords`,
  `categories`, `readme`)
- readme-file existence
- package assembly for every first-wave crate via `cargo package --list`
- the first-wave runtime dependency graph
- `publish = false` guards on Wave 1B crates
- a real `cargo publish --dry-run` for the standalone `tree-sitter-talkbank`
  crate

## Important limitation: Cargo cannot fully dry-run the bootstrap wave

For the first publication of an interdependent workspace, `cargo publish
--dry-run` is **not** a complete CI gate for every crate. Cargo rewrites path
dependencies to registry dependencies while preparing the package. That means a
crate such as `talkbank-model` cannot complete a registry-style dry-run until
its prerequisite `talkbank-derive` already exists on crates.io.

So the current automation is intentionally honest:

- `tree-sitter-talkbank` gets a real crates.io dry-run because it stands alone.
- The remaining Wave 1A crates are validated by metadata, readme, and
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

1. Verify crates.io name availability for every Wave 1A package.
2. Run `just crates-io-foundation-check`.
3. Ensure `.github/workflows/crates-io-foundation.yml` and the main CI workflow are green on the commit you intend to publish.
4. Publish in this exact order, waiting for the crates.io index to observe each crate before moving to the next:
   - `tree-sitter-talkbank`
   - `talkbank-derive`
   - `talkbank-model`
   - `talkbank-cache`
   - `talkbank-parser`
   - `talkbank-parser-re2c`
   - `talkbank-transform`
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
