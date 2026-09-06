# Pre-Release Audit Backlog

**Status:** Current
**Last updated:** 2026-09-05 22:56 EDT

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

## Re-triage 2026-07-28 (post v0.4.1)

Four open alerts. Same organizing question: does it reach a shipped artifact?
**None do, and none has an upstream fix.**

| Advisory | Severity | Path | Shipped? | Patched version |
|---|---|---|---|---|
| `fast-xml-parser` | high | `@wdio/cli` -> `@wdio/utils` -> `edgedriver` | No | none |
| `js-yaml` | high | `@wdio/mocha-framework` -> `mocha` | No | none |
| `brace-expansion` | high | `@wdio/cli` -> `glob` -> `minimatch` | No | none |
| `glib` 0.18.5 | medium | transitive Tauri GTK (Linux) | No (not in macOS builds) | none |

All three npm advisories arrive through `@wdio/*`, which `package.json` lists
under `devDependencies`: the WebdriverIO E2E harness. It is not bundled into
the desktop app, so the DoS vectors (entity expansion, quadratic YAML merge
keys, exponential brace expansion) are reachable only by someone running our
own test suite against hostile input they supplied themselves.

`glib` is an unsoundness in `Iterator`/`DoubleEndedIterator` impls, reached
transitively through Tauri's Linux GTK stack, and is not compiled into the
macOS artifacts at all.

**Action: none available.** Every advisory reports `first_patched_version:
none`, so there is nothing to bump to. Re-check when upstream ships fixes;
these will keep appearing in the public Dependabot tab until then, which is
expected rather than neglected.

Verified with `gh api repos/TalkBank/chatter/dependabot/alerts` and
`npm ls <pkg>` in `apps/chatter-desktop`, 2026-07-28.

## Re-triage 2026-09-05 (0.19.0)

The current Dependabot response has two open alerts. The July statement that
none has a fix and none reaches a shipped platform is historical: 0.19.0
ships Linux desktop artifacts, and GLib has a patched newer API line.

| Package | Current path and scope | Disposition |
|---|---|---|
| `extract-zip` 2.0.1, high | `@wdio/utils` -> `@puppeteer/browsers`; development browser-download tooling | No patched version reported; not bundled as desktop runtime JavaScript. Treat downloaded browser archives as the affected input boundary. |
| `glib` 0.18.5, medium | Tauri 2.11.5 -> GTK 0.18/WebKit; included in the Linux desktop dependency graph | Patched in 0.20.0, which is outside GTK 0.18's compatible dependency line. A direct version override would not migrate that stack. |

The [GLib advisory](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)
concerns `VariantStrIter` iteration, where a C out-pointer is passed with
incorrect mutability. It can become a null-pointer crash under optimization.
The local production-source review found no callers of `array_iter_str` or
`VariantStrIter` in this project's application code or downloaded dependency
sources outside GLib's own implementation/tests. That is bounded source
inspection, not proof of runtime unreachability or a claim that the dependency
is patched. Keep this open for a compatible upstream fix or a reviewed GTK
stack migration; do not represent it as absent from Linux artifacts.

The [extract-zip advisory](https://github.com/advisories/GHSA-jmr9-qjv8-65gv)
describes symlink path traversal while extracting an archive. The affected
package is marked `dev` in the desktop lockfile and arrives through WebdriverIO,
not the shipped frontend dependency graph. No forced dependency substitution
or local fork was introduced for either alert.

Reproduce the dependency-scope checks with:

```bash
cargo tree --locked --target x86_64-unknown-linux-gnu -i glib@0.18.5
npm explain extract-zip --prefix apps/chatter-desktop
gh api repos/TalkBank/chatter/dependabot/alerts
```

### Note for the cutover

The public repo will still show the accepted/deferred advisories in its
Dependabot tab. Before or shortly after the flip, decide whether to
record these as a committed `deny.toml` ignore list (with the
rationale above) so `cargo deny` is green in CI and the suppressions
are auditable, rather than leaving them as recurring noise. That is a
Pass 0 follow-up, tracked here, not a flip blocker.
