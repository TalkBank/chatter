//! Cache statistics display (text or JSON).
//!
//! Reports what is in the cache directory: no database, a database of an
//! older schema, or a current one with its entry count, size and
//! last-modified time. The command only reads: it looks with
//! [`CacheOnDisk::inspect`], which creates, migrates and writes nothing, and
//! the facts come from `talkbank_cache`; this module only renders them.
//! `--format json` emits a [`CacheStatistics`] for machine consumption, useful
//! in CI dashboards or monitoring scripts that track cache freshness.

use std::fmt;
use std::path::Path;

use jiff::Timestamp;
use serde::Serialize;
use talkbank_transform::{CacheOnDisk, DatabaseFile, StorageStats};

/// Serializable cache statistics in the format emitted by `chatter cache stats --format json`.
///
/// This is the wire model only: one variant per state of the cache directory,
/// tagged by `database`, built only by [`CacheStatistics::new`] from what the
/// cache crate found and read, so its nullable fields cannot disagree (a
/// `null` size beside a time, or an entry count for a database nobody could
/// read). The contract is documented in the book's diagnostic contract page.
#[derive(Debug, Serialize)]
#[serde(tag = "database", rename_all = "snake_case")]
enum CacheStatistics<'a> {
    /// The directory holds no database; nothing was created to say so.
    Absent {
        /// The directory looked in.
        cache_dir: &'a Path,
    },
    /// A database of an older schema, which only a writing command migrates;
    /// its entries are not counted, since migrating can change them.
    OlderSchema {
        /// The cache directory.
        cache_dir: &'a Path,
    },
    /// A database of this build's schema.
    Current {
        /// Total number of entries in the cache.
        total_entries: usize,
        /// Cache directory path; `null` for an in-memory cache. Written as a
        /// string; a path that is not UTF-8 fails serialization rather than
        /// being written with replacement characters.
        cache_dir: Option<&'a Path>,
        /// Cache database size in bytes; `null` when there is no database
        /// file. An empty file is `0`, a different fact from no file.
        cache_size_bytes: Option<u64>,
        /// When the cache database file last changed; `null` exactly when
        /// `cache_size_bytes` is, because there is no file.
        last_modified: Option<MachineTime>,
    },
}

/// What one inspection of the cache directory says, read once: the states
/// with nothing to count, or a current database's entry count and storage
/// facts.
enum Report<'a> {
    /// No database in the directory.
    Absent(&'a talkbank_transform::NoDatabase),
    /// A database of an older schema.
    OlderSchema(&'a talkbank_transform::OlderSchema),
    /// A current database's count and storage facts.
    Current(talkbank_transform::CacheStats),
}

impl<'a> CacheStatistics<'a> {
    /// The wire form of a report: one exhaustive match over the directory's
    /// states and, for a current database, the three storage states the
    /// cache reports. An in-memory cache is not rendered as a missing file:
    /// it has no file.
    fn new(report: &'a Report<'a>) -> Self {
        let stats = match report {
            Report::Absent(absent) => {
                return Self::Absent {
                    cache_dir: absent.cache_dir(),
                };
            }
            Report::OlderSchema(older) => {
                return Self::OlderSchema {
                    cache_dir: older.cache_dir(),
                };
            }
            Report::Current(stats) => stats,
        };
        let (cache_dir, cache_size_bytes, last_modified) = match &stats.storage {
            StorageStats::InMemory => (None, None, None),
            StorageStats::Directory {
                cache_dir,
                database: DatabaseFile::Missing,
                ..
            } => (Some(cache_dir.as_path()), None, None),
            StorageStats::Directory {
                cache_dir,
                database:
                    DatabaseFile::Present {
                        size_bytes,
                        modified,
                        ..
                    },
                ..
            } => (
                Some(cache_dir.as_path()),
                Some(*size_bytes),
                Some(MachineTime::from_timestamp(*modified)),
            ),
        };
        Self::Current {
            total_entries: stats.total_entries,
            cache_dir,
            cache_size_bytes,
            last_modified,
        }
    }
}

/// An instant written for programs: RFC 3339 in UTC with exactly three
/// fractional digits, `2026-09-28T16:00:00.000Z`. The fixed width keeps
/// string order equal to time order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineTime(Timestamp);

impl MachineTime {
    /// `timestamp` truncated to the millisecond, the precision written, so a
    /// value equals what it prints.
    fn from_timestamp(timestamp: Timestamp) -> Self {
        // `constant` panics only out of range; the whole milliseconds of a
        // valid instant are in range.
        let millis = timestamp.subsec_nanosecond() / 1_000_000 * 1_000_000;
        Self(Timestamp::constant(timestamp.as_second(), millis))
    }
}

impl fmt::Display for MachineTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.strftime("%Y-%m-%dT%H:%M:%S%.3fZ"))
    }
}

impl Serialize for MachineTime {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Display cache statistics so operators can report cache health before running heavy validations.
///
/// Reads only: a directory with no database is reported as such ("No cache
/// database at PATH") and exits 0, as `cache clear --dry-run` treats it, since
/// having no cache yet is a legal state; nothing is created to describe it. A
/// database of an older schema is reported, not migrated, and also exits 0.
/// Exits 1 when the cache cannot be read (including a database a newer build
/// wrote) or the report cannot be written.
pub fn cache_stats(format: crate::cli::OutputFormat) {
    let fail = |action: &str, error: &dyn fmt::Display| -> ! {
        eprintln!("Error: Failed to {action}: {error}");
        std::process::exit(1);
    };
    let found = CacheOnDisk::inspect().unwrap_or_else(|e| fail("open cache", &e));
    let report = match &found {
        CacheOnDisk::Absent(absent) => Report::Absent(absent),
        CacheOnDisk::OlderSchema(older) => Report::OlderSchema(older),
        CacheOnDisk::Current(cache) => Report::Current(
            cache
                .stats()
                .unwrap_or_else(|e| fail("read cache stats", &e)),
        ),
    };

    match format {
        crate::cli::OutputFormat::Json => {
            match serde_json::to_string_pretty(&CacheStatistics::new(&report)) {
                Ok(json_str) => outln!("{}", json_str),
                Err(e) => fail("serialize JSON", &e),
            }
        }
        crate::cli::OutputFormat::Text => print_text(&report),
    }
}

/// The human-readable report: a sentence for a directory with nothing to
/// count, else the table, whose three storage facts one exhaustive match
/// over the cache's three storage states decides.
fn print_text(report: &Report<'_>) {
    const NO_FILE: &str = "(no cache file)";
    let stats = match report {
        Report::Absent(absent) => {
            outln!("No cache database at {}", absent.database().display());
            return;
        }
        Report::OlderSchema(older) => {
            outln!(
                "The cache database at {} has an older schema; the next `chatter validate` or `chatter cache clear` migrates it.",
                older.database().display()
            );
            return;
        }
        Report::Current(stats) => stats,
    };
    let (directory, size, modified) = match &stats.storage {
        StorageStats::InMemory => (
            "(in memory)".to_owned(),
            NO_FILE.to_owned(),
            NO_FILE.to_owned(),
        ),
        StorageStats::Directory {
            cache_dir,
            database: DatabaseFile::Missing,
            ..
        } => (
            cache_dir.display().to_string(),
            NO_FILE.to_owned(),
            NO_FILE.to_owned(),
        ),
        StorageStats::Directory {
            cache_dir,
            database:
                DatabaseFile::Present {
                    size_bytes,
                    modified,
                    ..
                },
            ..
        } => (
            cache_dir.display().to_string(),
            format!("{:.1} MB", *size_bytes as f64 / 1024.0 / 1024.0),
            // For a person: local time with its zone abbreviation.
            modified
                .to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M:%S %Z")
                .to_string(),
        ),
    };
    outln!("Cache Statistics");
    outln!("================");
    outln!();
    outln!("Cache Directory: {directory}");
    outln!("Cache Size:      {size}");
    outln!("Total Entries:   {}", stats.total_entries);
    outln!("Last Modified:   {modified}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_transform::{CacheIdentity, CachePool, ParserKind, RulesVersion};

    /// The JSON written for real cache reports, which only the cache crate
    /// can make: an in-memory cache has `null` directory, size and time; a
    /// directory cache has all three; a directory with no database says so
    /// and counts nothing. Removing a database consumes its open handle;
    /// fresh inspection admits the absent state, not a query through the
    /// old handle. Missing-file metadata is tested in the cache crate.
    #[test]
    fn json_writes_absent_facts_as_null() {
        let json = |report: &Report<'_>| {
            serde_json::to_value(CacheStatistics::new(report)).expect("serialize")
        };

        let identity = CacheIdentity::new(RulesVersion::current(), ParserKind::TreeSitter);
        let in_memory = CachePool::in_memory(identity.clone()).expect("in-memory cache");
        assert_eq!(
            json(&Report::Current(in_memory.stats().expect("stats"))),
            serde_json::json!({
                "database": "current",
                "total_entries": 0,
                "cache_dir": null,
                "cache_size_bytes": null,
                "last_modified": null,
            })
        );

        let dir = tempfile::tempdir().expect("temp dir");
        let absent = CacheOnDisk::inspect_directory(dir.path().to_path_buf()).expect("inspect");
        let CacheOnDisk::Absent(absent) = absent else {
            panic!("an empty directory holds no database");
        };
        assert_eq!(
            json(&Report::Absent(&absent)),
            serde_json::json!({ "database": "absent", "cache_dir": dir.path() })
        );

        // A database no build migrated is of an older schema, and is
        // reported without being read or migrated.
        let unmigrated = tempfile::tempdir().expect("temp dir");
        std::fs::write(talkbank_transform::cache_db_path(unmigrated.path()), b"").expect("write");
        let CacheOnDisk::OlderSchema(older) =
            CacheOnDisk::inspect_directory(unmigrated.path().to_path_buf()).expect("inspect")
        else {
            panic!("an unmigrated database has an older schema");
        };
        assert_eq!(
            json(&Report::OlderSchema(&older)),
            serde_json::json!({ "database": "older_schema", "cache_dir": unmigrated.path() })
        );

        drop(CachePool::with_directory(dir.path().to_path_buf(), identity).expect("create"));
        let CacheOnDisk::Current(cache) =
            CacheOnDisk::inspect_directory(dir.path().to_path_buf()).expect("inspect")
        else {
            panic!("a database this build made is current");
        };
        let present = json(&Report::Current(cache.stats().expect("stats")));
        assert_eq!(present["database"], "current");
        assert_eq!(present["cache_dir"], serde_json::json!(dir.path()));
        assert!(
            present["cache_size_bytes"]
                .as_u64()
                .is_some_and(|size| size > 0)
        );
        assert!(present["last_modified"].is_string());

        // Consume the query capability before deleting its backing file.
        // Ownership prevents using this handle after the state transition;
        // only fresh inspection can admit the directory's new state.
        drop(cache);
        std::fs::remove_file(talkbank_transform::cache_db_path(dir.path())).expect("remove");
        let CacheOnDisk::Absent(absent) =
            CacheOnDisk::inspect_directory(dir.path().to_path_buf()).expect("inspect")
        else {
            panic!("the removed database must be admitted as absent");
        };
        assert_eq!(
            json(&Report::Absent(&absent)),
            serde_json::json!({ "database": "absent", "cache_dir": dir.path() })
        );
    }
}
