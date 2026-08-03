//! Deleting cache rows that no reader can ever bind.
//!
//! # The defect this closes
//!
//! Every read binds the opening pool's [`RulesVersion`] into the SQL `WHERE`
//! clause, so a row written under a superseded version is unreachable BY
//! CONSTRUCTION: no query any binary can issue will match it again. Nothing
//! deleted those rows. The only cleanup was a 30-day age cutoff, which answers
//! a different question ("is this stale?") and happily keeps rows that are
//! recent AND unreachable. Every release therefore stranded a complete copy of
//! the corpus. A real user cache measured 464,773 rows across 88 distinct
//! versions for a corpus of ~106,000 files: roughly 190 MB of a 243 MB file
//! that could never be read.
//!
//! # The criterion is reachability, not age
//!
//! A row is live if some reader will bind its version, and dead otherwise,
//! however recently it was written. That is a different question from the TTL,
//! so it gets its own pass rather than a tweak to the cutoff, and the TTL stays
//! for what it does.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use sqlx::SqlitePool;

use super::error::CacheError;
use super::rules_version::RulesVersion;

/// What the reachability prune did on one cache open.
///
/// A variant rather than a report with a zero count, because "nothing needed
/// deleting" and "rows were deleted" are different things for an operator to
/// read and the zero case has no space figures to report at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionPruneOutcome {
    /// Every row in the database was reachable. Nothing was deleted, and no
    /// `VACUUM` was paid for.
    NothingUnreachable,
    /// Rows under superseded versions were deleted.
    Pruned(VersionPruneReport),
}

/// How much unreachable state one prune removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionPruneReport {
    /// Rows deleted.
    rows_deleted: u64,
    /// Distinct superseded versions those rows belonged to.
    versions_deleted: u64,
    /// Whether the freed pages were returned to the filesystem.
    reclaimed: SpaceReclaimed,
}

impl VersionPruneReport {
    /// Rows deleted by this prune.
    pub fn rows_deleted(&self) -> u64 {
        self.rows_deleted
    }

    /// Distinct superseded versions removed by this prune.
    pub fn versions_deleted(&self) -> u64 {
        self.versions_deleted
    }

    /// Whether, and by how much, the database file shrank.
    pub fn reclaimed(&self) -> &SpaceReclaimed {
        &self.reclaimed
    }
}

/// Whether deleting rows actually gave the disk space back.
///
/// Deleting rows in SQLite frees pages for reuse WITHOUT shrinking the file, so
/// an operator who checks with `du` after a prune sees no change and reasonably
/// concludes the fix did nothing. This type makes the difference something the
/// caller must look at rather than assume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpaceReclaimed {
    /// `VACUUM` rewrote the database; the file is smaller by the difference.
    Vacuumed {
        /// File size before the rewrite, in bytes.
        bytes_before: u64,
        /// File size after the rewrite, in bytes.
        bytes_after: u64,
    },
    /// The rows are gone but the file was not rewritten, so its pages stay
    /// allocated for reuse. Carries why.
    NotReclaimed(VacuumSkipped),
}

/// Why a prune did not rewrite the database file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VacuumSkipped {
    /// An in-memory cache: there is no file to rewrite or measure.
    NotFileBacked,
    /// Another process held the database. The freed pages remain available for
    /// reuse, and the next prune that finds the database quiet will rewrite it.
    DatabaseBusy,
}

impl std::fmt::Display for VacuumSkipped {
    /// Render the reason as a clause safe to show an operator verbatim.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFileBacked => f.write_str("the cache is in memory, so there is no file"),
            Self::DatabaseBusy => {
                f.write_str("another process held the database; freed pages stay reusable")
            }
        }
    }
}

impl std::fmt::Display for VersionPruneReport {
    /// One line an operator can read without knowing the schema.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "pruned {} unreachable cache row(s) from {} superseded version(s)",
            self.rows_deleted, self.versions_deleted
        )?;
        match &self.reclaimed {
            SpaceReclaimed::Vacuumed {
                bytes_before,
                bytes_after,
            } => write!(
                f,
                "; database {} MB -> {} MB",
                bytes_before / 1_048_576,
                bytes_after / 1_048_576
            ),
            SpaceReclaimed::NotReclaimed(reason) => {
                write!(f, "; file not rewritten ({reason})")
            }
        }
    }
}

/// Which versions survive a prune.
///
/// The current version is obviously live. One predecessor is kept on purpose:
/// pruning strictly to the current version makes any downgrade cold, which is a
/// real cost during a bisect or a rollback, and it also means two chatter builds
/// sharing a machine would each delete the other's rows on every open. One
/// generation of grace costs at most one extra copy of the corpus (bounded)
/// while making both of those cases cheap.
struct RetainedVersions {
    /// The version the opening pool binds.
    current: RulesVersion,
    /// The most recently written OTHER version, if the database holds one.
    predecessor: Option<RulesVersion>,
}

/// Delete every row whose version no reader will bind, and reclaim the space.
///
/// Runs on open, where the version is already known, so there is no separate
/// entry point to remember and no background task to supervise.
pub(super) async fn prune_unreachable_versions(
    pool: &SqlitePool,
    db_path: Option<&std::path::Path>,
    current: &RulesVersion,
) -> Result<VersionPruneOutcome, CacheError> {
    let retained = retained_versions(pool, current).await?;

    let deleted = delete_unretained(pool, &retained).await?;
    if deleted.rows == 0 {
        return Ok(VersionPruneOutcome::NothingUnreachable);
    }

    let reclaimed = reclaim(pool, db_path).await;

    Ok(VersionPruneOutcome::Pruned(VersionPruneReport {
        rows_deleted: deleted.rows,
        versions_deleted: deleted.versions,
        reclaimed,
    }))
}

/// How much one `DELETE` removed.
struct DeletedRows {
    /// Rows removed.
    rows: u64,
    /// Distinct versions those rows belonged to.
    versions: u64,
}

/// Work out which versions survive: the current one, plus the most recently
/// written other one.
async fn retained_versions(
    pool: &SqlitePool,
    current: &RulesVersion,
) -> Result<RetainedVersions, CacheError> {
    // Newest by the most recent row written under it, which is what "the
    // previous build" means in practice: whichever version this machine last
    // used before the current one.
    //
    // The version string breaks ties. `cached_at` has one-second resolution, so
    // two versions written in the same second are indistinguishable by time,
    // and an arbitrary winner would flip between opens and delete a different
    // generation each time. Ordering by the version text as well makes the
    // choice stable; which of two same-second versions wins is not meaningful,
    // but answering the same way every time is.
    let predecessor: Option<(String,)> = sqlx::query_as(
        "SELECT version FROM file_cache
         WHERE version <> ?1
         GROUP BY version
         ORDER BY MAX(cached_at) DESC, version DESC
         LIMIT 1",
    )
    .bind(current.as_str())
    .fetch_optional(pool)
    .await
    .map_err(|source| CacheError::Database { source })?;

    Ok(RetainedVersions {
        current: current.clone(),
        predecessor: predecessor.map(|(version,)| RulesVersion::from_stored(version)),
    })
}

/// Delete everything outside the retention window, counting what went.
async fn delete_unretained(
    pool: &SqlitePool,
    retained: &RetainedVersions,
) -> Result<DeletedRows, CacheError> {
    // Two binds cover the whole window, so the statement shape is fixed and
    // needs no dynamic SQL: with no predecessor, the second bind repeats the
    // first and matches exactly the same rows.
    let predecessor = retained
        .predecessor
        .as_ref()
        .unwrap_or(&retained.current)
        .as_str()
        .to_owned();
    let current = retained.current.as_str().to_owned();

    // Counted before the delete: SQLite reports rows affected but not how many
    // distinct versions they spanned, and that count is what tells an operator
    // whether this was one stale build or a decade of them.
    let versions: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT version) FROM file_cache WHERE version NOT IN (?1, ?2)",
    )
    .bind(&current)
    .bind(&predecessor)
    .fetch_one(pool)
    .await
    .map_err(|source| CacheError::Database { source })?;

    let result = sqlx::query("DELETE FROM file_cache WHERE version NOT IN (?1, ?2)")
        .bind(&current)
        .bind(&predecessor)
        .execute(pool)
        .await
        .map_err(|source| CacheError::Database { source })?;

    Ok(DeletedRows {
        rows: result.rows_affected(),
        versions: versions.0.max(0) as u64,
    })
}

/// Rewrite the database so the freed pages go back to the filesystem.
///
/// Never fatal: the rows are already gone and the cache is correct either way,
/// so a busy database costs a smaller file, not a failed open.
async fn reclaim(pool: &SqlitePool, db_path: Option<&std::path::Path>) -> SpaceReclaimed {
    let Some(path) = db_path else {
        return SpaceReclaimed::NotReclaimed(VacuumSkipped::NotFileBacked);
    };

    let bytes_before = file_size(path);
    match sqlx::query("VACUUM").execute(pool).await {
        Ok(_) => SpaceReclaimed::Vacuumed {
            bytes_before,
            bytes_after: file_size(path),
        },
        Err(error) => {
            tracing::debug!(%error, "cache VACUUM skipped");
            SpaceReclaimed::NotReclaimed(VacuumSkipped::DatabaseBusy)
        }
    }
}

/// Size of `path` in bytes, or 0 when it cannot be measured.
///
/// A size is only ever reported to a human alongside another size; an
/// unreadable file yields a 0 that reads as "unknown" in that line rather than
/// failing a prune that already succeeded.
fn file_size(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rendering exists so an operator sees what a prune did; a silent
    /// 190 MB reclaim reads as "nothing happened".
    #[test]
    fn a_report_renders_rows_versions_and_space() {
        let report = VersionPruneReport {
            rows_deleted: 190_000,
            versions_deleted: 86,
            reclaimed: SpaceReclaimed::Vacuumed {
                bytes_before: 243 * 1_048_576,
                bytes_after: 53 * 1_048_576,
            },
        };
        let rendered = report.to_string();
        assert!(rendered.contains("190000"), "{rendered}");
        assert!(rendered.contains("86"), "{rendered}");
        assert!(rendered.contains("243 MB -> 53 MB"), "{rendered}");
    }

    /// A skipped rewrite says so, and says why, rather than reporting a
    /// reclaim that did not happen.
    #[test]
    fn a_skipped_vacuum_reports_its_reason() {
        let report = VersionPruneReport {
            rows_deleted: 5,
            versions_deleted: 1,
            reclaimed: SpaceReclaimed::NotReclaimed(VacuumSkipped::DatabaseBusy),
        };
        let rendered = report.to_string();
        assert!(rendered.contains("not rewritten"), "{rendered}");
        assert!(rendered.contains("another process"), "{rendered}");
    }
}
