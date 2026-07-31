//! Standard streamed validation runtime with text, JSON, and TUI frontends.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::commands::validate::cache::initialize_validation_cache;
use crate::commands::validate_parallel::renderer::create_presentation_renderer;
use crate::commands::validate_parallel::shared::empty_stats;
use crate::commands::validate_parallel::{
    ValidateDirectoryOptions, ValidationPresentation, ValidationTraversalMode,
};
use crate::ui::{TuiAction, run_validation_tui_streaming};
use talkbank_transform::validation_runner::{
    CacheMode, DirectoryMode, ValidationConfig, ValidationEvent, ValidationStatsSnapshot,
    validate_files_streaming,
};

/// Run the standard validation flow on a pre-collected list of CHAT
/// files. Both `chatter validate dir/` and
/// `chatter validate a.cha b.cha c.cha` resolve to this single path
/// after the CLI has walked input args into a flat file list, the
/// fix for the prior divergence between per-file and per-directory
/// code paths.
///
/// `summary_label` is cosmetic for the summary line; the actual
/// validation operates entirely on `files`.
pub fn run_validation_runtime(
    files: Vec<PathBuf>,
    summary_label: PathBuf,
    options: ValidateDirectoryOptions,
) -> ValidationStatsSnapshot {
    let ValidateDirectoryOptions {
        rules,
        traversal,
        execution,
        presentation,
        suppress,
    } = options;

    // An up-front note naming WHAT is being suppressed, replacing the old
    // post-hoc "Suppressed: N file(s)..." summary line. That per-file count
    // no longer exists to compute: a suppressed code is never emitted, so
    // the worker cannot tell "genuinely clean" apart from "clean because N
    // codes are disabled" after the fact. This is derived from the active
    // config at zero cost and is strictly more informative anyway, since it
    // says which codes are suppressed rather than how many files happened
    // to be affected this run. Deduped and sorted: `suppress` can contain
    // the same code twice (e.g. `--suppress xphon,E736` when E736 is
    // already a member of the `xphon` group). stderr, not stdout, so it
    // never lands inside `--format json` output.
    if !suppress.is_empty() {
        let mut codes: Vec<&str> = suppress.iter().map(|code| code.as_str()).collect();
        codes.sort_unstable();
        codes.dedup();
        eprintln!(
            "note: suppressing {} code(s): {}",
            codes.len(),
            codes.join(", ")
        );
    }

    // Suppression joins the RULE SET here, upstream of validation: a
    // suppressed code is folded into `model_config` as a disabled code, so
    // the worker's parser/model layer never emits it at all. Classification
    // (Valid/Invalid) then happens exactly once, inside the worker; there is
    // no separate post-hoc event-filtering pass left to reconcile with the
    // worker's own tallies (the previous `filter_suppressed_events` did that
    // reconciliation, and a double-adjustment of it once produced a
    // plausible `Invalid: 0` on a corpus with 15 genuinely invalid files).
    let mut model_config = talkbank_model::ValidationConfig::new();
    for code in &suppress {
        model_config = model_config.disable(*code);
    }
    if rules.strict_linkers {
        model_config = model_config.with_strict_linkers();
    }

    let config = ValidationConfig {
        check_alignment: rules.alignment.enabled(),
        jobs: execution.jobs,
        // An audit is a REPORTING sweep: it may benefit from cached work but
        // must not rewrite shared cache state as a side effect of producing a
        // report. That contract predates this runtime (the old standalone
        // audit pipeline read the cache and never wrote it) and is asserted by
        // the `..._without_cache_writes` tests.
        cache: match presentation {
            ValidationPresentation::Audit { .. } => CacheMode::ReadOnly,
            ValidationPresentation::Streaming(_) => CacheMode::Enabled,
        },
        directory: match traversal {
            ValidationTraversalMode::Recursive => DirectoryMode::Recursive,
            ValidationTraversalMode::SingleFile => DirectoryMode::SingleFile,
        },
        roundtrip: rules.roundtrip.enabled(),
        parser_kind: rules.parser_kind,
        model_config,
    };

    // The cache key MUST include the active rule set: suppression AND
    // strict-linkers both change which files count as Valid, so a cache row
    // produced under one active config is not a valid answer for a
    // different one. Pass `&config.model_config` (the exact value the
    // worker below validates with), never a re-derived summary of it, so
    // the cache key and the validation behavior cannot drift apart. See
    // `initialize_validation_cache` and `RulesVersion::current_with_config`.
    let cache = initialize_validation_cache(&files, execution.cache_refresh, &config.model_config);

    // The TUI is a streaming-only surface: audit mode writes a file and has no
    // interactive presentation to hand a terminal.
    if let ValidationPresentation::Streaming(output) = &presentation
        && output.interface.uses_tui()
    {
        return run_tui_loop(files, &summary_label, &config, cache, output.theme.clone());
    }

    // Build the renderer BEFORE starting the worker pool. Audit mode creates
    // its output file here, and creating it after `validate_files_streaming`
    // means an unwritable `--audit` path is only discovered once real parsing
    // is already under way.
    //
    // Renderer choice is the ONLY thing audit mode changes; everything else in
    // this function is shared, which is what keeps `--suppress`, `--parser`,
    // `--strict-linkers`, `--roundtrip`, `--jobs` and `--max-errors` working
    // identically in both modes.
    let mut renderer = create_presentation_renderer(&presentation);

    let (events_rx, cancel_tx) = validate_files_streaming(files, &config, cache.clone());
    install_ctrlc_handler(&cancel_tx);

    let mut final_stats = None;
    let mut error_count = 0usize;
    let mut files_completed = 0usize;

    for event in events_rx {
        match event {
            ValidationEvent::Discovering => renderer.handle_discovering(),
            ValidationEvent::Started { total_files } => renderer.handle_started(total_files),
            ValidationEvent::Errors(error_event) => {
                error_count = error_count.saturating_add(renderer.handle_errors(&error_event));
                cancel_if_error_limit_reached(&cancel_tx, execution.max_errors, error_count);
            }
            ValidationEvent::RoundtripComplete(rt_event) => {
                error_count =
                    error_count.saturating_add(renderer.handle_roundtrip_complete(&rt_event));
                cancel_if_error_limit_reached(&cancel_tx, execution.max_errors, error_count);
            }
            ValidationEvent::FileComplete(file_event) => {
                files_completed += 1;
                renderer.handle_file_complete(&file_event, files_completed);
            }
            ValidationEvent::Finished(snapshot) => {
                final_stats = Some(snapshot);
            }
        }
    }

    let stats = match final_stats {
        Some(stats) => stats,
        None => {
            eprintln!("Error: No validation stats received");
            std::process::exit(1);
        }
    };

    // These are the worker's own tallies, with no post-hoc adjustment: a
    // suppressed code was never emitted, so a fully-suppressed file was
    // simply Valid from the worker's point of view. Regression test:
    // `suppression_does_not_hide_other_files_invalid_count`.
    renderer.handle_finished(&stats, files_completed, execution.max_errors, error_count);
    renderer.print_summary(&summary_label, &stats, rules.roundtrip.enabled());

    stats
}

/// Drive the interactive TUI, supporting reruns until the user exits.
///
/// Re-streaming on Rerun re-uses the same file list so the rerun
/// honors the same input the user originally asked for.
fn run_tui_loop(
    files: Vec<PathBuf>,
    summary_label: &Path,
    config: &ValidationConfig,
    cache: Option<Arc<talkbank_transform::CachePool>>,
    theme: crate::ui::Theme,
) -> ValidationStatsSnapshot {
    let _ = summary_label; // reserved for future "rerunning <label>..." messaging
    loop {
        let (events_rx, cancel_tx) = validate_files_streaming(files.clone(), config, cache.clone());
        match run_validation_tui_streaming(events_rx, cancel_tx, theme.clone()) {
            Ok(TuiAction::Quit) => return empty_stats(false),
            Ok(TuiAction::ForceQuit) => std::process::exit(130),
            Ok(TuiAction::Rerun) => {
                eprintln!("Re-running validation...");
            }
            Err(error) => {
                eprintln!("TUI error: {}", error);
                return empty_stats(true);
            }
        }
    }
}

/// Install the Ctrl+C handler used by non-interactive validation modes.
fn install_ctrlc_handler(cancel_tx: &crossbeam_channel::Sender<()>) {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel_count = Arc::new(AtomicUsize::new(0));
    let cancelled_clone = Arc::clone(&cancelled);
    let cancel_count_clone = Arc::clone(&cancel_count);
    let cancel_tx_clone = cancel_tx.clone();

    if let Err(error) = ctrlc::set_handler(move || {
        let count = cancel_count_clone.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            let was_cancelled = cancelled_clone.swap(true, Ordering::SeqCst);
            cancel_tx_clone.send(()).ok();
            if !was_cancelled {
                eprintln!("\nCancelling validation... (press Ctrl+C again to force quit)");
            }
        } else {
            eprintln!("\nForce quitting.");
            std::process::exit(130);
        }
    }) {
        eprintln!("Error setting Ctrl+C handler: {}", error);
    }
}

/// Cancel the run when the configured error limit has been reached.
fn cancel_if_error_limit_reached(
    cancel_tx: &crossbeam_channel::Sender<()>,
    max_errors: Option<usize>,
    error_count: usize,
) {
    if let Some(limit) = max_errors
        && error_count >= limit
    {
        cancel_tx.send(()).ok();
    }
}
