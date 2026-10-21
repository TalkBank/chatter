# Edit CHAT with Live Diagnostics

**Status:** Current
**Last modified:** {{git-dates:page}}

`talkbank-lsp` connects an LSP-capable editor to Chatter's parser and validator.
The editor owns the buffer and saves; the language server reads its document
updates and returns diagnostics and supported editing actions. Installing the
desktop app alone does not install or configure this connection.

## 1. Select the language server

[Install the standalone language server](../../install/index.md#talkbank-lsp-language-server-editor-integration).
Locate the executable with `command -v talkbank-lsp` on macOS/Linux or
`Get-Command talkbank-lsp` in PowerShell. Use that exact path in the editor when
several installations exist. The editor's environment may differ from your shell.

Every client needs these settings, though their configuration syntax differs:

| Setting | Value |
| --- | --- |
| Server command | The selected `talkbank-lsp` executable; no extra arguments |
| Transport | Standard input/output (stdio), not a TCP port |
| Files | `*.cha` |
| Language identifier | `chat` |

The server is not an interactive terminal program. Running it by itself can
appear silent because it is waiting for LSP messages. Let your editor start
and stop it. Do not redirect logging or shell banners into its protocol stdout.
The server reports its name and version during LSP initialization; inspect
your client's connection information when checking which version attached.

## 2. Try a minimal Neovim connection

This example uses Neovim's built-in `vim.lsp.config` / `vim.lsp.enable` API
(Neovim 0.11 or later); no additional LSP plugin is needed. See
[Neovim's LSP documentation](https://neovim.io/doc/user/lsp/) for that API and
[filetype configuration](https://neovim.io/doc/user/filetype/) for file associations.

Use the `practice.cha` from the [CLI tutorial](quick-start.md), or download a
copy of the [minimal reference transcript](https://raw.githubusercontent.com/TalkBank/chatter/v0.30.0/corpus/reference/edge-cases/empty-and-minimal.cha)
into a practice folder. Keep the original transcript unchanged.

Save the following as `chatter-lsp.lua` in that folder. Replace the executable
placeholder with the absolute path you selected above (forward slashes also
work in a Windows Lua path):

```lua
vim.filetype.add({ extension = { cha = "chat" } })
vim.lsp.config("talkbank", {
  cmd = { "/absolute/path/to/talkbank-lsp" },
  filetypes = { "chat" },
  root_markers = { ".git" },
  workspace_required = false,
})
vim.lsp.enable("talkbank")
```

Open the practice file with this configuration instead of replacing your normal
editor configuration:

```sh
nvim -u chatter-lsp.lua practice.cha
```

Use your downloaded file's name if different. In Neovim, run `:set filetype?`
and expect `chat`. Run `:checkhealth vim.lsp` and check that `talkbank` is
attached to this buffer. `workspace_required = false` permits a standalone
practice file outside a Git repository.

Once this works, you can merge the snippet into your normal configuration.
Avoid enabling a second CHAT language-server client over the same buffer.
Other editors need the same command, transport and language association; use
their own LSP client's configuration rather than pasting Lua into their settings.

## 3. Verify an edit, not just a connection

1. Open the valid practice transcript and wait for initial diagnostics.
2. Remove only the final period from its `*CHI:` utterance. In this ordinary,
   non-CA transcript, expect E305 for the missing terminator.
3. Read the diagnostic before changing anything else. In Neovim,
   `:lua vim.diagnostic.open_float()` opens the diagnostic at the cursor.
4. Restore the period. E305 should clear after the editor sends the change.
5. Save the restored file. If you also have the CLI, validate the saved copy
   with `chatter --tui-mode disable validate practice.cha`.

The language server sees the editor's current buffer; the CLI sees saved bytes.
That difference explains many apparent disagreements. Errors, warnings and
available features also depend on the selected validation policy and client
capabilities. A quiet editor is not proof that a language server is attached.

## 4. Use editing actions deliberately

Hover, completion, navigation, formatting, highlighting and code actions depend
on what the client supports. CHAT-specific extension commands require a client
that implements those commands; installing a generic LSP client does not create
every specialized UI.

Formatting produces canonical CHAT, not a byte-identical copy. Keep a checkpoint,
review its diff, and save only the intended changes. Review quick-fix edits too:
the server cannot infer missing participant identities, roles or languages from
an invalid header. E308, E504 and E507 do not offer guessed values. Enter the
intended facts yourself, then revalidate.

Quick fixes require a client that advertises versioned workspace edits
(`workspace.workspaceEdit.documentChanges`). Other clients still receive
diagnostics, but the server withholds quick-fix edits rather than returning an
unversioned edit. If the buffer changes before you apply a suggested fix, request
it again; the client must honor the document version attached to the edit.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| No server attached | Executable path, execute permission, `chat` filetype and client startup log |
| Process waits silently in a terminal | It is a stdio protocol server; launch it through the editor |
| Old server version | The executable selected by the editor, not just the shell's PATH |
| Diagnostics differ from CLI | Save the buffer; compare versions, file names and selected policies |
| Duplicate diagnostics | Check whether two CHAT clients or extensions are active |
| Formatting changes too much | Undo in the editor, retain the original, and review a minimal example |

For a report, include the editor/client and server versions, operating system,
observed code and a minimal synthetic reproduction. Client protocol logs may
contain complete transcript text and paths: inspect them before sharing and
never publish restricted participant data. Maintainers can continue with
[LSP testing and diagnosis](../../contributing/lsp-testing.md).
