//! Watch mode - continuously validate CHAT files as they change.
//!
//! Uses the `notify` crate to monitor file system events and re-validate
//! files when they are modified.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crossbeam_channel::{Sender, select, unbounded};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use talkbank_transform::paths::is_chat_transcript_path;
use thiserror::Error;

use crate::commands::CacheRefreshMode;
use crate::commands::validate::cache::{CacheInit, initialize_validation_cache};
use crate::commands::validate_parallel::{CachePolicy, validate_watched_file};
use talkbank_transform::{RunEnding, ValidationConfig, ValidationRun};

/// Watch CHAT files for changes and revalidate each changed transcript with
/// the default rules and the shared validation cache.
///
/// File events are debounced (an editor that writes in bursts triggers one
/// validation), and `alignment` is the `--skip-alignment` choice, as for
/// `validate`. A directory is watched with everything below it. Each changed
/// file goes through the validation runner's one-file pipeline, the one
/// `validate` uses, with the cache opened once when the watch starts.
pub fn watch_files(
    path: &Path,
    alignment: talkbank_model::validation::AlignmentValidation,
    clear_screen: bool,
) -> Result<(), WatchError> {
    if !path.exists() {
        return Err(WatchError::MissingPath {
            path: path.to_path_buf(),
        });
    }

    outln!("👀 Watching {} for changes...", path.display());
    outln!("   Press Ctrl+C to stop\n");

    // The default rules, as `validate` without flags, so the two share
    // cached verdicts; the cache is opened (and pruned) once, here.
    let config = ValidationConfig {
        alignment,
        ..ValidationConfig::default()
    };
    let CacheInit {
        run,
        events: cache_events,
    } = initialize_validation_cache(
        &[],
        CachePolicy::ReadWrite {
            refresh: CacheRefreshMode::ReuseExisting,
        },
        config,
    );
    for event in &cache_events {
        eprintln!("{}", event.sentence());
    }
    let watched = Watched { run, clear_screen };

    // Run initial validation
    if path.is_file() {
        watched.validate(path);
    }

    // Set up file watcher (debounced events ensure we do not over-drain CPU on editors that fire multi events)
    let (event_tx, event_rx) = unbounded();
    let mut watcher = create_watcher(event_tx)?;

    watcher
        .watch(path, RecursiveMode::Recursive)
        .map_err(|source| WatchError::Watch {
            path: path.to_path_buf(),
            source,
        })?;

    // Set up Ctrl+C handler
    let (ctrl_c_tx, ctrl_c_rx) = unbounded();
    ctrlc::set_handler(move || {
        let _ = ctrl_c_tx.send(());
    })
    .map_err(|source| WatchError::CtrlC { source })?;

    // Debounce: wait 500ms after last event before validating
    let debounce_duration = Duration::from_millis(500);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        // Check for events or ctrl-c
        select! {
            recv(event_rx) -> msg => {
                if let Ok(file_path) = msg {
                    // Mark this file as pending with current time
                    pending.insert(file_path, Instant::now());
                }
            }
            recv(ctrl_c_rx) -> _ => {
                outln!("\n👋 Stopping watch mode...");
                break;
            }
            default(Duration::from_millis(100)) => {
                // Check if any pending files are ready (debounce expired)
                let now = Instant::now();
                let ready: Vec<PathBuf> = pending
                    .iter()
                    .filter(|(_, last_event)| now.duration_since(**last_event) >= debounce_duration)
                    .map(|(path, _)| path.clone())
                    .collect();

                for file_path in ready {
                    pending.remove(&file_path);
                    watched.validate(&file_path);
                }
            }
        }
    }

    Ok(())
}

/// Errors returned while setting up or running filesystem watch mode.
///
/// The watch mode is long-running, so each variant includes enough context to correlate with
/// the CLI’s monitoring/uptime guidance in the File Format appendix (e.g., missing paths or
/// watcher limitations reported verbatim to users).
#[derive(Debug, Error)]
pub enum WatchError {
    #[error("Path does not exist: {path}")]
    MissingPath { path: PathBuf },
    #[error("Failed to create file watcher")]
    CreateWatcher { source: notify::Error },
    #[error("Failed to watch path: {path}")]
    Watch {
        path: PathBuf,
        source: notify::Error,
    },
    #[error("Failed to set Ctrl+C handler")]
    CtrlC { source: ctrlc::Error },
}

/// Builds watcher for downstream use.
///
/// Returns a `notify::RecommendedWatcher` configured only to observe `.cha` create/modify events so
/// edit-time validation stays within the CHAT file format constraints.
fn create_watcher(tx: Sender<PathBuf>) -> Result<RecommendedWatcher, WatchError> {
    notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            // Only care about modify and create events
            if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                for path in event.paths {
                    // Only process .cha transcripts (skip AppleDouble sidecars).
                    if is_chat_transcript_path(&path) {
                        let _ = tx.send(path);
                    }
                }
            }
        }
    })
    .map_err(|source| WatchError::CreateWatcher { source })
}

/// What every validation in one watch shares.
struct Watched {
    /// The runner's configuration (default rules, the watch's alignment) and
    /// the cache opened for it once for the whole watch.
    run: ValidationRun,
    /// Whether to clear the screen before each file.
    clear_screen: bool,
}

impl Watched {
    /// Clear the screen if asked, print a header, validate `path` through
    /// the runner, and say whether it passed. Its diagnostics or failure are
    /// printed as `validate --quiet` prints them; a file that cannot be read
    /// (renamed or locked mid-edit) is said, and the watch goes on.
    fn validate(&self, path: &Path) {
        if self.clear_screen {
            // ANSI escape: clear screen and move cursor to top-left
            out!("\x1B[2J\x1B[1;1H");
        }

        outln!("📝 Validating: {}", path.display());
        outln!("{}", "─".repeat(60));

        let ending = validate_watched_file(path.to_path_buf(), &self.run);
        match &ending {
            RunEnding::Complete(stats) if ending.passed() => match stats.snapshot().cache_hits() {
                0 => outln!("✓ {} is valid", path.display()),
                _ => outln!("✓ {} is valid (cached)", path.display()),
            },
            // Its diagnostics, its failure or the run's own ending are
            // already said.
            RunEnding::Complete(_)
            | RunEnding::NothingFound
            | RunEnding::Stopped { .. }
            | RunEnding::Incomplete { .. }
            | RunEnding::Aborted(_) => {}
        }

        outln!();
    }
}
