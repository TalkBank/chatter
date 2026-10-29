//! The top-level `Cli` parser struct and its global flags.
//!
//! Separated from the `Commands` enum (in `core`) so that each file
//! stays under the 800-line hard limit while `Commands` (which spans
//! all subcommand variants) grows independently.

use clap::Parser;

use super::cli_types::LogFormat;
use super::core::Commands;

/// TalkBank utilities for CHAT format validation and transformation
#[derive(Parser)]
// `bin_name` pins the program name shown in usage/help/error output.
// Without it, clap derives the displayed name from `argv[0]`'s file name,
// which is `chatter.exe` on Windows: every usage line then read
// `Usage: chatter.exe ...`, diverging from the documented `chatter ...`
// invocation and failing the help-contract tests on windows-latest.
// Pinning it makes the name identical on every platform and independent
// of how the binary was invoked. Regression gate:
// `program_name_is_pinned_regardless_of_argv0` in
// `tests/command_execution_tests.rs`.
#[command(name = "chatter", bin_name = "chatter", version, long_version = concat!(env!("CARGO_PKG_VERSION"), " (build ", env!("BUILD_HASH"), ")"))]
#[command(
    about = "Tools for validating and transforming TalkBank CHAT files",
    long_about = None,
    after_long_help = long_help_footer()
)]
pub struct Cli {
    /// Logging verbosity level (can be repeated: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Logging format on stderr; command --format selects result encoding
    #[arg(long, value_enum, default_value = "text", global = true)]
    pub log_format: LogFormat,

    #[command(subcommand)]
    pub command: Commands,
}

fn long_help_footer() -> String {
    format!(
        "Getting started:\n  chatter validate myfile.cha          Validate a CHAT file\n  chatter validate corpus/             Validate an entire corpus\n  chatter convert to-json myfile.cha    Convert to JSON\n  chatter commands list --format json  Discover commands and process statuses\n\n{}\nFull documentation: https://talkbank.org/tools/",
        crate::exit_codes::ExitStatus::help()
    )
}
