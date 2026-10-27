#![warn(missing_docs)]
// Results reach stdout only through `outln!`/`out!` (`stdout.rs`); `println!`
// and `print!` panic when a consumer closes the pipe. Clippy refuses them here.
#![deny(clippy::print_stdout)]
// Test code is exempt from this crate's `deny`-level panic lints,
// see `docs/panic-audit/chatter.md`.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]
//! `chatter` -- command-line interface for CHAT format validation, conversion,
//! and analysis.
//!
//! This binary (`chatter`) is the main user-facing tool in the TalkBank
//! toolchain. It validates CHAT transcripts, converts between CHAT and JSON,
//! normalizes files to canonical form, and provides a continuous watch mode
//! for iterative editing.
//!
//! # Command discovery and dispatch
//!
//! `chatter --help` lists the command families; `chatter commands list
//! --format json` describes each executable leaf's syntax and effects. Stable
//! validation, repair, conversion and inspection commands are separate from
//! experimental speaker workflows, review and transforms.
//!
//! `main` starts the 16 MiB program thread. That thread constructs clap's
//! command tree, parses OS-native arguments and passes the request through
//! [`cli::run`] to [`commands::dispatch_command`]. Every handler returns
//! [`exit_codes::CommandResult`]. The joined result reaches the single exit
//! boundary, after command-owned stages and terminal guards have been dropped.
//! Help and version requests are successful output operations; usage failures
//! return status 2. Neither path starts transcript work.
//!
//! # Presentation and parsers
//!
//! Validation admits one presentation from text, JSON, audit and terminal UI
//! options. Machine output never enters the TUI. Both tree-sitter and re2c use
//! the same validation runner and model; select them with `--parser`.
//!
//! # Failed output and interruption
//!
//! Text writers use `outln!` and `out!`, which propagate failed writes instead
//! of panicking or continuing into later mutations. JSON records likewise stop
//! the stream at the first failed write. A closed pipe returns status 1; machine
//! output keeps stderr empty. Forced validation interruption returns status 130
//! through the owning event loop, allowing terminal and staged-output cleanup.
//!
//! # Module map
//!
//! ```text
//! src/
//! ├── main.rs          ← entry point (this file)
//! ├── cli/
//! │   ├── args.rs      ← Cli struct, Commands enum (clap derive)
//! │   ├── run.rs       ← composition root and tracing initialization
//! │   └── logging.rs   ← tracing-subscriber initialization
//! ├── commands/
//! │   ├── dispatch.rs  ← feature-oriented top-level command-family routing
//! │   ├── validate/    ← single-file and directory validation
//! │   ├── validate_parallel/  ← streaming validation runtime and renderers
//! │   ├── json.rs      ← to-json / from-json conversion
//! │   ├── normalize.rs ← canonical re-serialization
//! │   ├── watch.rs     ← file-watcher continuous validation
//! │   ├── clean.rs     ← cleaned-text debugging output
//! │   ├── debug.rs     ← debug-family commands
//! │   ├── new_file.rs  ← CHAT file scaffolding
//! │   ├── schema.rs    ← JSON Schema output
//! │   ├── cache/       ← cache stats and clear subcommands
//! │   └── alignment/   ← alignment visualization (show-alignment)
//! ├── output.rs        ← formatting and rendering helpers
//! ├── progress.rs      ← progress bar utilities
//! └── ui/              ← TUI rendering (ratatui), themes, validation display
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

// First, so its `outln!` and `out!` macros are in scope in every module below.
#[macro_use]
mod stdout;
mod cli;
mod commands;
pub mod exit_codes;
pub mod output;
pub mod progress;
pub mod ui;

use clap::{CommandFactory, FromArgMatches};
use std::panic::resume_unwind;

/// Stack size for the thread the whole program runs on.
///
/// The clap-derived command tree alone needs close to 1 MiB of stack in
/// debug builds, and it grows with every CLAN command the parity work
/// adds; recursive parser and validation code runs deeper still. The
/// Windows default main-thread stack is exactly 1 MiB (macOS and Linux
/// give 8 MiB), and on 2026-06-05 the command-tree construction outgrew
/// it: every `chatter` invocation aborted on Windows with
/// `STATUS_STACK_OVERFLOW` before parsing began. Running the program on
/// a thread with an explicit stack size removes the dependency on
/// platform main-stack defaults entirely (the same approach rustc
/// takes). Regression gate: `tests/stack_limit_tests.rs` plus the
/// native windows-latest CI job. It IS the worker pool's
/// `CHAT_THREAD_STACK_BYTES`, so the program thread and the workers that
/// parse beside it cannot be sized apart.
const PROGRAM_STACK_BYTES: usize = talkbank_transform::worker_pool::CHAT_THREAD_STACK_BYTES;

/// Entry point for this binary target.
///
/// Immediately hands off to [`program_main`] on a thread with an
/// explicit stack size (see [`PROGRAM_STACK_BYTES`]); the OS main
/// thread joins the typed result, chooses the exit status and propagates
/// unexpected panics.
fn main() -> std::process::ExitCode {
    let spawned = std::thread::Builder::new()
        .name("chatter-program".to_owned())
        .stack_size(PROGRAM_STACK_BYTES)
        .spawn(program_main);
    let result = match spawned {
        Ok(handle) => match handle.join() {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        },
        Err(err) => {
            eprintln!("chatter: failed to start program thread: {err}");
            Err(exit_codes::ReportedFailure::Internal.into())
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(stop) => stop.finish(),
    }
}

/// The real program body; runs on the explicitly sized program thread.
fn program_main() -> exit_codes::CommandResult {
    let raw_args: Vec<_> = std::env::args_os().collect();
    let mut cmd = cli::Cli::command();
    if raw_args.len() == 1 {
        outln!("{}", cmd.render_help());
        return Ok(());
    }
    let matches = match cmd.try_get_matches_from(raw_args) {
        Ok(matches) => matches,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion
                    | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ) =>
        {
            out!("{error}");
            return Ok(());
        }
        Err(error) => {
            if let Some(hint) = cli::migration_hint(&error) {
                eprintln!("Migration: {hint}");
            }
            return Err(exit_codes::CommandStop::Usage(error));
        }
    };
    let cli = cli::Cli::from_arg_matches(&matches).map_err(exit_codes::CommandStop::Usage)?;
    cli::run(cli)
}
