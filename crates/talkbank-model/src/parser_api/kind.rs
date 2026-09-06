//! Shared identity of the parser selected for a CHAT operation.

/// Which parser backend to use for validation.
///
/// Tree-sitter is the default and supports incremental reparsing (used by LSP).
/// Re2c is a DFA-based parser that is faster for batch validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
