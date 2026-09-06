//! Type definitions for unified cache
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::PathBuf;

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of file entries in the cache database.
    pub total_entries: usize,
    /// Filesystem path to the cache directory.
    pub cache_dir: PathBuf,
}

/// Validation identity: build/rule generation and parser row namespace.
/// Parser identity is deliberately outside the retained build generations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheIdentity {
    rules_version: crate::RulesVersion,
    parser: talkbank_model::ParserKind,
}

impl CacheIdentity {
    /// Bind a rule generation to the parser that will produce its verdicts.
    pub fn new(rules_version: crate::RulesVersion, parser: talkbank_model::ParserKind) -> Self {
        Self {
            rules_version,
            parser,
        }
    }

    /// Build/rule generation used for compatibility and retention.
    pub fn rules_version(&self) -> &crate::RulesVersion {
        &self.rules_version
    }

    /// Parser namespace carried by every validation and roundtrip lookup.
    pub fn parser(&self) -> talkbank_model::ParserKind {
        self.parser
    }

    pub(crate) fn validation_suffix(&self) -> &'static str {
        match self.parser {
            talkbank_model::ParserKind::TreeSitter => "validation:tree-sitter",
            talkbank_model::ParserKind::Re2c => "validation:re2c",
        }
    }
}
