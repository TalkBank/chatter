//! Validation commands for CHAT files.
//!
//! `run_validate_command` is the landing point for `chatter validate`: it
//! hands every invocation, one file or a corpus, to the one streaming
//! runtime (`validate_paths_parallel`) and decides the exit status. The module also holds the cache seam
//! (`cache`) and the audit JSONL writer (`audit_reporter`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod audit_reporter;
pub(crate) mod cache;

use std::path::PathBuf;

use super::validate_parallel::{
    ValidateDirectoryOptions, ValidationOutcome, validate_paths_parallel,
};

/// Execute one top-level `chatter validate` invocation: every input shape
/// (one file, many, directories, or a mix) goes through the one streaming
/// runtime, and this is the one place `chatter validate` decides its exit
/// status, from the run's outcome.
pub fn run_validate_command(
    paths: crate::commands::inputs::NonEmpty<PathBuf>,
    options: ValidateDirectoryOptions,
) {
    // The summary's label: the first argument as typed (the directory, for
    // a one-directory run).
    let summary_label = paths.first().clone();
    let outcome = validate_paths_parallel(paths.into_iter().collect(), summary_label, options);
    if let ValidationOutcome::AuditFileUnwritable { path, error } = &outcome {
        eprintln!(
            "Error: cannot create the audit file {}: {error}",
            path.display()
        );
    }
    if outcome.failed() {
        std::process::exit(1);
    }
}
