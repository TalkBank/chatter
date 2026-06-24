# Install

**Status:** Current
**Last modified:** 2026-06-24 07:38 EDT

Everything in the chatter toolchain installs from the [latest GitHub
release](https://github.com/TalkBank/chatter/releases/latest). Each tool is a
single signed binary (or app bundle) with no runtime dependencies. Pick the tool
you need.

## `chatter` CLI

Validate, normalize, convert (JSON / XML), lint, watch, and batch-process CHAT
files.

macOS / Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.sh | sh
```

Windows (PowerShell):

```powershell
irm https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.ps1 | iex
```

Then `chatter --help`. Full reference: [CLI installation](../chatter/user-guide/installation.md)
and [CLI Reference](../chatter/user-guide/cli-reference.md). `chatter` self-updates
with `chatter update`.

## `talkbank-lsp` language server

The Language Server Protocol server for CHAT: live validation, hover,
go-to-definition, semantic highlighting, and cross-tier alignment in any
LSP-aware editor (Neovim, Emacs, Helix, Zed, VS Code, and others). It ships as a
standalone, code-signed binary.

macOS / Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/talkbank-lsp-installer.sh | sh
```

Windows (PowerShell):

```powershell
irm https://github.com/TalkBank/chatter/releases/latest/download/talkbank-lsp-installer.ps1 | iex
```

Or download the per-platform archive (`talkbank-lsp-<target>.tar.xz`, or `.zip`
on Windows) directly from the release. Point your editor's LSP client at the
`talkbank-lsp` binary and launch it on `.cha` files (language id `chat`); it
speaks LSP over stdio.

## Chatter desktop app

A graphical CHAT validation app. Download the installer for your platform
(`.dmg` / `.exe` / `.deb` / AppImage) from the
[latest release](https://github.com/TalkBank/chatter/releases/latest). The macOS
`.dmg` is signed and notarized; the app self-updates on launch.

## Rust crates and the grammar

To embed CHAT parsing / validation / transformation in your own program, depend
on the `talkbank-*` crates and the `tree-sitter-talkbank` grammar. They are
source-available from this repository (not yet published to crates.io). See
[Library usage](../chatter/integrating/library-usage.md) and the
[CHAT format overview](../chat-format/overview.md).

---

As a 0.x release, APIs and flags may change before 1.0; see the
[Release Notes](../release-notes.md). For audio + ML pipelines (transcribe,
force-align, morphotag, benchmark), see the upstream `batchalign3` project, which
lives outside the chatter repo and has its own installation flow.
