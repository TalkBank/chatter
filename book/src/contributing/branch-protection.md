# Branch Protection and Required CI Checks

**Status:** Current
**Last updated:** 2026-07-26 20:04 EDT

This page defines the required status checks and protection policy for `main`.

## Branch Protection Policy
Enable branch protection for `main` with:
- Require pull request before merge.
- Require approvals (minimum 1; maintainers may set higher).
- Require conversation resolution before merge.
- Require status checks to pass before merge.
- Restrict force pushes and branch deletions.

## Required Status Checks
Configure these CI checks as required. The names are the GitHub check names,
which come from each job's `name:` in `.github/workflows/ci.yml`; that
workflow runs on every pull request to `main`. This is every job that
workflow defines, so the required set and the workflow do not drift apart:

- `Rust build + test`
- `wasm32 check (model + re2c parser)`
- `mdBook build`
- `Rust version pins in sync`
- `App version in sync`
- `Shell scripts (shellcheck, strictest)`
- `Grammar (generate staleness, tree-sitter test, queries)`
- `Dependency policy (cargo-deny)`

This list had drifted: until 2026-07-26 it named only the first, third and
fourth, having been written before the wasm, app-version-sync and shellcheck
jobs existed. A required-check list that silently omits jobs is worse than no
list, because it reads as a deliberate selection rather than an oversight.
**When you add a job to `ci.yml`, add it here in the same commit.**

Note that this page states the INTENDED required set; the live setting lives
in the repository's branch-protection configuration on GitHub and is changed
there by a maintainer, not by editing this file.

One other workflow is deliberately NOT in the required set:
- `cross-platform.yml` (the Ubuntu + macOS + Windows matrix) runs on push to
  `main`, a daily schedule, and manual dispatch, NOT on pull requests, so it
  cannot report a status on a PR and must not be required (requiring it would
  block every merge). It is a post-merge and daily drift gate. Add a
  `pull_request` trigger first if you want it required.

## Optional Hardening
- Require branches to be up to date before merging.
- Enable merge queue if PR volume increases.
- Restrict who can dismiss stale reviews.

## Operational Rule
If required checks fail:
- Do not bypass protection.
- Fix the issue or revert the breaking change.
- Re-run checks until green.
