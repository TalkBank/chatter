# Installation

**Status:** Current
**Last modified:** {{git-dates:page}}

`chatter` targets **Windows, macOS, and Linux**. There are two ways to
install it: the **prebuilt binaries** (recommended for most people,
including clinicians and researchers) and a **from-source** build (for
contributors or unsupported platforms).

## Prebuilt binaries (recommended)

Choose a [GitHub Release](https://github.com/TalkBank/chatter/releases) and
inspect its assets. CLI builds target macOS (Apple Silicon and Intel), Linux
(x86_64 and ARM64), and Windows (x64); desktop installers are separate downloads.
An archive's presence does not establish compatibility with every OS version.

### chatter CLI

One-line installers (they download the binary for your platform and place
it on your PATH):

- **macOS and Linux:**

  ```sh
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.sh | sh
  ```

- **Windows (PowerShell):**

  ```powershell
  irm https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.ps1 | iex
  ```

These commands download and execute the project's installer. If you need to
review it first, download the script from the release page and inspect it
before running it. Use your organization's approved installation method where
applicable; do not weaken a machine-wide execution or security policy.

Windows binaries are not yet code-signed, so SmartScreen may warn on first run.
Verify the official source and intended release before proceeding. Official
macOS releases are signed; treat an unexpected security warning as a reason to
check the download, not an instruction to bypass verification.

Prefer a manual download? Grab the archive for your platform from the
[latest release](https://github.com/TalkBank/chatter/releases/latest)
and extract `chatter` into a directory on your PATH. CLI archive names begin
with `talkbank-chatter-`; the extracted executable is still `chatter`.
The release provides `.sha256` checksum files for the archives. Verify the
selected archive against its matching checksum before executing it; a checksum
checks the bytes against that release record, not an independent trust source.

Verify:

```sh
chatter --version
chatter --help
```

Then follow the [practice tutorial](quick-start.md). If the command cannot be
found, open a new terminal after installation and check which executable your
shell selects:

```sh
command -v chatter
```

In PowerShell, use `Get-Command chatter`. Multiple source, package-manager and
installer copies can coexist; an old version often means PATH selected a
different copy. Check that location before reinstalling or deleting anything.

### chatter desktop app

The desktop app ("Chatter") is for people who prefer a window to a
terminal. Download the installer for your platform from the
[latest release](https://github.com/TalkBank/chatter/releases/latest):

- **macOS:** the `.dmg` is signed and notarized; open it and drag the app
  to Applications. No Gatekeeper override is required.
- **Windows:** the installer is not yet signed; verify the source and follow
  your organization's policy if SmartScreen warns.
- **Linux:** an AppImage and a `.deb` are provided.

The [desktop guide](desktop-app.md) explains choosing a file, interpreting the
completed result and revalidating after edits. Install `talkbank-lsp` separately
for [live feedback in an editor](editor-integration.md).

## Updating chatter

Update through the method that installed the selected executable:

- **CLI:** run

  ```sh
  chatter experimental update
  ```

  This self-update runs **in-process**: `chatter experimental update` embeds `axoupdater`
  as a library and downloads the newest release from GitHub Releases
  directly, without a separate bundled `chatter-update` program. It requires
  a supported official installer receipt for this executable. Source and
  package-manager installs should be updated through their original method;
  a missing or unrelated receipt is refused before update execution.
- **Desktop app:** the app checks for updates on launch and offers to
  install a new version when one is available.

## From source

Building from source needs the workspace-selected stable **Rust** toolchain
(install via [rustup](https://rustup.rs/)), native build tools (C compiler and
linker), and SQLite development headers where required by the platform.
See [contributor setup](../../contributing/setup.md) for the platform prerequisites.
Chatter 1.0 supports current stable Rust, with no older-compiler or fixed-MSRV
promise. Run `rustup update stable` if your installed stable toolchain is old.
Node.js and the Tree-sitter CLI are needed only when working on the grammar
or generated artifacts. Use the Node version in `grammar/.nvmrc`, then run
`npm ci` in `grammar/`; its lockfile pins the CLI so regeneration
uses the same toolchain as CI.

Clone and install the CLI and, if needed, the standalone LSP:

```bash
git clone https://github.com/TalkBank/chatter.git
cd chatter
cargo install --path crates/chatter --locked
cargo install --path crates/talkbank-lsp --locked
```

This installs the `chatter` binary to `~/.cargo/bin/` (macOS/Linux) or
`%USERPROFILE%\.cargo\bin\` (Windows). To update a source install, pull
and reinstall the selected package with `--force --locked`, retaining the
reviewed checkout revision. `chatter experimental update` targets published installer
releases, not the source revision you selected.

For a local verification install that must not replace your normal commands,
use an explicit install root:

```bash
cargo install --path crates/chatter --root /path/to/test-install --debug --locked
cargo install --path crates/talkbank-lsp --root /path/to/test-install --debug --locked
```

Its programs are `/path/to/test-install/bin/chatter` and
`/path/to/test-install/bin/talkbank-lsp` (`.exe` on Windows). `--debug` selects
a development artifact, not a release candidate. `--offline` may be added when
all locked dependencies are already cached; it is not a clean-machine drill.
A local `--path` install does not verify crates.io resolution or a published
candidate. Build/install evidence, actual executable behavior and independent
operator/platform acceptance are separate requirements.

### Building the libraries

If you are developing with the Rust crates directly, from your chatter
checkout root:

```bash
cargo build -p talkbank-model --locked
```

Select the package your change affects. See [Library Usage](../integrating/library-usage.md)
for API examples and [contributor setup](../../contributing/setup.md) for the
deliberate test, lint and final publication workflow; a source installation is
not a reason to run every workspace check.

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
