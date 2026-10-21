# Quick Start: Create, Check, and Export a Transcript

**Status:** Current
**Last modified:** {{git-dates:page}}

This walkthrough uses a tiny practice transcript, not your research data.
[Install chatter](installation.md) first, then open a terminal in an empty
working directory. Keep the output names below unused so you do not overwrite
existing files. The commands work in a checkout-free installation.

## 1. Create a valid starting point

```bash
chatter new-file --output practice.cha --speaker CHI --language eng --utterance 'hello .'
chatter --tui-mode disable validate practice.cha
```

`new-file` supplies the CHAT headers and tab separators. Validation should
report one valid file and zero invalid files. This confirms that the file
passes the selected format checks; it does not establish that words, speakers,
or timing accurately describe a recording.

## 2. Recognize and resolve a diagnostic

Open `practice.cha` in a text editor. On the `*CHI:` line, remove only the
final period, save, and run validation again:

```bash
chatter --tui-mode disable validate practice.cha
```

This ordinary, non-CA utterance now lacks a terminator. Expect an `E305`
diagnostic and a failing exit status. Read the code, highlighted location and
suggestion together. Restore the period, save, and validate once more; the
file should be valid again. Do not enable a different transcription policy or
suppress the code merely to silence a genuine defect.

Errors prevent a successful validation result. Warnings request review but do
not make an otherwise valid file invalid. A warnings-only run exits `0`.
Fixing an early structural error can reveal later errors, so counts need not
decrease on each edit. If a diagnostic conflicts with the documented CHAT
rule, preserve the original and investigate before changing its meaning.
See [Validation Errors](validation-errors.md) and the
[editor's playbook](chat-processing-playbook.md).

For scripts, exit `0` means success, `1` means validation or operational
failure, and `2` means invalid command-line usage. `--quiet` suppresses success
output, **not errors**. See [CI Integration](ci-integration.md) for automation.

## 3. Preview repairs; normalize into a separate file

```bash
chatter fix practice.cha
```

Without `--apply`, this reports available automatic fixes without changing
the file. The valid practice file should need none. Not every diagnostic has
an automatic repair. Review the proposed changes and keep an original before
choosing to apply them; semantic fixes require explicit code selection.
See [`fix` in the CLI reference](cli-reference.md#fix).

Normalization is a separate operation: it serializes CHAT consistently, not
a promise to repair invalid input or preserve every original byte.

```bash
chatter normalize practice.cha --validate --output practice.normalized.cha
chatter --tui-mode disable validate practice.normalized.cha
```

Both commands should succeed. This practice file has no `@Media` header.
For real transcripts, preserve the relationship between transcript and media
names when choosing output names; a renamed copy may need a reviewed header
change, not just a new filename.

## 4. Export the structured model and return to CHAT

```bash
chatter to-json practice.cha --output practice.json
chatter from-json practice.json --output restored.cha
chatter --tui-mode disable validate restored.cha
```

The restored file should validate. The JSON represents the typed CHAT model;
conversion is not a byte-for-byte backup. Keep the original when formatting
or source provenance matters. See the [JSON contract](../integrating/json-output.md).

## 5. Move to your own workflow

To validate a directory recursively:

```bash
chatter validate corpus/
```

An interactive terminal can show a progress TUI. Add `--tui-mode disable`
for plain text. For machine-readable validation results, use `--format json`:
these diagnostic records are **not** the transcript-model JSON from `to-json`.

For feedback while editing, `chatter watch practice.cha` keeps running and
revalidates on saves; stop it with Ctrl+C.

- [Batch Workflows](batch-workflows.md): directory validation, exports and cache management
- [CLI Reference](cli-reference.md): commands, options and policy choices
- [Library Usage](../integrating/library-usage.md): use the typed model in Rust
- [Install](../../install/index.md): desktop and editor options
- [CHAT Format Overview](../../chat-format/overview.md): learn the transcription format
