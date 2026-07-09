use clap::Subcommand;
use std::path::PathBuf;

use talkbank_transform::sanity_scan::SanityScanThreshold;

use super::cache_commands::CacheCommands;
use super::cli_types::{AlignmentTier, OutputFormat, ParserBackend};
use super::debug_commands::DebugCommands;
use super::judgment_args::JudgmentArgs;

/// Top-level `talkbank` subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Validate CHAT file(s)
    Validate {
        /// Path(s) to CHAT file(s) or directory(ies).
        ///
        /// Not required when `--list-checks` is supplied, since that mode
        /// exits before reading any files.
        #[arg(required_unless_present = "list_checks")]
        path: Vec<PathBuf>,

        /// Print every validation check and its Active/Planned status, then exit.
        ///
        /// Does not read any files. The list is derived from the
        /// `ErrorCode` enum and a hard-coded Planned list that mirrors
        /// `spec/errors/*.md` statuses.
        #[arg(
            long,
            help = "List all validation checks with Active/Planned status, then exit"
        )]
        list_checks: bool,

        /// Output format: text (default) or json
        #[arg(short, long, value_enum, default_value_t = OutputFormat::Text, help = "Validation output style (text|json)")]
        format: OutputFormat,

        /// Skip tier alignment (validation includes alignment by default)
        #[arg(
            long = "skip-alignment",
            help = "Disable dependent tier alignment checks (alignment is on by default)"
        )]
        skip_alignment: bool,

        /// Force fresh validation, clearing and updating cache
        #[arg(
            long,
            help = "Force fresh validation (clears and updates cache for specified path)"
        )]
        force: bool,

        /// Number of parallel jobs (default: number of CPUs)
        #[arg(short, long)]
        jobs: Option<usize>,

        /// Suppress success output (errors still print)
        #[arg(long, help = "Quiet mode (only emit errors, rely on exit codes)")]
        quiet: bool,

        /// Stop after this many errors (across all files)
        #[arg(long)]
        max_errors: Option<usize>,

        /// Run roundtrip test (serialize → re-parse → compare) after validation.
        /// Tests serialization idempotency. Developer tool for parser/serializer testing.
        #[arg(long, help = "Test serialization idempotency (developer tool)")]
        roundtrip: bool,

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
            value_name = "OUTPUT_FILE"
        )]
        audit: Option<PathBuf>,

        /// Enable strict cross-utterance linker validation (E351-E355).
        ///
        /// Checks that self-completion (+,) and other-completion (++)
        /// linkers are paired with the correct preceding terminators
        /// (+/. and +... respectively). Disabled by default because
        /// many existing corpora do not follow these strict conventions.
        #[arg(
            long = "strict-linkers",
            help = "Enable strict linker pairing validation (E351-E355)"
        )]
        strict_linkers: bool,

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
        #[arg(
            long,
            value_delimiter = ',',
            help = "Suppress error codes or groups (e.g., --suppress xphon,E726)"
        )]
        suppress: Vec<String>,

        /// Deprecated no-op. Phon `%x` validation now runs by default, so this
        /// flag is unnecessary; passing it prints a deprecation note. To silence
        /// Phon `%x` diagnostics, use `--suppress xphon` instead.
        #[arg(
            long = "check-xphon",
            hide = true,
            help = "Deprecated: Phon %x validation runs by default (no-op)"
        )]
        check_xphon: bool,
    },

    /// EXPERIMENTAL. Merge two CHAT transcripts of the same media into one.
    ///
    /// Speakers listed in `--retain` come from FILE1 (byte-preserved);
    /// all other speakers come from FILE2. Utterances are interleaved
    /// by start-time. See `book/src/chatter/user-guide/merge.md` for
    /// the full contract.
    Merge {
        /// First CHAT file. Retain-set speakers' utterances come
        /// from here.
        file1: PathBuf,

        /// Second CHAT file. All other speakers' utterances come
        /// from here.
        file2: PathBuf,

        /// Comma-separated speaker codes whose utterances come from
        /// FILE1 (e.g. `CHI` or `CHI,SI2`).
        #[arg(long, value_delimiter = ',')]
        retain: Vec<String>,

        /// Output path (prints to stdout if omitted).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// EXPERIMENTAL. Assign CHAT-conformant speaker codes to an anonymously-labeled
    /// CHAT file. Supports explicit-mapping mode (via `--mapping`) and
    /// reference mode (via `--reference` + `--anchor` +
    /// `--inserted-role`). See `book/src/chatter/user-guide/speaker-id.md`.
    SpeakerId {
        /// Input CHAT file with anonymous speaker codes (e.g. PAR0,
        /// PAR1, …) to be relabeled.
        input: PathBuf,

        /// Explicit-mapping specification, comma-separated
        /// assignments of the form `OLD=drop` or `OLD=CODE:ROLE`.
        /// Every speaker in the input must be named in the mapping.
        /// Mutually exclusive with `--reference`.
        #[arg(long, conflicts_with = "reference")]
        mapping: Option<String>,

        /// Reference-mode: path to a CHAT file containing the
        /// authoritative anchor speaker. The donor speaker whose
        /// content best matches the anchor by multiset-Jaccard text
        /// similarity is dropped; remaining donor speakers are
        /// renamed per `--inserted-role`.
        #[arg(long, requires = "anchor", requires = "inserted_role")]
        reference: Option<PathBuf>,

        /// Reference-mode: speaker code in `--reference` whose
        /// content represents the donor speaker to drop.
        #[arg(long)]
        anchor: Option<String>,

        /// Reference-mode: role spec for non-anchor donor speakers,
        /// formatted `CODE:ROLE` (e.g. `INV:Investigator`).
        #[arg(long = "inserted-role")]
        inserted_role: Option<String>,

        /// Reference-mode: minimum winner→runner-up Jaccard margin
        /// for the auto-decision to stand. Default 2.0×; below
        /// threshold the command refuses (exit 4) and prints
        /// per-speaker scores to stderr.
        #[arg(long, default_value_t = 2.0)]
        confidence_threshold: f64,

        /// Reference-mode: when the auto-decide succeeds, append the
        /// decision to this override file (created if absent). The
        /// file is the durable audit trail of the batch run, see
        /// `book/src/chatter/integrating/merge-overrides.md`.
        #[arg(long = "write-override")]
        write_override: Option<PathBuf>,

        /// Reference-mode: when the auto-decide refuses on low
        /// confidence, append a pending-adjudication entry to this
        /// file (created if absent). The orchestrator hands the
        /// resulting file to `chatter adjudicate` for human review.
        #[arg(long = "write-pending")]
        write_pending: Option<PathBuf>,

        /// Override-file mode: replay a prior adjudication recorded
        /// in this override file. Mutually exclusive with `--mapping`
        /// and `--reference`. Requires `--session-id` (or defaults
        /// to the input file's basename stem).
        #[arg(
            long = "override-file",
            conflicts_with = "mapping",
            conflicts_with = "reference"
        )]
        override_file: Option<PathBuf>,

        /// Override-file mode: the session ID whose entry to apply
        /// from `--override-file`. Defaults to the input file's
        /// basename stem when omitted.
        #[arg(long = "session-id")]
        session_id: Option<String>,

        /// Judgment engine + LLM connection + session context (shared
        /// with `pipeline` and `batch`).
        #[command(flatten)]
        judgment: JudgmentArgs,

        /// Output path (prints to stdout if omitted).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// EXPERIMENTAL. Re-attribute utterance speakers from an external
    /// diarization. Each utterance with a media time bullet is assigned
    /// to the diarization track with the greatest time overlap; the
    /// words are kept byte-stable. Repairs transcripts whose ASR
    /// under-counted or mixed speakers. See
    /// `book/src/chatter/user-guide/rediarize.md`.
    Rediarize {
        /// Input CHAT file whose utterances carry time bullets.
        input: PathBuf,

        /// Turns JSON from the external diarizer:
        /// `{"source": "...", "turns": [{"track": "PAR0",
        /// "start_ms": 0, "end_ms": 1200}, ...]}`. Format contract:
        /// the rediarize book page.
        #[arg(long)]
        turns: PathBuf,

        /// Output path (prints to stdout if omitted).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// EXPERIMENTAL. Batch driver: loop `chatter pipeline` over matched donor /
    /// reference pairs in two directories. Files match by basename;
    /// donors without a matching reference are warned and skipped.
    /// Low-confidence refusals are aggregated (with optional
    /// pending-file write) but don't abort the batch.
    Batch {
        /// Directory of donor CHAT files (ASR output).
        donor_dir: PathBuf,

        /// Directory of reference CHAT files. Reference for donor
        /// `X.cha` is `reference_dir/X.cha`.
        reference_dir: PathBuf,

        /// Anchor speaker code in each reference file (typically
        /// `CHI`).
        #[arg(long)]
        anchor: String,

        /// Inserted-role spec for the donor's non-anchor speakers
        /// (e.g., `INV:Investigator`).
        #[arg(long = "inserted-role")]
        inserted_role: String,

        /// Speaker codes whose utterances come from the reference in
        /// each merge step.
        #[arg(long, value_delimiter = ',')]
        retain: Vec<String>,

        /// Minimum winner→runner-up Jaccard margin (default 2.0×).
        #[arg(long, default_value_t = 2.0)]
        confidence_threshold: f64,

        /// Aggregate low-confidence refusals into this pending file.
        /// One operator run of `chatter adjudicate` resolves them all.
        #[arg(long = "write-pending")]
        write_pending: Option<PathBuf>,

        /// Override-file path threaded to every per-session
        /// `chatter pipeline` invocation. Sessions that have an
        /// entry are processed via override-file replay; others
        /// fall through to reference mode. Pass-2 workflow.
        #[arg(long = "override-file")]
        override_file: Option<PathBuf>,

        /// Audit-trail destination for clean-winner auto-decisions.
        /// Pass-1 workflow: each session that succeeds via reference
        /// mode appends its mapping + scores + margin to this file
        /// with `mode = "auto"`. Required for the post-merge
        /// `chatter sanity-scan` pass.
        #[arg(long = "write-override")]
        write_override: Option<PathBuf>,

        /// Run the post-merge sanity scan after the per-session
        /// pipeline loop completes. Requires `--write-override` (the
        /// scan reads pass-1 auto-decisions) and `--write-pending`
        /// (flagged sessions get appended). Exit code 4 fires when
        /// the scan flags any session.
        #[arg(long = "sanity-scan", requires_all = ["write_override", "write_pending"])]
        sanity_scan: bool,

        /// Ratio threshold for the sanity-scan mean-word-count
        /// heuristic. Only consulted when `--sanity-scan` is set.
        #[arg(long = "sanity-scan-threshold", default_value_t = SanityScanThreshold::DEFAULT.0)]
        sanity_scan_threshold: f64,

        /// Skip donors whose merged output already exists in the
        /// output directory. Lets the operator resume an interrupted
        /// batch or add new donors without redoing finished work.
        /// Default: re-process every matched donor.
        #[arg(long = "skip-existing")]
        skip_existing: bool,

        /// Judgment engine + LLM connection + session context (shared
        /// with `speaker-id` and `pipeline`; threaded to every
        /// per-session `chatter pipeline` subprocess).
        #[command(flatten)]
        judgment: JudgmentArgs,

        /// Output directory for merged files (created if absent).
        #[arg(short, long)]
        output: PathBuf,
    },

    /// EXPERIMENTAL. End-to-end per-session shortcut: run speaker-id in reference
    /// mode to relabel an anonymous donor, then merge the relabeled
    /// donor with the reference. One CLI invocation instead of two
    /// for the common case.
    Pipeline {
        /// Donor CHAT file with anonymous speaker codes (the ASR
        /// output).
        donor: PathBuf,

        /// Reference CHAT file carrying the authoritative anchor
        /// speaker (typically the hand-coded child transcript).
        reference: PathBuf,

        /// Speaker code in the reference whose content the algorithm
        /// matches against to identify the donor's anchor speaker.
        #[arg(long)]
        anchor: String,

        /// Role spec for the donor's non-anchor speakers,
        /// `CODE:ROLE` (e.g. `INV:Investigator`).
        #[arg(long = "inserted-role")]
        inserted_role: String,

        /// Speaker codes whose utterances come from the reference in
        /// the final merge step. Typically the same as `--anchor`.
        #[arg(long, value_delimiter = ',')]
        retain: Vec<String>,

        /// Minimum winner→runner-up Jaccard margin (default 2.0×).
        #[arg(long, default_value_t = 2.0)]
        confidence_threshold: f64,

        /// On low-confidence refusal, append a pending-adjudication
        /// entry to this file (created if absent). Exit code 4 still
        /// fires.
        #[arg(long = "write-pending")]
        write_pending: Option<PathBuf>,

        /// Override-file path. If the file contains an entry for
        /// this session (basename-stem of `donor`), the pipeline
        /// uses the recorded decision via override-file replay
        /// mode instead of running reference mode. Sessions
        /// without an entry fall through to reference mode. The
        /// same `chatter pipeline` command works for both pass 1
        /// (no entries yet) and pass 2 (entries from prior
        /// adjudication).
        #[arg(long = "override-file")]
        override_file: Option<PathBuf>,

        /// Audit-trail destination for the clean-winner
        /// auto-decision. When set and reference mode produces a
        /// merge, the pipeline appends a `mode = "auto"` entry for
        /// this session to the named file.
        #[arg(long = "write-override")]
        write_override: Option<PathBuf>,

        /// Judgment engine + LLM connection + session context (shared
        /// with `speaker-id` and `batch`).
        #[command(flatten)]
        judgment: JudgmentArgs,

        /// Output path for the merged CHAT file (required).
        #[arg(short, long)]
        output: PathBuf,
    },

    /// EXPERIMENTAL. Adjudicate pending speaker-id (and future) decisions and
    /// write the resolved entries to an override file. See
    /// `book/src/architecture/adjudication-workflow.md`.
    Adjudicate {
        /// Path to the pending-adjudications TOML file. Rewritten
        /// on success to remove the resolved entries.
        pending: PathBuf,

        /// Override file to append resolved decisions to. Created
        /// if absent.
        #[arg(long = "override-file")]
        override_file: PathBuf,

        /// Pre-canned operator decisions in TOML form, see the
        /// adjudication workflow doc for the format. Mutually
        /// exclusive with `--interactive`.
        #[arg(long, conflicts_with = "interactive")]
        scripted: Option<PathBuf>,

        /// Prompt the operator interactively (one stdin line per
        /// pending entry). Decisions: `accept` (take the suggestion),
        /// `choose SPK:CODE:TAG ...` (pick a mapping), or `override
        /// SPK:CODE:TAG ... SPK=action ...` (mapping plus per-speaker
        /// actions), each with an optional trailing note.
        #[arg(long)]
        interactive: bool,

        /// Operator identifier recorded in the override entries.
        /// Defaults to `$USER` (`"unknown"` if unset).
        #[arg(long)]
        operator: Option<String>,
    },

    /// EXPERIMENTAL. Post-merge sanity scan: detect sessions whose pass-1 auto-
    /// decision looks suspicious by an out-of-band heuristic (mean
    /// utterance word count asymmetry between anchor and inserted
    /// speakers). Flagged sessions become
    /// `sanity-scan-misclassification` pending entries the operator
    /// resolves via `chatter adjudicate`. See
    /// `book/src/architecture/adjudication-workflow.md`.
    SanityScan {
        /// Directory of merged CHAT files (produced by `chatter
        /// batch` or `chatter pipeline`). Each file's basename stem
        /// is treated as its session ID.
        merged_dir: PathBuf,

        /// Override file from pass-1. Sessions are matched by
        /// session ID; auto-decided entries are eligible for
        /// scanning, explicit-mode entries are skipped (operator
        /// already signed off).
        #[arg(long = "override-file")]
        override_file: PathBuf,

        /// Anchor speaker code in the merged files (typically
        /// `CHI`).
        #[arg(long)]
        anchor: String,

        /// Ratio threshold for the mean-word-count heuristic
        /// (default 1.5×). A session is flagged when
        /// `anchor_mean >= inserted_mean * threshold`.
        #[arg(long, default_value_t = 1.5)]
        threshold: f64,

        /// Pending-adjudications file to append flagged sessions
        /// to. Created if absent. Required, flagged sessions
        /// without a pending file would be discarded.
        #[arg(long = "write-pending")]
        write_pending: PathBuf,
    },

    /// Normalize CHAT file to canonical format
    Normalize {
        /// Input CHAT file path
        input: PathBuf,

        /// Output CHAT file path (if not specified, prints to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Validate (includes alignment) before normalization
        #[arg(long, help = "Validate and check alignment before writing output")]
        validate: bool,

        /// Skip alignment when validating
        #[arg(
            long = "skip-alignment",
            help = "Skip alignment checks when --validate is supplied"
        )]
        skip_alignment: bool,
    },

    /// Convert CHAT file to JSON
    #[command(long_about = "Convert CHAT transcript(s) to JSON.\n\n\
        Output conforms to the TalkBank CHAT JSON Schema:\n\
        https://talkbank.org/schemas/v0.1/chat-file.json\n\n\
        Single file: prints JSON to stdout or writes to --output.\n\
        Directory: requires --output-dir. Walks recursively, preserving structure.\n\
        Incremental by default: skips files whose JSON is already up-to-date (mtime check).\n\
        Use --force to rebuild all. Use --prune to remove orphaned .json files.")]
    ToJson {
        /// Input CHAT file or directory path
        input: PathBuf,

        /// Output JSON file path (single-file mode; prints to stdout if omitted)
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,

        /// Output directory (directory mode; preserves relative structure)
        #[arg(long)]
        output_dir: Option<PathBuf>,

        /// Compact (minified) JSON output instead of pretty-printed
        #[arg(long)]
        compact: bool,

        /// Force full rebuild (ignore mtime, reconvert all files)
        #[arg(long)]
        force: bool,

        /// Remove .json files with no matching .cha source (directory mode)
        #[arg(long)]
        prune: bool,

        /// Number of parallel workers for directory mode
        #[arg(short, long)]
        jobs: Option<usize>,

        /// (Deprecated) Validation is now on by default. This flag is ignored.
        #[arg(long, hide = true)]
        validate: bool,

        /// (Deprecated) Alignment is now on by default. Use --skip-alignment to disable.
        #[arg(short, long, hide = true)]
        alignment: bool,

        /// Skip tier alignment checks
        #[arg(
            long = "skip-alignment",
            help = "Disable tier alignment validation during conversion"
        )]
        skip_alignment: bool,

        /// Skip data model validation (parse only, always produce JSON)
        #[arg(
            long = "skip-validation",
            help = "Skip validation of the CHAT data model (parse only, no alignment)"
        )]
        skip_validation: bool,

        /// Skip validation against the CHAT JSON Schema
        #[arg(
            long,
            help = "Skip validation against the CHAT JSON Schema \
            (https://talkbank.org/schemas/v0.1/chat-file.json). \
            Useful for faster output when you trust the data model."
        )]
        skip_schema_validation: bool,
    },

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
    },

    /// Export CHAT file to TalkBank XML
    #[command(long_about = "Export one CHAT transcript to TalkBank XML.\n\n\
        This uses the Rust XML writer in talkbank-transform and validates the \
        input transcript before emitting XML.\n\n\
        XML ingest is not implemented, so there is no `from-xml` command.")]
    ToXml {
        /// Input CHAT file path
        input: PathBuf,

        /// Output XML file path (if not specified, prints to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Disable tier alignment validation during export
        #[arg(long = "skip-alignment")]
        skip_alignment: bool,
    },

    /// Show alignment visualization for debugging
    ShowAlignment {
        /// Input CHAT file path
        input: PathBuf,

        /// Show alignment for specific tier types (mor, gra, pho, sin)
        /// If not specified, shows all available alignments
        #[arg(short, long, value_enum)]
        tier: Option<AlignmentTier>,

        /// Compact output (one line per alignment)
        #[arg(short, long)]
        compact: bool,
    },

    /// Watch CHAT file(s) for changes and continuously validate
    Watch {
        /// Path to CHAT file or directory to watch
        path: PathBuf,

        /// Skip tier alignment checks
        #[arg(long)]
        skip_alignment: bool,

        /// Clear terminal before each validation run
        #[arg(short, long)]
        clear: bool,
    },

    /// Lint CHAT file(s) and optionally auto-fix issues
    Lint {
        /// Path to CHAT file or directory
        path: PathBuf,

        /// Automatically apply fixes
        #[arg(long)]
        fix: bool,

        /// Show what would be fixed without modifying files
        #[arg(long, requires = "fix")]
        dry_run: bool,

        /// Skip tier alignment checks
        #[arg(long)]
        skip_alignment: bool,
    },

    /// Show cleaned text for each word in utterances (debugging aid)
    Clean {
        /// Input CHAT file path
        path: PathBuf,

        /// Only show words where raw text differs from cleaned text
        #[arg(long)]
        diff_only: bool,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    /// Create a new minimal valid CHAT file
    NewFile {
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
    },

    /// Cache management operations
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },

    /// Print the CHAT JSON Schema
    #[command(long_about = "Print the CHAT JSON Schema to stdout.\n\n\
        The schema describes the structure of CHAT transcripts serialized to JSON \
        by `chatter to-json`. It is auto-generated from the Rust data model \
        and conforms to JSON Schema 2020-12.\n\n\
        Canonical URL: https://talkbank.org/schemas/v0.1/chat-file.json")]
    Schema {
        /// Print only the canonical schema URL instead of the full schema
        #[arg(
            long,
            help = "Print only the canonical URL (https://talkbank.org/schemas/v0.1/chat-file.json)"
        )]
        url: bool,
    },

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
    Update,

    /// Developer/debugging tools for CHAT analysis
    #[command(about = "Developer tools for inspecting and debugging CHAT files")]
    Debug {
        #[command(subcommand)]
        command: DebugCommands,
    },
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
