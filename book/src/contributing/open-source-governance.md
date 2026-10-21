# GitHub Readiness and Open Source Governance

**Status:** Current
**Last modified:** {{git-dates:page}}

## Objective
Prepare `TalkBank/chatter` to operate as a healthy public project with clear legal, security,
contribution, and release processes.

## Root Artifacts

| Artifact | Status | Notes |
|----------|--------|-------|
| `LICENSE-MIT` + `LICENSE-APACHE` | Done | Dual-licensed `MIT OR Apache-2.0` (standard Rust convention; both files present at root, no combined `LICENSE`). Every crate inherits `license = "MIT OR Apache-2.0"` from `[workspace.package]`. |
| `CONTRIBUTING.md` | Present | Routes to maintained setup and verification guides; independent onboarding remains an acceptance task |
| `CODE_OF_CONDUCT.md` | **TODO (deferred)** | Intentionally absent for now: it is held until a durable enforcement contact (an institutional address or successor handle, not an individual) is settled. The plan is to adopt the Contributor Covenant once that contact exists. |
| `SECURITY.md` | Present | Root policy and issue-template link use private vulnerability reporting; maintainers must enable and verify that GitHub reporting route |
| `CODEOWNERS` | **TODO** | Not added yet: repo contents do not currently publish an authoritative GitHub owner/team map for path-level review ownership |
| `.github/workflows/*.yml` | Done | `ci.yml` (Rust build+test, mdBook) + `cross-platform.yml` (OS matrix) + `release-lint.yml` (clippy + feature-off, on a tag) + `clippy-rolling.yml` + `crates-io-foundation.yml` + `release.yml` + `release-desktop.yml` |
| `.github/ISSUE_TEMPLATE/*` | Done | Bug report + feature request (YAML forms) |
| Pull request template | Done | `.github/PULL_REQUEST_TEMPLATE.md` mirrors current CONTRIBUTING + PR review requirements |

## CI Governance Policy

- [Branch Protection](branch-protection.md) describes the configured
  protection for `main`: which `ci.yml` jobs are required checks, which run
  without blocking a merge, and why `cross-platform.yml` is not a PR check. Do
  not maintain a second list here.
- Editing that page does not configure GitHub. When a maintainer changes the
  live settings, update that page in the same change; a green CI run alone
  does not establish which failed checks block merges.

## Release Governance

- Releases include CLI/LSP artifacts and desktop installers. Signing differs
  by platform, and updater signing is distinct from installer signing; see
  [CI and Release](ci-and-release.md). Do not describe every artifact as signed
  or infer registry availability from a GitHub release.
- Cargo publication governance: first-wave crates.io foundations are documented
  in [Crates.io Publication](crates-io-publication.md) and checked by
  `.github/workflows/crates-io-foundation.yml`.
- Binary release governance: `release.yml` is reserved for cargo-dist GitHub
  Release packaging of dist-enabled artifacts. It is **not** the crates.io
  publication workflow.
- Tagging rule: do not treat version tags as authorization to publish new
  surfaces. A surface becomes stable only when its release notes explicitly say
  so and its public distribution channel is live.
- Release-note rule: every public release note must state the surface's
  distribution channel, support boundary, and any closely related surfaces that
  remain held back.

## Community Operations

- Label taxonomy: `bug` and `enhancement` auto-applied by issue templates. Richer taxonomy (`drift`, `spec`, `grammar`, `parser`, `docs`, `good first issue`): **TODO** (GitHub settings).
- Contributor pathway: start with [Setup](setup.md), then
  [Developer Verification Checks](dev-checks.md). The guides are maintained;
  an independent contributor drill is still needed to establish onboarding.
- Public release conditions: [Toward Chatter 1.0](one-zero-readiness.md).
  It is a readiness contract, not a promised release date.

## Supply Chain and Security

- Dependency policy: CI runs `cargo-deny` through `just deps-check`, with
  `deny.toml`. Automated update PRs (Dependabot/Renovate): **TODO**.
- Release signing: follow the platform and credential boundaries in
  [CI and Release](ci-and-release.md); verify the actual candidate artifacts.
- Security advisories: `SECURITY.md` owns the policy. It relies on GitHub
  private vulnerability reporting, which is a repository setting; a policy
  file cannot enable it. Keep the setting on while `SECURITY.md` routes
  reporters there.

## Acceptance Criteria
- Repo has complete governance artifacts at root.
- CI and branch protections enforce stated policy.
- Contributors can onboard and submit PRs without tribal knowledge.
- Release/support tiers are documented per surface.
- Release process is repeatable and documented.
