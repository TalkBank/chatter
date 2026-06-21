# Security Policy

## Supported versions

`chatter` is in early release (0.x). Security fixes are made on the
latest released version; there is no long-term-support branch yet.

## Reporting a vulnerability

Please do **not** open a public issue for security problems.

Report a vulnerability privately through GitHub's
[private vulnerability reporting](https://github.com/TalkBank/chatter/security/advisories/new)
("Security" tab, then "Report a vulnerability"). This routes the report
to the maintainers privately and lets us coordinate a fix and
disclosure with you.

Please include enough detail to reproduce the issue: the affected
component (CLI, library crate, LSP, extension, desktop app), the
version or commit, and a minimal example where possible.

## What to expect

- We aim to acknowledge a report within a few business days.
- We will keep you updated as we investigate and prepare a fix.
- We will credit reporters who wish to be credited once a fix is
  released.

## Scope

In scope: the `chatter` CLI, the published library crates, the LSP
server, and the desktop app in this repository.

Out of scope: vulnerabilities in third-party dependencies (report those
upstream), and issues that require a privileged local attacker who
already controls the machine running `chatter`.
