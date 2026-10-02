//! Shared validation runner for both CLI and GUI
//!
//! This module provides a streaming validation system using channels.
//! Events stream as they happen, enabling real-time progress and error display.
//! Supports stopping early: by the caller, through a [`Canceller`], or by
//! the run itself when its [`ErrorLimit`] is reached.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! ## Architecture
//!
//! A file's result is one `FileComplete` event: its status, how it used the
//! cache, and the diagnostics it showed, all collected before the event is
//! sent. A consumer therefore renders each file once (one JSON record, one
//! TUI entry).
//!
//! The event channel is unbounded, so a worker never waits on a slow
//! consumer. Memory is therefore bounded by the files in flight plus the
//! events the consumer has not yet taken, and an event for a file with
//! diagnostics carries the file's source text (the diagnostics point into
//! it): a consumer that stops reading without dropping the stream holds
//! every such file's text until it reads on or drops the receiver, which
//! also ends the run at the next file.

mod cancel;
mod config;
pub mod roundtrip;
mod runner;
#[cfg(test)]
mod tests;
mod types;
mod worker;

// Re-export public API. CacheOutcome and ValidationCache live in
// `talkbank-cache`; re-exported here for convenience to existing consumers.
pub use cancel::{CancelReason, Canceller, ErrorLimit};
pub use config::{
    CacheIdentityMismatch, ParserKind, RoundtripCheck, RunCache, ValidationConfig, ValidationRun,
};
// Moved to the crate root (`crate::paths`) so it survives builds with the
// `validation-runner` feature off; re-exported here for downstream code that
// imported it from this module.
pub use crate::paths::is_chat_transcript_path;
pub use runner::{
    validate_arguments_streaming, validate_directory_streaming, validate_files_streaming,
};
pub use talkbank_cache::{
    CacheLookup, CacheOutcome, ContentHash, RoundtripOutcome, ValidationCache, VerdictReader,
};
pub use types::{
    AbortReason, CacheUse, CompleteStats, FailedAttempt, FileCompleteEvent, FileDiagnostics,
    FileStatus, InvalidDiagnostics, LossCause, PartialStats, RoundtripVerdict, RunEnding,
    ShownDiagnostics, ValidationEvent, ValidationStatsSnapshot, WorkerFault, WorkerFaults,
};
