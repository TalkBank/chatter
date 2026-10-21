# Branch Protection and Required CI Checks

**Status:** Current
**Last updated:** {{git-dates:page}}

This page describes the branch-protection settings configured for `main` and
the CI checks that pull requests must pass. The live settings live in the
repository's GitHub configuration and are changed there by a maintainer;
editing this page changes nothing on GitHub.

## Branch Protection Settings
`main` is configured with:
- A pull request before merge, with at least one approving review; stale
  approvals are dismissed when new commits are pushed.
- Conversation resolution before merge.
- The required status checks below. Branches need not be up to date with
  `main` before merging.
- No force pushes and no branch deletion.

Administrators are not subject to these rules, so maintainers can push
directly to `main`. A maintainer push still runs every CI job afterwards.

## Required Status Checks
Three checks are required. The names are GitHub check names, taken from each
job's `name:` in `.github/workflows/ci.yml`:

- `Rust build + test`
- `mdBook build`
- `Rust version pins in sync`

Every other job in `ci.yml` also runs on every pull request to `main`; a red
result is a defect to fix before merging, but GitHub does not block the merge
on it:

- `wasm32 check (model + re2c parser)`
- `App version in sync`
- `Shell scripts (shellcheck, strictest)`
- `Grammar (generate staleness, tree-sitter test, queries)`
- `Dependency policy (cargo-deny)`

When you add a job to `ci.yml`, list it in one of these two groups in the same
commit. Making a job required is a settings change by a maintainer; update the
lists here when that setting changes.

One other workflow is deliberately NOT in the required set:
- `cross-platform.yml` (the Ubuntu + macOS + Windows matrix) runs on push to
  `main`, a daily schedule, and manual dispatch, NOT on pull requests, so it
  cannot report a status on a PR and must not be required (requiring it would
  block every merge). It is a post-merge and daily drift gate. Add a
  `pull_request` trigger first if you want it required.

## Optional Hardening
Not configured today; possible later:
- Require branches to be up to date before merging.
- Enable merge queue if PR volume increases.
- Restrict who can dismiss stale reviews.

## Operational Rule
If required checks fail:
- Do not bypass protection.
- Fix the issue or revert the breaking change.
- Re-run checks until green.
