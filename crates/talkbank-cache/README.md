# talkbank-cache

**Status:** Current
**Last modified:** 2026-06-22 06:48 EDT

SQLite-backed pass/fail cache for CHAT validation and round-trip results.

## Overview

This crate provides the persistent cache extracted from the
`talkbank-transform` pipeline. It stores validation and round-trip outcomes in
an on-disk SQLite database under the OS-appropriate cache directory so repeated
corpus validation runs can skip unchanged files.

Key capabilities:

- **Stable cache location**: Resolves a per-user TalkBank cache directory on
  macOS, Linux, and Windows.
- **Validation result reuse**: Stores pass/fail outcomes keyed by content hash
  and a `RulesVersion` (cache crate version folded with a fingerprint of the
  active validation rule set). When the rules change, prior verdicts become a
  cache miss instead of being served stale.
- **Round-trip reuse**: Caches round-trip checks separately from plain
  validation so callers can opt into the more expensive gate.

## Usage

```rust,no_run
use std::path::Path;
use talkbank_cache::{CachePool, ValidationCache};

let cache = CachePool::in_memory().expect("cache opens");
assert_eq!(cache.get(Path::new("example.cha"), false), None);
```

## License

MIT OR Apache-2.0.
