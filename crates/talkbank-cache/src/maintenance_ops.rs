//! Cache maintenance operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use sqlx::SqlitePool;
use std::path::PathBuf;
use tracing::info;

use super::cache_utils::{self, CacheKey};
use super::error::CacheError;
use talkbank_model::ResolvedPrefix;

/// Which entries a maintenance count or clear covers.
///
/// `Under` holds a [`ResolvedPrefix`], so a scope can only name a path in
/// the form the cache stores (its directories resolved), never a spelling no
/// row was written under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheScope {
    /// Every entry.
    All,
    /// The entries for one path and everything under it, by whole path
    /// components, never by string prefix. Build it with
    /// [`ResolvedPrefix::of`], which resolves a directory whole.
    Under(ResolvedPrefix),
}

/// The keys under a path prefix, as the exact path and a half-open byte range
/// of its children.
///
/// The children are selected with `[prefix + SEP, prefix + succ(SEP))`, which
/// under SQLite's default BINARY collation captures exactly the strings
/// beginning with `prefix<SEP>`: no `LIKE`, so no wildcard-escaping pitfalls,
/// and the planner can drive it from an index.
struct PrefixRange {
    exact: String,
    children_from: String,
    children_before: String,
}

impl PrefixRange {
    fn new(prefix: &str) -> Self {
        let sep = std::path::MAIN_SEPARATOR;
        let exact = prefix.trim_end_matches(sep).to_owned();
        // The byte after the separator in ASCII ('/' -> '0', '\\' -> ']'); both
        // separators are ASCII so the +1 stays within ASCII.
        let sep_successor = char::from(sep as u8 + 1);
        Self {
            children_from: format!("{exact}{sep}"),
            children_before: format!("{exact}{sep_successor}"),
            exact,
        }
    }
}

/// Append the scope's `WHERE` clause, the one place the predicate is
/// written, to a statement over `file_cache`.
fn push_scope(builder: &mut sqlx::QueryBuilder<sqlx::Sqlite>, scope: &CacheScope) {
    match scope {
        CacheScope::All => {}
        CacheScope::Under(prefix) => {
            let range = PrefixRange::new(&prefix.as_path().to_string_lossy());
            builder
                .push(" WHERE file_path = ")
                .push_bind(range.exact)
                .push(" OR (file_path >= ")
                .push_bind(range.children_from)
                .push(" AND file_path < ")
                .push_bind(range.children_before)
                .push(")");
        }
    }
}

/// How many entries the scope covers.
pub async fn count(pool: &SqlitePool, scope: &CacheScope) -> Result<usize, CacheError> {
    let mut builder = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM file_cache");
    push_scope(&mut builder, scope);
    let (entries,): (i64,) = builder
        .build_query_as()
        .fetch_one(pool)
        .await
        .map_err(|source| CacheError::Database { source })?;
    cache_utils::count(entries)
}

/// Delete the entries the scope covers and return how many went, from the
/// same statement that deleted them: ONE bulk statement, never a
/// scan-and-loop over every row.
pub async fn clear(pool: &SqlitePool, scope: &CacheScope) -> Result<usize, CacheError> {
    let mut builder = sqlx::QueryBuilder::new("DELETE FROM file_cache");
    push_scope(&mut builder, scope);
    let result = builder
        .build()
        .execute(pool)
        .await
        .map_err(|source| CacheError::Database { source })?;
    cache_utils::count(result.rows_affected())
}

/// Batch size for [`clear_keys`]: comfortably under SQLite's host-parameter
/// ceiling while keeping statement count linear-over-chunks.
const CLEAR_KEYS_CHUNK: usize = 500;

/// Delete the rows with these keys, batched.
///
/// This is the `--force` seam: the CLI's inputs are resolved paths, and it
/// must clear exactly their rows (never a cosmetic label, never one query
/// per file). Each chunk is one `DELETE ... WHERE path_hash IN (...)`
/// statement over the primary key, so a corpus-sized refresh costs
/// `keys / CLEAR_KEYS_CHUNK` statements instead of one table scan per file.
/// Each key is bound as the borrowed text it already is.
pub async fn clear_keys(pool: &SqlitePool, keys: &[CacheKey]) -> Result<usize, CacheError> {
    let mut removed_entries = 0usize;
    for chunk in keys.chunks(CLEAR_KEYS_CHUNK) {
        // QueryBuilder keeps the statement fully parameterized (sqlx's
        // SqlSafeStr guard rejects hand-assembled dynamic SQL).
        let mut builder = sqlx::QueryBuilder::new("DELETE FROM file_cache WHERE path_hash IN (");
        let mut separated = builder.separated(", ");
        for key in chunk {
            separated.push_bind(key.as_str());
        }
        builder.push(")");
        let result = builder
            .build()
            .execute(pool)
            .await
            .map_err(|source| CacheError::Database { source })?;
        removed_entries += cache_utils::count(result.rows_affected())?;
    }
    Ok(removed_entries)
}

/// Purge cache entries for files that no longer exist on disk.
///
/// Returns the number of removed file entries.
pub async fn purge_nonexistent(pool: &SqlitePool) -> Result<usize, CacheError> {
    let paths: Vec<(String,)> = sqlx::query_as("SELECT file_path FROM file_cache")
        .fetch_all(pool)
        .await
        .map_err(|source| CacheError::Database { source })?;

    let mut removed_files = 0;
    for (path,) in paths {
        // `exists` answers "no" for a path it cannot check, which would
        // delete the entry of a file that is merely unreadable now.
        let gone = !PathBuf::from(&path)
            .try_exists()
            .map_err(|source| CacheError::Io {
                path: path.clone(),
                source,
            })?;
        if gone {
            sqlx::query("DELETE FROM file_cache WHERE file_path = ?1")
                .bind(&path)
                .execute(pool)
                .await
                .map_err(|source| CacheError::Database { source })?;
            removed_files += 1;
        }
    }

    if removed_files > 0 {
        info!(
            removed_files = removed_files,
            "Purged non-existent entries from cache"
        );
    }

    Ok(removed_files)
}
