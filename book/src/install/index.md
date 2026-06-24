# Install

**Status:** Current
**Last modified:** 2026-06-24 09:54 EDT

Everything here comes from the [latest
release](https://github.com/TalkBank/chatter/releases/latest).

> **Just want to check CHAT files, and you are not a programmer?** Get the
> **desktop app** below. You never need a terminal.

## Chatter desktop app (recommended for most people)

The Chatter app checks CHAT transcripts in an ordinary window: open a file, see
the problems highlighted, fix them, and re-check. No terminal and no setup, and
it updates itself when a new version comes out.

### macOS

The Mac app is signed and notarized by Apple, so it opens normally (no security
warnings).

1. Download Chatter for your Mac:
   - **Apple Silicon** (M1/M2/M3/M4, essentially every Mac sold since late
     2020): **[Download Chatter for Apple
     Silicon](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-macos-apple-silicon.dmg)**.
   - **Intel** (older models): **[Download Chatter for Intel
     Mac](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-macos-intel.dmg)**.

   Not sure which you have? Apple menu () then **About This Mac**: if it says
   "Apple M...", it is Apple Silicon.
2. Open the downloaded **`.dmg`** file.
3. Drag **Chatter** onto the **Applications** folder in the window that appears.
4. Open **Chatter** from your Applications folder (or Launchpad).

### Windows (Intel/AMD 64-bit, "x64")

**[Download Chatter for
Windows](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-windows-setup.exe)**
and run the installer. Windows binaries are not code-signed yet, so SmartScreen
may warn on first run: choose **More info**, then **Run anyway**.

### Linux (Intel/AMD 64-bit, "x86_64")

Download **[Chatter
(AppImage)](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-linux-x86_64.AppImage)**
(make it executable, then run it) or the **[`.deb`
package](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-linux-x86_64.deb)**
(install with your package manager).

(The desktop app is x86_64-only on Windows and Linux today; macOS has both
Apple Silicon and Intel builds. The `chatter` command-line tool below also ships
a Linux ARM build.)

## `chatter`, the command-line tool (for programmers and automation)

If you are comfortable in a terminal, the `chatter` CLI validates, normalizes,
converts (JSON / XML), lints, watches, and batch-processes CHAT files, and is
the right tool for scripting and CI.

macOS / Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.sh | sh
```

Windows (PowerShell):

```powershell
irm https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.ps1 | iex
```

Then run `chatter --help`. Full reference: [CLI
installation](../chatter/user-guide/installation.md) and [CLI
Reference](../chatter/user-guide/cli-reference.md). `chatter` self-updates with
`chatter update`.

## `talkbank-lsp` language server (editor integration)

For live CHAT validation, hover, go-to-definition, and cross-tier alignment
inside an LSP-aware editor (Neovim, Emacs, Helix, Zed, VS Code, and others),
install the standalone, code-signed language server:

macOS / Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/talkbank-lsp-installer.sh | sh
```

Windows (PowerShell):

```powershell
irm https://github.com/TalkBank/chatter/releases/latest/download/talkbank-lsp-installer.ps1 | iex
```

Or download the per-platform archive (`talkbank-lsp-<target>.tar.xz`, or `.zip`
on Windows) from the release and point your editor's LSP client at the
`talkbank-lsp` binary (it speaks LSP over stdio on `.cha` files, language id
`chat`).

## Rust crates and the grammar (embed in your own program)

To embed CHAT parsing / validation / transformation in your own program, depend
on the `talkbank-*` crates and the `tree-sitter-talkbank` grammar. They are
source-available from this repository (not yet published to crates.io). See
[Library usage](../chatter/integrating/library-usage.md) and the [CHAT format
overview](../chat-format/overview.md).

---

As a 0.x release, APIs and flags may change before 1.0; see the [Release
Notes](../release-notes.md). For audio + ML pipelines (transcribe, force-align,
morphotag), see the upstream `batchalign3` project, which has its own
installation flow.
