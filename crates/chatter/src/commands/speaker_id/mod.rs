//! `chatter speaker-id`, relabel anonymous speaker codes.
//!
//! Thin CLI shim over the `talkbank_transform::speaker_id` module.
//! Supports explicit-mapping mode (via `--mapping`), reference
//! mode (via `--reference` + `--anchor` + `--inserted-role`), and
//! override-file replay mode (via `--override-file`).
//!
//! Submodule layout:
//! - [`modes`], the three operation modes + `ReferenceModeOutcome`
//! - [`writes`], `--write-override` + `--write-pending` side-effects
//! - [`support`], session-ID derivation, role-spec parsing, typed
//!   error → exit-code dispatchers

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{Level, info, span, warn};

use crate::cli::JudgmentMode;
use crate::exit_codes::{EXIT_INPUT_ERROR, EXIT_PRECONDITION};
use talkbank_model::ParseValidateOptions;
use talkbank_transform::speaker_id::ConfidenceThreshold;

mod modes;
mod support;
mod writes;

// Items reused by the `chatter pipeline` shim. Keep the
// `pub(crate)` surface narrow: pipeline needs the reference /
// override-file mode entry points, the holistic-LLM entry point,
// the session-ID helper, and the override-entry writer for
// `--write-override` audit-trail support.
pub(crate) use modes::{
    ENV_SESSION_CONTEXT, HolisticModeArgs, ReferenceModeArgs, apply_override_entry,
    run_holistic_mode, run_override_file_mode, run_reference_mode,
    warn_session_context_ignored_if_configured,
};
pub(crate) use support::derive_session_id;
pub(crate) use writes::write_override_entry;

use modes::run_explicit_mode;

/// All inputs for one `chatter speaker-id` invocation.
///
/// Three operation modes are encoded by which optional fields are
/// `Some`: `mapping_spec` (explicit), `reference + anchor +
/// inserted_role` (reference mode), or `override_file_path`
/// (replay). Modes are runtime-checked because clap can't express
/// the cross-arg constraints; the body's match arm partitions them.
pub struct SpeakerIdArgs<'a> {
    /// Donor CHAT file to relabel.
    pub input: &'a Path,
    /// Explicit-mapping spec (e.g. `PAR0=drop,PAR1=INV:Investigator`).
    pub mapping_spec: Option<&'a str>,
    /// Reference CHAT file for reference-mode identification.
    pub reference: Option<&'a Path>,
    /// Reference anchor speaker code (typically `CHI`).
    pub anchor: Option<&'a str>,
    /// Inserted-role spec for non-anchor donor speakers (`CODE:Role`).
    pub inserted_role: Option<&'a str>,
    /// Jaccard winner→runner-up confidence threshold for reference
    /// mode.
    pub confidence_threshold: f64,
    /// If set, reference-mode auto-decisions append an audit entry
    /// here.
    pub write_override_path: Option<&'a Path>,
    /// If set, low-confidence reference-mode refusals append a
    /// pending-adjudication entry here before exit 4.
    pub write_pending_path: Option<&'a Path>,
    /// If set, override-file replay mode reads the recorded
    /// decision instead of running reference mode.
    pub override_file_path: Option<&'a Path>,
    /// Override-file replay session ID (defaults to input basename
    /// stem).
    pub session_id: Option<&'a str>,
    /// Output path; if `None`, the relabeled CHAT is printed to
    /// stdout.
    pub output: Option<&'a PathBuf>,
    /// How the judgment is powered. `Deterministic` runs the existing
    /// Jaccard reference-matching path; `Holistic` routes to the LLM
    /// provider in `run_holistic_mode`.
    pub judgment: JudgmentMode,
    /// LLM endpoint base URL (flag value; falls back to
    /// `CHATTER_LLM_ENDPOINT`). Holistic mode only.
    pub llm_endpoint: Option<&'a str>,
    /// LLM model id (flag value; falls back to `CHATTER_LLM_MODEL`).
    /// Holistic mode only.
    pub llm_model: Option<&'a str>,
    /// LLM API key for Bearer auth (flag value; falls back to
    /// `CHATTER_LLM_API_KEY`). Optional even in holistic mode.
    pub llm_api_key: Option<&'a str>,
    /// LLM per-attempt request timeout in seconds. Defaults to the
    /// provider's built-in timeout when `None`.
    pub llm_timeout_secs: Option<u64>,
    /// LLM retry budget for transport / 5xx failures. Defaults to the
    /// provider's built-in retry count when `None`.
    pub llm_max_retries: Option<u32>,
    /// `--session-context` JSON path for holistic-mode context lookup
    /// (falls back to `CHATTER_SESSION_CONTEXT`). When neither names a
    /// file, context fields fall back to `@ID` age or unknown.
    pub session_context_path: Option<&'a Path>,
}

/// Top-level entry for `chatter speaker-id`.
///
/// Exit-code contract (matches `speaker-id.md`):
/// - 0: success.
/// - 1: I/O or parse error on an input file.
/// - 2: precondition violation (invalid mapping spec, missing mode,
///   anchor not present in reference, donor too few speakers, …).
/// - 4: reference-mode auto-decision refused (Jaccard margin below
///   the supplied confidence threshold). Per-speaker scores are
///   printed to stderr so the operator can adjudicate.
pub fn run_speaker_id(args: SpeakerIdArgs<'_>) {
    let SpeakerIdArgs {
        input,
        mapping_spec,
        reference,
        anchor,
        inserted_role,
        confidence_threshold,
        write_override_path,
        write_pending_path,
        override_file_path,
        session_id,
        output,
        judgment,
        llm_endpoint,
        llm_model,
        llm_api_key,
        llm_timeout_secs,
        llm_max_retries,
        session_context_path,
    } = args;
    let _span = span!(
        Level::INFO,
        "chatter_speaker_id",
        input = %input.display(),
    )
    .entered();

    let input_content = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to read {}: {}", input.display(), e);
            eprintln!("Error reading {}: {}", input.display(), e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    // Holistic mode is dispatched before the deterministic mapping-supply
    // partition: it is a different judgment engine, not a fourth way of
    // supplying a mapping. It always writes its review-gated suggestion to
    // `--write-pending` (never to `--output`) and never falls through to the
    // deterministic path. The deterministic arms below are byte-identical to
    // their pre-holistic form.
    if matches!(judgment, JudgmentMode::Holistic) {
        return modes::run_holistic_mode(modes::HolisticModeArgs {
            input,
            input_content: &input_content,
            anchor,
            write_pending_path,
            llm_endpoint,
            llm_model,
            llm_api_key,
            llm_timeout_secs,
            llm_max_retries,
            session_context_path,
        });
    }

    // Deterministic judgment never consults session context; if the
    // operator configured one (flag or env fallback), say so instead of
    // ignoring their input silently. Warning only: the run proceeds.
    warn_session_context_ignored_if_configured(session_context_path);

    let options = ParseValidateOptions::default();
    let relabeled = match (mapping_spec, reference, override_file_path) {
        // Explicit-mapping mode: parse the spec and apply directly.
        (Some(spec), None, None) => {
            if write_override_path.is_some() {
                eprintln!(
                    "Error: --write-override is reference-mode only; explicit-mapping mode \
                     does not auto-decide and so produces no audit entry"
                );
                std::process::exit(EXIT_PRECONDITION);
            }
            run_explicit_mode(&input_content, spec, options)
        }
        // Reference mode: text-similarity identify, build mapping from
        // the winner + inserted-role, then apply. Optionally append an
        // entry to the override file.
        (None, Some(ref_path), None) => {
            let anchor = anchor.unwrap_or_else(|| {
                eprintln!("Error: --reference requires --anchor (clap should have caught this)");
                std::process::exit(EXIT_PRECONDITION);
            });
            let inserted_role = inserted_role.unwrap_or_else(|| {
                eprintln!(
                    "Error: --reference requires --inserted-role (clap should have caught this)"
                );
                std::process::exit(EXIT_PRECONDITION);
            });
            let outcome = run_reference_mode(ReferenceModeArgs {
                donor_content: &input_content,
                reference_path: ref_path,
                anchor,
                inserted_role_spec: inserted_role,
                threshold: ConfidenceThreshold(confidence_threshold),
                write_pending_path,
                input_path: input,
                options,
            });
            if let Some(path) = write_override_path {
                write_override_entry(path, input, &outcome);
            }
            outcome.relabeled
        }
        // Override-file mode: read the recorded decision for this
        // session and apply it verbatim. No reference file, no
        // Jaccard step, the prior adjudication is the source of
        // truth.
        (None, None, Some(override_path)) => {
            let session = session_id
                .map(str::to_string)
                .unwrap_or_else(|| derive_session_id(input));
            run_override_file_mode(&input_content, override_path, &session, options)
        }
        (None, None, None) => {
            eprintln!(
                "Error: one operation mode required: pass --mapping SPEC, \
                 --reference FILE --anchor CODE --inserted-role CODE:ROLE, \
                 or --override-file FILE [--session-id ID]"
            );
            std::process::exit(EXIT_PRECONDITION);
        }
        // clap's `conflicts_with` should catch every multi-mode case
        // before we reach this arm, but the runtime guard makes the
        // failure mode explicit if a future clap refactor regresses.
        _ => {
            eprintln!(
                "Error: --mapping, --reference, and --override-file are mutually exclusive \
                 (clap should have caught this)"
            );
            std::process::exit(EXIT_PRECONDITION);
        }
    };

    match output {
        Some(path) => {
            if let Err(e) = fs::write(path, relabeled) {
                warn!("failed to write {}: {}", path.display(), e);
                eprintln!("Error writing {}: {}", path.display(), e);
                std::process::exit(EXIT_INPUT_ERROR);
            }
            info!("wrote relabeled file: {}", path.display());
        }
        None => print!("{relabeled}"),
    }
}
