//! The top-level `Cli` parser struct and its global flags.
//!
//! Separated from the `Commands` enum (in `core`) so that each file
//! stays under the 800-line hard limit while `Commands` (which spans
//! all subcommand variants) grows independently.

use clap::Parser;

use super::cli_types::{LogFormat, TuiMode};
use super::core::Commands;

pub use crate::ui::ThemePreset;

/// TalkBank utilities for CHAT format validation and transformation
#[derive(Parser)]
#[command(name = "chatter", version, long_version = concat!(env!("CARGO_PKG_VERSION"), " (build ", env!("BUILD_HASH"), ")"))]
#[command(
    about = "Tools for validating and transforming TalkBank CHAT files",
    long_about = None,
    after_long_help = "\
Getting started:
  chatter validate myfile.cha          Validate a CHAT file
  chatter validate corpus/             Validate an entire corpus
  chatter to-json myfile.cha           Convert to JSON
  chatter to-xml myfile.cha            Export TalkBank XML

Exit codes:
  0    All files valid / command succeeded
  1    Validation errors found or command failed
  2    Invalid arguments or missing required options

Full documentation: https://talkbank.org/tools/"
)]
pub struct Cli {
    /// Logging verbosity level (can be repeated: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Logging output format
    #[arg(long, value_enum, default_value = "text", global = true)]
    pub log_format: LogFormat,

    /// TUI mode: auto (detect terminal), force (always), disable (never)
    #[arg(long, value_enum, default_value_t, global = true)]
    pub tui_mode: TuiMode,

    /// Color theme for TUI mode
    #[arg(long, value_enum, global = true)]
    pub theme: Option<ThemePreset>,

    #[command(subcommand)]
    pub command: Commands,
}
