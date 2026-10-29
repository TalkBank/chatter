# CI Integration

**Status:** Current
**Last updated:** {{git-dates:page}}

How to use `chatter` in continuous integration pipelines.

Before wiring a real corpus into CI, try the
[complete batch exercise](batch-workflows.md#practice-a-complete-batch-run).
It demonstrates a passing directory, a deliberate E305 failure with retained
logs, and a corrected rerun. A useful CI integration must preserve the failure
exit and evidence as well as handle the successful case.

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
  shell: bash
  run: |
    report_dir=$(mktemp -d "$RUNNER_TEMP/chatter-validation.XXXXXX")
    echo "CHATTER_REPORT_DIR=$report_dir" >> "$GITHUB_ENV"
    if chatter validate corpus/ --audit "$report_dir/results.jsonl" \
        > "$report_dir/stdout.log" 2> "$report_dir/stderr.log"; then
      validation_status=0
    else
      validation_status=$?
    fi
    cat "$report_dir/stdout.log"
    cat "$report_dir/stderr.log" >&2
    exit "$validation_status"

- name: Upload validation evidence
  if: always() && env.CHATTER_REPORT_DIR != ''
  uses: actions/upload-artifact@v4
  with:
    name: validation-evidence
    path: ${{ env.CHATTER_REPORT_DIR }}/
```

The `--audit results.jsonl` flag streams one JSON line per diagnostic to a
file, each carrying its `severity` (`"Error"` or `"Warning"`), which is
useful for archiving or downstream analysis even when the step fails. The
record shape is in the [diagnostic contract](../integrating/diagnostic-contract.md).

Each attempt gets a fresh directory, so a failed or incomplete run cannot upload
a retained report from an earlier attempt as its own. Stdout and stderr are
archived on success and failure; the step retains Chatter's exit status.
The JSONL report is published only after a complete run, including a completed
run that found invalid CHAT. Operational failure can leave it absent: inspect
the logs and step result, rather than treating a missing or empty report as a
successful validation. An accounted-for read failure can instead publish an
empty report while exiting `1`; its cause is in stderr, not a fabricated CHAT
diagnostic. In a matrix job, give each job a distinct artifact name.

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

This validates the working-tree CHAT files under the checkout, including
pre-existing and unstaged changes. It does not validate the Git index alone.
Cache hits still read and hash files; eligible unchanged results can be reused
when their content and validation policy match.

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

For bulk validation with streamed diagnostic records:

```bash
chatter validate corpus/ --audit errors.jsonl --tui-mode disable
```

The report contains one JSON object per displayed diagnostic, including
warnings. It has no valid-file records or final summary record. An empty
report alone cannot establish success: an input-discovery or operational
failure may also leave no diagnostics. Check the process exit status and
retain both stdout and stderr. The human summary and cache counts go to stdout;
operational errors, notices and progress go to stderr.

Audit mode can reuse eligible results from an existing cache but does not
create, migrate, prune, clear or write that cache. Without a usable cache it
validates directly and reports the cache problem. `--refresh-cache` with `--audit` is
a usage error.

Use a separate report path. Regular-file reports are staged beside the destination
and atomically published only after every discovered input is accounted for and
the writer flushes successfully. A complete report of invalid CHAT is published
with a nonzero validation exit. A stopped, aborted or incomplete run, empty
discovery, or an output failure preserves the previous report (or leaves a new
destination absent). Check this run's exit and completion messages: a retained
report may belong to an earlier run.

Destinations that alias a discovered transcript, including hard links and
symlinks, are refused. Existing symlinks to regular reports select their resolved
target; dangling symlinks are refused. Successful replacement changes one report
directory entry, so other hard links retain the old contents. A new destination
created by another writer is never overwritten; replacement of an existing
report is refused if its observed file identity changed. This is not a filesystem
sandbox against concurrent path manipulation or a promise of crash durability,
ownership/ACL preservation, or cross-platform qualification.

Device and pipe destinations remain streaming outputs. They cannot be atomically
published or rolled back and may contain partial data after failure. Retain stdout
and stderr as well as the report.
