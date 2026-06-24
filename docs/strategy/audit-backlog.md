# Pre-Release Audit Backlog

**Status:** Current
**Last updated:** 2026-06-15 15:05 EDT

P2 (non-release-blocking) findings deferred from the v0.1.0 audit
passes, plus the security-advisory triage. P0/P1 items are fixed in
place, not parked here. After the public flip, the open entries here
become public-safe GitHub issues.

## Dependency security advisories (triaged 2026-06-13)

Sources: GitHub Dependabot (38 open alerts at triage time) and
`cargo audit`. The organizing question for each is **does it reach a
shipped v0.1.0 artifact?** The shipped artifacts are the `chatter` CLI
and the chatter-desktop installers.

### Fixed (reached a shipped artifact)

- **`atty` (RUSTSEC-2024-0375, unmaintained / potential unaligned
  read), direct dependency of `chatter`, shipped in the CLI.**
  Replaced its single use (`atty::is(Stream::Stdout)` for color
  autodetection) with `std::io::IsTerminal` (std since Rust 1.70,
  identical semantics) and dropped the dependency. The CLI dependency
  tree no longer contains atty.
- **chatter-desktop npm: non-breaking subset cleared.** `npm audit fix`
  (lockfile only, package.json untouched); frontend still builds
  (`tsc && vite build`). Production dependencies were already at 0
  vulnerabilities before the fix.

### Accepted / deferred (does not reach a shipped artifact, or no fix exists)

- **`rsa` 0.9.x (RUSTSEC-2023-0071, Marvin timing attack, medium
  5.9).** Transitive under the Tauri desktop build only; absent from
  the `chatter` dependency tree. No patched release exists
  upstream (the advisory is unresolved across the ecosystem). Accept
  for v0.1.0; re-check when upstream ships a fix.
- **GTK/GLib stack unmaintained warnings (`atk`, `gdk`, `gdk-sys`,
  `gdkwayland-sys`, `gdkx11`, `gtk`, `glib`, and siblings; ~19 cargo
  audit warnings).** All transitive via Tauri on Linux (gtk-rs is in
  upstream maintenance mode); not in the CLI. Nothing to do until
  Tauri's Linux backend moves off gtk-rs. Accept.
- **`bincode` unmaintained warning.** Transitive, desktop side. Accept.
- **spec workspace `rand` (GHSA-cq8v-f236-94qc, low).** In
  `spec/Cargo.lock`, transitive via `tera` (template engine) and
  `chrono-tz-build` (a build dependency); spec tooling is not shipped.
  Accept.
- **chatter-desktop dev-tooling npm (3 remaining: `serialize-javascript`
  via `mocha` via `@wdio/mocha-framework`).** The WebdriverIO e2e test
  harness; not shipped, and clearing it needs a breaking
  `npm audit fix --force` wdio/mocha bump. Not worth forcing for an
  experimental app's test harness at v0.1.0. Defer to a focused wdio
  upgrade.

### Note for the cutover

The public repo will still show the accepted/deferred advisories in its
Dependabot tab. Before or shortly after the flip, decide whether to
record these as a committed `deny.toml` ignore list (with the
rationale above) so `cargo deny` is green in CI and the suppressions
are auditable, rather than leaving them as recurring noise. That is a
Pass 0 follow-up, tracked here, not a flip blocker.
