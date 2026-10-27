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
use tracing::{info, warn};

use talkbank_model::ParseValidateOptions;
use talkbank_transform::PipelineError;
use talkbank_transform::speaker_id::{
    ConfidenceThreshold, RecordedSpeakerIdentificationAttempt, RecordedSpeakerIdentificationInput,
};

mod modes;
mod support;
mod writes;

use modes::{ReferenceModeArgs, run_override_file_mode, run_reference_mode};
use writes::write_override_entry;

use modes::run_explicit_mode;

/// Exactly one source of a speaker mapping, with only that source's options.
pub enum SpeakerRequest<'a> {
    Map {
        specification: &'a str,
    },
    Match {
        reference: &'a Path,
        anchor: &'a str,
        inserted_role: &'a str,
        confidence_threshold: ConfidenceThreshold,
        write_match_report_path: Option<&'a Path>,
        write_override_path: Option<&'a Path>,
        write_pending_path: Option<&'a Path>,
    },
    Replay {
        overrides: &'a Path,
        session: &'a str,
    },
}

/// Execute a mapping request. Mode contradictions are not representable.
pub fn run_speaker_id(
    input: &Path,
    output: Option<&PathBuf>,
    request: SpeakerRequest<'_>,
) -> crate::exit_codes::CommandResult {
    let (
        reference,
        override_file_path,
        write_match_report_path,
        write_override_path,
        write_pending_path,
    ) = match &request {
        SpeakerRequest::Map { .. } => (None, None, None, None, None),
        SpeakerRequest::Match {
            reference,
            write_match_report_path,
            write_override_path,
            write_pending_path,
            ..
        } => (
            Some(*reference),
            None,
            *write_match_report_path,
            *write_override_path,
            *write_pending_path,
        ),
        SpeakerRequest::Replay { overrides, .. } => (None, Some(*overrides), None, None, None),
    };
    let input_content = match fs::read_to_string(input) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("Error reading {}: {error}", input.display());
            if let Some(path) = write_match_report_path {
                writes::write_match_report(
                    path,
                    &RecordedSpeakerIdentificationAttempt::input_rejected(
                        RecordedSpeakerIdentificationInput::Donor,
                        &PipelineError::Io(error),
                    ),
                )?;
            }
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    };
    let options = ParseValidateOptions::default();
    let relabeled = match request {
        SpeakerRequest::Map { specification } => {
            run_explicit_mode(&input_content, specification, options)?
        }
        SpeakerRequest::Match {
            reference,
            anchor,
            inserted_role,
            confidence_threshold,
            write_match_report_path,
            write_override_path,
            write_pending_path,
        } => {
            let outcome = run_reference_mode(ReferenceModeArgs {
                donor_content: &input_content,
                reference_path: reference,
                anchor,
                inserted_role_spec: inserted_role,
                threshold: confidence_threshold,
                write_match_report_path,
                write_pending_path,
                input_path: input,
                options,
            })?;
            if let Some(path) = write_override_path {
                write_override_entry(path, input, &outcome)?;
            }
            outcome.relabeled
        }
        SpeakerRequest::Replay { overrides, session } => {
            run_override_file_mode(&input_content, overrides, session, options)?
        }
    };
    match output {
        Some(path) => {
            if let Err(e) = writes::write_chat(
                path,
                &relabeled,
                [Some(input), reference, override_file_path]
                    .into_iter()
                    .flatten(),
                [
                    write_match_report_path,
                    write_override_path,
                    write_pending_path,
                ]
                .into_iter()
                .flatten(),
            ) {
                warn!("failed to write {}: {}", path.display(), e);
                eprintln!("Error writing {}: {}", path.display(), e);
                return Err(crate::exit_codes::ReportedFailure::Input.into());
            }
            info!("wrote relabeled file: {}", path.display());
        }
        None => out!("{relabeled}"),
    }
    Ok(())
}

/// LLM suggestions have their own request and always require a pending destination.
pub fn run_suggestion(
    input: &Path,
    anchor: Option<&str>,
    pending: &Path,
    provider: crate::cli::args::judgment_args::JudgmentArgs,
) -> crate::exit_codes::CommandResult {
    let content = fs::read_to_string(input).map_err(|error| {
        eprintln!("Error reading {}: {error}", input.display());
        crate::exit_codes::CommandStop::Reported(crate::exit_codes::ReportedFailure::Input)
    })?;
    modes::run_holistic_mode(modes::HolisticModeArgs {
        input,
        input_content: &content,
        anchor,
        write_pending_path: Some(pending),
        llm_endpoint: provider.llm_endpoint.as_deref(),
        llm_model: provider.llm_model.as_deref(),
        llm_api_key: provider.llm_api_key.as_deref(),
        llm_timeout_secs: provider.llm_timeout_secs,
        llm_max_retries: provider.llm_max_retries,
        llm_cache_path: provider.llm_cache.as_deref(),
        session_context_path: provider.session_context.as_deref(),
    })?;
    Ok(())
}
