//! Selective or full cache clearing for the validation cache.
//!
//! Supports two scopes: `--all` removes every entry in the database, and
//! `--prefix <PATH>` removes only the entries for that path and everything
//! under it (whole path components, not a string prefix).
//! Prefix mode is the typical choice after a corpus changes; it avoids invalidating
//! results for unrelated corpora. Both modes support `--dry-run` to preview what a
//! clear would do, from the same look at the cache directory the clear starts from.

use talkbank_transform::{CacheError, CacheOnDisk, CacheScope};

use crate::cli::ClearMode;

/// Clear validation cache entries for a more reproducible `chatter validate` run.
///
/// Both modes start from one look at the cache directory
/// ([`CacheOnDisk::inspect`], which writes nothing), so a preview and the
/// clear it previews agree about what is there:
///
/// - no database: nothing to clear, and none is created (both modes say so
///   and exit 0);
/// - a database of an older schema: a clear migrates it, then clears; a dry
///   run says it would, and cannot count the entries, since migrating can
///   remove some, so it says that too;
/// - a current database: a dry run counts read-only, and a clear reopens it
///   writable and reports the number its delete removed.
///
/// Exits 1 if the cache cannot be opened, migrated, counted or cleared,
/// including a database a newer build wrote.
pub fn cache_clear(scope: CacheScope, mode: ClearMode) {
    let fail = |action: &str, error: CacheError| -> ! {
        eprintln!("Error: Failed to {action}: {error}");
        std::process::exit(1);
    };

    // What the scope selects, said the same way by a dry run and a clear.
    let selection = match &scope {
        CacheScope::All => "cache entries".to_owned(),
        CacheScope::Under(prefix) => format!(
            "cache entries matching prefix '{}'",
            prefix.as_path().display()
        ),
    };
    let found = CacheOnDisk::inspect().unwrap_or_else(|e| fail("open cache", e));
    match (found, mode) {
        (CacheOnDisk::Absent(absent), ClearMode::DryRun) => outln!(
            "Would clear 0 {selection} (dry-run): no cache database at {}",
            absent.database().display()
        ),
        (CacheOnDisk::Absent(absent), ClearMode::Apply) => outln!(
            "Cleared 0 {selection}: no cache database at {}",
            absent.database().display()
        ),
        (CacheOnDisk::OlderSchema(older), ClearMode::DryRun) => outln!(
            "Would migrate the cache database at {} to this build's schema, then clear the {selection} it holds (dry-run; how many is known only after the migration, which can remove entries)",
            older.database().display()
        ),
        (CacheOnDisk::OlderSchema(older), ClearMode::Apply) => {
            let database = older.database();
            let cache = older.migrate().unwrap_or_else(|e| fail("migrate cache", e));
            outln!(
                "Migrated the cache database at {} to this build's schema",
                database.display()
            );
            let count = cache
                .clear(&scope)
                .unwrap_or_else(|e| fail("clear cache", e));
            outln!("Cleared {count} {selection}");
        }
        (CacheOnDisk::Current(cache), ClearMode::DryRun) => {
            let count = cache
                .count(&scope)
                .unwrap_or_else(|e| fail("count cache entries", e));
            outln!("Would clear {count} {selection} (dry-run)");
        }
        (CacheOnDisk::Current(cache), ClearMode::Apply) => {
            let cache = cache
                .into_maintenance()
                .unwrap_or_else(|e| fail("open cache", e));
            let count = cache
                .clear(&scope)
                .unwrap_or_else(|e| fail("clear cache", e));
            outln!("Cleared {count} {selection}");
        }
    }
}
