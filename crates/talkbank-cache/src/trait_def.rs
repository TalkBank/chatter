//! Validation cache trait
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use talkbank_model::validation::AlignmentValidation;

use crate::{CacheError, ResolvedPath};

/// A cached validation verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheOutcome {
    /// The file produced no diagnostic at all.
    Valid,
    /// The file produced at least one diagnostic.
    Invalid,
}

/// A cached roundtrip verdict: whether the file wrote back byte for byte.
///
/// Its own type, not a second use of [`CacheOutcome`]: a roundtrip that
/// "failed" says nothing about CHAT validity, and reading one verdict as the
/// other is the confusion two meanings of one enum invited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundtripOutcome {
    /// Serialized, re-parsed and compared equal.
    Passed,
    /// The serialization did not reproduce the file.
    Failed,
}

/// What a cache lookup found: a verdict of type `V`, or none. A failed
/// lookup is an `Err`, never a `Miss`, so a locked or corrupt database is
/// reported rather than answering like a cold cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLookup<V> {
    /// A verdict for this content, rules, parser and alignment coverage.
    Hit(V),
    /// No verdict for it.
    Miss,
}

/// The content of a file as the cache keys it: a blake3 hash of the bytes
/// the caller actually read. Made from those bytes, never from a second read
/// of the path, so a verdict is stored for the content that was validated
/// even if the file changes in between.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentHash(String);

impl ContentHash {
    /// Hash the bytes that were read.
    pub fn of(content: &[u8]) -> Self {
        Self(blake3::hash(content).to_hex().to_string())
    }

    /// The hex form stored in the `content_hash` column.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reading cached verdicts: the half of a cache a read-only run may use.
///
/// Uses interior mutability (`&self`) for concurrent access. Every method is
/// required: a cache that keeps no roundtrip verdicts answers `Miss` in its
/// own body, rather than inheriting a silent default.
pub trait VerdictReader: Send + Sync {
    /// The identity this cache was opened for: the rule generation and the
    /// parser whose verdicts it reads and records. A validation run admits a
    /// cache only when this equals its own (see
    /// `talkbank_transform::ValidationRun`), so a cache opened for one rule set
    /// cannot answer a run under another.
    fn identity(&self) -> &crate::CacheIdentity;

    /// The cached validation verdict for this content.
    fn get(
        &self,
        path: &ResolvedPath,
        content: &ContentHash,
        alignment: AlignmentValidation,
    ) -> Result<CacheLookup<CacheOutcome>, CacheError>;

    /// The cached roundtrip verdict for this content.
    fn get_roundtrip(
        &self,
        path: &ResolvedPath,
        content: &ContentHash,
        alignment: AlignmentValidation,
    ) -> Result<CacheLookup<RoundtripOutcome>, CacheError>;
}

/// A cache that reads and records verdicts.
///
/// Returns pass/fail outcomes only; error details are not cached. Every
/// method is required: a cache that stores no roundtrip verdicts says so by
/// storing nothing in its own body, rather than by inheriting a silent
/// default. A run that may only read (an audit) is handed a
/// [`VerdictReader`], which has no way to write.
pub trait ValidationCache: VerdictReader {
    /// Store a validation verdict for this content.
    fn set(
        &self,
        path: &ResolvedPath,
        content: &ContentHash,
        alignment: AlignmentValidation,
        outcome: CacheOutcome,
    ) -> Result<(), CacheError>;

    /// Store a roundtrip verdict for this content.
    fn set_roundtrip(
        &self,
        path: &ResolvedPath,
        content: &ContentHash,
        alignment: AlignmentValidation,
        outcome: RoundtripOutcome,
    ) -> Result<(), CacheError>;
}
