# GitHub Readiness and Open Source Governance

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

## Objective
Prepare `TalkBank/chatter` to operate as a healthy public project with clear legal, security,
contribution, and release processes.

## Root Artifacts

| Artifact | Status | Notes |
|----------|--------|-------|
| `LICENSE-MIT` + `LICENSE-APACHE` | Done | Dual-licensed `MIT OR Apache-2.0` (standard Rust convention; both files present at root, no combined `LICENSE`). Every crate inherits `license = "MIT OR Apache-2.0"` from `[workspace.package]`. |
| `CONTRIBUTING.md` | Done | Setup, standards, PR flow, pre-PR checklist |
| `CODE_OF_CONDUCT.md` | **TODO (deferred)** | Intentionally absent for now: it is held until a durable enforcement contact (an institutional address or successor handle, not an individual) is settled. The plan is to adopt the Contributor Covenant once that contact exists. |
| `SECURITY.md` | Done | Root file added; issue-template contact link now resolves to a real policy |
| `CODEOWNERS` | **TODO** | Not added yet: repo contents do not currently publish an authoritative GitHub owner/team map for path-level review ownership |
| `.github/workflows/*.yml` | Done | `ci.yml` (Rust build+test, mdBook, Rust-version-sync) + `cross-platform.yml` (OS matrix) + `clippy-rolling.yml` + `crates-io-foundation.yml` + `release.yml` + `release-desktop.yml` |
| `.github/ISSUE_TEMPLATE/*` | Done | Bug report + feature request (YAML forms) |
| Pull request template | Done | `.github/PULL_REQUEST_TEMPLATE.md` mirrors current CONTRIBUTING + PR review requirements |

## CI Governance Policy

- Required status checks: the `ci.yml` jobs that run on every pull request,
  `Rust build + test`, `mdBook build`, and `Rust version pins in sync`. See
  [Branch Protection](branch-protection.md) for the exact GitHub check names
  and which other workflow (`cross-platform.yml`) is deliberately not in the
  required set.
- Branch protection rules: documented in
  [Branch Protection](branch-protection.md); configure on GitHub once the
  repo is public.

## Release Governance

- Support/stability contract: documented in
  [Support and Stability Tiers](support-tiers.md). No surface in this repo is
  currently stable; the repo is still in the staging phase.
- Cargo publication governance: first-wave crates.io foundations are documented
  in [Crates.io Publication](crates-io-publication.md) and checked by
  `.github/workflows/crates-io-foundation.yml`.
- Binary release governance: `release.yml` is reserved for cargo-dist GitHub
  Release packaging of dist-enabled artifacts. It is **not** the crates.io
  publication workflow.
- Tagging rule: do not treat version tags as authorization to publish new
  surfaces. A surface becomes stable only when its release notes explicitly say
  so and its public distribution channel is live.
- Release-note rule: every public release note must state the surface's tier,
  distribution channel, support boundary, and any closely related surfaces that
  remain held back.

## Community Operations

- Label taxonomy: `bug` and `enhancement` auto-applied by issue templates. Richer taxonomy (`drift`, `spec`, `grammar`, `parser`, `docs`, `good first issue`): **TODO** (GitHub settings).
- Contributor pathway: `CONTRIBUTING.md` covers setup and PR flow. First-time/advanced contributor pathways: **TODO**.
- Public project roadmap: **TODO**.

## Supply Chain and Security

- Dependency scanning: CI runs `rustsec/audit-check` and `cargo-deny` (with `deny.toml`). Automated update PRs (Dependabot/Renovate): **TODO**.
- Signed release artifacts: **TODO**.
- Security advisories process: documented in `SECURITY.md`.

## Acceptance Criteria
- Repo has complete governance artifacts at root.
- CI and branch protections enforce stated policy.
- Contributors can onboard and submit PRs without tribal knowledge.
- Release/support tiers are documented per surface.
- Release process is repeatable and documented.
