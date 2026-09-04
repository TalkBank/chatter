//! Single-file validation flow with error-sink wiring.
//!
//! Implements `chatter validate <file>` by composing the cache, parser, validator,
//! and output layers. Three output modes are supported: streaming terminal (text),
//! structured JSON, and interactive TUI with rerun capability.
//!
//! # Error-sink architecture
//!
//! In text mode, a [`TeeErrorSink`] mirrors each error to both a [`TerminalErrorSink`]
//! (for immediate display) and an [`ErrorCollector`] (for caching and exit-code logic).
//! JSON and TUI modes collect into an [`ErrorCollector`] alone and format after the parse
//! completes.

use std::fs;
use std::path::PathBuf;

use talkbank_model::{ErrorCollector, ErrorSink, ParseValidateOptions, TeeErrorSink};
use talkbank_transform::parse_and_validate_streaming_for_path;

use crate::cli::OutputFormat;
use crate::commands::{AlignmentValidationMode, CacheRefreshMode, ValidationInterface};
use crate::output::TerminalErrorSink;
use crate::ui::Theme;

use super::cache::{get_cached_validation, initialize_validation_cache, set_cached_validation};
use super::output::output_validation_result;

/// Outcome of validating one file via [`validate_file`].
///
/// Per the project no-boolean-blindness rule, this is an enum rather
/// than `bool`, `validate_file(...) == Valid` reads correctly
/// without the caller having to remember which polarity `true` means.
///
/// Used today only by `chatter watch`, which discards the outcome
/// (the per-file watch UI handles its own state). Multi-file CLI
/// invocations route through `validate_paths_parallel` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileValidationOutcome {
    /// File parsed and validated cleanly (or was a cache hit).
    Valid,
    /// File produced one or more validation errors. Errors were
    /// already streamed to the terminal or printed in JSON form
    /// inline; nothing further is deferred to the caller.
    Invalid,
}

/// The rule set `validate_file` validates against, and the ONLY input to its
/// cache key.
///
/// `suppress` is deliberately not a parameter: a suppression list changes what
/// the user is shown, never what the validator computes, so it cannot change
/// which cache row answers this run. Folding it in is the v0.6.0 regression.
fn build_rule_selection(strict_linkers: bool) -> talkbank_model::RuleSelection {
    let rules = talkbank_model::RuleSelection::new();
    if strict_linkers {
        rules.with_strict_linkers()
    } else {
        rules
    }
}

/// The display policy `validate_file` applies to computed diagnostics.
///
/// One owner for the `--suppress` list, so the filtering that happens on the
/// way to the terminal and any other consumer of the same list cannot disagree.
/// (They did: a string-set filter and a typed cache-key builder each
/// interpreted the list separately, and only one of them was ever right.)
///
/// Values are matched case-insensitively against real [`talkbank_model::ErrorCode`]s, and a
/// value naming no real code is skipped: an unrecognised string would never
/// have matched a diagnostic either. Unlike `validate_paths_parallel`'s
/// `--suppress` there is no named-group expansion (`xphon`, etc.); `chatter
/// watch` has no `--suppress` flag today, so there is no group syntax to
/// support yet. Add it here (mirroring `expand_suppress_groups`) if that
/// changes.
fn build_presentation_policy(suppress: &[String]) -> talkbank_transform::PresentationPolicy {
    let mut policy = talkbank_transform::PresentationPolicy::new();
    for raw in suppress {
        if let Some(code) = talkbank_model::ErrorCode::parse_exact(&raw.to_uppercase()) {
            policy = policy.disable(code);
        }
    }
    policy
}

/// Present one cache event on the single-file path.
///
/// This path has no `ValidationRenderer`: single-file validation predates the
/// streaming runtime and presents through its own output functions. That
/// duplication is the real defect and is bigger than this change; keeping the
/// decision in ONE function here makes it visible rather than spreading a
/// `match format` over every call site.
fn report_cache_event(event: &super::cache::CacheEvent, format: OutputFormat) {
    match format {
        OutputFormat::Json => println!("{}", event.record()),
        OutputFormat::Text => eprintln!("{}", event.sentence()),
    }
}

/// Validate a single CHAT file with optional alignment and caching behavior.
///
/// This routine encapsulates the CLI behavior for the `validate` subcommand when the target
/// path is a single file. It manages the shared `UnifiedCache`, optionally purges entries when
/// `--force` is provided, reads the CHAT content, and builds `ParseValidateOptions` that align
/// with the Main Tier and Dependent Tier rules described in the CHAT manual. Errors are streamed
/// through the appropriate sinks (JSON, TUI, or terminal).
///
/// Returns [`FileValidationOutcome::Valid`] / [`FileValidationOutcome::Invalid`] so callers
/// processing multiple files can keep going instead of having this function `process::exit`
/// after the first invalid file. Inability-to-validate conditions (unreadable file, internal
/// parser error, TUI subsystem failure) still call `process::exit(1)` since the per-file run
/// genuinely cannot continue; the TUI `ForceQuit` action still calls `process::exit(130)`
/// because the user explicitly asked the whole command to stop.
//
// TODO: roll these args into a ValidateFileConfig struct. For now the
// argument count is high but each is a distinct CLI surface concern.
#[allow(clippy::too_many_arguments)]
pub fn validate_file(
    path: &PathBuf,
    format: OutputFormat,
    alignment: AlignmentValidationMode,
    cache_refresh: CacheRefreshMode,
    quiet: bool,
    interface: ValidationInterface,
    _theme: Theme,
    suppress: &[String],
    strict_linkers: bool,
) -> FileValidationOutcome {
    // `theme` is no longer used inside `validate_file` because the TUI
    // launch moved to the multi-file driver in `run_validate_command`
    // (so all files end up in one consolidated TUI). Kept in the
    // signature so the public API and the watch-mode caller don't break.
    let check_alignment = alignment.enabled();

    // The two halves, mirroring `validate_paths_parallel`'s runtime: only the
    // rule selection keys the cache, and the presentation policy is applied to
    // diagnostics that have already been computed and already decided what gets
    // cached.
    let rule_selection = build_rule_selection(strict_linkers);
    let presentation = build_presentation_policy(suppress);
    let cache_init =
        initialize_validation_cache(std::slice::from_ref(path), cache_refresh, &rule_selection);
    // This path has no `ValidationRenderer`: single-file validation predates
    // the streaming runtime and presents through its own output functions.
    // That duplication is the real defect here and is bigger than this change;
    // rendering the events locally makes it VISIBLE in one place instead of
    // hiding it inside a cache function that used to write to stderr whatever
    // the caller wanted.
    let (cache, cache_events) = cache_init.into_parts();
    for event in &cache_events {
        report_cache_event(event, format);
    }

    // Try to get cached results.
    // On Some(true): cached valid, skip revalidation.
    // On Some(false) or None: revalidate.
    if get_cached_validation(cache.as_ref(), path, check_alignment) == Some(true) {
        // Cached success: output and return without revalidating.
        // (TUI mode included: a cached-valid file has no errors to
        // contribute to the consolidated TUI.)
        output_validation_result(path, &[], None, format, true, quiet);
        return FileValidationOutcome::Valid;
    }

    // Not in cache or cache disabled - validate file
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file {:?}: {}", path, e);
            std::process::exit(1);
        }
    };

    // Build pipeline options (alignment adds `%wor`/`%pho` checks while validation-only skips them).
    // Alignment is on by default; use --skip-alignment to disable
    let mut options = if check_alignment {
        ParseValidateOptions::default().with_alignment()
    } else {
        ParseValidateOptions::default().with_validation()
    };
    if strict_linkers {
        options = options.with_strict_linkers();
    }

    // Use different error sinks based on output format and TUI mode
    // JSON/TUI needs structured values, interactive CLI streams to terminal plus collecting.
    let mut errors = if matches!(format, OutputFormat::Json) || interface.uses_tui() {
        // JSON mode or TUI mode: collect errors for structured output or TUI display
        let error_sink = ErrorCollector::new();

        match parse_and_validate_streaming_for_path(path, &content, options.clone(), &error_sink) {
            Ok(_)
            | Err(
                talkbank_transform::PipelineError::Parse(_)
                | talkbank_transform::PipelineError::Validation(_),
            ) => error_sink.into_vec(),
            Err(e) => {
                if matches!(format, OutputFormat::Json) {
                    let json_output = serde_json::json!({
                        "file": path.to_string_lossy(),
                        "status": "error",
                        "error": format!("{}", e)
                    });
                    match serde_json::to_string_pretty(&json_output) {
                        Ok(serialized) => println!("{}", serialized),
                        Err(err) => {
                            eprintln!("Error serializing JSON output: {}", err);
                        }
                    }
                } else {
                    eprintln!("Error: {}", e);
                }
                std::process::exit(1);
            }
        }
    } else if !presentation.shows_everything() {
        // Text mode with suppression: collect first, then filter, then display
        // (cannot stream-and-suppress because TerminalErrorSink prints immediately)
        let error_sink = ErrorCollector::new();

        match parse_and_validate_streaming_for_path(path, &content, options.clone(), &error_sink) {
            Ok(_)
            | Err(
                talkbank_transform::PipelineError::Parse(_)
                | talkbank_transform::PipelineError::Validation(_),
            ) => error_sink.into_vec(),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Interactive mode (not TUI): stream errors immediately to terminal AND collect for caching/output
        let terminal_sink = TerminalErrorSink::new(path, &content);
        let collecting_sink = ErrorCollector::new();
        let tee_sink = TeeErrorSink::new(&terminal_sink, &collecting_sink);

        match parse_and_validate_streaming_for_path(path, &content, options.clone(), &tee_sink) {
            Ok(_)
            | Err(
                talkbank_transform::PipelineError::Parse(_)
                | talkbank_transform::PipelineError::Validation(_),
            ) => collecting_sink.into_vec(),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Track whether the sink above streamed errors to stderr as it
    // went. Only the no-suppress / text-mode branch does so; the
    // JSON, TUI, and text+suppress branches all collected silently
    // and therefore owe the user a printed rendering below. Before
    // `resolve_suppress` defaulted `xphon` on, the suppress list was
    // typically empty in interactive runs, so the streamed branch
    // covered almost every case and this distinction wasn't visible
    // in test fixtures. It became visible once xphon suppression
    // fired by default (2026-04-21).
    let errors_streamed_already = matches!(format, OutputFormat::Text)
        && !interface.uses_tui()
        && presentation.shows_everything();

    // Enhance errors with source context for proper miette display (TUI, JSON output, etc.)
    talkbank_model::enhance_errors_with_source(&mut errors, &content);

    // THE CACHED FACT comes from the COMPLETE diagnostic set, before any
    // display policy touches it: "did this file produce any diagnostic at all",
    // which is the same answer under every `--suppress` list and so is safely
    // shared between runs that differ only in one.
    if let Some(event) =
        set_cached_validation(cache.as_ref(), path, check_alignment, errors.is_empty())
    {
        report_cache_event(&event, format);
    }

    // Only now the reader's view.
    let errors = presentation.apply_all(errors);

    // TUI mode in this single-file path is unused by the modern CLI
    // (multi-file invocations route through `validate_paths_parallel`,
    // which uses the streaming TUI). `chatter watch` is the only
    // remaining caller and it always passes `ValidationInterface::Plain`.
    // Keep a defensive branch that prints a plain-text summary so
    // any future TUI=true single-file caller still produces useful
    // output without launching a TUI from this code path.
    if interface.uses_tui() {
        if errors.is_empty() {
            println!("✓ No errors found in {}", path.display());
            return FileValidationOutcome::Valid;
        }
        // Key the headline on hard-error count: a file whose only
        // diagnostics are warnings is valid CHAT and must not be
        // headlined as an error (presentation-only; validity is decided
        // by the parallel pipeline that the modern CLI actually uses).
        if crate::output::has_hard_error(&errors) {
            eprintln!(
                "✗ Errors found in {} ({} error(s))",
                path.display(),
                errors.len()
            );
        } else {
            eprintln!(
                "⚠ Warnings in {} ({} warning(s))",
                path.display(),
                errors.len()
            );
        }
        return FileValidationOutcome::Invalid;
    }

    // Non-TUI output paths.
    let source_for_print = if matches!(format, OutputFormat::Text) {
        Some(content.as_str())
    } else {
        None
    };

    // Text mode normally streams errors live via `TerminalErrorSink`;
    // when that happened, re-printing through
    // `output_validation_result` would duplicate every diagnostic.
    // The suppressed branch collected silently, however, so in that
    // case we still owe the user a printed rendering.
    if matches!(format, OutputFormat::Text) && !errors.is_empty() {
        if !errors_streamed_already {
            let terminal_sink = TerminalErrorSink::new(path, &content);
            for error in &errors {
                terminal_sink.report(error.clone());
            }
        }
        if !quiet && crate::output::should_show_cascading_hint(&errors) {
            eprintln!("{}", crate::output::CASCADING_HINT);
        }
        return FileValidationOutcome::Invalid;
    }

    output_validation_result(path, &errors, source_for_print, format, false, quiet);
    if errors.is_empty() {
        FileValidationOutcome::Valid
    } else {
        FileValidationOutcome::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed stand-in parser/grammar fingerprint: these tests isolate the
    /// rule-selection dimension, so the parser dimension is held constant.
    const TEST_PARSER_FINGERPRINT: &str = "test-fingerprint";

    /// Two runs differing only in `--suppress` must land on the SAME cache key.
    ///
    /// The v0.6.0 regression, at the seam `validate_file` actually uses.
    /// `chatter watch` (its only caller) has no `--suppress` flag today, so the
    /// bug cannot be reproduced by varying that command's arguments; this pins
    /// the property one level down, where the key is composed.
    ///
    /// It is a weaker guard than the type split itself: the reason the two
    /// calls below cannot differ is that `build_rule_selection` has no
    /// `suppress` parameter to pass. The test documents the property; the
    /// signature enforces it.
    #[test]
    fn a_suppress_list_cannot_reach_the_cache_key() {
        let rules = build_rule_selection(false);
        let key = talkbank_transform::RulesVersion::current_with_rule_selection(
            &rules,
            TEST_PARSER_FINGERPRINT,
        );

        let suppressing = build_presentation_policy(&["E736".to_string()]);
        assert!(
            !suppressing.shows_everything(),
            "the policy under test must actually suppress something"
        );

        assert_eq!(
            key,
            talkbank_transform::RulesVersion::current_with_rule_selection(
                &build_rule_selection(false),
                TEST_PARSER_FINGERPRINT
            ),
            "a display preference must not partition the cache"
        );
    }

    /// `strict_linkers` genuinely changes which checks run, so it must still
    /// change the key. The regression guard on the other side of the split.
    #[test]
    fn strict_linkers_changes_the_cache_key() {
        let lenient = build_rule_selection(false);
        let strict = build_rule_selection(true);
        assert_ne!(
            talkbank_transform::RulesVersion::current_with_rule_selection(
                &lenient,
                TEST_PARSER_FINGERPRINT
            ),
            talkbank_transform::RulesVersion::current_with_rule_selection(
                &strict,
                TEST_PARSER_FINGERPRINT
            )
        );
    }

    /// Suppress values are matched case-insensitively, as the string-set filter
    /// this replaced already did.
    #[test]
    fn suppress_values_are_matched_case_insensitively() {
        let lowercase = build_presentation_policy(&["e736".to_string()]);
        let uppercase = build_presentation_policy(&["E736".to_string()]);
        assert_eq!(
            lowercase.overrides().len(),
            uppercase.overrides().len(),
            "case must not decide whether a code is suppressed"
        );
        assert!(!lowercase.shows_everything());
    }

    /// A value naming no real error code suppresses nothing, rather than
    /// silently building a policy that hides an invented code.
    #[test]
    fn an_unrecognized_suppress_value_suppresses_nothing() {
        let bogus = build_presentation_policy(&["NOTACODE".to_string()]);
        assert!(bogus.shows_everything());
    }
}
