//! Top-level CLI configuration types shared across all `chatter`
//! subcommands: logging format, TUI mode, output encoding, parser
//! backend, alignment tier filter, and judgment mode for speaker-id.

use clap::ValueEnum;

/// Supported formats for tracing output.
#[derive(Debug, Clone, ValueEnum)]
pub enum LogFormat {
    /// Human-readable text format
    Text,
    /// JSON format for observability/telemetry tools
    Json,
}

/// Controls whether the interactive TUI is used for validation output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum TuiMode {
    /// Automatically detect terminal capability (TUI when stdout is a TTY)
    #[default]
    Auto,
    /// Force TUI mode regardless of terminal detection
    Force,
    /// Disable TUI mode even in interactive terminals
    Disable,
}

impl TuiMode {
    /// Resolve the mode into a concrete decision, consulting the terminal when `Auto`.
    pub fn should_use_tui(self) -> bool {
        use std::io::IsTerminal;
        match self {
            Self::Force => true,
            Self::Disable => false,
            // `std::io::IsTerminal` (stable since Rust 1.70) replaces the
            // unmaintained `atty` crate (RUSTSEC-2024-0375); identical
            // semantics, no third-party dependency.
            Self::Auto => std::io::stdout().is_terminal(),
        }
    }
}

/// Output encodings for command results.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable validation output
    Text,
    /// Structured JSON output
    Json,
}

/// Which parser backend to use for CHAT parsing.
///
/// Tree-sitter (default) supports incremental reparsing and is used by the LSP.
/// Re2c is a DFA-based parser that is faster for batch validation.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ParserBackend {
    /// Tree-sitter parser (default, supports incremental reparsing)
    #[default]
    TreeSitter,
    /// Re2c DFA parser (faster batch validation)
    Re2c,
}

/// Dependent tiers that `show-alignment` can filter on.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum AlignmentTier {
    /// Morphology tier (%mor)
    Mor,
    /// Grammar tier (%gra)
    Gra,
    /// Phonology tier (%pho)
    Pho,
    /// Syntax tier (%sin)
    Sin,
}

/// How the speaker-id judgment is powered. Orthogonal to the
/// explicit/reference/override mapping-supply modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum JudgmentMode {
    /// Deterministic Jaccard reference matching; no model. (Default.)
    #[default]
    Deterministic,
    /// One holistic LLM judgment per session via the configured endpoint.
    Holistic,
}
