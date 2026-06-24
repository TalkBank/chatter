//! Validation commands for CHAT files.
//!
//! This module exposes the low-level `validate_file` entrypoint plus formatting helpers
//! and utilities (audit reporting, output formatting). It is the landing point for CLI `validate`
//! subcommands (single file, directory, TUI) and orchestrates caching, alignment toggles, and
//! structured outputs.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod audit_reporter;
pub(crate) mod cache;
mod file;
mod output;

use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::ui::Theme;
use talkbank_transform::validation_runner::{ParserKind, is_chat_transcript_path};

use super::validate_parallel::{
    AlignmentValidationMode, CacheRefreshMode, RoundtripValidationMode, StreamingValidationOutput,
    ValidateDirectoryOptions, ValidationExecution, ValidationInterface, ValidationPresentation,
    ValidationRules, ValidationTraversalMode, validate_paths_parallel,
};

pub use file::validate_file;

/// Typed options for the top-level `chatter validate` command.
#[derive(Clone, Debug)]
pub struct ValidateCommandRules {
    /// Alignment validation policy.
    pub alignment: AlignmentValidationMode,
    /// Roundtrip validation policy.
    pub roundtrip: RoundtripValidationMode,
    /// Parser backend selection.
    pub parser_kind: ParserKind,
    /// Enable strict cross-utterance linker validation (E351-E355).
    pub strict_linkers: bool,
}

/// Execution settings for the top-level `chatter validate` command.
#[derive(Clone, Debug)]
pub struct ValidateCommandExecution {
    /// Cache refresh policy for the target path.
    pub cache_refresh: CacheRefreshMode,
    /// Optional parallel worker count.
    pub jobs: Option<usize>,
    /// Optional global error cap for directory validation.
    pub max_errors: Option<usize>,
}

/// Output and interaction settings for the top-level `chatter validate` command.
#[derive(Clone, Debug)]
pub struct ValidateCommandPresentation {
    /// Output format for file or directory validation.
    pub format: OutputFormat,
    /// Whether to suppress success output.
    pub quiet: bool,
    /// Optional audit JSONL output path.
    pub audit_output: Option<PathBuf>,
    /// Interactive presentation surface to use.
    pub interface: ValidationInterface,
    /// Loaded theme for TUI validation.
    pub theme: Theme,
}

/// Typed options for the top-level `chatter validate` command.
#[derive(Clone, Debug)]
pub struct ValidateCommandOptions {
    /// Validation rules and parser choices.
    pub rules: ValidateCommandRules,
    /// Cache, worker-count, and failure-limit settings.
    pub execution: ValidateCommandExecution,
    /// Output, audit, and TUI settings.
    pub presentation: ValidateCommandPresentation,
    /// Raw `--suppress` list as received from the CLI. Named groups
    /// (like `xphon`) are still unexpanded at this point; final
    /// resolution happens in `expand_suppress_groups`.
    pub suppress: Vec<String>,
    /// Deprecated no-op. Phon `%x` validation (E725-E728 plus the content
    /// checks E735-E746) now runs by default; pass `--suppress xphon` to
    /// silence it. Retained so existing `--check-xphon` invocations do not
    /// break; passing it prints a deprecation note.
    pub check_xphon: bool,
}

/// Every Phon `%x` dependent-tier diagnostic, grouped under the `xphon` suppress
/// name. Phon validation now runs by default (these tiers are first-class CHAT),
/// so the group exists only as the opt-out: `--suppress xphon` silences them all.
///
/// The codes themselves are the single source of truth in
/// `talkbank_model::XPHON_ERROR_CODES` (co-located with the `ErrorCode`
/// definitions), so the CLI group can never drift from the codes the validator
/// actually emits.
///
/// Expand named suppress groups into concrete error codes. Named groups are a
/// user-friendly shorthand; unknown names are treated as literal error codes
/// (e.g., "E726"). Phon `%x` validation runs by default, with no automatic
/// suppression: the user silences it with `--suppress xphon` (the whole group)
/// or an individual code. (The historical `--check-xphon` opt-in is now a
/// deprecated no-op.)
fn expand_suppress_groups(raw: Vec<String>) -> Vec<String> {
    let mut codes = Vec::new();
    for item in raw {
        match item.to_lowercase().as_str() {
            // The whole Phon `%x` validation surface (`%xmodsyl`, `%xphosyl`,
            // `%xphoaln`, `%xphoint`). Opt-out only; validation is on by default.
            "xphon" => codes.extend(
                talkbank_model::XPHON_ERROR_CODES
                    .iter()
                    .map(|code| code.as_str().to_string()),
            ),
            _ => codes.push(item.to_uppercase()),
        }
    }
    codes
}

/// Execute one top-level `chatter validate` invocation.
///
/// Accepts one or more paths. Each path can be a file or directory.
/// Multiple files are validated individually. A single directory uses
/// the parallel directory validation pipeline.
pub fn run_validate_command(paths: Vec<PathBuf>, options: ValidateCommandOptions) {
    let ValidateCommandOptions {
        rules,
        execution,
        presentation,
        suppress: raw_suppress,
        check_xphon,
    } = options;
    if check_xphon {
        eprintln!(
            "note: --check-xphon is deprecated and has no effect; Phon %x validation \
             now runs by default (use --suppress xphon to silence it)"
        );
    }
    let suppress = expand_suppress_groups(raw_suppress);
    let ValidateCommandRules {
        alignment,
        roundtrip,
        parser_kind,
        strict_linkers,
    } = rules;
    let ValidateCommandExecution {
        cache_refresh,
        jobs,
        max_errors,
    } = execution;
    let ValidateCommandPresentation {
        format,
        quiet,
        audit_output,
        interface,
        theme,
    } = presentation;

    // ARCHITECTURAL NOTE (2026-05-03): every CLI input, single file,
    // multiple files, single directory, multiple directories, or any
    // mix, funnels through ONE pipeline (`validate_paths_parallel` →
    // `validate_files_streaming`). This replaces a previous fork where
    // multi-file inputs went through a per-file `validate_file` loop
    // (no progress bar, noisy "✓ valid" lines for each file, separate
    // per-file TUI) while directory inputs went through the parallel
    // streaming pipeline. The fork was flagged as wrong UX: multi-file
    // input behaved differently from directory input despite both
    // resolving to the same logical set of files, leaving duplicate
    // code paths reinventing the streaming pipeline.
    //
    // The unified shape means CLI args drive ONLY which .cha files are
    // collected; everything downstream (renderer, progress, TUI,
    // suppression, summary, exit code) is identical regardless of
    // input shape.

    // Walk every input path into a flat .cha file list. Files contribute
    // themselves directly; directories contribute their recursive .cha
    // descendants.
    let mut files: Vec<PathBuf> = Vec::new();
    for p in &paths {
        if p.is_file() {
            files.push(p.clone());
        } else if p.is_dir() {
            collect_cha_files_recursive(p, &mut files);
        } else {
            eprintln!("Error: {:?} is not a file or directory", p);
            std::process::exit(1);
        }
    }

    if files.is_empty() {
        eprintln!("Error: no .cha files found in {:?}", paths);
        std::process::exit(1);
    }

    // Sort for deterministic processing order (matches directory walk
    // behavior, which sorts collected files before dispatch).
    files.sort();

    // Cosmetic summary label: use the first input path verbatim. For
    // a single-directory invocation this preserves the old behavior of
    // printing the directory name in the summary; for a multi-file or
    // mixed-input invocation it's just the first arg the user typed.
    let summary_label = paths.first().cloned().unwrap_or_else(|| PathBuf::from("."));

    let stats = validate_paths_parallel(
        files,
        summary_label,
        ValidateDirectoryOptions {
            rules: ValidationRules {
                alignment,
                roundtrip,
                parser_kind,
                strict_linkers,
            },
            traversal: ValidationTraversalMode::Recursive,
            execution: ValidationExecution {
                cache_refresh,
                jobs,
                max_errors,
            },
            presentation: match audit_output {
                Some(output_path) => ValidationPresentation::Audit { output_path },
                None => ValidationPresentation::Streaming(StreamingValidationOutput {
                    format,
                    quiet,
                    interface,
                    theme,
                }),
            },
            suppress,
        },
    );

    if stats.invalid_files > 0 || stats.parse_errors > 0 {
        std::process::exit(1);
    }
}

/// Walk `dir` and append every `.cha` file (recursively) to `files`.
/// Mirrors the directory-walk behavior of
/// `validation_runner::collect_cha_files` but lives on the CLI side
/// because the CLI is the layer that mixes file-and-directory args.
fn collect_cha_files_recursive(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Warning: failed to read {:?}: {}", dir, e);
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_cha_files_recursive(&path, files);
        } else if is_chat_transcript_path(&path) {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::expand_suppress_groups;
    use talkbank_model::XPHON_ERROR_CODES;

    /// The Phon `%x` group codes as strings, derived from the single model-side
    /// source of truth (so these tests cannot drift from the actual codes).
    fn xphon_code_strings() -> Vec<String> {
        XPHON_ERROR_CODES
            .iter()
            .map(|code| code.as_str().to_string())
            .collect()
    }

    #[test]
    fn nothing_suppressed_by_default() {
        // Phon `%x` validation is on by default: no automatic suppression.
        assert!(expand_suppress_groups(vec![]).is_empty());
    }

    #[test]
    fn suppress_xphon_silences_whole_group() {
        let effective = expand_suppress_groups(vec!["xphon".to_string()]);
        for code in xphon_code_strings() {
            assert!(
                effective.contains(&code),
                "--suppress xphon should include {code}"
            );
        }
    }

    #[test]
    fn explicit_user_suppress_does_not_add_xphon() {
        let effective = expand_suppress_groups(vec!["E316".to_string()]);
        assert_eq!(effective, vec!["E316"]);
    }

    #[test]
    fn redundant_xphon_entry_not_doubled() {
        let effective = expand_suppress_groups(vec!["xphon".to_string()]);
        for code in xphon_code_strings() {
            let count = effective.iter().filter(|c| **c == code).count();
            assert_eq!(count, 1, "code {code} should appear exactly once");
        }
    }

    #[test]
    fn single_xphon_code_can_be_suppressed_individually() {
        let effective = expand_suppress_groups(vec!["E742".to_string()]);
        assert!(effective.contains(&"E742".to_string()));
        assert!(!effective.contains(&"E743".to_string()));
    }

    #[test]
    fn xphon_expands_to_all_phon_codes() {
        let result = expand_suppress_groups(vec!["xphon".to_string()]);
        assert_eq!(result.len(), XPHON_ERROR_CODES.len());
        for code in xphon_code_strings() {
            assert!(result.contains(&code), "missing {code}");
        }
    }

    #[test]
    fn literal_codes_pass_through_uppercased() {
        let result = expand_suppress_groups(vec!["e316".to_string()]);
        assert_eq!(result, vec!["E316"]);
    }

    #[test]
    fn mixed_groups_and_codes() {
        let result = expand_suppress_groups(vec!["xphon".to_string(), "E316".to_string()]);
        assert_eq!(result.len(), XPHON_ERROR_CODES.len() + 1);
        assert!(result.contains(&"E725".to_string()));
        assert!(result.contains(&"E742".to_string()));
        assert!(result.contains(&"E316".to_string()));
    }
}
