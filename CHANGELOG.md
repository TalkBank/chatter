# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Before 1.0, breaking changes to the CLI or library APIs bump the minor
version and are listed under "Changed" / "Removed".

## [Unreleased]

## [0.28.0] - 2026-10-02

This release is about telling the truth about a validation run: how it ended,
what it read, what it did with the cache, and what each output promises. Most
of it is breaking for library callers and for scripts that relied on a
command silently ignoring something; each such change is marked.

### Changed

- **Breaking (Rust API):** `RunEnding::Complete` requires producer-admitted
  `CompleteStats`; `Stopped` and `Incomplete` require `PartialStats`.
  Read counts through `snapshot()` and shortfalls through `missing_files()`;
  the separate `unprocessed` and `lost_files` fields are removed. Cloning
  stopped-run counts cannot promote them to complete-run success. CLI JSON
  and desktop event formats are unchanged.

- Documentation publication metadata follows its own Git history: rendered
  book headers use the existing Git-date preprocessor, and other affected
  documents link to their own history. Content-preserving release squashes
  do not require fresh content-review dates. The date check admits the header
  itself, so a placeholder example in the body or a history link to another
  document cannot mask a stale handwritten date. The existing stale-document
  baseline is unchanged.

#### How a validation run ends

- **Breaking (CLI):** a `chatter validate` run that did not cover every file
  fails the command (exit 1) and says so. A run told to stop (Ctrl-C, or
  `--max-errors`) reports how many files it left, `Stopped after reaching the
  error limit (N); M file(s) were not validated.` (or `Validation cancelled;
  ...`), and a stop that came after the last file is no stop at all: the run
  is complete. A run that found no transcript says `Error: no .cha files
  found in PATH` and exits 1.
- **Breaking (CLI):** the TUI's exit status is the run's, as the other
  outputs' are: closing it (`q` or Esc) exits 0 only after a complete run
  with no invalid, unreadable or tool-failed file, and 1 otherwise, including
  when it is closed before the run ends or the terminal fails. Ctrl-C stops
  the run and leaves the TUI open on its ending; a second Ctrl-C force-quits
  at once with exit 130, as it always did, whatever the run found. It lists every file that
  failed, a file it could not read, a failed roundtrip and a tool failure
  among them, says "No .cha files found" for an input with none, and shows
  the `--suppress` and cache notes the other outputs print. An unreadable
  argument used to show "no errors found" and exit 0.
- **Breaking (CLI):** `--max-errors N` counts errors only, never warnings, and
  the validation runner enforces it: each worker counts its file's errors
  before taking the next file, so with `--jobs 1` the stop is exact, and the
  text, JSON, audit and TUI surfaces all honour it. The CLI used to count
  every rendered diagnostic, so a warnings-only file could spend the limit
  and the run still exit 0 with files never validated; the TUI ignored the
  limit. `--max-errors 0` is a usage error (exit 2).
- A run that lost files says why: workers that panicked, workers that could
  not create their parser, or a worker thread the system refused, every one
  observed and none hidden behind another, or no explanation (a validator
  defect). A worker that could not create its parser used to return an empty
  tally, so a run in which every worker failed that way read as a stop, or as
  a loss with nothing to explain it. Text, audit and TUI endings and the
  desktop's incomplete state say the cause.
- **Breaking (CLI JSON):** `validate --format json` reports the ending on
  stdout: the summary's `cancelled` is replaced by `outcome` (`"complete"`,
  `"stopped"` or `"incomplete"`); a stopped run emits a `stop` record (`reason`
  `"max_errors"` with `limit`, or `"cancelled"`, and `unprocessed_files`)
  before its summary; a run that lost files emits an `incomplete` record
  (`lost_files`, `total_files`, `cause` (`"worker_faults"` or
  `"unexplained"`) and `detail`); a run that died emits an `aborted` record.
  All of these were stderr text, which JSON mode promises to keep empty.
- JSON mode keeps stderr empty by construction. `--suppress` and the
  deprecated `--check-xphon` are `notice` records, as is a Ctrl-C handler
  that could not be installed (`interrupt_unavailable`); cache maintenance is
  a `cache` record, Ctrl-C prints nothing (the run ends with a `cancelled`
  stop record), and an unwritable `--audit` file is reported by the command
  instead of exiting from inside the renderer.
- `validate --format json` writes each record straight to stdout, with no
  intermediate string, and a consumer that closes the pipe ends the stream:
  the run exits 1 with stderr empty. It panicked (exit 101, a panic message
  on stderr).
- **Breaking (CLI JSON):** a run that found no transcript ends with a summary
  whose `outcome` is `"nothing_found"` and which carries no counts; it was a
  `"complete"` summary of `total_files: 0`.
- `validate --format json` records and `--audit` JSONL lines are serialized
  from typed models: a diagnostic's `severity` (`Error`, `Warning`) is
  spelled by the model rather than taken from a `Debug` rendering, and keys
  come in a fixed order (`type` first).
- **Breaking (CLI JSON):** `validate --format json` emits exactly one file
  record per file. A file with warnings and no error is one `valid` record
  carrying a `warnings` array; it was an `invalid` record followed by a
  `valid` one. An `invalid` record's `error_count` counts errors only (it
  counted warnings too), while its `errors` array still lists every
  diagnostic with its severity. A `roundtrip_failed` record carries its
  `diff` and any `warnings`, and the contract lists that status.

#### What a run presents, and what it reads

- **Breaking (CLI):** `chatter validate` decides its output once, from all of
  `--format`, `--quiet`, `--audit` and `--tui-mode`: plain text, quiet text,
  JSON, an audit file, or the TUI. The TUI is chosen automatically only for
  plain text with stdout a terminal; `--format json` on a terminal used to
  open it. Flags that name two outputs are usage errors (exit 2): `--audit`
  with `--format` or `--quiet` (it printed text and exited 0), `--format json`
  with `--quiet` (`--quiet` was ignored), and `--tui-mode force` with any of
  them.
- **Breaking (CLI):** `chatter validate` reports an argument it cannot read,
  a nonexistent one included, as a read error in its results (`✗ PATH (read
  error: ...)`, a `read_error` record in JSON mode, counted among the invalid
  files), validates the rest, and exits 1. It used to refuse the whole run
  with stderr text a JSON consumer could not see.
- One walk finds the input of `validate`, `to-json`, `fix`, the `debug`
  commands and the desktop app's directory runs. A directory or entry that
  cannot be read is reported, never skipped: `validate` and the desktop
  record it as a file that could not be read, and the other commands report
  it and exit 1 before processing anything. Links are followed (`to-json`
  used to skip them), a directory reached twice is walked once, and a link
  whose target is gone is a failure whatever its name, since it may have
  been a directory (a link to an unmounted volume's subcorpus). A file named
  on the command line of `fix` or a `debug` command is used whatever its
  extension, as `validate` already did.
- A transcript's stored name is resolved once, where it is found: a walk
  takes it from the listing that found the file, and a file argument is
  resolved once before any worker starts. `fix` and the `debug` tools refuse
  an argument whose stored name cannot be resolved before processing
  anything (exit 1; `fix` used to skip it with an error line), and `to-json`
  on a directory refuses a transcript whose stem is not UTF-8 the same way.
- A transcript named twice on the command line (two spellings of one file,
  `dir/a.cha` beside `./dir/a.cha` or `dir/sub/../a.cha`, or a file inside a
  directory also given) is processed once, under the first spelling given.
- `fix` and every `debug` tool refuse an argument list that names no
  transcript (exit 1); `debug overlap-audit` and `debug linker-audit` used to
  analyze nothing and exit 0.
- **Breaking (CLI):** a `validate --audit` file that could not be written
  whole fails the run (exit 1), and the writer stops at its first failed
  write; a failed write was a warning, the file kept going with a hole in it,
  and the run could exit 0.
- **Breaking (CLI JSONL):** every `validate --audit` record carries
  `severity` (`"Error"` or `"Warning"`, spelled as the `--format json`
  records spell it), after `code`: `{"file", "code", "severity", "message",
  "line", "column"}`. A warning record had nothing to tell it from an
  error, though a warning does not fail its file. The record shape is now
  documented in the diagnostic contract.
- **Breaking (CLI):** the `validate --audit` summary counts what its labels
  say. `Files that failed` (was `Files with errors`) counts the files the run
  failed (invalid, unreadable, a failed roundtrip or a tool failure), so a
  valid file with warnings is not one, and a file that failed with no
  diagnostic (unreadable) is. `Total errors` counts errors only and the new
  `Total warnings` counts warnings; `Total errors` counted every diagnostic.
  `Diagnostics by code` (was `Errors by code`) gives each code's errors and
  warnings separately (`E301: 2 error(s), 1 warning(s) in 2 file(s)`).

#### The validation cache

- `chatter cache clear --dry-run` writes nothing: it opens the cache
  read-only, as an audit does (SQLite's `-shm` and `-wal` files may appear
  beside an existing database, as for an audit), and a cache with no
  database has nothing to clear. It created the cache directory, its lock
  file and the database, and ran migrations, to count zero rows.
- **Breaking (CLI):** `chatter cache stats` only reads the cache, as the dry
  run does. With no cache database it says `No cache database at PATH` and
  exits 0 (no cache yet is a legal state, as the dry run and an audit treat
  it), and it creates nothing; it created the directory, its lock file and
  the database, and migrated it, to report zero entries of a cache it had
  just made. A database an older build left is reported, not migrated.
- **Breaking (CLI):** `chatter cache clear --dry-run` and `chatter cache
  clear` agree, because both start from one look at the cache directory.
  Over a database of an older schema the dry run exited 1 ("a schema this
  build does not read") while the clear migrated it and went ahead; the dry
  run now says the clear would first migrate the database, and that the
  number of entries is known only after the migration (which can remove
  duplicates), and writes nothing; the clear says it migrated, then what it
  cleared. With no cache database, a clear creates none and says so
  (`Cleared 0 cache entries: no cache database at PATH`); it created and
  migrated an empty database to clear nothing. A database a newer build
  wrote fails `stats`, `clear` and its dry run alike (exit 1).
- **Cache keys are a specified hash.** Every row was keyed by `DefaultHasher`,
  whose algorithm Rust does not promise to keep, so a toolchain update could
  have turned the persistent cache into misses with nothing to say so. Rows
  are now keyed by blake3 over a written-down encoding of the path's
  components and the row's namespace (the book's validation-cache chapter
  gives it). The key scheme is part of the cache version, so rows written by
  earlier releases are never served and leave through the existing prune of
  superseded versions: **the first run after upgrading revalidates every file
  once.**
- The cache knows a transcript by its location with every directory
  resolved by the operating system and its own name kept as stored, so
  every spelling of a file is one row: relative or absolute, through `..` or
  a linked directory, and `/tmp` beside `/private/tmp` on macOS.
  `validate --force` clears the cached verdicts of the files it validates
  whatever spelling the paths were given in; with a relative argument
  (`validate --force a.cha`) it cleared nothing and the stale verdict was
  served. `cache clear --prefix` resolves its prefix the same way (a
  directory since deleted through its deepest existing ancestor), and the
  missing-file purge no longer depends on where it is run from.
- **Breaking (CLI):** `validate --audit` only reads the cache: it opens an
  existing cache read-only, creating, migrating, pruning, clearing and
  writing nothing (with no cache, or one an older build left, it runs
  without one and says so), and `--audit --force` is a usage error (exit 2;
  it cleared the files' rows). SQLite may still create its shared-memory and
  write-ahead-log files (`talkbank-cache.db-shm`, `talkbank-cache.db-wal`)
  beside an existing database, which any reader of a WAL database needs; the
  database itself is not written.
- Validation reads each transcript once and keys the cache by the hash of
  those bytes, so a verdict is stored for exactly the content validated even
  if the file changes during the run. A cache read or write that fails is
  counted and reported (a stderr warning, `cache_errors` in the JSON summary,
  a note in the desktop summary) instead of looking like a cold cache.
- A run reports cache hits and misses only for files that consulted a cache,
  and `cache_hit_rate` (the JSON summary's, the text summary's `Hit rate`,
  `ValidationStatsSnapshot::cache_hit_rate()`) is hits over those files
  (hits plus misses); it was hits over every file, so an unreadable file
  lowered it. A run whose cache did not open reports no misses, its JSON
  `cache_hit_rate` is `null` (it was 0.0, with every file a miss), and the
  text summary says `Cache: not consulted`. A cached roundtrip verdict counts
  as a hit.
- **Breaking (CLI JSON):** `chatter cache stats --format json` is tagged by
  `database`: `{"database": "absent", "cache_dir": ...}` when the directory
  holds no database, `{"database": "older_schema", "cache_dir": ...}` for a
  database an older build left (its entries are not counted), and
  `"database": "current"` with `total_entries`, `cache_dir`,
  `cache_size_bytes` and `last_modified`. It reports what it could not find
  as `null`, never as a stand-in: `cache_dir` is `null` for
  an in-memory cache (it was the string `"in-memory"`); `cache_size_bytes` is
  the database file's length, or `null` if the file is missing when the
  statistics are read (it was `0`, the same as an empty file); `last_modified` is UTC
  with exactly three fractional digits (`2026-03-09T13:05:31.000Z`), `null`
  exactly when `cache_size_bytes` is (it was the current time). A size or
  time that cannot be read, a time outside the years -9999 to 9999, and a
  cache directory that is not UTF-8 fail the command instead of being
  reported as 0, `null` or a lossy path. The text output says `(in memory)`
  and `(no cache file)`.
- **Breaking (CLI):** `chatter cache clear` with neither or both of `--all`
  and `--prefix` is a usage error (exit 2; it exited 1). `--dry-run` with
  `--prefix` reports how many entries it would clear, and a clear reports the
  number its delete removed. `--prefix` takes any path the system accepts,
  UTF-8 or not, and selects the rows the cache wrote for it.

#### Commands

- **Breaking (CLI):** a command whose standard output closes (`chatter ...
  | head`) exits 1, silently, from every text writer, as `validate --format
  json` already did. Text output caught the `println!` panic and exited 0,
  so a `validate` run that found invalid files exited 0 when its summary had
  nowhere to go; a missed catch was a panic (exit 101). Every CLI text
  writer goes through one fallible writer, and the panic hook that matched
  "Broken pipe" in panic messages is removed.
- **Breaking (CLI):** `to-json` on a directory with no `.cha` file (an
  empty tree, or a mount point with nothing mounted) exits 1 with `ERROR: no
  .cha files found in DIR` before converting or pruning anything. With
  `--prune` it exited 0 and deleted every `.json` under `--output-dir`, each
  read as an orphan of the empty input; a prune now needs a non-empty
  population of transcripts by type.
- **Breaking (CLI):** flags are parsed into what they select, and a
  combination a command would have ignored is a usage error (exit 2):
  `normalize --skip-alignment` without `--validate`; `to-json
  --skip-validation --skip-alignment`; `validate --list-checks` beside a path;
  and `to-json` with an option for the other kind of input (`--output-dir`,
  `--force`, `--prune` or `--jobs` for a file; `-o/--output` for a
  directory). A file input used to ignore the directory options. A directory
  input without `--output-dir` is a usage error (exit 2; it exited 1).
- **Breaking (CLI):** a `--code` (`fix`) or `--suppress` (`validate`) value
  that names no known error code (or, for `--suppress`, no known group) is a
  usage error (exit 2) naming the value; the commands printed their own error
  and exited 1.
- **Breaking (CLI):** `--jobs 0` (on `validate` and `to-json`) is a usage
  error (exit 2); it ran one worker with a logged warning. Omit `--jobs` to
  use every CPU.
- **Breaking (CLI):** `chatter cache stats` takes `-f/--format text|json`, as
  `validate` does, in place of `--json`.
- `chatter fix` exits 1 when a file could not be read or `--apply` could not
  write a fix, and says how many on stderr; it exited 0 after reporting each
  failure.
- `chatter to-json` and `chatter normalize` report a failure the same way: a
  line `ERROR: <path>: <failure>`, then the rendered diagnostics of a parse
  failure, a validation failure, an incomplete validation or an internal
  failure. `to-json` on one file printed only a one-line summary for some of
  these, under its own `✗ Validation errors found:` and `✗ JSON error:`
  headlines; `normalize` printed `✗ Validation errors found:` for a
  validation failure and dropped the diagnostics of every other failure
  behind `Error: <failure>`.
- `chatter to-json --prune` still never follows a link in the output tree,
  while the walk that reads transcripts now follows them: its deletions stay
  inside `--output-dir`. `--prune` also no longer deletes a JSON file whose
  transcript merely could not be checked, and reports a file or empty
  directory it cannot remove (the run then exits 1).
- `chatter to-json <dir>` warns when it cannot remove the stale JSON of a
  transcript that now fails to convert, runs on the shared worker pool (a
  worker that panics is reported with the files it left unconverted, and the
  run exits 1), and counts files queued on its progress line.
- `chatter watch` validates each changed file through the same one-file
  pipeline as `validate` (stored name, cache, rules), with `validate`'s
  default cache identity, and opens the cache once when it starts (it
  reopened and pruned it on every save). It keeps watching when a watched
  file cannot be read (a rename or a lock mid-edit) instead of exiting, and
  prints a file's diagnostics as `validate --quiet` does.

#### Library API

- **Breaking (library, talkbank-cache):** `RulesVersion::for_testing` exists
  only with the new `test-support` feature, which no production build
  enables; a production caller could name a version no rule set produced.
  The crate's examples use `RulesVersion::current()`.
- **Breaking (library, talkbank-model):** a file stem is checked where it is
  made. `FileStem::from_stem` returns `Result<FileStem, FileStemError>`,
  refusing an empty stem and one with a path separator, and
  `OwnedTranscriptName::Named` holds an `OwnedFileStem` (built from a path, a
  checked string or a `FileStem`) instead of a `String`;
  `TranscriptName::to_owned_name` makes one. The LSP names a document by its
  decoded file path (`TranscriptName::for_path`), so a file name with a space
  or an accent is matched against `@Media` as stored, not percent-encoded.
- **Breaking (library):** a validation run takes `DistinctTranscripts`
  (`talkbank_transform::paths`, built only by `DistinctTranscripts::new`),
  stored transcripts each named once by resolved location, so
  `validate_files_streaming` handed one file under two spellings validates
  and counts it once, as the argument path already did; it validated it
  twice. `ExpandedArguments::into_parts` returns them.
- **Breaking (library):** a validation run's ending is typed. A stream ends
  with one `ValidationEvent::Finished(RunEnding)` (it ended with
  `Finished(stats)`, `FinishedIncomplete` or `Aborted`), where `RunEnding`
  is `Complete(stats)`,
  `NothingFound` (no transcript at all; it was a complete run of zero
  files), `Stopped { stats, unprocessed, reason }`, `Incomplete { stats,
  lost_files, cause: LossCause }` (`LossCause` is `WorkerFaults`, never
  empty, or `Unexplained`) or `Aborted(AbortReason)`, with counts as
  `NonZeroUsize`. `RunEnding::passed()` is the one answer to whether a run
  vouches for its input, the answer the CLI, the TUI and the desktop all
  give. `AbortReason` gains `NoEnding`, for a consumer whose stream closed
  without an ending, and the new `CancelReason` (`ErrorLimit { limit }` or
  `Requested`) implements `Display`, the wording every surface uses for a
  stop. `ValidationStatsSnapshot` has private
  fields read through accessors, with a non-zero `total_files`, and only the
  runner makes one (its `cancelled` field is gone: a stop is the `Stopped`
  ending); `RunCoverage` is no longer public. The streaming entry
  points return a `Canceller` (was `Sender<()>`) and are no longer generic
  over the cache (they take a `ValidationRun`).
- **Breaking (library):** `ValidationConfig`: `check_alignment: bool` is
  `alignment: AlignmentValidation`; `roundtrip` is a `RoundtripCheck` (`Skip`
  or `Run`; was a `bool`); `jobs` is `Option<NonZeroUsize>`; `error_limit:
  ErrorLimit` (`Unlimited` or `StopAfter(n)`) is new; and `directory` and
  `cache` are removed (every walk descends every level, and the cache is a
  parameter). `DirectoryMode` and `CacheMode` are removed. The atomic
  `ValidationStats` is removed: each worker counts its own files and the
  runner sums them after the join.
- **Breaking (library):** every runner entry point
  (`validate_directory_streaming(directory, &run)`,
  `validate_files_streaming(files, &run)`,
  `validate_arguments_streaming(input, &run)`) takes a `ValidationRun`, a
  `ValidationConfig` bound to the cache the run may use; they took the
  configuration and the cache as two arguments, so a library caller could
  hand a run under one rule set a cache opened for another (strict linkers,
  another parser) and get that cache's verdicts. `ValidationRun::new(config,
  cache)` refuses a cache whose identity is not `config.cache_identity()`
  (`CacheIdentityMismatch`), `ValidationRun::uncached(config)` binds none, and
  `ValidationRun::config()` reads the bound configuration. `VerdictReader`
  gains a required `identity()`. The CLI opens the cache from the run's
  configuration and binds it; the desktop binds the pool it memoizes per
  identity, and `validate_target_streaming_with_config(target, &run)` takes
  the bound run.
- **Breaking (library):** a run's cache is a `RunCache` (`Absent`,
  `ReadOnly(Arc<dyn VerdictReader>)` or `ReadWrite(Arc<dyn
  ValidationCache>)`), and how a file used the cache is
  `FileCompleteEvent::cache: CacheUse` (`Hit`, `Miss`, `NotConsulted`);
  `FileStatus` loses its `cache_hit` fields, and `cache_hit_rate()` returns
  `Option<f64>`.
- **Breaking (library):** the cache traits. `VerdictReader` (`get`,
  `get_roundtrip`) and `ValidationCache: VerdictReader` (`set`,
  `set_roundtrip`), all required, take a `ResolvedPath` (from
  `talkbank-model`: the parent directory resolved, the name kept as stored;
  every `StoredTranscript` carries its own, `StoredTranscript::resolved`),
  the `ContentHash` of the bytes validated, and an `AlignmentValidation`,
  and return `Result<CacheLookup<V>, CacheError>` (a failed lookup is an
  error, never a miss). Validation verdicts are `CacheOutcome`; roundtrip
  verdicts are the new `RoundtripOutcome` (`Passed`, `Failed`).
  `ReadOnlyCache` opens an existing, current cache read-only and writes
  nothing; it refuses a missing one (`CacheError::NoCacheDatabase`) or one
  whose schema is not this build's (`CacheError::SchemaNotCurrent`).
  `CachePool`'s own get/set methods, `clear_prefix` and `clear_all` are
  removed: maintenance is `count(&CacheScope)` and `clear(&CacheScope)` with
  `CacheScope::All` or `Under(ResolvedPrefix)`, and `clear_paths` (any
  iterator of `&ResolvedPath`) clears by key in every namespace.
  `CacheStats` holds a `StorageStats` (`InMemory`, or `Directory { cache_dir,
  database }` with `DatabaseFile::Missing` or `Present { size_bytes, modified
  }`); `SpaceReclaimed` gains `VacuumedSizeUnknown`; `VersionPruneOutcome`
  gains `FreshDatabase` (an in-memory cache, where no prune ran);
  `VersionPruneReport::versions_deleted` is a `usize`;
  `purge_nonexistent` fails on a path it cannot check instead of deleting its
  entry. New `CacheError` variants: `CorruptColumn`, `CountOutOfRange`,
  `ModifiedOutOfRange`, `NoCacheDatabase`, `SchemaNotCurrent`.
- **Breaking (library, talkbank-cache):** administration starts from
  `CacheOnDisk::inspect()` (or `inspect_directory(dir)`), one look that
  creates, migrates and writes nothing and returns what is there:
  `Absent(NoDatabase)`, `OlderSchema(OlderSchema)` or
  `Current(InspectionCache)`. A `MaintenanceCache` is reached only from that
  value, by `OlderSchema::migrate()` or `InspectionCache::into_maintenance()`:
  `MaintenanceCache::open` and `open_directory`, which created and migrated a
  database in any directory they were given, and `InspectionCache::open` and
  `open_directory` are removed. A database a newer build migrated is the new
  `CacheError::SchemaNewer` (for `ReadOnlyCache` too, which reported it as
  `SchemaNotCurrent`, whose message said a writing run would upgrade it);
  `SchemaNotCurrent` now means an older schema only.
- **Breaking (library):** one event per file. `ValidationEvent::Errors` and
  `RoundtripComplete`, with `ErrorEvent` and `RoundtripEvent`, are removed:
  a file's diagnostics travel inside its `FileStatus`, in the variants that
  can have them: `Invalid { diagnostics: InvalidDiagnostics }` (never
  empty, with a `NonZeroUsize` `error_count()`), `Valid { warnings }` and
  `RoundtripFailed { warnings, diff }` (`Option<FileDiagnostics>`), and
  `InternalFailure { failure, attempt }`, whose `FailedAttempt` says
  whether the failure's diagnostics point into the file's text
  (`Validation { source }`) or into the roundtrip's serialized text
  (`RoundtripReparse`). `FileStatus::shown()` gives the diagnostics to show
  against the file, with that text; `FileStatus::failed()` says whether the
  file failed. A consumer renders each file once, and the file's text moves
  into its status without a copy.
- **Breaking (library):** `StoredTranscript` carries its `ResolvedPath`
  (`StoredTranscript::resolved`), made when it is admitted, so a transcript
  whose directory cannot be resolved is refused by `resolve` and
  `from_entry`.
- **Breaking (library, talkbank-model):** the coordinated splice
  `MorTier::splice_range_coordinated(gra, item_range, block, root, redirects)`
  (and `splice_coordinated(gra, item_idx, block, root, redirects)`, the same
  over one item) takes its replacement as a `SplicedBlock`, says where the
  block's root attaches with a `SpanRoot`, and where host dependents of the
  replaced items go with a `HostRedirects`; it adds no cycle and no second
  root, so a host `%gra` tree stays a tree. Every type is re-exported beside
  `MorTier` in `talkbank_model::model::dependent_tier::mor`, with
  `CoordinatedMutationError`.
  - `SplicedBlock::new(mors, relations)` replaces the `new_mors` and
    `new_relations` arguments. It parses the block-relative relations once
    and refuses (`SplicedBlockError`) a relation count that differs from the
    chunk count (was `CoordinatedMutationError::CountMismatch`), a head
    outside the block (was `HeadOutOfNewBlock`), a relation whose `index` is
    not its chunk (`IndexOutOfOrder`), a block with no root or several
    (`RootCount`), and a cycle (`Cycle`). The splice wrote a rootless cyclic
    block into the host: the `1 -> 2`, `2 -> 1` reparse of `por@s favor@s`
    became a cycle in the host's `%gra`.
    `SplicedBlock::root_chunk()` exposes the admitted root as a `BlockChunk`
    so callers can direct host dependents to it without a separate root scan.
  - A host `%gra` relation that depended on a replaced item keeps depending on
    that word. The splices rewrote such a head to the first chunk of the new
    block, by position: in morphotag's L2 output for `ich glaube
    it's@s:eng working@s:eng und don't@s:eng stop@s:eng .` that made `und`
    depend on `do` and `stop` on `it`, where they depend on `stop` and
    `working`. The caller states the correspondence: `HostRedirects::ByItem`
    (equal item counts, each item following its counterpart) or
    `HostRedirects::PerItem(Vec<ItemTarget>)`, one target per replaced item:
    `ItemTarget::Chunk(BlockChunk)`, a chunk of the block, or
    `ItemTarget::Counterpart`, the block item at the same position (chunk for
    chunk when the two items have the same chunk count, otherwise the new
    item's head chunk), so a caller can state one target and let the rest
    follow. A statement that does not fit, or a dependent whose item has no
    unique head chunk, is refused before either tier changes
    (`RedirectItemCountsDiffer`, `RedirectCountMismatch`,
    `RedirectOutOfBlock`, `NoCounterpart`, `NoUniqueHeadChunk`).
  - `SpanRoot` replaces the `root_anchor_override: Option<usize>` argument:
    `UtteranceRoot` (head `0`, relation `ROOT`), or `HostChunk { chunk,
    relation }`, a host chunk (`SemanticWordIndex1`) numbered as before the
    splice and translated by it, with the relation the span root takes under
    it, an `AttachmentRelation`, whose constructor refuses a root label
    (`RootRelationUnderHost`), so `ROOT` under a host head cannot be written;
    the block's own label for its root is not kept. `SpanRoot::from_gra_head`
    builds one from a host relation's `GraHeadRef` and the relation. The old
    anchor was a pre-splice index never shifted, and `None` took the head of
    the range's first chunk: in `dont@s:eng mal geh .` the span's `do` came to
    depend on `mal`. A span root at the utterance's root while a host
    relation outside the range is already the root is refused
    (`UtteranceRootTaken`; the splice wrote two roots), as is a host chunk
    inside the replaced range or past the host (`SpanRootInReplacedRange`,
    `SpanRootOutOfHost`), or one whose own chain of heads reaches the range
    (`SpanRootDependsOnSpan`: with `x@s y@s z .` and `z -> x`, anchoring the
    span `x y` at `z` returned a cycle with no root).
  - A host whose `%gra` does not number each relation by its chunk (relation
    `k`, from 1, with index `k`) is refused (`HostIndexOutOfOrder`), since
    every head is read as a chunk number. Such a host was spliced anyway, its
    misnumbered relations' indices shifted by arithmetic that could wrap
    below zero on a shrinking splice. Inside the splice, the numberings
    before and after it, of the block, and of the replaced range are
    distinct types with one translation between the host's numbering before
    and after, so a pre-splice index cannot be written into the result.
  - `CoordinatedMutationError` derives `Clone`, `PartialEq` and `Eq`, and its
    host-chunk fields are `SemanticWordIndex1`. It stays exhaustive, as the
    crate's error enums are: a new refusal is a compile error for a caller
    that matches them.
  talkbank-tools adapts when it moves to this release.
- **Breaking (library):** `talkbank_model::ParseValidateOptions` has private
  fields; its public `validate`, `alignment` and `strict_linkers` booleans
  could say "alignment without validation". The level is the new
  `CheckLevel` (`ParseOnly` or `Validate(AlignmentValidation)`), set with
  `with_level` and read with `level()`; `should_validate` is removed (use
  `validation_policy().is_some()`). `RuleSelection` holds a `LinkerChecks`.
- **Breaking (library, talkbank-model `async`):** `validate_async` and
  `validate_with_rules_async` take an `OwnedTranscriptName` (`Named(String)`
  or `Anonymous`) in place of `filename: Option<String>`, whose `None`
  silently meant "skip the rules about the transcript's name".
- **Breaking (library):** `chat_to_json`, `chat_to_json_named`,
  `chat_to_json_with_schema_policy` and `chat_to_json_unvalidated` take a
  `JsonLayout` (`Pretty` or `Compact`; was `pretty: bool`), and
  `JsonSchemaPolicy::from_skip_flag` is removed: the libraries keep no
  bool-to-mode constructors, and the CLI parses each presence flag straight
  into the mode it selects.
- **Breaking (library):** timestamps in the TOML decision files are
  `talkbank_transform::recorded_time::RecordedTime`, replacing
  `chrono::DateTime<Utc>` (`PendingEntry::created_at`,
  `MergeOverride::decided_at`, and the matching parameters of
  `MergeOverride::auto_decision`, `operator_decision` and
  `judgment_to_pending`). It is written as New York time in whole seconds
  with its offset (`"2026-05-27T08:41:00-04:00"`), whichever host writes it,
  and reads every RFC 3339 time with an offset, so existing files load
  unchanged, and in TOML also a native datetime.

#### Desktop

- The desktop app shows a stopped (cancelled) run as its own state, with the
  number of files it never reached, and an incomplete run with its cause;
  `FrontendStats.cancelled` is replaced by a `stopped` event.
- The desktop's all-valid claim is the runner's verdict, carried as
  `passed` on the `finished` event, so it agrees with the CLI's exit status:
  a run whose files have only warnings is "All N files valid; K warnings".
  A target with no transcript is its own `nothingFound` state, with
  Re-validate offered; it was a finished run of zero files. Stop, loss and
  abort reasons are the runner's own wording.
- A validation cache that will not open is said in the run's summary, with
  the reason, and the run goes on without it; it was a line on the app's
  stderr, which a desktop user never sees.
- The `validate`, `open_in_clan` and `export_results` commands each take one
  `request` argument (`ValidateRequest`, `OpenInClanRequest`,
  `ExportResultsRequest`), the value the backend already used, in place of
  loose ones. A text export writes each file's status as the app shows it,
  one label from one owner, where the backend kept its own copy of the
  sentences.
- `ValidateRequest`'s `jobs` is `Option<NonZeroUsize>` (was `Option<u32>`),
  so `jobs: 0` is refused when the request is deserialized. The Parallel jobs
  field parses its text into a positive whole number (or empty for all
  CPUs), says when an entry is not one and which value the next run uses,
  and never sends `0` or `1.5` on.

#### Internals

- One worker pool, `talkbank_transform::worker_pool::fan_out`, runs the
  validation runner and `chatter to-json <dir>`. Its workers run on the same
  16 MiB stack as the CLI's program thread (they used the 2 MiB thread
  default), `--jobs 1` is the same pool at width one, and each worker returns
  its own counts instead of updating shared counters.
- chatter no longer depends on `chrono`; it uses `jiff`.

### Removed

- **Breaking (CLI):** `chatter fix --dry-run`. A bare `fix` already reports
  without writing, and `--apply --dry-run` (meaning "do not apply") was
  confusing. Passing `--dry-run` is an unknown-argument usage error (exit 2),
  so a script using it fails instead of writing.
- **Breaking (CLI):** `chatter to-json`'s hidden `--validate` and
  `-a/--alignment` flags, deprecated no-ops since validation and alignment
  became the default; passing one is a usage error (exit 2) instead of being
  ignored.
- **Breaking (library, JSON, desktop protocol):** the parse-error file
  status, which nothing produced (both parsers always return a model with
  diagnostics, and an unparsable file is `Invalid`): `FileStatus::ParseError`,
  `ValidationStatsSnapshot::parse_errors`, the JSON summary's `parse_errors`
  (always 0), the `parse_error` record status, the text summary's
  `Parse errors:` line, and the desktop's `parseError` status and
  `parseErrors` count. The desktop summary counts "invalid or unreadable
  files", which is what `invalid_files` holds.
- chatter's `RoundtripValidationMode`, a second copy of `RoundtripCheck`.
- **Breaking (CLI):** `chatter debug overlap-audit -f/--format`, which the
  command never read (it prints TSV, with `--database` for JSON lines);
  passing it is a usage error (exit 2).
- **Breaking (library):** `LossCause::tag`; a JSON consumer reads the CLI's
  `incomplete` record, whose `cause` is serialized from its own wire type.

### Added

- W110 warns when the `@Media` filename and the transcript's own name differ
  only in letter case (`Session.cha` declaring `@Media: session`). CHECK's
  comparison ignores case, so this stays out of E531, but such a name finds
  its recording on a case-insensitive filesystem and not on a case-sensitive
  one. Case and Unicode spelling are reported independently, so a name can
  carry both W110 and W109.
- `PipelineError::diagnostics()` returns the located diagnostics a pipeline
  failure carries, or `None`; `to-json`, `normalize` and speaker
  identification report failures through it.
- `talkbank_model::LinkerChecks` (`Lenient`, `Strict`) and
  `RuleSelection::with_linkers`, so a caller holding the linker choice as a
  value selects the strict linker checks without a branch of its own.
- `ChatFile::validate_at(policy, errors, name)` validates under a
  `ValidationPolicy`: its rules, at its alignment coverage.
- `talkbank_transform::paths` finds input the one way every command uses:
  `walk_files` (a `Walk` of `FoundFile`s and `WalkFailure`s, with a `Links`
  policy, `Follow` or `Skip`), `walk_transcripts` (a `TranscriptWalk` of
  `FoundTranscript`s, each a relative path and a `StoredTranscript`) and
  `expand_transcript_arguments` (`ExpandedArguments`, one `StoredTranscript`
  per resolved location). `StoredTranscript::into_path`.
- `talkbank_model::ResolvedPath` and `ResolvedDirectory`, a file's location
  with its directory resolved by the operating system and its own name kept,
  the identity the cache keys by, and `ResolvedPrefix`, a resolved
  directory or file that a cache scope selects under (all re-exported by
  `talkbank-cache` and `talkbank-transform`).
- `validate_arguments_streaming`, the runner entry point for expanded
  command-line arguments, which reports each unreadable argument as a read
  error in the run's results.
- `talkbank_transform::worker_pool` (`fan_out`, `PoolRun`, `PoolOutcome`),
  the one bounded worker pool (see Internals), and `PoolOutcome::faults`,
  the one reading of how its workers ended, as `PoolFault`s (`Unwound`,
  `ThreadRefused`); the runner's `WorkerFault` wraps one as `Pool`.

## [0.27.0] - 2026-09-28

### Changed

- Cross-platform verification keeps Windows workspace doctests on Cargo's
  normal C runtime, avoiding a Tauri static-runtime library-search collision.
  Desktop release packaging retains its static-runtime configuration.

- Experimental re2c conversion preserves multiword participant names and
  treats lexer continuation tokens as separators in logical gem labels.
  Diagnostic differences remain explicitly assessed, not copied from the
  default parser; re2c is still not a production validity authority.

- Pseudonymization review stores proposed morphology and refused-plan payloads
  behind owned boxes, reducing inline variant and error sizes without changing
  admission or output policy. `MorphologyOutcome::Proposed::proposed` is now
  `Box<Mor>` for Rust callers.

- Updated the Rust and JavaScript desktop dependencies together, including
  Tauri and its plugins, and refreshed jsonschema, cc, and Vite.

- Participant parsing admits selected source ranges before decoding names and
  roles. Recovery states are retained; unreadable producer ranges reject the
  entry as an internal failure rather than admitting a partial participant.

- **Breaking (low-level Rust parser API):** `parse_postcode_node` accepts a
  generated `SourceBound<PostcodeNode>` instead of a node and separate source
  string. Final-code lowering preserves source association and propagates
  range failures as internal failures. `ChatParser` APIs and CHAT/JSON behavior
  are unchanged.

- The primary parser preserves postcode source spans, allowing incremental
  relocation and diagnostics to retain the original token location. CHAT and
  JSON output are unchanged.

- **Breaking (Rust API):** `Utterance::new` starts with unknown provenance;
  `parse_health` is private and read through `parse_health()`. Parser adapters
  finish through accumulated `ParseHealth::finish_utterance`. Appending dependent
  tiers withdraws provenance and alignment caches. Programmatic producers use
  `ChatFile::validate_construction_with_policy` for checked typed construction,
  without reparsing CHAT. `ParseHealthState::Constructed` permits model checks
  but never source-byte splicing or a claim of parser-backed cleanliness.
  JSON remains unchanged and carries no runtime admission evidence.

- English decade generation now admits only multiples of ten in 0-90 shorthand
  or 1100-2990 full-year form. Other suffix-bearing inputs retain their exact
  spelling instead of acquiring a guessed plural year phrase.

- Generated typed-CST carriers support opt-in range admission, preserving
  recovery while making subsequent payload text reads infallible. `@Media`
  body lowering uses this boundary; CHAT and JSON policy are unchanged.
- **Breaking (generated Rust API):** selected repeat/optional elements carry
  an uninhabited `Absent` payload. Empty repetitions and optional `None` remain
  possible; an element already selected cannot independently become absent.

- LSP quick fixes no longer invent participant or language facts for E308,
  E504 or E507, matching the shared fix catalog. Diagnostic prose is no longer
  parsed to construct participant declarations; enter the actual facts explicitly.

- English ordinal generation preserves tokens outside its supported 0-9999
  range unchanged, instead of rewriting their suffix to `th`.

- English ordinal generation omits prose commas in thousands with remainders,
  retaining the existing conjunction convention. For example, `1234th` expands
  to `one thousand two hundred and thirty-fourth`, a spoken word sequence.

- **Breaking (Rust API):** `ReplacementWords::new` and `TryFrom<Vec<Word>>`
  reject empty lists; JSON admission enforces the same invariant.
  `Replacement::new` accepts admitted `ReplacementWords`. Use `into_vec` and
  checked reconstruction instead of `take`/`retain`, or edit elements through
  mutable slices. Single-word construction remains infallible.
- re2c replacements use the existing word grammar directly, retaining source
  spans instead of splitting/reparsing text or inventing plain words on failure.
  Empty and unclosed replacements are parser errors. Glued replacements receive
  a diagnostic on re2c's opening token, without copied tree-sitter recovery
  diagnostics. Historical E208 is deprecated; primary-parser E376/E342 rejection
  and the empty-replacement regression remain.

- **Breaking (Rust API):** `PauseTimedDuration::Parsed` now carries a checked
  `ParsedPauseDuration` with private fields and read-only `seconds()`,
  `millis()` and `as_str()` accessors. Construct through
  `PauseTimedDuration::new`; CHAT and JSON formats are unchanged.

- Diagnostic display enrichment preserves legitimate zero-width EOF locations
  instead of moving them back onto the preceding byte, including when the file
  ends with a newline.

- E306 empty-turn deletion now declines turns with dependent tiers, preventing
  their content from being silently reassigned to the preceding turn.

- E241 marker-spelling repairs now require an exact source-bound word. The
  existing spelling vocabulary remains authoritative; comment text and
  omission/shortening notation are not rewritten as plain markers.

- E750 group-edge repairs now require source-bound whitespace at the owning
  annotated group's content boundary instead of neighboring delimiter bytes.
  Complete space runs and nested groups retain recovery-repair admission;
  groups with unrelated structural recovery decline a proposal.

- E244 now reports adjacent stress markers once per affected word. Longer runs
  no longer create duplicate diagnostics and overlapping repair proposals.

- Duplicate-comma proposals now require exact source-bound comma tokens in a
  clean tier body, sharing token admission with semantic comma deletion.

- Duplicate-primary-stress repair now traverses source-bound stress tokens.
  An earlier isolated mark no longer hides a later duplicate run, and all
  separate primary-duplicate runs in the word are repaired together. Only
  duplicate tokens are removed; other stress positions and diagnostics remain.

- E259 comma-deletion proposals now use source-bound comma, tier-body and
  whitespace nodes. Removing an initial comma consumes its complete separator
  instead of leaving leading spaces; interior commas preserve their separator.
  The proposal remains a semantic change requiring user review.

- Missing-terminator fix alternatives now use a source-bound grammar ending
  for insertion, before final postcodes on main tiers. This prevents misplaced
  terminators and splitting CRLF line endings, and refuses tiers
  whose structural boundary cannot be established. Choosing the terminator
  remains a user decision; these proposals are not automatic repairs.

- E501 fix proposals now require source-bound, byte-identical repeated headers
  with no conflicting declaration of that kind. Conflicts, formatting differences
  and recovered structure decline a proposal. Complete CST header ranges replace
  physical-line guessing; the existing header-write restriction is unchanged.

- Structured dependent-tier recovery now retains generated source ownership
  through recursive traversal instead of accepting raw nodes and independent
  text. Utterance recovery slots likewise preserve their source-bound fields
  through read admission. Recovery classifications, conservative alignment taint
  and placeholder behavior are unchanged.

- Adding recovery taint no longer promotes unknown parse provenance (such as
  JSON-imported utterances) into partially clean provenance. Alignment remains
  unavailable until parser-backed provenance has actually been established.

- Unreadable CST source ranges in generic, dependent-tier and utterance recovery
  now report internal failure, not invalid CHAT or an encoding repair suggestion.
  Readable recovery diagnostics and conservative alignment taint are unchanged.

- Morphology tier-body, main-word and post-clitic parsing now consumes the
  generated canonical-grammar admission proof. Impossible missing-composite
  states are removed from these boundaries; lexical and structural recovery,
  absent children and internal-failure reporting remain distinct.

- Fix planning no longer invents participant declarations, roles or default
  languages for E308/E504/E507. Supply those facts explicitly. E604 removal
  uses the complete typed GRA tier, preserving intervening dependent tiers and
  handling continuation lines; multiple targets refuse selection. E306 likewise
  uses the typed main-tier boundary rather than a line-prefix guess and refuses
  grammar-recovered main tiers.

- `splice::catalog_fix` now borrows `ParsedSource` instead of accepting raw
  source text. Retain the owner from `TreeSitterParser::parse_chat_file_with_source`
  and pass diagnostics from the same input. W109 planning reuses this CST instead
  of creating a parser and reparsing for each diagnostic. Recovery admission and
  changed-output verification are unchanged.

- Speaker-identification input-rejection reports now use the distinct
  `incomplete_validation` failure category when parser provenance prevents
  complete validation. Previously this was collapsed into `validation`.
  Consumers of the typed enum or JSON failure category must handle the new
  variant; no match evidence or CHAT-invalidity claim is inferred from it.

- Word spelling is derived from typed structure, including the JSON `raw_text`
  field. Imported computed spelling cannot override content. Rust
  `Word::new` and `new_unchecked` no longer accept a separate raw argument;
  `raw_text()` returns an owned string and `set_raw_text` is removed. Use
  typed builders for markers and streaming `WriteChat`/`Display` when appropriate.
  Original source bytes must be read from the source, not from derived spelling.
  External token producers should handle fragment-parser refusal instead of
  falling back to unchecked construction. Shortening/embedded-marker errors
  retain grammar diagnostics without duplicate raw-spelling rescans.

- Number conversion uses exact lexical entries for languages without a
  language-specific composer. Unsupported numerals, currency and number groups
  remain unchanged instead of using generic multiplication or partial rewrites.
- Transcript construction rejects malformed media source spelling before
  reducing local paths, including malformed discarded directory components.
  Admitted remote URLs remain unchanged.

- Generated source-bound `extract_admitted` APIs accept compiled-language
  evidence and exclude Missing only for proven nonterminal slots. Participant
  and language headers, document/utterance and main-tier/body/ending reconstruction
  use this admission; lexical Missing, Error, applicable
  Absent and source-read failures remain supported. Raw extraction stays broad.
  `ReconstructionFault` adds `GrammarBinding` for failed producer admission;
  downstream exhaustive matches must handle this as an internal tool failure.
  Low-level pre-begin and dependent-tier dispatch now consume generated admitted
  choice types. Document recovery source-ownership failures report E001 rather
  than classifying a producer fault as invalid CHAT.

- Generated repeat elements and present optional elements now use selected-slot
  types with uninhabited `Absent` payloads. Consumers can eliminate impossible
  absence diagnostics while retaining MISSING/ERROR recovery, nested fixed
  positions, empty repeats and optional `None`. Kind-preserving projections
  retain the slot's absence type.

- Low-level `%pho`/`%mod` parsing now requires source-bound generated nodes
  and returns `CstFailure` for source/reconstruction failures. Compound spelling,
  grouped content, recovery and string fragment APIs are unchanged.

- Low-level `%sin` parsing now requires a source-bound generated node and
  returns `CstFailure` for source/reconstruction failures. Sign-group fallback
  and empty-token policies remain; string fragment APIs are unchanged.

- Low-level `%gra` parsing requires a source-bound generated node and returns
  `CstFailure` for source/reconstruction failures. String fragment APIs, numeric
  admission, recovery and relation completeness accounting are unchanged.

- Low-level `%mor` tier parsing now requires a source-bound generated node
  instead of a node plus independent text. Bind through the existing
  `ParsedSource` owner; string fragment APIs are unchanged. Recovery and
  placeholder handling are retained. Word/post-clitic/feature readers retain
  the same source association; producer faults propagate separately from
  missing morphological content.

- Generated `ChoiceSlot` makes `Unexpected` uninhabited: selected-choice
  extraction consumes its retained match plan and returns producer faults
  through `ReconstructionFault`. Parser consumers no longer invent CHAT
  diagnostics for that impossible choice state. Missing, Error, Absent,
  displaced nodes and supertype-classification recovery remain supported.

- Source-bound bullet/text tier adapters return `CstFailure`, preserving
  source-binding failures alongside reconstruction faults without substituting
  empty content. The nested bullet-content reader propagates the same failures
  rather than returning a shortened segment list. High-level fragment parser
  signatures are unchanged.

- `ChatDate::Valid` now holds `CheckedChatDate` with private fields. Construct
  dates through `ChatDate::from_text` or `new`; match `Valid(date)` and use
  `day()`, `month()`, `year()` and `as_str()` instead of field access. JSON
  remains a string; format/day-range admission is unchanged (not calendar validation).

- Generated concrete CST extraction now returns `Result<_, ReconstructionFault>`;
  ERROR-root extraction returns `Result<Option<_>, _>`. Source-bound match plans
  retain selection decisions for consumption instead of independently rematching.
  Affected public header and tier adapters also return `Result`; callers must
  propagate producer failure, not substitute empty carriers. `DocumentRoot::classify`
  now returns the publicly exported `CstFailure`.
- E001 is classified as `DiagnosticKind::InternalFailure`, not CHAT invalidity.
  Validation/admission and transformation pipelines preserve this distinction.
  CLI file records add `internal_failure`, summary records add `internal_failures`,
  and desktop events carry `internalFailure`/`internalFailures`. Failed attempts
  are neither valid nor invalid and never enter the validation cache. Producer
  faults during optional roundtrip reparsing also remain internal failures,
  rather than mismatches, and write neither validation nor roundtrip cache entries.
  Legacy checked node/source reads report E001 for incompatible ranges or UTF-8
  boundaries instead of misclassifying those API faults as CHAT errors.

- Flagged reference-first merge drafts return ordering uncertainty exclusively
  through `DraftOrderReview` records; they no longer insert generated review
  `@Comment` lines. Contributor comments, source ordering and strict-policy
  refusals are unchanged. Reviews are also accessible on unvalidated `MergeDraft`.
  `before_output_utterance` now has type `OutputUtteranceBoundary`; use
  `utterances_before()` for its count rather than treating it as a line index.

- `ParseErrorBuilder` requires message and location in its type state before
  `finish()`, which now returns `ParseError` directly. Removed
  `ParseErrorBuilderError` and `try_finish()`; callers must supply both fields
  and remove result/option handling around finishing. Streaming source-range
  rejection now returns the original parse diagnostic, not a parser-creation
  error or a fabricated empty document.
- Spanish cardinal expansion uses bounded hundreds/thousands composition,
  correcting `100000` to `cien mil`. Unsupported noun scales and unresolved
  agreement preserve the original input rather than inventing a phrase.

- Lexical Unicode validation now rejects private-use scalars and noncharacters
  across all planes instead of copying CHECK's high-BMP blacklist and private
  markup exemption. Ordinary compatibility characters are no longer rejected
  by that range rule; control-character checks are unchanged.

- Retrace joining carries admitted utterance ownership into mutation instead
  of raw line indices. A forward scan preserves header barriers and chain
  repairs without repeated removal of later lines; repair policies are unchanged.

- Vector and small-vector semantic differences share one sequence comparison
  policy, preserving bounded-report ordering and unmatched-tail diagnostics.

- `ErrorContext::from_source_with_span` now returns `Result<_, SourceExcerptError>`.
  Invalid source slices and unrepresentable snippet coordinates are refused,
  rather than replaced by an empty context. Valid zero-width ranges remain valid.

- Context label fields are private. Construct `SampleTypeLabel`, `RoleLabel`
  and `ConsentTierLabel` through `TryFrom<String>`; read them with `as_str()`.
  Corrected sample-type verdicts share this nonblank admission boundary.

- Standalone-word conversion and `%wor` parsing require producer-bound nodes,
  not separate node/source pairs. Main-tier words, replacement words, timed
  words and standalone fragments preserve their source association to conversion.
- Main-tier CST conversion now requires a producer-bound `MainTierNode`, not a
  typed node paired with independent source text. Fragment, utterance, and EOF
  recovery callers retain that association through conversion.
- Main-tier contents parsing now consumes a bound `ContentsNode`; the shared
  contents/group cycle retains generated source association through recursive
  group and quotation dispatch, including recovery placeholders.
- Speaker-mapping strings now refuse repeated source assignments, including
  identical repeats, instead of silently keeping the last assignment.
- `AdjudicationError` adds `InvalidDecisionMapping`; exhaustive library matches
  must handle this refusal when a rename lacks its required adult role.

### Fixed

- Transcript construction preserves admitted HTTP/HTTPS media references
  verbatim instead of reducing them to local filesystem basenames.

- Line-map end lookup clamps extreme out-of-range line indexes to source EOF
  instead of overflowing its next-line calculation.

- Empty source spans no longer report overlap with a surrounding range;
  diagnostic insertion points remain distinct from byte-covering highlights.

- `ParseError::internal` now emits E001 (`InternalError`) rather than a CHAT
  syntax diagnostic, preserving failure-versus-invalidity admission even if
  presentation severity is downgraded.

- Display-span normalization clamps extreme out-of-source coordinates before
  interpolation, avoiding integer overflow while retaining UTF-8-safe spans.

- Structured bullet timestamp readers now retain the producing source through
  all four consumers and distinguish failed source admission from missing fields.

- Main-tier body lowering propagates failed content/ending reads as internal
  failures and preserves source ownership into utterance-ending extraction.

- `%wor` bullet handling retains internal reconstruction failures instead of
  silently treating them as unavailable word timing.

- Header lowering no longer substitutes an unknown comment or a partial
  participant name/role after an internal source-read failure.

- Rebasing a `ParseError` now preserves its independently owned context text
  and relative highlight instead of shifting that highlight out of its snapshot.
  Document locations and secondary labels continue to move together.

- Dependent-tier fragments reject trailing tiers or speech instead of returning
  the first tier and silently discarding the rest of the caller's input.

- Single grammatical-relation, phonological-word and participant-entry fragments
  reject extra items instead of silently discarding them. GRA and PHO fragments
  no longer append synthetic content; empty-input errors retain caller offsets.

- Single MOR-word fragments reject multiple items and post-clitics instead of
  silently returning only the first main word. Refusal diagnostics retain caller
  coordinates; complete MOR-tier parsing still accepts those structures.

- Lenient transform parsing no longer hides diagnostics on tiers whose names
  merely start with `mor` or `gra`. Suppression uses actual parsed generated-tier
  ownership and retains unlocated or cross-tier diagnostics and alignment taint.
- Shortening validation uses source-bounded, nonnegative nesting depth, avoiding
  signed-counter overflow on very large imported spellings while retaining
  unmatched-closing and unclosed-opening diagnostics.
- Judgment context consumes typed header ages instead of reparsing raw text.
  Unsupported ages no longer yield a numeric age from a valid-looking prefix;
  bounded components prevent arithmetic overflow, and explicit ages skip fallback.
- Speaker-sample head/tail limits no longer overflow for large budgets. Selection
  is bounded by available turns and overlapping windows never duplicate speech.
- Adjudication library failures now preserve the failing request and every later
  pending request in order. Retrying does not reapply already accepted decisions.
- Adjudication admits mapping conversion before recording a decision. A rename
  without its required role is refused without consuming the pending request;
  the CLI exits with status 2 and leaves its files unchanged.

## [0.26.0] - 2026-09-24

- `spec-perturb` emits explicitly unreviewed candidates, not stale expected
  diagnostic labels. Its JSON replaces `expected_error` with
  `assessment: "unreviewed"`; reviewed spec claims remain the golden authority.

### Changed

- Validation and roundtrip cache keys retain native path identity instead of
  hashing lossy display text. Equivalent separator spellings now reuse the
  same cache fact, including mixed Windows separators; existing cache entries
  may be relearned without deleting the cache.

- English cardinal generation composes short-scale units correctly instead of
  multiplying complete table phrases (for example, 2000 now yields "two
  thousand", not "two one thousand"). Authored reference controls cover scales
  through the `u64` boundary; other languages' generation policies are unchanged.

- Bullet-text tier adapters and `parse_bullet_content` now require producer-bound
  nodes rather than separately supplied source text. `BulletTextNode` carries
  the source lifetime as well as the tree lifetime. Generated child-range
  admission and recovery diagnostics remain; this is source association, not
  a guarantee of valid CHAT.
  Inner text/bullet/picture choices and leaves now preserve the same binding;
  the segment sink no longer accepts or stores independent source text.

- A generated missing-TAB recovery node in a header separator now reports
  specific E303 instead of generic E342, including multiword header names.
  Other recovery nodes remain diagnosed; recovered text is not treated as valid.

- Overlap analysis retains each anchor's original main-tier span; orphan
  diagnostics no longer reconstruct it through a second utterance-index lookup.
  `OverlapAnchor` exposes `utterance_span()`. Its origin and the per-utterance
  origin are private, so callers obtain these records from `analyze_file_overlaps`
  rather than struct literals. Matching and diagnostic-location policy are unchanged.

- Prefix-marker language validation no longer guesses a disallowed language
  when word-language resolution is unavailable. Missing-header diagnostics
  remain, and an explicit disallowed word language still reports E763.

- The public `LeafContent` traversal view now distinguishes underline opening
  and closing markers with their optional source spans. Exhaustive downstream
  matches must treat both as notation. Underline validation uses the shared
  structural owner, preserving replacement targets and source locations without
  maintaining a separate container list. CHAT and JSON formats are unchanged.

- Incremental editor parsing now uses parser-owned source revisions. Cached
  trees cannot be replaced independently of their source, and the parser
  derives edits from the actual previous revision before reuse. Raw-tree
  compatibility APIs retain their caller obligations and recovery checks.

- Phon `%xphoint` media-bounds validation no longer overflows at maximum
  timestamps. Its 1 ms tolerance and separate invalid-interval diagnostics remain.
- E704 includes timed untranscribed speech (`xxx`, `yyy`, `www`) when checking
  same-speaker overlap, matching CHECK. The 500 ms tolerance is unchanged.
- Media-header lowering retains producer-bound source identity through filename,
  type and status fields, borrowing checked text without temporary payload
  strings. Existing recovery and validated filename admission remain in place.
- Scalar/text headers and the single-value number, recording-quality and
  transcription headers now retain the same source-bound payload association,
  with borrowed text and existing malformed-header recovery preserved.
- E220 rejects bare numeral words even in languages that permit embedded tone
  or homonym digits. Omission notation and unresolved-language policy are
  unchanged; numbers must be written out according to their pronunciation.
- Chinese number spelling no longer duplicates the zero between skipped
  four-digit groups (`100000001` becomes `一亿零一`, not `一亿零零一`).

- Desktop keeps read, parse and roundtrip failures visible even without CHAT
  diagnostics. File details, completion summaries, window titles and
  notifications no longer misreport these failures or cancelled runs as valid.
  Concurrent update triggers share one check/prompt/install operation, and a
  failed native error dialog cannot reject the best-effort update command.
  The parser selector explicitly labels re2c experimental and incomplete.
  Plain-text exports now retain every file's status and failure reason, including
  failures without diagnostics; malformed export records refuse before writing.

- W109 now warns when both media and transcript names use the same non-NFC
  spelling. Disk validation and fixing resolve the stored directory basename,
  rather than trusting a normalization-equivalent argument spelling. Failed
  resolution is an explicit I/O error. `chatter fix --code W109 --apply` can
  normalize only the media filename token; it never renames files or changes
  remote URLs, and a file-side warning may remain after a successful fix.

- `@Time Start` reports E541 for out-of-range clock components, matching the
  existing duration bounds: hours 0-23 and minutes/seconds 0-59. Both accepted
  and rejected values retain their original spelling. Diagnostic rendering
  requires model-issued refusal evidence.

- Bracketed content owns all inter-item spaces. A leading timing bullet no
  longer inserts a spurious space after an opening bracket; standalone
  bracketed-item serialization emits the bullet payload without a separator.

- Main-tier timing bullets before any utterance material now report E770,
  including bullets inside retraced groups and after linkers. Explicit zero,
  words, events and pauses establish material. Parse-recovered main tiers do
  not produce this absence claim; timing evidence is retained.

- Main-tier lowering preserves internal timing bullets before a terminator and
  alongside a distinct terminal bullet. Unterminated tiers transfer only their
  final bullet to the terminal slot, retaining earlier timing scopes instead
  of silently discarding them. Missing-terminator diagnostics remain unchanged.

- Media filename comparison uses explicit equality, mismatch and canonical-
  equivalence outcomes. Unicode normalization advice always identifies an
  actual noncanonical side; existing filename-matching behavior is preserved.

- Standalone slash words now report E243, including nested and replacement
  words. Repetition annotations `[/]` and free-text `%com:` slashes remain
  valid; validation preserves the source rather than guessing a repair.

- Generated CST extraction preserves source identity through document recovery,
  child slots and choice projections. Document/header dispatch, the utterance
  entry point, dependent-tier dispatch and participant lowering use these
  capabilities; participant text reads no longer accept an
  independent source string. Recovery states and range admission remain checked.

- Generated single-node CST choices preserve an admitted source range through
  variant selection. Dependent-tier attachment admits its choice once and
  shares that proof with dispatch and parse-health classification, replacing
  per-variant range checks without changing recovery policy.

- Compound validation requires spoken material in every part. Stress or other
  nonlexical markers no longer hide an empty first, middle, or final part;
  E232/E233 report the defect without modifying the original word.

- Unicode ellipsis in word text now reports E243, including nested and
  replacement words. Original source text is retained; the valid CHAT
  trailing-off terminator `+...` is unchanged.

- Roundtrip difference reports only say "and more" when another difference
  exists. Present lines are quoted and kept distinct from missing lines.

- Main-tier semicolons now report E769, matching current CHAT and CHECK 48.
  Legacy separator syntax remains parseable and roundtrippable; nested and
  retraced semicolons receive the same source-positioned diagnostic.

- Removed an unused annotation argument and unreachable exclusion check from
  extraction's ordinary-word helper; the scoped walker remains the exclusion
  owner. Canonical category specs retain extraction/validity separation.

- Expanded E220 specs with mixed/ambiguous language controls and paired
  candidate substitutions, preserving the existing digit-permission policy.
  Scoped examples additionally verify language precedence through NLP
  extraction, with the observed CHECK discrepancy documented separately.

- Corrected reserved bullet-rule examples and documentation to preserve the
  adopted default timing policy; optional CHECK continuity checks remain
  documented divergences, not implemented Chatter requirements.

- Timed pauses no longer overflow when converting minutes to bounded seconds.
  Unrepresentable numeric projections retain their original spelling as
  unsupported values through CHAT and JSON roundtrips.

- Comma licensing now follows nested content in document order, reporting
  commas before spoken content inside groups while retaining omission policy.

- Cross-utterance quotation/completion checks now consume file-issued positions
  instead of independent indices, retaining real first/last-position behavior
  while eliminating silent invalid-index exits.

- Expanded canonical language-context controls and mutations to verify bare
  shortcut resolution and unresolved metadata without fabricated fallbacks.

- Expanded canonical underline controls and marker-deletion cases to grouped
  standalone markers and nested replacement text, including JSON provenance.

- `@Date` and `@Birth` reject leading plus signs in day/year components;
  fixed-width ASCII-digit admission replaces permissive integer parsing.
  Date construction and JSON decoding share this admission with validation,
  preserving malformed spellings as unsupported values.

- Replacement words retain their source wrapper span and reject a missing
  separator after the replacement or its trailing scoped annotations.

- Bracket-to-word spacing validation now descends into retraces, groups and
  quotations, retaining sibling boundaries and exact source locations.

- Timed pauses using minutes and seconds now parse on `%mod` and `%pho`,
  matching main-tier pause syntax. ASCII colons remain invalid in ordinary IPA words.

- Phon reconstruction checks share the alignment mapping's independent
  `%mod`/`%pho` positions, preventing false errors after one-sided pauses.

- `OverlapMarkerIndex::new` is now fallible and admits only 1-9; JSON decoding
  enforces the same range. The unused post-construction validator is removed.
  Scoped overlap token decoders reject malformed indices instead of silently
  changing them into unindexed markers.

- Ambiguous word-language markers validate every candidate ISO code, using
  the same E519 rules as explicit and mixed markers. Undeclared but valid
  word-level language codes remain permitted.

- Opaque postcode labels no longer trigger quotation-balance errors when
  their text resembles quotation syntax. Actual quotation checks are unchanged.

- Non-ASCII speaker IDs are consistently invalid. Typed validation and
  main-tier recovery report E307; recovery no longer mislabels this syntax
  fault as an undefined speaker (E522).

- Bracket recovery recommends adding a separator only when its retained
  source actually lacks whitespace; separated malformed annotations retain
  their rejection without misleading spacing advice.

- Invalid-control-character diagnostics acknowledge permitted underline
  markers rather than describing only standalone CHAT delimiters.

- `@Options` lowering uses generated typed flag slots, preserving ordered
  supported and unsupported values and retaining empty/missing-name recovery.

- Recovery diagnostics no longer recommend the retired `[x N]` repetition
  notation. Recognizable legacy counts suggest explicit repeated speech with
  `[/]`; fragmented bracket errors retain their less-specific diagnostics.

- `OffsetAdjustingErrorSink` projects secondary labels as well as primary
  locations, clears stale line/column coordinates, and preserves independently
  indexed source context instead of replacing it based on text length.
  `RebasedErrorSink` likewise clears cached primary line/column coordinates
  after translating document offsets; unlocated spans remain unlocated.

- The re2c backend retains scoped annotations and ordered retrace chains on
  quotations, including quotations nested inside other groups.

- Bare `@G` lazy gem markers now parse without a colon or label, as specified
  by CHAT. Labels remain optional typed groups; recovery diagnostics are retained.

- Splice admission refuses replacements crossing utterance boundaries; a clean
  starting point no longer licenses changes extending into another utterance.
  Multi-tier replacements within the same utterance remain admissible.
- CHAT construction preserves recognized media types, including `missing`,
  and refuses unsupported declared types instead of changing them to audio.
  Omitted media types retain the documented audio default.
- CHAT construction rejects partial timing pairs and timing attached to empty
  main-tier text instead of silently discarding supplied timing.
- CHAT construction applies declared `@Options` to utterance parsing, preserving
  CA omission semantics before serialization. Its private build context owns a
  nonempty language declaration and the contextual parsing operation; header
  and utterance construction borrow their inputs from that same description.
- Diagnostic contexts decode an omitted `expected` list as empty, matching
  their existing serialization. Computed alignment metadata containing such
  diagnostics can now roundtrip through JSON without inventing parse provenance.
- Rediarization preserves header-only transcripts, including declared
  participants. Header pruning requires admitted nonempty track evidence,
  preventing an invalid empty `@Participants` header when there is no speech.
- **Breaking (transform API):** `rediarize` now requires a diagnostic sink and
  rebuilds participant metadata through the canonical header join. Its returned
  model no longer loses participant records that reappear on parsing serialized
  output. `rediarize_content` refuses serialization on header-join errors.
- Sanitization uses delimiter-free placeholders for inline events, freecodes
  and other spoken events, preserving parseable CHAT instead of introducing
  nested bracket syntax. Documentation now distinguishes the supported fields
  from complete de-identification and identifies preserved metadata risks.
- Sanitization now redacts the nine documented free-text header payloads beyond
  `@Comment`, retaining header kinds and speaker references. Exhaustive typed
  dispatch requires an explicit policy for every future header variant.
- Coordinated `%mor`/`%gra` replacement validates and exclusively borrows the
  host range before mutation; reversed/empty ranges and short grammatical tiers
  are refused atomically. Single-item replacement shares the same checked path.
  **Breaking (model API):** `CoordinatedMutationError` adds `InvalidItemRange`;
  single-item replacement now also refuses donor heads outside the new block.
- Diagnostic highlights stay within their display text at UTF-8 boundaries,
  including zero-width positions on empty lines and at end of text.
  **Breaking (model API):** `PlainDisplayResult::text` is private; use `text()`
  or consume the mapping with `into_text()` so text cannot drift from offsets.
- **Breaking (model API):** `UnderlineMarker` stores an optional private source
  span exposed by `span()`. Use `from_span`/`with_span` instead of struct literals;
  source-independent and JSON-decoded markers now explicitly have no location.
  Underline diagnostics still fall back to their enclosing word or tier span.
- Underline marker decoding and JSON Schema generation share an explicit wire
  type, so schema-checked conversion accepts the existing null word-marker
  payload as well as internally tagged markers without changing serialized output.
- Language-switch declaration updates retain the selected header's typed
  mutable collection instead of indexing and checking its kind again; missing
  headers and last-header selection keep their existing behavior.
- Identity language retagging preserves declarations and returns unchanged
  statistics, including files with span notation that would block a real rename.
- Media reconciliation retains an exclusive header borrow while proving it is
  the sole declaration, eliminating a second search and redundant missing-header
  path while preserving exact duplicate counts.
- Media reconciliation recognizes internal main-tier bullets through the typed
  recursive content walker; timed documents can no longer be certified untimed
  merely because their bullet is not utterance-final.
- Main-tier recovery collection belongs to its admitted source-bound fragment;
  callers can no longer supply independent nodes, source strings or offsets.
- Participant entries own their canonical CHAT serialization; whole headers
  delegate to the same writer used by standalone fragment clients.
- Legacy utterance probing uses checked source admission and propagates parser
  failure; classified input owns its complete envelope or required scaffolding.
- Internal bullet-text carriers now enter only through generated typed nodes;
  removed their redundant raw-node classifier and unused raw projection.
- Unclosed-delimiter findings retain their admitted recovery node and text;
  diagnostic conversion no longer accepts an independent node or span.
- Re2c text-tier admission diagnoses and omits all-zero inline bullets, sharing
  the policy across full-file and offset-aware fragment entry points.
- E360's specification distinguishes undelimited text from actual media bullets
  and includes a parse-backed inline all-zero timestamp rejection fixture.
- Top-level dependent-tier and unknown-header recovery consume producer-bound
  source slices; binding now precedes dependent-tier diagnostics and tainting.
- Generic file-error analysis also requires a bound source slice, including the
  line-slot recovery route, rather than independently supplied node and text.
- Document and line-slot recovery share one source-binding diagnostic boundary;
  a real-node regression rejects foreign trees even with identical source text.
- Malformed dependent-tier routing admits nonempty labels without manual byte
  indexing; exact delimiters and conservative unknown-label taint are preserved.
- Main-tier prefix decoding retains generated kind proofs for MISSING slots;
  speaker and colon diagnostics consume the corresponding typed nodes.
- Inline-picture decoding admits a nonempty, delimiter-checked filename before
  ownership conversion; incompatible source ranges still reject diagnostically.
- Inline bullet times retain their all-zero-pair policy through a private
  validated value, with separately parsed timestamp-boundary tests.
- Phonology fallback retains its generated group type through checked source
  admission, preserving fallback text and rejecting incompatible ranges safely.
- Word and main-tier fragments retain generated typed nodes and source identity
  in one sealed source binding, instead of independent node and slice fields.
- Main-tier fragment admission derives original input from its parsed envelope,
  removing independent source/input pairing and checking capacity before allocation.
- Main-tier rejection carries producer-issued evidence of an emitted diagnostic,
  eliminating the fragment consumer's diagnostic-free rejection fallback.
- Non-colon separators consume generated typed placeholder admission directly;
  unclassified-placeholder and other recovery diagnostics remain distinct.
- Marked-token decoding checks source ranges before UTF-8 admission, reporting
  incompatible sources instead of indexing outside them; marker refusal remains explicit.
- Language-list recovery retains offending nodes with their grammatical roles
  and shares one diagnostic constructor for code-slot and repeated-group faults.
- Participant list and entry recovery likewise carry typed fault roles into a
  shared reporter, preserving their distinct diagnostic codes and contexts.
- Sign-tier token decoding shares a typed word/group source transition;
  whole-group fallback retains its generated kind and checked source boundary.
- Morphology feature values retain generated kind proofs through MISSING
  recovery and check source ranges before decoding, preserving refusal diagnostics.
- Word-recovery fragment diagnostics derive their spans and display contexts
  from the admitted recovery value instead of independently paired node/text.
- Generic recovery uses the same admitted fragment diagnostic constructor,
  preserving full-source and subspan policies at their separate boundaries.
- Annotated-group recovery retains its typed main-tier body ownership, so a
  missing form suffix inside a retraced group reports E202 rather than E316.
  Paired specification seeds and deliberate mutations cover form suffixes and
  scoped annotations after replacements.
- Phonology and sign groups use the same typed body-owned recovery path;
  paired specification mutations preserve E202 for missing form suffixes
  regardless of which group contains the word.
- Timing-tier source spans now participate in derived rebasing for parsed,
  unsupported and empty content, preserving document coordinates through
  dependent-tier fragment APIs. Corpus-backed tests cover full-line and
  content-only dependent-tier parsing.
- Document lowering consumes source-ordered document/recovery parts; duplicate
  `@End` retains E501 even without a final newline. Reconstructed error wrappers
  retain generic whole-input recovery for siblings whose document role is unknown.
  The internal lowering handoff replaces the public `DocumentRoot::into_children`
  projection, which discarded outer recovery.
- Generated repeat selection now retains its extraction cursor through a
  consuming capability; leading extras and all recovery states remain preserved.
- Utterance construction admits a readable typed main tier before conversion;
  incompatible source ranges diagnose and taint the main tier instead of panicking.
- Utterance recovery also requires readable source admission, conservatively
  tainting alignment domains when no tier label can be read.
- CA element and delimiter decoding retains generated node types through a
  shared checked-source boundary; incompatible ranges reject without panicking.
- Dependent-tier recovery consumes the shared readable-source carrier before
  classifying text, retaining its diagnostic families for malformed input.
- First and repeated language codes share typed slot admission, preserving
  positional diagnostics and all producible recovery states.
- Participant slots likewise share admission; only the first-slot position
  carries the enclosing header needed to diagnose a wholly absent entry.
- Empty-POS morphology diagnostics retain the recognized token's occurrence,
  avoiding an earlier identical split tail when selecting the error span.
- Generic and word recovery diagnostics share a validated readable-source
  carrier; incompatible node ranges produce diagnostics instead of panicking.
- Wrapped-header selection descends through producer-bound source slices.
  Header admission takes its parsed wrapper and ordinal, rather than an
  independently selected node; complete-input and header-counting policy remain.
- **Breaking:** Generated wrapper/supertype child slots are now `KindSlot`.
  Their MISSING payload retains its proven kind; use `known_or_placeholder`
  for typed recovery. Raw and composite slots retain fallible classification.
  Removed foreign-kind-placeholder branches only where the new producer type
  rules them out; ordinary ERROR, absence and displaced recovery remain.
- **Breaking:** `DocumentRoot::classify` now accepts a producer-owned
  `ParsedSource` rather than a bare tree. Use
  `TreeSitterParser::parse_source_incremental` to obtain it. Document lowering
  derives source text from this owner; raw tree access remains available through
  the consuming `into_tree` transition and `parse_tree_incremental`.
- Other-speaker events consume generated typed slots rather than positional
  child assertions. Failed transitional text reads reject instead of constructing
  successful empty values; required-slot recovery remains explicit.
- Standalone word and main-tier fragments retain producer-bound source slices
  through admission and lowering. They share the document parser's 32-bit range
  admission and `ParseFailed` diagnostic when tree-sitter cannot produce a tree.
- Generated kind classifiers cover nested, kind-disjoint choices. Content
  recovery uses that producer instead of a hand-written alternative list, and
  content dispatch retains its typed node rather than rechecking a raw copy.
- Wrapped header admission derives both its tree and its original-input mapping
  from one parsed wrapper owner; lowering retains a checked source slice.
- Shared header field decoding retains typed nodes through checked range and
  UTF-8 admission. Language-code admission returns the validated model value,
  without an independently supplied node/text pair.
- Main-tier separator dispatch retains its generated `SeparatorNode`, removing
  raw-node kind checks and bare-leaf paths outside the producer's alternatives.
  Recovery inside separator nodes is unchanged.
- Base-content, pause, overlap and standalone-word lowering retain generated
  node types through dispatch. `%wor` uses the generated word-wrapper extractor
  instead of a positional child read. Word conversion still rejects MISSING
  placeholders; nested recovery slots remain explicit.
- Bullet-capable text retains its generated carrier and segment choices through
  lowering, including nested trailing spaces and picture alternatives. Structured
  bullet readers require `BulletNode` and use generated timestamp fields with
  closed start/end roles; range failures reject without indexing panics.
- Unclosed-delimiter findings retain the text that established them. Removed
  a shadowed lone-bracket diagnostic and redundant empty-text checks without
  changing recovery priority.
- Main-tier displaced-body reporting derives its sink and grammar context from
  sealed typed carriers, removing independent node-slice, region and label
  arguments. Raw slot-level recovery keeps its explicit region policy.
- Linker decoding uses generated first/repeated groups and exhaustive typed
  alternatives. Recovered displaced linkers retain source order and token spans;
  the obsolete raw-kind membership helper and legacy wrapper path are removed.
  Postcode lowering retains `PostcodeNode` through checked text admission.
- Main-tier speaker admission retains `SpeakerNode` through checked text decoding.
  Its private admitted value keeps nonempty text with its source span; mismatched
  source ranges reject instead of panicking during prefix conversion.
- User-defined tier taint classification consumes the generated typed prefix
  slot instead of raw child zero. Only a readable Present `%xmod` prefix names
  the model-alignment domain; recovery retains conservative taint policy.

### Fixed

- Main-tier recovery classification admits a readable node/source slice before
  inspecting content, rejecting out-of-range and split-UTF-8 inputs without
  indexing panics. Existing marker priority and bracket recovery are preserved.
- Generated traversal cursors consume a bounded remaining-child iterator through
  their existing typestate transitions. Exhaustion cannot advance past EOF, and
  final recovery sweeps no longer substitute an empty tail for an invalid index.
- Grammatical-relation field reads reject out-of-source ranges without panicking;
  field roles retain their generated slot types and admitted indices are nonzero.
- Header-fragment parsing preserves `@Window`, `@Font` and `@Color words` as
  typed editor metadata, sharing the document parser's exhaustive pre-`@Begin`
  dispatch instead of silently lowering these headers as `Unknown`.

### Removed

- **Breaking:** removed the experimental `chatter merge`, `chatter pipeline`,
  and `chatter batch` CLI commands. Their structural transcript interleaving did not perform fuzzy event correspondence,
  repair diarization, or reconcile segmentation. The `talkbank-transform`
  structural merge APIs remain available; their reporting typestate is unchanged.

## [0.25.0] - 2026-09-15

### Added

- Source-bound donor selection for structural merging preserves original header
  boundaries after utterance removal or splitting. Selected donor origins map
  back to original parents; recorded header brackets constrain order without
  synthesizing utterance timestamps.
- Selected-donor merges accept source-bound relative order constraints
  (`RelativeOrderConstraint`, `with_relative_order`), an opt-in timed gem
  exterior policy (`with_timed_gem_exterior`; the placements it decided are
  reported as `GemExteriorPlacement`), an opt-in flagged draft order that
  serializes unresolved frontiers reference first with a review comment
  (`with_flagged_draft_order`, `DraftOrderReview`), and header-only references.
- `merge_chat_files_with_donor_selection_draft` returns a `MergeDraft` before
  validation. Its only edit, `set_terminal_bullet`, replaces an end-of-line
  bullet and is recorded as a `BulletEdit`; `Merged::bullet_edits` reports the
  edits after `validate`.
- `talkbank_model::validation` exposes `SPEAKER_OVERLAP_TOLERANCE_MS` and
  `has_transcribed_content`.

### Changed

- `MergeError::InvalidDonorSelection` reports inconsistent selection coordinates.
  Downstream exhaustive matches must handle this new variant.
- A repeated `@Languages` header is reported as E501.
- Source-order merges serialize utterances with exactly equal starts reference
  first; section markers at the same instant are refused as ambiguous.
- Merges refuse a missing, repeated or empty `@Languages` declaration in either
  input (`MergeError::InvalidLanguageDeclaration`), and selected-donor merges
  refuse a selected child whose bullet lies outside its parent's.
- `MergeError` adds `InvalidLanguageDeclaration`, `InvalidRelativeOrder`,
  `RelativeOrderTimingConflict` and `InvalidGemExterior`. Downstream exhaustive
  matches must handle them; the CLI reports each as a precondition (exit 2).
- Releases publish only after the desktop installers are built and verified;
  the draft carries the CHANGELOG notes, and the app banner is added after
  publication.
- Release artifacts are built with cargo-dist 0.33.0. Its shell installer keeps
  the `env` PATH helper beside the install receipt (by default
  `~/.config/chatter`) for flat installs, and moves an existing helper there.

### Security

- rustls is updated to 0.23.45 (RUSTSEC-2026-0285), which rejects TLS 1.3
  handshake messages accepted across encryption level boundaries. It reaches
  the CLI's self-update and LLM client and the desktop app's updater.

## [0.24.2] - 2026-09-12

### Added

- An explicit source-order merge policy derives a unique interleaving from
  source ordering and available timing evidence without synthesizing timestamps.
  Ambiguous ordering is refused; the existing timed merge policy remains the
  default.

### Changed

- `MergeError` includes `AmbiguousUtteranceOrder`; downstream exhaustive
  matches must handle the new variant.
- CLI release artifacts are published through an explicit release workflow
  dispatch.

## [0.24.1] - 2026-09-11

### Fixed

- A `@Media` declaration with the `missing` medium no longer triggers E544
  for absent timing. An explicitly absent recording does not promise linked
  media; expected recordings still require timing or an appropriate status.
- Added a specification example and generated regression fixture for the
  missing-medium declaration.

## [0.24.0] - 2026-09-10

### Changed

- Structural merge now consumes ordered AST streams rather than collecting
  headers and sorting utterances. Interleaved body headers and dependent-tier
  order survive; selected utterances without timing or with reversed source
  starts are refused. Section markers use neighboring source timing bounds,
  and ambiguous cross-source placement is refused instead of guessed.
- `MergeError` has new variants for timing, section-placement, metadata-order,
  participant-join and output-validation failures. Exhaustive library matches
  must handle them. `Merged` and `Reported` retain a validated document;
  `Reported::into_file` relinquishes that proof for subsequent edits.

### Fixed

- Documentation-date checks now include pending commit/squash changes before
  publication. The commit hook checks the actual index, preventing an unstaged
  repair or an older gate receipt from masking stale staged date headers.
- Merge builds the derived participant map through the canonical header join
  and validates the assembled AST, including tier alignment, before returning
  success. Callers no longer need serialization and reparsing to obtain a
  consistent participant map.
- Donor IDs extend the contiguous opening ID block; donor body comments remain
  at their source position. An opening ID after a comment is refused rather
  than silently reordered.

### Added

- `WorSlotMembershipPolicy::admits(&Word) -> bool`, the public per-word
  `%wor` admission predicate. `WorMainTierProjection::from_main` admits its
  slots through it, so it is the projection's own rule rather than a second
  statement of it. A downstream consumer that counts `%wor`-eligible words
  per content item (both Batchalign trees carried a hand copy of
  `counts_for_tier(word, TierDomain::Wor)` beside a `walk_words` for this)
  asks the policy and deletes the copy.

## [0.23.0] - 2026-09-09

### Removed

- `walk_overlap_points` and `OverlapPointVisit` from
  `talkbank_model::alignment::helpers`: a visitor over overlap markers with
  no caller in any tree, carrying a third private walk of its own (and a
  position convention for intra-word closing markers that differed from
  the collector's). `extract_overlap_info` is the one API.
- `talkbank_transform`'s corpus discovery and manifest API (`discover_corpora`,
  `build_manifest`, `corpus_summary`, `format_manifest`, `CorpusManifest`,
  `CorpusEntry`, `FileEntry`, `CorpusFileStatus`, `FailureReason`,
  `ErrorDetail`, `ErrorLocation`, `ManifestError`). No command in this
  repository and no known dependant
  used it; its only caller was its own test. `TierContent`'s
  `to_content_string_no_bullets` and `write_tier_content_no_bullets`,
  `ValidationContext::with_quotation_validation` and `with_bullets_mode`
  with the `bullets_mode` field they set (the `bullets` `@Options` was
  removed from CHAT and the flag was always false, so E362's check on
  bullet monotonicity now simply runs), and the parser API's always-false
  `bullets_mode()`; none had a caller anywhere. Breaking for a library user
  who called any of them.
- The `Utterance` builder helpers with no caller: `with_preceding_headers`,
  `with_user_defined` and every per-tier `with_*` except `with_mor`,
  `with_gra`, `with_sin` and `with_com` (`add_dependent_tier` is the one
  route they were sugar over and remains); the semantic-diff renderers `short_summary`, `short_summary_with_source`,
  `render_with_source`, `render_comparison`, `render_comparison_short` and
  `render_tree_diff` on `SemanticDiffReport` with the `RenderMode` they took
  and the tree renderer behind them, and the `Utterance` accessors `mor`,
  `gra` (the cloning aliases; `mor_tier` and `gra_tier` stay, as do `pho`
  and `sin`), `mor_tier_mut`, `computed_language_metadata`,
  `wor_alignable_word_count`, `pho_alignable_word_count` and
  `sin_alignable_word_count`. None had a caller in this repository's root
  workspace, in talkbank-tools or in the downstream Batchalign, and a
  whole-workspace coverage run showed every one unreached; the two cloning
  aliases had one caller in the spec runtime tools (a separate workspace),
  moved to the borrowing accessors. Breaking for a library user who called
  any of them; `SemanticDiffReport::render` (also its `Display`) remains,
  and the alignable count that is used, `mor_alignable_word_count`,
  remains.
- `TreeSitterParser::parse_utterance_cst`. It forwarded to the free
  `parse_utterance_node` and had no caller in this repository or in any
  repository known to depend on it; a whole-workspace coverage run showed
  it unreached. Breaking for a library user who called it; the public
  routes are `TreeSitterParser::parse_utterance` (one utterance from its
  text) and `TreeSitterParser::parse_utterance_fragment`.
- `talkbank_parser::parse_dependent_tier` (the free function) and
  `talkbank_parser::tiers::parse_mod_tier_from_unparsed`. Neither had a
  caller in this repository or in any repository known to depend on it. The
  first returned an untyped `UserDefinedTier` for every tier, where the
  `talkbank_model::ChatParser::parse_dependent_tier` route returns the typed
  `DependentTier`; a whole-workspace coverage run showed both entirely
  unreached, `%mod` parsing having gone through the typed `%pho` tier parser
  for as long as the typed traversal has existed. Breaking for a library
  user who called either; use the `talkbank_model::ChatParser` trait's
  methods.

### Added

- `talkbank_model::content::word::Word::new(NonEmptyString, WordText)`: the
  checked constructor, taking the two proofs a word's texts must carry. The
  tree-sitter parser builds through it; `new_unchecked` remains for test
  support and the front ends not yet migrated.

### Changed

- `talkbank_parser::generated_traversal::NodeSlot` is `NodeSlot<'tree, T, M, U, A>`:
  the payloads of `Missing`, `Unexpected` and `Absent` are the node (or
  `NoChild`) where the position can produce the state and the uninhabited
  `Never` where it cannot, and every generated accessor names its position's
  kind through one of four aliases (`ChildSlot`, `SeqSlot`, `ChoiceSlot`,
  `ClassifiedSlot`). `Absent` carries `NoChild`; `Recovery` and `SlotValue`
  carry the same parameters; `NodeSlot::view` gives a borrowed slot's
  states back by value as a `SlotView`, so a match through a reference can
  omit the arms the position kind rules out. Breaking for a library user
  matching the slot directly: an arm for a state the position cannot
  produce no longer compiles, which is the point. The parser's own
  hand-written arms for those states, each carrying a diagnostic for a
  case that cannot happen, are gone with it.
- A content-bearing recovery node inside a `%mor` tier is E702 at any
  depth. It was E702 for a direct child of the tier and E316 for a node
  below one, because the utterance parser walked every dependent tier's
  children before attaching it and reported them without the tier's name,
  and the typed dispatch then reported the direct children again: a `%wor`
  line with an unparsable word carried the same E316 twice at the same span.
  One reporter now walks the whole tier in the tier's words. Three E316
  examples and E711's first are E702's (`subsumed_by`), E702's first example
  is a `violates` claim, and the E342 text for a MISSING node inside a tier
  names the tier ("in gra tier").
- Counting and extraction take `PositionalDomain` (`Mor`, `Pho`, `Sin`), a
  new type with no `Wor`: `count_tier_positions`,
  `count_tier_positions_until`, `collect_tier_items`, `TierCountable`,
  `AlignableTier::DOMAIN`, and `talkbank_transform::extract::{extract_words,
  collect_utterance_content}`. `TierDomain` keeps `Wor` and stays the
  vocabulary of the walkers and of `counts_for_tier`; `PositionalDomain`
  converts into it, and `TryFrom<TierDomain>` refuses `Wor` with
  `NotPositional`. The `%wor` count and pairing are
  `WorMainTierProjection`'s, and the two `Wor` arms in the counter and the
  one in the extractor were a second implementation of that count, agreeing
  with the projection only by test; a probe over every reference-corpus
  file and every spec example found them equal before they were deleted.
  The overlap-marker position walk in `alignment::helpers::overlap` still
  counts on the `%wor` scale with its own traversal and is not changed
  here.
  `WorMainTierProjection::slots` is crate-private (pair through
  `bind_timing`). A downstream caller passing `TierDomain::Mor` to any of
  these writes `PositionalDomain::Mor`; one asking a `%wor` count calls
  `MainTier::wor_projection().slot_count()`. This is a breaking Rust API
  change.
- Two validity rulings (maintainer, 2026-09-08), both grounded on real CLAN
  CHECK. A bullet INSIDE a main-tier utterance is timing evidence for the
  media-consistency family: `hello \u{15}100_200\u{15} world .` is E752
  without an `@Media` header (CLAN CHECK 112 fires on it) and satisfies a
  declared `@Media` (it was E544 before, and valid without the header). And
  whitespace-only content on a bullet-payload tier (`%com`, `%add`, `%exp`,
  `%gpx`, `%int`, `%sit`, `%spa`, `%act`, `%cod`) declares nothing: E756
  beside E758, as `%eng` already was (CLAN CHECK 31 rejects the same lines).
  Transcripts that relied on either gap now validate differently.

- Diagnostics inside an angle group, a quotation, a pho group or a sin
  group are now the ones the tier body gives for the same material. The
  four constructs parsed their contents through a second, hand-written
  walker with its own generic ERROR analysis; the one typed `contents`
  walker serves both now. Visible changes: a curly single quote inside a
  quotation is E256 (it was E331); a stray `[` inside a construct is E316
  at that byte (it was three E330 "expected X" messages); an ERROR
  fragment that opens a bracket or parenthesis and never closes it is E312
  or E313 on the tier body as well as inside a construct (`hel(lo .` was
  E316; a parenthesis that opens the whole utterance still fails at file
  level), and "never closes" means no closer anywhere in the fragment, not
  merely not at its end. E331
  (`UnexpectedNodeInContext`) has no known route from CHAT input any more
  and is recorded as unreachable.

- `WordLengthening::count`, its `with_count` argument, and re2c's AST
  lengthening count now use `NonZeroUsize` instead of `u8`. JSON keeps the
  integer field and its omitted-one default, accepts longer runs, and rejects
  zero. This is a breaking Rust API and JSON-admission change.

### Fixed

- The phonological tiers accept superscript one, two and three (U+00B9,
  U+00B2, U+00B3), the only superscript forms those digits have, wherever
  the other superscript digits were already accepted, and the last three
  modifier tone letters of their block (U+A71D to U+A71F, the raised and
  low exclamation-mark letters, `ꜞ` among them) as the rest of the block
  already was. A `%pho` or `%mod` word carrying one was E316 unparsable
  content (TalkBank/chatter#6, #7): 52 of 52 sessions of one tone-language
  corpus, 18 of 96 in a second, 3 of 58 in a third, and 2 in a fourth.

- E715 and E734 no longer report a `%pho` or `%mod` tier one token long
  when the main tier carries a pause inside a `<...>` group: a pause was
  counted as a phonological token at the top level of the utterance but
  not inside a group, while the phonological tiers carry it in both
  places, as the Phon team's French corpora show at scale (72 records in
  49 sessions of one corpus, every one a pause inside an overlap or
  retrace group; TalkBank/chatter#5). The counting walker's own 2026-08-08
  note had held that arm open for exactly this evidence.

- E740 and E741 no longer report a `%mod` or `%pho` word that carries the
  linking tie `‿` (U+203F) as a mismatch against its `%xphoaln`
  reconstruction: the tie joins two symbols into one segment or marks the
  absence of a break, is never a phone, and has no alignment column, so
  the source word is compared modulo the tie as it already was modulo
  the stress and syllable-boundary marks (a pair side carrying a tie is
  not a bare segment and is compared as written). TalkBank/chatter#3,
  with the inventory in #4: until now every such word in two PhonBank
  corpora was a spurious mismatch.

- A group with no code after it (`<w> .`) is E342 alone, and the model
  keeps the bare group that was written. The grammar requires a code there,
  tree-sitter inserts a MISSING placeholder, and the annotation decoder used
  to read the placeholder's kind and build a full retrace nobody wrote: the
  validator then reported E757 and E370 against constructs the file does not
  contain, and the E342 spec example's roundtrip diverged. The decoder skips
  MISSING nodes now. CHECK-parity for CHECK 51 expects E342; the E342
  example leaves the backend-parity baseline, both parsers agreeing.
- E710 is reported only by the `%gra` relation parser. The dependent-tier
  recovery analyzer had a branch that fired on the substring `%gra:` anywhere
  in an ERROR node's text and called it E710, "non-numeric index": an `%eng`
  or `%x` body mentioning `%gra:`, or junk after a well-formed `%gra`
  relation, was reported as an invalid relation. The branch is gone; such a
  node is the generic E316 (E258 for a double comma, E760 for a `%mor` item
  with an empty part of speech, as before). The E760 branch's own gate
  accepted `%mor:` anywhere in the text for the same reason and now needs
  the line, the tier context, or a text that starts with the prefix.
- A MISSING node is reported once. The whole-tree recovery backstop
  suppresses a candidate already covered by a region diagnostic by span
  overlap, and widened only its own zero-width MISSING span to a byte, so a
  region's E342 for the same point (itself zero-width) never covered it:
  every MISSING node inside a dependent tier or a header list carried two
  E342 texts at one span. A zero-width region diagnostic at the same point
  now covers the candidate when it reports the same code; a different code
  there (E376 for an empty replacement beside its MISSING word segment)
  still leaves the E342 reported, as E208.md documents.
- Overlap-marker positions count a replaced word inside a group once. The
  collector behind `extract_overlap_info` (and so `top_onset_fraction`,
  `estimate_onset_ms`, and the cross-utterance E347 and E704 checks, which
  read the paired positions; E373 reads only the indices and was not
  affected) walked with two private
  traversals, and the bracketed one scanned a replaced word's replacement
  words too, so `<doggie [: dog]>` under a marker counted two words where
  the `%wor` projection counts one and every later marker position, and
  the onset fraction, drifted. The collector now walks with the shared
  `walk_content` at the `%wor` domain, the projection's own leaf set, so
  `total_words` is the projection's slot count by construction; a snapshot
  of every marker-bearing utterance in the reference corpus and the spec
  examples was byte-identical across the change, and the group case is
  pinned.
- `chatter debug sanitize` redacts `%act` and `%cod` tiers. Both carry the
  same bullet payload as `%com`, and passed through the strict sanitizer
  with their text intact under a comment deferring their redaction; an
  action line is free text about the participant. A parse-backed table of
  every dependent-tier kind through the sanitizer is the pin.
- `chatter debug sanitize` redacts the words on `%wor`. The tier repeats
  every main-tier word beside its bullet and passed through untouched, so a
  sanitized file with a `%wor` tier still carried the whole utterance in
  clear. The tier is now judged the way timing recovery judges it, before
  the main tier is rewritten: `WorMainTierProjection::bind_timing` for the
  counts, then `corroborate_wor_timing` for the words. A tier that
  corroborated the main tier has each word rewritten as its paired
  main-tier word's display text, now that word's placeholder (`w1w1` for a
  compound), so it corroborates the sanitized main tier exactly as before;
  a tier that drifted in count, or carried a word the main tier did not,
  takes fresh placeholders rather than a manufactured agreement, and
  disagrees after as it did before. Bullets are kept byte-exact, and
  sanitizing the output again reproduces it. The test that
  claimed to pin `%wor` offsets wrote them as bare `1000_1100` tokens, which
  the model parses as words; it passed only because nothing touched the
  tier. It now pins whole lines, drift and compounds included.
  `CountMatchedWorTimings::pairs` exposes the owner's pairing.
- `chatter validate --roundtrip` counts a file's roundtrip on every run.
  When the file's validity and roundtrip verdicts were both served from the
  cache, the summary said `Passed: 0` for a file whose roundtrip had
  passed, and `Failed: 0` for one whose roundtrip had failed: the cached
  branch built the file's status and never touched either roundtrip
  counter. The status now records whether the roundtrip ran
  (`FileStatus::Valid { roundtrip: RoundtripVerdict }`, a new public field
  and type) and both counters are derived from it.
- E370 (a retrace marker with nothing after it to retrace) is labelled at
  the marker's own bytes whatever the spacing around it. The rule used to
  find the marker by rendering the main tier back to CHAT text and taking
  the offset there, which was right only when the source was already
  canonical: on `<hello  there> [/] .` the label sat one byte early. The
  parser now records the marker token's own span on the retrace
  (`Retrace::marker_span`, a new public field, `None` for a retrace built
  without a source) and the rule reports there.
- Overlap markers inside an angle group, a quotation, a pho group or a sin
  group are kept in the model and written back. The constructs' old
  contents walker handed each item to a second walk over the item's
  children, and an `overlap_point` is a single token with none, so
  `<hello \u{2308} there \u{2309}> [/] hello there .` parsed clean,
  validated clean and wrote back with both markers gone.
- `%gra` structural diagnostics (E721, E722, E723, E724) no longer fire on a
  tier the parser had to shorten. A relation the model cannot hold is rejected
  and dropped, and the rules for sequential indices, root count and cycles
  describe the graph the author wrote, not what survived; E722 in particular
  reported "no ROOT relation" against a tier whose only surviving relation was
  a ROOT. A new `JudgeableGra` witness is the sole route to those rules and
  asks both questions that decide it, parse recovery and prior alignment
  findings.
- The re2c backend records parse recovery on a dependent tier it could not
  build as written: a `%gra` that lost a relation, a `%mor` whose conversion
  failed, a `%wor` body it could not re-lex. Cross-tier alignment previously
  compared such a tier against its neighbours and reported the difference its
  own recovery had created, so `%mor` and `%gra` counts disagreed (E720) on a
  transcript where they agree.
- Both parsers preserve lengthening runs beyond 255 colons without integer
  overflow or truncation. The default model marker now consistently contains
  one colon, with no zero-count repair during serialization.
- Re2c retains malformed form suffixes for specific E202/E203 diagnostics;
  repeated dangling markers no longer produce both errors for one defect.
- Release lint checks application-version synchronization before compilation.

## [0.22.0] - 2026-09-06

### Changed

- `talkbank_lsp::backend::utils::LineIndex` borrows its source. Its
  `offset_to_position` method accepts only the offset, preventing callers from
  pairing indexed line starts with another text. This is a breaking Rust API
  change.

### Fixed

- LSP edits, formatting, semantic tokens, selection ranges, symbols and quick
  fixes consistently use UTF-16 coordinates. Multiline semantic captures split
  into individual lines, and whole-document formatting includes the final newline.
- Gem outlines use parsed header spans and matching labels, preserving CRLF
  positions and counting only actual utterances in the parent outline.
- Language-service initialization retains its result in `OnceCell`; nested
  highlighter access returns an error instead of panicking. Execute-command
  services accept only their own request enums, removing routing panic branches.

## [0.21.0] - 2026-09-06

### Changed

- LSP backend cache fields are replaced by a private source-bound analysis.
  The unused public incremental-splice and validation-cache modules are removed.
  Syntax reuse remains incremental; models and validation results are rebuilt
  together using the shared model validator.

- `DocumentRoot` is a private-field classification with method accessors rather
  than a publicly constructible enum. It owns both document lowering and
  whole-source diagnostic scope.

- re2c parsed header lines carry `HeaderProvenance` in place of a standalone
  separator field, and box their header payload. The owned lexer extent now
  reaches model header spans; boxing keeps the file-line enum compact.

- re2c pause tokens and parsed pause variants retain a `PauseLexeme` instead
  of discarding the full lexical extent. This changes their Rust payload types.

### Fixed

- A truncated document without a final newline retains its complete simple
  final main tier. Shared terminal recovery reuses the normal fragment parser
  and preserves caller coordinates while validation reports missing `@End`.

- LSP diagnostics after edits now agree with fresh-open text, including deleted
  headers, recovery suffixes, Unicode edits and skipped debounce revisions.
  Tree-sitter edits use the cached tree's own source and UTF-8 byte coordinates.
  Feature and pull-diagnostic requests cannot reuse another revision's spans.
  Published diagnostics carry editor versions; obsolete analyses are discarded.
- LSP validation includes shared file-level rules such as E752 instead of a
  separate incomplete validation sequence. Protocol regression tests share the
  existing executable harness, preserving interleaved responses/notifications.

- Recovery before or after a complete document receives localized diagnostics
  without discarding the document. A complete final main tier stranded outside
  its line wrapper when `@End` is missing is retained through normal utterance
  construction.

- Documents missing `@UTF8` retain their headers and utterances and report
  E503, without cascades claiming present headers are absent. Both parsers
  locate the diagnostic at the end of the file, including rebased fragments.

- Both parser backends admit complete fragment coordinate ranges before
  parsing. Origins above 2 GiB retain correct model and diagnostic spans;
  overflowing 32-bit ranges are rejected instead of truncated. Synthetic
  wrapper text no longer consumes the caller's document range.

- re2c header fragments reject extra headers, utterances and unsupported
  trailing lines instead of returning a partial result. Lowering consumes an
  admitted logical header; folded content and recovery diagnostics survive.
- re2c preserves pause spans, including timed-pause parentheses, through nested
  content and fragment rebasing. Shared validation now owns pause spacing;
  duplicate token scans are removed.
- Separator spacing validation visits nested groups, reporting E765 at the
  missing space just as it does for top-level content. Spaced group controls
  remain valid.

## [0.20.2] - 2026-09-06

### Fixed

- Single-header parsing rejects extra headers instead of silently returning
  only the first. Lowering consumes a complete `HeaderFragment` that owns the
  selected node and its source; folded content and LF/CRLF remain accepted.
- Standalone header and dependent-tier parsing derive diagnostic coordinates
  from their owned synthetic source rather than separately supplied prefix
  lengths. Header lookup failures carry the caller's text and document origin
  instead of empty context. Existing malformed-header diagnostics are retained.

## [0.20.1] - 2026-09-06

### Fixed

- The standalone LSP exits after the editor's `exit` notification even when
  the editor keeps stdin open. A completed shutdown permits exit code 0;
  exit before a successful shutdown returns code 1. Protocol completion now
  stops the transport, and runtime teardown does not wait on Tokio's
  uncancellable stdin reader. Process-level regression tests cover the actual
  binary, rejected shutdown, early exit, and EOF.

## [0.20.0] - 2026-09-06

### Changed

- **Breaking:** removed `CachePool::open_or_else`; use `CachePool::new` and
  handle its `Result` directly. Cache opening no longer splits failures between
  an optional handle and a callback; the CLI retains the concrete opening error.

- Validation caches include both parser implementation source fingerprints,
  closing stale verdict reuse after parser-only edits without a version bump.
  Shared build-only source hashing reads each crate's own packaged files.

- **Breaking:** re2c `Token::TierPrefix` carries `DependentPrefixToken`, including
  the lexer-selected `DependentBodyKind`; dependent parsing matches that enum.

- **Breaking:** `CacheStats::cache_dir` is optional: in-memory storage has no
  filesystem directory. File-backed statistics retain the opening directory.

- **Breaking:** validation-cache constructors require `CacheIdentity` (rules and
  parser), and roundtrip cache methods use that bound identity instead of a
  parser string. `ParserKind` is shared from `talkbank-model` and re-exported.
  `MaintenanceCache` exposes administrative operations without verdict methods.

- **Breaking:** re2c prefix tokens carry `PrefixToken` payload/separator state.
  AST header lines, main tiers and `DependentTierEntryParsed` retain separator
  provenance. Access a prefix payload with `text()`; file AST snapshots reflect
  the new header and dependent-entry shapes.

- **Breaking:** re2c dependent-tier AST adds `RejectedMor`, retaining raw input
  after failed morphology admission without fabricating a model tier.

- **Breaking:** re2c `Token::TierPrefix` denotes a complete colon-tab prefix;
  the new `IncompleteTierPrefix` variant identifies recovery from a bare label.

- **Breaking:** re2c postcode tokens and main-tier AST postcodes carry checked
  payload state and lexer locations. `main_tier_to_model` and
  `utterance_to_model` now require an error sink; callers can no longer lower
  these structures without deciding where recovery diagnostics go.

- **Breaking:** re2c AST `ParsedAnnotation` separates `Scoped` annotations
  from retrace, replacement, language-code and postcode structures. Match
  `ParsedAnnotation::Scoped(ScopedAnnotationParsed::...)` for scoped kinds;
  their conversion to model annotations is now total. AST inspection snapshots
  reflect this category; serialized CHAT model output retains its shape.

### Fixed

- Release bumping updates and checks both desktop npm lockfile version fields.
  Dependency versions remain unchanged; CLI regression checks run in the fast
  app-version gate.

- Owned fragment wrappers project secondary labels with primary locations and
  identify synthetic context by exact source text instead of a length heuristic.
  Independent diagnostic context is preserved even when longer than the input.

- Main-tier fragments reject trailing material instead of silently accepting
  their first tier. Aggregate spans exclude a synthetic final newline, while
  retaining caller-supplied LF and CRLF line endings. Root-admission diagnostics
  now describe the required source shape without raw CST-kind wording.

- Fragment APIs no longer subtract obsolete word/main-tier wrapper lengths or
  subtract caller offsets from diagnostics. Synthetic utterance, participant
  and dependent-tier wrappers own their input boundary for model and error
  projection. Complete CHAT documents are recognized by the utterance adapter.

- re2c reports unsupported lines as E326 with their original source spans and
  preserves following utterances. Diagnostic rebasing keeps context highlights
  relative to their own source text.

- Speaker-qualified `@Birth of`, `@Birthplace of`, and `@L1 of` headers retain
  their separator spans, including non-CA whitespace violations and CA exemptions.

- Tree-sitter's whole-file fragment API now rebases model spans along with
  streamed diagnostics when parsing embedded CHAT at a nonzero offset.

- Tree-sitter recovery no longer reports E758 for spaces after rejected
  dependent-tier or header content. Separator provenance requires adjacency to
  the actual tab, preserving the original content diagnostics.

- Gate receipts verify the actual committed trees in every pushed ref, including
  annotated tags. Uncommitted fixes cannot authorize an older commit, and a
  gate whose source changes during verification cannot issue a receipt.

- re2c E760 highlights the original empty-POS morphology item, including across
  continuation lines and non-ASCII text. Recovery inspects source-owned items
  without rebuilding rich-token payloads or using a dummy diagnostic span.

- re2c no longer dispatches longer Phon labels through `%mod` or `%pho` body
  parsers. Bare and x-prefixed syllabification, alignment and interval tiers
  retain their own grammar without false E316 diagnostics.

- Cache statistics report the directory actually opened instead of resolving
  the current default again, including for explicitly located maintenance pools.

- Validation cache rows are isolated by parser in both CLI and desktop. Switching
  parser/rule combinations no longer risks serving another parser's verdict or
  consumes extra retained generations. Maintenance opens do not prune rule
  generations or expire rows merely to display statistics.

- re2c records trailing separator spaces across headers and tiers. The shared
  validator reports E758 outside CA, and serialization canonicalizes separators
  in either mode. This removes the separate main-tier scan and CA probe.

- re2c enforces morphological lemma starts and nonempty features at lexing.
  Rejected `%mor` reports E316/E600 and preserves morphology taint instead of
  converting to an unsupported tier with E605.

- re2c rejects a replacement glued to its word with E375/E316, using the
  original bracket locations. A spaced replacement remains valid. The canonical
  parser's malformed closing-bracket highlight excludes absorbed trailing
  whitespace and uses the original source for its diagnostic context.

- re2c reports E602 for malformed dependent-tier separators even when content
  follows the label, including a space in place of the required tab. Recovery
  uses the lexer-classified prefix and locates the complete malformed line;
  empty content no longer participates in deciding whether its prefix is valid.

- re2c rejects whitespace-only postcodes with E363 while preserving the rest
  of the tier. Valid postcodes preserve leading payload whitespace and trim
  only trailing whitespace, matching the canonical parser. Postcode diagnostics
  retain the complete token span through file, utterance and main-tier APIs.
- re2c utterance fragments forward parse diagnostics, and file diagnostics honor
  the caller's offset. A shared streaming adapter replaces temporary diagnostic
  collectors in header and participant fragments.

- re2c reports E757 when rich bracketed annotations are glued to the following
  word, including `[!]there` and `[= toy]there`. The check uses the parser's
  annotation categories and reports the following word's original lexer span.

## [0.19.0] - 2026-09-05

### Changed

- **Breaking:** the LLM response cache has one owning handle per path across
  processes. Share the handle across threads and drop it before reopening.
  `CacheError` distinguishes a busy cache and a visible replacement whose
  final directory sync failed.

- **Breaking:** removed `SinToken::new_unchecked`; use checked `SinToken::new`.
  `SinTier::from_tokens` now returns `Result<SinTier, EmptyText>`. re2c's
  `SinTierParsed` and `SinItemParsed` own checked `SinToken` values and no
  longer take a source lifetime parameter.

- **Breaking:** re2c AST word `raw_text` is now `Cow<str>`, distinguishing
  borrowed source from owned reconstructed text. Parser combinators separate
  source and token-storage lifetimes.

- **Breaking:** re2c's `sin_tier_from_text` returns `ParseOutcome<SinTier>`;
  malformed fragments can no longer appear as successfully parsed empty tiers.

- **Breaking: diagnostic enrichment takes a source-bound index.** Replace
  `enhance_errors_with_line_map(errors, source, map)` with
  `enhance_errors_with_index(errors, &SourceIndex::new(source))`, or retain
  one `SourceIndex` for repeated batches. Its immutable borrow prevents source
  edits while the index is used; callers cannot pair another file's line
  boundaries with the source and trigger a UTF-8 slicing panic. The existing
  `enhance_errors_with_source` convenience API retains its signature.

### Fixed

- LLM response-cache writes now prepare, flush, and atomically publish snapshots
  before updating memory. Failed writes preserve old entries; concurrent puts
  cannot publish stale snapshots. Unix builds also confirm directory durability.

- Both parsers now share source-bound control-character checking before parsing.
  re2c no longer silently accepts forbidden controls in free-text tiers; the
  lexical diagnostic retains its original source and exact byte range.

- E212 spec coverage now demonstrates its reachable CA-mode word-category
  boundary, alongside legal controls. Its existing implementation is marked
  implemented, replacing the misleading legal-only deferred fixture.

- Utterance validation now reaches bare and grouped `%sin` tokens admitted
  through JSON, reporting empty text at the tier's span. Both parsers construct
  tokens through the checked constructor; the duplicate unchecked path is gone.

- re2c parsing no longer leaks copied source, token arrays, recovery buffers,
  or reconstructed words. The lexer safely handles unpadded input at EOF, and
  word-fragment conversion retains spans from the caller's original source.

- re2c `%sin` fragment parsing now uses the whole-file grammar, preserving
  single-token gesture groups and rejecting unclosed groups with a diagnostic.
  The duplicate whitespace parser and its independent group state are removed.

- Diagnostic line/column lookup no longer caches by source address and length,
  which returned stale positions after same-length edits or allocation reuse.
  One-off lookup scans without allocation; indexed batches retain logarithmic
  lookups without retaining a hidden source copy or thread-local cache.

- Foundation publication checks now cover every workspace crate outside the
  approved first wave. `talkbank-llm` is explicitly held back; a metadata-only
  mode checks manifests and dependencies without packaging or registry access.

- Generated fixture and documentation directories now retain unchanged files
  and prune only obsolete output through an ownership capability. Conflicting
  ownership, nested human content and symlinks are refused before pruning.

- Spec regeneration preserves unchanged outputs in shared directories, including
  generated model code and Rust test bodies, while still removing explicitly
  retired files. Progress counts now report actual writes.

- Tree-sitter generation now stages all grammar artifacts and preserves
  unchanged files. The grammar currency check no longer rewrites source files,
  and also checks generated C headers.

- Node-type, traversal and conformance-inventory regeneration now preserves
  unchanged files and publishes changed output only after the generator
  succeeds. A failing generator no longer truncates those committed Rust files.

- Removed an unnecessary schema rewrite that treated valid Draft 2020-12
  `$ref` siblings as invalid and could modify literal schema data. Generated
  schemas now retain schemars' structure; enum tags and referenced payloads
  remain jointly validated.

- Schema generation now preserves unchanged files and runs only when explicitly
  requested by `just schema-gen` or `just regen`. Ordinary tests no longer
  rewrite a compile-time dependency and trigger avoidable recompilation.
  Schema currency failures report the repair command without dumping the
  complete schema.

- `to-json --skip-schema-validation` retains the transcript name and requested
  CHAT checks, including E531 for mismatched media filenames. Single-file and
  directory conversion now share one named parsing path before schema policy
  selects serialization.
- Directory JSON conversion prints individual parse/validation diagnostics
  and exits with failure when any file fails; successfully converted sibling
  files remain available.

### Added

- `JsonSchemaPolicy` and `chat_to_json_with_schema_policy` let library callers
  select JSON Schema validation independently of CHAT validation and transcript
  identity. Existing conversion functions retain their signatures.

## [0.18.1] - 2026-09-05

### Added

- **Timing-producing transforms now have a typed media-link transition.**
  `reconcile_media_timing` consumes a `ChatFile` and returns either an
  `UntimedChatFile` or a `LinkedMediaChatFile`. A timed document must have one
  usable `@Media` declaration; the transition removes `unlinked`, accepts an
  already-linked declaration, and returns typed errors for missing, ambiguous,
  or contradictory media. Both states expose only an immutable document and
  post-transition serialization. This prevents a forced-alignment pipeline
  from writing fresh timing bullets while retaining the contradictory
  `@Media: ..., unlinked` status rejected by E552. Validation verdicts are
  unchanged.

## [0.18.0] - 2026-09-04

### Changed

- **Warm development tests no longer scan unpacked macOS codegen objects.**
  The root and specification workspaces embed line-table debug information in
  linked artifacts instead of retaining every `.rcgu.o`. This bounds the file
  count in both target directories and removes filesystem enumeration from the
  warm-test path while preserving source locations in diagnostics. Nine spec
  generator commands are also excluded as empty libtest harnesses; their
  library and integration tests remain in the suite.

- **Breaking: validation returns owned evidence rather than a mutable phase marker.**
  `ChatFile` is no longer generic. `validate_into` returns
  `Result<ValidChatFile, ValidationFailure>`; accepted payloads are read-only,
  errors retain the rejected model, and unknown/recovered tier provenance cannot
  pass. `validate_with_policy` records rules, alignment coverage and transcript
  name. `parse_validated_with_parser` additionally requires error-free source
  parsing. Remove the old `NotValidated`/`Validated`/`ValidationState` imports
  and consume `into_unchecked()` before editing an accepted document. Serialized
  transcript fields remain unchanged.

  Required-validation compatibility APIs now use the same proof-producing
  transition before returning an explicitly editable model. Their streaming
  variants return `Err` after parse or validation failure even when the caller's
  diagnostic sink discards messages. Merge preflight retains `ValidChatFile`
  while reading the accepted reference and borrows its document for the merge.

- **Breaking: utterance builders can retain an utterance comment.**
  `UtteranceDesc` adds `comment: Option<ComTier>`; Rust struct literals must
  supply this field. A supplied comment is emitted as `%com`. A comment on an
  empty utterance is rejected instead of being silently discarded.

- **`%pho`, `%mod` and `%sin` count mismatches are reported by one
  algorithm.** The utterance metadata path used its own copy of the
  positional alignment with the diagnostic codes passed in as parameters;
  it now uses the `AlignableTier` route, which reads the tier's own type
  (`%pho` or `%mod`) to choose E714/E715 or E733/E734. The codes are
  unchanged; the messages are the positional form the `%sin` route already
  used (a per-position table instead of a bare count).

- **A control character is a lexical error anywhere in the file.** E315 is
  now decided over the whole input before any parse, so a forbidden control
  character in a word, a `%com` line or a header value is reported at its own
  offset; previously a word's surfaced as generic E316 and free-text tiers
  accepted it silently. Permitted are TAB, LF, CR, the bullet delimiter
  U+0015 and the CA underline pairs U+0002 U+0001 / U+0002 U+0002; CLAN's
  italics pairs are reported (CHECK 102).

- **E303 covers every header whose colon is not followed by a TAB.** It used
  to fire only for `@Comment:`, every other header fell to E316, and the
  message said "space" whatever followed the colon; it now names what was
  found (a space, nothing, or the character).

- **`Word` lexical content is read-only outside its owning type.** Direct
  access to the former public `content` field is replaced by `content()`;
  callers that intentionally replace typed content use the named mutation
  APIs, which invalidate derived `cleaned_text`. This prevents a content edit
  from leaving stale lexical text in JSON. Direct crate-internal access to
  `raw_text` is closed as well, so recovery spelling changes use the explicit
  setter rather than bypassing the field boundary.

- **Speaker-code structure now has one typed assessment across every model
  surface.** Direct `SpeakerCode::validate` previously mislabeled an overlong
  code as undeclared (E308), mislabeled a reserved character as a missing CST
  node (E302), and enforced a different character policy from headers and main
  tiers. All three routes now consume the same producer-issued valid/invalid
  state and report E307. The seven-character limit counts Unicode scalar
  values rather than UTF-8 bytes, and diagnostic context records the offending
  code rather than an internal field label.

- **Malformed regions now distinguish unpaired CHAT quotation delimiters from
  unrelated parser recovery.** Structurally unpaired `“` or `”` delimiters
  report E242 even when tree-sitter encloses them in a larger error node.
  Balanced quotation delimiters inside some other malformed region no longer
  produce a false E242, and ASCII straight quotes are not mislabeled as CHAT
  quotation delimiters.

## [0.17.0] - 2026-08-30

### Changed

- **Reference-mode speaker identification now preserves typed lexical support.**
  `DonorMatchReport` retains the reference, donor, shared, and union token
  counts that derive each Jaccard score; its winner, evidence, and confidence
  margin are no longer independently constructible public fields. Thresholds
  are checked `ConfidenceThreshold` values, while confidence is explicitly
  `NoInformation`, `Finite`, or `Unbounded` instead of overloading `0.0` and
  infinity. Use the read-only report accessors in place of direct field access.
  `chatter speaker-id --write-match-report NEW.json` writes the accepted,
  low-confidence, structural-refusal, or input-refusal evidence without
  replacing an existing report.

- **`%wor` timing is now admitted through explicit typed evidence states.**
  `MainTier::wor_projection()` defines the shared positional membership policy;
  count binding, canonical-token corroboration, and complete positive interval
  assessment are separate states, so equal word counts alone cannot be treated
  as trustworthy timing. The impossible tier-level `WorTier::bullet` field is
  removed: timing evidence exists only when an actual `%wor` word carries a
  bullet. Callers of the former alignment and tier-bullet APIs must migrate to
  the projection, binding, corroboration, sequence-assessment, and
  `WorTier::timing_evidence()` APIs.

- **`rediarize` and `rediarize_content` require a `DiarizationTimeline`.**
  The windowed overlap algorithm needs turns ordered by start time, but the
  former `&[DiarizationTurn]` API let every library caller bypass that
  precondition and silently obtain a wrong winner. `DiarizationTimeline::new`
  owns the sorting transition, retains the longest-turn window bound, and
  keeps its ordered storage private. `TurnsFile` now exposes `source()` and
  `timeline()` accessors instead of independently public fields.

- **Tree-sitter 0.27.0** now drives the Rust parser, highlighting runtime,
  grammar crate, spec tooling, and grammar-generation CLI. The Node binding is
  independently current at 0.25.1. Generated parser artifacts and the complete
  parser/backend parity gates are regenerated and checked with this toolchain.

- **The vendored Rust lexer is regenerated with re2c 4.6.**
  `re2c-version.toml` is now the exact generator source of truth, and
  `just verify-vendored-lexer` refuses a different `re2rust` before comparing
  generated bytes. The 4.6 output differs from 4.5.1 only in its generator
  provenance header; upstream's 4.6 implementation change is Zig-only.

- **`Merged::report` is the only route from a merge to a file.** `into_file`
  and `file` are gone from `Merged`; `report(sink)` yields a `Reported`, which
  owns them. Serializing a merge without asking what it dropped is no longer
  writable, which is what two commands did until each was fixed by hand.
- **`chatter merge` and `chatter pipeline` now warn when a File 1 speaker is
  dropped.** A speaker outside `--retain` loses every utterance while keeping
  its `@Participants` row, so the output declares someone who says nothing.
  `AmbiguousSpeaker` does not catch it: that fires only when a code appears in
  both files. Both commands print the same warning, naming the speakers and how
  many utterances each lost, from one shared reporter. `chatter batch` drives
  the pipeline path, so the silent one was the path that runs whole corpora.

- **`merge_chat_files` returns the provenance of every merged utterance**, not
  only the merged file. It always knew this and threw it away: it walks each
  input in order building two lists, then stable-sorts the combination by
  `start_ms`. A consumer joining the output back to its inputs had to
  reconstruct the mapping by matching `(speaker, raw bullet)`, which is correct
  only while two facts hold that no caller can check, that the sort is stable
  and that inserted utterances are cloned unedited.

  The return type is now `Merged`, carrying the file, one `MergeOrigin` per
  output utterance in output order, one `ReferenceFate` per File 1 utterance
  and one `DonorFate` per File 2 utterance, each in its own input's order.
  Ordinals are `ReferenceIdx` / `DonorIdx`, separate types because two
  same-signature accessors over one index type answered confidently about the
  wrong file. `merge_chats`, the string wrapper, is **removed**: it had no non-test caller,
  and a `String` return cannot carry the provenance, so every consumer that
  wants the report has to work on parsed files anyway. `MergeError::Parse` goes
  with it, since `merge_chat_files` takes files that are already parsed.

  Prefer `Merged::utterances_with_origin()` to pairing the accessors by hand:
  zipping `origins()` against `file().lines` type-checks and is wrong by the
  number of header lines.

  `DonorFate` is a partition rather than a list of exclusions, so "this donor
  utterance is unaccounted for" is not expressible. An earlier form returned
  only the excluded ordinals and proved completeness with arithmetic, which
  balances just as well when every ordinal is shifted by the donor's header
  count.

  Both inputs are accounted for. `DonorFate` covers File 2; `ReferenceFate`
  covers File 1, where an utterance whose speaker is not retained is dropped and
  `AmbiguousSpeaker` does not catch it, because that fires only when a code
  appears in both files. A reference-only `MOT` with `retain = [CHI]` therefore
  passed every precondition, kept its `@Participants` row, and lost every
  utterance silently.

  `DonorFate::Inserted` carries `tiers_stripped`, because `strip_tiers` applies
  to the donor and only the donor: a bare `Inserted` claimed "carried over" for
  an utterance that was carried over AND edited.

  Still not a complete account of everything a merge omits, which is why the
  accessors are named `excluded_by_retain` and `dropped_not_retained` rather
  than `excluded` and `dropped`: donor headers other than `@ID` and `@Comment`
  are not carried.

### Fixed

- **`chatter pipeline --override-file` now refuses invalid override files.**
  Malformed TOML, unsupported schema versions, and read failures previously
  disappeared into "no override configured", silently triggering a fresh
  automatic speaker match. Explicit operator input now travels through the
  existing typed `OverrideFileError` exit path and no merged output is written.

- **`chatter rediarize` no longer counts overlapping turns from one track
  twice.** A track appearing in several turns is now measured by the UNION of
  its coverage: gaps remain gaps, while same-track overlaps count their shared
  interval once instead of manufacturing speaker time and distorting
  `--contested-at`. Cross-track overlap remains evidence for both simultaneous
  speakers, so `ownership.total_ms` is the sum of per-track union-held time and
  can exceed the utterance bullet's duration. The JSON shape is unchanged; its
  corrected measurement semantics are documented in the user guide.

- **`chatter fix --apply` can now repair E750 inside its recovered utterance.**
  Ordinary splice edits remain barred from parser-tainted regions. The E750
  catalog entry alone carries the typed state that it removes the delimiter
  whitespace responsible for that recovery, and the command still reparses and
  independently validates the resulting CHAT before writing it.

## [0.16.0] - 2026-08-27

### Removed

- **`E214` is retired**, and the reason is worth more than the code was. It
  began as "a bare `[*]` carries no error code" and was deliberately DISABLED
  as leniency Decision 1, because reference files use bare `[*]` as valid CHAT.
  Its number was then reused in the same file for a DIFFERENT rule, "the
  scoped-annotation list is empty", while its spec file went on documenting the
  original. So one code carried a retired rule in its documentation and an
  unreachable one in its implementation, and its own spec example produced no
  diagnostic at all. Nothing detected the drift because neither rule could
  fire. `ErrorCode::EmptyAnnotatedContentAnnotations` is gone; code matching on
  it will not compile.

- **`rules::should_skip_group`** is absorbed into the descent module that was
  its only remaining caller.

### Changed

- **Scoped annotations are NON-EMPTY by construction.**
  `AnnotatedContentAnnotations::new` returns `Option<Self>`, `TryFrom<Vec<_>>`
  replaces an infallible `From` that skipped the check, `Deserialize` rejects
  an empty list rather than accepting one off the wire, and there is no
  `Default`. `Annotated::new(inner, annotations)` takes the annotations instead
  of starting empty; `Annotated::with_one(inner, annotation)` is the
  single-annotation path, and `with_scoped_annotations` takes the newtype.

  The `Option` IS the bare-versus-annotated decision, so seven
  `if scoped.is_empty()` branches in the two parsers collapsed into it.

- **`UtteranceContent` gains `Action`, and `BracketedItem` gains `Group`.** Both
  enums had a gap where their sibling had a bare variant, and the parser filled
  it by wrapping the construct in an `Annotated` carrying nothing. That was
  20,184,072 values across a 106,000-file corpus, 99.3% of all
  `annotated_action` nodes, almost all of them a bare `0` marking silence in
  daylong audio. The two content enums are symmetric now: every annotatable
  construct has a bare and an annotated form on both sides. **Exhaustive
  matches over either enum will not compile until they handle the new
  variant**, which is the intended outcome.

- **`ErrorCode` is GENERATED from `spec/codes/error-codes.toml`**, a new
  per-code registry that is the single owner of a code's variant name, its
  rustdoc, its `kind` and its `status`, plus the retired numbers. `kind` and
  `status` are removed from all 236 files under `spec/errors/`; a spec's `code`
  is a foreign key resolved at load, so a loaded spec proves its code exists.
  **Anything reading `kind` or `status` out of a spec file must read the
  registry instead.** Exhaustiveness moved from a generator check to the
  compiler: a wrong match arm fails the build rather than a lint.

- **`GoverningMarker` is now `pub(crate)`; the public face is `GoverningMark`.**
  Its variants were public, so a caller could construct one directly and
  resolve a word's language without ever saying what enclosed the word, which
  is the question the type exists to force. `GoverningMark` is opaque, with two
  constructors: `of(word, enclosing)` and `without_own_marker(span, enclosing)`.
  **This supersedes the 0.15.0 migration note below**, which tells callers to
  use `GoverningMarker::of(word, enclosing_span)`. That path is no longer
  public; use `GoverningMark::of` with the same arguments.

- **`talkbank_lsp::content_span` is removed**, and with it the free function
  `content_span(&UtteranceContent)`. Deciding WHERE an item is now belongs to
  the model (`WordRef::span`, `GroupRef::span`), and deciding WHETHER the editor
  targets it belongs to `talkbank_lsp::editor_target`, which dispatches on
  `ContentStructure` rather than on 28 `UtteranceContent` variants. A new
  variant is therefore classified once, in the model, instead of once there and
  once in the LSP where the two could disagree.

- **Parser entry points take the generated typed node wrappers**, not a node
  plus a kind string. Every route from a loose node into a typed one is
  `FromNodeKind::from_node`, and the 411 sites that ASSERTED a node's kind
  rather than testing it are gone. Callers passing `(KIND_CONSTANT, raw_node)`
  pairs pass the wrapper instead, so the proof is required where the value is
  born.

- **JSON output changes, with no compatibility shim.** An `annotated_action` or
  `annotated_group` carrying no annotations is now `action` or `group`.
  Previously-emitted JSON containing the empty annotated form will not
  deserialize. Regenerate rather than reading cached `to-json` output. CHAT text
  is byte-identical either way, so no file changes validity.

### Known limitations

- **`--parser re2c` is NOT READY to judge CHAT validity, and this release says
  so in the tool.** A clean `--parser re2c` run is not evidence that a file is
  valid: the backend ACCEPTS constructs the default backend refuses. Measured
  2026-08-27: an unrecognised scoped annotation on a quotation
  (`“hello” [qq] .`), on a pause (`hello (.) [qq] .`), and in utterance-initial
  position (`[x 2] hey .`) are all accepted, where the default backend reports
  E316 or E375.

  The cause is information lost before validation runs, not a missing rule.
  Both parsers build the same `talkbank_model` types and share one validator,
  but each has its own intermediate parse tree, and re2c's does not carry
  annotations for every construct: `ast::Group` has an `annotations` field and
  `ast::Quotation`, six lines below it, does not. Three of the five hosts that
  regressed here ARE fixed in this release, because their annotations do reach
  the model; these three do not reach it.

  `--parser` help and `book/src/architecture/parser-backends.md` now say this
  at the point of use. That page also carried three claims this contradicts,
  including a parity table row reading "Re2c silent (misses error): 0", and a
  recommendation to prefer re2c for batch and CI validation. All corrected;
  the parity figures are marked as not re-measured.

  Use re2c to COMPARE two implementations, which is what a specification
  oracle is for. Use the default backend to decide validity. The default
  backend is unaffected by any of this, and is what `chatter validate`,
  `normalize`, `to-json` and the LSP use unless you ask otherwise.

### Fixed

- **`word@@` reported one defect twice**, the specific diagnostic buried under
  the generic one. The parser names a repeated `@` run as E203 with the run in
  hand ("a word may carry only one '@' suffix, found '@@'"); `check_inline_at_
  markers` then added its own E202 ("dangling '@' marker") for the same word,
  because the suppression that stops a double already existed for the
  E203-against-E203 case and was never applied to the E202 branch four lines
  above it. `word@c@` was the same. Both now report E203 alone.

  A bare trailing `@` (`hello@`) still reports E202: it carries no form type,
  so nothing else has named it, which is the case that branch exists for.

  Found by the release review; fixed by writing the spec example first, where
  the backend-parity gate stated it exactly: `tree-sitter [E202, E203] ... spec
  expects [E203]`.


- **`chatter rediarize` assigned an utterance to the track of its single
  LONGEST TURN, not the track holding the most of it.** `best_track` took the
  greatest `overlap_ms` over the turn list with no per-track accumulator, so
  three short turns of one track lost to one longer turn of another even when
  the first held twice as much of the utterance. Its own docstring and the CLI
  help both said "the track with the greatest overlap", which is what it now
  computes.

  This is the shape a diarizer actually produces: pyannote emits short turns
  with gaps inside a single speaker's run. A track appearing in several turns
  is accumulated now, and ties break on the track code rather than on turn
  order, so the winner is a function of the input rather than of how the
  diarizer sorted its file.

  **`best_track` is replaced by `TrackOwnership`, which keeps the whole
  distribution** (`winner()`, `shares()`, `total_ms()`, `runner_up_share()`)
  rather than computing it and returning one name. Returning only the winner
  is why the defect was invisible: nothing downstream could tell a track that
  held 95% of an utterance from one that held 34% of a three-way split.

  **Breaking:** `rediarize` and `rediarize_content` take a further argument
  (below), and `RediarizeOutcome` gains a field.

### Added

- **`chatter rediarize --contested-at SHARE`** reports utterances whose time is
  meaningfully split between tracks, in the stderr summary and in
  `--summary-json` under a new `contested` list. Each entry carries the
  utterance index, the track it was assigned to, and the full ownership
  distribution: every overlapping track with its summed milliseconds,
  descending, plus the total. The WHOLE distribution rather than a winner and a
  runner-up, because that narrower shape cannot tell a 55/45 split from
  55/23/22.

  Contested utterances are still reattributed to their winner, so they are
  reported separately from `flagged`, which keeps its narrower meaning of
  "declined to reattribute". Placement is byte-identical with and without the
  flag; this is a reporting change.

  **There is deliberately no default.** Omit the flag and nothing is reported.
  What share makes an utterance genuinely mixed has not been measured against
  human listening, and a default would hand every user a constant wearing this
  tool's authority. A value outside `0.0` to `1.0`, or `NaN`, fails the command
  before any file is read, rather than silently meaning "nothing is ever
  contested".

  Known limitation, stated in the book page: summed milliseconds per track
  cannot distinguish a speaker change INSIDE an utterance from crosstalk across
  the whole of it, and those want opposite remedies.

- **`TimeSpanMs::start_ms()` and `end_ms()`.** The fields are private so that
  `new()` is the only route in and an inverted span cannot be built, which is
  right, but it left the type WRITE-ONLY through the public API:
  `DiarizationTurn::span` is a public field of a type a caller could hold and
  could not read, so a downstream consumer of `parse_turns_json` had to
  re-declare the same concept to get the numbers back out. Reading cannot
  invert anything.


- **THREE COMMANDS COULD DELETE A TRANSCRIPT AND REPORT SUCCESS.** The worst
  class in this release, found by review rather than by any gate, and every
  case exited 0 with a green line.

  `chatter normalize notes.cha -o notes.cha`, the documented in-place idiom, on
  a file of ordinary prose left a ZERO-BYTE FILE and printed `✓ Normalized`.
  v0.15.0 refused it, so this was a regression. On a transcript missing its
  `@End` it deleted the LAST UTTERANCE, which is exactly the shape a file
  truncated mid-transfer has. On a malformed `%gra` tier it emptied the tier,
  producing a file it then refused to read again. Those two are unchanged from
  v0.15.0 and were shipping in both.

  `chatter debug retag-language` and `debug fix-s` wrote back a model that had
  DISCARDED an unparsable region: `hello [[[[ test ]]]] world .` became
  `world .`, in place, recursing over whole directories, with no `--dry-run`
  and no backup. `debug join-retrace` had the same shape and was found while
  fixing the other two.

  All four rewriters were the same three steps: parse, `to_chat_string()`,
  write. The return type was `String`, which cannot carry the one fact the
  caller needed, so no caller had it. `chatter to-json` refused all three
  `normalize` inputs, because it happened to route through a stricter path, and
  the two commands disagreeing about the same model is what made this findable.

  **Two different proofs, because the commands promise different things.**
  `normalize` reshapes and must lose nothing, so `talkbank_transform::Rewrite`
  refuses when a source line has no counterpart in the output, compared with
  whitespace removed so the canonicalisation it exists for still passes. The
  three EDITING commands change content on purpose, so that test would refuse
  every legitimate edit they make; they require instead that the model
  reproduce the source BYTE FOR BYTE **before** the edit, which is the only
  point where faithfulness is a clean question for them. A refused file is left
  untouched and the message names the line, or tells the operator to run
  `chatter normalize` first.

  Six CLI subprocess tests pin all of it, including the case that must NOT
  refuse: the six reference-corpus files `normalize` legitimately rewrites.

- **`chatter debug retag-language` is new**, and was missing from this section
  entirely. It retags a language code across all three notations it can reach
  (`@Languages`, the `[- code]` utterance precode, and `word@s:code`) and
  REFUSES a file naming the code in a `<a b> [@s:code]` span, which it cannot
  rewrite. `--to` deduplicates in `@Languages`. It is a tool and not a
  find-and-replace because a language code is also ordinary transcript content:
  its first use retagged `sun` to `fin` across a corpus where `sun` is also
  colloquial Finnish for "your" and appears 27 times as real speech.

- **A nested quotation stopped being detected as soon as either quotation
  carried an annotation**, on the default backend only, so `“a “b” c” [//]
  hello .` validated CLEAN while `“a “b” c” .` reported E372, and the two
  backends disagreed about a validity rule on identical bytes. `[/]`, `[*]` and
  `[% note]` leaked the same way, and so did an annotation on the INNER
  quotation.

  A quotation has TWO spellings in the model, with and without its own scoped
  annotations, and each half of the rule named only the first. `descent.rs`
  named both; `main_tier.rs` named one. The annotated spelling was introduced
  by the same release that gave quotations scoped annotations, and the nesting
  rule was never taught about it.

  Fixed as a type rather than as two more match arms: `GroupRef::Quotation` and
  `GroupRef::AnnotatedQuotation` are folded into one `Quotation(QuotationRef)`
  variant, mirroring the `RetraceRef` beside it, so "is this a quotation" is a
  single arm that cannot be half-written. **This is a breaking change to
  `GroupRef`**; a caller matching `AnnotatedQuotation` will not compile, and
  the two spellings remain distinguishable one level down through
  `QuotationRef`. `QuotationRef::span` preserves the distinction that
  `GroupRef::span` drew between the two, which folding them could have lost
  silently. The outer scan that looked for a quotation to test now descends
  through `ContentStructure` as well, so a wrapper cannot hide either side of
  the relation again. Spec examples 4 and 5 of `E372.md` are the two
  directions.


- **`@Location` and 14 other headers were REJECTED by the public fragment
  parser.** `parse_header_fragment`, and the `ChatParser::parse_header` trait
  method behind it, dispatched through 19 hand-written arms plus a catch-all,
  while the grammar's `header` supertype has 34 subtypes. `@Activities`,
  `@Bck`, `@G`, `@Location`, `@Number`, `@Options`, `@Page`,
  `@Recording Quality`, `@Room Layout`, `@Time Duration`, `@Time Start`,
  `@Transcriber`, `@Transcription`, `@Thumbnail` and `@Unsupported` all reached
  it and came back as errors rather than as `Header::Unknown`. The same headers
  parsed correctly inside a whole document, because that path matches the
  generated `HeaderChoice` exhaustively: two dispatchers for one job, one of
  them a drifted subset, and the tests covered only the arms that existed.

- **Five call sites silently discarded a group and every word inside it.**
  `convert_to_group_content` returned `Result<BracketedItem, Group>` where
  neither outcome is a failure, and the shape invited `if let Ok(item)`, which
  five call sites duly wrote. It is a TOTAL function returning `BracketedItem`
  now, so there is no second case to ignore and those sites preserve the
  content. An intermediate two-variant enum was tried first and reverted: it
  moved the decision without closing it.

- **The container descent rule had two implementations that had already
  drifted.** The eight walkers and `count.rs`'s four traversals each carried
  their own container arms, about thirty per side; `walk/bracketed.rs` shipped
  four ungated `AnnotatedQuotation` arms while `count.rs` gated the same
  variant, so one node was walked by one and skipped by the other. One
  `helpers::descent` module owns it for every traversal now.

- **A quotation could not carry a scoped annotation**, which CLAN CHECK accepts.
  The grammar takes `quotation_with_optional_annotations`.

- **An `@ID` age with a component too large for its field parsed as ZERO.**
  `AgeValue::from_text` parsed each component with `.parse::<u8>()` behind an
  all-ASCII-digits guard, which leaves exactly one way to fail (a value above
  255) and answered it with `0`. `2;300.` became `Valid { years: 2, months:
  Some(0) }`: two years and no months, presented as a successful parse, in the
  field that is the primary variable of most CHILDES research.

  **Bounded honestly: no validation verdict changes.** A component of three or
  more digits also fails the two-digit depfile pattern, so such a file was
  always reported invalid. What was wrong is what the typed model then SAID
  about it, which reaches library callers and anything reading the parsed age,
  not `chatter validate`'s answer. `age_component` returns `Result<Option<u8>,
  Unrepresentable>` now, so an ABSENT component (`1;` has no months) stays
  distinct from an unrepresentable one, and an unrepresentable one sinks the
  whole age to `Unsupported`, which preserves the original text byte for byte.

- **`ErrorCode`'s string constructor with a silent fallback is REMOVED.** It
  mapped any unrecognised string to `UnknownError` through a catch-all, so
  `ErrorCode::new("E7O5")` with a letter O compiled and would have shipped E999
  with nothing to catch it. Three of the `%mor` alignment checks built their
  codes that way (E705, E706 and E716; the count mismatch among them is CLAN
  CHECK 140), which also meant nothing reasoning over the enum could see those
  three checks at all. **The codes they emitted were correct**, so no
  diagnostic changes; what changes is the API. All three return typed variants,
  and `ErrorCode::parse_exact` returns `Option<Self>` and is now the only route
  from a string. Callers of the old constructor must handle the `None`.

- **A word carrying two `@` suffixes was reported wrongly, and one such word
  was DELETED on the way out.** Two distinct symptoms, which an earlier draft of
  this entry ran together:

  `hello@@c` and `hello@c@d` never formed a word at all, so the utterance fell
  to error recovery and reported the generic E316, "content could not be
  parsed", while the model's own rule for the shape could never fire.

  `word@k@s:spa` DID form a word and DID report E203 in v0.15.0. What was wrong
  there was the message, and what was dangerous was `chatter normalize`, which
  exited 0 and wrote the word back split in two: `word@k@st` became
  `word@k@s t`, silently, at exit 0. That is the deletion, and it is the reason
  this entry exists.

  Both shapes now parse, are refused as E203 with a message naming the actual
  defect (`A word may carry only one '@' suffix, found '@c@s:spa'`), and
  serialize back verbatim, so `normalize` REFUSES the file instead of rewriting
  it. `hello@c`, `dog@j` and `hola@s:spa` are unchanged from v0.15.0.

  **A word may carry at most ONE `@` suffix.** Ruled 2026-08-27, asked because
  CLAN CHECK accepts multiple suffixes and chatter does not: "Multiple suffixes
  might make logical sense, but it is computationally messy. So, let's disallow
  that." `word@k@s:spa` is therefore invalid even though the form marker `@k`
  and the language suffix `@s:spa` are each fine alone. **A documented
  divergence from CHECK**, which passes these files; main-tier words with two
  `@` runs number zero across the ~106,000 kept files.

  A bare trailing `@` (`hello@`) deliberately keeps its existing E202: `@` is
  the header sigil, and admitting a single one in word position moved the
  diagnostic for a doubled `@End`.

  **`--parser=re2c` REFUSES all of these too, and agrees on the code for some
  of them.** Measured: `gumma@c@s:spa` and `bebe@k@st` report E203 on both
  backends, because that lexer takes the two suffixes as separate tokens and
  the model's own rule counts them. Not full agreement: on the `@s:`-bearing
  case the default backend now reports E203 alone where re2c reports E203 plus
  E255, because the word carries an undeclared form type and its `@s:spa` no
  longer registers as a language marker. Both refuse the file. `dog@b@c` (E209 plus E253) and `hello@@c`
  (E321) it refuses for other reasons, since its lexer cannot form those words
  at all. The named message above is the default backend's. The divergence is
  recorded in the parser-parity baseline as a Conflicting row for `E203.md`.

- **E207's message asserted that a KNOWN annotation marker was unknown.** It
  read `"x" is not a known scoped annotation type`. An annotation reaches the
  unknown path whenever no specific rule matched it WHOLE, which happens both
  when the marker really is unknown (`[qq]`, `[@ xyz]`) and when a known marker
  carries content the rule refuses. Under `--parser=re2c`, whose rule set is
  narrower, `[x 0]` and `[:]` both land there, and the message then told the
  reader that `x` and `:` are not known scoped annotation types, when the
  marker is not the thing at fault. `[: replacement]` is ordinary valid CHAT on
  both backends, so the old message was plainly false there. **The message now
  names the annotation as written**, `could not read [x 0] as a scoped
  annotation`, which is true in every case and shows more than the marker
  alone.

  **Scoped honestly: this reaches the `--parser=re2c` path only.** On the
  default backend the diagnostic is issued by the parser rather than the model,
  and its message is byte-identical to 0.15.0's. An earlier draft of this entry
  said "affects both backends"; measured, it does not.

  And `[x 3]` is NOT an example of a valid construct: `hello [x 3] .` is
  refused by both backends. Only the group and utterance-initial spellings
  parse, and only under re2c, which is its own divergence.

- **Under `--parser=re2c`, an annotation on a top-level word reported E207 at
  byte 0**, on line 1, pointing at `@UTF8`. `Annotated::new` starts at the
  dummy span and the tree-sitter parser follows it with `.with_span(..)`; this
  converter never did. That path now takes the annotated construct's span
  widened to cover every annotation whose text can be placed, and REFUSES
  rather than answering with the sentinel when nothing can be, so a span is
  either real or absent.

  **SCOPED HONESTLY: this is one of eleven `Annotated::new` sites.** An
  annotation on a bracketed word, a group, an event, an action or a retrace
  still reports at byte 0 under this backend. Those need the same AST work as
  the retrace spans, which is the queued change that carries the lexer's own
  token ranges through the parser instead of re-deriving them.

- **`validate --list-checks` advertised two checks that cannot fire.** E361
  ("invalid timestamp value in media bullet") and E382 ("failed to parse `%mor`
  tier content") were marked `implemented` in the code registry, and nothing
  can produce either. Both now list as `Planned`: `--list-checks` goes from
  `224 checks (184 Active, 40 Planned)` to `223 checks (182 Active, 41
  Planned)`, the one retired check being `E214` above. The checks themselves
  are unchanged; the advertisement was wrong.

- **Under `--parser=re2c`, every rule keyed on a span was silently
  unreachable.** Separators and words both reached the model at `Span::DUMMY`,
  which is `{0, 0}` and therefore also a real position, and validation FILTERS
  on that value: a dummy span makes the model answer "there is no comma here",
  so E258 (consecutive commas) and every other span-keyed rule never fired on
  that backend. **Words and separators on the main tier now report the same
  spans on both backends**, which is what was fixed and what was measured: 61
  of 64 word and separator span sets match tree-sitter exactly.

  **The backends are NOT span-identical in general, and this entry does not
  claim they are.** Across the repository's own `.cha` files, 434 (file, code)
  pairs are reported by both backends and 338 of them still differ, 329 because
  re2c answers at byte 0. Untouched here: the pause-glue mirror in
  `parser/file.rs`, every terminator, and every dependent-tier diagnostic. E370
  is worse than byte 0, reporting an offset that is not a file position at all;
  that is unchanged from 0.15.0 and is not fixed here.

  This affects only the opt-in oracle parser; the default backend was never
  wrong.

- **Under `--parser=re2c`, three diagnostics were reported TWICE.** Giving
  words and separators real spans made three model rules reachable while the
  hand-written mirrors that existed BECAUSE they were unreachable were still in
  place, so E749, E764 and E765 arrived doubled. The parity gate stores codes
  in a `BTreeSet`, so multiplicity is structurally invisible to it and STILL
  is: a new six-utterance test (`re2c_reports_no_diagnostic_twice`) covers the
  mirrors it knows about instead. One doubled diagnostic survives that test's
  case list, E307 on a bad speaker ID, unchanged from 0.15.0.

- **Under `--parser=re2c`, an interposed word lost its form marker.** `&*SPK:`
  took a bare word body where the grammar defines the payload as a whole
  standalone word, so a form marker, an `@s` language suffix or a `$` POS tag
  on an interposed word was dropped. Found on real Spanish transcripts, where
  the two parsers had disagreed for as long as the rule existed.

- **Under `--parser=re2c`, an unrecognised annotation killed the utterance.**
  `[@ xyz]` reported E321 ("unparsable utterance") rather than E207 ("unknown
  annotation"): every specific bracket form had a lexer rule and anything else
  fell to a bare `[` no parser rule could use. E321 is a statement about the
  parser where E207 is a statement about the file.

- **Under `--parser=re2c`, `un++do` reported a misplaced linker.** The word
  body consumed `+` only when an atom followed, so the word ended at `un` and
  `++` matched the linker rule: E766, "a linker placed after utterance
  content", on a construct containing no linker. It reports E233, "empty part
  in compound word", as the specification says it should.

## [0.15.0] - 2026-08-25

### Changed

- **`ExtractedWord.lang` becomes `ExtractedWord.language: ExtractedLanguage`,
  and the word gains a `span`.** The old field carried only a word's OWN `@s`
  marker, so a word inside a `<...> [@s:hin]` span came out of extraction
  indistinguishable from an unmarked word in a plain English utterance. The
  extractor WALKS the tree and therefore knew about the span; it discarded what
  it had computed, and every NLP consumer downstream was left unable to recover
  it. Batchalign's morphotag read the old field, so span-governed words fell out
  of second-language dispatch and were tagged against the tier language.

  `ExtractedLanguage` is `Utterance | Own(marker) | Span(span)`. It is not an
  `Option`, because "the utterance governs" is a real answer rather than a
  missing one, and treating no-mark as no-language is the mistake the type
  exists to prevent. `ExtractedLanguage::resolve(span, tier, declared)` gives
  the resolved language directly.

- **`GoverningMarker::resolve_at(span, ..)` resolves without a `Word`.** The
  resolver only ever used the word for `word.span`, to place diagnostics, so a
  consumer holding an already-extracted word had to fabricate a
  `Word::new_unchecked` purely to satisfy the signature. Batchalign was doing
  exactly that. `resolve_word_language_with_marker` is deleted; one span-based
  core serves both paths.

## [0.14.0] - 2026-08-25

### Removed

- **`resolve_word_language` is gone from the public API.** It answered "what
  language is this word?" without saying what SCOPE it was asking under, and
  silently assumed a word's own marker was the whole story. Once `<...> [@s]`
  spans existed that was false, and the function had nowhere to put the span.
  Use `GoverningMarker::of(word, enclosing_span)` followed by `.resolve(..)`:
  the constructor takes the scope, so a caller with none writes `None`
  explicitly. `resolve_word_language_with_marker` is no longer public either;
  it is a primitive of that operation.

- **`FileStem::from_str` is renamed `from_stem`.** It can never implement
  `std::str::FromStr`, whose signature has no input lifetime, while this type
  borrows its stem; the old name invited callers to expect a trait they could
  use generically.

### Added

- **Multi-word code-switch spans: `<word word> [@s]` and `<word word> [@s:code]`.**
  Every word in the scope takes the switched language, exactly as if each
  carried the `@s` / `@s:code` suffix, so a switched stretch no longer has to be
  annotated word by word. Bare `[@s]` resolves the way a bare `word@s` does. As
  with any scoped annotation, a single content item needs no angle brackets:
  `hallo [@s]` is well-formed.

  **A word inside the span may carry its own marker, and the word wins.** This
  is attested usage rather than a case to reject: transcripts mark a switched
  stretch with the span and individual borrowed words inside it with the donor
  language. Resolution is innermost-first (word, then span, then utterance) and
  each layer records its own provenance, so `language_metadata[].source` gains
  `span_shortcut` and `span_explicit` alongside the existing `word_*` values. A
  span and a suffix can resolve to the same CODE, and that field is the only way
  to tell which mark decided it.

  Consumers reading a word's `lang` field alone will under-report switches: a
  span is an annotation on the group, and the words inside keep `lang: null`
  unless suffixed. `language_metadata` carries the resolved answer for every
  word regardless of which mark produced it.

### Fixed

- **`E220` and `E763` gated on the wrong language inside a code-switch span.**
  Span resolution reached metadata but not word validation, so a word's recorded
  language and the language it was checked against could disagree:
  `<ha# kelev> [@s:heb]` in an English-headed file was reported `E763` as
  English while its own metadata said Hebrew. Both paths now share one
  precedence decision.

- **A `[@s:code]` span could not be serialized to JSON at all.** The enum was
  internally tagged, which serde cannot use for a variant carrying a string, so
  `chatter to-json` failed at runtime on any transcript containing one while the
  bare `[@s]` form worked. The committed JSON Schema described a shape the
  serializer could never emit; it is regenerated and smaller.

- **Every generated error fixture parsed with a spurious `MISSING newline`
  recovery node**, because the generator stripped the trailing newline the
  grammar requires. Invisible for as long as each fixture also emitted a real
  diagnostic to hide it behind. Restoring it changed the diagnostics of zero of
  the 335 existing examples.

- **`chatter debug fix-s` no longer rewrites an utterance containing a
  code-switch span.** It strips each word's `@s` suffix after writing the
  `[- LANG]` precode, so for a word inside a span the span would then govern it
  and its language would silently change: `<how@s:fra to@s:fra> [@s:eng] .`
  became `[- fra] <how to> [@s:eng] .`, whose words resolve to eng. It now
  refuses such utterances, which is lossless where rewriting was not. **No
  released version could do this**, because `[@s:eng]` did not parse before
  this release; it was found and fixed within the same cycle.

- **`E220` no longer fires on a word whose language is unresolved.** It treated
  an empty candidate set as "no language permits digits", so an unresolvable
  `@s` produced "illegal digits in language X" with no X. `E763` already
  skipped in that case and the two are documented as agreeing. The visible
  effect: `[- zho] ni3hao3@s .` now reports `E248` alone, which names the
  actual defect, rather than `E248` plus a consequence of it.

- **Language-gated word rules run whenever the language is KNOWN, not only when
  the file declares one.** The gate asked whether `@Languages` exists, which is
  a different question: `<...> [@s:eng]` names a word's language with no header
  present, and nothing was checking those words.

- **A word carrying its own `@s` inside a span, and a span on a replaced word or
  a retrace, are now validated under the span** like any other span-governed
  word. Only annotated GROUPS were handled, so `hallo [@s]` was recorded as
  switched in metadata while being validated against the tier language.

### Changed

- **"CA" named three different things, and two names picked the wrong one.**
  The symbol registry's exported arrays are computed from `parse_role` and were
  called `ca_element_symbols` and `ca_delimiter_symbols`, naming PROVENANCE on a
  value holding a PARSE ROLE. They are now `word_attached_symbols` and
  `paired_stretch_symbols`, and their union, which builds the set forbidden
  inside a `word_segment`, is `ALL_MARKER_SYMBOLS`. Library consumers reading
  `talkbank_model::generated::symbol_sets` see the renamed constants.

  The registry already carried both facts: `notation_family` says where a
  symbol's notation came from, and 2 of the 25 are `disfluency` rather than
  Conversation Analysis. Those two are `≠` (blocking) and `↫` (segment
  repetition), which every fluency corpus depends on, so the old name was false
  for exactly its load-bearing members. **The `ca_element` and `ca_delimiter`
  NODE names are unchanged**, and `parser.c`, `grammar.json` and
  `node-types.json` regenerate byte-identical.

- **`ChatOptionFlag::enables_ca_mode` is replaced by
  `has_effect(CaOptionEffect)`.** The old name asserted that `@Options: CA`
  turns Conversation Analysis parsing on. It does not: CA-originated markup
  needs no option at all, and the flag's scope is material judged specifically
  weird CA. A predicate that reads "is this file CA" is what invites gating
  SYMBOL ADMISSIBILITY on the option, which would be wrong for every symbol,
  including the genuinely CA-originated ones.

  The two effects are named separately because they are not the same kind of
  thing: `TerminatorRequirementWaived` waives a requirement, while
  `ParentheticalIsCaOmission` changes what a construct MEANS. Calling both
  leniency would be a quieter version of the same conflation. The match on the
  effect is exhaustive, so a third effect, or a second flag granting one, breaks
  compilation rather than silently inheriting `CA`'s answer.

  **Validation verdicts: UNCHANGED.** Renames and one predicate; every call site
  computes what it computed before.

- **Tree-sitter 0.26.13** across the workspace, the grammar crate, the spec
  workspace and the `tree-sitter-cli` devDependency, plus desktop npm
  devDependency bumps and jsonschema 0.51.

  **Validation verdicts: UNCHANGED over the sampled corpus, and that needed
  measuring rather than assuming.** 0.26.13 avoids wide error nodes on
  unparseable input, which is a change to RECOVERY, so it can move what
  `validate` reports on malformed CHAT without changing one byte of the
  regenerated `parser.c` (which is in fact byte-identical here) and without any
  fixture in the suites noticing, because they all parse. The corpus
  differential is what can see it: over 2,147 files at stride 50, stratified per
  repo, against the v0.13.0 released build, there were no new error codes, no
  per-code count increases, no newly failing roundtrips and no new cross-backend
  disagreements. That is a statement about the sample, not about the whole
  corpus; at this stride a defect in a few dozen of ~106,000 files could still
  hide.

  The generated typed CST traversal is byte-identical apart from its generator
  provenance stamp, and `just spec-gen` moved no artifact.

## [0.13.0] - 2026-08-21

**Validation verdicts: UNCHANGED.** Nothing here moves what `validate` reports
on a CHAT file. Every prior entry states this either way, and the published
promise is that an entry without the note did not move its verdicts, so an
entry that omits it cannot be told from one nobody filled in.

### Removed

- **`talkbank_transform::capitalize` is GONE.** The English capitalization
  transform announced in 0.7.0 (`capitalize_english`, `capitalized_pronoun_i`,
  `is_capitalizable_initial`, `capitalize_first`) is deleted. It is the only
  change here that affects a library consumer.

  Why: chatter is the CHAT-format authority, and English orthography is a
  convention of one language rather than a fact about CHAT. Nothing inside
  chatter ever called it; its two users were downstream generators, which
  wanted different policies. One of them had already written its own version
  of `is_capitalizable_initial` and documented that chatter's answered a
  different question. The module also had no stopping rule: pronoun "I" and
  sentence capitals today, then contractions and proper nouns on request.

  If you used it: copy it into your own generator, where the policy belongs.
  It is built entirely on public API (`walk_words_mut`, `Word::category`,
  `Word::untranscribed`), so nothing about the move needs chatter internals.
  Note that the version shipped here had three defects in
  `is_capitalizable_initial`, all from deciding a structural question from
  `cleaned_text()`, which strips the very prefixes the question needs: a
  non-letter-initial word did not consume the utterance-initial slot, so the
  capital landed on the following word; an apostrophe-initial word received no
  capital at all; and the `&`-fragment guard could never fire, so a filler took
  the sentence capital. Ask the typed model instead.

  `num_words` is unaffected and stays: it serves E220, a rule chatter enforces.

### Changed

- **`chatter new-file` builds its template through the typed model.** The
  emitted skeleton is produced by parsing and serializing a typed `ChatFile`
  rather than formatting text, so it is roundtrip-proven by construction; the
  default output is unchanged.

- **`docs/errors/index.md` is one table, sorted by code.** It emitted one `##`
  section per spec, 236 of them with 31 exact duplicates, each over a
  single-row table, and reprinted every description. It is now a flat table
  with Code, Name, Category, Kind, Level and Status columns: 234 lines where it
  was 2,247. Anything that scraped the old section structure will need
  updating; anything that followed the `E###.md` links is unaffected.

- **The error-spec format is TOML frontmatter with a required CLAIM per
  example.** Landed in stages within this unreleased window, superseding
  earlier entries' details: metadata moved from `## Metadata` bullets to `+++`
  frontmatter (an unknown or missing field is a load error); the authored
  `Layer` field was then DELETED (which pipeline stage catches a rule is
  recorded per example in the generated `spec/observations/` snapshot, and
  every example is a fixture in the validation corpus); and
  `Expected Error Codes` was replaced by `claim = 'violates' | 'legal' |
  { subsumed_by = ... }`, whose negative halves (a code that must NOT fire)
  are enforced. `level` moved from the spec file to the example, where it is
  required: a code can be violated at one level in one example and another in
  the next (E519 has header-level and utterance-level violations), so the fault site
  is a fact about the example; a code's page renders the distinct set. A
  non-empty `Description` remains required. This matters only if you author
  specs against `spec/errors/`.

- **Corpus tests require `TALKBANK_DATA`.** The re2c integration tests defaulted
  to a hard-coded directory under `$HOME`, which could only ever be right on one
  machine and silently sent everyone else to a path that does not exist. The
  variable is now required and its absence fails loudly. `just corpus-tests`
  needs it set; the default test suite is unaffected, since those tests are
  `#[ignore]`d.

### Internal

Not part of any published API, listed because the commits are marked breaking:
`spec/errors/*.md` now has ONE parser in the spec workspace rather than two (`ErrorCorpusSpec` and
its types are deleted), and the spec format's vocabulary moved to a new
dependency-light `talkbank-spec-vocabulary` crate that both cargo workspaces
share. The `generators` and `talkbank-parser-tests` crates are `publish =
false`.

## [0.12.0] - 2026-08-16

**Validation verdicts: CHANGED.** Four rules report where they were silent:
E241 on illegal untranscribed spellings, E756 on any empty dependent tier,
the participants check on files declaring an empty set, and the re2c backend
on empty tiers it used to paper over. If you gate a pipeline on `validate`,
diff your own corpus before upgrading; see
[What a Version Bump Promises](https://talkbank.github.io/chatter/chatter/integrating/versioning.html).

Adjudicated against real corpus data before shipping, per the standing
grammar-change gate. The full-stride differential against the shipped 0.11.0
build covers all 106,507 corpus files and reports EVERY error code unchanged
except E241, whose 661 new instances are every one an illegal short or miscased
spelling of an untranscribed marker: 624 `ww`, 18 `Www`, 10 `XX`, 6 `Ww`, 2
`Xxx`, 1 `Xx`. All adjudicated INTENDED, the rule correctly flagging invalid
data, and the 194 affected files join the cleanup queue. No new cross-backend
disagreements and no newly-failing roundtrips.

### Added

- **E241 rejects the illegal untranscribed spellings.** The corpus authority
  ruled that `ww` is not legal CHAT and `www` is canonical, adding `yy` against
  `yyy` unprompted. Which spellings are wrong is now DERIVED from the canonical
  set rather than listed, so `ww` cannot be missed while `xx` and `yy` are
  caught, which is what happened before. Eight instances in the differential
  sample, every one adjudicated as the rule correctly flagging invalid data.
- **E756 covers every dependent tier, not only `%x*`.** A tier line whose
  payload is absent or whitespace-only declares nothing. The rule always said
  that; only its name was `%x`-specific, and it could not be applied to a
  standard tier until the model could represent an empty one. Before this, an
  empty `%eng:` was read as VALID by the re2c backend and rejected by
  tree-sitter through an undescribed code, so the two backends disagreed about
  a file neither could explain. Zero instances in the differential sample: the
  construct is invalid CHAT and correspondingly rare.

  The rule now reaches EVERY tier whose grammar body is free text, which is
  every dependent tier except the structured ones (`%mor`, `%gra`, `%pho`,
  `%mod`, `%sin`, `%wor`), whose bodies are not free text and whose empty case
  fails earlier and more specifically. That boundary is a grammar fact, not a
  list: a tier qualifies exactly when its rule marks its body `optional(...)`.
  `%tim` gained an `Empty` state to make this expressible, since both of its
  content variants hold a non-empty string; the Phon tiers (`%xmodsyl`,
  `%xphosyl`, `%xphoaln`, `%xphoint`) answer from the word or group count they
  already reported.

### Fixed

- **An empty dependent tier is no longer papered over.** The re2c backend met
  `%eng:` with no content and substituted a single space, which made the tier
  look well formed and the whole FILE read as valid where tree-sitter reported
  errors. The model can now say that a tier declares nothing, so the parser
  reports what the file contains and E756 judges it.
- **An empty `%x` tier survives a roundtrip, and `normalize` no longer swallows
  the file.** `%xtst:` with no content reported E756 from the PARSE path and
  returned without adding the tier to the model, so the line vanished on
  roundtrip while an empty `%eng:` was preserved. Worse, because the report came
  from parsing rather than validation, `chatter normalize` treated the whole
  file as unparseable and wrote NOTHING. The parser now says what the file
  contains and the validator judges it, as it does for every other tier kind.
- **An empty `%tim:` is a `%tim` tier.** The re2c backend lowered it to an
  unsupported DEPENDENT TIER and reported E605, "unsupported dependent tier
  '%tim'", about a tier name that is perfectly supported; a whitespace-only body
  additionally drew E603 ("Invalid %tim tier format: ''") alongside E756, two
  codes for one fact and the more specific of them false. Same for an empty
  `%xphoaln:` and `%xphoint:`, which conflated an absent body with a malformed
  one. All four now report E756 on both backends.
- **The participants check reads the declaration.** An empty participant set
  used to disable the check rather than fail it, so the files least likely to
  be well formed were the ones exempted from the rule.
- **An annotation's separator is not part of its text.** `[=!  contacts]`,
  written with two spaces, parsed as `" contacts"` in one backend and
  `"contacts"` in the other. That was the last content-level disagreement
  between the two parser backends across all 107,403 corpus files.
- `chatter validate --format json` no longer writes cache housekeeping to
  stderr. Two facts leaked there: `Cleared N cache entries` on every `--force`
  run, and `note: pruned N unreachable cache row(s)...` whenever a prune fired.
  Both broke the documented promise that JSON mode's stderr is empty, and the
  test suite contained two tests with contradictory expectations about it, one
  requiring stderr empty and one asserting it contained the cleared count. The
  first only failed when a prune happened to fire, which is why both shipped
  green through four releases.

### Changed

- **Breaking (library): `TimTier` gained a third variant.** `TimTier::Empty
  { span }` represents a `%tim:` line that declares nothing, which neither
  `Parsed` nor `Unsupported` could hold: both carry a `NonEmptyString`. Code
  matching on `TimTier` exhaustively must add an arm. `TimTier::empty()`
  constructs one, `declared_content()` returns `None` for it (`as_str()` still
  flattens to `""` for `Display` and serialization), and the serde form is
  unchanged apart from `""` now deserializing to `Empty` instead of erroring.
- **Breaking (library): the `test-utils` feature is REMOVED**, and with it
  `ChatCleanedText::test_unchecked` and `ChatRawText::test_unchecked`. Not
  renamed: gone. This is the breaking change that bites FIRST, because cargo
  refuses to resolve a graph that asks for a feature which no longer exists, so
  it fails before anything compiles and is invisible to a "what will fail to
  build" scan. A consumer sees:

  ```
  package `X` depends on `talkbank-model` with feature `test-utils`
  but `talkbank-model` does not have that feature
  ```

  Build fixtures through the parser instead: `TreeSitterParser::parse_word`
  followed by `ChatCleanedText::from_word`. The hatch was removed because a type
  whose existence proves "this text came from a parsed AST" is only as strong as
  its weakest constructor, and one any dev-dependency could switch on was that
  constructor. Downstream adoption on the day of release found three fixtures
  that had been asserting on a shape production cannot emit (a terminator in a
  `words` list), passing only because the hatch let them fabricate it.
- **Breaking (library): `BulletContent::empty()`.** A named constructor for a
  payload that carries nothing, distinct from `from_text("")`, which fabricates
  an empty text segment that is not in the file. Additive; no existing call site
  changes.
- **NDJSON surface: a new record type.** Those facts now arrive on stdout as
  `{"type":"cache","action":"clear"|"prune"|"warning",...}`, emitted only when
  cache maintenance did something. Silencing them under `--format json` was
  considered and rejected: they are results a caller can act on. A consumer
  that ignores unknown `type` values needs no change; one that errors on an
  unrecognised `type` will see these. The `type` field's documented value set
  is now `"file"`, `"summary"`, `"cache"`, and the contract page says to treat
  unknown values as ignorable.
  See [Diagnostic contract](https://talkbank.github.io/chatter/chatter/integrating/diagnostic-contract.html).

## [0.11.0] - 2026-08-13

**Validation verdicts: CHANGED, in BOTH directions.** Six error codes that had
silently degraded to E316 "unparsable content" now report themselves again
(E202, E307, E311, E314, E370, E375), and two false positives are gone. If you
gate a pipeline on `validate`, diff your own corpus before upgrading; see
[What a Version Bump Promises](https://talkbank.github.io/chatter/chatter/integrating/versioning.html).

Adjudicated against real corpus data before shipping: the operator's corpus
differential over a 2158-file stratified sample reports byte-identical per-code
counts and no newly-failing roundtrips against v0.10.0. The changes below are
all on malformed input, which a curated corpus contains almost none of.

**Library APIs: BREAKING.** This release changes the public API in several
ways. The list below is from a mechanical diff of the public surface between
the two tags, made after the notes first shipped saying "additive" and then
being corrected twice as a downstream consumer hit one break after another. A
release note written from memory of a 415-file change is a guess; this one is a
measurement.

Removed items (6):

- `FormType::A`. The `@a` marker was retired by the corpus authority in 2024
  and is absent from the form-marker registry that now generates every site of
  that closed set; the variant survived only because sixteen hand-written
  copies of the list disagreed. No replacement: the construct is not CHAT.
- `ALL_MARKERS`, `all_markers_string`. Superseded by the same registry.
- `collect_bracketed_content`, `collect_bracketed_item`. Superseded by the
  typed traversal.
- `counts_for_tier_in_context`. Use `counts_for_tier`, now re-exported at
  `talkbank_model::alignment`.
- `iso`.

Added, and breaking for an exhaustive match:

- `FormType::Undeclared(String)` carries the raw text of a marker naming no
  declared form, so `word@zz` roundtrips instead of being silently rewritten to
  `word@z:zz`.

Changed signatures:

- `ChatFile::validate` and `validate_into` take `TranscriptName<'_>` rather
  than `Option<&str>`. `None` becomes `TranscriptName::Anonymous`; a real name
  becomes `TranscriptName::Named`. The `Option` could not say which of "no
  name" and "a name we failed to read" it meant, and both reached the same
  branch.

**Library APIs: additive.** `talkbank_model::alignment` re-exports
`walk_words`, `walk_words_mut` and `counts_for_tier`, which previously required
naming the `helpers` module.

### Fixed

- **A recovery node could displace an entire `tier_body`.** An utterance ending
  in " ." was told its terminator was missing (E305), and a retrace or bracket
  at utterance start took the rest of the line with it. The parser was reading
  its own recovery artefact as evidence about the user's file. The generated
  typed CST traversal is regenerated from a generator that no longer absorbs an
  ERROR child at whatever position its cursor had reached.

- **Six codes degraded to the E316 catch-all.** Which classifier a recovery node
  reached was decided by WHERE tree-sitter had put it, so the same construct was
  named precisely at utterance start and generically after spoken material.
  `MainTierRegion` is now stated by the caller that knows it, and every
  main-tier Unexpected sink routes through one owner.

- **E246 blamed a lengthening marker for a stray tab.** The classifier saw a `:`
  before the recovery node, and that `:` was the SPEAKER's. A tab inside the
  main tier now reports the tab.

- **E758 pointed at whitespace nowhere near a tab.** It claims "extra whitespace
  between the tab and the tier content"; filling that slot never established the
  adjacency the sentence asserts, so ordinary space between two words was
  reported as a leading-space violation. The span is now built only when it
  starts at the tab's end byte.

- **E754 retired.** `@l` and `@ls` no longer require a single character.

- **Windows: the content-catch-all gate reported every exempted file as new**,
  because repo-relative paths were compared with the host separator against a
  forward-slashed list.

### Changed

- Two wall-clock test assertions became hang detectors with order-of-magnitude
  ceilings; both were tuned to one machine and one of them turned the Windows
  matrix red doing correct work.

- `chatter-desktop` and `talkbank-llm` set `doctest = false`. Neither has doc
  examples, and each was paying a full rustdoc compile to run zero doctests.


## [0.10.0] - 2026-08-07

**Validation verdicts: CHANGED, in the stricter direction.** Two new error
codes reject retrace constructions that previously passed silently, and two
existing rules stopped being suppressed by the shape of the content they were
asked about. Adjudicated over all 107,376 corpus files: E377 fires 53 times in
42 files and E378 15 times in 12 files, all real transcription defects and
queued for data cleanup; restoring E372 and E704 costs **zero** new instances,
so those two are pure correctness.

**Library APIs: BREAKING.** `Retrace` loses its `annotations` field, both
content enums gain an `AnnotatedRetrace` variant, and per-word language records
lose `word_index`. Pre-1.0, so this is a minor bump.

Every fix below except the `merge` one is the same defect: a traversal carrying
its own private list of which content variants contain other content, plus a
catch-all arm for everything the list forgot. Five such traversals existed.

### Added

- **E377 `RetraceWithNoMaterial`.** A retracing marker whose material is
  another marker, so it retraces nothing of its own. One rule covers the
  unbracketed `на [//] [/] на` and the bracketed `<<a> [/]> [//]`, because the
  lowering folds both into the same tree; naming it for the shape rather than
  for a spelling is what makes that possible. Deliberately narrow: 11,163
  retraces in the corpora sit inside another retrace and only **4** wrap a lone
  marker, so a "no retrace inside a retrace" rule would have rejected ordinary
  stutter chains (`<the [/] the piece> [//] the people`) in exactly the aphasia
  and fluency corpora that study them.
- **E378 `RetraceWithoutWords`.** The retraced material must contain a word at
  some depth. Phrased against absent WORDS rather than a present event, so
  `<the floor on the &=laughs water> [//]` stays legal while `<&=sigh> [/]`
  does not. The boundary falls out of what the model already calls a word:
  `0det [/] 0det dog` is legal (an omitted determiner is lexical content),
  `<xxx> [/] xxx` is legal (untranscribed speech is speech), and
  `0 [=! snuffles] [/] ok` is not.

### Fixed

- **A retracing marker's position among its annotations was discarded.**
  `dog [* p:w] [/] dog` and `dog [/] [* p:w] dog` are different claims: the
  first codes the error on the abandoned attempt, the second on the retrace.
  chatter built the identical model for both and wrote the first back as the
  second. **12,226** places in the corpora put an annotation immediately before
  a retrace marker. A second adjacent marker had nowhere to go at all, so
  `на [//] [/] на` round-tripped as `на [/] на`, losing a marker outright in
  **105** places across 46 files, 31 of them bilingual or language-impairment
  corpora where disfluency is the research variable.
- **E704 (overlapping bullets) was silently disabled** on any utterance whose
  content held a retrace or a group. The predicate for "does this utterance say
  anything timeable" recursed into neither, so two speakers' bullets could
  overlap by a full second and report nothing whenever either line contained a
  retrace.
- **E372 (nested quotation) was invisible below every container except an
  annotated group.** `“a <“b”> [/] c”` is a quotation inside a retrace inside a
  quotation, and reported nothing.
- **Per-word language metadata skipped every word inside a quotation,
  phonological group, sign group or retrace**, in a tool whose per-word
  language resolution is the point. `hao3 “ni3” <ma> [/] ma` produced records
  for two words out of four.
- **The re2c backend diverged from tree-sitter on marker runs.** It split a run
  where tree-sitter folds it, dropped retrace markers on events, and never
  raised E377 at all, despite three doc comments saying it did. The
  parser-equivalence gate now covers all three.
- **`chatter merge` dropped donor `@Comment` rows** when the reference file had
  none of its own.
- **Five missing CHANGELOG link references.** Every release from v0.6.0 to
  v0.9.1 shipped a `## [X.Y.Z]` heading with no matching `[X.Y.Z]:` definition.
  It renders as literal bracketed text rather than as a broken link, so the
  book's link check reports zero errors and cannot see it. The version gate now
  requires both halves of the entry.

### Changed

- **`Retrace::annotations` is gone.** Annotated retraces are
  `AnnotatedRetrace(Box<Annotated<Retrace>>)` in both content enums, parallel
  to the existing `Group`/`AnnotatedGroup` pair. Parser lowering is now a left
  fold over the marker run, one wrapper per marker, which absorbed three
  hand-rolled copies of the same tail.
- **Adjacency is validated, not refused at parse time.** Folding the offending
  input faithfully means it still round-trips, so a file that trips E377 stays
  recoverable rather than being partly discarded during recovery.
- **Per-word language records no longer carry `word_index`**, and
  `get_word_language` is removed. The records are a `#[serde(transparent)]`
  list, so a consumer reads position with `enumerate()`. The stored index was a
  second representation of that position whose documentation claimed it matched
  the tier-alignment domains; it cannot, because `%mor` excludes retraces and
  `%pho` counts them, so no single integer indexes both.
- **`--parser re2c` is documented as reporting unreliable diagnostic
  positions.** The lexer emits spans and the parser discards them; until that is
  plumbed through, the flag is for cross-checking verdicts, not for locating
  them.

### Internal

- **Design rule 3 (no `_ =>` catch-all over the content enums) is now enforced
  by the compiler**, through `#![deny(clippy::wildcard_enum_match_arm)]` added
  per file as each is cleaned, seven so far. A reintroduced catch-all is a
  compile error at the exact line, which no scalar count could be.
  `cargo run -p talkbank-parser-tests --bin audit_content_catch_alls`
  inventories the 24 modules still to clean.
- **`ContentStructure` is the single owner of which content contains what**,
  carrying `WordRef` and `GroupRef` payloads so a caller can ask not merely
  whether something is a container but which one. That set had been encoded
  independently in 18 files, and two copies disagreeing about phonological and
  sign groups is what let E377 escape from inside `‹...›`.

## [0.9.1] - 2026-08-05

**Validation verdicts: UNCHANGED.** No rule was added, removed or altered, and
no file changes its valid/invalid verdict. This release completes the library
API that v0.9.0 closed the fields on, and every entry below was found by
compiling a real downstream consumer against v0.9.0, which is a gate this
project did not previously have.

### Added

- **`into_vec()`, `take()` and `retain()` on every collection newtype.** v0.9.0
  made these types' inner fields private but shipped only the READING half of
  the resulting API. With no consuming accessor there was no way to move the
  items out, so a consumer rebuilding a content list or resegmenting a file
  could only clone through `as_slice().to_vec()`, on paths that run per
  utterance; and with no `retain`, every caller wrote take-edit-rebuild by
  hand, which hands a closure a `&mut Vec<_>` and is `DerefMut` under another
  name. Downstream, these three methods delete three helper functions and
  sixteen copies of one incantation.
- **One owner for that API.** `collection_newtype_ops!` now emits the accessor
  set for all seventeen `Vec`-backed newtypes. They had drifted while
  hand-written: `into_vec` was on 6 of 17, `as_slice` on 11, `as_mut_slice` on
  6, so what a consumer could do depended on which type it happened to hold.
- **`TierContentItems` and `BracketedItems` are re-exported from `model`.**
  Both were `pub` but reachable only through a glob, so a consumer could not
  name the type to reconstruct one after its field closed.

### Fixed

- **A doc-comment claim that was not true.** Several comments said
  reconstruction "goes through `new`, where a future invariant would be
  enforced". Every one of these seventeen types also has `impl From<Vec<T>>`
  and `impl Deref<Target = Vec<T>>`, so `new` is not the only door and no
  invariant is enforceable on them today. Closing the fields prevents literal
  construction and destructuring, and nothing more. The docs now say that, and
  name the open question (whether `From` should become `TryFrom`) rather than
  implying it is already answered.
- **A test whose "unique" temporary directory repeated 98% of the time.** The
  name was the pid plus `SystemTime::now()`, but the pid is constant across a
  test binary and 19,584 of 20,000 consecutive `SystemTime` samples measured
  identical, so parallel tests shared a directory and one's cleanup deleted the
  other's file. Now a process-wide counter.

## [0.9.0] - 2026-08-05

**Validation verdicts: CHANGED, in the permissive direction.** Files that
earlier versions wrongly REJECTED now parse: an unquoted `@Media` filename may
contain dots, parentheses, interior spaces and non-ASCII characters. Nothing
that used to pass now fails. A comparison over a 2,136-file
stratified sample of the reference corpora reports no new error code and no
count increase on any code, and no newly-failing roundtrip file.

Three rules changed with no effect on any known transcript. E767 (new) reports
whitespace before the `@Media` comma; those files were already invalid, and what
changes is the diagnostic. E768 (new) cannot be reached from a `.cha` file at
all. E602 became E756 on empty user-defined tiers, a construct that occurs zero
times in the wild corpus.

This release closes the library's newtype surface ahead of 1.0, so it carries a
lot of breaking API change and very little behaviour change.

### Fixed

- **`@Media` rejected legal media filenames.** `media_filename` was an ASCII
  allowlist (`[a-zA-Z0-9_-]+`), so a dot, a space, a parenthesis or any non-ASCII
  character made the header fail to match. The failure surfaced as E330 "Missing
  media_type node" on a line that visibly ended in `, audio`, and as E525 about a
  header chatter had recognised perfectly well. A filename is now defined the way
  the format defines it, as everything up to the comma that introduces the media
  type, with the quoted form still available for URLs (which may contain commas).
  This was costing real transcription runs: a media file named in Chinese, or
  containing a space, could not be referenced at all.
- **A `%mor` tier could be silently dropped on an empty user-defined tier.**
  `UserDefinedDependentTier::content` was a `NonEmptyString`, so the model could
  not represent a `%x` tier with empty content and the two parsers disagreed about
  what to do with one. The state is now representable and rejected by a validation
  rule (E756) rather than being unrepresentable and handled twice.
- **The validation cache could panic on drop** inside an async runtime. This was
  the second half of the nesting bug fixed in 0.8.0: the first half covered the
  call, this one covers teardown.
- **A `%mor` clone was a no-op**, cloning a reference rather than the owning
  vector it was meant to copy.
- **A declared speaker with no `@ID` was reported as undeclared.** The
  "Speaker *X not declared in @Participants" check read the
  `@Participants`-to-`@ID` join rather than the `@Participants` header, so for
  a speaker declared without an `@ID` it asserted the opposite of the file. The
  missing `@ID` is a real fault and E522 already reported it correctly. The
  neighbouring "@Participants header missing or has no participants" check had
  the same confusion: an empty join and an absent header are different facts.
- **E767 never fired in the editor.** It was implemented as a file-level sweep,
  and the LSP calls `validate_headers_only`, which does not run those. Both
  `@Media` payload rules now live on the per-header dispatcher that every entry
  point calls, so the CLI and the editor report the same thing. The LSP's
  per-speaker code lens had a quieter version of the roster bug: a speaker
  without an `@ID` got no lens while speaking.
- **A spec file had been failing to load silently.**
  `E502_wor_cascade_regression.md` carried a malformed title, and the loader
  downgraded every load failure to a warning on stderr, so it simply left the
  corpus unnoticed. The loader now fails closed on a spec it cannot parse, and
  distinguishes a spec from the prose that shares its directory.

### Added

- **`ChatFile::declared_speakers()`** returns every speaker declared in
  `@Participants`, in declaration order, each enriched with its `@ID` metadata
  when present. `participants` is populated from the `@Participants`-to-`@ID`
  join, so a speaker declared without an `@ID` raised E522 and was then absent
  from the map: consumers saw fewer speakers than the file declares. Prefer this
  for "who is in this transcript"; `all_participants()` remains the `@ID` join.
- **`ChatFile::participant_entries()`**, the named `@Participants` extraction,
  alongside the existing `id_headers()`.
- **`MorWord::analysis()`** borrows the analysis half of a `%mor` item
  (`lemma[-Feature]*`) so a consumer whose token model keeps the tag and the
  analysis in separate fields need not serialize the whole item and strip the
  `POS|` prefix back off. `MorWord::write_chat` now delegates to it, so the two
  renderings cannot drift.
- **`DependentTierEntry::kind()`, `span()` and `content_span()`**, the last
  giving the byte range of a tier's content without its label or terminator.
- **`MediaFilename::parse()`, `unquoted()`, and `MediaFilenameProblem`.**
- **E767**: whitespace between the `@Media` filename and its comma. Reported
  from the validation layer so both parser front ends raise it from one
  implementation.
- **E768**: an `@Media` filename that cannot be written to a header and read
  back unchanged. Unreachable from CHAT by construction; it guards the JSON
  ingress, where a document can carry a value no transcript could express.
- **`string_newtype_read_impls!`**, the read and render surface shared by every
  string newtype, so a newtype WITH an invariant can share it instead of copying
  it.
- **`Status: unreachable_from_chat`** for error specs: a rule that IS
  implemented but that no CHAT input can trigger, so it carries no corpus
  fixture and owes a named out-of-corpus test instead. This closes a hole in
  the gate meant to stop an implemented rule shipping untested: a spec with no
  example used to fail to parse, and the loader turned that into a warning, so
  the gate never saw the one case it names. Both directions are now checked, a
  spec marked unreachable that carries an example is also an error.

### Changed

- **BREAKING: no model newtype exposes its inner field.** Every newtype in the
  model, including every one generated by `string_newtype!`, now has a private
  field. Code reading `.0` uses `as_str()` / `as_slice()` / `raw()`; code
  mutating through it uses the named accessors.
- **BREAKING: `DerefMut` is gone from the collection newtypes.** While it
  existed, a private field bought nothing: any caller could still push, clear or
  replace the contents. `as_mut_slice()` allows element mutation without allowing
  the collection to be resized.
- **BREAKING: `MediaHeader::new` takes a `MediaFilename`,** not
  `impl Into<MediaFilename>`, and `MediaFilename` has no `new`, no `From<&str>`
  and no `From<String>`. `parse` is the only way in. An `@Media` filename
  containing the delimiter was constructible, and `build_chat` built one.
- **BREAKING: `build_header_lines` and `build_media_header` are fallible,** and
  `BuildChatError` gains a `MediaFilename` variant, because a caller-supplied
  media name is external input that `@Media` cannot always represent.
- **BREAKING: `UserDefinedDependentTier::content` is no longer a
  `NonEmptyString`.**
- **BREAKING: the crates are edition 2024.**
- Deserialization of `MediaFilename` is lenient, like every other checked
  newtype in the model: the serde boundary reconstructs what the document held
  and validation reports the violation with a code and a span.


## [0.8.0] - 2026-08-03

**Validation verdicts: UNCHANGED.** No rule was added, removed or altered, and
no file changes its valid/invalid verdict in this release. What changes is that
the desktop app can run at all, that runs differing only in `--suppress` share a
cache again, and the library API named under "Changed" below.

### Fixed

- **Chatter Desktop can validate again.** Since v0.6.0 the desktop app could not
  start a run at all: it stopped on "Starting..." forever, on every machine and
  every folder. Tauri drives a command on its async runtime, and the validation
  cache bridges its synchronous API to an async database by owning a runtime and
  blocking on it; nesting runtimes panics, the panic unwound out of the command,
  and the IPC call then never resolved OR rejected, so the window had nothing to
  report and no error to show. The cache now runs such a call on a thread with no
  ambient runtime, so nesting cannot arise, and the desktop `validate` command
  always produces an outcome, reporting a panic as a failed run rather than as
  silence. The CLI was never affected. Introduced 2026-07-07; shipped in v0.6.0
  and v0.7.0.

### Changed

- **Desktop commands return typed errors instead of `String`.** Each command now
  names the failures it actually has (`TargetError`, `ValidationStartError`,
  `ClanError`, `InstallCliError`, `RevealError`, `ExportError`,
  `OpenExternalError`), so a failure can be matched on and carries its source
  error. Errors still cross the IPC boundary as the same display text, so
  nothing the user sees changes.

- **`--suppress` no longer throws the validation cache away.** Suppression is a
  presentation preference: it changes which diagnostics are printed, never which
  ones the validator computes. v0.6.0 folded the suppression set into the cache
  key, so every distinct `--suppress` list got its own private cache and
  `chatter validate ~/corpus` followed by `chatter validate --suppress xphon
  ~/corpus` re-validated all ~106,000 files from cold instead of hitting the
  cache. Runs that differ only in `--suppress` now share one cache;
  `--strict-linkers`, which genuinely turns extra checks on, still validates
  afresh. Suppression behaviour itself is unchanged: a suppressed code is not
  reported, and a file with other diagnostics still counts invalid.

- **The cache no longer grows without bound across releases.** Every read binds
  the current rules version, so rows written under a superseded one can never be
  matched again, yet nothing deleted them: only a 30-day age cutoff existed,
  which answers a different question. Each release therefore stranded a complete
  copy of the corpus in the database, which had reached 464,773 rows across 88
  versions (about 190 MB of a 243 MB file) for a corpus of ~106,000 files.
  Opening the cache now deletes rows outside a two-generation window (the
  current version plus the most recently written previous one, so a rollback or
  a bisect is not cold), rewrites the file so the space actually returns to the
  filesystem, and reports what it reclaimed.

- **Rule selection and presentation policy are now separate types.**
  `ValidationConfig` held both "which rules run" and "how diagnostics are shown",
  and the validation cache key was derived from the whole thing, which is what
  let a display preference partition the cache. It is replaced by
  `talkbank_model::RuleSelection` (what is computed; the only input to the cache
  key) and `talkbank_transform::PresentationPolicy` (what is shown). The cache
  crate cannot name the second, since the crate that owns it depends on the
  cache, so folding a display preference into the key is now a compile error
  rather than a judgement call.

  Library callers: `ChatFile::validate_with_config` and
  `validate_with_alignment_and_config` are now `validate_with_rules` and
  `validate_with_alignment_and_rules`, taking a `RuleSelection`, and they report
  the complete diagnostic set with nothing filtered. `ConfigurableErrorSink`
  moved to `talkbank_transform` and takes a `PresentationPolicy`. The validation
  runner's config field `model_config` is now the pair `rules` and
  `presentation`.

## [0.7.0] - 2026-08-03

**Validation verdicts: UNCHANGED.** No rule was added, removed or altered, and
no file changes its valid/invalid verdict in this release. What changes is what
a run REPORTS about itself when it does not complete normally.

### Changed

- **A validation run now always terminates its event stream, and says how.**
  Previously a run whose thread died emitted no terminal event at all, and the
  three surfaces each guessed differently: the CLI exited non-zero, the desktop
  app waited forever showing "Discovering files", and the TUI marked the run
  COMPLETE, so a dead run was presented as a finished one. Terminality now
  belongs to the runner, which guarantees a terminal event on every exit path,
  so all three surfaces inherit the same guarantee instead of reconstructing it.

- **A run that lost files can no longer report success.** A panicking worker was
  caught, logged where no graphical user could see it, and then ignored: the run
  reported `Finished` with partial statistics, so a 500 file corpus could
  validate 480 and be presented as "all valid". `Finished` now means every
  discovered file was accounted for and is the only basis for a claim about the
  whole input; a run that covered less reports `FinishedIncomplete` with the
  number of files lost. **`chatter validate` exits non-zero in that case**, where
  it previously exited 0.

- **Cancelling a run now cancels it.** The cancel request was a single token on a
  channel that the dispatch loop, every worker, and the end of run check each
  consumed destructively, so exactly one of them observed it: cancelling stopped
  one worker while the rest drained the queue, and the run's own statistics
  usually recorded that it had not been cancelled.

- **`ValidationEvent` gains `Aborted` and `FinishedIncomplete`** (BREAKING for
  library consumers). The enum is deliberately NOT `#[non_exhaustive]`: a
  consumer that upgrades gets a compile error and has to decide what a dead or
  partial run means for its own interface, rather than silently inheriting
  "pretend it finished", which is the defect these variants exist to fix.

### Fixed

- **Desktop: a run that never started looked identical to one in progress.** The
  app showed "Discovering files" from the moment it sent the request, and the
  backend's own discovery event set the same state, so a backend that never
  answered was indistinguishable from one still working. The two are now
  separate states: the app shows "Starting" until the validator actually
  responds, and says so if that takes more than a few seconds. A run stuck there
  is a start-up fault rather than anything about your files, which is worth
  quoting in a bug report.

- **Desktop: an aborted run is no longer a dead end.** It reports why it stopped
  and offers Re-validate, instead of leaving the window with no way forward.

## [0.6.0] - 2026-07-31

**Validation verdicts: CHANGED.** Files that earlier versions accepted may
now be rejected, and files they rejected may now be accepted. Both
directions occur in this release: the Phon `%x` fixes below remove false
rejects, while the removed error codes and the suppression fix change what
`validate` reports and what exit code it returns. Pin with `~` if you depend
on a fixed rule set.

### Added

- **`chatter fix`**, built on the span-splicing engine. It supersedes the
  deleted `chatter lint`: the old `lint --fix` is now `fix --apply`. `fix`
  covers the full fix catalog rather than three codes, applies fixes at
  exact byte spans validated against the source text, and repairs a clean
  utterance in a file whose other regions did not parse (the utterance
  containing an edit must have parsed clean, or the edit is refused and
  reported, never silently dropped). Every catalog entry carries a
  batch-safety tier and a bare `--apply` writes only the mechanical ones;
  a semantic fix is written only when its code is named with `--code`; an
  ambiguous fix is only ever reported, never written by this command.

### Removed

- **`chatter lint` (the `--fix` auto-fixer) is deleted.** It was a live
  span-driven byte writer built before the splice engine's safety
  guarantees existed: it read `error.location.span` with no dummy-span
  guard (`Span::DUMMY` is `{0,0}`, a real file offset), called
  `String::replace_range` with no `is_char_boundary` check (a panic on a
  non-character-boundary span), inserted its E301 terminator fix at zero
  width with no dummy-span guard either (corrupting the `@UTF8` header had
  one ever fired at offset 0), and detected no overlap between fixes. An
  audit found zero production callers (no `talkbank-tools` reference, no
  workspace script, no IISRP pipeline usage; only its own tests and the
  book mentioned it). `chatter fix` is its successor; see Added above.

- **Five `ErrorCode` variants**, each unreachable or redundant:
  `LongFeatureLabelMismatch`, `NonvocalLabelMismatch`, `UnexpectedTierNode`,
  `UnexpectedMorphologyNode`, and `LegacyWarning` (with its generated spec
  entry). Consumers matching on `ErrorCode` will see these gone.

- **Twelve of the language server's twenty-one quick fixes.** Each was
  attached to a code it did not repair: the action offered for a duplicate
  header inserted a missing one, and eleven others were similarly
  mismatched. The nine that remain repair the diagnostic they are attached
  to. Quick-fix matching now goes through parsed error codes rather than
  string literals, so a renamed code is a compile error instead of a
  silently dead action.

### Fixed

- **Phon `%x` dependent tiers, reconciled against the upstream spec.** Three
  fixes, two of which were false rejects on valid Phon output:
  - `%xphoaln` no longer requires its word count to equal `%mod`/`%pho`
    exactly. The spec allows a pause present on only one of the two tiers to
    consume a word slot only on the tier that contains it, so the counts
    legitimately differ by one.
  - Numeric inter-word pauses (`(1.5)`, `(1:05.2)`) are accepted on the
    syllabification tiers, alongside the three untimed forms. They were
    rejected on the grounds of being unattested in available corpora, which
    is not a basis for refusing a construct the spec declares legal.
  - Intra-word pauses (`^`, U+005E) are tokenized rather than absorbed into
    the neighbouring phone. A word-final `^` previously produced a spurious
    error, and a mid-word `^` silently became part of the following phone.
    Reconstruction preserves the pause in place, per the spec's rule that
    stripping each unit's `:CODE` and concatenating must reproduce the
    source word exactly.

- **`validate --suppress` no longer zeroes the invalid count and the exit
  code.** Suppressing a code removed it from the report AND from the
  tallies, so a file with genuine OTHER errors could be counted valid and the
  command could exit 0. A file that still has unsuppressed diagnostics now
  counts invalid and the command exits non-zero, as it should. A file whose
  every diagnostic was suppressed does count valid: that is what asking for
  those codes to be suppressed means.

- **The validation cache key covers every dimension of the verdict**,
  including parse behaviour and the active rule set, as a required
  parameter rather than a hand-picked subset. A cached verdict from one
  configuration could previously be served for another.

- **Strict parsing no longer discards the model it built** on failure, so a
  caller can inspect what parsed alongside the diagnostics.

### Changed

- **Diagnostic classification happens once, from the active rule set**,
  rather than being recomputed at three call sites that could disagree.

- **The diagnostic kind is generated from the spec** instead of mirrored in
  a hand-maintained match, and a divergence between the spec and the
  `ErrorCode` enum now fails the build in both directions rather than
  falling through to a default.

## [0.5.1] - 2026-07-30

### Fixed

- **`validate --force` was unusable at corpus scale** (v0.5.0 DOA): the
  cache refresh called `clear_prefix` once per resolved FILE, and each call
  scanned every `file_path` in the cache, so a corpus-sized invocation did
  quadratic work (on a 136k-file cache, effectively forever) at 100% CPU
  behind a blank screen before the progress display started. The refresh is
  now one batched `DELETE ... IN (...)` pass over the resolved file list,
  and `clear_prefix` itself became a single range-predicate statement
  instead of a scan-and-loop. Pinned by a real-CLI regression test that
  warms a 6,000-file cache and bounds the forced pass (old: 34s at that
  size; new: seconds, dominated by validation itself).

## [0.5.0] - 2026-07-30

### Removed

- **Two ungrounded CA-mode validation exemptions.** `@Options: CA` no longer
  disables the E241 illegal-untranscribed checks, nor the E701/E704 temporal
  checks (which it had skipped wholesale via an early return, while E362
  bullet monotonicity kept running on the same files). Neither skip had a
  CLAN CHECK counterpart: CHECK's whole CA behavior is three suppressed
  errors (21 terminator, 155 parenthesized word, 123 leading space), and
  chatter keeps exactly those three. Measured before removal on ALL 994 kept
  CA-declared files: both gates protected zero occurrences. The temporal
  skip's recorded rationale (leniency-policy Decision 6, false positives on
  CA reference files) no longer reproduces, since the temporal rules gained
  the 500 ms tolerance and per-speaker semantics; its Revisit line
  anticipated this removal. CA files with genuine timing defects or illegal
  untranscribed markers are now diagnosed like any other file.

### Fixed

- **E326 now says when the skipped line looks like a CHAT line pushed off
  column 1.** An indented dependent tier (` %mor:	...`) was reported as
  "Unsupported line skipped", accurate but useless: the reader hunts for junk
  when the fix is deleting one space. The message now names the shape ("looks
  like a dependent tier line pushed off column 1; it must begin at column 1")
  for tier-, main-tier-, and header-shaped lines, with a suggestion to remove
  the leading whitespace.

- **An annotated word's wrapper span was never set**, left `Span::DUMMY` at
  construction while the annotated event, action, and group paths all set a
  real one. Two consequences: any diagnostic located on an annotated word
  pointed at byte zero, and E757 could not see a bracketed code glued to the
  following word (`hello [!]there`) at all, because its detection is span
  adjacency. The wrapper now spans the word through the enclosing node's end,
  covering its trailing `[...]` codes, exactly as the retrace paths do.

### Changed

- **E757 now covers every bracketed code, not only retraces.** `hello [/]x`
  was rejected; `hello [!]x` and `bobo [= toy]x` were silently accepted,
  though they are the same defect and the code's own description already said
  "bracketed code". Juxtaposition-matrix cell 8, ruled REJECT 2026-07-18.
  Mirrored in the re2c front end, where a bare closing bracket joins the
  retrace tokens. The 2026-07-18 matrix scan found `][letter` unattested
  corpus-wide and the differential confirms no new instances, so no kept file
  is affected.

### Added

- **`talkbank-transform` gained a default-on `validation-runner` feature.**
  The corpus-scale validation runner is the crate's only SQL consumer (sqlx,
  via `talkbank-cache`), so it, that dependency, and the runner-only
  `crossbeam-channel`/`num_cpus` now sit behind the feature. Default builds
  are unchanged; a consumer that wants the transform surface without a SQL
  stack opts out with `default-features = false`. The path predicate
  `is_chat_transcript_path` moved to the feature-independent
  `talkbank_transform::paths` (still re-exported from `validation_runner`),
  since the corpus walk and CLI walks need it on every build.

- **E766, a linker placed after utterance content** (`yeah that go +" okay .`).
  Linkers connect an utterance to the previous one, so they are
  utterance-initial by definition; a misplaced one used to surface as generic
  unparsable content (E316), which gave the transcriber nothing to act on.
  The grammar now parses the misplaced linker into the CST (the same
  strict+catch-all pattern as the curly-quote rule) so the diagnostic names
  the construct at the exact token, in both parser front ends. One deliberate
  carve-out: a `++` glued to words on both sides (`un++do`) is a word run
  with an empty compound part and keeps its E233 diagnosis. A side effect of
  the grammar change is finer error recovery on several unparsable-content
  shapes: diagnostics that used to blame a whole line now land on the exact
  offending region (e.g. an unmatched `<` now yields E316 on `<word ` with
  the rest of the utterance parsed normally).

- **E765, a free-standing `:` or `;` separator, or a pause, glued to the item
  after it** (`:and`, `;;`, `(.)dog`). Same family and same span-adjacency
  mechanism as E764; the preceding side stays valid, since `word↘` and `dog,`
  are documented convention and `dog:` fuses into the word.

  Juxtaposition-matrix cell 7 was ruled REJECT for the whole separator class,
  against an estimate of roughly six affected files. A real-corpus comparison
  measured that reading at 270 new instances on a 2%, 2,134-file sample (about
  13,500 corpus-wide), every inspected one legitimate CA notation rather than a
  missing space: `≡` is latching and is written glued on both sides, and the
  intonation arrows attach to the material they mark, including directly before
  an overlap close. Adjudicated UNINTENDED, so the rule ships narrowed to the
  plain punctuation separators and pauses, where the differential is clean.
  Whether any CA mark should forbid trailing glue is left open, with receipts
  in the spec.

- **E764, a `&`-prefixed form glued to the preceding word** (`dog&-um`,
  `dog&~gaga`, `dog&+fr`). The shape parses as two words, because `&` cannot
  continue a word, so a missing space silently manufactures a word boundary
  that the transcriber did not write and nothing reported. Style rule in the
  E749/E751/E757 family, detected by span adjacency, mirrored in the re2c
  front end as a token scan. Glued omission (`dog0is`) is not this code: it
  yields one malformed word and E220 already rejects it.

  Juxtaposition-matrix cell 6, ruled REJECT 2026-07-18; zero main-tier
  attestations in the kept corpus at adoption, so no existing file is
  affected. Validator-only: grammar, model shape, and roundtrip behavior
  are untouched.

## [0.4.1] - 2026-07-27

### Fixed

- **`talkbank_transform::dependent_tiers::replace_or_add_tier` could not be
  called on an utterance.** It still took `SmallVec<[DependentTier; 3]>` after
  `DependentTierEntry` was introduced and `Utterance::dependent_tiers` became
  `SmallVec<[DependentTierEntry; 3]>`, so the one thing the helper exists to do
  no longer type-checked. It shipped in this state in 0.3.6 and 0.4.0.

  It compiled because it was internally consistent, and no test caught it
  because this workspace has no callers of it: the helper is public API for
  downstream consumers, and the only one was pinned to an older release. The
  regression guard added with the fix is a compile-time function taking
  `&mut Utterance`, so any future drift between the signature and the field
  fails the build rather than passing silently.

  On replace, the existing entry's `TierSeparator` is preserved (only the
  payload is regenerated, and the separator is the provenance E758 is detected
  from); on append the new entry is `CLEAN`. Serialization canonicalizes to a
  single tab either way, so this affects diagnostics, not output.

## [0.4.0] - 2026-07-27

### Added

- Three validation rules that catch real, previously-invisible defects in
  transcript data. All three live entirely in the validator: the grammar,
  the model's serialized shape, and roundtrip behavior are untouched.

  - **E761, `%gra` relation head is not a Universal Dependencies
    relation.** A `%gra` label is `HEAD` or `HEAD-SUBTYPE`; UD fixes the
    head set at 37 universal relations and leaves subtypes open and
    language-specific, so the head is checked against that closed set and
    the subtype is never checked. Nothing validated relation labels before,
    in chatter or in CLAN CHECK, so a corrupted label rode silently into
    every downstream analysis that reads the dependency graph. Grounded in
    a survey of the entire corpus (138,565,864 relation instances across
    106,158 files): all 37 universal heads are attested, 150 distinct
    labels occur, and exactly three heads fall outside the set, all of them
    defects (`IOB` for `IOBJ`, `PAD`, `PUNCTT` for `PUNCT`).

  - **E762, the prefix marker `#` stands alone as a word or opens one.**
    The marker attaches to the END of the prefix it marks, and the prefix
    is a word of its own (Hebrew `ha# kelev`), so neither shape can be that
    construct in any language. Language-independent, and zero-attested
    corpus-wide.

  - **E763, prefix marker in a language that does not use it.** Gated on
    the WORD's resolved language rather than the file's `@Languages`
    header, exactly as the digits rule (E220) is, so a code-switched word
    brings its own rules with it. Languages that write the marker: `heb`,
    `ara`. Word-internal markers stay legal wherever the language allows
    the marker at all.

- `TreeSitterParser` now implements the shared `ChatParser` trait
  (`talkbank_model::ChatParser`), making the two parser backends
  interchangeable behind one generic bound at every granularity (file,
  header, utterance, tier, word, relation). Previously only
  `Re2cParser` implemented the trait, so consumers selecting a backend
  per target (tree-sitter natively, pure-Rust re2c on wasm) had to
  hand-roll a cfg-gated facade. Every trait method delegates to the
  matching inherent `parse_*_fragment` method, so trait-path and
  inherent-path behavior are identical; conformance is pinned by
  `talkbank-parser/tests/chat_parser_trait.rs`.

- Dedicated error codes for two malformations that previously fell
  through to the generic E316 unparsable-content catch-all, from the
  CHECK-parity adjudication of CLAN CHECK errors 52 and 11: E759 (an
  utterance beginning with a postfix annotation such as `[/]`, `[<]`,
  or `[: text]`, which has no preceding material to scope over) and
  E760 (a `%mor` item with an empty part-of-speech field, `|we`). Both
  are recognized by the tree-sitter front end's error analysis and
  mirrored in the re2c oracle's front end; both files were already
  rejected, so no validity verdict changes, only the diagnosis.

### Removed

- nextest. CI, the cross-platform workflows and the documented local
  commands run plain `cargo test`, with job time limits in place of
  nextest's hang guard.
- **TalkBank XML support, in full.** The `to-xml` command, the
  `talkbank_transform::xml` emitter, the `corpus/reference-xml/` golden
  corpus, the `xml_golden` and `xml_schema_validate` suites, the bundled
  `talkbank.xsd`/`xml.xsd` schemas, the XML Emitter book chapter, and the
  `quick-xml` dependency.

  TalkBank stopped generating TalkBank XML on 2025-10-29, when its last
  consumer said he no longer used it, and the published `data-xml/`
  distribution has been offline since. Phon moved off the format some time
  ago. Nothing produced by this emitter had a consumer.

  **Breaking:** `chatter to-xml` no longer exists and there is no
  replacement. Use `chatter to-json`, which is the format the toolchain
  actually maintains. `talkbank_transform::xml::XmlWriteError` is gone from
  the public API surface.

### Changed

- The `%gra` documentation, examples, reference corpus, and error-spec
  fixtures no longer use retired TalkBank relation labels (`SUBJ`, `JCT`,
  `POBJ`, `COM`, `VOC`, `MOD`, `NEG`, `PRED`, `COMP`, `ADV`, `INCROOT`,
  `QUANT`, `LINK`), which E761 now rejects. None of them occurs anywhere in
  the real corpora; they were fixture inventions that would have taught
  readers of the API docs a vocabulary the validator rejects. Replaced
  throughout by the UD relations the corpora actually use (`NSUBJ`, `OBL`,
  `CASE`, `DISCOURSE`, `VOCATIVE`, `AMOD`, `ADVMOD-NEG`, `CCOMP`, `EXPL`,
  `DET`, `DEP`). The `gra_incroot` grammar construct deliberately keeps
  `INCROOT`: it pins the property that relation labels are open text at the
  grammar layer, which is why the vocabulary is a validation policy and not
  a syntax.

### Fixed

- **Word validation now reaches words nested inside groups.** Main-tier
  validation iterated content items flatly and matched only `Word`,
  `AnnotatedWord` and `ReplacedWord`, with a catch-all that silently
  discarded every container, so a word inside a retrace, a reformulation,
  an angle group or a quotation was never word-validated at all.

  The symptom: the identical token was rejected outside a group and
  accepted inside one. In English `hello3 dog .` was invalid (E220) while
  `hello3 [/] hello dog .` was valid, on every release up to this one.
  Every word-level rule inherited the hole, so E220 has carried it for as
  long as the rule has existed; the newer prefix-marker rules inherited it
  on arrival.

  Corpus impact, measured over all 106,158 files: 341 to 348 invalid files,
  8 new error instances across 7 files (E241 x2, E252 x4, E248 x1,
  E763 x1), each a pre-existing data defect that had been hiding inside a
  group rather than any change in what counts as valid CHAT.


- `ErrorCollector::is_empty()` violated the standard Rust contract
  `len() == 0 <=> is_empty()`: it answered "is the internal buffer
  unallocated?", so a collector created with `with_capacity` (which
  pre-allocates) reported non-empty while holding zero errors. Found
  by the 1.0 contract-set API audit; now implemented as `len() == 0`
  with a regression test.

- `TreeSitterParser::parse_gra_relation_fragment` (and the trait's
  `parse_gra_relation`) rejected EVERY bare `%gra` relation and leaked
  a spurious E709 diagnostic into the caller's sink, because the
  wrapper appended a scaffold terminator with the never-valid index 0
  (`0|0|PUNCT`) and the tier wrapper rejects on any internal
  diagnostic. The scaffold is now valid CHAT (`2|1|PUNCT`), and a
  scaffold-region filter guarantees diagnostics against wrapper
  scaffolding can never reach the caller. The re2c backend was
  unaffected (it parses the relation directly); the fix restores
  backend agreement. Caught by the new `ChatParser` trait conformance
  test.

- Validation cache: initialization is now concurrency-safe across
  processes, not just threads. Every opener takes an exclusive advisory
  file lock (`talkbank-cache.init.lock`, beside the database) around
  first-time create + migrate, so parallel `chatter` runs (or parallel
  test processes) sharing one cache directory can no longer race sqlx's
  SQLite migration (`UNIQUE constraint failed: _sqlx_migrations.version`,
  the 2026-07-13 flake) or collide on first-connection WAL setup. Lock
  acquisition is bounded: on timeout, opening fails with the new typed
  `CacheError::InitLockTimeout` and the CLI degrades to running
  uncached instead of blocking. The 2026-07-13 bounded retry is
  retained as a backstop for older builds that share the cache
  directory without honoring the lock protocol. Regression coverage:
  a cross-process stress test races 8 processes over a fresh cache
  directory for 4 rounds under a hard deadline, so both failure modes
  (constraint error and hang) fail the suite instead of flaking or
  wedging it.

- Desktop release: the macOS updater bundle is now uploaded under a
  per-arch asset name (`Chatter-<target>.app.tar.gz`). Previously both
  the aarch64 and x86_64 macOS jobs uploaded the arch-independent
  `Chatter.app.tar.gz`, which raced on the shared release asset (the
  v0.3.6 `release-desktop` upload failure) and pointed both darwin
  entries in `latest.json` at a single URL holding one arch's binary.
  Fresh `.dmg` downloads were unaffected; the desktop auto-updater is
  the surface this corrects. (Ships with the next release.)
- Public API: a downstream crate that depends only on `talkbank-parser`
  can now name the error type of its parse methods. The six
  `TreeSitterParser::parse_*` methods return
  `ParseResult<T> = Result<T, ParseErrors>`, but `ParseErrors` /
  `ParseResult` were not reachable from the `talkbank-parser` crate root
  (only via a `pub(crate)` module), forcing consumers to add a separate
  `talkbank-model` dependency or stringify at the boundary; both are now
  re-exported. Also re-exported `talkbank_model::SylWordError` (the error
  of `classify_syl_word` / `tokenize_syl_word`), which was omitted from
  the model root while its sibling phon parse-error types were present.
  Completes the BUG-3 audit: a compile-test now names every public
  fallible constructor's error type so this class cannot regress.

## [0.3.6] - 2026-07-17

### Fixed

- The Phon `%x`-tier content checks (introduced with the %x fold-in)
  no longer mass-flag valid Phon exports. Two wild-corpus conventions
  the original specification never confronted are now accepted:
  (1) pause fillers (`(.)`, `(..)`, `(...)`) mirrored at the same word
  position on `%mod`/`%pho`/`%xmodsyl`/`%xphosyl` (and as pause pairs
  on `%xphoaln`) to keep word-aligned tiers in index lockstep, which
  E735 previously rejected as malformed `phone:CODE` units (roughly
  13,000 spurious errors across the PhonBank corpora); and
  (2) `^` and IPA `.` syllable-boundary notation in `%mod`/`%pho`
  words, which the segment-level `%xphoaln` reconstruction comparison
  now ignores exactly as it already ignored stress markers (roughly
  770 spurious E740/E741). Genuine misalignments (index-shift chains,
  pause fillers standing in for real words) are still reported. Users
  who adopted `--suppress xphon` to silence the storm can remove it
  and regain the genuine `%x`-tier checks.
- Generated error-documentation pages (`docs/errors/`) no longer fuse
  words across wrapped spec lines or drop backticked text: the spec
  text extractor now renders soft line breaks as spaces and includes
  inline code spans.

### Added

- New validation rule E752: timing bullets without an `@Media` header.
  A transcript carrying timing evidence (utterance bullets or `%wor`
  word timing) must declare the media those timestamps index; completes
  the media-consistency family (E544: declared linkage without timing;
  E552: declared `unlinked` contradicted by timing). Mirrors CLAN CHECK
  error 112.
- New validation rule E753: a word consisting only of a repetition
  segment (fully `↫...↫`-wrapped, no stem outside the delimiters) is
  rejected; word-category prefixes (`&-` filler, `&~` nonword, `0`
  omission) count as a stem. Adopted from GUI CLAN CHECK error 151 as
  a chatter-authority rule (the unix CHECK build never enforced it).
- New validation rule E754: the `@l` letter form must carry exactly one
  letter of stem (`b@l`); multi-letter content belongs under `@k` /
  `@ls`. Repeated-segment material (`↫b^↫b@l`) does not count toward
  the stem, matching real CLAN CHECK behavior. Mirrors CLAN CHECK
  error 76.
- New validation rule E755: a `[- CODE]` utterance-level language must
  be declared in `@Languages` (utterance-level presence is
  substantial). Mirrors CLAN CHECK error 152.

- Word-level explicit language codes (`word@s:CODE`) are now validated
  against the ISO 639-3 registry (E519), the same rule that guards
  `@Languages` and `@ID`; declaration in `@Languages` remains not
  required.

- `@L1 of` values are now typed ISO 639-3 language codes and validated
  against the registry (E519), completing registry validation at every
  position language codes appear. Wild usage was already uniformly
  codes; generation via `build_chat` now takes a `LanguageCode` for the
  participant first language.

- E756 (empty user-defined `%x` tier) replaces W601: the rejection is
  unchanged; the old code fired as a hard error despite its warning
  prefix, so the number was the bug. The diagnostic message also no
  longer double-prefixes the tier name (`%xfoo`, not `%xxfoo`).

### Removed

- `W210` and `W211` are retired, and their numbers are not reused. No
  production code path emitted either; CLAN CHECK accepts W210's
  glued-terminator construct, and W211's shape is valid overlap-hugging CA
  notation. The JSON schema no longer lists them.
- The E254 warning (word-level `@s:CODE` not listed in `@Languages`)
  is retired: an explicit word-level language code is self-contained
  and deliberately carries no declaration requirement. `@Languages`
  declares the transcript's substantial languages; a one-word
  insertion is not substantial presence. (This matches CLAN CHECK,
  which dropped its own `@s` declaration requirement in 2019.)

<!--
Deferred to a later release:
- Word-content validity: reject junk inside words (`|`, ideographic comma,
  mojibake, ...) per the curated word-segment allowlist. Pending adjudication.
- CHECK-parity endgame closes (48 illegal `|`, 76 single-letter `@l`) and the
  remaining per-rule decisions.
-->


## [0.3.5] - 2026-07-15

Emergency release restoring corpus-correct word parsing. Versions
0.3.3 and 0.3.4 have been YANKED (releases and tags removed).

### Fixed

- Reverted the whitespace-boundary overlap-custody grammar introduced
  in 0.3.3. Its GLR-arbitrated word readings fragmented words carrying
  four or more glued markers (for example multi-syllable-pause chains
  like `or^ga^ni^zi^ra`), causing spurious E252/E331/E600/E705
  validation errors across real corpora and, worse, a serialization
  mutation (a space inserted into such words on rewrite). Word parsing
  is restored to the 0.3.2 grammar, verified by an error-code
  differential and a roundtrip comparison against the 0.3.2 binary
  over a corpus sample: identical profiles.
- A regression test pins that multi-marker words parse as one word and
  validate cleanly.

### Retained from the yanked releases

- Typed `@u` phonetic word forms (UNIBET).
- The `build_chat` header emitters and @ID demographics fix.
- The shared English capitalization transform.
- The long-tier stack-overflow fix and its regression test.
- The SQLite cache concurrency-safety fix; CI runs under nextest.

## [0.3.4] - 2026-07-15 [YANKED]

### Added

- **`@u` phonetic forms are now typed phonetic content.** A `@u` word
  (a UNIBET/IPA phonetic transcription standing in a word slot, e.g.
  the spoken side of an aphasia `[: target]` replacement) now models
  its content as a dedicated `WordContent::Phonetic(WordPhonetic)`
  node instead of orthographic text, in both parsers. Orthographic
  word-hygiene rules structurally cannot apply to phonetic content;
  the phonetic string itself stays deliberately lenient (IPA, ASCII
  UNIBET, X-SAMPA), matching the `%pho` tier's stance. `to-json`
  emits `{"type": "phonetic", ...}` for these nodes (schema updated);
  `cleaned_text` remains the phonetic string verbatim; the sanitizer
  redacts phonetic forms like spoken text. Scope is `@u` only;
  sibling special forms remain orthographic words.

- **`build_chat` now emits the full standard header set.** The general
  CHAT-generation schema (`TranscriptDescription` / `ParticipantDesc`)
  gained typed optional fields for `@Date`, `@Situation`, `@Options`,
  `@Transcriber`, `@Comment`, per-speaker `@L1 of`, and `@PID`
  (preserved from a source, never minted), each emitted in canonical
  header order. `@ID` demographics (age, sex, group, SES, education,
  custom) are now carried through `ParticipantDesc` instead of being
  silently dropped, fixing empty demographic slots in generated `@ID`
  headers.
- **Shared English capitalization transform**
  (`talkbank_transform::capitalize`): capitalizes the pronoun "I"
  family and the first real word of each utterance on the typed model,
  for generators whose sources are all-lowercase (improves downstream
  `%mor` accuracy). Token-level helpers are public for generators that
  capitalize their own word representation.

### Fixed

- **`chatter validate` no longer headlines a warnings-only file as an
  error.** A file whose findings are all warnings (which is valid CHAT,
  and was already counted valid in the summary) now prints
  `⚠ Warnings in <file>` instead of the contradictory
  `✗ Errors found in <file>`, and the "fix structural errors first"
  hint fires only on hard errors. Presentation only; validation logic
  unchanged.
- **The validation cache no longer fails to initialize when opened
  concurrently.** Two `chatter` runs sharing a cache directory (or a
  multi-threaded consumer) could race the one-time SQLite setup and hit
  `UNIQUE constraint failed: _sqlx_migrations.version` or a WAL init
  collision, silently disabling caching for that run. Concurrent opens on a
  fresh cache directory now retry the transient init race and all succeed.

## [0.3.3] - 2026-07-13 [YANKED]

### Added

- **Desktop app: a "Check for Updates..." menu item and a periodic background
  update check.** The app previously checked for a new release only at launch,
  so an app that was rarely relaunched could sit far behind. It now also checks
  every six hours in the background, and the app menu has a manual "Check for
  Updates..." item that reports when you are already up to date.
- **Desktop app: a real "About Chatter" panel** with the version, a short
  description, and clickable links to the TalkBank site and the source
  repository, replacing the bare version-only default.
- **`talkbank_transform::build_chat`: assemble a validated CHAT file from a
  typed transcript description.** Given participants, optional media, and
  utterances as pre-formatted CHAT main-tier text (`TranscriptDescription`),
  it synthesizes the header block, parses each utterance through the
  tree-sitter parser, and returns a `ChatFile`. The description carries a
  `media_status`, so a transcript that names its media but has no timing
  bullets yet (pre-forced-alignment) can emit `@Media: <id>, audio, unlinked`
  and stay valid instead of falsely claiming linkage (E544).
- **`talkbank_transform::num_words::expand_number`: spell digit tokens as
  language-appropriate number words** (13 lookup-table languages, CJK, and
  English ordinals/decades), so generated CHAT satisfies E220 (numeric digits
  are not allowed in words for languages that do not permit them).

### Changed

- **Overlap custody now follows whitespace boundaries, with canonical overlap
  serialization.** Overlap markers bind to the token on the correct side of a
  whitespace boundary, and serialization emits a single canonical form.
- **tree-sitter updated to 0.26.11** across the workspace (CLI, grammar
  bindings, and the generated parser).

### Fixed

- **Long dependent-tier reconstruction is now linear-time.** A quadratic blowup
  on very long utterance tiers is eliminated; pathological inputs that
  previously stalled the parser now reconstruct in linear time.
- **Desktop app: the validation settings popover no longer opens hidden behind
  the results panel.** It was rendered below the panels in the stacking order;
  it now sits above them.
- **Desktop app: the "up to date" dialog now dismisses on the first OK.** A
  listener leak (an async menu subscription whose cleanup could run before it
  resolved) let duplicate listeners accumulate, so one menu click stacked
  several identical dialogs.

## [0.3.2] - 2026-07-10

### Added

- **`chatter rediarize`: repair speaker attribution from external
  diarization turns.** Takes a transcript whose utterance timing is
  trusted but whose speaker labels are not, plus a speaker-turns JSON
  file (`{"source": ..., "turns": [{"track", "start_ms", "end_ms"}]}`)
  from an external diarizer, and re-attributes each timed utterance to
  the dominant overlapping turn. Utterances with no turn coverage are
  flagged, never guessed. Reconciled `@ID` rows are inserted in the
  header block. `--summary-json` emits a machine-readable outcome
  summary (per-utterance reattributions and flag reasons) for
  downstream tooling.
- **Four validation rules for constructs that do not make sense**,
  each adjudicated against real CLAN CHECK behavior and the wild
  corpus: E748 leading-zero media-bullet times; E749 comma glued to
  the following word; E750 whitespace inside angle-group delimiters;
  E751 pause marker glued to a word.

### Fixed

- The re2c oracle lexer now tokenizes short-form parenthesized
  material the same way the canonical parser does (its catch-all
  previously swallowed a trailing delimiter), keeping the two
  independent parsers in cross-check agreement on the new spacing
  rules.

### Changed

- Rust toolchain pin bumped to 1.97.0 (CI workflow pins synced);
  workspace and spec lockfiles refreshed; desktop dependency bumps
  (jsonschema 0.47, TypeScript 7).
- Documentation: an architecture page on overlap-marker binding (why
  edge-adjacent overlap markers bind into words, the ideal top-level
  model, and the conversion-layer path); the grammar's empty-`extras`
  (all-whitespace-explicit) design rationale is now recorded at the
  declaration site.

## [0.3.1] - 2026-07-08

### Fixed

- **Every public fallible constructor's error type is now publicly
  nameable.** `LanguageCodeError` (from `LanguageCode::new`),
  `XphointParseError`, and `PhoalnParseError` were not re-exported, so
  downstream crates could not store them in typed `#[source]` fields and
  had to stringify at the boundary; found by the first real downstream
  consumption of the 0.3.0 API. A new API-surface guard test pins the
  contract so a constructor error type can never silently become
  unnameable again.

## [0.3.0] - 2026-07-07

### Added

- **`--llm-cache <file>` (env `CHATTER_LLM_CACHE`) for holistic speaker-id
  judgment.** A persistent, write-through JSON response cache for
  `speaker-id` / `pipeline` / `batch --judgment holistic`: an identical
  request (same endpoint, model, and rendered prompt) is served from the
  cache instead of making another LLM call, so re-running a batch after a
  crash or an unrelated code change does not re-pay completed sessions.
  Absent flag and env variable means uncached, unchanged from before.

### Fixed

- **`chatter batch` no longer reports holistic suggestions as merges.** In
  holistic-judgment mode the per-session pipeline exits 0 after writing a
  suggestion to the pending file without merging (the operator adjudicates
  first); the batch summary counted those as "merged" and reported zero
  pending work. Outcomes are now classified by whether the merged output
  actually exists, and the summary separately counts merges, suggestions
  awaiting adjudication, and low-confidence refusals awaiting adjudication.
- **E552 (`@Media` says `unlinked` but timing exists) now says where the
  timing was found and how to fix it.** When the only timing evidence is
  word-level bullets inside a `%wor` tier (invisible in normal display), the
  message names the `%wor` tier and offers both remedies (the media is in
  fact aligned: remove `unlinked`; or the `%wor` tier is stale: remove it)
  instead of asserting the media is linked and pointing at bullets the user
  cannot see. The main-tier-bullet case keeps its direct advice.
- **Chatter Desktop's single-file validation now shares the CLI's validation
  engine.** Previously, validating a single `.cha` file in the desktop app
  (as opposed to its parent folder) bypassed the on-disk cache entirely,
  skipped the `@Media`-filename check (E531), and could not honor
  `--roundtrip` / `--parser` / `--strict-linkers`. All of these now work
  identically to `chatter validate` and to the desktop's own folder
  validation, and a new **Settings** panel exposes the equivalent options.
- **Chatter Desktop no longer shows "N files, all valid" before a run has
  actually finished.** The file tree previously derived this message from
  the partial, still-streaming result set, so it could flash "all valid"
  mid-run whenever no error had streamed in yet.

## [0.2.1] - 2026-06-24

### Added

- **The `talkbank-lsp` language server now ships as a standalone release
  artifact.** Prebuilt, code-signed `talkbank-lsp` binaries for macOS (Apple
  Silicon and Intel), Linux (x86_64 and aarch64, static musl), and Windows are
  attached to the GitHub Release, each with its own `talkbank-lsp-installer.sh`
  / `talkbank-lsp-installer.ps1`. Any LSP-aware editor can now install the server
  without building it from source; it is a first-class artifact in its own right,
  not only the binary the VS Code extension bundles per platform.

## [0.2.0] - 2026-06-23

### Added

- **More of CLAN CHECK's invalidity is now enforced.** A batch of CHECK-parity
  rules was implemented so `chatter validate` rejects more invalid CHAT:
  - `E514`: an `@ID` line's corpus field is required (CHECK 63).
  - `E547`: a constant participant header must follow the `@ID` block.
  - `E548`: closes the case CHECK 126 covers.
  - `E549`: a speaker may not be declared twice (CHECK 13).
  - Duplicate `@ID` lines and out-of-order `@Options` fields (CHECK 13, 125).
  - A dependent tier used without being declared (CHECK 17).
  - An out-of-range `@Time Duration` (CHECK 35).
  - An `@Media` header marked unlinked while the transcript still carries timing
    bullets (CHECK 124), and an `@Media` filename that does not match the data
    file (CHECK 157).
  - A replacement `[: ...]` now requires a preceding space (CHECK 161).
  - Tree-sitter recovery nodes are surfaced as invalidity rather than silently
    repaired: a surviving `ERROR` node maps to `E316` and a `MISSING` node to
    `E342` (with the re2c oracle mirroring it), covering a group with no
    annotation and swallowed recovery nodes inside comma-list headers
    (CHECK 5/6/106/108).
- **Phon:** `U` (unknown) is accepted as a legal syllable-constituent code on the
  `%xmodsyl` and `%xphosyl` tiers.
- A formal behavioral CHECK-validity parity test suite that runs real CLAN CHECK
  and chatter on the same fixtures and fails if either side drifts.

### Changed

- **`chatter update` now self-updates in process.** It embeds the axoupdater
  self-updater as a library, reads the cargo-dist install receipt (keyed by the
  package name), and replaces the running binary from GitHub Releases. This
  removes the package-name coupling that previously made `chatter update` report
  "not installed" on a correctly installed binary.
- **The CLI package is renamed `talkbank-cli` to `chatter`** (the crate now lives
  at `crates/chatter/`). The generated install scripts are therefore
  `chatter-installer.sh` and `chatter-installer.ps1` (previously
  `talkbank-cli-installer.*`); update any pinned install URL accordingly. The
  binary is still `chatter`, and the library/API crates keep their `talkbank-*`
  names.
- **Validation is stricter.** Because of the new CHECK-parity rules above, some
  files that passed `chatter validate` under 0.1.1 may now report errors. This is
  intended: chatter is the CHAT-validity authority and is at least as strict as
  CLAN CHECK.

- Word-level explicit language codes (`word@s:CODE`) are now validated
  against the ISO 639-3 registry (E519), the same rule that guards
  `@Languages` and `@ID`; declaration in `@Languages` remains not
  required.

### Removed

- The standalone self-updater binary (cargo-dist `install-updater = false`). The
  `chatter update` subcommand is unchanged for users; it now updates in process
  instead of shelling out to a separate program.

### Fixed

- The recovery-node invalidity backstop is scoped to localized errors so it does
  not over-flag, and several malformed `@ID` test fixtures were corrected.
- Hardened the CHECK-parity audit and corrected a CHECK 126 verdict it had
  falsely certified; the curated CHECK error-code map is restored in place of a
  brittle keyword heuristic.

## [0.1.1] - 2026-06-22

### Fixed

- **Validation cache could serve a stale verdict across rule-set changes.**
  `chatter validate` keyed its result cache on the cache crate's package
  version, which does not change when validation rules change, so a "Valid"
  result cached before a new rule (such as a retrace-marker check) existed kept
  being served, while a fresh conversion of the same bytes correctly rejected
  them. The cache key now folds in a fingerprint over every error-code rule, so
  adding, removing, or renaming any rule invalidates stale entries; the cache
  is kept and still functions, only keyed correctly.
- CLI usage lines pin the binary name to `chatter` regardless of the invoked
  path (clap `bin_name`).
- The book renders Mermaid diagrams again (restored mdbook-mermaid assets).
- **Desktop app version is now locked to the release version.** The desktop
  bundle (`.dmg` / `.exe` / `.deb`) and the Tauri auto-updater manifest now report
  the same version as the CLI. A version-sync gate (`scripts/sync-app-version.py`,
  enforced in CI and at release time) keeps `tauri.conf.json`, `package.json`, the
  workspace version, and this changelog from drifting, so the updater can never
  again advertise a version the installed bundle does not match.

### Changed

- CI book toolchain bumped to mdBook 0.5.3 and mdbook-mermaid 0.17.0.
- Build: force `serialize-javascript >= 7.0.5` to clear advisories, and bump
  `rand` in the spec crate.
- Docs: the book intro is de-staged for the public release (download-first).

## [0.1.0] - 2026-06-15

First public release.

### Added

- **CHAT-format core.** A strict, incremental tree-sitter parser
  (`talkbank-parser`) with an independent re2c oracle parser
  (`talkbank-parser-re2c`) that cross-checks it on every file; a typed
  CHAT data model with structured validation, error codes, and tier
  alignment (`talkbank-model`); and CHAT-to-JSON / JSON-to-CHAT / XML
  conversion, normalization, transcript-merge, and redaction pipelines
  (`talkbank-transform`).
- **Phon extension tiers.** The four Phon `%x` dependent tiers
  (`%xmodsyl`, `%xphosyl`, `%xphoaln`, `%xphoint`) are parsed and
  validated as first-class CHAT tiers, on by default (pass
  `--suppress xphon` to opt out): syllabification constituent codes and
  phone-vs-source reconstruction, model-to-actual phone alignment, and
  per-phone time intervals, with dedicated error codes.
- **`chatter` CLI.** `validate`, `normalize`, `to-json` / `from-json` /
  `to-xml`, `merge`, `speaker-id`, `batch`, `pipeline`, `adjudicate`,
  `sanity-scan`, `lint`, `clean`, `watch`, `new-file`, `show-alignment`,
  `validate-utseg`, `schema`, `update`, and a content cache.
- **Language server** (`talkbank-lsp`): real-time validation, hover,
  go-to-definition, and cross-tier alignment for any LSP-aware editor.
- **Desktop app** (`Chatter`): a Tauri-based CHAT validation app, shipping
  in the coordinated release alongside the CLI.
- **Auto-update.** The `chatter` CLI self-updates with `chatter update`
  (the bundled cargo-dist / axoupdater self-updater), and the desktop app
  checks for and installs new releases on launch (Tauri updater). Both pull
  from GitHub Releases. The CLI self-updater is experimental.
- **Prebuilt binaries** for macOS (Apple Silicon and Intel), Linux, and
  Windows, plus desktop installers, attached to the GitHub Release. The
  macOS desktop `.dmg` is signed and notarized.

### Known limitations

- **The merge and adjudication surface is experimental.** `merge`,
  `adjudicate`, `speaker-id`, and `sanity-scan` work, but their
  interfaces and heuristics may change before 1.0.
- **Windows binaries are not code-signed yet**, so Windows SmartScreen
  warns on first run (choose "More info" then "Run anyway"). macOS CLI
  binaries are codesigned but not notarized; install via the release
  installer script to avoid the Gatekeeper quarantine prompt.
- **Not on crates.io yet.** crates.io publication is deferred.

## Earlier changes (moved from the book)

The book states the current design only. These entries record fixes and
behaviour changes that its pages used to narrate. They are not attributed to a
release because the book did not record one; dates are those the book gave.

### Spec system

- The machine-written `_auto` error specs are gone. `corpus_to_specs` and
  `enhance_specs` were deleted (spec-system redesign, R5), and `spec/errors/`
  carries a `.human-authored` marker that every generator refuses to write
  into. In August 2026, 152 of 238 error spec files were `*_auto.md`; they
  recorded what chatter did rather than what it should do, because the tool
  wrote the spec's own filename code whenever its (never existing)
  `expectations.json` gave no codes. By 2026-09-03 one `_auto` file was left,
  and it has since been merged into `E519.md`.
- Duplicate spec files for one code were reconciled on 2026-09-03: the
  residue pairs (E202, E241, E604) were deleted, `E243_auto.md`'s example was
  re-filed under E202, and E316, E342, E375, E522, E360 and E502 were each
  merged into one `E###.md`. That changed the keys the re2c parity baseline
  uses, so `KNOWN_DIVERGENCES` was regenerated.
- `Category` was removed from specs on 2026-08-19 (a free-text grouping nothing
  read); `Level` moved from the file onto each example (Phase 2, 2026-08-21);
  all error specs moved to `+++` TOML frontmatter (Phase 1b, 2026-08-21).
- Every example now carries a required typed `claim` (R2, 2026-08-21), which
  deleted `SpecSelfDemonstrationGate` and its 36-entry baseline. The authored
  `layer` field was deleted (R4, 2026-08-21), together with the string-based
  error tests: the authored field disagreed with the observation snapshot on 17
  examples. `kind` and `status` moved from each spec file to the code registry
  (R1, 2026-08-26), which removed the `spec_status` gate, the
  `spec/errors <-> ErrorCode` divergence check and the per-code `kind`
  agreement loop. `status` no longer defaults to `implemented` when absent
  (this had been true of 104 of 238 specs on 2026-08-11). A retired code
  number reused is now a registry load error, replacing a comment in the enum.
- The `docs/errors/*.md` pages are a registry artifact written by
  `just spec-gen`; the standalone `gen_error_docs` binary was deleted. The
  hand-written artifact table in the spec chapter was replaced by one generated
  from the registry. `grammar/test/corpus/generated/` and `manual/` are
  separate trees; they had been one tree, which destroyed 1,468 lines of
  hand-mined corpus tests twice in three days.
- The backend-parity harness now carries each example's declared source path,
  so contextual rules such as E531 run for both parsers; dropping it had made
  both backends appear to miss E531.

### Reference corpus overhaul (Phases 0-6)

- The reference corpus grew from 345 English-only files to 374 files in 20
  languages, then was reorganised into nine topical subdirectories under
  `corpus/reference/`. At the end of the overhaul: concrete grammar node
  coverage went from 316/334 (94.6%) to 334/334 (100%); error specs from
  177/181 to 181/181 (169 with CHAT examples, 12 documented stubs); the
  golden artifacts were regenerated and `reference_corpus.rs` rebuilt with 374
  cases.
- Phase 0 built `corpus_node_coverage` (it confirmed 18 uncovered node types);
  Phase 1 built `extract_corpus_candidates` and selected 25 files across 20
  languages (eng, zho, fra, deu, spa, jpn, nld, heb, por, ell, tur, hrv, pol,
  ita, hun, rus, est, dan, ara, isl); Phase 2 added four handcrafted files in
  `constructs/` (`rare-terminators.cha`, `uptake.cha`, `best-guess.cha`,
  `unsupported.cha`) for the 18 gaps; Phase 3 ran batchalign3 morphotag over
  the language files; Phase 4 created error specs E707, E711 and E717, corrected
  E376's recorded code, filled 17 triggerable stub specs, documented 12
  untriggerable ones (E001, E002, E211, E317, E318, E340, E374, E377, E378,
  E380, E385, E386), corrected 5 misclassified specs (E319-E322, E376) and
  built `perturb_corpus` with 11 mutation strategies.
- Mining the MacWhinney subcorpus (407 files) found zero tree-sitter parse
  errors, and mining all of Eng-NA took over four minutes; that is why
  perturbation is the systematic route.
- The former Chumsky direct parser could not handle `unsupported_line` nodes
  (373 of 374 roundtrips passed under it); it has been removed and tree-sitter
  is the sole canonical parser.

### Chatter 1.0 readiness work log (2026-09-05 onward)

- Baseline on 2026-09-05: 223 error specs (179 implemented, 37 not implemented,
  five unreachable from CHAT, two deprecated); of 418 examples, 368 satisfied
  their claims, 50 were deferred and none failed. The CHECK mapping audit
  stopped inferring parity from code names and reading an obsolete source
  path; it reads the compiled registry. Real CHECK grounding passed (12.59 s
  and 12.45 s on two runs) over the committed fixture set.
- E246 was marked implemented (its reachable `(he):` example emits E246 and
  E209; `hel:o` and `(he)l:o` are legal controls). E212 gained a whole-file
  violation and a CA-mode legal control (an explicit category prefix prevents
  the CA normalizer from rewriting `0(the)`; the standalone shortening reports
  E209 and E212), and its original `hello world .` example now declares
  `legal`. E251's malformed word sample moved to E342 as an active
  missing-element example (tree-sitter reports E255/E342; re2c reports
  E209/E253/E255; neither emits E251), raising verified examples to 365 and
  lowering deferred to 51.
- `Word` prosodic checks use the private `ProsodicWord` measurement and two
  linear passes with constant-time neighbor queries (each marker used to search
  a prefix or suffix again); E244-E252 behaviour and diagnostic order are
  unchanged.
- `JsonSchemaPolicy` selects serialization after the shared named CHAT parse.
  A single-file conversion with schema validation skipped used to succeed on a
  mismatched media name; both policies now reject mismatches. Directory
  conversion exposes failing diagnostics and returns exit 1.
- `just regen` now refreshes the JSON schema (model documentation is embedded
  in it). The schema writer preserves unchanged files; generation is the
  explicit `just schema-gen`. The generator no longer rewrites `$ref` siblings
  into `allOf` (Draft 2020-12 allows siblings; the transform rewrote literal
  data as though it were a schema and its removal dropped 74 wrappers and five
  shape-only tests). `just traversal-gen` and the other generated-Rust recipes
  stage output through `scripts/generate_if_changed.py`; `tree-sitter generate`
  runs into a staging directory with `--output` and `--check` is read-only.
- The spec artifact writer used to delete every current named output before
  writing; `Ownership::NamedFiles` now deletes only retired names. `GeneratedDir`
  replaced `clear_owned` and proves ownership before pruning. A bounded nextest
  trial did not improve the warm generator suite.
- The foundation publication check derives the held-back set from Cargo
  metadata and requires `publish = false`; it rejected `talkbank-llm`, which
  was publishable by default and now holds publication back explicitly.
- Diagnostic indexes: editing a string in place reused stale line positions,
  and another source's line map panicked on a multibyte character. `SourceIndex`
  replaced the public `enhance_errors_with_line_map` (a breaking library API
  change) and the hidden thread-local cache.
- The re2c parser separated source lifetime from temporary token-storage
  lifetime, removing every production `Box::leak`; `SinToken::new_unchecked`
  was removed and `SinTier::from_tokens` returns `Result`.
- The hygiene scanner mistook a nested `fn report(...)` declaration for a call
  and missed a call with whitespace before `(`; both are fixed with a
  regression.

### Word grammar

- `standalone_word` had at one point been coarsened into one opaque DFA token,
  with a Chumsky direct parser re-parsing it into `WordContent`. That cost two
  parsers with independent bugs, validation that could not find markers without
  re-parsing, one opaque editor node, and a `cleaned_text()` that scanned for
  marker characters. When the Chumsky parser was eliminated the structured word
  grammar was restored (markers re-excluded from `word_segment` via the symbol
  registry, one CST child per marker, `WordContent` aligned 1:1 with grammar
  nodes, the purity invariant made a gate).
- In commit `fdceeac2` the consolidated `word_segment_purity.txt` (8 named
  tests) was replaced by per-construct test files generated from the specs.

### Parser

- The editor uses the `ParsedRevision` cache (`parse_chat_file_revision`); the
  LSP no longer owns its own edit calculator or a separately replaceable
  source/tree pair. The raw CST, strict-model and streaming incremental methods
  remain as compatibility entry points. Tree-sitter incremental parsing no
  longer clones the whole-document tree.
- E311 (parser-only unclosed-replacement diagnostic), the `UnclosedDelimiter`
  wrapper and its E312/E313 text classifiers, and the `BracketRecovery`
  classifier (which inferred annotations from text prefixes) were removed; the
  malformed inputs remain rejected by grammar recovery, with generic E316 where
  no structural evidence supports a narrower fault. The raw-argument error
  collector and its recursive wrapper were removed.
- A dummy replacement span used to conceal missing separators at either closing
  bracket; replacement producers now retain the whole CST wrapper's span.
- A missing `@UTF8` anchor is an optional grammar slot: the canonical parser
  used to discard the whole document and report the present headers as
  missing; it now keeps them and shared validation rejects the file with E503.
- Header lowering used a start-only check that accepted the first of two
  headers and discarded the second; `HeaderFragment` admission now requires the
  node to account for all caller text. Routing only `@PID` through the shared
  pre-`@Begin` decoder had let `@Window`, `@Color words` and `@Font` fall
  through to successful `Unknown` values.
- Fragment adapters no longer use the legacy error sink's length heuristic.

### Symbols and CA terminators

- A Chumsky-based direct parser provided combinator-based fragment parsing
  until it was removed in March 2026; tree-sitter is the sole parser.
- The derived symbol arrays were named `ca_delimiter_symbols` and
  `ca_element_symbols` until 2026-08-25; they are `paired_stretch_symbols` and
  `word_attached_symbols`. The two arrays used to be hand-written and need a
  disjointness check, which was deleted when they became derived from one
  `parse_role` field. Before 2026-08-12 no gate compared the symbol outputs
  against the registry; `generated_symbol_sets_are_current` was added then, and
  on 2026-08-20 its hand-written generator list became a glob of
  `spec/symbols/generate_*.js`. Its first run found two rustfmt-wrapped Rust
  outputs that the generator unwrapped.
- The parser/model used to promote trailing CA markers into utterance
  terminators through a post-hoc `resolve_ca_terminator()` pass; the pass was
  removed and CA arrows and `≈`/`≋` stay `Separator` content items.
- The validation page no longer describes one downstream consumer's server
  behaviour, PyO3 boundary types and report directory, and no longer calls the
  reference corpus "the sacred semantic target".

### re2c backend

- As of 0.19.0 re2c-parsed values borrow the caller's source (the earlier
  `Box::leak` strategy is gone). The re2c newline token used to fuse
  consecutive breaks and lose blank-line structure. The main-tier-only
  whitespace scan and its separate CA probe were removed in favour of the
  shared file validator. `WordLengthening::count` and the re2c AST moved from
  `u8` to `NonZeroUsize` (source runs over 255 colons could overflow or
  silently wrap, and zero counts were repaired with `max(1)` on
  serialization); JSON now accepts longer runs and rejects zero.
- Postcode, glued-replacement and separator silence recorded in older parity
  reports are fixed. The "both parsers correct" claim for CI validation is not
  currently true.
- Stored benchmark timings (tree-sitter vs re2c): small file (13 lines) 44 us
  vs 9.6 us (4.6x); medium file with dependent tiers 69 us vs 9.4 us (7.3x);
  large file 7,734 us vs 970 us (8.0x); batch of 35 files 21.7 ms vs 3.0 ms
  (7.2x). Older wild-corpus percentages and a 140-case diagnostic table were
  dropped as not describing the current spec suite.

### Correctness architecture work log (2026-09-08)

- The first session of work against the correctness plan: the workspace guard
  that permitted one named test at a time was removed (the whole suite ran in
  31 seconds for 3,048 tests); 368 dead snapshots then the last 56 were
  resolved and `snapshot-hygiene` joined `gate` (the 56 were 47 stale `.cha`
  stems from a reorganised corpus layout, 4 from a `snapshot_tests` module
  rewritten to plain assertions, 4 superseded copies and 1 naming a missing
  file); undemonstrated error codes were corrected from fifty-three to ten (219
  codes carried a spec, 166 demonstrated, 44 excused by registry status, five
  of the ten closed that night); 107 fabricated-AST constructions became
  `Word::simple` (66 others pass two different strings); the grammar corpus
  was found to be 211 generated cases (233 recorded, later re-derived as 139
  specs) expecting what the parser produced, with 137 of 138 construct specs
  carrying a `cst` block nothing asserts (41 naming nonexistent node types, 43
  containing `...`).
- Review of the first probe mechanism found `Outcome::Clean(String, Examined)`
  forgeable, a suite of one control probe satisfying every check, a tier axis
  with one reachable value (every `Precondition` declared `PrePush`), an absent
  directory minting the same witness as an empty one, and a second-tree hole
  closed by `gate_discipline`. Both Python ratchets (fabricated-AST and
  demonstration) became gates and six script files were deleted. A final review
  found the re2c fragment entry points passing the caller's raw sink into
  `%gra` lowering (a head overflow reported at byte 2 instead of the caller's
  offset) and `just gate` running at the inner-loop tier because the tier was
  exported from `test`; the tier became the `_test` recipe argument.
- The probe run was 5.3 s of a 13.7 s `just test`; threads made the standalone
  binary 4.5x faster (1.2 s) but `just test` slower (16.6 s) and were
  reverted; a shared read cache shipped (loop 12.0 s). The ratchet that
  counted comments scored prose as the hazard (505 and 601, of which 54 were
  prose) and was corrected on 2026-09-08. Coverage measured 2026-09-08 over the
  whole suite: validation 89.2% reported vs 47.7% parse-backed; model 83.8% vs
  43.0%; parser 69.3% vs 68.8%; transform 89.5% vs 89.5%. E370 located its
  marker through a second main-tier serializer; the parser started recording
  `Retrace::marker_span` that day and the renderer was deleted.
- The `uncovered_branches` JSON field was renamed `uncovered_region_starts`.
  The repository-root error-corpus generator resolved one parent too many and
  wrote outside the repository (66 files found beside it); it was fixed to
  refuse a root without the manifest directory.

### Contributor workflow documentation

- `just push` once ran four fast checks (and at another time no tests at all)
  under a comment claiming to be the full CI gate; a green `just test` was read
  as a green gate and CI went red on a doctest. The gate is now `just gate`.
  The book once told contributors to run `cargo check` before `cargo test`
  (which recompiles the dependency graph twice), listed eight `just` recipes
  when there were thirty-one, described a `make verify` target as "not yet
  ported" (there is no Makefile), gave per-file `--test <name>` targets that
  had not existed for some time, and listed fewer CI jobs than exist.
  `build.rs` once claimed a CI job verified the vendored re2c lexer; there has
  never been one. A specific, plausible-looking error message was once defended
  as a loss when the corruption producing it was fixed. Per-push CI no longer
  runs clippy or the feature-off build (`just release-lint`).

### Annotations

- Until 2026-08-26 `UtteranceContent` had no bare `Action`, so the parser
  wrapped every unannotated action in an `Annotated` with an empty list;
  across a 106,000-file corpus that was 20,184,072 values claiming to be
  annotated while carrying nothing (almost all a bare `0` marking silence in
  daylong recordings). `BracketedItem` had no bare `Group`, so an unannotated
  nested group became an `AnnotatedGroup` with an empty list. Two error codes
  meant to catch the empty case could not: one was disabled because bare `[*]`
  is valid CHAT, its number was reused for a rule that was unreachable. Both
  bare variants were added and the empty state became unconstructible.

### Form markers

- The form-marker meanings were corrected wholesale on 2026-08-11 against the
  CHAT manual's "Special Form Markers" table: six had been glossed with
  plausible expansions of the letters (`@k` as "kinship", `@p` "proper name",
  `@sl` "slang", `@sas` "second attempt success", `@g` "gemination", `@ls`
  "letter sequence"). `@a` was removed from chatter the same day: the corpus
  authority had eliminated it from every file on 2024-09-03 together with `@e`
  and `@lp`; the other two were dropped from chatter then and `@a` was
  overlooked.

### %mor

- Earlier documentation described a "comma-stripping" convention where
  `PronType=Int,Rel` became `-IntRel`; the grammar and parser preserve the
  comma. The UD MOR redesign (2026) removed `MorSuffix`, `MorCompound`,
  `MorPrefix`, `MorSubcategory`, `AnnotatedChunk` and `Chunk` from the data
  model, taking it from about 12 types to 4.

### Phon tiers

- CLAN's dependent-tier definitions added `%phoint` on September 25, 2026
  (`clan-info` commit `f062b58`), closing the earlier missing-declaration
  issue for the unprefixed tier. Current Phon exports no longer use the
  leading `x` on tier names. The Phon `%x` checks were once opt-in via
  `--check-xphon`; they are on by default and the flag is a deprecated no-op.

### JSON output

- A `word_index` field in the per-word language output existed until 2026-08-07
  and was removed as derivable and misleading. The independent raw/cleaned
  `Word` constructor arguments and `set_raw_text` were removed; `raw_text()`
  returns an owned string and no raw-text cache can go stale.

### Validation rules

- E756 (empty dependent tier) is formerly W601, renumbered because it always
  was a hard error. It read only user-defined `%x` tiers until 2026-08-15
  because the model could not represent an empty standard tier, so an empty
  `%eng:` had nowhere to be recorded and the two parser backends disagreed
  about it. `%com` and `%add` were exempt from its whitespace-only check by
  accident until 2026-09-08.
- E757 once caught only `hello [/]there`; it now applies to any item ending in
  a bracketed code (`hello [!]there`, `bobo [= toy]there`). The cause was a
  parser omission: an annotated word's wrapper span was left DUMMY at
  construction, so the glue was invisible to a span-adjacency check and any
  diagnostic reported on an annotated word pointed at byte zero.
- E767: an `@Media` line with a space before the comma used to fail to match,
  so the header fell back to `Unknown` and reported E525 alongside E330; the
  grammar now parses it so the rule can name the space (a change of
  diagnostic, not of verdict).
- E764: nothing reported `dog&-um` (two words) before the rule existed. E243
  now reports a bare or embedded pipe in a word (CLAN CHECK error 48). The
  `%gra` relation-head check was added because neither chatter nor CLAN CHECK
  validated relation labels, so a typo like `PUNCTT` rode silently into
  analyses.

### Test infrastructure measurements

- With unpacked debug artifacts disabled, a clean `spec/target` measured 586
  deps entries, no `.rcgu.o` files and 1.3 GB; the full spec suite took 20.14 s
  from an empty target and 1.64 s warm. A bounded nextest trial on the
  generators library (51 tests, one binary, four workers, warm) took about
  0.6 s against 0.4 s for Cargo, so Cargo remains the runner.
- A measured no-op `just regen` preserved bytes and nanosecond modification
  times of all 3,815 tracked files, took 8.177 s and compiled nothing; the next
  `just test` took 10.625 s with no compilation (2,985 passed, 61 ignored,
  across 34 test harnesses).

### Documentation and CHECK assessment

- The CHECK assessment manifest's notes had accumulated fix narratives
  ("GAP CLOSED 2026-07-09", "ROOT CAUSE was...", "E316 until 2026-09-08",
  "previously missed", "wrongly recorded as not-firing", and similar); they now
  state the current verdict and its grounds. Rulings and their dates are kept.
  Facts the old notes carried: the streaming lowering used to drop `@Begin:`
  and `@Begins` ERROR nodes (the whole-tree recovery backstop now reports
  E316); duplicate `@ID` lines were missed until E549; `@Time Duration` range
  was unchecked until E540; E552 was added as the inverse of E544; `@Media`
  filename E531 was dead through the CLI until the file stem was threaded
  through `validate_single_file_streaming`; `@zXXX` without a colon fell
  through to a bare user-defined label until the `@z:` colon was required;
  E242 named only the close-quote case until 2026-09-01; `%mor` malformed words
  reported E316 beside E702 until 2026-09-08; CHECK 152 was wrongly in the dead
  list until 2026-07-15.
- The spec-tooling page once described a bootstrap-era pipeline: it referred to
  `make test-gen` (there is no Makefile), listed as open a concern about
  `spec/tools` carrying parser/model dependencies (resolved by the
  `spec/runtime-tools` split), prescribed per-spec metadata that no loader read
  (ownership, `draft`/`accepted`/`deprecated`), and proposed an
  `input`/`ir`/`emit`/`validate`/`sync` module split and a `spec lint` binary
  that were never built.
- The branch-protection required-check list named only four jobs until
  2026-07-26, having been written before the wasm, app-version-sync and
  shellcheck jobs existed.
- `ParseError::build(...).finish()` is infallible: `try_finish()` and
  `ParseErrorBuilderError` were removed, and a hand-picked subset of the form
  markers that once sat in the word-syntax page glossed `@si` as "signed word"
  (it is singing; `@sl` is signed language).
- A consumer's ledger cited E754 (`LetterFormMultipleLetters`, retired
  2026-08-11) in August 2026 for a repair that is still correct.

### Compile-time investigation (2026-03, pre-fold)

- The compile-times investigation found that a global sccache `rustc-wrapper`
  was disabling incremental compilation (2.7% Rust cache hit rate; 36 of 37
  compilations non-cacheable), that full DWARF debug info inflated link times,
  and that third-party crates at `-O0` ran serde, regex and tree-sitter paths
  about 10x slower than necessary. Pre-fold measurements on the original
  ten-crate workspace: clean build about 3-5 min (estimated) to about 39 s;
  incremental rebuild after touching `talkbank-model` about 60-90 s to about
  4 s. The 2026-04-28 batchalign3 fold roughly tripled the third-party
  dependency surface, which made `[profile.dev.package."*"] opt-level = 1` (and
  the `profile.test` equivalent) prohibitive; both were removed.

### API changes recorded in the book

- `ChatDate::Valid` held `{ day, month, year, raw }` public fields; it is now
  `Valid(CheckedChatDate)` with private components and the accessors `day()`,
  `month()`, `year()` and `as_str()`.
- The `chatter merge`, `chatter pipeline` and `chatter batch` commands were
  removed from the CLI; the structural library in
  `talkbank_transform::transcript_merge` remains.

- The development-loop section of the CI and release page was written on
  2026-08-27 after a single parser fix cost a day to the process around it
  rather than to the fix. The parity baseline stopped listing E550 and E747
  once file and fragment participant recovery agreed and both lexers preserved
  single logical line breaks.

[Unreleased]: https://github.com/TalkBank/chatter/compare/v0.28.0...HEAD
[0.28.0]: https://github.com/TalkBank/chatter/compare/v0.27.0...v0.28.0
[0.27.0]: https://github.com/TalkBank/chatter/compare/v0.26.0...v0.27.0
[0.26.0]: https://github.com/TalkBank/chatter/compare/v0.25.0...v0.26.0
[0.25.0]: https://github.com/TalkBank/chatter/compare/v0.24.2...v0.25.0
[0.24.2]: https://github.com/TalkBank/chatter/compare/v0.24.1...v0.24.2
[0.24.1]: https://github.com/TalkBank/chatter/compare/v0.24.0...v0.24.1
[0.24.0]: https://github.com/TalkBank/chatter/compare/v0.23.0...v0.24.0
[0.23.0]: https://github.com/TalkBank/chatter/compare/v0.22.0...v0.23.0
[0.22.0]: https://github.com/TalkBank/chatter/compare/v0.21.0...v0.22.0
[0.21.0]: https://github.com/TalkBank/chatter/compare/v0.20.2...v0.21.0
[0.20.2]: https://github.com/TalkBank/chatter/compare/v0.20.1...v0.20.2
[0.20.1]: https://github.com/TalkBank/chatter/compare/v0.20.0...v0.20.1
[0.20.0]: https://github.com/TalkBank/chatter/compare/v0.19.0...v0.20.0
[0.19.0]: https://github.com/TalkBank/chatter/compare/v0.18.1...v0.19.0
[0.18.1]: https://github.com/TalkBank/chatter/compare/v0.18.0...v0.18.1
[0.18.0]: https://github.com/TalkBank/chatter/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/TalkBank/chatter/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/TalkBank/chatter/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/TalkBank/chatter/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/TalkBank/chatter/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/TalkBank/chatter/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/TalkBank/chatter/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/TalkBank/chatter/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/TalkBank/chatter/compare/v0.9.1...v0.10.0
[0.9.1]: https://github.com/TalkBank/chatter/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/TalkBank/chatter/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/TalkBank/chatter/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/TalkBank/chatter/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/TalkBank/chatter/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/TalkBank/chatter/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/TalkBank/chatter/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/TalkBank/chatter/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/TalkBank/chatter/compare/v0.3.6...v0.4.0
[0.3.6]: https://github.com/TalkBank/chatter/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/TalkBank/chatter/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/TalkBank/chatter/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/TalkBank/chatter/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/TalkBank/chatter/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/TalkBank/chatter/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/TalkBank/chatter/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/TalkBank/chatter/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/TalkBank/chatter/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/TalkBank/chatter/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/TalkBank/chatter/releases/tag/v0.1.0
