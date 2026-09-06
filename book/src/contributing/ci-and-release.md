# CI and Release

**Status:** Current
**Last updated:** 2026-09-05 20:37 EDT

## Pre-Merge Verification

Run the shared local gate from [Developer Verification Checks](dev-checks.md):

```bash
just gate
```

This runs the checks used by per-push CI, including doctests, both Rust
workspaces, generated-artifact currency, and the book. Wait for GitHub Actions
on the exact pushed commit before announcing it as ready. Release-only checks
run separately through `just release-lint`.

## Generated artifact drift

After changing grammar, spec, or a registry, run `just regen`, then `just test`.
The regeneration recipe builds derived artifacts in dependency order; currency
tests detect stale output. Never hand-edit generated artifacts.

See [Spec Workflow](spec-workflow.md) and `spec/CLAUDE.md` for the current
source-of-truth guidance.

## Release Process

`TalkBank/chatter` is the public release source of truth. `release.yml`
(cargo-dist) creates the GitHub Release and CLI artifacts;
`release-desktop.yml` adds the desktop installers. Signing differs by platform
as described below.

### Cutting a release: the two-command procedure

The version literal lives in many places (the workspace version, every
internal path-dep pin, the desktop `package.json`, the CHANGELOG section),
and the tag is the release trigger, so both steps are mechanized and
fail-closed. Hand-editing version fields or tagging with raw `git tag` is
how releases break (v0.1.1 shipped a desktop version mismatch; v0.5.0
tagged a bump commit before its CI reported and the desktop build died on
drift CI would have caught). The procedure:

1. `just release-bump X.Y.Z` rewrites the canonical
   `[workspace.package] version`, every `path = "crates/…"` pin, and
   `package.json`, then refreshes both lockfiles (root + `spec/`).
2. Write the `## [X.Y.Z]` CHANGELOG section (the one deliberately manual
   step; every gate enforces its presence).
3. Format, run `just release-lint` and `just gate`, then squash the commits
   since the previous release tag into one release commit whose message is the
   CHANGELOG section. Verify the gate on the squashed tree and, with maintainer
   authorization, push and wait for CI on that commit. The content stamp survives
   a squash that leaves the checked bytes unchanged.
4. `just release-tag X.Y.Z` tags and pushes `vX.Y.Z`, refusing on a dirty
   tree, an unpushed HEAD, any version-copy drift, a missing CHANGELOG
   section, or CI/Cross-platform not yet green on the exact tagged commit.

After the tag: `release.yml` and `release-desktop.yml` build and publish;
verify the release page carries the CLI archives, the LSP standalone
artifacts, and the desktop installers before announcing.

### Workflows that actually exist in this repo

| Workflow | Purpose | Notes |
|----------|---------|-------|
| `.github/workflows/ci.yml` | Main build/test/book CI | Primary shared signal on pushes and PRs |
| `.github/workflows/cross-platform.yml` | Cross-platform build coverage | Supplements the main CI workflow |
| `.github/workflows/crates-io-foundation.yml` | First-wave crates.io readiness | Checks foundation-crate metadata, package surfaces, hold-backs, and publish order |
| `.github/workflows/release.yml` | cargo-dist release automation | Builds dist-enabled workspace artifacts from version tags; owns the GitHub Release |
| `.github/workflows/release-desktop.yml` | Desktop installer release automation | Builds chatter-desktop installers on the same version tags and uploads them into the release that `release.yml` creates; `workflow_dispatch` runs build-only |
| `.github/workflows/release-lint.yml` | Release-time lint | `just release-lint`: clippy over both workspaces plus the feature-off build. Runs on a version tag and on `workflow_dispatch`, never per push |
| `.github/workflows/clippy-rolling.yml` | New-stable clippy drift detection | Weekly maintenance workflow |

### Current release stance

- `release.yml` is about workspace artifact packaging via cargo-dist, not about
  crates.io publication.
- The first-wave crates.io path is documented separately in
  [Crates.io Publication](crates-io-publication.md) and is checked by
  `just crates-io-foundation-check` plus
  `.github/workflows/crates-io-foundation.yml`.

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

## The development loop

Set on 2026-08-27, after a single parser fix cost a day to the process
around it rather than to the fix.

1. **Inner loop:** `just test`. Write the failing test or the type change
   first, then make it green. `clippy` and `fmt` are run before a release,
   not per edit.
2. **After any change under `grammar/`, `spec/` or a registry:** `just
   regen`, then `just test`. Every derived artifact has a currency test, and
   they are far cheaper to satisfy together than one gate run at a time.
3. **Before committing:** review the final diff once.
4. **Before pushing:** `just gate`, once. It mirrors per-push CI exactly, so
   CI is a confirmation and never a discovery. The pre-push hook refuses a
   push without the stamp; the stamp hashes tree content, so a gate run on
   uncommitted changes stays valid once the same bytes are committed. Clippy
   and the feature-off build are `just release-lint`, run before a release.
5. **Releasing:** format, `just release-lint`, gate, squash every commit since the last tag into
   one release commit carrying the changelog section, gate once more, push,
   wait for CI, then `just release-tag`. Public history carries one commit
   per release.

Nothing on this path needs data that is not in the repository.
