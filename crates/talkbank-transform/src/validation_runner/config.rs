//! Validation-runner configuration types.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

/// Which parser backend to use for validation.
///
/// Tree-sitter is the default and supports incremental reparsing (used by LSP).
/// Re2c is a DFA-based parser that is faster for batch validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserKind {
    /// Tree-sitter parser (default, supports incremental reparsing).
    TreeSitter,
    /// Re2c DFA parser (faster batch validation, no incremental support).
    Re2c,
}

impl ParserKind {
    /// Label used for cache keys (must be stable across runs).
    pub fn cache_label(self) -> &'static str {
        match self {
            ParserKind::TreeSitter => "tree-sitter",
            ParserKind::Re2c => "re2c",
        }
    }
}

/// How a validation run may use the cache.
///
/// Reads and writes are separate permissions because bulk audit runs need
/// exactly one of them. An audit is a reporting sweep over a whole corpus: it
/// should benefit from work already cached, but must not rewrite shared cache
/// state as a side effect of producing a report. Before this was modelled
/// here, audit mode was a separate pipeline that achieved read-without-write
/// by hand, calling the getter and simply never calling the setter, so the
/// contract lived in one function body instead of in the type. Folding audit
/// onto the shared runtime silently turned the writes back on, which the
/// `..._without_cache_writes` tests caught.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheMode {
    /// Read cached results and record new ones (default).
    #[default]
    Enabled,
    /// Read cached results, but never write. For reporting runs that must not
    /// mutate shared state.
    ReadOnly,
    /// Skip all cache lookups and writes.
    Disabled,
}

impl CacheMode {
    /// May this run consult cached results?
    pub fn allows_reads(self) -> bool {
        matches!(self, Self::Enabled | Self::ReadOnly)
    }

    /// May this run record results for later runs?
    pub fn allows_writes(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Whether to recurse into subdirectories when collecting .cha files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DirectoryMode {
    /// Process only the immediate directory.
    SingleFile,
    /// Recurse into subdirectories (default).
    #[default]
    Recursive,
}

/// Configuration for validation runner
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Check tier alignment (more thorough, slower)
    pub check_alignment: bool,

    /// Number of parallel jobs (None = use all CPUs)
    pub jobs: Option<usize>,

    /// Whether to use the validation cache
    pub cache: CacheMode,

    /// How to traverse directories when collecting .cha files
    pub directory: DirectoryMode,

    /// Run roundtrip test (serialize -> re-parse -> compare) after validation
    pub roundtrip: bool,

    /// Which parser backend to use
    pub parser_kind: ParserKind,

    /// WHICH RULES RUN: the rule set the worker validates against, and the
    /// only part of this configuration the cache key is derived from.
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
            check_alignment: true,
            jobs: None,
            cache: CacheMode::Enabled,
            directory: DirectoryMode::Recursive,
            roundtrip: false,
            parser_kind: ParserKind::TreeSitter,
            rules: talkbank_model::RuleSelection::default(),
            presentation: crate::PresentationPolicy::default(),
        }
    }
}
