//! Standard streamed validation runtime with text, JSON, and TUI frontends.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::commands::validate::cache::{CacheInit, initialize_validation_cache};
use crate::commands::validate_parallel::renderer::{
    AuditFileError, RenderedRun, create_presentation_renderer, render_run,
};
use crate::commands::validate_parallel::shared::{
    InteractiveEnd, InterruptReport, RunNotice, ValidationOutcome,
};
use crate::commands::validate_parallel::{ValidateDirectoryOptions, ValidationPresentation};
use crate::ui::{TuiAction, run_validation_tui_streaming};
use talkbank_transform::validation_runner::{
    Canceller, ValidationConfig, ValidationRun, validate_arguments_streaming,
};

/// Run one `chatter validate`: expand the command-line `paths` the one way,
/// open the cache the run's policy allows, and drive the TUI or one
/// renderer over the runner's event stream. Files and directories alike
/// take this path.
///
/// `summary_label` is cosmetic for the summary line; the actual
/// validation operates entirely on what `paths` expand to.
pub fn run_validation_runtime(
    paths: Vec<PathBuf>,
    summary_label: PathBuf,
    options: ValidateDirectoryOptions,
) -> ValidationOutcome {
    let ValidateDirectoryOptions {
        rules,
        execution,
        presentation,
        suppress,
        notices,
    } = options;

    // The two halves of what the user asked for, kept apart because they are
    // different kinds of thing and only one of them may reach the cache key.
    //
    // WHAT IS COMPUTED: `--strict-linkers` turns on checks a lenient run never
    // reaches, so it changes the diagnostics that exist.
    let rule_selection = talkbank_model::RuleSelection::new().with_linkers(rules.linkers);

    // WHAT IS SHOWN: `--suppress` hides computed diagnostics from the reader.
    // It is applied once, inside the worker, at the boundary where results are
    // handed to this runtime, so the worker's tallies already reflect it and
    // nothing filters events afterwards. Named `display_policy` because
    // `presentation` here is the output SURFACE (text, JSON, audit file).
    let display_policy = suppress.codes().iter().fold(
        talkbank_transform::PresentationPolicy::new(),
        |policy, code| policy.disable(*code),
    );

    let config = ValidationConfig {
        alignment: rules.alignment,
        jobs: execution.jobs,
        roundtrip: rules.roundtrip,
        // The runner owns the limit: it counts errors from each file's
        // status and stops itself, so the TUI honours it too.
        error_limit: execution.error_limit,
        parser_kind: rules.parser_kind,
        rules: rule_selection,
        presentation: display_policy,
    };

    // The arguments, expanded the one way. What could not be read goes to
    // the runner, which reports each as a read error in the run's results.
    let input = talkbank_transform::paths::expand_transcript_arguments(&paths);
    // An audit is a REPORTING sweep: `execution.cache` is then ReadOnly, so
    // the run gets a handle with no write method, opening it prunes nothing,
    // and `--force` (which would clear rows) was refused at the command line.
    //
    // The cache key covers the active RULE SET and nothing else: a row records
    // what validation found, which `--strict-linkers` changes and `--suppress`
    // does not, so the cache is opened for `config`'s identity and bound to
    // it: the run cannot read a cache opened for other rules. Opening the
    // cache returns what maintenance did as events, held until the surface
    // that says them exists.
    let CacheInit {
        run,
        events: cache_events,
    } = initialize_validation_cache(input.files(), execution.cache, config);

    // The TUI owns its own loop; every other surface is a renderer over the
    // one event loop below. Both say the run's notices and cache events.
    let presentation = match presentation {
        ValidationPresentation::Tui { theme } => {
            let notes = notices
                .iter()
                .map(RunNotice::sentence)
                .chain(cache_events.iter().map(|event| event.sentence()))
                .collect();
            return run_tui_loop(input, &paths, &run, TuiSettings { theme, notes });
        }
        ValidationPresentation::Streamed(presentation) => presentation,
    };

    // Build the renderer BEFORE starting the worker pool. Audit mode creates
    // its output file here, and creating it after `validate_arguments_streaming`
    // means an unwritable `--audit` path is only discovered once real parsing
    // is already under way.
    //
    // Renderer choice is the ONLY thing audit mode changes; everything else in
    // this function is shared, which is what keeps `--suppress`, `--parser`,
    // `--strict-linkers`, `--roundtrip`, `--jobs` and `--max-errors` working
    // identically in both modes.
    let mut renderer = match create_presentation_renderer(&presentation) {
        Ok(renderer) => renderer,
        Err(AuditFileError { path, error }) => {
            return ValidationOutcome::AuditFileUnwritable { path, error };
        }
    };
    for notice in &notices {
        renderer.handle_notice(notice);
    }
    for event in &cache_events {
        renderer.handle_cache_event(event);
    }

    let (events_rx, canceller) = validate_arguments_streaming(input, &run);
    if let Err(error) = install_ctrlc_handler(&canceller, renderer.interrupt_report()) {
        renderer.handle_notice(&RunNotice::InterruptUnavailable(error.to_string()));
    }

    let RenderedRun { ending, output } = render_run(
        events_rx,
        renderer.as_mut(),
        &summary_label,
        rules.roundtrip,
    );
    ValidationOutcome::Streamed { ending, output }
}

/// What the TUI shows besides the run: its theme and the run's notes.
struct TuiSettings {
    theme: crate::ui::Theme,
    notes: Vec<String>,
}

/// Drive the interactive TUI, supporting reruns until the user exits.
///
/// The first run takes the arguments already expanded for the cache; a
/// rerun expands the same `paths` again, so it sees the files as they are
/// now (one added or removed since is found or reported).
fn run_tui_loop(
    first_input: talkbank_transform::paths::ExpandedArguments,
    paths: &[PathBuf],
    run: &ValidationRun,
    settings: TuiSettings,
) -> ValidationOutcome {
    let mut input = first_input;
    loop {
        let (events_rx, canceller) = validate_arguments_streaming(input, run);
        match run_validation_tui_streaming(
            events_rx,
            canceller,
            settings.theme.clone(),
            &settings.notes,
        ) {
            // The interactive surface has already SHOWN the user how the run
            // ended; the outcome carries that ending, so the exit status says
            // the same thing.
            Ok(TuiAction::Quit(phase)) => {
                return ValidationOutcome::Interactive(InteractiveEnd::Closed(phase));
            }
            Ok(TuiAction::ForceQuit) => std::process::exit(130),
            Ok(TuiAction::Rerun) => {
                eprintln!("Re-running validation...");
                input = talkbank_transform::paths::expand_transcript_arguments(paths);
            }
            // The terminal failed, so nothing reliable was shown.
            Err(error) => {
                eprintln!("TUI error: {}", error);
                return ValidationOutcome::Interactive(InteractiveEnd::TerminalFailed);
            }
        }
    }
}

/// Install the Ctrl+C handler used by non-interactive validation modes.
///
/// The first Ctrl-C asks the run to stop (it then ends `Stopped`, which the
/// renderer reports); the second force-quits. Whether either says anything
/// on stderr is the renderer's answer (`report`): JSON mode keeps stderr
/// empty. A handler that cannot be installed is returned, for the renderer
/// to say in its own channel.
fn install_ctrlc_handler(
    canceller: &Canceller,
    report: InterruptReport,
) -> Result<(), ctrlc::Error> {
    let presses = AtomicUsize::new(0);
    let canceller = canceller.clone();
    ctrlc::set_handler(move || match presses.fetch_add(1, Ordering::SeqCst) {
        0 => {
            canceller.cancel();
            match report {
                InterruptReport::OnStderr => {
                    eprintln!("\nCancelling validation... (press Ctrl+C again to force quit)");
                }
                InterruptReport::Silent => {}
            }
        }
        _ => {
            match report {
                InterruptReport::OnStderr => eprintln!("\nForce quitting."),
                InterruptReport::Silent => {}
            }
            std::process::exit(130);
        }
    })
}
