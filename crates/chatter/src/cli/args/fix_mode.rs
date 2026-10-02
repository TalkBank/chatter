//! `chatter fix`'s write mode, selected by its `--apply` flag.
//!
//! No boolean leaves the command line: clap hands
//! [`Commands::Fix`](super::Commands::Fix) a `Flag<FixMode>` (see
//! [`super::flag_modes`]), so the command matches on a mode rather than
//! testing a flag.
//!
//! There used to be a second flag, `--dry-run`, which required `--apply`
//! and cancelled its write; `--apply --dry-run` did exactly what a bare
//! `fix` does. It was removed (a bare `fix` already reports without
//! writing), and clap now refuses it as an unknown argument.

use super::flag_modes::FlagMode;

/// Whether `chatter fix` writes its fixes or only reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixMode {
    /// Say what would be fixed and write nothing (a bare `fix`).
    Report,
    /// Write the fixes (`--apply`).
    Apply,
}

impl FlagMode for FixMode {
    const LONG: &'static str = "apply";
    const HELP: &'static str = "Write the fixes (without this, report only)";
    fn absent() -> Self {
        Self::Report
    }
    fn present() -> Self {
        Self::Apply
    }
}
