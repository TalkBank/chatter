//! Interactive TUI for validation error browsing with CLAN integration.
//!
//! Displays validation errors in a two-pane layout:
//! - Left: File list with error counts
//! - Right: Error details for selected file with source context
//!
//! Keyboard controls:
//! - Tab: Switch between file list and error list
//! - j/k or ↑/↓: Navigate within pane
//! - Enter: Open selected error in CLAN (via send2clan)
//! - r: Re-run validation
//! - q or Esc: Quit
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod models;
mod rendering;
mod state;
mod text_processing;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, poll},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders, Paragraph},
};
use std::io;
use std::time::Duration;
use talkbank_transform::validation_runner::{
    AbortReason, FileCompleteEvent, FileStatus, RunEnding, ValidationEvent,
};

use crate::commands::RunPhase;
use crate::ui::Theme;

/// Return value from TUI indicating user action.
#[derive(Debug)]
pub enum TuiAction {
    /// User quit normally, with the run where it was.
    Quit(RunPhase),
    /// User requested immediate process termination
    ForceQuit,
    /// User requested rerun validation
    Rerun,
}

use models::{FileErrors, FileFailure};

use rendering::{
    render_error_details, render_file_list, render_footer_streaming, render_header_streaming,
    render_notes,
};
use state::{Redraw, TuiState};

/// Launch the validation TUI with streaming error display.
///
/// Errors appear in real-time as validation progresses. User can cancel validation
/// by pressing 'c' or Ctrl+C. Files are kept sorted alphabetically. `notes`
/// are the run's notices and cache events, shown above the file list.
pub fn run_validation_tui_streaming(
    events_rx: crossbeam_channel::Receiver<ValidationEvent>,
    canceller: talkbank_transform::Canceller,
    theme: Theme,
    notes: &[String],
) -> Result<TuiAction> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // The file list populates as files with diagnostics or failures arrive.
    let mut state = TuiState::new(theme);
    let mut phase = RunPhase::Running;
    let mut ctrl_c_count = 0usize;

    // Main event loop with non-blocking polls
    let result = loop {
        // Draw UI
        terminal.draw(|f| ui_streaming(f, &mut state, &phase, notes))?;

        // Poll for keyboard input (non-blocking, 50ms timeout)
        if poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            // Clear any transient status message on keypress
            state.status_message = None;

            if !state.handle_common_key(key.code, key.modifiers) {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('c'), KeyModifiers::NONE) => {
                        canceller.cancel();
                    }
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        ctrl_c_count += 1;
                        canceller.cancel();
                        if ctrl_c_count >= 2 {
                            break Ok(TuiAction::ForceQuit);
                        }
                    }
                    (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => {
                        canceller.cancel();
                        break Ok(TuiAction::Quit(phase));
                    }
                    // Rerun is offered for every ended run, including one
                    // that aborted or lost files: re-running is exactly what a
                    // user wants after either.
                    (KeyCode::Char('r'), KeyModifiers::NONE) if phase.is_ended() => {
                        break Ok(TuiAction::Rerun);
                    }
                    _ => {}
                }
            }
        }

        // Drain all pending validation events (non-blocking)
        phase = drain_validation_events(&events_rx, &mut state, phase);
    };

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Absorb every event currently queued from the runner into `state`, returning
/// the run's phase afterwards.
///
/// Takes the phase by value and returns the next one, so a transition is a
/// function from one state to another rather than a mutation some caller might
/// forget to apply.
fn drain_validation_events(
    events_rx: &crossbeam_channel::Receiver<ValidationEvent>,
    state: &mut TuiState,
    phase: RunPhase,
) -> RunPhase {
    let mut phase = phase;
    loop {
        match events_rx.try_recv() {
            Ok(ValidationEvent::Discovering) => {
                state.progress.discovering = true;
            }
            Ok(ValidationEvent::Started { total_files }) => {
                state.progress.total_files = total_files;
                state.progress.discovering = false;
            }
            Ok(ValidationEvent::FileComplete(complete)) => {
                record_file(state, complete);
                state.progress.files_processed += 1;
                state.update_progress_display(Redraw::WhenStrideReached);
            }
            Ok(ValidationEvent::Finished(ending)) => {
                // The counts still describe what WAS processed and are worth
                // showing; the ending is what says whether they are totals.
                if let Some(stats) = ending.stats() {
                    state.progress.total_files = stats.total_files().get();
                    state.progress.files_processed = stats.files_accounted_for();
                    state.update_progress_display(Redraw::Immediately);
                }
                phase = RunPhase::Ended(ending);
            }
            Err(crossbeam_channel::TryRecvError::Empty) => break,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                // The runner's drop guard always sends an ending before its
                // senders close, so a silent close means that guarantee
                // broke: an abort, never a finished run.
                if !phase.is_ended() {
                    phase = RunPhase::Ended(RunEnding::Aborted(AbortReason::NoEnding));
                }
                break;
            }
        }
    }
    phase
}

/// List a file that has something to show: its diagnostics (warnings
/// included), and how it failed beyond them (an unreadable file, a failed
/// roundtrip, a tool failure). A valid file with no diagnostic is not
/// listed.
fn record_file(state: &mut TuiState, complete: FileCompleteEvent) {
    let failure = match &complete.status {
        FileStatus::Valid { .. } | FileStatus::Invalid { .. } => None,
        FileStatus::ReadError { message } => Some(FileFailure::Unreadable(message.clone())),
        FileStatus::RoundtripFailed { reason, .. } => {
            Some(FileFailure::RoundtripFailed(reason.clone()))
        }
        FileStatus::InternalFailure { failure, .. } => {
            Some(FileFailure::ToolFailed(failure.to_string()))
        }
    };
    let (mut errors, source) = match (complete.status.shown(), &failure) {
        (Some(shown), _) => (shown.errors.to_vec(), std::sync::Arc::from(shown.source)),
        (None, None) => return,
        (None, Some(_)) => (Vec::new(), std::sync::Arc::from("")),
    };
    // Full line context for miette-style display.
    talkbank_model::enhance_errors_with_source(&mut errors, &source);
    let mut file = FileErrors {
        path: complete.path,
        errors,
        source,
        failure,
    };
    file.ensure_line_columns();
    state.add_file(file);
}

/// UI rendering for streaming validation (shows validation status).
fn ui_streaming(f: &mut Frame, state: &mut TuiState, phase: &RunPhase, notes: &[String]) {
    let notes_height = u16::try_from(notes.len()).unwrap_or(u16::MAX);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),            // Header with title + gauge
            Constraint::Length(notes_height), // The run's notices, one per line
            Constraint::Min(0),               // Main content
            Constraint::Length(4),            // Footer (action row + nav row)
        ])
        .split(f.area());

    render_header_streaming(f, chunks[0], state, phase);
    render_notes(f, chunks[1], state, notes);

    if state.files.is_empty() {
        // Nothing to browse. The text is per-ending, because "no errors
        // found" is a claim about every discovered file, which only a run
        // that passed can make.
        let (msg, color) = empty_state_message(state, phase);
        let paragraph = Paragraph::new(msg)
            .style(Style::default().fg(color))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(paragraph, chunks[2]);
    } else {
        // Split main content into two panes
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // File list (left)
                Constraint::Percentage(70), // Error details (right)
            ])
            .split(chunks[2]);

        render_file_list(f, main_chunks[0], state);
        if let Some(metrics) = render_error_details(f, main_chunks[1], state) {
            state.apply_detail_metrics(metrics);
        }
    }

    render_footer_streaming(f, chunks[3], state, phase);
}

/// The message and color of the main pane when no file is listed.
fn empty_state_message(state: &TuiState, phase: &RunPhase) -> (String, ratatui::style::Color) {
    let ended = |text: String| (text, state.theme.header_err);
    match phase {
        RunPhase::Running => {
            let msg = if state.progress.discovering {
                "Discovering files... (press 'c' to cancel)"
            } else {
                "Validating files... (press 'c' to cancel)"
            };
            (msg.to_owned(), state.theme.header_progress)
        }
        RunPhase::Ended(ending) => match ending {
            RunEnding::Complete(stats) if ending.passed() => (
                format!(
                    "✓ {} files validated, no errors found! Press 'q' to quit.",
                    stats.snapshot().total_files()
                ),
                state.theme.header_ok,
            ),
            RunEnding::Complete(stats) => ended(format!(
                "✗ {} of {} files failed. Press 'r' to re-run, 'q' to quit.",
                stats.snapshot().failed_files(),
                stats.snapshot().total_files()
            )),
            RunEnding::NothingFound => {
                ended("✗ No .cha files found. Press 'r' to re-run, 'q' to quit.".to_owned())
            }
            RunEnding::Stopped { stats, reason } => ended(format!(
                "⚠ {reason}: {} of {} files were never checked. \
                 No errors in the rest. Press 'r' to re-run, 'q' to quit.",
                stats.missing_files(),
                stats.snapshot().total_files()
            )),
            RunEnding::Incomplete { stats, cause } => ended(format!(
                "⚠ Validation did not finish: {} of {} files were never checked: \
                 {cause} No errors in the rest. Press 'r' to re-run, 'q' to quit.",
                stats.missing_files(),
                stats.snapshot().total_files()
            )),
            RunEnding::Aborted(reason) => {
                ended(format!("⚠ {reason} Press 'r' to re-run, 'q' to quit."))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Drain everything `events` holds into a fresh state, the channel then
    /// closed.
    fn drain(events: Vec<ValidationEvent>) -> (TuiState, RunPhase) {
        let (events_tx, events_rx) = crossbeam_channel::unbounded::<ValidationEvent>();
        for event in events {
            events_tx.send(event).unwrap();
        }
        drop(events_tx);
        let mut state = TuiState::new(Theme::default());
        let phase = drain_validation_events(&events_rx, &mut state, RunPhase::Running);
        (state, phase)
    }

    /// A run whose stream dies without ever reporting its end is shown as
    /// aborted, never as a completed run.
    #[test]
    fn a_stream_that_dies_without_finishing_is_not_reported_as_complete() {
        let (_, phase) = drain(vec![
            ValidationEvent::Discovering,
            ValidationEvent::Started { total_files: 9 },
        ]);
        assert_eq!(
            phase,
            RunPhase::Ended(RunEnding::Aborted(AbortReason::NoEnding))
        );
    }

    /// The ending the runner sent is the phase, unchanged: whatever it was,
    /// the TUI shows it rather than a judgement of its own.
    #[test]
    fn the_runners_ending_is_the_phase() {
        let ending = RunEnding::Aborted(AbortReason::Panicked);
        let (_, phase) = drain(vec![ValidationEvent::Finished(ending.clone())]);
        assert_eq!(phase, RunPhase::Ended(ending));
    }

    /// A file that could not be read is listed, with why, so a run over an
    /// unreadable argument never shows an empty, clean-looking list.
    #[test]
    fn a_file_that_failed_without_diagnostics_is_listed() {
        let (state, _) = drain(vec![
            ValidationEvent::Started { total_files: 1 },
            ValidationEvent::FileComplete(FileCompleteEvent {
                path: "missing.cha".into(),
                status: FileStatus::ReadError {
                    message: "No such file or directory".to_owned(),
                },
                cache: talkbank_transform::CacheUse::NotConsulted,
            }),
        ]);
        assert_eq!(state.files.len(), 1);
        assert_eq!(state.files[0].path, std::path::Path::new("missing.cha"));
        assert_eq!(
            state.files[0].failure,
            Some(FileFailure::Unreadable(
                "No such file or directory".to_owned()
            ))
        );
    }

    /// An input with nothing in it ends `NothingFound`, a failing ending,
    /// and the empty pane says so instead of "no errors found".
    #[test]
    fn an_empty_input_is_not_shown_as_clean() {
        let (state, phase) = drain(vec![
            ValidationEvent::Started { total_files: 0 },
            ValidationEvent::Finished(RunEnding::NothingFound),
        ]);
        let (message, color) = empty_state_message(&state, &phase);
        assert!(message.contains("No .cha files found"), "{message}");
        assert_eq!(color, state.theme.header_err);
    }
}
