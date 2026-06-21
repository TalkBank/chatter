# CI and Release

**Status:** Current
**Last updated:** 2026-06-15 15:10 EDT

## Pre-Merge Verification

Use the concrete local verification commands from [Setup](setup.md) and
[Developer Verification Checks](dev-checks.md):

```bash
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo nextest run --workspace
cargo test --doc
```

Then rely on GitHub Actions CI as the authoritative shared signal before you
announce a change as ready.

## Generated artifact drift

Generated artifacts are still important, but the old root wrappers from the
predecessor workspace are not yet ported into this repo. In practice:

- regenerate only the affected spec/symbol outputs,
- do not hand-edit generated artifacts,
- and run the surface-specific verification commands that match the change.

See [Spec Workflow](spec-workflow.md) and `spec/CLAUDE.md` for the current
source-of-truth guidance.

## Release Process

This repository already contains release-oriented automation, but it is still a
**staging repo**. Do not describe `TalkBank/chatter` as the live public release
source of truth until the cutover from the predecessor repo actually happens.

### Workflows that actually exist in this repo

| Workflow | Purpose | Notes |
|----------|---------|-------|
| `.github/workflows/ci.yml` | Main build/test/book CI | Primary shared signal on pushes and PRs |
| `.github/workflows/cross-platform.yml` | Cross-platform build coverage | Supplements the main CI workflow |
| `.github/workflows/crates-io-foundation.yml` | First-wave crates.io readiness | Checks foundation-crate metadata, package surfaces, hold-backs, and publish order |
| `.github/workflows/release.yml` | cargo-dist release automation | Builds dist-enabled workspace artifacts from version tags; owns the GitHub Release |
| `.github/workflows/release-desktop.yml` | Desktop installer release automation | Builds chatter-desktop installers on the same version tags and uploads them into the release that `release.yml` creates; `workflow_dispatch` runs build-only |
| `.github/workflows/clippy-rolling.yml` | New-stable clippy drift detection | Weekly maintenance workflow |

### Current release stance

- `release.yml` is about workspace artifact packaging via cargo-dist, not about
  crates.io publication.
- The first-wave crates.io path is documented separately in
  [Crates.io Publication](crates-io-publication.md) and is checked by
  `just crates-io-foundation-check` plus
  `.github/workflows/crates-io-foundation.yml`.
- Release docs must stay honest about this repo's staging status until the
  release-source cutover is complete.

### Desktop release workflow: how the two tag workflows compose

On a version tag, `release.yml` (cargo-dist) and `release-desktop.yml` run
in parallel. cargo-dist owns creating the GitHub Release and attaching the
CLI archives, checksums, and installer scripts; `release-desktop.yml` builds
the Tauri installers, then polls until the release exists and uploads its
installers into it. Two platform notes baked into the workflow:

- **macOS**: Tauri signs, notarizes, and staples the `.app`, but NOT the
  `.dmg` it wraps around it. The workflow therefore submits the `.dmg`
  itself to the notary service and staples it, then verifies `codesign`,
  `spctl`, and `stapler validate` on both artifacts. The signing identity is
  supplied via environment, never hardcoded in `tauri.conf.json`.
- **Windows / Linux**: artifacts are currently unsigned by decision; see
  `docs/strategy/distribution-and-signing.md` ("Decisions, 2026-06-12") and
  the SmartScreen guidance in the install docs.

### Release secrets (Actions secrets on this repository)

Required by the macOS jobs of `release-desktop.yml` (and by cargo-dist
macOS codesigning if `macos-sign` is enabled, which uses the separate
`CODESIGN_*` names documented in the strategy doc):

| Secret | Content |
|--------|---------|
| `APPLE_CERTIFICATE` | base64-encoded Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | password for the `.p12` |
| `APPLE_SIGNING_IDENTITY` | full identity string, `Developer ID Application: <Name> (<TEAMID>)` |
| `APPLE_API_KEY` | App Store Connect API key ID (notarization) |
| `APPLE_API_ISSUER` | App Store Connect issuer ID |
| `APPLE_API_KEY_CONTENT` | contents of the `AuthKey_*.p8` file |

Rotation: replacing the certificate or notary key means updating these
secrets and nothing else; no workflow edits are needed. A maintainer must
re-create all of them on any new repository (secrets do not transfer).
