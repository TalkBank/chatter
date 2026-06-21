# Branch Protection and Required CI Checks

**Status:** Current
**Last updated:** 2026-06-15 15:00 EDT

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
workflow runs on every pull request to `main`:
- `Rust build + test`
- `mdBook build`
- `Rust version pins in sync`

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
