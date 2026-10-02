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

use crate::commands::{self, CommandContext};
use crate::ui::Theme;

/// Execute the CLI command
pub fn run(cli: super::Cli) {
    super::init_tracing(cli.verbose, &cli.log_format);

    // Load color theme for TUI mode
    let theme = Theme::load(cli.theme);

    commands::dispatch_command(
        cli.command,
        &CommandContext {
            tui_mode: cli.tui_mode,
            theme,
        },
    );
}
