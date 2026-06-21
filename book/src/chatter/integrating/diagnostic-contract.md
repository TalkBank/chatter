# Diagnostic and JSON Output Contract

**Status:** Current
**Last updated:** 2026-06-15 15:00 EDT

This page documents the machine-readable JSON surfaces currently exposed by the
top-level `chatter` CLI.

## Stability policy

- Treat field names documented here as the public contract.
- Treat additional fields as additive unless this page says otherwise.
- Treat message wording as human-facing text, not a stable machine contract.

## `chatter validate ... --format json`

Both `chatter validate FILE --format json` and
`chatter validate DIR --format json` emit **newline-delimited JSON
(NDJSON)** on stdout, with the same record shapes in both modes:

1. zero or more per-file records (one per validated file), then
2. one final summary record.

A single-file invocation still emits a file record followed by a
summary record; it is not a single-object surface.

### Per-file records

Valid files:

```json
{"type":"file","file":"/path/to/file.cha","status":"valid","cache_hit":false}
```

Invalid files (the `errors` array is opaque per-error JSON; the
`note` field is appended when the validator stopped further checks
because of structural errors):

```json
{
  "type": "file",
  "file": "/path/to/file.cha",
  "status": "invalid",
  "error_count": 1,
  "errors": [
    {
      "code": "E502",
      "message": "Missing @End header at end of file",
      "severity": "Error"
    }
  ],
  "note": "Some additional checks may not have run because of structural errors. Fix the structural errors first, then re-validate."
}
```

Parser-failure files use `"status":"parse_error"` with an `error`
string. Read-failure files use `"status":"read_error"` with an
`error` string.

### Summary record

```json
{
  "type": "summary",
  "directory": "/path/to/dir",
  "total_files": 2,
  "valid": 1,
  "invalid": 1,
  "parse_errors": 0,
  "cache_hits": 0,
  "cache_misses": 2,
  "cache_hit_rate": 0.0,
  "cancelled": false
}
```

When `--roundtrip` is set, the summary also includes
`roundtrip_passed` and `roundtrip_failed` counters.

### Contract notes

- The `type` field is stable: `"file"` or `"summary"`.
- For file records: `file` and `status` are stable; `cache_hit` is
  stable for `valid` records. `error_count` and `errors` are
  stable for `invalid` records.
- For summary records: `directory`, `total_files`, `valid`,
  `invalid`, `parse_errors`, `cache_hits`, `cache_misses`,
  `cache_hit_rate`, and `cancelled` are stable.
- `status` values currently observed: `valid`, `invalid`,
  `parse_error`, `read_error`. New status values may appear.
- Errors do not include a byte-offset `location` field in the
  NDJSON surface; for byte-offset diagnostics use the LSP or the
  non-JSON renderer.
- The `note` field on invalid file records is human-facing
  guidance and may be added or omitted between releases.
- Exit code `0` means all files validated successfully; exit code
  `1` means at least one file failed or an I/O error occurred.

## `chatter to-json`

`chatter to-json` emits the full `ChatFile` JSON model rather than a diagnostic
summary. The authoritative contract for that output is the JSON Schema
documented in [JSON Schema](json-schema.md).

Practical notes:

- The JSON itself is the contract, not any validation status lines printed by the CLI.
- Use `-o/--output` if you want only the JSON in a file.
- Use `--skip-validation`, `--skip-alignment`, or `--skip-schema-validation`
  only when you explicitly want to bypass those checks.

## `chatter cache stats --json`

Cache statistics emit one JSON object on stdout:

```json
{
  "total_entries": 743,
  "cache_dir": "/Users/example/Library/Caches/talkbank-chat",
  "cache_size_bytes": 274432,
  "last_modified": "2026-03-09T13:05:31+00:00"
}
```

Contract notes:

- `total_entries`, `cache_dir`, `cache_size_bytes`, and `last_modified` are stable.
- `last_modified` is RFC 3339 / ISO 8601 text.
