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
    parser_kind: talkbank_transform::ParserKind,
    alignment: talkbank_model::validation::AlignmentValidation,
    clear_screen: bool,
) -> crate::exit_codes::CommandResult {
    let scope = WatchScope::admit(path)?;

    // The selected backend and alignment policy enter the same cache identity
    // as `validate`; the cache is opened (and pruned) once, here.
    let config = ValidationConfig {
        alignment,
        parser_kind,
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

    let (event_tx, event_rx) = unbounded();
    let (root, mode) = scope.registration();
    let mut watcher = create_watcher(scope.clone(), event_tx)?;
    watcher
        .watch(root, mode)
        .map_err(|source| WatchError::Watch {
            path: root.to_path_buf(),
            source,
        })?;

    // Set up Ctrl+C handler
    let (ctrl_c_tx, ctrl_c_rx) = unbounded();
    ctrlc::set_handler(move || {
        let _ = ctrl_c_tx.send(());
    })
    .map_err(|source| WatchError::CtrlC { source })?;

    outln!("👀 Watching {} for changes...", path.display());
    outln!("   Press Ctrl+C to stop\n");
    if let WatchScope::File { path, .. } = &scope {
        watched.validate(path)?;
    }

    // Debounce: wait 500ms after last event before validating
    let debounce_duration = Duration::from_millis(500);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        // Check for events or ctrl-c
        select! {
            recv(event_rx) -> msg => {
                match msg {
                    Ok(WatchEvent::Changed(path)) => { pending.insert(path, Instant::now()); }
                    Ok(WatchEvent::Fault(source)) => return Err(WatchError::Monitor { source }.into()),
                    Err(_) => return Err(WatchError::Disconnected.into()),
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
                    watched.validate(&file_path)?;
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
    #[error("Cannot inspect watch path {path}: {source}")]
    Inspect {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Watch path is not a regular file or directory: {path}")]
    Unsupported { path: PathBuf },
    #[error("Filesystem monitoring failed: {source}")]
    Monitor { source: notify::Error },
    #[error("Filesystem monitoring channel closed")]
    Disconnected,
    #[error("Failed to create file watcher: {source}")]
    CreateWatcher { source: notify::Error },
    #[error("Failed to watch path {path}: {source}")]
    Watch {
        path: PathBuf,
        source: notify::Error,
    },
    #[error("Failed to set Ctrl+C handler: {source}")]
    CtrlC { source: ctrlc::Error },
}

impl From<WatchError> for crate::exit_codes::CommandStop {
    fn from(error: WatchError) -> Self {
        eprintln!("Error: {error}");
        crate::exit_codes::ReportedFailure::Input.into()
    }
}

/// An admitted scope owns both registration and event selection. Files observe
/// their parent so replacement saves cannot detach the watch from the filename.
#[derive(Clone)]
enum WatchScope {
    File { path: PathBuf, parent: PathBuf },
    Directory(PathBuf),
}

impl WatchScope {
    fn admit(path: &Path) -> Result<Self, WatchError> {
        let inspect = |source| WatchError::Inspect {
            path: path.to_path_buf(),
            source,
        };
        let resolved = path.canonicalize().map_err(inspect)?;
        let metadata = resolved.metadata().map_err(inspect)?;
        if metadata.is_dir() {
            Ok(Self::Directory(resolved))
        } else if metadata.is_file() {
            let parent = resolved
                .parent()
                .ok_or_else(|| WatchError::Unsupported {
                    path: resolved.clone(),
                })?
                .to_path_buf();
            Ok(Self::File {
                path: resolved,
                parent,
            })
        } else {
            Err(WatchError::Unsupported { path: resolved })
        }
    }

    fn registration(&self) -> (&Path, RecursiveMode) {
        match self {
            Self::File { parent, .. } => (parent, RecursiveMode::NonRecursive),
            Self::Directory(path) => (path, RecursiveMode::Recursive),
        }
    }

    fn accepts(&self, candidate: &Path) -> bool {
        match self {
            Self::File { path, .. } => candidate == path,
            Self::Directory(root) => {
                candidate.starts_with(root) && is_chat_transcript_path(candidate)
            }
        }
    }
}

enum WatchEvent {
    Changed(PathBuf),
    Fault(notify::Error),
}

fn forward_event(
    scope: &WatchScope,
    tx: &Sender<WatchEvent>,
    result: Result<Event, notify::Error>,
) {
    match result {
        Err(error) => {
            let _ = tx.send(WatchEvent::Fault(error));
        }
        Ok(event) => {
            if matches!(
                event.kind,
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
            ) {
                for path in event.paths {
                    if scope.accepts(&path) {
                        let _ = tx.send(WatchEvent::Changed(path));
                    }
                }
            }
        }
    }
}

fn create_watcher(
    scope: WatchScope,
    tx: Sender<WatchEvent>,
) -> Result<RecommendedWatcher, WatchError> {
    notify::recommended_watcher(move |result| forward_event(&scope, &tx, result))
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
    fn validate(&self, path: &Path) -> crate::exit_codes::CommandResult {
        if self.clear_screen {
            // ANSI escape: clear screen and move cursor to top-left
            out!("\x1B[2J\x1B[1;1H");
        }

        outln!("📝 Validating: {}", path.display());
        outln!("{}", "─".repeat(60));

        let ending = validate_watched_file(path.to_path_buf(), &self.run)?;
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_path_events_and_monitor_errors_are_preserved() {
        let directory = tempfile::tempdir().expect("watch scope");
        let path = directory.path().join("selected.cha");
        std::fs::write(&path, "").expect("file");
        let scope = WatchScope::admit(&path).expect("admit");
        let selected = path.canonicalize().expect("resolved path");
        let (root, mode) = scope.registration();
        assert_eq!(root, selected.parent().expect("parent"));
        assert_eq!(mode, RecursiveMode::NonRecursive);
        let (tx, rx) = unbounded();
        forward_event(
            &scope,
            &tx,
            Ok(
                Event::new(EventKind::Create(notify::event::CreateKind::File))
                    .add_path(root.join("other.cha")),
            ),
        );
        assert!(rx.try_recv().is_err());
        for kind in [
            EventKind::Remove(notify::event::RemoveKind::File),
            EventKind::Create(notify::event::CreateKind::File),
        ] {
            forward_event(&scope, &tx, Ok(Event::new(kind).add_path(selected.clone())));
            assert!(matches!(rx.try_recv(), Ok(WatchEvent::Changed(path)) if path == selected));
        }
        forward_event(&scope, &tx, Err(notify::Error::generic("monitor failed")));
        assert!(
            matches!(rx.try_recv(), Ok(WatchEvent::Fault(error)) if error.to_string().contains("monitor failed"))
        );
    }
}
