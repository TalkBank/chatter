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
use talkbank_transform::validation_runner::ValidationEvent;

use crate::ui::Theme;

/// Return value from TUI indicating user action.
#[derive(Debug)]
pub enum TuiAction {
    /// User quit normally
    Quit,
    /// User requested immediate process termination
    ForceQuit,
    /// User requested rerun validation
    Rerun,
}

pub use models::FileErrors;

use rendering::{
    render_error_details, render_file_list, render_footer, render_footer_streaming, render_header,
    render_header_streaming,
};
use state::{Redraw, RunPhase, TuiState};

/// Launch the validation TUI.
pub fn run_validation_tui(mut files: Vec<FileErrors>, theme: Theme) -> Result<TuiAction> {
    if files.is_empty() {
        println!("✓ No errors found!");
        return Ok(TuiAction::Quit);
    }

    // Ensure all errors have line/column information
    for file in &mut files {
        file.ensure_line_columns();
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut state = TuiState::new(files, theme);

    // Main event loop
    let result = run_static_app(&mut terminal, &mut state);

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

/// Launch the validation TUI with streaming error display.
///
/// Errors appear in real-time as validation progresses. User can cancel validation
/// by pressing 'c' or Ctrl+C. Files are kept sorted alphabetically.
pub fn run_validation_tui_streaming(
    events_rx: crossbeam_channel::Receiver<ValidationEvent>,
    cancel_tx: crossbeam_channel::Sender<()>,
    theme: Theme,
) -> Result<TuiAction> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state with empty file list (will populate as errors arrive)
    let mut state = TuiState::new(Vec::new(), theme);
    let mut phase = RunPhase::Running;
    let mut ctrl_c_count = 0usize;

    // Main event loop with non-blocking polls
    let result = loop {
        // Draw UI
        terminal.draw(|f| ui_streaming(f, &mut state, &phase))?;

        // Poll for keyboard input (non-blocking, 50ms timeout)
        if poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            // Clear any transient status message on keypress
            state.status_message = None;

            if !state.handle_common_key(key.code, key.modifiers) {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('c'), KeyModifiers::NONE) => {
                        cancel_tx.send(()).ok();
                    }
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        ctrl_c_count += 1;
                        cancel_tx.send(()).ok();
                        if ctrl_c_count >= 2 {
                            break Ok(TuiAction::ForceQuit);
                        }
                    }
                    (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => {
                        cancel_tx.send(()).ok();
                        break Ok(TuiAction::Quit);
                    }
                    // Rerun is offered for every stopped run, including one
                    // that aborted or lost files: re-running is exactly what a
                    // user wants after either.
                    (KeyCode::Char('r'), KeyModifiers::NONE) if phase.is_terminal() => {
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
            Ok(ValidationEvent::Errors(mut error_event)) => {
                // Enhance errors with full line context for miette display
                talkbank_model::enhance_errors_with_source(
                    &mut error_event.errors,
                    &error_event.source,
                );

                // Check if this file already exists in the list
                if let Some(existing) = state.files.iter_mut().find(|f| f.path == error_event.path)
                {
                    // File already exists - merge errors
                    existing.errors.extend(error_event.errors);
                } else {
                    // New file with errors - add to list
                    let mut file_errors = FileErrors {
                        path: error_event.path,
                        errors: error_event.errors,
                        source: error_event.source,
                    };

                    // Ensure line/column information
                    file_errors.ensure_line_columns();

                    // Add to state
                    state.files.push(file_errors);

                    // Keep files sorted alphabetically
                    state.files.sort_by(|a, b| a.path.cmp(&b.path));

                    // Update selection if this is the first file
                    if state.files.len() == 1 {
                        state.file_list_state.select(Some(0));
                        if !state.files[0].errors.is_empty() {
                            state.error_list_state.select(Some(0));
                        }
                    }
                }
            }
            Ok(ValidationEvent::Discovering) => {
                state.progress.discovering = true;
            }
            Ok(ValidationEvent::Started { total_files }) => {
                state.progress.total_files = total_files;
                state.progress.discovering = false;
            }
            Ok(ValidationEvent::RoundtripComplete(_)) => {}
            Ok(ValidationEvent::FileComplete(_)) => {
                state.progress.files_processed += 1;
                state.update_progress_display(Redraw::WhenStrideReached);
            }
            Ok(ValidationEvent::Finished(snapshot)) => {
                phase = RunPhase::Finished;
                record_final_counts(state, &snapshot);
            }
            Ok(ValidationEvent::FinishedIncomplete { stats, lost_files }) => {
                phase = RunPhase::Incomplete { lost_files };
                // The counts still describe what WAS processed and are worth
                // showing; the phase is what stops them being read as totals.
                record_final_counts(state, &stats);
            }
            Ok(ValidationEvent::Aborted(reason)) => {
                phase = RunPhase::Aborted {
                    reason: reason.to_string(),
                };
            }
            Err(crossbeam_channel::TryRecvError::Empty) => break,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                // BACKSTOP, kept deliberately. The runner's drop guard should
                // always deliver a terminal event before its senders close, so
                // reaching here without one means that guarantee broke. The
                // answer is still never "the run completed": an unexplained
                // silence is reported as an abort, which is the truthful
                // reading and leaves the counts unclaimed.
                if !phase.is_terminal() {
                    phase = RunPhase::Aborted {
                        reason: "The validator stopped without reporting a result.".to_owned(),
                    };
                }
                break;
            }
        }
    }
    phase
}

/// Copy a terminal snapshot's tallies into the progress display.
///
/// Shared by the complete and incomplete endings because both produce real
/// counts for the files they did process; what differs is the claim the UI is
/// entitled to make about them, which lives in [`RunPhase`], not here.
fn record_final_counts(
    state: &mut TuiState,
    snapshot: &talkbank_transform::validation_runner::ValidationStatsSnapshot,
) {
    state.progress.total_files = snapshot.total_files;
    state.progress.files_processed = snapshot.total_files;
    state.update_progress_display(Redraw::Immediately);
    state.progress.final_valid_files = Some(snapshot.valid_files);
    state.progress.final_invalid_files = Some(snapshot.invalid_files);
    state.progress.final_cache_hits = Some(snapshot.cache_hits);
    state.progress.final_cache_misses = Some(snapshot.cache_misses);
}

/// Run the main event loop for static validation.
fn run_static_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &mut TuiState,
) -> Result<TuiAction>
where
    <B as ratatui::backend::Backend>::Error: 'static + std::error::Error + Send + Sync,
{
    loop {
        terminal.draw(|f| ui(f, state))?;

        if let Event::Key(key) = event::read()? {
            // Clear any transient status message on keypress
            state.status_message = None;

            if !state.handle_common_key(key.code, key.modifiers) {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => {
                        return Ok(TuiAction::Quit);
                    }
                    (KeyCode::Char('r'), KeyModifiers::NONE) => {
                        return Ok(TuiAction::Rerun);
                    }
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        return Ok(TuiAction::Quit);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// UI rendering for streaming validation (shows validation status).
fn ui_streaming(f: &mut Frame, state: &mut TuiState, phase: &RunPhase) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Header with title + gauge
            Constraint::Min(0),    // Main content
            Constraint::Length(4), // Footer (action row + nav row)
        ])
        .split(f.area());

    render_header_streaming(f, chunks[0], state, phase);

    if state.files.is_empty() {
        // Nothing to browse yet. WHY this text is per-phase: "no errors found"
        // is a claim about every discovered file, and only a complete run has
        // examined every discovered file. An incomplete or aborted run says so
        // instead, so silence never reads as a clean bill of health.
        let msg = match phase {
            RunPhase::Finished => format!(
                "✓ {} files validated, no errors found! Press 'q' to quit.",
                state.progress.total_files
            ),
            RunPhase::Incomplete { lost_files } => format!(
                "⚠ Validation did not finish: {lost_files} of {} files were never checked. \
                 No errors in the rest. Press 'r' to re-run, 'q' to quit.",
                state.progress.total_files
            ),
            RunPhase::Aborted { reason } => {
                format!("⚠ {reason} Press 'r' to re-run, 'q' to quit.")
            }
            RunPhase::Running => {
                if state.progress.discovering {
                    "Discovering files... (press 'c' to cancel)".to_string()
                } else if state.progress.total_files > 0 {
                    "Validating files... (press 'c' to cancel)".to_string()
                } else {
                    "Validating... (press 'c' to cancel)".to_string()
                }
            }
        };

        let color = match phase {
            RunPhase::Finished => state.theme.header_ok,
            // A run that lost files or died is an error condition in its own
            // right, whatever the files it did manage to check looked like.
            RunPhase::Incomplete { .. } | RunPhase::Aborted { .. } => state.theme.header_err,
            RunPhase::Running => state.theme.header_progress,
        };

        let paragraph = Paragraph::new(msg)
            .style(Style::default().fg(color))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(paragraph, chunks[1]);
    } else {
        // Split main content into two panes
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // File list (left)
                Constraint::Percentage(70), // Error details (right)
            ])
            .split(chunks[1]);

        render_file_list(f, main_chunks[0], state);
        if let Some(metrics) = render_error_details(f, main_chunks[1], state) {
            state.apply_detail_metrics(metrics);
        }
    }

    render_footer_streaming(f, chunks[2], state, phase);
}

/// UI rendering for static validation.
fn ui(f: &mut Frame, state: &mut TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(4), // Footer (action row + nav row)
        ])
        .split(f.area());

    render_header(f, chunks[0], state);

    // Split main content into two panes
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // File list (left)
            Constraint::Percentage(70), // Error details (right)
        ])
        .split(chunks[1]);

    render_file_list(f, main_chunks[0], state);
    if let Some(metrics) = render_error_details(f, main_chunks[1], state) {
        state.apply_detail_metrics(metrics);
    }

    render_footer(f, chunks[2], state);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// A run whose stream dies without ever reporting `Finished` must NOT be
    /// rendered as a completed run.
    ///
    /// The TUI's only terminal signal used to be "the channel closed", which
    /// conflates "every file was validated" with "the validator died holding
    /// partial counts". Both then drew the same "Done | N files with errors /
    /// M files" header, so a dead run advertised its partial tallies as final
    /// and the user had no way to tell.
    #[test]
    fn a_stream_that_dies_without_finishing_is_not_reported_as_complete() {
        let (events_tx, events_rx) = crossbeam_channel::unbounded::<ValidationEvent>();
        let mut state = TuiState::new(Vec::new(), Theme::default());

        events_tx.send(ValidationEvent::Discovering).unwrap();
        events_tx
            .send(ValidationEvent::Started { total_files: 9 })
            .unwrap();
        drop(events_tx);

        let phase = drain_validation_events(&events_rx, &mut state, RunPhase::Running);

        assert!(
            matches!(phase, RunPhase::Aborted { .. }),
            "a run that never sent a terminal event must be reported as aborted, got {phase:?}"
        );
    }

    /// A run the RUNNER reports as incomplete must be shown as incomplete, not
    /// as a clean finish: its counts describe only the files it managed to
    /// open, so "no errors found" would be a claim about files nobody read.
    #[test]
    fn a_run_that_lost_files_is_shown_as_incomplete() {
        let (events_tx, events_rx) = crossbeam_channel::unbounded::<ValidationEvent>();
        let mut state = TuiState::new(Vec::new(), Theme::default());

        events_tx
            .send(ValidationEvent::FinishedIncomplete {
                stats: snapshot(5, 3),
                lost_files: 2,
            })
            .unwrap();
        drop(events_tx);

        let phase = drain_validation_events(&events_rx, &mut state, RunPhase::Running);

        assert_eq!(
            phase,
            RunPhase::Incomplete { lost_files: 2 },
            "a run that lost files must not be reported as Finished"
        );
    }

    /// An explicit abort from the runner carries the runner's own reason, so
    /// every surface quotes one wording rather than composing its own.
    #[test]
    fn an_abort_event_carries_the_runners_reason() {
        let (events_tx, events_rx) = crossbeam_channel::unbounded::<ValidationEvent>();
        let mut state = TuiState::new(Vec::new(), Theme::default());

        events_tx
            .send(ValidationEvent::Aborted(
                talkbank_transform::validation_runner::AbortReason::Panicked,
            ))
            .unwrap();
        drop(events_tx);

        let phase = drain_validation_events(&events_rx, &mut state, RunPhase::Running);

        match phase {
            RunPhase::Aborted { reason } => assert_eq!(
                reason,
                talkbank_transform::validation_runner::AbortReason::Panicked.to_string(),
                "the TUI must show the runner's reason verbatim"
            ),
            other => panic!("expected an aborted phase, got {other:?}"),
        }
    }

    /// The normal path still reports completion, so the fix above cannot be
    /// satisfied by simply never completing.
    #[test]
    fn a_stream_that_finishes_is_reported_as_complete() {
        let (events_tx, events_rx) = crossbeam_channel::unbounded::<ValidationEvent>();
        let mut state = TuiState::new(Vec::new(), Theme::default());

        events_tx
            .send(ValidationEvent::Finished(snapshot(2, 2)))
            .unwrap();
        drop(events_tx);

        let phase = drain_validation_events(&events_rx, &mut state, RunPhase::Running);

        assert_eq!(
            phase,
            RunPhase::Finished,
            "a run that sent Finished must be shown as complete"
        );
    }

    /// Snapshot with `valid_files` of `total_files` accounted for.
    fn snapshot(
        total_files: usize,
        valid_files: usize,
    ) -> talkbank_transform::validation_runner::ValidationStatsSnapshot {
        talkbank_transform::validation_runner::ValidationStatsSnapshot {
            total_files,
            valid_files,
            invalid_files: 0,
            cache_hits: 0,
            cache_misses: valid_files,
            parse_errors: 0,
            roundtrip_passed: 0,
            roundtrip_failed: 0,
            cancelled: false,
        }
    }
}
