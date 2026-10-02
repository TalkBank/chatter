# talkbank-cache

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-cache/README.md)

SQLite-backed pass/fail cache for CHAT validation and round-trip results.

## Overview

This crate provides the persistent cache extracted from the
`talkbank-transform` pipeline. It stores validation and round-trip outcomes in
an on-disk SQLite database under the OS-appropriate cache directory so repeated
corpus validation runs can skip unchanged files.

Key capabilities:

- **Stable cache location**: Resolves a per-user TalkBank cache directory on
  macOS, Linux, and Windows.
- **Validation result reuse**: Stores pass/fail outcomes keyed by the
  `ContentHash` of the bytes the caller read and validated, the alignment
  coverage (`AlignmentValidation`), and a `RulesVersion` (cache crate version
  folded with a fingerprint of the active validation rule set). When the
  rules change, prior verdicts become a cache miss instead of being served
  stale.
- **Failures are not misses**: every lookup returns
  `Result<CacheLookup<V>, CacheError>`, `CacheLookup` being `Hit(verdict)` or
  `Miss` and `V` the verdict's own type (`CacheOutcome` for validation,
  `RoundtripOutcome` for roundtrip), so a locked or corrupt database is an
  error the caller reports, never a cold cache. Every method of the two
  cache traits is required; there are no silent defaults.
- **Reading is its own capability**: `VerdictReader` has the lookups (`get`,
  `get_roundtrip`) and `ValidationCache: VerdictReader` adds the writes
  (`set`, `set_roundtrip`). `ReadOnlyCache` opens an existing cache to read
  only: it implements `VerdictReader` alone, creates and migrates nothing
  (a missing cache, or one whose schema is older or newer than this
  build's, is refused),
  connects read-only, runs no expiry or generation prune, and has no delete
  method, so a reporting sweep holds nothing that writes.
- **One path, however spelled**: every method takes a `ResolvedPath`
  (from `talkbank-model`), the file's parent directory resolved by the
  operating system and its own name kept as stored, so `a.cha`, `./a.cha`,
  `sub/../a.cha` and an absolute spelling through a linked directory are one
  row. Its constructors read the filesystem; transcripts carry theirs from
  the moment they are found
  (`talkbank_transform::paths::StoredTranscript::resolved`).
- **A specified key**: rows are keyed by a `CacheKey`, blake3 over a
  written-down encoding of the `ResolvedPath`'s components and the row's
  namespace, never `DefaultHasher` (whose algorithm may change with any
  toolchain). The key scheme (`KEY_SCHEME`) is part of every row's version,
  so a change to it retires old rows through the version prune.
- **Round-trip reuse**: Caches round-trip checks separately from plain
  validation so callers can opt into the more expensive gate.
- **Concurrency-safe initialization**: An exclusive advisory file lock
  beside the database serializes first-time create + migrate across
  threads and processes (sqlx's SQLite migrator has no cross-connection
  lock of its own), with a bounded wait that fails typed rather than
  hanging. Steady-state reads and writes are serialized by SQLite WAL
  mode plus a busy timeout on every connection.

A validation `CachePool` requires a `CacheIdentity` containing a rule generation
and parser. The parser is a row namespace, so switching backends does not spend
another retained build generation. Validation and roundtrip methods use the
bound identity; no method takes an independent parser string.
Production callers obtain the identity from `ValidationConfig::cache_identity`
in `talkbank-transform`. The example below uses this build's rule generation
(`RulesVersion::current()`, which names no parser fingerprint).

Administration starts from one read-only look at the directory,
`CacheOnDisk::inspect`, which creates, migrates and writes nothing and says
what is there: `Absent` (no database), `OlderSchema` or `Current` (an
`InspectionCache`, with `count` and `stats` only); a database a newer build
wrote is `CacheError::SchemaNewer`. `MaintenanceCache`, which exposes
statistics and explicit maintenance without verdict methods, expiration
cleanup or generation pruning, is reached only from that value:
`OlderSchema::migrate` or `InspectionCache::into_maintenance`. There is no
route to it from a directory with no database, so maintenance never creates
a cache to act on. Maintenance takes a
`CacheScope` (`All`, or `CacheScope::Under(prefix)`, a `ResolvedPrefix` built
by `ResolvedPrefix::of`, for a path and everything under it by whole
components): `count(&scope)` reads the database alone, and
`clear(&scope)` returns the number of rows the delete itself removed.
`CachePool::clear_paths` (the `--force` seam) clears by key, in every
namespace a row can be written under. Every writable open shares the same
locked initialization procedure.

## Usage

```rust,no_run
use std::path::Path;
use talkbank_cache::{
    CacheIdentity, CacheLookup, CachePool, ContentHash, ParserKind, ResolvedPath, RulesVersion,
    VerdictReader,
};
use talkbank_model::validation::AlignmentValidation;

let identity = CacheIdentity::new(RulesVersion::current(), ParserKind::TreeSitter);
let cache = CachePool::in_memory(identity).expect("cache opens");
// Key by the bytes you read and validate, not by a second read of the path.
let content = ContentHash::of(b"@UTF8\n@Begin\n@End\n");
let path = ResolvedPath::of_file(Path::new("example.cha")).expect("resolvable path");
let lookup = cache.get(&path, &content, AlignmentValidation::Structure);
assert_eq!(lookup.expect("lookup runs"), CacheLookup::Miss);
```

## License

MIT OR Apache-2.0.
