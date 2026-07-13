# Quick Start

**Status:** Current
**Last updated:** 2026-07-13 17:59 EDT

This page gets you from zero to productive with `chatter` in five minutes.
[Install chatter first](installation.md) if you haven't already.

## Validate a CHAT file

Check a single transcript for errors:

```bash
chatter validate transcript.cha
```

If the file is valid you get a summary (a cache-statistics block follows it;
use `--quiet` to suppress all output and rely on the exit code):

```text
=== Summary ===
Total files: 1
Valid: 1
Invalid: 0
```

If there are problems, you'll see rich diagnostics with the exact location
and a stable error code. For example, a `*CHI:` line missing its terminator:

```text
✗ Errors found in transcript.cha

E305 (https://talkbank.org/errors/E305)

  × error[E305]: Expected terminator not found (line 6, column 1)
   ╭─[input:6:1]
 6 │ *CHI:   hello world
   · ─────────┬─────────
   ·          ╰── here
   ╰────
  help: Add a terminator at the end: Standard (. ? !), Interruption
        (+... +/. ...), or CA intonation (⇗ ↗ → ↘ ⇘ ...)
```

Every error code (`E305`, `E705`, etc.) is documented with fix guidance in the
[validation error reference](validation-errors.md).

Not every diagnostic is an error. Some codes are warnings: the file is valid
CHAT, but something is worth flagging (for example `E254`, a word-level
`@s:` language override that is not listed in `@Languages`). A file whose only
diagnostics are warnings is reported as valid, and its heading reflects that:

```text
⚠ Warnings in transcript.cha

E254 (https://talkbank.org/errors/E254)

  ⚠ warning[E254]: Explicit word language 'spa' is not listed in @Languages
   ╭─[input:6:15]
 6 │ *CHI:   hello hola@s:spa .
   ·               ─────┬────
   ·                    ╰── here
   ╰────
  help: Add 'spa' to @Languages or confirm the word-level override is intentional
```

The summary still counts this file under `Valid`, and the exit code stays `0`.

## Validate an entire corpus

Point `chatter` at a directory, it walks recursively, validates in parallel,
and caches results:

```bash
chatter validate corpus/
```

The interactive TUI shows progress and lets you browse errors per file.
Use `--format json` for machine-readable output, or `--quiet` for CI
(exit code 1 on errors).

## Convert to JSON

Get a structured representation of any CHAT file:

```bash
chatter to-json transcript.cha
```

The output conforms to the [TalkBank CHAT JSON Schema](https://talkbank.org/schemas/v0.1/chat-file.json).
Convert back with `chatter from-json`.

## Watch for changes

Edit a file and get live validation feedback:

```bash
chatter watch transcript.cha
```

Every time you save, `chatter` re-validates and shows updated diagnostics.

## What next?

- **[CLI Reference](cli-reference.md)**: all commands, flags, and output formats
- **[Validation Errors](validation-errors.md)**: every error code, with examples and fix guidance
- **[Batch Workflows](batch-workflows.md)**: corpus-scale validation and analysis
