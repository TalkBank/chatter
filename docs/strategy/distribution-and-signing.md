# Distribution & Code Signing Strategy

**Status:** Current (state-of-the-world below); the phased proposal and
per-release decision log that follow are retained as historical context.
**Last updated:** 2026-07-07 21:20 EDT

## State of the world (as of the 0.3.0 release)

Everything the original v0.1.0 plan below scoped has SHIPPED and has been
exercised across multiple releases (v0.1.0, v0.1.1, v0.2.0, v0.2.1):

- **macOS is the signed, primary platform.** The desktop `.dmg` is
  signed AND notarized on every release; CLI binaries are codesigned via
  cargo-dist. The signing/notarization wiring described as "in progress"
  in the decision log below is long done (`release.yml` +
  `release-desktop.yml`).
- **Auto-update is live in both channels** and has carried real users
  across multiple version transitions: `chatter update` for the CLI and
  the Tauri updater for the desktop app.
- **The standalone `talkbank-lsp` server** ships in the same cargo-dist
  release (its own installers and archives) since v0.2.1.
- **Windows ships unsigned, and signing is DEPRIORITIZED** (decision
  2026-07-07): every current chatter user is on macOS, so the
  Authenticode/EV procurement analyzed below is parked indefinitely, not
  queued. The SmartScreen workaround note in the install docs is the
  standing posture. Revisit trigger: a real Windows user population.
- **crates.io publication** remains deferred, now explicitly planned to
  coincide with the 1.0 release. Until then downstream consumers use git
  dependencies pinned to release tags.

Everything below this section is the original planning record.

## Decisions, 2026-06-12 (v0.1.0 release shape)

These supersede the corresponding open items in the phased proposal
later in this document; the proposal text is retained for context.

1. **One coordinated first public release, v0.1.0:** CLI binaries for
   all five cargo-dist targets AND chatter-desktop installers in the
   same GitHub Release. The release is not split; the desktop app does
   not trail.
2. **macOS:** the desktop `.dmg` is signed AND notarized (via
   tauri-action; latest release `action-v0.6.2` as of 2026-06-12). CLI
   binaries are codesigned via cargo-dist's `macos-sign = true`.
   Verified in cargo-dist 0.32.0 source (`src/sign/macos.rs`): it
   codesigns but does NOT notarize; notarization is marked there as
   future work. Consequence: the installer script (`curl | sh`), which
   does not set the quarantine attribute, is the primary CLI install
   path; browser-downloaded CLI tarballs get a documented
   right-click-Open / `xattr -d com.apple.quarantine` note. cargo-dist
   signing secrets: `CODESIGN_CERTIFICATE` (base64 p12),
   `CODESIGN_CERTIFICATE_PASSWORD`, `CODESIGN_IDENTITY`, optional
   `CODESIGN_OPTIONS`. cargo-dist 0.32.0 is the latest release as of
   2026-06-12; re-check before tagging.
3. **Windows:** v0.1.0 ships unsigned (CLI and desktop installer) with
   documented SmartScreen workarounds. Authenticode/EV procurement is
   a follow-up decision (Phase C), not a release blocker.
4. **Desktop release mechanics:** a dedicated
   `.github/workflows/release-desktop.yml` using tauri-action,
   triggered on the same `v*` tag as `release.yml`, uploading its
   installers into the SAME GitHub Release cargo-dist creates
   (tagName mode).
5. **Versioning and notes:** SemVer starting at 0.1.0; hand-curated
   `CHANGELOG.md` in keep-a-changelog format at the repo root; the
   GitHub Release body links to it. GitHub Releases is the canonical
   download host.

The gate-by-gate execution list for this release lives in
[v0.1.0-release-checklist.md](v0.1.0-release-checklist.md).

## Decisions, 2026-06-17 (crates.io publication)

chatter's public-API crates and the `chatter` binary WILL be published
to crates.io, with API docs auto-built on docs.rs, as a first-class
discovery and distribution channel alongside the cargo-dist binary
releases. crates.io is the `cargo install` path already listed in the
CLI channel table below; this decision commits to it rather than
leaving it optional.

This was assessed against the case for git-only distribution in
Sylvain Kerkour, "Why stdx is not on crates.io"
(https://kerkour.com/stdx-cratesio, 2026-06-17), which ships a
64-crate library exclusively via git and rejects registries as the
"wrong solution" to discovery and distribution. We adopt that post's
supply-chain hygiene and reject its git-only conclusion for chatter.

**Why crates.io, not git-only:**

1. **Discoverability is the stated goal, and it is the one axis where
   git-only loses outright.** Rust discovery is crates.io + docs.rs +
   lib.rs. A git-only crate is effectively invisible to that surface,
   and Rust has no equivalent of Go's pkg.go.dev. The stdx post itself
   concedes registries solve discovery and offers no Rust replacement.
2. **The namespace pain barely applies at our scale.** chatter ships a
   handful of `talkbank-*` crates plus the CLI, not 64, so the
   name-collision and `prefix-<pkg>`-squatting concerns that drive the
   stdx decision reduce to a one-time name-availability check here, not
   a structural problem.
3. **Long-term maintainability favors the managed, immutable
   registry.** A published crates.io version is immutable, archived,
   and consumable with no account or gatekeeper. Git-only makes the
   whole toolchain's availability depend on the hosting org staying
   alive, reachable, and un-renamed indefinitely, and on mutable refs:
   the post's own example pins `branch = "main"`, which can be
   force-pushed and is strictly less reproducible than a pinned semver
   whose checksum is recorded in `Cargo.lock`. That fragility is
   exactly what a successor handoff must avoid.
4. **crates.io keeps downstream doors open.** A crate published to
   crates.io cannot depend on git dependencies, and every dependency
   must carry a registry version requirement. Publishing chatter
   therefore lets other crates.io crates depend on it; git-only would
   foreclose that permanently.

**What we adopt from the stdx post (supply-chain hygiene, fully
compatible with publishing):**

- Commit and ship `Cargo.lock`.
- Run `cargo audit` in CI; evaluate `cargo vet` for dependency review.
- Pin and review dependencies deliberately; publish only the crates
  meant to be public API, keeping internal crates unpublished.

**Still open (separate from the publish-or-not question settled here):**

- Exactly which crates are published (public-API crates such as
  `chatter` and `talkbank-model` versus internal-only crates) and
  under what names; resolve name availability before the first publish.
- Whether to reserve the crate names ahead of the v0.1.0 tag.

## Decisions, 2026-06-21 (Intel macOS kept for now)

**chatter keeps `x86_64-apple-darwin` (Intel) as a supported macOS target
for v0.1.0 and the early 0.1.x line, for both the desktop app and the
CLI. It is dropped only when it stops being trivially cheap to produce.**
This supersedes the 2026-06-17 "Intel dropped" note: that note recorded an
intent but was never applied to the pipeline, and on review the keep case
is stronger for the first public release.

Why keep it now:

1. **The audience runs Intel.** chatter's users are clinicians and
   language researchers on slow-turnover institutional hardware (lab
   machines, clinic computers, multi-year procurement cycles). Intel Macs
   are well represented there and still run the supported macOS 26 Tahoe.
   v0.1.0 is the first public release, where reach matters most, and the
   succession audience inherits mixed-Mac fleets. Dropping a target later
   is a one-line change; recovering from "it will not run on my Mac" at
   launch is not.
2. **It currently costs nothing.** The release pipeline already builds
   `x86_64-apple-darwin` (`dist-workspace.toml` `targets` +
   `release-desktop.yml` matrix), so shipping Intel for v0.1.0 is zero
   extra work. The earlier decision's own escape clause, "drop unless
   x86_64 stays trivially cheap to produce", currently resolves to keep.

The sunset is real but not here yet (this is the revisit trigger, both
verified against primary sources):

1. **Apple is ending Intel support.** macOS 27 "Golden Gate" (announced
   WWDC 2026, ships fall 2026) is the first Apple-silicon-only release;
   `apple.com/macos`'s own device-compatibility list contains only
   Apple-silicon Macs, no Intel models. The currently shipping macOS 26
   Tahoe still runs on 2019-2020 Intel Macs and is supported into roughly
   2027-2028, so Intel users are on a sunset path, not gone.
2. **Rust demoted Intel macOS.** `x86_64-apple-darwin` dropped from Tier 1
   to **Tier 2 with host tools** in Rust 1.90.0 (blog.rust-lang.org,
   2025-08-19; confirmed in the platform-support doc, where
   `aarch64-apple-darwin` remains Tier 1). Tier 2 guarantees the build but
   not project-run tests; in practice it builds and runs fine for a
   CLI/desktop app. Rust's cited reasons are Apple's end-of-x86_64
   announcement and GitHub dropping free macOS x86_64 CI runners.

Revisit and drop Intel when any of these makes it no longer trivially
cheap:

- `release-desktop.yml`'s macOS matrix needs a paid/dedicated Intel runner
  (GitHub no longer gives free ones) rather than a cheap arm64
  cross-compile.
- A dependency or the toolchain breaks the Tier 2 `x86_64-apple-darwin`
  build.
- Post-launch download metrics show Intel users are a rounding error
  (realistically a 0.2.x decision, informed by data not available until
  after the public launch).

The release pipeline needs NO change to keep Intel: `dist-workspace.toml`
and `release-desktop.yml` already include both macOS targets. (The local
`build-chatter-desktop-macos.sh` builds `aarch64-apple-darwin` only, which
is fine for local dev; CI produces both.)

## Goal

Make it easy for **non-technical researchers** on macOS, Windows, and
Linux to install chatter's user-facing tools, the `chatter` CLI and
the chatter-desktop Tauri app, without hitting OS security warnings,
manual unblocking, or build-from-source.

This is the difference between "academic tool seven people can compile"
and "tool that ships to the CHILDES community."

## The three distribution surfaces

### 1. `chatter` CLI (Rust binary)

Single static binary per platform. Reasonable distribution paths,
ordered by setup cost:

| Channel | Setup cost | User cost | Audience |
|---|---|---|---|
| **GitHub Releases** with prebuilt binaries | low | technical (download + chmod + path) | early-adopter researchers, devs |
| **Homebrew tap** | low | `brew install talkbank/chatter/chatter` | macOS power users |
| **Scoop bucket** (Windows) | low | `scoop install chatter` | Windows power users |
| **winget package** | medium (review) | `winget install chatter` | broader Windows audience |
| **Cargo crates.io** | medium (publish prep) | `cargo install chatter` | Rust devs only |
| **`.dmg` / `.msi` / `.deb` installers** | medium | install wizard | non-technical users |
| **Mac App Store** | high (entitlements, review, sandboxing) | App Store install | non-technical macOS users at scale |
| **Microsoft Store** | high (review, MSIX packaging) | Store install | non-technical Windows users at scale |

### 2. chatter-desktop (Tauri app)

Tauri 2 produces platform-native installers out of the box:

- **macOS:** `.app` bundle, `.dmg` disk image
- **Windows:** `.msi` (WiX), `.exe` (NSIS), pick one
- **Linux:** `.AppImage`, `.deb`, `.rpm`

These need code signing on macOS and Windows for non-technical users
to install them without a scary "developer cannot be verified" / "Windows
SmartScreen blocked this app" warning.

### 3. talkbank-lsp standalone

The `talkbank-lsp` server is distributed for LSP-aware editors
(Neovim, Helix, Emacs, Sublime, and any other LSP client) as a
standalone download. Same distribution shape as the `chatter` CLI.

## Code signing & notarization

This is where the cost wall sits.

### macOS

> **Status (2026-06-05): macOS signing is provisioned.** An Apple Developer
> Program membership, a Developer ID Application certificate (with its private
> key), and an App Store Connect API key for notarization are all in hand and
> held in the private operator workspace. The remaining work is wiring the
> signing identity, a hardened-runtime entitlements file, and the notarization
> step into the desktop app's Tauri bundle (and, later, the CLI archive). The
> cost discussion below is retained for context and for the still-open Windows
> question.

- **Apple Developer Program membership: $99/year.** Required for both
  CLI signing (codesigning command-line binaries with Developer ID
  Application cert) and Tauri app signing/notarization.
- **Developer ID Application certificate**: issued through the Apple
  Developer portal; used for distribution outside the Mac App Store.
- **Notarization**: `xcrun notarytool` submits the signed binary or
  bundle to Apple; Apple staples a notarization ticket; macOS Gatekeeper
  then admits it without warning. Required for binaries shipped outside
  the App Store as of macOS Catalina+ (10.15).
- **Mac App Store**: separate cert (Mac App Distribution + Mac
  Installer Distribution), separate review process. Substantial
  ongoing overhead: entitlements declarations, sandbox compliance,
  IAP rules if monetized. **Probably not worth it for chatter.**

### Windows

- **Authenticode code-signing certificate** issued by a trusted CA
  (DigiCert, Sectigo, SSL.com, GlobalSign). Pricing varies:
  - **EV (Extended Validation):** $300-600/year. Trusted immediately by
    Microsoft SmartScreen. Requires a hardware token (USB or HSM)
    delivered after identity verification. Best for code signing
    because EV-signed binaries get zero SmartScreen warnings on first
    run.
  - **OV (Organization Validation):** $200-400/year. Less expensive.
    Triggers SmartScreen warnings until the binary accumulates
    "reputation" via downloads, which can take weeks for low-volume
    tools.
  - **Free options:** Sigstore via `cosign` works for verification but
    Windows SmartScreen doesn't honor it; not a substitute for
    Authenticode.
- **Microsoft Store:** separate signing pipeline; requires App Store
  packaging (MSIX). Same cost/benefit calculation as the Mac App
  Store, probably not worth the certification overhead unless we
  want broad consumer reach.

### Linux

Linux doesn't require code signing for binary distribution. Users
trust the repository (PPA, package archive) rather than the binary
itself. Sign with GPG for distro packagers if/when we publish via
official package repos.

### Total certificate cost

Realistic minimum to support both macOS and Windows non-technical
users:

- Apple Developer Program: **$99/year**
- Windows EV code signing cert: **~$300-500/year**
- **Total: ~$400-600/year ongoing.**

This is **not chatter's decision to make alone**: it commits TalkBank
funds to a recurring cost. The recommendation is to flag this for the
TalkBank PI's awareness now, ahead of any first public release, so the
question of who pays + who holds the certs gets answered before
distribution timing forces it.

## Recommended tooling

### `cargo-dist` (for the CLI + LSP binaries)

The current Rust-ecosystem standard for "ship Rust binaries to
end-users." Built by Astral (uv / ruff team). Handles:

- Cross-compilation for the standard target list (darwin-x64,
  darwin-arm64, linux-x64-musl, linux-arm64-musl, windows-x64)
- GitHub Releases creation with the binaries + checksums
- Installer scripts (shell + PowerShell `irm | iex`)
- Homebrew formula generation (for a tap repo)
- Scoop manifest generation
- macOS codesigning integration (`macos-sign = true` with Apple
  Developer creds). Notarization is NOT supported as of 0.32.0
  (verified in `src/sign/macos.rs`; marked there as future work)
- Windows code signing integration (with cert + token)
- Updater integration via `axoupdater` (see "Auto-update" below; wired
  into v0.1.0 as of 2026-06-16)

Setup: `cargo install cargo-dist`, then `cargo dist init` (interactive
wizard), commit the generated `.github/workflows/release.yml`. Releases
trigger on tag push.

### `tauri-action` (for the chatter-desktop app)

GitHub Action published by the Tauri team. Builds the Tauri app on
each OS runner, signs + notarizes (with secrets), and uploads to a
GitHub Release. Integrates with cargo-dist or runs independently.

## Auto-update

**Decision (Franklin, 2026-06-16): both the `chatter` CLI and the
chatter-desktop app ship an auto-update facility in v0.1.0.** Smooth,
low-friction updates matter for the target audience: a clinician should
not have to track releases, re-download, and re-clear Gatekeeper /
SmartScreen by hand every time a fix lands. v0.1.0 is the first release,
so the very first cross-version update any user experiences is
v0.1.0 to v0.1.1; the updaters are wired and validated now (see
"Validation" below), but the first real end-to-end update happens at the
next release.

### CLI: cargo-dist + `axoupdater`

cargo-dist ships a self-updater. Setting `install-updater = true` in
`dist-workspace.toml` makes the shell and PowerShell installers also
install a standalone program named **`chatter-update`** alongside the
binary; running it polls GitHub Releases and installs the newest release
in place. Because `chatter` exposes external subcommands, the same thing
is reachable as **`chatter update`** (the documented, discoverable
form). Source of these facts: the cargo-dist book, `installers/updater.md`.

Properties that make this the cheap half:

- **No new trust root.** The updater downloads the same GitHub Release
  artifacts the installer already uses. On macOS those CLI binaries are
  codesigned (`macos-sign = true`), and the in-place replacement does not
  set the quarantine attribute, so it does not re-trigger Gatekeeper. On
  Windows the binaries are unsigned today, but an in-place CLI update is
  not a browser download, so it does not surface SmartScreen.
- **Experimental upstream.** The cargo-dist self-updater is marked
  experimental (since cargo-dist 0.12.0). The shell-installer bug in
  cargo-dist 0.21.1 / 0.22.0 / 0.22.1 does not affect us; we pin 0.32.0.
  We surface "experimental" in the user docs.
- **CI rate limits.** axoupdater uses unauthenticated GitHub API calls by
  default; anywhere we exercise it in CI we set
  `AXOUPDATER_GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}`.

### Desktop: Tauri v2 updater plugin

The desktop app uses `tauri-plugin-updater` (Rust) +
`@tauri-apps/plugin-updater` (JS). On launch it checks an update
endpoint; when a newer version is available it offers a non-blocking
"update now / later" prompt (Windows install mode `passive`). Source of
these facts: the Tauri v2 updater plugin documentation.

This is the expensive half, because the Tauri updater is a **second,
independent trust root**:

- **Signing is mandatory and cannot be disabled.** It uses its own
  minisign keypair, entirely separate from the Apple Developer ID
  certificate. The Apple cert satisfies Gatekeeper (is this app from a
  known developer?); the minisign key satisfies the updater (is this
  update bundle the one we published?). A macOS update therefore carries
  two signatures, Apple codesign/notarize on the `.app` and a Tauri
  minisign signature on the update bundle.
- **Key generation:** `tauri signer generate` produces the keypair. The
  PUBLIC key is committed in `tauri.conf.json` (`plugins.updater.pubkey`,
  the key content, not a path). The PRIVATE key and its password are
  supplied to the release workflow as the secrets
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
  (`.env` files do not work; the build reads them from the environment).
- **Losing the private key is unrecoverable:** existing installs can
  never be updated again. It is stored with the rest of the signing
  material in the private operator workspace (`~/talkbank/codesign/`),
  NEVER in any repo tree.
- **Manifest hosting needs no new server.** `createUpdaterArtifacts: true`
  makes the bundler emit update bundles plus detached `.sig` signatures
  (macOS `.app.tar.gz`, Windows `-setup.nsis.zip`, Linux
  `.AppImage.tar.gz`). The endpoint is a static `latest.json` published on
  the GitHub Release:
  `https://github.com/TalkBank/chatter/releases/latest/download/latest.json`.
  The release workflow generates and uploads it alongside the bundles.

### Secret inventory (updated 2026-06-16): 11 total

Auto-update adds two secrets to the release workflows, taking the total
from 9 to 11:

| Workflow | Secrets |
|---|---|
| `release.yml` (cargo-dist CLI) | `CODESIGN_CERTIFICATE`, `CODESIGN_CERTIFICATE_PASSWORD`, `CODESIGN_IDENTITY` (+ the automatic `GITHUB_TOKEN`, reused as `AXOUPDATER_GITHUB_TOKEN`) |
| `release-desktop.yml` (Tauri) | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_CONTENT`, **`TAURI_SIGNING_PRIVATE_KEY`**, **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`** |

Note: `APPLE_API_KEY_PATH` is NOT a secret; the desktop workflow derives
it at runtime from `RUNNER_TEMP` after decoding `APPLE_API_KEY_CONTENT`.

### Succession

The Tauri updater key is a new long-lived secret with a hard failure
mode (lose it and the installed base is stranded on its current version).
It belongs in the same succession story as the Apple Developer ID
certificate (the still-open signing-succession item): who holds it, where
the CI secrets live, and what a successor rotates. Rotating the updater
key is a breaking event (it requires every existing user to reinstall
once from a freshly-keyed release), so it is documented as a deliberate,
announced migration rather than a routine rotation.

### Validation under "no rc rehearsal"

The v0.1.0 release does not do a private rc rehearsal, so the updaters are
validated without a live cross-version release:

- The release workflows are confirmed to emit the updater artifacts
  (`chatter-update` in the installers; the Tauri update bundles + `.sig`
  files + `latest.json`).
- The Tauri updater path is exercised against a locally-built manifest
  that advertises a fabricated higher version pointing at a signed
  bundle, confirming signature verification, download, and install.
- The genuine end-to-end "v0.1.0 detects and installs v0.1.1" path is
  first exercised at the v0.1.1 release and is part of the post-release
  protocol's smoke matrix.

## Cross-platform CI

Distribution starts with **catching platform bugs early.** The CI
matrix is the leading indicator for distribution health.

Current state (2026-06-12): DONE. `cross-platform.yml` exercises
Ubuntu + macOS + Windows on push to `main`, daily, and on manual
dispatch; `ci.yml` remains the Ubuntu merge gate.

Rationale (kept for context): the matrix catches Windows
path-handling bugs (forward vs back slashes, drive letters, `\\?\`
long-path prefix), macOS-specific keychain / signing test paths, and
Linux-specific `pkg-config` / `apt` deps that other OSes don't have.
The main-plus-daily cadence (rather than every-PR) keeps Mac runner
minutes acceptable.

## Phased rollout proposal

These are recommended phases, not commitments. Each phase requires a
go/no-go decision.

### Phase A, Internal testing (now)

- Add cross-platform CI matrix (Ubuntu + macOS + Windows). Catches
  platform bugs.
- Test cargo-dist locally to confirm it builds clean cross-platform
  artifacts.
- macOS Developer ID certificate + notarization key are now in hand (held
  privately); Windows certs not yet purchased; no public distribution yet.

### Phase B, Unsigned releases (low cost)

- Tag a `v0.1.0-pre1` and let cargo-dist build GitHub Releases with
  unsigned binaries for all platforms.
- Document in README: "macOS: right-click → Open to bypass Gatekeeper
  on first run. Windows: SmartScreen will warn; click 'More Info' →
  'Run Anyway'."
- Audience: technical researchers + early adopters who can navigate
  the warnings.

### Phase C, Signed CLI + Tauri app (Apple Developer + Windows EV)

- Apple Developer Program membership + Developer ID Application certificate +
  notarization API key: **DONE** (2026-06-05).
- Acquire Windows EV code signing certificate (~$300-500/year): still open.
- Wire signing into the desktop Tauri build first (active), then cargo-dist and
  tauri-action via GitHub secrets.
- Publish first signed v0.1.0 to GitHub Releases + Homebrew tap +
  Scoop bucket.
- Audience: full researcher community; non-technical install flow.

### Phase D, Package-manager / store presence (later)

- winget package submission (free, but review process).
- Linux distro packaging (debian / rpm via AUR / volunteer maintainers).
- Mac App Store / Microsoft Store: probably skip, see §"Mac App
  Store" above.

## Open questions for the TalkBank PI and the batchalign3 maintainer

These touch shared resources or org-level decisions:

1. **Who pays for the certificates?** TalkBank org funds, or
   personal? Recommended: TalkBank, since the project is
   TalkBank-org-owned (TalkBank/chatter).
2. **Whose Apple Developer + Microsoft account holds the cert?**
   TalkBank as an organization, or an individual? Recommended:
   create org accounts so the cert outlives any single contributor.
3. **What's the release cadence?** Tag every commit to main, every
   month, every quarter? Recommended: every tagged release.
4. **Are the same signing keys reusable for batchalign3?** The
   batchalign3 distribution will face the same signing question.
   If certs are TalkBank-org-owned, they're reusable; if held by
   individual contributors, they're not.

## What this strategy doc does NOT cover

- Specific cargo-dist or tauri-action config; those land when Phase
  A / B starts.
- Linux distro packaging mechanics, separate decision per distro.
- Pricing comparison between Authenticode certificate vendors,
  separate procurement decision when Phase C is funded.
- Mac App Store / Microsoft Store entitlements review, out of
  scope unless we decide to pursue those channels.
