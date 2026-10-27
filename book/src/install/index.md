# Install

**Status:** Current
**Last modified:** {{git-dates:page}}

Everything here comes from the [latest
release](https://github.com/TalkBank/chatter/releases/latest).

> **Just want to check CHAT files, and you are not a programmer?** Get the
> **desktop app** below. You never need a terminal.

## Chatter desktop app (recommended for most people)

The Chatter app checks CHAT transcripts in an ordinary window: choose a file,
read its diagnostics, edit it in your usual transcript editor, then re-check.
It does not edit transcripts itself. No terminal or separate CLI installation
is needed. It offers updates for you to approve.

### macOS

Official Mac releases are Developer ID signed and Apple-notarized. If macOS
reports unexpected damage or an unverifiable download, stop and check that you
have the intended release; do not disable system security to make it open.

1. Download Chatter for your Mac:
   - **Apple Silicon** (an Apple M-series chip): **[Download Chatter for Apple
     Silicon](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-macos-apple-silicon.dmg)**.
   - **Intel** (older models): **[Download Chatter for Intel
     Mac](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-macos-intel.dmg)**.

   Not sure which you have? Choose **About This Mac** in the Apple menu: an
   Apple chip uses Apple Silicon; an Intel processor uses the Intel download.
2. Open the downloaded **`.dmg`** file.
3. Drag **Chatter** onto the **Applications** folder in the window that appears.
4. Open **Chatter** from your Applications folder (or Launchpad).

### Windows (Intel/AMD 64-bit, "x64")

**[Download Chatter for
Windows](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-windows-setup.exe)**
and run the installer. Windows binaries are not code-signed yet, so SmartScreen
may warn on first run. Verify the source and selected release before deciding
whether to proceed; follow your organization's software policy. Do not disable
SmartScreen globally.

### Linux (Intel/AMD 64-bit, "x86_64")

Download **[Chatter
(AppImage)](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-linux-x86_64.AppImage)**
(make it executable, then run it) or the **[`.deb`
package](https://github.com/TalkBank/chatter/releases/latest/download/Chatter-linux-x86_64.deb)**
(install with your package manager).

(The desktop app is x86_64-only on Windows and Linux today; macOS has both
Apple Silicon and Intel builds. The `chatter` command-line tool below also ships
a Linux ARM build.)

Continue with [your first desktop validation session](../chatter/user-guide/desktop-app.md#a-first-validation-session).

## `chatter`, the command-line tool (for programmers and automation)

If you are comfortable in a terminal, the `chatter` CLI validates, normalizes,
converts (JSON), watches, and batch-processes CHAT files, and is
the right tool for scripting and CI.

macOS / Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.sh | sh
```

Windows (PowerShell):

```powershell
irm https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.ps1 | iex
```

Then run `chatter --version` and `chatter --help`, followed by the
[practice tutorial](../chatter/user-guide/quick-start.md). Full reference: [CLI
installation](../chatter/user-guide/installation.md) and [CLI
Reference](../chatter/user-guide/cli-reference.md). Official installer-based
installations can use `chatter experimental update`; source and package-manager installations
must use their original installation method.

## `talkbank-lsp` language server (editor integration)

For live CHAT validation, hover, go-to-definition, and cross-tier alignment
inside an LSP-aware editor (Neovim, Emacs, Helix, Zed, VS Code, and others),
install the standalone language server. It is separate from the desktop app:

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

Follow the [editor integration tutorial](../chatter/user-guide/editor-integration.md)
to configure a client, check that it attached, and verify diagnostics after an
edit. Running `talkbank-lsp` alone in a terminal does not open an editor: it
waits for protocol messages on standard input.

Editor quick fixes do not supply missing participant names, roles or languages.
Enter those facts yourself: an undeclared speaker or missing/empty header does
not establish the right value. E308, E504 and E507 therefore offer no guessed
edit, consistent with the [CLI fix policy](../chatter/user-guide/cli-reference.md).
Diagnostics remain visible, including parser recovery diagnostics when a
malformed header cannot be lowered. Other supported quick fixes remain available.

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
