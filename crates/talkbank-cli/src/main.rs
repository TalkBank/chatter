#![warn(missing_docs)]
// Test code is exempt from this crate's `deny`-level panic lints,
// see `docs/panic-audit/talkbank-cli.md`.
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
//! # Command overview
//!
//! All commands are implemented as clap derive subcommands. Run
//! `chatter --help` for the full listing; the highlights are:
//!
//! | Command            | Purpose                                                    |
//! |--------------------|------------------------------------------------------------|
//! | `validate`         | Parse and validate one file or an entire directory tree     |
//! | `normalize`        | Re-serialize a CHAT file in canonical formatting            |
//! | `to-json`          | Convert CHAT to JSON (conforming to the CHAT JSON Schema)  |
//! | `from-json`        | Convert JSON back to CHAT                                  |
//! | `to-xml`           | Export one CHAT file to TalkBank XML                       |
//! | `show-alignment`   | Visualize main-tier / dependent-tier alignment              |
//! | `watch`            | Re-validate on every file save (uses `notify` file watcher)|
//! | `lint`             | Detect and optionally auto-fix common issues               |
//! | `clean`            | Show cleaned text for each word (debugging aid)            |
//! | `new-file`         | Scaffold a minimal valid CHAT file                          |
//! | `cache`            | Manage the on-disk validation cache (stats, clear)          |
//! | `schema`           | Print the CHAT JSON Schema or its canonical URL             |
//! | `debug <cmd>`      | Developer/debugging tools and corpus inspection            |
//!
//! # Dispatch architecture
//!
//! ```text
//! main()
//!  ├─ clap::Parser::parse()          ← cli::Cli, cli::Commands (clap derive)
//!  ├─ cli::init_tracing(verbose, ..) ← tracing-subscriber w/ env-filter
//!  └─ cli::run(cli)                  ← composition root, then family-based dispatch
//!       └─ commands::dispatch_command
//!            ├─ ValidationCommandService
//!            ├─ UtilityCommandService
//!            ├─ CacheCommandService
//!            └─ DebugCommandService
//! ```
//!
//! Argument definitions live in [`cli::args`](cli/args.rs) (the `Cli` struct
//! and `Commands` enum). The dispatch switch is in [`cli::run`](cli/run.rs).
//! Each command handler is a function in the [`commands`] module, which in turn
//! calls into the core library crates (`talkbank-transform`, `talkbank-model`).
//!
//! # TUI / interactive mode
//!
//! When stdout is a TTY (or `--tui-mode force` is passed), validation commands
//! render a ratatui-based terminal UI with live progress, color-coded
//! diagnostics, and a theme system (`--theme`). The TUI can be disabled with
//! `--tui-mode disable` for piping output to files or other tools. Tracing
//! output is automatically suppressed in TUI mode to avoid interleaving.
//!
//! # Parser selection
//!
//! The `--parser` flag on `validate` selects between the canonical tree-sitter
//! parser (`tree-sitter`, default) and the experimental direct parser
//! (removed). The tree-sitter parser is the sole parser.
//! `talkbank_model::ChatFile` AST.
//!
//! # Broken pipe handling
//!
//! `main()` installs a custom panic hook that silences broken-pipe panics
//! (common when output is piped to `head` or similar), and catches unwind
//! payloads so the process exits cleanly with code 0 rather than printing a
//! panic backtrace.
//!
//! # Module map
//!
//! ```text
//! src/
//! ├── main.rs          ← entry point (this file)
//! ├── cli/
//! │   ├── args.rs      ← Cli struct, Commands enum (clap derive)
//! │   ├── run.rs       ← composition root (TUI detection, theme loading)
//! │   └── logging.rs   ← tracing-subscriber initialization
//! ├── commands/
//! │   ├── dispatch.rs  ← feature-oriented top-level command-family routing
//! │   ├── validate/    ← single-file and directory validation
//! │   ├── validate_parallel.rs ← parallel directory validation with progress
//! │   ├── json.rs      ← to-json / from-json conversion
//! │   ├── xml.rs       ← to-xml export
//! │   ├── normalize.rs ← canonical re-serialization
//! │   ├── watch.rs     ← file-watcher continuous validation
//! │   ├── lint.rs      ← auto-fixable issue detection
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

mod cli;
mod commands;
pub mod exit_codes;
pub mod output;
pub mod progress;
pub mod ui;

use clap::{CommandFactory, FromArgMatches};
use std::panic::{AssertUnwindSafe, PanicHookInfo, catch_unwind, resume_unwind};

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
/// native windows-latest CI job.
const PROGRAM_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Entry point for this binary target.
///
/// Immediately hands off to [`program_main`] on a thread with an
/// explicit stack size (see [`PROGRAM_STACK_BYTES`]); the OS main
/// thread only waits and propagates panics so exit behavior is
/// unchanged.
fn main() {
    let spawned = std::thread::Builder::new()
        .name("chatter-program".to_owned())
        .stack_size(PROGRAM_STACK_BYTES)
        .spawn(program_main);
    match spawned {
        Ok(handle) => {
            if let Err(payload) = handle.join() {
                // The program thread panicked; re-raise on the main
                // thread so the process exits exactly as an unwound
                // panic in `main` would (code 101, hook already ran).
                resume_unwind(payload);
            }
        }
        Err(err) => {
            // Thread creation failing is an OS-resource failure that
            // precedes all program logic; report and exit nonzero.
            eprintln!("chatter: failed to start program thread: {err}");
            std::process::exit(1);
        }
    }
}

/// The real program body; runs on the explicitly sized program thread.
fn program_main() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if panic_info_is_broken_pipe(info) {
            return;
        }
        default_hook(info);
    }));

    let raw_args: Vec<String> = std::env::args().collect();

    // Build the clap Command and parse.
    let cmd = cli::Cli::command();
    let matches = cmd.get_matches_from(raw_args);
    // clap-validation invariant: `cmd.get_matches_from(...)` above
    // exits the process on any validation failure, so reaching
    // `from_arg_matches` guarantees the matches are well-formed.
    #[allow(clippy::expect_used)]
    let cli =
        cli::Cli::from_arg_matches(&matches).expect("clap should have validated all arguments");

    let result = catch_unwind(AssertUnwindSafe(|| {
        cli::run(cli);
    }));

    if let Err(payload) = result {
        if panic_is_broken_pipe(&payload) {
            std::process::exit(0);
        }
        resume_unwind(payload);
    }
}

/// Return `true` when a panic hook payload reports a broken pipe.
fn panic_info_is_broken_pipe(info: &PanicHookInfo<'_>) -> bool {
    if let Some(msg) = info.payload().downcast_ref::<String>() {
        return msg.contains("Broken pipe");
    }
    if let Some(msg) = info.payload().downcast_ref::<&str>() {
        return msg.contains("Broken pipe");
    }
    false
}

/// Return `true` when an unwind payload reports a broken pipe.
fn panic_is_broken_pipe(payload: &Box<dyn std::any::Any + Send>) -> bool {
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.contains("Broken pipe");
    }
    if let Some(msg) = payload.downcast_ref::<&str>() {
        return msg.contains("Broken pipe");
    }
    false
}
