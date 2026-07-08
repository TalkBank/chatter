# Contributing to chatter

**Last modified:** 2026-07-07 21:17 EDT

## Development setup

A fresh clone needs:

- **Rust** (pinned by `rust-toolchain.toml` to a specific stable release,
  not a floating `stable`, so per-push CI
  is reproducible). `rustup` installs it automatically on first `cargo`
  invocation.
- **mdBook + mdbook-mermaid + lychee** for building and link-checking the
  docs (link-checking runs on the built HTML via lychee, not the
  mdbook-linkcheck2 renderer):

  ```sh
  just book-install-tools
  ```

- **Node.js 20+** for the desktop app (`apps/chatter-desktop`).
  `nvm use 20` or whatever your version-manager equivalent is.
- **SQLite dev headers** for `talkbank-cache`. macOS bundles them.
  Linux contributors install `libsqlite3-dev` (Debian/Ubuntu) or the
  equivalent for their distro.

Standard build commands:

```sh
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
just book
cd apps/chatter-desktop && npm install   # desktop app dependencies
```

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

Maintainers additionally run an automated identifier screen as a
commit-time hook. A standalone, in-repo version of that screen is not
yet packaged for external contributors; until it is, the manual
review above is the contract.

## Coding conventions

The repo's conventions are codified in each crate's CLAUDE.md and the
workspace-level `[workspace.lints.clippy]` table in `Cargo.toml`.
High-friction rules to know up front:

- **No panics in long-lived logic.** `unwrap_used`, `expect_used`,
  `panic`, `todo`, `unimplemented` are `warn` at the workspace level
  and `deny` per-crate where the panic-site audit has completed (see
  the `[lints.clippy]` table in each crate's `Cargo.toml`). Use
  `thiserror`-based domain errors instead.
- **Types are the primary documentation.** Newtype every stable
  domain boundary; never raw `String`/`&str`/`bool` at a seam.
- **Specs are the source of truth for CHAT format behavior.** Edit
  `spec/constructs/` or `spec/errors/`, then regenerate tests via
  the spec-tooling workflow.

## CI

GitHub Actions runs `cargo fmt --check`, `cargo build`, `cargo test
--workspace`, `cargo clippy`, `mdbook build`, and `npm run
compile` on every push and PR. See `.github/workflows/ci.yml`.

## Reporting issues

GitHub Issues: <https://github.com/TalkBank/chatter/issues>
