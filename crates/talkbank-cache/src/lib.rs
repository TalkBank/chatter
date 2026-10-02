// Unit-test modules: panic-family clippy lints relaxed by policy
// (see the workspace [lints] table for the production deny).
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented,
    )
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(missing_docs)]
//! SQLite-backed validation and roundtrip cache for CHAT workflows.
//!
//! The cache answers one question: "Has this file already been
//! validated/roundtrip-tested at this content hash AND under the current
//! validation rule set?" It stores only the pass/fail outcome, not the full
//! diagnostics payload.
//!
//! The "rule set" dimension is captured by [`RulesVersion`], which folds the
//! cache crate version together with a fingerprint of every validation
//! [`ErrorCode`](talkbank_model::ErrorCode) the validator can emit. Adding,
//! removing, or renaming a rule changes the `RulesVersion`, so verdicts cached
//! under the old rule set become a cache MISS instead of being served stale.
//! This is what keeps `chatter validate` honest after a rule like E370 is
//! added.
//!
//! # Start here
//!
//! - [`CachePool`] is the concrete SQLite-backed cache implementation used by
//!   higher-level tools
//! - [`ValidationCache`] is the trait for callers that want to abstract over the
//!   caching backend
//! - [`CacheOutcome`] is the pass/fail enum stored in the cache
//! - [`CacheStats`] exposes coarse cache statistics for reporting and tests:
//!   the entry count and a [`StorageStats`] (in memory, or a directory and its
//!   [`DatabaseFile`])
//!
//! Validation constructors require a `CacheIdentity`. `MaintenanceCache` has no
//! verdict methods and never performs validation-generation pruning; it is
//! reached only from what [`CacheOnDisk::inspect`] found in the directory.
//!
//! # Common entry points
//!
//! - [`CachePool::new`] opens the default OS cache directory
//! - [`CachePool::with_directory`] points the cache at a caller-chosen directory
//! - [`CachePool::in_memory`] provides an isolated in-memory cache for tests
//!
//! # Example
//!
//! ```rust
//! use std::path::Path;
//! use talkbank_cache::{
//!     CacheIdentity, CacheLookup, CachePool, ContentHash, ParserKind, ResolvedPath,
//!     RulesVersion, VerdictReader,
//! };
//! use talkbank_model::validation::AlignmentValidation;
//!
//! let identity = CacheIdentity::new(RulesVersion::current(), ParserKind::TreeSitter);
//! let cache = CachePool::in_memory(identity).expect("cache opens");
//! let content = ContentHash::of(b"@UTF8\n@Begin\n@End\n");
//! let path = ResolvedPath::of_file(Path::new("example.cha")).expect("resolvable path");
//! let lookup = cache.get(&path, &content, AlignmentValidation::Structure);
//! assert_eq!(lookup.expect("lookup runs"), CacheLookup::Miss);
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod error;
mod trait_def;
mod types;

// Utility and infrastructure modules
mod blocking;
mod cache_location;
mod cache_utils;
mod init_lock;
mod rules_version;
mod schema_init;
mod version_prune;

// Operation modules
mod maintenance_ops;
mod roundtrip_ops;
mod validation_ops;

// Core implementation
mod cache_impl;

// Re-export public API
pub use cache_impl::{
    CacheOnDisk, CachePool, InspectionCache, InspectionScope, MaintenanceCache, MaintenanceScope,
    NoDatabase, OlderSchema, ReadOnlyCache, ReadOnlyScope, ValidationScope,
};
pub use cache_location::{CACHE_DIR_ENV, cache_db_path, default_cache_dir};
pub use error::CacheError;
pub use maintenance_ops::CacheScope;
pub use rules_version::RulesVersion;
pub use talkbank_model::ParserKind;
pub use talkbank_model::{ResolvedDirectory, ResolvedPath, ResolvedPrefix};
pub use trait_def::{
    CacheLookup, CacheOutcome, ContentHash, RoundtripOutcome, ValidationCache, VerdictReader,
};
pub use types::{CacheIdentity, CacheStats, DatabaseFile, StorageStats};
pub use version_prune::{SpaceReclaimed, VacuumSkipped, VersionPruneOutcome, VersionPruneReport};

/// Backward-compatible alias. Prefer `CachePool` in new code.
pub type UnifiedCache = CachePool;

/// Compiles this crate's README so its examples cannot rot.
///
/// `#[cfg(doctest)]`, so the README is not duplicated into the rendered crate
/// docs; it exists only to make `cargo test --doc` typecheck every ```rust
/// block in it. Until 2026-08-27 no crate here did this and every README
/// example in the workspace was unchecked prose; see `talkbank-transform` for
/// the one that was already wrong.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
