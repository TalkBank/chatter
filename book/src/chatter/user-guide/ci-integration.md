# CI Integration

**Status:** Current
**Last updated:** {{git-dates:page}}

How to use `chatter` in continuous integration pipelines.

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | All files valid / command succeeded |
| `1` | Validation errors found or command failed |
| `2` | Invalid arguments or missing required options |

All examples below rely on exit code 1 to signal validation failure.

## Basic Usage

```bash
chatter validate corpus/ --quiet --tui-mode disable
```

- `--quiet` suppresses per-file success output
- `--tui-mode disable` keeps the interactive TUI away even on a terminal. A
  run whose stdout is not a terminal, or that asks for `--quiet`,
  `--format json` or `--audit`, never opens the TUI anyway.
- Exit code 0 means all files valid; 1 means errors found

`--format json`, `--quiet` and `--audit` each name one output: `--audit`
with `--format` or `--quiet`, `--format json` with `--quiet`, and
`--tui-mode force` with any of them are usage errors (exit 2).

## GitHub Actions Example

```yaml
- name: Validate CHAT corpus
  run: |
    chatter validate corpus/ --audit results.jsonl

- name: Upload validation report
  if: failure()
  uses: actions/upload-artifact@v4
  with:
    name: validation-report
    path: results.jsonl
```

The `--audit results.jsonl` flag streams one JSON line per diagnostic to a
file, each carrying its `severity` (`"Error"` or `"Warning"`), which is
useful for archiving or downstream analysis even when the step fails. The
record shape is in the [diagnostic contract](../integrating/diagnostic-contract.md).

## JSON Output for Automation

```bash
chatter validate corpus/ --format json --tui-mode disable 2>/dev/null
```

Each file produces a JSON object on stdout with `status`, `error_count`,
and `errors` array. The exit code still reflects overall pass/fail.

## Pre-commit Hook

```bash
#!/bin/sh
# .git/hooks/pre-commit
chatter validate . --quiet --tui-mode disable
```

This blocks commits that introduce invalid CHAT files. The hook runs
quickly on cached files; only modified files are re-validated.

## Suppressing Specific Errors

Some corpora have known issues that should not block CI. Use `--suppress`
to ignore specific error codes or named groups:

```bash
chatter validate corpus/ --suppress E726,E727,E728 --tui-mode disable
```

Or use the named group shorthand:

```bash
chatter validate corpus/ --suppress xphon --tui-mode disable
```

Suppressed errors do not appear in output and do not affect the exit code.

## Audit Mode for Large Corpora

For bulk corpus validation where you want a full error database without
caching overhead:

```bash
chatter validate corpus/ --audit errors.jsonl --tui-mode disable
```

The `--audit` flag streams one JSON object per error to the specified file.
A summary is printed to stderr at the end.
