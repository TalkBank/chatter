# Diagnostic and JSON Output Contract

**Status:** Current
**Last updated:** {{git-dates:page}}

This page documents the machine-readable JSON surfaces currently exposed by the
top-level `chatter` CLI.

## Stability policy

- Treat field names documented here as the public contract.
- Treat additional fields as additive unless this page says otherwise.
- Treat message wording as human-facing text, not a stable machine contract.
- Every record is serialized from one typed model, so a field is spelled
  the same in every record that carries it. Key order is fixed (`type`
  first, then a record's own tag such as `status`) but is not part of the
  contract: read records as objects.

## `chatter validate ... --format json`

Both `chatter validate FILE --format json` and
`chatter validate DIR --format json` emit **newline-delimited JSON
(NDJSON)** on stdout, with the same record shapes in both modes:

1. exactly one file record per file the run accounted for, then
2. one final summary record.

A single-file invocation still emits a file record followed by a
summary record; it is not a single-object surface.

### Per-file records

Valid files (no diagnostic of severity `Error`; a `warnings` array is
present when the file showed warnings):

```json
{"type":"file","status":"valid","file":"/path/to/file.cha","cache_hit":false}
{"type":"file","status":"valid","file":"/path/to/Session.cha","cache_hit":false,"warnings":[{"code":"W110","severity":"Warning","message":"..."}]}
```

Invalid files: `error_count` counts the diagnostics of severity `Error`, and
the `errors` array holds every diagnostic shown, warnings included, each with
its `severity`. The `note` field is appended when the validator stopped
further checks because of structural errors:

```json
{
  "type": "file",
  "status": "invalid",
  "file": "/path/to/file.cha",
  "error_count": 1,
  "errors": [
    {
      "code": "E502",
      "severity": "Error",
      "message": "Missing @End header at end of file"
    }
  ],
  "note": "Some additional checks may not have run because of structural errors. Fix the structural errors first, then re-validate."
}
```

Files whose roundtrip check failed (`--roundtrip`) use
`"status":"roundtrip_failed"` with a `reason` string, a `diff` (the first
differing lines, `null` for a verdict read from the cache) and, when the
file showed warnings, a `warnings` array; they count among the summary's
`invalid` files.

Read-failure files use `"status":"read_error"` with an `error` string, and
count among the summary's `invalid` files. A file the parser cannot make
sense of is an `invalid` record carrying its parse diagnostics: both parsers
always produce a model with diagnostics, so there is no separate parse-failure
status.

Internal tool failures use `"status":"internal_failure"` with an `error`
string and the retained `errors` array. They do not also emit an `invalid`
record. E001 belongs to `DiagnosticKind::InternalFailure`, not CHAT invalidity.
The attempt determines neither validity nor invalidity, even if other findings
were collected. It is not cached as a validation verdict and exits unsuccessfully.
Report the failure; do not alter CHAT merely to accommodate a producer bug.
Suppression or severity downgrades cannot admit the failed attempt.

This also applies to producer faults during `--roundtrip` reparsing: the attempt
contributes to neither roundtrip counter and writes neither cache verdict.
Retained fault spans from that reparse refer to serialized intermediate text,
not the original file; they are not emitted as original-source annotations.
Earlier input findings are withheld if that roundtrip attempt fails internally,
so they cannot publish a conflicting invalid-file verdict.

### Summary record

```json
{
  "type": "summary",
  "directory": "/path/to/dir",
  "total_files": 2,
  "valid": 1,
  "invalid": 1,
  "internal_failures": 0,
  "outcome": "complete",
  "cache_hits": 0,
  "cache_misses": 2,
  "cache_hit_rate": 0.0,
  "cache_errors": 0
}
```

`outcome` says how the run ended: `"complete"` (every discovered file was
accounted for), `"nothing_found"` (the input named no transcript; this
summary carries only `directory` and `outcome`), `"stopped"` (a stop record
precedes the summary) or `"incomplete"` (an incomplete record precedes it).
Only a `"complete"` summary counts the whole input, so a consumer reads the
ending before it reads any total.

When `--roundtrip` is set, the summary also includes
`roundtrip_passed` and `roundtrip_failed` counters. `cache_errors` counts
cache reads and writes that failed (a locked or corrupt database); those
files were validated without the cache, so their results stand.

`cache_hits` and `cache_misses` count only files that consulted a cache. A
file that could not be read, or any file of a run whose cache did not open,
is neither, and `cache_hit_rate` (hits over consultations, a percentage) is
`null` when nothing consulted the cache. A file record's `cache_hit` is
`true` only when the verdict that decided its status (validation or
roundtrip) was served from the cache.

### Stop record

Emitted once, before the summary, when the run stopped with files left
unvalidated:

```json
{"type":"stop","reason":"max_errors","limit":50,"unprocessed_files":120}
{"type":"stop","reason":"cancelled","unprocessed_files":3}
```

`reason` is `"max_errors"` (`--max-errors N` was reached; `limit` is `N`) or
`"cancelled"` (Ctrl-C). It comes from the runner's terminal event, so it is
emitted only when files really were left: a limit reached by the run's last
file stopped nothing and is not reported. A stopped run exits 1. Text mode
prints the same fact on stderr, as
`Stopped after reaching the error limit (N); M file(s) were not validated.`
or `Validation cancelled; M file(s) were not validated.`

### Incomplete and aborted records

```json
{"type":"incomplete","lost_files":2,"total_files":500,"cause":"worker_faults","detail":"1 worker(s) failed with an internal error."}
{"type":"aborted","reason":"..."}
```

`incomplete` precedes the summary of a run that lost files nobody asked to
skip. `cause` is `"worker_faults"` (workers panicked, could not create their
parser, or could not be started; `detail` lists each) or `"unexplained"` (no
worker failed and no stop was requested, a defect in the validator).
`detail` is a sentence for a person, not a format to parse. `aborted`
replaces the summary of a run that died before producing totals. Both exit
1.

### Notice record

A fact about the run, said before it starts:

```json
{"type":"notice","notice":"suppressing","codes":["E736","E737"]}
{"type":"notice","notice":"deprecated_flag","flag":"--check-xphon"}
{"type":"notice","notice":"interrupt_unavailable","error":"..."}
```

`interrupt_unavailable` says the Ctrl-C handler could not be installed, so
an interrupt ends the process without a stop record. Text mode prints the
same notes on stderr (`note: suppressing 2 code(s): E736, E737`). An input
path that cannot be read is not a notice: it is a `file` record with
`"status": "read_error"`, and it fails the run. A run that found no
transcript at all ends with a `"nothing_found"` summary and exits 1, since
it validated nothing. Ctrl-C in JSON mode prints nothing on stderr; the run
ends with a `"cancelled"` stop record.

### Cache record

Emitted only when cache maintenance actually did something: a prune
reclaimed rows, `--force` cleared entries, or maintenance failed and
the run continued without it.

```json
{"type":"cache","action":"clear","entries_cleared":1}
{"type":"cache","action":"prune","rows_deleted":12,"versions_deleted":2}
{"type":"cache","action":"warning","operation":"initialize","error":"..."}
```

A warning's `operation` is one of `"initialize"` or `"clear"` (the cache
operations a `validate` run performs before it starts); `error` is a
sentence for a person.

They are records rather than stderr sentences because JSON mode leaves
stderr empty (see below), and rather than silenced because they are
results a caller can act on, so they belong on the stream in a form a
reader can parse. A consumer should dispatch on `type` and ignore values
it does not recognise.

### Contract notes

- The `type` field is stable, and its values are `"file"`,
  `"summary"`, `"cache"`, `"stop"`, `"incomplete"`, `"aborted"` and
  `"notice"`. Treat an unknown `type` as ignorable
  rather than as an error: new record kinds may appear.
- **Stderr is not part of the JSON surface and is empty in JSON mode.**
  Anything a run wants to tell you arrives as a record on stdout.

- For file records: `file` and `status` are stable; `cache_hit` and
  `warnings` are stable for `valid` records. `error_count` and `errors` are
  stable for `invalid` records; `reason`, `diff` and `warnings` for
  `roundtrip_failed` records.
- For summary records: `directory`, `total_files`, `valid`,
  `invalid`, `internal_failures`, `cache_hits`, `cache_misses`,
  `cache_hit_rate`, `cache_errors`, and `outcome` are stable.
- For stop records: `reason` (`"max_errors"` or `"cancelled"`),
  `unprocessed_files`, and for `"max_errors"` `limit`, are stable.
- `status` values: `valid`, `invalid`, `roundtrip_failed`, `read_error`,
  `internal_failure`. New status values may appear.
- Errors do not include a byte-offset `location` field in the
  NDJSON surface; for byte-offset diagnostics use the LSP or the
  non-JSON renderer.
- The `note` field on invalid file records is human-facing
  guidance and may be added or omitted between releases.
- Exit code `0` means all files validated successfully; exit code
  `1` means at least one file failed or an I/O error occurred.

## `chatter validate --audit FILE`

An audit writes one JSON object per line to `FILE`, one record per
diagnostic (errors and warnings alike), and prints its summary to stdout:

```json
{"file":"/path/to/bad.cha","code":"E502","severity":"Error","message":"...","line":12,"column":1}
{"file":"/path/to/ok.cha","code":"W110","severity":"Warning","message":"...","line":4,"column":7}
```

Contract notes:

- `file`, `code`, `severity`, `message`, `line` and `column` are stable, and
  every record has all six.
- `severity` is `"Error"` or `"Warning"`, spelled as the `--format json`
  records spell it. A warning does not fail its file; filter on `severity`
  to read errors only.
- `line` and `column` are 1-based, and `null` when the diagnostic has no line
  position.
- A file with no diagnostic writes no record; the summary counts it.
- A file that cannot be written whole fails the run (exit 1).

## `chatter to-json`

`chatter to-json` emits the full `ChatFile` JSON model rather than a diagnostic
summary. The authoritative contract for that output is the JSON Schema
documented in [JSON Schema](json-schema.md).

Practical notes:

- The JSON itself is the contract, not any validation status lines printed by the CLI.
- Use `-o/--output` if you want only the JSON in a file.
- Use `--skip-validation`, `--skip-alignment`, or `--skip-schema-validation`
  only when you explicitly want to bypass those checks.

## `chatter cache stats --format json`

Cache statistics emit one JSON object on stdout, tagged by `database`, which
says what is in the cache directory. The command only reads: it creates,
migrates and writes nothing.

```json
{
  "database": "current",
  "total_entries": 743,
  "cache_dir": "/Users/example/Library/Caches/talkbank-chat",
  "cache_size_bytes": 274432,
  "last_modified": "2026-03-09T13:05:31.000Z"
}
```

```json
{ "database": "absent", "cache_dir": "/Users/example/Library/Caches/talkbank-chat" }
```

```json
{ "database": "older_schema", "cache_dir": "/Users/example/Library/Caches/talkbank-chat" }
```

Contract notes:

- `database` is `"current"`, `"absent"` (the directory holds no database; the
  command exits 0, as having no cache yet is a legal state) or
  `"older_schema"` (a database an older build left, which the next
  `chatter validate` or `chatter cache clear` migrates; its entries are not
  counted, since migrating can remove some). A database a newer build wrote
  fails the command (exit 1).
- `absent` and `older_schema` carry only `cache_dir`. `current` carries
  `total_entries`, `cache_dir`, `cache_size_bytes` and `last_modified`.
- `cache_dir` is the cache directory as a string, or `null` for an in-memory
  cache, which has no directory. A directory whose path is not valid UTF-8
  fails the command rather than being written with replacement characters.
- `cache_size_bytes` is the database file's length in bytes, or `null` when
  there is no database file (an in-memory cache, or a database removed
  between the command's look and its read). `0` means a file that exists and is
  empty, never "no file".
- `last_modified` is RFC 3339 in UTC with exactly three fractional digits
  (`2026-03-09T13:05:31.000Z`), so string order is time order. It is `null`
  exactly when `cache_size_bytes` is, because there is no database file.
- A database file that exists but whose size or modification time cannot be
  read, including a modification time outside the years -9999 to 9999, fails
  the command; it is never reported as a zero size or a `null` time.
