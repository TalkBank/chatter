//! The operation modes of `chatter speaker-id`: explicit-mapping
//! (manual `--mapping`), reference-mode (text-similarity), override-file
//! replay, and holistic-LLM (`--judgment holistic`). The first three
//! produce relabeled CHAT text; the holistic mode produces a
//! review-gated pending entry instead. Each returns its result or
//! `std::process::exit`s with the contract exit code.

use std::fs;
use std::path::Path;
use tracing::{info, warn};

use crate::exit_codes::{EXIT_INPUT_ERROR, EXIT_PRECONDITION};
use talkbank_model::{ParseValidateOptions, ParticipantRole, SpeakerCode};
use talkbank_transform::parse_and_validate;
use talkbank_transform::speaker_id::{CURRENT_PROMPT_VERSION, JudgmentProvider, PromptVersion};
use talkbank_transform::speaker_id::{
    ConfidenceThreshold, EndpointUrl, JudgmentRequest, MappingSpec, MergeOverride, ModelId,
    OverrideFile, ProvenanceMeta, SampleBudget, SessionContextFile, SessionId, SpeakerAssignment,
    SpeakerIdError, apply_mapping, apply_mapping_chat, identify_mapping, judgment_to_pending,
    parse_mapping_spec, sample_session, session_context,
};

use talkbank_llm::{ApiKey, HttpJudgmentProvider, HttpProviderConfig, RetryCount, TimeoutSecs};

use super::support::{
    derive_session_id, exit_with_override_file_error, exit_with_speaker_id_error,
    parse_inserted_role,
};
use super::writes::{append_pending_entry, write_pending_entry};

/// Carries the relabeled CHAT plus everything an override-file entry
/// needs to record about the decision. Exposed `pub(crate)` so the
/// per-session `chatter pipeline` shim can reuse the
/// reference-mode helpers without duplicating their LowConfidence /
/// `--write-pending` handling.
pub(crate) struct ReferenceModeOutcome {
    pub(crate) relabeled: String,
    pub(crate) report: talkbank_transform::speaker_id::DonorMatchReport,
    pub(crate) mapping: MappingSpec,
    pub(crate) inserted_code: SpeakerCode,
    pub(crate) inserted_role_tag: ParticipantRole,
}

/// Explicit-mapping mode: parse the `--mapping` spec, apply it,
/// return the relabeled CHAT text.
pub(super) fn run_explicit_mode(
    content: &str,
    spec: &str,
    options: ParseValidateOptions,
) -> String {
    let mapping = match parse_mapping_spec(spec) {
        Ok(m) => m,
        Err(e) => {
            warn!("mapping parse failed: {}", e);
            eprintln!("Error: {}", e);
            std::process::exit(crate::exit_codes::EXIT_PRECONDITION);
        }
    };
    match apply_mapping(content, &mapping, options) {
        Ok(s) => s,
        Err(e) => exit_with_speaker_id_error(e),
    }
}

/// Reference mode: identify the donor speaker matching the reference
/// anchor, build a mapping (winner → drop; non-winner → inserted
/// role), apply. The returned outcome carries both the relabeled
/// text and the data needed to write an override-file entry.
///
/// If `write_pending_path` is `Some` and `identify_mapping` returns
/// `SpeakerIdError::LowConfidence`, a pending-adjudication entry is
/// appended to the named file before exiting with code 4. The pending
/// entry's `suggested` field carries the algorithm's would-have-been
/// decision so the operator can accept-as-is in `chatter adjudicate`.
/// All inputs to one reference-mode invocation. Constructed by the
/// CLI orchestrators (`chatter speaker-id` and `chatter pipeline`)
/// from their respective clap surfaces.
pub(crate) struct ReferenceModeArgs<'a> {
    /// Already-loaded donor CHAT text (the caller's `fs::read_to_string`
    /// result).
    pub donor_content: &'a str,
    /// Reference CHAT file to load + parse.
    pub reference_path: &'a Path,
    /// Reference anchor speaker code (typically `CHI`).
    pub anchor: &'a str,
    /// Inserted-role spec for non-anchor donor speakers.
    pub inserted_role_spec: &'a str,
    /// Jaccard winner→runner-up margin threshold.
    pub threshold: ConfidenceThreshold,
    /// If set, low-confidence refusals append a pending entry here
    /// before exit 4.
    pub write_pending_path: Option<&'a Path>,
    /// Donor input path, needed for the pending entry's session ID
    /// derivation.
    pub input_path: &'a Path,
    /// Parser options threaded through to `parse_and_validate`.
    pub options: ParseValidateOptions,
}

pub(crate) fn run_reference_mode(args: ReferenceModeArgs<'_>) -> ReferenceModeOutcome {
    let ReferenceModeArgs {
        donor_content,
        reference_path,
        anchor,
        inserted_role_spec,
        threshold,
        write_pending_path,
        input_path,
        options,
    } = args;
    let reference_content = match fs::read_to_string(reference_path) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to read {}: {}", reference_path.display(), e);
            eprintln!("Error reading {}: {}", reference_path.display(), e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    let donor_chat = match parse_and_validate(donor_content, options.clone()) {
        Ok(c) => c,
        Err(e) => {
            warn!("donor parse failed: {}", e);
            eprintln!("Error parsing donor: {}", e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };
    let reference_chat = match parse_and_validate(&reference_content, options.clone()) {
        Ok(c) => c,
        Err(e) => {
            warn!("reference parse failed: {}", e);
            eprintln!("Error parsing reference: {}", e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    // Parse `--inserted-role` upfront so it's available on both the
    // happy path AND the low-confidence path that writes the pending
    // entry's `suggested.inserted_role`.
    let (inserted_code, inserted_role) = match parse_inserted_role(inserted_role_spec) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(crate::exit_codes::EXIT_PRECONDITION);
        }
    };

    let anchor_code = SpeakerCode::new(anchor);
    let report = match identify_mapping(&reference_chat, &anchor_code, &donor_chat, threshold) {
        Ok(r) => r,
        Err(SpeakerIdError::LowConfidence { report, threshold }) => {
            if let Some(pending_path) = write_pending_path {
                write_pending_entry(
                    pending_path,
                    input_path,
                    &report,
                    threshold,
                    &donor_chat,
                    &inserted_code,
                    &inserted_role,
                );
            }
            exit_with_speaker_id_error(SpeakerIdError::LowConfidence { report, threshold })
        }
        Err(e) => exit_with_speaker_id_error(e),
    };

    let mut mapping = MappingSpec::new();
    mapping.insert(report.winner.clone(), SpeakerAssignment::Drop);
    for spk in donor_chat.unique_utterance_speakers() {
        if spk != report.winner {
            mapping.insert(
                spk,
                SpeakerAssignment::Rename {
                    code: inserted_code.clone(),
                    role: inserted_role.clone(),
                },
            );
        }
    }

    // Reuse the already-parsed donor AST, `apply_mapping_chat`
    // skips the redundant second `parse_and_validate` that the
    // string-entry `apply_mapping` would otherwise do.
    let relabeled = apply_mapping_chat(&donor_chat, &mapping);

    ReferenceModeOutcome {
        relabeled,
        report,
        mapping,
        inserted_code,
        inserted_role_tag: inserted_role,
    }
}

/// Override-file replay mode: load the file, look up the recorded
/// entry by session ID, apply it. Standalone-CLI entry point.
pub(crate) fn run_override_file_mode(
    input_content: &str,
    override_path: &Path,
    session_id: &str,
    options: ParseValidateOptions,
) -> String {
    let file = match OverrideFile::read_or_default(override_path) {
        Ok(f) => f,
        Err(e) => exit_with_override_file_error(override_path, e),
    };
    let entry = match file.get(session_id) {
        Some(e) => e,
        None => {
            let available: Vec<String> = file.session_ids().map(str::to_string).collect();
            exit_with_speaker_id_error(SpeakerIdError::SessionIdNotFound {
                session_id: session_id.to_string(),
                available,
            })
        }
    };
    apply_override_entry(input_content, entry, options)
}

/// Apply an already-loaded override entry to donor content. Pipeline
/// callers that have already parsed the override file once skip the
/// re-read by going through this function directly.
pub(crate) fn apply_override_entry(
    input_content: &str,
    entry: &MergeOverride,
    options: ParseValidateOptions,
) -> String {
    let mapping = entry.to_mapping_spec();
    match apply_mapping(input_content, &mapping, options) {
        Ok(s) => s,
        Err(e) => exit_with_speaker_id_error(e),
    }
}

// ---------------------------------------------------------------------------
// Holistic-LLM mode (`--judgment holistic`)
// ---------------------------------------------------------------------------

/// Environment-variable fallback for the session-context JSON path.
/// Consulted only when `--session-context` is absent, matching the
/// `CHATTER_LLM_*` fallback style below. `pub(crate)` so the batch
/// driver can strip it from deterministic per-session subprocesses
/// after warning once itself.
pub(crate) const ENV_SESSION_CONTEXT: &str = "CHATTER_SESSION_CONTEXT";

/// Where a configured session-context path came from. Carried alongside
/// the resolved path so error and warning messages name the surface the
/// operator actually used, naming `--session-context` when the path in
/// fact came from the env fallback would send them hunting for a flag
/// that is not in their command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionContextSource {
    /// The `--session-context` CLI flag.
    Flag,
    /// The `CHATTER_SESSION_CONTEXT` environment-variable fallback.
    Env,
}

impl SessionContextSource {
    /// Operator-facing name of the configuration surface.
    fn operator_name(self) -> &'static str {
        match self {
            Self::Flag => "--session-context",
            Self::Env => ENV_SESSION_CONTEXT,
        }
    }
}

/// Resolve the configured session-context path and its provenance.
/// The `--session-context` flag value wins; when absent, the
/// `CHATTER_SESSION_CONTEXT` environment variable supplies the path
/// (empty values count as unset). `None` when neither names a file.
fn resolve_session_context(
    flag_path: Option<&Path>,
) -> Option<(std::path::PathBuf, SessionContextSource)> {
    match flag_path {
        Some(p) => Some((p.to_path_buf(), SessionContextSource::Flag)),
        None => std::env::var_os(ENV_SESSION_CONTEXT)
            .filter(|v| !v.is_empty())
            .map(|v| (std::path::PathBuf::from(v), SessionContextSource::Env)),
    }
}

/// Warn on stderr when a session-context file is configured (flag or
/// env fallback) but the current judgment mode never consults it.
/// Deliberately a warning, not an error: deterministic runs must keep
/// working, but an operator-supplied input must never be ignored
/// silently. Called by the deterministic paths of `chatter speaker-id`,
/// `chatter pipeline`, and `chatter batch`.
pub(crate) fn warn_session_context_ignored_if_configured(flag_path: Option<&Path>) {
    if let Some((path, source)) = resolve_session_context(flag_path) {
        warn!(
            "session context {} configured but judgment is not holistic; ignored",
            path.display()
        );
        eprintln!(
            "Warning: {} ({}) is ignored: session context is only consulted by \
             --judgment holistic",
            source.operator_name(),
            path.display()
        );
    }
}

/// Resolve and load the session-context JSON file if one is configured
/// (see [`resolve_session_context`] for the flag-then-env resolution).
///
/// A configured-but-unreadable or malformed file is a hard error
/// (exit [`EXIT_INPUT_ERROR`]) naming the surface the path came from:
/// the operator named a file they intended to use, so silently falling
/// back to all-unknown context would silently corrupt the judgment.
fn load_session_context(flag_path: Option<&Path>) -> Option<SessionContextFile> {
    let (resolved, source) = resolve_session_context(flag_path)?;
    match SessionContextFile::read_json(&resolved) {
        Ok(file) => Some(file),
        Err(e) => {
            eprintln!(
                "Error reading session context from {}: {e}",
                source.operator_name()
            );
            std::process::exit(EXIT_INPUT_ERROR);
        }
    }
}

/// Environment-variable fallback for the LLM endpoint base URL. Consulted
/// only when `--llm-endpoint` is absent.
const ENV_LLM_ENDPOINT: &str = "CHATTER_LLM_ENDPOINT";

/// Environment-variable fallback for the LLM model id. Consulted only when
/// `--llm-model` is absent.
const ENV_LLM_MODEL: &str = "CHATTER_LLM_MODEL";

/// Environment-variable fallback for the LLM Bearer API key. Consulted only
/// when `--llm-api-key` is absent. The key remains optional.
const ENV_LLM_API_KEY: &str = "CHATTER_LLM_API_KEY";

/// All inputs to one holistic-LLM `chatter speaker-id` invocation.
///
/// Holistic mode produces a single LLM judgment per session and lands it in
/// the `--write-pending` file as a review-gated suggestion; it never writes
/// relabeled CHAT to `--output`. The LLM config fields mirror the `--llm-*`
/// flags; each missing flag falls back to the matching `CHATTER_LLM_*`
/// environment variable (endpoint and model are required after that
/// fallback; the API key stays optional).
///
/// `pub(crate)` so that `commands::pipeline` can call the holistic handler
/// directly without duplicating the LLM wiring.
pub(crate) struct HolisticModeArgs<'a> {
    /// Donor input path, needed for session-ID derivation and messages.
    pub(crate) input: &'a Path,
    /// Already-loaded donor CHAT text (the caller's `fs::read_to_string`).
    pub(crate) input_content: &'a str,
    /// Anchor (child) speaker code; required in holistic mode.
    pub(crate) anchor: Option<&'a str>,
    /// Pending-file destination for the review-gated suggestion; required.
    pub(crate) write_pending_path: Option<&'a Path>,
    /// `--llm-endpoint` flag value (falls back to `CHATTER_LLM_ENDPOINT`).
    pub(crate) llm_endpoint: Option<&'a str>,
    /// `--llm-model` flag value (falls back to `CHATTER_LLM_MODEL`).
    pub(crate) llm_model: Option<&'a str>,
    /// `--llm-api-key` flag value (falls back to `CHATTER_LLM_API_KEY`).
    pub(crate) llm_api_key: Option<&'a str>,
    /// `--llm-timeout-secs` flag value; provider default when `None`.
    pub(crate) llm_timeout_secs: Option<u64>,
    /// `--llm-max-retries` flag value; provider default when `None`.
    pub(crate) llm_max_retries: Option<u32>,
    /// `--session-context` flag value (falls back to
    /// `CHATTER_SESSION_CONTEXT`). When neither names a file, context
    /// fields fall back to the donor's `@ID` age or unknown.
    pub(crate) session_context_path: Option<&'a Path>,
}

/// Holistic-LLM mode: build a deterministic judgment request from the donor,
/// ask the configured endpoint for one holistic judgment, and append the
/// result to `--write-pending` as an `engine = "llm"` review-gated entry.
///
/// Fail-loud contract (no silent fallback to the deterministic path):
/// - missing `--write-pending`: exit [`EXIT_PRECONDITION`] (2).
/// - missing `--anchor`: exit [`EXIT_PRECONDITION`] (2).
/// - missing endpoint or model after the `CHATTER_LLM_*` fallback: exit
///   [`EXIT_PRECONDITION`] (2).
/// - donor parse/validate failure: exit [`EXIT_INPUT_ERROR`] (1).
/// - LLM/provider runtime failure: exit [`EXIT_INPUT_ERROR`] (1).
/// - judgment-to-pending consume failure (multiple adults, missing adult
///   role, self-contradictory merge): exit [`EXIT_INPUT_ERROR`] (1), failing
///   closed rather than writing a misleading entry.
///
/// On success the suggestion is appended and the process exits 0 via a normal
/// return.
///
/// `pub(crate)` so that `commands::pipeline` can call this handler for
/// `--judgment holistic` without duplicating the LLM wiring.
pub(crate) fn run_holistic_mode(args: HolisticModeArgs<'_>) {
    let HolisticModeArgs {
        input,
        input_content,
        anchor,
        write_pending_path,
        llm_endpoint,
        llm_model,
        llm_api_key,
        llm_timeout_secs,
        llm_max_retries,
        session_context_path,
    } = args;

    // 1. Holistic mode is review-gated: its only output is a pending entry, so
    //    `--write-pending` is mandatory.
    let pending_path = match write_pending_path {
        Some(p) => p,
        None => {
            eprintln!(
                "Error: --judgment holistic requires --write-pending FILE (the holistic \
                 suggestion is review-gated and written there, not to --output)"
            );
            std::process::exit(EXIT_PRECONDITION);
        }
    };

    // 2. The anchor (child) speaker code identifies which donor speaker is the
    //    child; required to build the judgment request.
    let anchor = match anchor {
        Some(a) => a,
        None => {
            eprintln!("Error: --judgment holistic requires --anchor CODE (e.g. --anchor CHI)");
            std::process::exit(EXIT_PRECONDITION);
        }
    };
    let anchor_code = SpeakerCode::new(anchor);

    // 3. Resolve endpoint / model / api_key from flags, falling back to the
    //    CHATTER_LLM_* environment variables. NO silent fallback to the
    //    deterministic path: a missing endpoint or model is a hard error.
    let endpoint = resolve_required(llm_endpoint, ENV_LLM_ENDPOINT);
    let model = resolve_required(llm_model, ENV_LLM_MODEL);
    let (endpoint, model) = match (endpoint, model) {
        (Some(e), Some(m)) => (e, m),
        _ => {
            eprintln!(
                "Error: holistic mode requires --llm-endpoint and --llm-model (or \
                 CHATTER_LLM_ENDPOINT / CHATTER_LLM_MODEL)"
            );
            std::process::exit(EXIT_PRECONDITION);
        }
    };
    let api_key = llm_api_key
        .map(str::to_string)
        .or_else(|| std::env::var(ENV_LLM_API_KEY).ok());

    // 3b. Load the session-context file (flag, else CHATTER_SESSION_CONTEXT
    //     env var). A configured-but-malformed file exits here; no file at
    //     all means every context field resolves to @ID-age or unknown.
    let context_file = load_session_context(session_context_path);

    // 4. Parse + validate the donor, mirroring reference mode's donor handling.
    let options = ParseValidateOptions::default();
    let donor_chat = match parse_and_validate(input_content, options) {
        Ok(c) => c,
        Err(e) => {
            warn!("donor parse failed: {}", e);
            eprintln!("Error parsing donor: {}", e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    // 5. Session ID: the donor file's basename stem (same as every other mode).
    let session_id = derive_session_id(input);

    // 6. Build the deterministic judgment request, with session-context
    //    record fields (sample type, declared roles, consent tier, age)
    //    when a context file is configured; age falls back to the donor's
    //    @ID header. Absent -> unknown.
    let ctx = session_context(context_file.as_ref(), &session_id, &donor_chat);
    let request = JudgmentRequest {
        session_id: SessionId(session_id.clone()),
        sample_type: ctx.sample_type,
        declared_roles: ctx.declared_roles,
        consent_tier: ctx.consent_tier,
        age_months: ctx.age_months,
        anchor: anchor_code.clone(),
        samples: sample_session(&donor_chat, &anchor_code, SampleBudget::default()),
    };

    // 7. Build the HTTP provider config; flags override the provider's built-in
    //    timeout / retry defaults only when supplied.
    let mut config = HttpProviderConfig::new(EndpointUrl(endpoint.clone()), ModelId(model.clone()));
    config.api_key = api_key.map(ApiKey);
    if let Some(secs) = llm_timeout_secs {
        config.timeout = TimeoutSecs(secs);
    }
    if let Some(retries) = llm_max_retries {
        config.max_retries = RetryCount(retries);
    }
    let provider = HttpJudgmentProvider::new(config);

    // 8. Ask the endpoint for one holistic judgment. Any provider/transport/
    //    parse failure is fatal in this cut (no silent swallow).
    let judgment = match provider.judge(&request) {
        Ok(j) => j,
        Err(e) => {
            warn!("holistic judgment failed: {}", e);
            eprintln!("Error: holistic judgment failed: {}", e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    // 9. Stamp model / endpoint / prompt-version provenance onto the entry.
    let meta = ProvenanceMeta {
        model: ModelId(model),
        endpoint: EndpointUrl(endpoint),
        prompt_version: PromptVersion(CURRENT_PROMPT_VERSION.to_string()),
    };

    // 10. Convert the judgment into a pending entry. Consume errors (multiple
    //     adults, missing adult role, self-contradictory merge) fail closed
    //     rather than writing a misleading suggestion.
    let entry = match judgment_to_pending(&session_id, &judgment, &meta, chrono::Utc::now()) {
        Ok(e) => e,
        Err(e) => {
            warn!("judgment could not be consumed into a pending entry: {}", e);
            eprintln!(
                "Error: could not build a pending entry from the judgment: {}",
                e
            );
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    // 11. Append the review-gated suggestion to the pending file and report.
    append_pending_entry(pending_path, entry);
    info!(
        "holistic suggestion for session {} written to {}",
        session_id,
        pending_path.display()
    );
    println!(
        "Wrote holistic speaker-id suggestion for session {} to {}",
        session_id,
        pending_path.display()
    );
}

/// Resolve a required LLM config value: the flag value if present, else the
/// named environment variable. Returns `None` only when both are absent (the
/// caller turns that into a fail-loud precondition error). Empty-string
/// values (flag or env) are rejected as missing so a blank `--llm-endpoint`
/// cannot masquerade as configured.
fn resolve_required(flag: Option<&str>, env_var: &str) -> Option<String> {
    flag.map(str::to_string)
        .or_else(|| std::env::var(env_var).ok())
        .filter(|s| !s.trim().is_empty())
}
