# Batch Workflows

**Status:** Current
**Last modified:** {{git-dates:page}}

The `chatter` CLI is designed for processing large CHAT corpora efficiently. This page covers common batch workflows.

## Practice a complete batch run

This exercise uses two copies of a public reference transcript, so you can learn
the success and failure paths before using research data. Install Chatter and
open Bash at the root of a Chatter source checkout. Python 3 is used only to
make one deliberate error in a practice copy.

Create a fresh directory outside the checkout. Keep reports outside its `corpus`
subdirectory, and use an isolated validation cache:

```bash
practice_dir=$(mktemp -d)
mkdir "$practice_dir/corpus"
cp corpus/reference/core/basic-conversation.cha "$practice_dir/corpus/first.cha"
cp corpus/reference/core/basic-conversation.cha "$practice_dir/corpus/second.cha"
TALKBANK_CHAT_CACHE_DIR="$practice_dir/cache" \
  chatter validate "$practice_dir/corpus" --format json \
  > "$practice_dir/valid.jsonl" 2> "$practice_dir/valid.stderr"
cat "$practice_dir/valid.jsonl"
```

Expect exit `0`, two `file` records with `status: "valid"`, and a final `summary`
with `outcome: "complete"`, `total_files: 2`, `valid: 2` and `invalid: 0`.
Parallel processing can change the order of file records. The summary comes
last; these are diagnostic records, not transcript-model JSON.

Next, remove one ordinary utterance terminator from the second copy. The script
refuses to edit if the reference sentence has changed:

```bash
python3 - "$practice_dir/corpus/second.cha" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
source = path.read_bytes()
sentence = b"*CHI:\tI want some cookies .\n"
if source.count(sentence) != 1:
    raise SystemExit("Expected reference sentence is absent or ambiguous; no edit made")
path.write_bytes(source.replace(sentence, b"*CHI:\tI want some cookies\n", 1))
PY

if TALKBANK_CHAT_CACHE_DIR="$practice_dir/cache" \
    chatter validate "$practice_dir/corpus" --audit "$practice_dir/invalid.jsonl" \
    > "$practice_dir/invalid.stdout" 2> "$practice_dir/invalid.stderr"; then
  validation_status=0
else
  validation_status=$?
fi
printf 'Validation exit: %s\n' "$validation_status"
cat "$practice_dir/invalid.stdout"
cat "$practice_dir/invalid.stderr" >&2
cat "$practice_dir/invalid.jsonl"
```

Expect exit `1`, a completed run with one valid and one invalid file, and an
`E305` error in the audit report. The changed file is rechecked even though the
first run populated the cache. This failure is expected; do not suppress it.
Capturing the status in an `if` keeps the logs available even in a shell using
`set -e`. An unattended CI step must still exit with that captured status, as in
the [CI example](ci-integration.md#github-actions-example).

Preserve the failing practice input alongside its report, then restore the
second copy from the unchanged first copy and make a new report:

```bash
cp "$practice_dir/corpus/second.cha" "$practice_dir/second-before-correction.cha"
cp "$practice_dir/corpus/first.cha" "$practice_dir/corpus/second.cha"
TALKBANK_CHAT_CACHE_DIR="$practice_dir/cache" \
  chatter validate "$practice_dir/corpus" --audit "$practice_dir/corrected.jsonl" \
  > "$practice_dir/corrected.stdout" 2> "$practice_dir/corrected.stderr"
cat "$practice_dir/corrected.stdout"
cat "$practice_dir/corrected.stderr" >&2
printf 'Practice files and reports: %s\n' "$practice_dir"
```

Expect exit `0`, two valid files and an empty `corrected.jsonl`. Keep the failing
report and its input until you have compared the results. An empty audit report
alone is not a success signal: require the successful exit and complete summary
too. For real data, review and correct the actual defect; copying an unrelated
valid transcript over a failing one is never a repair.

## Validating a Corpus

Validate all `.cha` files in a directory tree:

```bash
chatter validate /path/to/corpus/
```

The validator recursively discovers `.cha` files and processes them in parallel.
Eligible cached results can be reused when the input content and validation
policy match. Cache hits still read and hash the files.

### Forcing Revalidation

To bypass the cache and revalidate everything:

```bash
chatter validate /path/to/corpus/ --refresh-cache
```

### Filtering Output

Hide progress and the success summary while retaining diagnostics:

```bash
chatter validate /path/to/corpus/ --quiet
```

`--quiet` does not filter diagnostic severity: warnings are retained if emitted,
as are errors and operational failures. It does not change validation rules or
the exit status. Use the exit status to decide whether the run succeeded.

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
chatter convert to-json corpus/ --output-dir corpus-json/
```

The export preserves the directory structure. To restore one exported file
into a new CHAT file:

```bash
chatter convert from-json corpus-json/example.json --output restored.cha
chatter --tui-mode disable validate restored.cha
```

Single-file `to-json --output` and `from-json --output` refuse an output that
is the input itself, including a hard link or symlink to it, and exit 1 without
truncating that file. A distinct existing output can still be replaced. Keep
separate output paths: this protection is not atomic publication, and an I/O
failure can leave a partial output. Shell redirection (`>`) opens its destination
before Chatter runs, so use `--output` for this protection.

Directory export checks every existing destination against the full discovered
input population before using timestamps to skip it. An output that is a hard
link or symlink to any input is a failed conversion; it is neither overwritten
nor removed as stale output. New destinations are admitted before writing too.
Source files must remain in place during export: this is not a filesystem
snapshot or protection against concurrent source replacement.

A directory export reports converted, up-to-date and failed counts on stderr.
Any conversion failure makes the command exit 1; successfully exported files
remain available. Fix the reported failure and rerun. Incremental export uses
file modification times, unlike the validation cache's content hashes; use
`--rebuild` when timestamps do not establish freshness. A failed conversion removes
its stale JSON from an earlier run. Keep the export directory dedicated to these
outputs, and review failures before restoring or consuming the exported set.

`from-json` takes a single input file, not a directory. For transcripts with
`@Media`, choose names that preserve the transcript/media naming contract;
arbitrarily renaming a restored transcript can introduce a validation finding.

The roundtrip is designed to preserve the `ChatFile` model. In regression
tests, compare normalized output rather than assuming byte-for-byte identity
after parser or serializer changes.

## Cache Management

The validation cache stores results for previously validated files
(matched by content hash and validation policy). The cache database file is named
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
chatter cache clear --prefix corpus/
```

The second command previews a scoped removal; it does not delete entries.
Only add `--apply` after checking that the listed scope is what you intend.
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
