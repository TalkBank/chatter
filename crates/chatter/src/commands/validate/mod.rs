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
use talkbank_model::ErrorCode;
use talkbank_transform::paths::is_chat_transcript_path;
use talkbank_transform::validation_runner::ParserKind;

use super::validate_parallel::{
    AlignmentValidationMode, CacheRefreshMode, RoundtripValidationMode, StreamingValidationOutput,
    ValidateDirectoryOptions, ValidationExecution, ValidationInterface, ValidationOutcome,
    ValidationPresentation, ValidationRules, ValidationTraversalMode, validate_paths_parallel,
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

/// A named `--suppress` group: a user-friendly shorthand for a fixed set of
/// error codes. Closed on purpose (a `match` on this type must be exhaustive),
/// so adding a new group is a compile-time-visible decision, not a new string
/// literal someone has to remember to wire up everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SuppressionGroup {
    /// The whole Phon `%x` dependent-tier validation surface (`%xmodsyl`,
    /// `%xphosyl`, `%xphoaln`, `%xphoint`). Opt-out only; this validation
    /// runs by default.
    Xphon,
}

impl SuppressionGroup {
    /// The error codes this group expands to.
    ///
    /// For [`Self::Xphon`] this is `talkbank_model::XPHON_ERROR_CODES`
    /// (co-located with the `ErrorCode` definitions), so the CLI group can
    /// never drift from the codes the validator actually emits.
    fn codes(self) -> &'static [ErrorCode] {
        match self {
            Self::Xphon => talkbank_model::XPHON_ERROR_CODES,
        }
    }
}

/// One resolved `--suppress` argument: either a named group or a single error
/// code. Replaces the earlier untyped behavior where any value not matching a
/// known group name was assumed, unchecked, to be a literal error code and
/// silently uppercased; a typo (`E9999` for `E999`, or a misspelled group
/// name) then suppressed nothing and the CLI still exited 0, indistinguishable
/// from a suppression that worked.
enum SuppressionSelector {
    /// A named group, e.g. `xphon`.
    Group(SuppressionGroup),
    /// A single error code, e.g. `E736`.
    Code(ErrorCode),
}

/// A `--suppress` value that names neither a known group nor a known error
/// code.
///
/// Carries the offending value verbatim (as the user typed it, before any
/// case normalization) so the CLI's error message can point at exactly what
/// was wrong.
#[derive(Clone, Debug, thiserror::Error)]
#[error(
    "--suppress {value:?} is not a known suppression group (xphon) or a known error code; \
     see `chatter validate --list-checks` for valid codes"
)]
struct UnknownSuppressionValue {
    value: String,
}

impl SuppressionSelector {
    /// Resolve one raw `--suppress` value.
    ///
    /// Group names are matched case-insensitively (`xphon`, `XPHON`); error
    /// codes are matched case-insensitively too (`e241`, `E241`). Leniency
    /// about case is preserved from the pre-existing behavior; leniency about
    /// *existence* is exactly the bug this type closes: any value that is
    /// neither a real group nor a real code is refused, not guessed at.
    fn parse(raw: &str) -> Result<Self, UnknownSuppressionValue> {
        match raw.to_lowercase().as_str() {
            "xphon" => Ok(Self::Group(SuppressionGroup::Xphon)),
            _ => super::error_codes::resolve_error_code(raw)
                .map(Self::Code)
                .ok_or_else(|| UnknownSuppressionValue {
                    value: raw.to_string(),
                }),
        }
    }
}

/// Expand named suppress groups into concrete error codes.
///
/// Named groups are a user-friendly shorthand; every other value must name a
/// real [`ErrorCode`] or the whole `--suppress` argument is rejected (see
/// [`SuppressionSelector::parse`]). Phon `%x` validation runs by default,
/// with no automatic suppression: the user silences it with `--suppress
/// xphon` (the whole group) or an individual code. (The historical
/// `--check-xphon` opt-in is now a deprecated no-op.)
///
/// Returns typed [`ErrorCode`] values, not strings, so a caller can feed the
/// result straight into a typed suppression configuration without an
/// intermediate string round-trip.
fn expand_suppress_groups(raw: Vec<String>) -> Result<Vec<ErrorCode>, UnknownSuppressionValue> {
    let mut codes = Vec::new();
    for item in raw {
        match SuppressionSelector::parse(&item)? {
            SuppressionSelector::Group(group) => codes.extend_from_slice(group.codes()),
            SuppressionSelector::Code(code) => codes.push(code),
        }
    }
    Ok(codes)
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
    let suppress = match expand_suppress_groups(raw_suppress) {
        Ok(codes) => codes,
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    };
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

    let outcome = validate_paths_parallel(
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

    // The ONE place `chatter validate` decides its exit status, matched
    // exhaustively so that a new way for a run to end badly cannot be added
    // without deciding what it exits with. The point of the enum: a run that
    // lost files to a crashed worker has immaculate-looking counts, because
    // the missing files contributed to no counter, and exiting 0 on it would
    // hand a researcher a false clean bill of health.
    let failed = match outcome {
        ValidationOutcome::Complete { stats } => stats.invalid_files > 0 || stats.parse_errors > 0,
        ValidationOutcome::Incomplete { .. } => true,
        ValidationOutcome::Aborted { .. } => true,
        ValidationOutcome::NoTerminalEvent => true,
    };

    if failed {
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
    use talkbank_model::{ErrorCode, XPHON_ERROR_CODES};

    #[test]
    fn nothing_suppressed_by_default() {
        // Phon `%x` validation is on by default: no automatic suppression.
        assert!(
            expand_suppress_groups(vec![])
                .expect("empty suppress list always resolves")
                .is_empty()
        );
    }

    #[test]
    fn suppress_xphon_silences_whole_group() {
        let effective =
            expand_suppress_groups(vec!["xphon".to_string()]).expect("xphon is a known group");
        for code in XPHON_ERROR_CODES {
            assert!(
                effective.contains(code),
                "--suppress xphon should include {code}"
            );
        }
    }

    #[test]
    fn explicit_user_suppress_does_not_add_xphon() {
        let effective =
            expand_suppress_groups(vec!["E316".to_string()]).expect("E316 is a known code");
        assert_eq!(effective, vec![ErrorCode::UnparsableContent]);
    }

    #[test]
    fn redundant_xphon_entry_not_doubled() {
        let effective =
            expand_suppress_groups(vec!["xphon".to_string()]).expect("xphon is a known group");
        for code in XPHON_ERROR_CODES {
            let count = effective.iter().filter(|c| *c == code).count();
            assert_eq!(count, 1, "code {code} should appear exactly once");
        }
    }

    #[test]
    fn single_xphon_code_can_be_suppressed_individually() {
        let effective =
            expand_suppress_groups(vec!["E742".to_string()]).expect("E742 is a known code");
        assert!(effective.contains(&ErrorCode::XphointBulletInvalid));
        assert!(!effective.contains(&ErrorCode::XphointIntervalNotMonotonic));
    }

    #[test]
    fn xphon_expands_to_all_phon_codes() {
        let result =
            expand_suppress_groups(vec!["xphon".to_string()]).expect("xphon is a known group");
        assert_eq!(result.len(), XPHON_ERROR_CODES.len());
        for code in XPHON_ERROR_CODES {
            assert!(result.contains(code), "missing {code}");
        }
    }

    #[test]
    fn literal_codes_pass_through_case_insensitively() {
        let result =
            expand_suppress_groups(vec!["e316".to_string()]).expect("e316 is a known code");
        assert_eq!(result, vec![ErrorCode::UnparsableContent]);
    }

    #[test]
    fn mixed_groups_and_codes() {
        let result = expand_suppress_groups(vec!["xphon".to_string(), "E316".to_string()])
            .expect("xphon and E316 are both known");
        assert_eq!(result.len(), XPHON_ERROR_CODES.len() + 1);
        assert!(result.contains(&ErrorCode::ModsylModCountMismatch));
        assert!(result.contains(&ErrorCode::XphointBulletInvalid));
        assert!(result.contains(&ErrorCode::UnparsableContent));
    }

    #[test]
    fn unknown_group_name_is_rejected() {
        let err = expand_suppress_groups(vec!["notagroup".to_string()])
            .expect_err("notagroup names no group or code");
        assert!(err.to_string().contains("notagroup"));
    }

    #[test]
    fn unknown_code_is_rejected() {
        let err = expand_suppress_groups(vec!["E9999".to_string()])
            .expect_err("E9999 names no real error code");
        assert!(err.to_string().contains("E9999"));
    }
}
