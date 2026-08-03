//! Domain error types for the desktop command surface.
//!
//! Every command used to return `Result<_, String>`. A `String` is the least
//! informative type that will compile: it admits any message at all, so it
//! carries no record of which failures an operation actually has, cannot be
//! matched on, loses the underlying error's source chain, and lets an unrelated
//! message be returned from the wrong place without anything noticing. It is
//! also how "no path provided" and "CLAN refused the request" came to have the
//! same type despite having nothing in common.
//!
//! Each error below names the failures of ONE operation, so a command's
//! signature states what can go wrong with it and a new failure mode has to be
//! added deliberately rather than smuggled in as another string.
//!
//! # The wire contract is deliberately unchanged
//!
//! Tauri requires a command's error to be `Serialize`, and the frontend renders
//! whatever arrives with `String(err)`. Each type here serializes as its
//! `Display` text, so the JSON crossing the IPC boundary is the same string it
//! has always been. The typing is for the Rust side, where the mistakes happen;
//! the presentation boundary is left alone on purpose, so this refactor cannot
//! change what a user sees.

use std::path::PathBuf;

use serde::{Serialize, Serializer};

/// Serialize each error as its `Display` text.
///
/// A macro rather than a blanket impl over `thiserror::Error`: a blanket impl
/// would collide with `serde`'s own impls and would silently capture types that
/// want a structured wire form later.
macro_rules! serialize_as_display {
    ($($error:ty),+ $(,)?) => {
        $(
            impl Serialize for $error {
                fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                    serializer.collect_str(self)
                }
            }
        )+
    };
}

/// Why a target could not be turned into a running validation.
///
/// These are properties of the PATH the user chose, decided before any file is
/// read, which is what separates them from anything the validator itself
/// reports (those arrive as diagnostics, not as errors).
#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("Path does not exist: {path}")]
    Missing { path: PathBuf },

    #[error("Chatter validates one .cha file or one folder at a time: {path}")]
    NotChatTranscript { path: PathBuf },

    #[error("Path is not a file or directory: {path}")]
    NotFileOrDirectory { path: PathBuf },
}

/// Why a validation run failed to START.
///
/// Distinct from anything the run reports once it is going: a run that starts
/// reports its outcome through the event stream, so this type covers only the
/// window before the first event, which is exactly the window that used to fail
/// invisibly.
#[derive(Debug, thiserror::Error)]
pub enum ValidationStartError {
    #[error("No path provided")]
    EmptyPath,

    #[error(transparent)]
    Target(#[from] TargetError),

    /// The startup sequence panicked.
    ///
    /// A variant rather than a propagated panic because a panic unwinding out of
    /// a Tauri command leaves the IPC promise unsettled forever, which the UI
    /// cannot distinguish from a slow disk. See [`crate::commands::validate`].
    #[error("The validator panicked before the run began: {message}")]
    Panicked { message: String },
}

/// Why an Open-in-CLAN request could not be completed.
#[derive(Debug, thiserror::Error)]
pub enum ClanError {
    #[error("Cannot read {path}: {source}")]
    ReadSource {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Cannot resolve the CLAN location: {0}")]
    Resolve(#[from] talkbank_model::ClanHiddenLineError),

    #[error("CLAN did not accept the request: {0}")]
    Send(#[from] send2clan::Error),
}

/// Why the bundled CLI could not be installed.
#[derive(Debug, thiserror::Error)]
pub enum InstallCliError {
    #[error("Cannot locate the app's resource directory: {source}")]
    ResourceDir {
        #[source]
        source: tauri::Error,
    },

    #[error(
        "Bundled CLI not found at {path}. Build with `cargo build --release -p chatter` first."
    )]
    NotBundled { path: PathBuf },

    #[error("Cannot remove existing {path}: {source}. Try running with sudo.")]
    RemoveExisting {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Cannot create symlink at {path}: {source}. Try running with sudo.")]
    Symlink {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Cannot determine the local application data directory")]
    NoLocalDataDir,

    #[error("Cannot install to {path}: {source}")]
    Install {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Why a file could not be revealed in the platform file manager.
#[derive(Debug, thiserror::Error)]
pub enum RevealError {
    #[error("Path does not exist: {path}")]
    Missing { path: PathBuf },

    #[error("Could not launch the file manager: {source}")]
    Launch {
        #[source]
        source: std::io::Error,
    },
}

/// Why validation results could not be exported.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Results are not valid JSON: {source}")]
    MalformedResults {
        #[source]
        source: serde_json::Error,
    },

    #[error("Cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Why an external URL was not opened.
///
/// The two refusals are a security boundary, not a formatting complaint: this
/// command hands a string to a platform opener, so anything that is not plainly
/// an `http(s)` URL is rejected rather than sanitized.
#[derive(Debug, thiserror::Error)]
pub enum OpenExternalError {
    #[error("refusing to open non-http(s) URL: {url}")]
    NotHttp { url: String },

    #[error("refusing to open URL containing whitespace or control characters: {url}")]
    Unprintable { url: String },

    #[error("Could not launch the browser: {source}")]
    Launch {
        #[source]
        source: std::io::Error,
    },
}

serialize_as_display!(
    TargetError,
    ValidationStartError,
    ClanError,
    InstallCliError,
    RevealError,
    ExportError,
    OpenExternalError,
);

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire form is the property this refactor promises not to change, so
    /// it is pinned rather than assumed. This is a serialization contract, which
    /// is one of the things a type genuinely cannot express.
    #[test]
    fn errors_cross_the_ipc_boundary_as_their_display_text() {
        let error = ValidationStartError::Target(TargetError::Missing {
            path: PathBuf::from("/nope"),
        });

        let json = serde_json::to_string(&error).expect("serialize");

        assert_eq!(json, "\"Path does not exist: /nope\"");
    }

    #[test]
    fn an_empty_path_reads_the_way_it_always_did() {
        let json = serde_json::to_string(&ValidationStartError::EmptyPath).expect("serialize");

        assert_eq!(json, "\"No path provided\"");
    }
}
