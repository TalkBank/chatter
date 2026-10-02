//! `chatter cache` subcommands.

use clap::Subcommand;

/// Cache maintenance subcommands under `chatter cache`.
#[derive(Subcommand)]
pub enum CacheCommands {
    /// Display cache statistics
    Stats {
        /// Output style (text|json), as for `validate`.
        #[arg(short, long, value_enum, default_value_t = super::cli_types::OutputFormat::Text)]
        format: super::cli_types::OutputFormat,
    },

    /// Clear cache entries
    Clear {
        /// `--all` or `--prefix PATH`, parsed into one scope.
        #[command(flatten)]
        scope: super::cache_clear_args::ClearScope,

        /// `--dry-run`, parsed into a mode.
        #[command(flatten)]
        mode: super::flag_modes::Flag<super::cache_clear_args::ClearMode>,
    },
}
