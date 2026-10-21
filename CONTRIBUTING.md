# Contributing to chatter

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/CONTRIBUTING.md)

## Development setup

Start with [Contributor Setup](book/src/contributing/setup.md), which owns the
toolchain, native dependencies and package-specific first checks. A full
workspace build includes the desktop app and its platform prerequisites;
it is not the minimum setup for every parser or documentation change.

[Developer Verification Checks](book/src/contributing/dev-checks.md) explains
the focused development loop, generation, documentation checks and final
publication gate. Use those maintained recipes rather than a second checklist
here. For a first library change, the
[tested library examples](book/src/chatter/integrating/library-usage.md) show
parsing, validation, model access and serialization separately.

## Avoid committing private or internal content (mandatory)

Do not commit secrets, credentials, personally-identifying
information, or details of any private infrastructure. Before every
commit, review your diff for:

- Real first names in non-citation contexts
- Internal or personal email addresses
- Machine/host names used in private infrastructure
- Internal absolute paths (`/Volumes/...`, machine-specific `~` paths)
- Operational detail tied to private infrastructure (deploy logs,
  scratch directories, run state)

The manual review above is the commit contract for all contributors.
Maintainers also perform an explicit publication review. Commit hooks do not
automatically run privacy, publication-date, evidence or changelog checks;
optional contributor-owned local hooks are preserved.

## Coding conventions

The repo's conventions are codified in each crate's AGENTS.md and the
workspace-level `[workspace.lints.clippy]` table in `Cargo.toml`.
High-friction rules to know up front:

- **No panics in long-lived logic.** `unwrap_used`, `expect_used`,
  `panic`, `todo` and `unimplemented` are denied by the workspace Clippy
  policy. Crates inherit that policy; test code may explicitly relax it for
  assertions. Use typed, `thiserror`-based domain errors instead.
- **Types are the primary documentation.** Newtype every stable
  domain boundary; never raw `String`/`&str`/`bool` at a seam.
- **Specs are the source of truth for CHAT format behavior.** Edit
  `spec/constructs/` or `spec/errors/`, then regenerate tests via
  the spec-tooling workflow.

## CI

The [CI and Release guide](book/src/contributing/ci-and-release.md) describes
the workflow roles. `just gate` is the local counterpart of per-push CI,
including doctests, both Rust workspaces, generated-artifact checks and the
book. Clippy and the feature-off build belong to `just release-lint`, not an
extra gate on every edit. The workflow files and `justfile` own the exact
command set; a local pass does not certify every supported platform.

Before pushing, run `just gate` on the final source, then commit those same
bytes. Its receipt survives staging and committing, but any content edit
requires another gate. The gate compares its opening and closing trees and
records a receipt only after every check succeeds on unchanged content.

The pre-push hook checks both the working tree and every Git object being
pushed against that receipt, peeling annotated tags to their commit trees.
Uncommitted fixes cannot authorize an older commit. A mixed push containing
another tree is refused. The hook does not launch tests while Git holds an
open network connection. Install it with `just install-hooks`; the isolated
protocol regression is `just gate-receipts-test` and also runs in CI.

## Reporting issues

GitHub Issues: <https://github.com/TalkBank/chatter/issues>

Use a minimal synthetic example and include the affected version, command and
expected versus actual result. Do not attach private transcripts or credentials.
For security problems, follow [the private reporting policy](SECURITY.md)
instead of opening a public issue.
