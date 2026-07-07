//! Shared judgment-engine arguments for the `speaker-id`, `pipeline`,
//! and `batch` subcommands.
//!
//! The three subcommands expose an identical judgment surface (engine
//! selection, LLM connection, optional session-context input). Defining
//! it once and flattening it (`#[command(flatten)]`) into each variant
//! keeps the three CLI surfaces identical by construction and keeps
//! `core.rs` (the `Commands` enum spanning every subcommand) under the
//! file-size limit.

use std::path::PathBuf;

use clap::Args;

use super::cli_types::JudgmentMode;

/// Judgment-engine configuration shared by `speaker-id`, `pipeline`,
/// and `batch`: which engine powers the speaker judgment, the LLM
/// connection used by the holistic engine, and the optional
/// session-context JSON input. `batch` threads each of these through to
/// every per-session `chatter pipeline` subprocess.
#[derive(Args)]
pub struct JudgmentArgs {
    /// How the judgment is powered (deterministic or holistic LLM).
    #[arg(long, value_enum, default_value_t = JudgmentMode::Deterministic)]
    pub judgment: JudgmentMode,

    /// LLM endpoint base URL (OpenAI-compatible), e.g.
    /// http://localhost:8000/v1. Env: CHATTER_LLM_ENDPOINT. Required
    /// for --judgment holistic.
    #[arg(long = "llm-endpoint")]
    pub llm_endpoint: Option<String>,

    /// LLM model id (e.g. deepseek-v4-flash). Env: CHATTER_LLM_MODEL.
    /// Required for --judgment holistic.
    #[arg(long = "llm-model")]
    pub llm_model: Option<String>,

    /// LLM API key for Bearer auth. Env: CHATTER_LLM_API_KEY. Optional.
    #[arg(long = "llm-api-key")]
    pub llm_api_key: Option<String>,

    /// LLM request timeout in seconds.
    #[arg(long = "llm-timeout-secs")]
    pub llm_timeout_secs: Option<u64>,

    /// LLM max retries on transport / 5xx errors.
    #[arg(long = "llm-max-retries")]
    pub llm_max_retries: Option<u32>,

    /// Response-cache file for holistic judgments. When set, identical
    /// requests (same endpoint, model, and rendered prompt) are answered
    /// from the cache, so re-running a batch after a crash or a code tweak
    /// does not re-pay completed LLM calls. Env: CHATTER_LLM_CACHE. Absent
    /// means uncached (today's behavior).
    #[arg(long = "llm-cache")]
    pub llm_cache: Option<PathBuf>,

    /// Optional session-context JSON file mapping session IDs to
    /// context records (sample_type, declared_roles, consent_tier,
    /// age_months; all optional, free vocabulary). --judgment
    /// holistic surfaces the session's record verbatim into the
    /// LLM prompt. Env: CHATTER_SESSION_CONTEXT. Absent sessions
    /// or fields are passed as unknown.
    #[arg(long = "session-context")]
    pub session_context: Option<PathBuf>,
}
