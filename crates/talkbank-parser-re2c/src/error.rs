//! Parse diagnostics and error infrastructure.

use miette::{Diagnostic, SourceSpan};
use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;
use thiserror::Error;

/// A span in the source input (byte offsets).
pub type Span = Range<usize>;

/// Kinds of dependent tiers, for tracking parse health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TierKind {
    /// The `%mor` morphology tier.
    Mor,
    /// The `%gra` grammatical-relations tier.
    Gra,
    /// The `%pho` phonology tier.
    Pho,
    /// The `%sin` dependent tier.
    Sin,
    /// The `%wor` word-level timing tier.
    Wor,
    /// The `%act` actions tier.
    Act,
    /// The `%cod` coding tier.
    Cod,
    /// The `%com` dependent comment tier.
    Com,
    /// Any other or unrecognized dependent tier.
    Other,
}

/// Tracks which parts of an utterance failed to parse.
///
/// Downstream consumers check this before operating on partial data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseHealth {
    /// The set of dependent tiers that failed to parse cleanly.
    pub tainted_tiers: BTreeSet<TierKind>,
}

impl ParseHealth {
    /// Mark `tier` as tainted (it failed to parse cleanly).
    pub fn taint(&mut self, tier: TierKind) {
        self.tainted_tiers.insert(tier);
    }

    /// Returns `true` if no tier is tainted.
    pub fn is_clean(&self) -> bool {
        self.tainted_tiers.is_empty()
    }

    /// Returns `true` if the given `tier` is tainted.
    pub fn is_tainted(&self, tier: TierKind) -> bool {
        self.tainted_tiers.contains(&tier)
    }
}

/// A parse diagnostic with source location.
#[derive(Debug, Clone, Error, Diagnostic)]
pub enum ParseDiagnostic {
    /// An unexpected token was encountered during parsing.
    #[error("unexpected token: {message}")]
    #[diagnostic(code(chat::unexpected_token))]
    UnexpectedToken {
        /// The source text the span refers into.
        #[source_code]
        src: String,
        /// Byte span of the offending token.
        #[label("{message}")]
        span: SourceSpan,
        /// Human-readable description of what went wrong.
        message: String,
    },

    /// The lexer could not tokenize the input at this location.
    #[error("lexer error: {message}")]
    #[diagnostic(code(chat::lexer_error))]
    LexerError {
        /// The source text the span refers into.
        #[source_code]
        src: String,
        /// Byte span of the lexer failure.
        #[label("{message}")]
        span: SourceSpan,
        /// Human-readable description of what went wrong.
        message: String,
    },

    /// An expected token was missing (the parser recovered by insertion).
    #[error("missing expected token: {expected}")]
    #[diagnostic(code(chat::missing_token), help("expected {expected}"))]
    MissingToken {
        /// The source text the span refers into.
        #[source_code]
        src: String,
        /// Byte span where the expected token should have appeared.
        #[label("expected {expected} here")]
        span: SourceSpan,
        /// Description of the token that was expected.
        expected: String,
    },

    /// A dependent tier appeared with no preceding main (`*SPK:`) tier.
    #[error("orphan dependent tier")]
    #[diagnostic(
        code(chat::orphan_dependent_tier),
        help("dependent tiers must follow a main tier (*SPK:)")
    )]
    OrphanDependentTier {
        /// The source text the span refers into.
        #[source_code]
        src: String,
        /// Byte span of the orphaned dependent tier.
        #[label("this dependent tier has no preceding main tier")]
        span: SourceSpan,
    },
}

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A hard error.
    Error,
    /// A non-fatal warning.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
        }
    }
}

/// Parse result: AST + accumulated diagnostics.
#[derive(Debug)]
pub struct ParseResult<T> {
    /// The (possibly partial) parse output.
    pub value: T,
    /// All diagnostics accumulated during parsing.
    pub diagnostics: Vec<ParseDiagnostic>,
}

impl<T> ParseResult<T> {
    /// Returns `true` if any diagnostics were accumulated during parsing.
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}
