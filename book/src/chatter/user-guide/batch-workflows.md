# Batch Workflows

**Status:** Current
**Last modified:** {{git-dates:page}}

The `chatter` CLI is designed for processing large CHAT corpora efficiently. This page covers common batch workflows.

## Validating a Corpus

Validate all `.cha` files in a directory tree:

```bash
chatter validate /path/to/corpus/
```

The validator recursively discovers `.cha` files and processes them in parallel. Results are cached, subsequent runs skip unchanged files.

### Forcing Revalidation

To bypass the cache and revalidate everything:

```bash
chatter validate /path/to/corpus/ --force
```

### Filtering Output

Show only errors (hide warnings):

```bash
chatter validate /path/to/corpus/ --quiet
```

Stop after the first reported error:

```bash
chatter validate /path/to/corpus/ --max-errors 1
```

The limit is at least 1 (`--max-errors 0` is a usage error), and only errors
count: a warnings-only corpus is never stopped. When the count reaches the
limit, no new file starts, and if files were left the run says
`Stopped after reaching --max-errors N; M file(s) were not validated.` (on
stderr; a `stop` record in JSON mode) and exits 1. A run whose limit was
reached by its last file stopped nothing and never says it. The TUI honours
the limit too.

Write a JSONL audit file while validating:

```bash
chatter validate /path/to/corpus/ --audit validation.jsonl
```

## CHAT-JSON Roundtrip

Export a directory using the built-in batch command. Choose a fresh output
directory outside the input tree; preserve the original transcripts:

```bash
chatter to-json corpus/ --output-dir corpus-json/
```

The export preserves the directory structure. To restore one exported file
into a new CHAT file:

```bash
chatter from-json corpus-json/example.json --output restored.cha
chatter --tui-mode disable validate restored.cha
```

`from-json` takes a single input file, not a directory. For transcripts with
`@Media`, choose names that preserve the transcript/media naming contract;
arbitrarily renaming a restored transcript can introduce a validation finding.

The roundtrip is designed to preserve the `ChatFile` model. In regression
tests, compare normalized output rather than assuming byte-for-byte identity
after parser or serializer changes.

## Cache Management

The validation cache stores results for previously validated files
(keyed by content hash). The cache database file is named
`talkbank-cache.db` and lives in the OS cache directory:

- **macOS:** `~/Library/Caches/talkbank-chat/talkbank-cache.db`
- **Linux:** `~/.cache/talkbank-chat/talkbank-cache.db`
- **Windows:** `%LocalAppData%\talkbank-chat\talkbank-cache.db`

It can hold results for large file collections.

To relocate the cache (a different disk, a per-project cache, or an
isolated cache for scripted runs), set the `TALKBANK_CHAT_CACHE_DIR`
environment variable to a directory; the database is created directly
inside it. This is the supported override on every platform, and the
only effective one on Windows, where the default location comes from
the system Known Folder API rather than environment variables.

```bash
chatter cache stats    # Show the cache's location, size and entry count
chatter cache clear --prefix corpus/ --dry-run
```

The second command previews a scoped removal; it does not delete entries.
Only remove `--dry-run` after checking that the listed scope is what you intend.
Cache clearing affects performance, not the source transcripts.

Do not delete the cache file manually while `chatter` is running.

## Reference Corpus Validation

This repository includes a reference corpus at
`corpus/reference/`. It is a finite regression population, not a substitute
for validating your own inputs or proving support for every CHAT construct.
Contributors should use the current commands in the
[reference corpus guide](../../contributing/reference-corpus.md).

## Integration with batchalign

The Batchalign pipeline uses the same Rust core (via PyO3) for CHAT
parsing and serialization. Batchalign itself is the upstream `batchalign3`
project, outside this repository. Validate generated transcripts before using
them downstream, and review their words, speakers and timing separately:
successful format validation does not establish transcription accuracy.
