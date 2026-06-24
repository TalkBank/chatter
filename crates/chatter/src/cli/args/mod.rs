//! CLI argument definitions for `talkbank` commands and global flags.
//!
//! This module is split by concern:
//! - `cli_struct`, top-level `Cli` parser struct and its global flags
//! - `core`, the `Commands` enum (all subcommand variants)
//! - `cli_types`, shared config enums (log format, TUI mode, output format, parser
//!   backend, judgment mode)
//! - `judgment_args`, shared judgment-engine arg group (speaker-id / pipeline / batch)
//! - `cache_commands`, `chatter cache` subcommands
//! - `debug_commands`, `chatter debug` subcommands

mod cache_commands;
mod cli_struct;
mod cli_types;
mod core;
mod debug_commands;
mod judgment_args;

pub use cache_commands::CacheCommands;
pub use cli_struct::Cli;
pub use cli_types::{AlignmentTier, JudgmentMode, LogFormat, OutputFormat, ParserBackend};
pub use core::Commands;
pub use debug_commands::DebugCommands;
