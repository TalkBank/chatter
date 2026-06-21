//! `chatter cache` subcommands.

use clap::Subcommand;
use std::path::PathBuf;

/// Cache maintenance subcommands under `chatter cache`.
#[derive(Subcommand)]
pub enum CacheCommands {
    /// Display cache statistics
    Stats {
        /// Output JSON format
        #[arg(long)]
        json: bool,
    },

    /// Clear cache entries
    Clear {
        /// Clear all cache entries
        #[arg(long, conflicts_with = "prefix")]
        all: bool,

        /// Clear entries matching this path prefix
        #[arg(long, conflicts_with = "all")]
        prefix: Option<PathBuf>,

        /// Show what would be cleared without actually clearing
        #[arg(long)]
        dry_run: bool,
    },
}
