//! Cache maintenance operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use sqlx::SqlitePool;
use std::path::PathBuf;
use tracing::info;

use super::error::CacheError;

/// Clear cache entries for files under the given path prefix.
///
/// ONE bulk statement, never a scan-and-loop: the original implementation
/// fetched every `file_path` in the cache and issued one `DELETE` per
/// matching file, which turned a corpus-sized `--force` into minutes of
/// silent work (v0.5.0 DOA, 2026-07-30).
///
/// Path-component prefix semantics (`Path::starts_with`): the exact path
/// itself, plus everything under `prefix<SEP>`. The children are selected
/// with a half-open byte range `[prefix + SEP, prefix + succ(SEP))`, which
/// under SQLite's default BINARY collation captures exactly the strings
/// beginning with `prefix<SEP>`: no `LIKE`, so no wildcard-escaping
/// pitfalls, and the planner can drive it from an index.
pub async fn clear_prefix(pool: &SqlitePool, prefix: &str) -> Result<usize, CacheError> {
    let sep = std::path::MAIN_SEPARATOR;
    let prefix_norm = prefix.trim_end_matches(sep);
    // The byte after the separator in ASCII ('/' -> '0', '\\' -> ']'); both
    // separators are ASCII so the +1 stays within ASCII.
    let sep_successor = char::from(sep as u8 + 1);
    let child_lo = format!("{prefix_norm}{sep}");
    let child_hi = format!("{prefix_norm}{sep_successor}");
    let result = sqlx::query(
        "DELETE FROM file_cache WHERE file_path = ?1 OR (file_path >= ?2 AND file_path < ?3)",
    )
    .bind(prefix_norm)
    .bind(&child_lo)
    .bind(&child_hi)
    .execute(pool)
    .await
    .map_err(|source| CacheError::Database { source })?;

    Ok(result.rows_affected() as usize)
}

/// Batch size for [`clear_paths`]: comfortably under SQLite's host-parameter
/// ceiling while keeping statement count linear-over-chunks.
const CLEAR_PATHS_CHUNK: usize = 500;

/// Clear the cache entries for an explicit set of file paths, batched.
///
/// This is the `--force` seam: the CLI resolves its inputs to a file list
/// and must clear exactly those entries (never a cosmetic label, never one
/// query per file). Each chunk is one `DELETE ... WHERE file_path IN (...)`
/// statement, so a corpus-sized refresh costs `paths / CLEAR_PATHS_CHUNK`
/// statements instead of `paths` full-table scans.
pub async fn clear_paths(pool: &SqlitePool, paths: &[String]) -> Result<usize, CacheError> {
    let mut removed_entries = 0usize;
    for chunk in paths.chunks(CLEAR_PATHS_CHUNK) {
        // QueryBuilder keeps the statement fully parameterized (sqlx's
        // SqlSafeStr guard rejects hand-assembled dynamic SQL).
        let mut builder = sqlx::QueryBuilder::new("DELETE FROM file_cache WHERE file_path IN (");
        let mut separated = builder.separated(", ");
        for path in chunk {
            separated.push_bind(path);
        }
        builder.push(")");
        let result = builder
            .build()
            .execute(pool)
            .await
            .map_err(|source| CacheError::Database { source })?;
        removed_entries += result.rows_affected() as usize;
    }
    Ok(removed_entries)
}

/// Clear all cache entries.
pub async fn clear_all(pool: &SqlitePool) -> Result<(), CacheError> {
    sqlx::query("DELETE FROM file_cache")
        .execute(pool)
        .await
        .map_err(|source| CacheError::Database { source })?;

    Ok(())
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
        if !PathBuf::from(&path).exists() {
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
