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

Treat any nonzero exit as failure. Exit `1` can also mean an operational
failure or an incomplete run, not just invalid CHAT; retain stderr when diagnosing it.

## Basic Usage

```bash
chatter validate corpus/ --quiet --tui-mode disable
```

- `--quiet` suppresses per-file success output
- `--tui-mode disable` keeps the interactive TUI away even on a terminal. A
  run whose stdout is not a terminal, or that asks for `--quiet`,
  `--format json` or `--audit`, never opens the TUI anyway.
- Exit code 0 means success under the selected checks; 1 means validation,
  operational or incomplete-run failure. Format validity is not transcription accuracy.

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
chatter validate corpus/ --format json --tui-mode disable
```

Stdout contains newline-delimited JSON records, including file results and a
summary. Branch on the record's `type` and `status`; a valid-file record need
not have an `errors` array or `error_count`. Keep stderr for operational
diagnostics and check the process exit status as well as the summary.
See the [diagnostic contract](../integrating/diagnostic-contract.md) for shapes
and incomplete-run records. This output is not the CHAT model from `to-json`.

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
Record suppression choices alongside the report: a passing suppressed run
does not establish that the suppressed rules passed. Use a documented policy,
not suppression merely to turn a failing run green.

## Audit Mode for Large Corpora

For bulk corpus validation where you want a full error database without
caching overhead:

```bash
chatter validate corpus/ --audit errors.jsonl --tui-mode disable
```

The `--audit` flag streams one JSON object per diagnostic (error or warning) to the specified file.
A summary is printed to stderr at the end.
