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

// The mode is decided together with the output format, `--quiet` and
// `--audit`, never alone: see `ValidationPresentation::resolve`, which also
// consults the terminal (with `std::io::IsTerminal`) for `Auto`.

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
/// Re2c is an independent DFA-based implementation for backend comparison.
///
/// Use the default backend for production validity decisions. Neither
/// implementation is a language oracle: adjudicate disagreements against the
/// CHAT specification, and do not infer completeness from finite comparisons.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ParserBackend {
    /// Tree-sitter parser (default, supports incremental reparsing).
    /// Use for production validity decisions.
    #[default]
    TreeSitter,
    /// Re2c DFA parser for backend comparison, not production validity
    /// decisions. Check disagreements against the CHAT specification.
    Re2c,
}

impl From<ParserBackend> for talkbank_transform::ParserKind {
    fn from(backend: ParserBackend) -> Self {
        match backend {
            ParserBackend::TreeSitter => Self::TreeSitter,
            ParserBackend::Re2c => Self::Re2c,
        }
    }
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
