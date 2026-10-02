//! Roundtrip test cache operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use sqlx::SqlitePool;

use talkbank_model::ResolvedPath;
use talkbank_model::validation::AlignmentValidation;

use super::cache_utils::{
    self as cache_utils, CacheKey, CachedAt, alignment_column, roundtrip_column,
    roundtrip_from_column,
};
use super::error::CacheError;
use super::trait_def::{CacheLookup, ContentHash, RoundtripOutcome};
use super::types::CacheIdentity;

/// The cached roundtrip verdict for `content`.
///
/// `rules_version` is bound into the `version` column so a roundtrip outcome
/// produced under a different validation rule set is a cache MISS. A
/// database failure, or a tested row with no verdict, is an `Err`.
pub async fn get_roundtrip(
    pool: &SqlitePool,
    identity: &CacheIdentity,
    path: &ResolvedPath,
    content: &ContentHash,
    alignment: AlignmentValidation,
) -> Result<CacheLookup<RoundtripOutcome>, CacheError> {
    let key = CacheKey::of(path, identity.roundtrip_namespace());

    let row = sqlx::query_as::<_, (String, i64, Option<i64>)>(
        "SELECT content_hash, roundtrip_tested, roundtrip_passed
         FROM file_cache
         WHERE path_hash = ?1 AND version = ?2 AND check_alignment = ?3 AND parser_kind = ?4",
    )
    .bind(key.as_str())
    .bind(identity.rules_version().as_str())
    .bind(alignment_column(alignment))
    .bind(identity.parser().cache_label())
    .fetch_optional(pool)
    .await
    .map_err(|source| CacheError::Database { source })?;

    match row {
        // A verdict for other content is no verdict for this content.
        Some((cached_hash, ..)) if cached_hash != content.as_str() => Ok(CacheLookup::Miss),
        None => Ok(CacheLookup::Miss),
        // Not roundtrip-tested: no roundtrip verdict.
        Some((_, 0, _)) => Ok(CacheLookup::Miss),
        Some((_, 1, Some(passed))) => Ok(CacheLookup::Hit(roundtrip_from_column(passed)?)),
        Some((_, 1, None)) => Err(CacheError::CorruptColumn {
            column: "roundtrip_passed",
            value: None,
        }),
        Some((_, tested, _)) => Err(CacheError::CorruptColumn {
            column: "roundtrip_tested",
            value: Some(tested),
        }),
    }
}

/// Store the roundtrip verdict for `content`.
///
/// The row is tagged with `rules_version`, so it is only ever served back to a
/// query carrying the same validation rule set.
pub async fn set_roundtrip(
    pool: &SqlitePool,
    identity: &CacheIdentity,
    path: &ResolvedPath,
    content: &ContentHash,
    alignment: AlignmentValidation,
    outcome: RoundtripOutcome,
) -> Result<(), CacheError> {
    let key = CacheKey::of(path, identity.roundtrip_namespace());
    let path_str = cache_utils::column(path);
    let passed = roundtrip_column(outcome);

    sqlx::query(
        "INSERT OR REPLACE INTO file_cache
         (path_hash, file_path, content_hash, version, cached_at, check_alignment, is_valid,
          roundtrip_tested, roundtrip_passed, parser_kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9)",
    )
    .bind(key.as_str())
    .bind(&path_str)
    .bind(content.as_str())
    .bind(identity.rules_version().as_str())
    .bind(CachedAt::now().column())
    .bind(alignment_column(alignment))
    .bind(passed) // is_valid mirrors roundtrip result
    .bind(passed)
    .bind(identity.parser().cache_label())
    .execute(pool)
    .await
    .map_err(|source| CacheError::Database { source })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("open sqlite in-memory");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        pool
    }

    #[tokio::test]
    async fn set_roundtrip_replaces_existing_row_for_same_key() {
        let pool = test_pool().await;
        let path =
            ResolvedPath::of_file(std::path::Path::new("sample.cha")).expect("resolvable path");
        let content = ContentHash::of("@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|demo|CHI|2;00.00|||Target_Child|||\n*CHI:\thello .\n@End\n".as_bytes());
        let identity = CacheIdentity::new(
            crate::RulesVersion::for_testing("test-rules"),
            talkbank_model::ParserKind::TreeSitter,
        );
        for outcome in [RoundtripOutcome::Failed, RoundtripOutcome::Passed] {
            set_roundtrip(
                &pool,
                &identity,
                &path,
                &content,
                AlignmentValidation::Structure,
                outcome,
            )
            .await
            .expect("cache roundtrip result");
        }

        let key = CacheKey::of(&path, identity.roundtrip_namespace());
        let row_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM file_cache
             WHERE path_hash = ?1 AND version = ?2 AND check_alignment = ?3 AND parser_kind = ?4",
        )
        .bind(key.as_str())
        .bind(identity.rules_version().as_str())
        .bind(0_i32)
        .bind("tree-sitter")
        .fetch_one(&pool)
        .await
        .expect("count roundtrip rows");

        assert_eq!(
            row_count.0, 1,
            "cache should keep exactly one row per roundtrip key"
        );
        assert_eq!(
            get_roundtrip(
                &pool,
                &identity,
                &path,
                &content,
                AlignmentValidation::Structure
            )
            .await
            .unwrap(),
            CacheLookup::Hit(RoundtripOutcome::Passed),
            "latest roundtrip result should win"
        );
    }
}
