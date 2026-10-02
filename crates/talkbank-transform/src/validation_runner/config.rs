//! Validation-runner configuration types.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub use talkbank_model::ParserKind;

use std::sync::Arc;

/// The cache a run uses, and how: the one value that says both whether
/// there is a cache and what the run may do with it, so "a mode but no
/// cache" cannot be built. A read-only run holds a
/// [`VerdictReader`](talkbank_cache::VerdictReader), which has no write
/// method at all.
#[derive(Clone)]
pub enum RunCache {
    /// No cache: nothing is consulted and nothing recorded.
    Absent,
    /// Read verdicts, never record or delete (an audit).
    ReadOnly(Arc<dyn talkbank_cache::VerdictReader>),
    /// Read verdicts and record new ones.
    ReadWrite(Arc<dyn talkbank_cache::ValidationCache>),
}

impl std::fmt::Debug for RunCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Absent => "RunCache::Absent",
            Self::ReadOnly(_) => "RunCache::ReadOnly(..)",
            Self::ReadWrite(_) => "RunCache::ReadWrite(..)",
        })
    }
}

impl RunCache {
    /// The identity of the cache, or `None` when there is none.
    fn identity(&self) -> Option<&talkbank_cache::CacheIdentity> {
        match self {
            Self::Absent => None,
            Self::ReadOnly(reader) => Some(reader.identity()),
            Self::ReadWrite(cache) => Some(cache.identity()),
        }
    }
}

/// A run's configuration bound to the cache it may use: what every runner
/// entry point takes.
///
/// A cache's verdicts answer one rule set and one parser (its
/// [`CacheIdentity`](talkbank_cache::CacheIdentity)), and a run validates
/// under its configuration's ([`ValidationConfig::cache_identity`]). The only
/// routes to a `ValidationRun` are [`Self::uncached`] and [`Self::new`], which
/// refuses a cache opened for another identity, so a cache opened for one
/// rule set (strict linkers, say) cannot serve its verdicts to a run under
/// another. The configuration cannot change once bound: it is read through
/// [`Self::config`].
#[derive(Debug, Clone)]
pub struct ValidationRun {
    /// What the run validates with.
    config: ValidationConfig,
    /// The cache, opened for `config`'s identity, or none.
    cache: RunCache,
}

/// A cache opened for another rule set or parser than the run's.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "the cache was opened for {cache:?}, but this run validates under {run:?}; \
     open the cache with the run's ValidationConfig::cache_identity"
)]
pub struct CacheIdentityMismatch {
    /// The identity the run validates under.
    pub run: talkbank_cache::CacheIdentity,
    /// The identity the cache was opened for.
    pub cache: talkbank_cache::CacheIdentity,
}

impl ValidationRun {
    /// A run that consults no cache.
    pub fn uncached(config: ValidationConfig) -> Self {
        Self {
            config,
            cache: RunCache::Absent,
        }
    }

    /// A run with `cache`, refused when the cache was opened for another
    /// identity than `config`'s.
    pub fn new(config: ValidationConfig, cache: RunCache) -> Result<Self, CacheIdentityMismatch> {
        let run = config.cache_identity();
        match cache.identity() {
            Some(opened) if *opened != run => Err(CacheIdentityMismatch {
                run,
                cache: opened.clone(),
            }),
            Some(_) | None => Ok(Self { config, cache }),
        }
    }

    /// The configuration the run validates with.
    pub fn config(&self) -> &ValidationConfig {
        &self.config
    }

    /// The cache the run uses, and how.
    pub(super) fn cache(&self) -> &RunCache {
        &self.cache
    }
}

/// Whether a file that validates clean is also roundtrip-checked: serialized,
/// re-parsed and compared. A developer check of the serializer, off by
/// default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundtripCheck {
    /// Validate only (the default).
    #[default]
    Skip,
    /// Also roundtrip-check each clean file.
    Run,
}

/// Configuration for validation runner
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Whether validation also checks tier alignment (more thorough, slower).
    /// Part of the cache key: verdicts for the two coverages are kept apart.
    pub alignment: talkbank_model::validation::AlignmentValidation,

    /// Number of parallel workers, or `None` for the machine's available
    /// parallelism. Non-zero by type: a zero count is refused where it is
    /// parsed (the CLI's `--jobs`, the desktop request), so no run is ever
    /// asked for zero workers.
    pub jobs: Option<std::num::NonZeroUsize>,

    /// Whether a clean file is also roundtrip-checked.
    pub roundtrip: RoundtripCheck,

    /// How many errors the run may find before it stops itself. Counted by
    /// the runner from each file's status, so every frontend (CLI, TUI,
    /// desktop) gets the same stop, and warnings never count.
    pub error_limit: super::ErrorLimit,

    /// Which parser backend to use
    pub parser_kind: ParserKind,

    /// WHICH RULES RUN: the rule set the worker validates against, and the
    /// semantic rule component of the cache identity, alongside `parser_kind`.
    ///
    /// Deliberately separate from [`Self::presentation`]. v0.6.0 held both in
    /// one value and keyed the cache on all of it, so a `--suppress` list
    /// partitioned the cache and a second pass over a 106,000-file corpus
    /// re-validated everything. The two fields answer different questions and a
    /// caller composing a cache key can only reach for this one.
    pub rules: talkbank_model::RuleSelection,

    /// WHAT A READER SEES: suppression and severity remapping, applied to
    /// diagnostics the worker has already computed and already counted.
    ///
    /// Never consulted when deciding what to cache. The cached fact is "this
    /// file produced no diagnostics at all under `rules`", which is true or
    /// false independently of how any run chooses to display them.
    pub presentation: crate::PresentationPolicy,
}

impl Default for ValidationConfig {
    /// Create the default validation-runner configuration.
    fn default() -> Self {
        Self {
            alignment: talkbank_model::validation::AlignmentValidation::IncludeTierAlignment,
            jobs: None,
            roundtrip: RoundtripCheck::Skip,
            error_limit: super::ErrorLimit::Unlimited,
            parser_kind: ParserKind::TreeSitter,
            rules: talkbank_model::RuleSelection::default(),
            presentation: crate::PresentationPolicy::default(),
        }
    }
}

impl ValidationConfig {
    /// Bind the actual parser and semantic rules without presentation policy.
    pub fn cache_identity(&self) -> talkbank_cache::CacheIdentity {
        talkbank_cache::CacheIdentity::new(
            talkbank_cache::RulesVersion::current_with_rule_selection(
                &self.rules,
                crate::parser_behavior_fingerprint(),
            ),
            self.parser_kind,
        )
    }
}
