#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(missing_docs)]
//! SQLite-backed validation and roundtrip cache for CHAT workflows.
//!
//! The cache answers one question: "Has this file already been
//! validated/roundtrip-tested at this mtime and tool version?" It stores only
//! the pass/fail outcome, not the full diagnostics payload.
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
pub use trait_def::{CacheOutcome, ValidationCache};
pub use types::CacheStats;

/// Backward-compatible alias. Prefer `CachePool` in new code.
pub type UnifiedCache = CachePool;
