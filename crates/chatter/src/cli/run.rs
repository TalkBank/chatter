//! Command dispatch from parsed CLI arguments into command handlers.
//!
//! [`run`] is the single entry point after argument parsing. It resolves cross-cutting
//! concerns (tracing initialisation, color theme loading), then hands the parsed
//! command to the feature-oriented CLI command services in [`crate::commands`].
//!
//! The TUI decision is not made here: `validate` decides it together with its
//! output format, `--quiet` and `--audit` (`ValidationPresentation::resolve`).
//! Tracing is off unless `-v` or `RUST_LOG` asks for it, so nothing interleaves
//! with the interactive display by default.

/// Execute a parsed request. Presentation belongs to the applicable command.
pub fn run(cli: super::Cli) -> crate::exit_codes::CommandResult {
    super::init_tracing(cli.verbose, &cli.log_format);
    crate::commands::dispatch_command(cli.command)
}
