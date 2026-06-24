//! `chatter debug` subcommands, internal debugging / audit /
//! sanitization tools.

use clap::Subcommand;
use std::path::PathBuf;

use super::cli_types::OutputFormat;

/// Internal debugging subcommands under `chatter debug`.
#[derive(Subcommand)]
pub enum DebugCommands {
    /// Rewrite whole-utterance runs of `@s` into utterance precodes (`[- LANG]`).
    ///
    /// Walks `.cha` files under the given paths, detects the same E255
    /// whole-utterance language-switch pattern that `chatter validate` rejects,
    /// and rewrites qualifying utterances in place. Files that need no changes
    /// are left untouched.
    FixS {
        /// Path to CHAT file(s) or directory trees to rewrite in place.
        path: Vec<PathBuf>,
    },

    /// Join dangling-retrace utterances (E370) with their successor.
    ///
    /// Repairs the unambiguous subset of E370 ("dangling retrace"): an
    /// utterance whose last main-tier content is a retrace marker with nothing
    /// after it, followed by a same-speaker utterance. By default only
    /// partial-repetition retraces (`[/]`) are joined, and only when the
    /// successor's leading words repeat the retraced material. Passing
    /// `--include-corrections` additionally joins correction retraces
    /// (`[//]`/`[///]`/`[/-]`) using same-speaker presence alone as the gate
    /// (corrections replace rather than repeat the retraced material, so no
    /// prefix check is applied). When either side carried dependent tiers
    /// (`%mor`/`%gra`/...), those tiers are dropped on the joined utterance
    /// and reported as needing re-morphotag.
    JoinRetrace {
        /// Path to CHAT file(s) or directory trees to repair in place.
        path: Vec<PathBuf>,

        /// Show what would be joined without modifying any files.
        #[arg(long)]
        dry_run: bool,

        /// Also join correction retraces (`[//]` Full, `[///]` Multiple,
        /// `[/-]` Reformulation). By default only partial-repetition retraces
        /// (`[/]`) are joined. With this flag, any dangling correction retrace
        /// followed by a same-speaker utterance is joined regardless of whether
        /// the successor repeats the retraced material. Use `--dry-run` first
        /// to review every proposed correction-join before writing.
        #[arg(long)]
        include_corrections: bool,
    },

    /// Analyze CA overlap markers (⌈⌉⌊⌋): pairing, temporal consistency, orphans
    OverlapAudit {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Output format
        #[arg(short, long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Write JSON lines database to this file (one JSON object per file).
        /// Enables persistent overlap data for downstream analysis.
        #[arg(long, value_name = "PATH")]
        database: Option<PathBuf>,
    },

    /// Audit linker and special terminator usage across a corpus
    ///
    /// Analyzes cross-utterance pairing correctness for linkers (+<, ++, +^,
    /// +", +,, +≋, +≈) and special terminators (+..., +/., +//., +"/.etc.).
    /// Reports frequency tables, pairing violations, orphaned terminators,
    /// and +< overlap block patterns.
    LinkerAudit {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Write per-anomaly JSON lines to this file. Each line is a JSON
        /// object with file, line, anomaly type, context, and suggested fix.
        #[arg(long, value_name = "PATH")]
        anomalies: Option<PathBuf>,
    },

    /// Sanitize a CHAT file for protected-corpus debugging, strip
    /// contributor lexical content while preserving structure.
    ///
    /// Replaces all word content, free-text dependent tiers, and
    /// free-text headers with placeholders / `[redacted]`, while
    /// preserving timing bullets, %wor offsets, speaker codes, POS
    /// tags, language markers, and structural counts. Intended for use
    /// before loading a protected `.cha` into LLM-assisted debugging
    /// tools. See `talkbank/docs/protected-corpus-debugging-workflow.md`.
    Sanitize {
        /// Path to a single .cha file to sanitize.
        input: PathBuf,

        /// Output path. If omitted, writes to stdout.
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}
