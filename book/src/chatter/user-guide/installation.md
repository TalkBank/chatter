# Installation

**Status:** Current
**Last modified:** 2026-07-07 21:20 EDT

`chatter` targets **Windows, macOS, and Linux**. There are two ways to
install it: the **prebuilt binaries** (recommended for most people,
including clinicians and researchers) and a **from-source** build (for
contributors or unsupported platforms).

## Prebuilt binaries (recommended)

Every [GitHub Release](https://github.com/TalkBank/chatter/releases)
attaches prebuilt binaries for macOS (Apple Silicon and Intel), Linux
(x86_64 and ARM64), and Windows (x64), plus desktop-app installers.

### chatter CLI

One-line installers (they download the binary for your platform, place
it on your PATH, and also install the `chatter-update` self-updater):

- **macOS and Linux:**

  ```sh
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.sh | sh
  ```

- **Windows (PowerShell):**

  ```powershell
  powershell -ExecutionPolicy Bypass -c "irm https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.ps1 | iex"
  ```

On Windows the binary is not yet code-signed, so SmartScreen may warn on
first run: choose **More info**, then **Run anyway**. The macOS binaries
are codesigned, and the installer above does not set the quarantine
attribute, so Gatekeeper does not prompt.

Prefer a manual download? Grab the archive for your platform from the
[latest release](https://github.com/TalkBank/chatter/releases/latest)
and extract `chatter` onto your PATH. (On macOS, a browser-downloaded
archive is quarantined; right-click the binary and choose **Open** once,
or run `xattr -d com.apple.quarantine ./chatter`.)

Verify:

```sh
chatter --version
chatter --help
```

### chatter desktop app

The desktop app ("Chatter") is for people who prefer a window to a
terminal. Download the installer for your platform from the
[latest release](https://github.com/TalkBank/chatter/releases/latest):

- **macOS:** the `.dmg` is signed and notarized; open it and drag the app
  to Applications. No Gatekeeper override is required.
- **Windows:** the installer is not yet signed (same SmartScreen note as
  above: **More info** then **Run anyway**).
- **Linux:** an AppImage and a `.deb` are provided.

## Updating chatter

`chatter` keeps itself current so you do not have to track releases by
hand.

- **CLI:** run

  ```sh
  chatter update
  ```

  This runs the bundled `chatter-update` program, which checks GitHub
  Releases and installs the newest release in place. (The self-update
  facility is experimental. It is installed only by the one-line
  installers above; if you installed another way, update the same way you
  installed.)
- **Desktop app:** the app checks for updates on launch and offers to
  install a new version when one is available.

## From source

Building from source needs only a stable **Rust** toolchain (install via
[rustup](https://rustup.rs/), which supports Windows, macOS, and Linux).
Node.js and the tree-sitter CLI (`cargo install tree-sitter-cli`) are
needed only when working on the grammar or generated artifacts.

Clone and install the CLI:

```bash
git clone https://github.com/TalkBank/chatter.git
cd chatter
cargo install --path crates/chatter --locked
```

This installs the `chatter` binary to `~/.cargo/bin/` (macOS/Linux) or
`%USERPROFILE%\.cargo\bin\` (Windows). To update a source install, pull
and re-run the `cargo install` command above (`chatter update` is only
for installer-based installs).

### Building the libraries

If you are developing with the Rust crates directly, from your chatter
checkout root:

```bash
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --all-targets -- -D warnings
```

See the [contributor setup](../../contributing/setup.md) for additional
commands.

## Directory layout

Everything lives in a single repository:

```text
<your-chatter-checkout>/
├── grammar/            # Tree-sitter grammar
├── crates/             # All Rust crates (talkbank-* + the chatter binary)
├── spec/               # CHAT specification
├── apps/               # Tauri desktop app (chatter-desktop)
└── book/               # Chatter mdBook (this book)
```

The CLI, grammar, crates, and the LSP/desktop integrations all live in
this single repository.
