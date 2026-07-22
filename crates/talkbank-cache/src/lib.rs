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
//! - [`CacheStats`] exposes coarse cache statistics for reporting and tests
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
//! use talkbank_cache::{CachePool, ValidationCache};
//!
//! let cache = CachePool::in_memory().expect("cache opens");
//! assert_eq!(cache.get(Path::new("example.cha"), false), None);
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
mod cache_utils;
mod init_lock;
mod rules_version;
mod schema_init;

// Operation modules
mod maintenance_ops;
mod roundtrip_ops;
mod validation_ops;

// Core implementation
mod cache_impl;

// Re-export public API
pub use cache_impl::CachePool;
pub use error::CacheError;
pub use rules_version::RulesVersion;
pub use trait_def::{CacheOutcome, ValidationCache};
pub use types::CacheStats;

/// Backward-compatible alias. Prefer `CachePool` in new code.
pub type UnifiedCache = CachePool;
