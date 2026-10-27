//! CLI argument definitions for `talkbank` commands and global flags.
//!
//! This module is split by concern:
//! - `cli_struct`, top-level `Cli` parser struct and its global flags
//! - `core`, the `Commands` enum (all subcommand variants)
//! - `cli_types`, shared config enums (log format, TUI mode, output format, parser
//!   backend, judgment mode)
//! - `judgment_args`, judgment-engine arg group for speaker-id
//! - `cache_commands`, `chatter cache` subcommands
//! - `cache_clear_args`, `cache clear`'s flags parsed into a scope and a mode
//! - `debug_commands`, experimental transform subcommands
//! - `fix_mode`, `chatter fix`'s `--apply` parsed into one mode
//! - `flag_modes`, presence flags parsed straight into the modes they select

mod cache_clear_args;
mod cache_commands;
mod cli_struct;
mod cli_types;
mod core;
mod debug_commands;
mod fix_mode;
mod flag_modes;
pub(crate) mod judgment_args;
mod speaker_commands;

pub use cache_clear_args::{ClearMode, ClearScope};
pub use cache_commands::CacheCommands;
pub use cli_struct::Cli;
pub use cli_types::{AlignmentTier, LogFormat, OutputFormat, TuiMode};
pub use core::{
    CheckCommands, Commands, ConvertCommands, DiscoveryCommands, ExperimentalCommands,
    InspectCommands,
};
pub use debug_commands::{DebugCommands, JoinRetraceScope};
pub use fix_mode::FixMode;
pub use flag_modes::{Flag, FlagMode, NormalizeCheckArgs, ToJsonCheckArgs};
pub use speaker_commands::{ReviewCommands, SpeakerCommands};
