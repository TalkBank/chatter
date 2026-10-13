//! Data models for validation TUI.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::PathBuf;
use std::sync::Arc;
use talkbank_model::SourceDiagnostic;

/// A file the TUI lists: one with diagnostics, one that failed without
/// any (it could not be read, its roundtrip failed), or both (a tool
/// failure keeps the diagnostics of its attempt).
#[derive(Debug, Clone)]
pub struct FileErrors {
    /// Path to the validated CHAT file.
    pub path: PathBuf,
    /// Validation and parse errors associated with this file.
    pub errors: Vec<SourceDiagnostic>,
    /// Producing source snapshot, also used for CLAN line adjustment.
    pub source: Arc<str>,
    /// How the file failed beyond its diagnostics, if it did.
    pub failure: Option<FileFailure>,
}

/// How a file failed beyond its diagnostics: the statuses that fail a run
/// without being diagnostics in a file, so a run over them never looks clean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileFailure {
    /// The file could not be read.
    Unreadable(String),
    /// The file validated but did not write back byte for byte.
    RoundtripFailed(String),
    /// The tool failed; the file's validity is undetermined.
    ToolFailed(String),
}

impl std::fmt::Display for FileFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(why) => write!(f, "Could not read this file: {why}"),
            Self::RoundtripFailed(why) => write!(f, "Roundtrip failed: {why}"),
            Self::ToolFailed(why) => write!(f, "Internal failure (validity undetermined): {why}"),
        }
    }
}
