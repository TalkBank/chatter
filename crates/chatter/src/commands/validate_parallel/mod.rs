//! Parallel directory validation with streaming progress and caching.
//!
//! This module is split into:
//! - [`runtime`] for the single streamed validation flow
//! - [`renderer`] for text/JSON presentation of streamed events
//! - [`audit_renderer`] for JSONL audit sweeps, a renderer over that same flow
//! - [`shared`] for the run's outcome and the sentences every surface shares

mod audit_renderer;
pub(crate) mod json_records;
mod renderer;
mod runtime;
mod shared;

use std::path::PathBuf;

use crate::cli::{OutputFormat, TuiMode};
use crate::ui::Theme;
use talkbank_transform::validation_runner::ParserKind;

pub use shared::{RunNotice, RunPhase, ValidationOutcome};

/// Cache policy for one validation run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheRefreshMode {
    /// Reuse cached entries when they are valid.
    ReuseExisting,
    /// Clear cached entries before validating.
    ForceRefresh,
}

/// What a run may do with the validation cache: one value, so "read-only,
/// but clear first" cannot be asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePolicy {
    /// Read and record verdicts; with `ForceRefresh`, clear the run's files'
    /// rows first.
    ReadWrite {
        /// Whether to clear the run's files' rows before validating.
        refresh: CacheRefreshMode,
    },
    /// Read verdicts only: no clear, no prune on open, no write (an audit).
    ReadOnly,
}

impl CachePolicy {
    /// The policy a run's presentation and `--force` allow: an audit reads
    /// only, and asking it to refresh is a conflict, never a silent clear.
    pub fn for_run(
        presentation: &ValidationPresentation,
        refresh: CacheRefreshMode,
    ) -> Result<Self, PresentationConflict> {
        match (presentation, refresh) {
            (ValidationPresentation::Streamed(StreamedPresentation::Audit { .. }), refresh) => {
                match refresh {
                    CacheRefreshMode::ReuseExisting => Ok(Self::ReadOnly),
                    CacheRefreshMode::ForceRefresh => Err(PresentationConflict::ForceWithAudit),
                }
            }
            (
                ValidationPresentation::Streamed(
                    StreamedPresentation::Lines { .. } | StreamedPresentation::Json,
                )
                | ValidationPresentation::Tui { .. },
                refresh,
            ) => Ok(Self::ReadWrite { refresh }),
        }
    }
}

/// How much a line-oriented text run prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verbosity {
    /// Progress, each file's diagnostics or failure, and the summary. A
    /// valid file with no diagnostic prints no line of its own; it is
    /// counted in the summary.
    Normal,
    /// Only problems (`--quiet`); rely on the exit status.
    Quiet,
}

/// A non-interactive run's output: where it goes and in what shape.
#[derive(Clone, Debug)]
pub enum StreamedPresentation {
    /// Human-readable lines: diagnostics on stderr, results on stdout.
    Lines {
        /// How much to print.
        verbosity: Verbosity,
    },
    /// JSON records on stdout; stderr stays empty.
    Json,
    /// A JSONL audit file, with progress and the summary on the terminal.
    ///
    /// An audit reads the validation cache and writes nothing to it
    /// ([`CachePolicy::ReadOnly`]): a reporting sweep must not mutate shared
    /// state.
    Audit {
        /// Output path for JSONL audit records.
        output_path: PathBuf,
    },
}

/// How one validation run presents itself: exactly one surface, decided
/// once, from all the flags that bear on it together.
///
/// It replaces four values (`--format`, `--quiet`, `--audit` and the TUI
/// decision) that were combined by an unwritten precedence rule in three
/// places: `--format json` on a terminal opened the TUI, `--audit` beside
/// `--format json` silently printed text, and `--quiet` meant nothing to
/// the TUI. Every combination now either names one variant or is a usage
/// error ([`PresentationConflict`]).
#[derive(Clone, Debug)]
pub enum ValidationPresentation {
    /// Text, JSON or an audit file.
    Streamed(StreamedPresentation),
    /// The interactive terminal interface.
    Tui {
        /// Its color theme.
        theme: Theme,
    },
}

/// Flags that name more than one presentation. Reported as a usage error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationConflict {
    /// `--quiet` asks for less text, and JSON is not text.
    QuietWithJson,
    /// `--tui-mode force` beside a surface that is not the TUI.
    ForcedTuiWith(NonTuiSurface),
    /// `--force` clears cache rows, and an audit never writes the cache.
    ForceWithAudit,
}

/// A flag that names a surface other than the TUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonTuiSurface {
    /// `--audit`.
    Audit,
    /// `--format json`.
    Json,
    /// `--quiet`.
    Quiet,
}

impl std::fmt::Display for NonTuiSurface {
    /// The flag as the user typed it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Audit => "--audit",
            Self::Json => "--format json",
            Self::Quiet => "--quiet",
        })
    }
}

impl std::fmt::Display for PresentationConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QuietWithJson => f.write_str("--quiet cannot be used with --format json"),
            Self::ForcedTuiWith(surface) => {
                write!(f, "--tui-mode force cannot be used with {surface}")
            }
            Self::ForceWithAudit => {
                f.write_str("--force cannot be used with --audit: an audit never writes the cache")
            }
        }
    }
}

impl ValidationPresentation {
    /// The one presentation the flags name. Clap already refuses `--audit`
    /// beside `--format` or `--quiet`; this decides the rest, and the TUI is
    /// chosen automatically only for plain text with stdout a terminal.
    pub fn resolve(
        format: OutputFormat,
        quiet: bool,
        audit: Option<PathBuf>,
        tui: TuiMode,
        theme: Theme,
    ) -> Result<Self, PresentationConflict> {
        use std::io::IsTerminal;
        let lines = |verbosity| Ok(Self::Streamed(StreamedPresentation::Lines { verbosity }));
        match (audit, format, quiet, tui) {
            (Some(_), _, _, TuiMode::Force) => {
                Err(PresentationConflict::ForcedTuiWith(NonTuiSurface::Audit))
            }
            (Some(output_path), _, _, TuiMode::Auto | TuiMode::Disable) => {
                Ok(Self::Streamed(StreamedPresentation::Audit { output_path }))
            }
            (None, OutputFormat::Json, true, _) => Err(PresentationConflict::QuietWithJson),
            (None, OutputFormat::Json, false, TuiMode::Force) => {
                Err(PresentationConflict::ForcedTuiWith(NonTuiSurface::Json))
            }
            (None, OutputFormat::Json, false, TuiMode::Auto | TuiMode::Disable) => {
                Ok(Self::Streamed(StreamedPresentation::Json))
            }
            (None, OutputFormat::Text, true, TuiMode::Force) => {
                Err(PresentationConflict::ForcedTuiWith(NonTuiSurface::Quiet))
            }
            (None, OutputFormat::Text, true, TuiMode::Auto | TuiMode::Disable) => {
                lines(Verbosity::Quiet)
            }
            (None, OutputFormat::Text, false, TuiMode::Force) => Ok(Self::Tui { theme }),
            (None, OutputFormat::Text, false, TuiMode::Auto) => {
                match std::io::stdout().is_terminal() {
                    true => Ok(Self::Tui { theme }),
                    false => lines(Verbosity::Normal),
                }
            }
            (None, OutputFormat::Text, false, TuiMode::Disable) => lines(Verbosity::Normal),
        }
    }
}

/// Validation-specific rules and parser choices.
#[derive(Clone, Copy, Debug)]
pub struct ValidationRules {
    /// Whether alignment-sensitive validation should run.
    pub alignment: talkbank_model::validation::AlignmentValidation,
    /// Whether roundtrip validation should run after the main pass.
    pub roundtrip: talkbank_transform::RoundtripCheck,
    /// Which parser backend should power validation.
    pub parser_kind: ParserKind,
    /// Whether the strict cross-utterance linker checks run. The codes they
    /// turn on are listed per code in the generated error index.
    pub linkers: talkbank_model::LinkerChecks,
}

/// Execution policy for one validation run.
#[derive(Clone, Copy, Debug)]
pub struct ValidationExecution {
    /// What the run may do with the cache.
    pub cache: CachePolicy,
    /// Optional worker-count override.
    pub jobs: Option<std::num::NonZeroUsize>,
    /// How many errors the run may find before it stops itself (warnings
    /// never count). Enforced by the runner, for every presentation.
    pub error_limit: talkbank_transform::ErrorLimit,
}

/// Runtime options for parallel directory validation.
#[derive(Clone, Debug)]
pub struct ValidateDirectoryOptions {
    /// Validation rules and parser choices.
    pub rules: ValidationRules,
    /// Execution policy for cache and worker usage.
    pub execution: ValidationExecution,
    /// Presentation mode for the validation stream.
    pub presentation: ValidationPresentation,
    /// Error codes to suppress, already resolved from the `--suppress`
    /// values (named groups expanded to their member codes).
    pub suppress: crate::commands::error_codes::SuppressedCodes,
    /// Facts to say before the run starts, through the renderer.
    pub notices: Vec<RunNotice>,
}

/// Validate the files the command-line `paths` name: the one runtime for
/// every `chatter validate`, one file or a corpus, through the shared
/// streaming runner (worker pool, events, renderer or TUI). The arguments
/// are expanded inside, the one way (`expand_transcript_arguments`), and
/// anything that could not be read is a read error in the run's own results,
/// never a refusal printed outside it.
///
/// `summary_label` is cosmetic, used as the `directory` field in
/// JSON summaries and the header in text output: the first argument.
pub fn validate_paths_parallel(
    paths: Vec<PathBuf>,
    summary_label: PathBuf,
    options: ValidateDirectoryOptions,
) -> ValidationOutcome {
    // Every presentation, INCLUDING audit, goes through this one flow; see
    // `audit_renderer` for why audit is a renderer rather than a pipeline.
    runtime::run_validation_runtime(paths, summary_label, options)
}

/// Validate one changed file for `chatter watch`: the runner's one-file
/// pipeline (stored name, cache, validation) with the watch's `run`, its
/// cache opened once for the whole watch, its result rendered as quiet text
/// (diagnostics and failures, no progress and no summary). Returns how the
/// run ended, for the watch to say whether the file passed.
pub(crate) fn validate_watched_file(
    path: PathBuf,
    run: &talkbank_transform::ValidationRun,
) -> talkbank_transform::RunEnding {
    let mut renderer = renderer::lines_renderer(Verbosity::Quiet);
    let label = path.clone();
    let (events, _canceller) =
        talkbank_transform::validation_runner::validate_files_streaming(vec![path], run);
    // Quiet text has no output that can fail apart from the run; the
    // ending is the result.
    renderer::render_run(events, renderer.as_mut(), &label, run.config().roundtrip).ending
}

#[cfg(test)]
mod watch_tests {
    use super::*;
    use talkbank_transform::{RunEnding, ValidationConfig, ValidationRun};

    /// A watched file goes through the runner's one-file pipeline: a
    /// reference transcript passes, and a path that vanished mid-edit is the
    /// runner's read error, a complete run that does not pass.
    #[test]
    fn a_watched_file_is_validated_by_the_runner() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let reference = root.join("corpus/reference/core/basic-conversation.cha");
        let run = ValidationRun::uncached(ValidationConfig::default());
        let ending = validate_watched_file(reference, &run);
        assert!(ending.passed(), "{ending:?}");
        let dir = tempfile::tempdir().expect("temporary directory");
        let gone = validate_watched_file(dir.path().join("gone.cha"), &run);
        assert!(
            matches!(&gone, RunEnding::Complete(stats) if stats.snapshot().invalid_files() == 1),
            "{gone:?}"
        );
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;

    fn resolve(
        format: OutputFormat,
        quiet: bool,
        audit: Option<&str>,
        tui: TuiMode,
    ) -> Result<ValidationPresentation, PresentationConflict> {
        ValidationPresentation::resolve(
            format,
            quiet,
            audit.map(PathBuf::from),
            tui,
            Theme::default(),
        )
    }

    /// JSON and audit runs never become the TUI, whatever the terminal: the
    /// old precedence opened the TUI for `--format json` on a terminal.
    /// `Auto` for them does not consult the terminal at all, so this holds
    /// in a test with no terminal and on one alike.
    #[test]
    fn json_and_audit_never_become_the_tui() {
        assert!(matches!(
            resolve(OutputFormat::Json, false, None, TuiMode::Auto),
            Ok(ValidationPresentation::Streamed(StreamedPresentation::Json))
        ));
        assert!(matches!(
            resolve(OutputFormat::Text, false, Some("out.jsonl"), TuiMode::Auto),
            Ok(ValidationPresentation::Streamed(
                StreamedPresentation::Audit { .. }
            ))
        ));
        assert!(matches!(
            resolve(OutputFormat::Text, true, None, TuiMode::Auto),
            Ok(ValidationPresentation::Streamed(
                StreamedPresentation::Lines {
                    verbosity: Verbosity::Quiet
                }
            ))
        ));
    }

    /// Flags naming two surfaces are a usage error, never a silent winner.
    #[test]
    fn conflicting_flags_are_refused() {
        assert_eq!(
            resolve(OutputFormat::Json, true, None, TuiMode::Auto).err(),
            Some(PresentationConflict::QuietWithJson)
        );
        for (format, quiet, audit) in [
            (OutputFormat::Json, false, None),
            (OutputFormat::Text, true, None),
            (OutputFormat::Text, false, Some("out.jsonl")),
        ] {
            assert!(matches!(
                resolve(format, quiet, audit, TuiMode::Force),
                Err(PresentationConflict::ForcedTuiWith(_))
            ));
        }
        assert!(matches!(
            resolve(OutputFormat::Text, false, None, TuiMode::Force),
            Ok(ValidationPresentation::Tui { .. })
        ));
    }
}
