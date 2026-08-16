//! Formatting generated Rust, owned once.
//!
//! # Why a generator formats rather than emitting careful whitespace
//!
//! Without this, two processes claim authority over the same bytes. Anyone
//! running `just fmt` reflows the generated file, the drift gate then reports
//! it stale, regenerating un-reflows it, and `just fmt` reflows it again. The
//! gate and the formatter would disagree forever, each correctly.
//!
//! It is a hard failure when rustfmt is missing: a generator that skipped
//! formatting would emit a file the next `cargo fmt` immediately invalidates,
//! and the currency gate would read that as staleness, which it is not.
//!
//! # Why this module exists
//!
//! Because the doc comment that used to sit on the second copy of this function
//! said "a THIRD copy is a conversation", and the third copy got written anyway
//! (2026-08-15, in `artifacts.rs`, in this same crate). A sentence asking the
//! next person to think is not a mechanism; a module they have to import is.
//!
//! The genuinely separate copy is `talkbank-parser-tests`'s
//! `conformance_inventory::format_rust_source`, which lives in the OTHER cargo
//! workspace. That one stays: `spec/` deliberately depends on no root crate but
//! the grammar, and a cross-workspace path dependency to share twenty lines
//! would cost more than it saves.
//!
//! The edition is pinned here and owned by `Cargo.toml`. If the workspace moves
//! edition, `just fmt` and every generator will format differently and the
//! currency gates will fail reporting staleness; update both together.

use std::io::Write as _;
use std::process::{Command, Stdio};

use thiserror::Error;

/// Why generated Rust could not be formatted.
///
/// Every variant is a failure to RUN the formatter or to read its output.
/// There is deliberately no "formatted anyway, unchecked" outcome: returning
/// the unformatted input on failure is what would make a currency gate report
/// staleness on a machine that is merely missing a toolchain component.
#[derive(Debug, Error)]
pub enum RustfmtError {
    #[error(
        "cannot run rustfmt, which generated Rust must be formatted by: {source}. \
         It ships with the Rust toolchain (`rustup component add rustfmt`)."
    )]
    Unavailable {
        #[source]
        source: std::io::Error,
    },

    #[error("rustfmt rejected the generated Rust (exit {status}): {stderr}")]
    Failed { status: String, stderr: String },

    #[error("rustfmt produced output that is not UTF-8")]
    OutputNotUtf8,
}

/// Format generated Rust with the same tool `cargo fmt` uses.
pub fn format_generated_rust(source: &str) -> Result<String, RustfmtError> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| RustfmtError::Unavailable { source })?;

    // Scoped so the pipe closes and rustfmt sees end of input.
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| RustfmtError::Unavailable {
                source: std::io::Error::other("rustfmt stdin was not captured"),
            })?;
        stdin
            .write_all(source.as_bytes())
            .map_err(|source| RustfmtError::Unavailable { source })?;
    }

    let output = child
        .wait_with_output()
        .map_err(|source| RustfmtError::Unavailable { source })?;
    if !output.status.success() {
        return Err(RustfmtError::Failed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| RustfmtError::OutputNotUtf8)
}
