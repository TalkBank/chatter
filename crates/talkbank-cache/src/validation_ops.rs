//! Validation cache read/write operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use sqlx::SqlitePool;

use talkbank_model::ResolvedPath;
use talkbank_model::validation::AlignmentValidation;

use super::cache_utils::{
    self as cache_utils, CacheKey, CachedAt, alignment_column, outcome_column, outcome_from_column,
};
use super::error::CacheError;
use super::trait_def::{CacheLookup, CacheOutcome, ContentHash};
use super::types::CacheIdentity;

/// The cached validation verdict for `content`.
///
/// `rules_version` is bound into the `version` column so a verdict produced
/// under a different validation rule set is a cache MISS, not a stale hit. A
/// database failure is an `Err`, never a miss.
pub async fn get_validation(
    pool: &SqlitePool,
    identity: &CacheIdentity,
    path: &ResolvedPath,
    content: &ContentHash,
    alignment: AlignmentValidation,
) -> Result<CacheLookup<CacheOutcome>, CacheError> {
    let key = CacheKey::of(path, identity.validation_namespace());

    // Query exactly one alignment coverage; callers ask for either aligned or
    // unaligned validation, and both may coexist for the same file hash.
    let row = sqlx::query_as::<_, (String, i64)>(
        "SELECT content_hash, is_valid FROM file_cache
         WHERE path_hash = ?1 AND version = ?2 AND check_alignment = ?3 AND parser_kind IS NULL",
    )
    .bind(key.as_str())
    .bind(identity.rules_version().as_str())
    .bind(alignment_column(alignment))
    .fetch_optional(pool)
    .await
    .map_err(|source| CacheError::Database { source })?;

    match row {
        // A verdict for other content is no verdict for this content.
        Some((cached_hash, is_valid)) if cached_hash == content.as_str() => {
            Ok(CacheLookup::Hit(outcome_from_column(is_valid)?))
        }
        Some(_) | None => Ok(CacheLookup::Miss),
    }
}

/// Store the validation verdict for `content`.
///
/// The row is tagged with `rules_version`, so it is only ever served back to a
/// query carrying the same validation rule set.
pub async fn set_validation(
    pool: &SqlitePool,
    identity: &CacheIdentity,
    path: &ResolvedPath,
    content: &ContentHash,
    alignment: AlignmentValidation,
    outcome: CacheOutcome,
) -> Result<(), CacheError> {
    let key = CacheKey::of(path, identity.validation_namespace());
    let path_str = cache_utils::column(path);

    sqlx::query(
        "INSERT OR REPLACE INTO file_cache
         (path_hash, file_path, content_hash, version, cached_at, check_alignment, is_valid,
          roundtrip_tested, roundtrip_passed, parser_kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, NULL)",
    )
    .bind(key.as_str())
    .bind(&path_str)
    .bind(content.as_str())
    .bind(identity.rules_version().as_str())
    .bind(CachedAt::now().column())
    .bind(alignment_column(alignment))
    .bind(outcome_column(outcome))
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

    const SAMPLE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|demo|CHI|2;00.00|||Target_Child|||\n*CHI:\thello .\n@End\n";

    fn identity() -> CacheIdentity {
        CacheIdentity::new(
            crate::RulesVersion::for_testing("test-rules"),
            talkbank_model::ParserKind::TreeSitter,
        )
    }

    #[tokio::test]
    async fn get_validation_uses_alignment_dimension_in_lookup() {
        let pool = test_pool().await;
        let path =
            ResolvedPath::of_file(std::path::Path::new("sample.cha")).expect("resolvable path");
        let content = ContentHash::of(SAMPLE.as_bytes());
        let identity = identity();

        // Cache contradictory outcomes for the same content but different
        // alignment coverage.
        set_validation(
            &pool,
            &identity,
            &path,
            &content,
            AlignmentValidation::Structure,
            CacheOutcome::Invalid,
        )
        .await
        .expect("cache unaligned result");
        set_validation(
            &pool,
            &identity,
            &path,
            &content,
            AlignmentValidation::IncludeTierAlignment,
            CacheOutcome::Valid,
        )
        .await
        .expect("cache aligned result");

        let lookup = |alignment| get_validation(&pool, &identity, &path, &content, alignment);
        assert_eq!(
            lookup(AlignmentValidation::IncludeTierAlignment)
                .await
                .unwrap(),
            CacheLookup::Hit(CacheOutcome::Valid),
            "aligned lookup should read the aligned cache row"
        );
        assert_eq!(
            lookup(AlignmentValidation::Structure).await.unwrap(),
            CacheLookup::Hit(CacheOutcome::Invalid),
            "unaligned lookup should read the unaligned cache row"
        );
    }

    #[tokio::test]
    async fn set_validation_replaces_existing_row_for_same_key() {
        let pool = test_pool().await;
        let path =
            ResolvedPath::of_file(std::path::Path::new("sample.cha")).expect("resolvable path");
        let content = ContentHash::of(SAMPLE.as_bytes());
        let identity = identity();
        for outcome in [CacheOutcome::Invalid, CacheOutcome::Valid] {
            set_validation(
                &pool,
                &identity,
                &path,
                &content,
                AlignmentValidation::Structure,
                outcome,
            )
            .await
            .expect("cache result");
        }

        let key = CacheKey::of(&path, identity.validation_namespace());
        let row_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM file_cache
             WHERE path_hash = ?1 AND version = ?2 AND check_alignment = ?3 AND parser_kind IS NULL",
        )
        .bind(key.as_str())
        .bind(identity.rules_version().as_str())
        .bind(0_i32)
        .fetch_one(&pool)
        .await
        .expect("count validation rows");

        assert_eq!(
            row_count.0, 1,
            "cache should keep exactly one row per validation key"
        );
        assert_eq!(
            get_validation(
                &pool,
                &identity,
                &path,
                &content,
                AlignmentValidation::Structure
            )
            .await
            .unwrap(),
            CacheLookup::Hit(CacheOutcome::Valid),
            "latest validation result should win"
        );
    }

    /// A verdict stored for other content is a miss for this content.
    #[tokio::test]
    async fn other_content_is_a_miss() {
        let pool = test_pool().await;
        let path =
            ResolvedPath::of_file(std::path::Path::new("sample.cha")).expect("resolvable path");
        let identity = identity();
        set_validation(
            &pool,
            &identity,
            &path,
            &ContentHash::of(b"old"),
            AlignmentValidation::Structure,
            CacheOutcome::Valid,
        )
        .await
        .unwrap();
        assert_eq!(
            get_validation(
                &pool,
                &identity,
                &path,
                &ContentHash::of(b"new"),
                AlignmentValidation::Structure
            )
            .await
            .unwrap(),
            CacheLookup::Miss
        );
    }

    /// A database failure is an error, never a miss.
    #[tokio::test]
    async fn a_database_failure_is_an_error_not_a_miss() {
        let pool = test_pool().await;
        sqlx::query("DROP TABLE file_cache")
            .execute(&pool)
            .await
            .unwrap();
        let lookup = get_validation(
            &pool,
            &identity(),
            &ResolvedPath::of_file(std::path::Path::new("any.cha")).expect("resolvable path"),
            &ContentHash::of(b"any"),
            AlignmentValidation::Structure,
        )
        .await;
        assert!(
            matches!(lookup, Err(CacheError::Database { .. })),
            "{lookup:?}"
        );
    }

    #[tokio::test]
    async fn legacy_parser_unqualified_rows_are_never_served() {
        let pool = test_pool().await;
        let path =
            ResolvedPath::of_file(std::path::Path::new("legacy.cha")).expect("resolvable path");
        let content = ContentHash::of(b"cached source bytes");
        let identity = CacheIdentity::new(
            crate::RulesVersion::for_testing("same-generation"),
            talkbank_model::ParserKind::TreeSitter,
        );
        set_validation(
            &pool,
            &identity,
            &path,
            &content,
            AlignmentValidation::Structure,
            CacheOutcome::Valid,
        )
        .await
        .unwrap();
        sqlx::query("UPDATE file_cache SET path_hash = ?1")
            .bind(
                CacheKey::of(&path, crate::cache_utils::KeyNamespace::LegacyValidation)
                    .as_str()
                    .to_owned(),
            )
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            get_validation(
                &pool,
                &identity,
                &path,
                &content,
                AlignmentValidation::Structure
            )
            .await
            .unwrap(),
            CacheLookup::Miss
        );
    }
}
