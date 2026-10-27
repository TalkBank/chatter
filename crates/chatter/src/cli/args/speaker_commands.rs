//! Mode-specific speaker and review requests. Irrelevant options cannot cross modes.

use crate::cli::catalog::{Contract, Shape, command_tree};
use std::path::PathBuf;
use talkbank_transform::sanity_scan::SanityScanThreshold;
use talkbank_transform::speaker_id::ConfidenceThreshold;

command_tree! {
pub enum SpeakerCommands {
    /// Relabel speakers using an explicit mapping; emit CHAT to stdout or --output.
    Map {
        input: PathBuf,
        #[arg(long)]
        mapping: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    } => Shape::Leaf(Contract::Map),
    /// Match donor speakers against a reference; refuse insufficient confidence.
    Match {
        input: PathBuf,
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        anchor: String,
        #[arg(long)]
        inserted_role: String,
        #[arg(long, default_value = "2.0")]
        confidence_threshold: ConfidenceThreshold,
        #[arg(long)]
        write_match_report: Option<PathBuf>,
        #[arg(long)]
        write_override: Option<PathBuf>,
        #[arg(long)]
        write_pending: Option<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    } => Shape::Leaf(Contract::Match),
    /// Replay a recorded speaker decision for an explicit session identity.
    Replay {
        input: PathBuf,
        #[arg(long)]
        override_file: PathBuf,
        #[arg(long)]
        session_id: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    } => Shape::Leaf(Contract::Replay),
    /// Request a review-gated suggestion from an explicitly configured LLM.
    Suggest {
        input: PathBuf,
        #[arg(long)]
        anchor: Option<String>,
        #[arg(long)]
        write_pending: PathBuf,
        #[command(flatten)]
        provider: super::judgment_args::JudgmentArgs,
    } => Shape::Leaf(Contract::Suggest),

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

        /// Also write a machine-readable outcome summary (counts plus
        /// the full flagged list) as JSON to this path. Shape contract:
        /// the rediarize book page.
        #[arg(long)]
        summary_json: Option<PathBuf>,

        /// Report an utterance as CONTESTED when the runner-up track holds at
        /// least this share (0.0 to 1.0) of its overlapped time.
        ///
        /// NO DEFAULT, deliberately: omit it and nothing is reported as
        /// contested. What share makes an utterance genuinely mixed has not
        /// been measured against human listening, and a default here would
        /// hand every user a number wearing this tool's authority. Contested
        /// utterances are still reattributed to their winning track; this
        /// makes the mixed population visible, it does not change placement.
        #[arg(long, value_name = "SHARE")]
        contested_at: Option<f64>,
    } => Shape::Leaf(Contract::Rediarize),

    /// EXPERIMENTAL. Post-merge sanity scan: detect sessions whose pass-1 auto-
    /// decision looks suspicious by an out-of-band heuristic (mean
    /// utterance word count asymmetry between anchor and inserted
    /// speakers). Flagged sessions become
    /// `sanity-scan-misclassification` pending entries the operator
    /// resolves via `chatter experimental review`. Missing selected transcripts
    /// fail the scan; the pending destination must be distinct from all sources.
    Scan {
        /// Directory of CHAT files to inspect. Each file's basename stem
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
        /// (finite and positive; default 1.5×). A session is flagged when
        /// `anchor_mean >= inserted_mean * threshold`.
        #[arg(long, default_value_t = SanityScanThreshold::DEFAULT)]
        threshold: SanityScanThreshold,

        /// Pending-adjudications file to append flagged sessions
        /// to. Created if absent. Required, flagged sessions
        /// without a pending file would be discarded.
        #[arg(long = "write-pending")]
        write_pending: PathBuf,
    } => Shape::Leaf(Contract::Scan),
}
}

command_tree! {
pub enum ReviewCommands {
    /// Resolve pending decisions from a TOML script and publish the result.
    Scripted {
        pending: PathBuf,
        #[arg(long)]
        override_file: PathBuf,
        #[arg(long)]
        script: PathBuf,
        #[arg(long)]
        operator: String,
    } => Shape::Leaf(Contract::ScriptedReview),
    /// Resolve pending decisions at a terminal and publish the result.
    Interactive {
        pending: PathBuf,
        #[arg(long)]
        override_file: PathBuf,
        #[arg(long)]
        operator: String,
    } => Shape::Leaf(Contract::InteractiveReview),
}
}
