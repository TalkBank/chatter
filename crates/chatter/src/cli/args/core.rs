use crate::cli::catalog::{Contract, Shape, command_tree};
use std::path::PathBuf;

use super::cache_commands::CacheCommands;
use super::cli_types::{AlignmentTier, OutputFormat, ParserBackend};
use super::debug_commands::DebugCommands;
use super::flag_modes::{Flag, NormalizeCheckArgs, ToJsonCheckArgs};

command_tree! {
pub enum Commands {
    /// Validate CHAT file(s)
    Validate {
        /// Paths to CHAT files or directories.
        #[arg(required = true)]
        path: Vec<PathBuf>,

        /// Terminal UI policy.
        #[arg(long, value_enum, default_value_t)]
        tui_mode: super::TuiMode,

        /// Color theme for terminal validation.
        #[arg(long, value_enum)]
        theme: Option<crate::ui::ThemePreset>,

        /// Output format: text (default) or json
        #[arg(short, long, value_enum, default_value_t = OutputFormat::Text, help = "Validation output style (text|json)")]
        format: OutputFormat,

        /// `--skip-alignment`, parsed into the coverage it selects.
        #[command(flatten)]
        alignment: Flag<talkbank_model::validation::AlignmentValidation>,

        /// `--refresh-cache`, parsed into the cache refresh it selects.
        #[command(flatten)]
        cache_refresh: Flag<crate::commands::CacheRefreshMode>,

        /// Number of parallel jobs (default: number of CPUs). At least 1:
        /// `--jobs 0` is a usage error.
        #[arg(short, long)]
        jobs: Option<std::num::NonZeroUsize>,

        /// Suppress success output (errors still print)
        #[arg(long, help = "Quiet mode (only emit errors, rely on exit codes)")]
        quiet: bool,

        /// Stop once this many errors have been found, across all files.
        /// Errors only: warnings never count. Files already being validated
        /// finish; the run then reports how many were not validated and
        /// exits unsuccessfully. At least 1: `--max-errors 0` is a usage
        /// error.
        #[arg(long)]
        max_errors: Option<std::num::NonZeroUsize>,

        /// `--roundtrip` (serialize, re-parse, compare each clean file; a
        /// developer check of the serializer), parsed into its mode.
        #[command(flatten)]
        roundtrip: Flag<talkbank_transform::RoundtripCheck>,

        /// Parser backend for CHAT parsing.
        /// tree-sitter (default) supports incremental reparsing.
        /// re2c is a DFA-based parser that is faster for batch validation.
        #[arg(long, value_enum, default_value_t)]
        parser: ParserBackend,

        /// Audit mode: stream errors to JSONL file without caching (for bulk corpus validation).
        /// Reads from cache to skip clean files (fast), but doesn't write new errors to cache (avoids OOM).
        /// Generates summary statistics at the end.
        #[arg(
            long,
            help = "Stream errors to JSONL file (bulk audit mode)",
            value_name = "OUTPUT_FILE",
            conflicts_with_all = ["format", "quiet", "refresh-cache"]
        )]
        audit: Option<PathBuf>,

        /// Enable strict cross-utterance linker validation.
        ///
        /// Checks that the quotation linkers (+", +"/. , +".) and the
        /// completion linkers (+, and ++) are paired with the terminators
        /// they continue. Off by default because many existing corpora do
        /// not follow these strict conventions.
        ///
        /// The code list that stood here, "(E351-E355)", was wrong in both
        /// this doc and the help text below: the option also turns on E341,
        /// E344 and E346. Naming a range here at all was the mistake, since
        /// the generated error index already states, per code, which option
        /// it requires, derived from the spec example that demonstrates it.
        #[command(flatten)]
        strict_linkers: Flag<talkbank_model::LinkerChecks>,

        /// Suppress error codes or named groups. Suppressed errors are not
        /// reported and do not cause a non-zero exit code.
        ///
        /// Named groups:
        ///   "xphon": the whole Phon `%x` dependent-tier validation surface
        ///            (%xmodsyl/%xphosyl/%xphoaln/%xphoint, codes E725-E728 and
        ///            E735-E746). These checks run by default; pass
        ///            `--suppress xphon` to silence the group.
        ///
        /// Can mix groups and codes: --suppress xphon,E316
        ///
        /// Group and code names are matched case-insensitively. A value that
        /// names neither a known group nor a known error code is a usage
        /// error (exit 2, naming the offending value), parsed by clap.
        #[arg(
            long,
            value_delimiter = ',',
            value_parser = crate::commands::error_codes::parse_suppression,
            help = "Suppress error codes or groups (e.g., --suppress xphon,E726)"
        )]
        suppress: Vec<crate::commands::error_codes::SuppressionSelector>,

    } => Shape::Leaf(Contract::Validate),


    /// Apply catalog fixes to CHAT file(s) at exact byte spans
    Fix {
        /// Paths to CHAT files or directories
        #[arg(required = true)]
        paths: Vec<PathBuf>,

        /// `--apply` writes; without it fix only reports. Parsed straight
        /// into one value.
        #[command(flatten)]
        mode: Flag<super::fix_mode::FixMode>,

        /// Restrict to these error codes; required to apply a Semantic fix.
        /// An unknown code is a usage error (exit 2).
        #[arg(long = "code", value_parser = crate::commands::error_codes::parse_error_code)]
        codes: Vec<talkbank_model::ErrorCode>,

        /// `--skip-alignment`, parsed into the coverage it selects.
        #[command(flatten)]
        alignment: Flag<talkbank_model::validation::AlignmentValidation>,
    } => Shape::Leaf(Contract::Fix),


    /// Normalize CHAT file to canonical format
    Format {
        /// Input CHAT file path
        input: PathBuf,

        /// Output CHAT file path (if not specified, prints to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// `--validate` and `--skip-alignment`, parsed into one check level.
        #[command(flatten)]
        checks: NormalizeCheckArgs,
    } => Shape::Leaf(Contract::Format),


    /// Create a new minimal valid CHAT file
    New {
        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Speaker code (default: CHI)
        #[arg(short, long, default_value = "CHI")]
        speaker: String,

        /// ISO 639-3 language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Participant role (default: Target_Child)
        #[arg(short, long, default_value = "Target_Child")]
        role: String,

        /// Corpus identifier (default: corpus)
        #[arg(short, long, default_value = "corpus")]
        corpus: String,

        /// Initial utterance content (optional)
        #[arg(short, long)]
        utterance: Option<String>,
    } => Shape::Leaf(Contract::New),


    /// Watch CHAT file(s) for changes and continuously validate
    Watch {
        /// Path to CHAT file or directory to watch
        path: PathBuf,

        /// Parser backend for initial validation and every subsequent change.
        #[arg(long, value_enum, default_value_t)]
        parser: ParserBackend,

        /// `--skip-alignment`, parsed into the coverage it selects.
        #[command(flatten)]
        alignment: Flag<talkbank_model::validation::AlignmentValidation>,

        /// Clear terminal before each validation run
        #[arg(short, long)]
        clear: bool,
    } => Shape::Leaf(Contract::Watch),


    /// Cache management operations
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    } => Shape::Namespace(CacheCommands::shapes),


    /// Print the CHAT JSON Schema
    #[command(long_about = "Print the CHAT JSON Schema to stdout.\n\n\
        The schema describes the structure of CHAT transcripts serialized to JSON \
        by `chatter convert to-json`. It is auto-generated from the Rust data model \
        and conforms to JSON Schema 2020-12.\n\n\
        Canonical URL: https://talkbank.org/schemas/v0.1/chat-file.json")]
    Schema {
        /// Print only the canonical schema URL instead of the full schema
        #[arg(
            long,
            help = "Print only the canonical URL (https://talkbank.org/schemas/v0.1/chat-file.json)"
        )]
        url: bool,
    } => Shape::Leaf(Contract::Schema),
    /// Convert between CHAT and model JSON.
    Convert { #[command(subcommand)] command: ConvertCommands } => Shape::Namespace(ConvertCommands::shapes),
    /// Inspect transcript structure without changing it.
    Inspect { #[command(subcommand)] command: InspectCommands } => Shape::Namespace(InspectCommands::shapes),
    /// Discover validation diagnostics and repair policies.
    Checks { #[command(subcommand)] command: CheckCommands } => Shape::Namespace(CheckCommands::shapes),
    /// Experimental speaker workflows and specialist transformations.
    Experimental { #[command(subcommand)] command: ExperimentalCommands } => Shape::Namespace(ExperimentalCommands::shapes),
    /// Discover executable commands, options, output and side effects.
    #[command(name = "commands")]
    Discovery { #[command(subcommand)] command: DiscoveryCommands } => Shape::Namespace(DiscoveryCommands::shapes),

}
}
command_tree! {
pub enum ConvertCommands {

    /// Convert CHAT file to JSON
    #[command(long_about = "Convert CHAT transcript(s) to JSON.\n\n\
        Output conforms to the TalkBank CHAT JSON Schema:\n\
        https://talkbank.org/schemas/v0.1/chat-file.json\n\n\
        Single file: prints JSON to stdout or writes to --output.\n\
        Directory: requires --output-dir. Walks recursively, preserving structure.\n\
        Incremental by default: skips files whose JSON is already up-to-date (mtime check).\n\
        Use --rebuild to rebuild all. Use --prune to remove orphaned .json files.")]
    ToJson {
        /// Input CHAT file or directory path
        input: PathBuf,

        /// Output JSON file path (single-file mode; prints to stdout if omitted)
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,

        /// Output directory (directory mode; preserves relative structure)
        #[arg(long)]
        output_dir: Option<PathBuf>,

        /// `--compact`, parsed into the layout it selects.
        #[command(flatten)]
        layout: Flag<talkbank_transform::JsonLayout>,

        /// `--rebuild`, parsed into the rebuild it selects.
        #[command(flatten)]
        refresh: Flag<crate::commands::json::JsonRefresh>,

        /// `--prune`, parsed into what happens to orphaned JSON.
        #[command(flatten)]
        orphans: Flag<crate::commands::json::OrphanJson>,

        /// Number of parallel workers for directory mode (default: number of
        /// CPUs). At least 1: `--jobs 0` is a usage error.
        #[arg(short, long)]
        jobs: Option<std::num::NonZeroUsize>,

        /// `--skip-validation` and `--skip-alignment`, parsed into one
        /// check level.
        #[command(flatten)]
        checks: ToJsonCheckArgs,

        /// `--skip-schema-validation`, parsed into the schema policy.
        #[command(flatten)]
        schema: Flag<talkbank_transform::JsonSchemaPolicy>,
    } => Shape::Leaf(Contract::ToJson),


    /// Convert JSON file to CHAT
    #[command(long_about = "Convert a JSON file back to CHAT format.\n\n\
        The input should conform to the TalkBank CHAT JSON Schema:\n\
        https://talkbank.org/schemas/v0.1/chat-file.json\n\n\
        Use `chatter schema` to print the full schema.")]
    FromJson {
        /// Input JSON file path
        input: PathBuf,

        /// Output CHAT file path (if not specified, prints to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    } => Shape::Leaf(Contract::FromJson),
}
}
command_tree! {
pub enum InspectCommands {

    /// Show alignment visualization for debugging
    Alignment {
        /// Input CHAT file path
        input: PathBuf,

        /// Show alignment for specific tier types (mor, gra, pho, sin)
        /// If not specified, shows all available alignments
        #[arg(short, long, value_enum)]
        tier: Option<AlignmentTier>,

        /// `-c`/`--compact`, parsed into the view it selects.
        #[command(flatten)]
        view: Flag<crate::commands::alignment::AlignmentView>,
    } => Shape::Leaf(Contract::Alignment),


    /// Show cleaned text for each word in utterances (debugging aid)
    Words {
        /// Input CHAT file path
        path: PathBuf,

        /// Only show words where raw text differs from cleaned text
        #[arg(long)]
        diff_only: bool,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    } => Shape::Leaf(Contract::Words),
    /// Analyze CA overlap markers (⌈⌉⌊⌋): pairing, temporal consistency, orphans
    Overlaps {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Write a new JSON-lines report (one object per file with overlaps).
        /// Refuses existing paths. Failed runs may retain a partial report.
        #[arg(long, value_name = "PATH")]
        database: Option<PathBuf>,
    } => Shape::Leaf(Contract::Overlaps),

    /// Audit linker and special terminator usage across a corpus
    ///
    /// Analyzes cross-utterance pairing correctness for linkers (+<, ++, +^,
    /// +", +,, +≋, +≈) and special terminators (+..., +/., +//., +"/.etc.).
    /// Reports frequency tables, pairing violations, orphaned terminators,
    /// and +< overlap block patterns.
    Linkers {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Write per-anomaly JSON lines to a new file, refusing existing paths.
        /// Records carry an utterance index, context and a suggested fix.
        /// Failed runs may retain a partial report; suggestions require review.
        #[arg(long, value_name = "PATH")]
        anomalies: Option<PathBuf>,
    } => Shape::Leaf(Contract::Linkers),


}
}

command_tree! {
pub enum ExperimentalCommands {
    /// Assign speaker identities or inspect diarization.
    Speakers { #[command(subcommand)] command: super::speaker_commands::SpeakerCommands } => Shape::Namespace(super::speaker_commands::SpeakerCommands::shapes),
    /// Resolve pending decisions using an explicit source.
    Review { #[command(subcommand)] command: super::speaker_commands::ReviewCommands } => Shape::Namespace(super::speaker_commands::ReviewCommands::shapes),
    /// Preview specialist transcript transformations.
    Transform { #[command(subcommand)] command: DebugCommands } => Shape::Namespace(DebugCommands::shapes),

    /// Update chatter to the latest release
    #[command(
        long_about = "Update chatter to the latest release (experimental).\n\n\
        Self-updates in place: checks GitHub Releases and replaces the running \
        `chatter` binary with the newest release. The update happens in-process \
        (no separate updater program).\n\n\
        It works for installs from the official chatter installer, which records \
        the metadata the updater needs. If you installed chatter via a package \
        manager or from source, update the same way you installed it. The \
        self-update facility is experimental."
    )]
    Update => Shape::Leaf(Contract::Update),
}
}

command_tree! {
pub enum CheckCommands {
    /// List diagnostics and their enforcement status.
    List => Shape::Leaf(Contract::Checks),
    /// Explain one diagnostic from the generated registry.
    Explain {
        #[arg(value_parser = crate::commands::error_codes::parse_error_code)]
        code: talkbank_model::ErrorCode,
    } => Shape::Leaf(Contract::Checks),
}
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;

command_tree! {
pub enum DiscoveryCommands {
    /// List every executable command; JSON is schema chatter.commands.v1.
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    } => Shape::Leaf(Contract::Discovery),
    /// Show a command or namespace, including options and effects.
    Show {
        #[arg(required = true)]
        path: Vec<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    } => Shape::Leaf(Contract::Discovery),
}
}
