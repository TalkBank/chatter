//! The OpenAI-compatible HTTP `JudgmentProvider`.
//!
//! Wire shape: a single POST to `{endpoint}/chat/completions` carrying a
//! `ChatCompletionRequest` (model, `temperature = 0`, the rendered system +
//! user messages, and a `json_object` response format). The model's single
//! choice message content is parsed as a [`HolisticJudgment`].
//!
//! All transport, status, and parse failures are mapped to the
//! provider-agnostic [`JudgmentError`] from `talkbank-transform`. This module
//! contains the toolchain's only network call.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ureq::Agent;

use talkbank_transform::speaker_id::{
    EndpointUrl, HolisticJudgment, JudgmentError, JudgmentProvider, JudgmentRequest, ModelId, Role,
    render_messages,
};

use crate::cache::ResponseCache;

// ---------------------------------------------------------------------------
// Named constants (no bare numeric literals in logic)
// ---------------------------------------------------------------------------

/// Default per-request global timeout, in seconds. Covers connect + send +
/// receive for one attempt (ureq's `timeout_global`).
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Default number of additional attempts after the first on a retryable
/// failure (transport error or HTTP 5xx). Zero means "try once, never retry".
const DEFAULT_MAX_RETRIES: u32 = 2;

/// Sampling temperature for the completion. Always zero: the judgment task is
/// deterministic classification, not generation, so we want the model's
/// most-likely structured output with no sampling noise.
const TEMPERATURE_ZERO: u8 = 0;

/// The OpenAI `response_format.type` value requesting a raw JSON object.
const RESPONSE_FORMAT_JSON_OBJECT: &str = "json_object";

/// The path appended to the configured endpoint for chat completions.
const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

/// The role string for a system message in the OpenAI wire format.
const WIRE_ROLE_SYSTEM: &str = "system";

/// The role string for a user message in the OpenAI wire format.
const WIRE_ROLE_USER: &str = "user";

/// The `Content-Type` for the JSON request body.
const CONTENT_TYPE_JSON: &str = "application/json";

// ---------------------------------------------------------------------------
// Configuration newtypes
// ---------------------------------------------------------------------------

/// Bearer API key for the endpoint. Wrapped so an auth secret is never just a
/// bare `String` flowing through call sites; the inner value is sent verbatim
/// in the `Authorization: Bearer <key>` header.
#[derive(Debug, Clone)]
pub struct ApiKey(pub String);

/// Per-attempt request timeout, in whole seconds.
#[derive(Debug, Clone, Copy)]
pub struct TimeoutSecs(pub u64);

/// Number of additional attempts after the first on a retryable failure.
/// `RetryCount(0)` means a single attempt with no retries.
#[derive(Debug, Clone, Copy)]
pub struct RetryCount(pub u32);

/// Everything the HTTP provider needs to make one judgment call.
#[derive(Debug, Clone)]
pub struct HttpProviderConfig {
    /// Endpoint base URL (e.g. `https://api.example.com/v1`). The provider
    /// appends `CHAT_COMPLETIONS_PATH` to it.
    pub endpoint: EndpointUrl,
    /// Model identifier sent in the request body (e.g. `deepseek-v4-flash`).
    pub model: ModelId,
    /// Optional bearer key. When `Some`, an `Authorization` header is added.
    pub api_key: Option<ApiKey>,
    /// Per-attempt request timeout.
    pub timeout: TimeoutSecs,
    /// Retry budget for transport errors and HTTP 5xx responses.
    pub max_retries: RetryCount,
}

impl HttpProviderConfig {
    /// Build a config for `endpoint` + `model`, filling the timeout and retry
    /// budget from the named defaults and leaving the API key unset. Callers
    /// that need auth or non-default timing set those fields afterward.
    pub fn new(endpoint: EndpointUrl, model: ModelId) -> Self {
        Self {
            endpoint,
            model,
            api_key: None,
            timeout: TimeoutSecs(DEFAULT_TIMEOUT_SECS),
            max_retries: RetryCount(DEFAULT_MAX_RETRIES),
        }
    }
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/// An OpenAI-compatible HTTP judgment provider. Holds the config and a
/// prebuilt ureq [`Agent`] (a cheap-to-clone connection pool) so repeated
/// calls reuse connections. The optional [`ResponseCache`] is wrapped in an
/// `Arc` so the provider (and every clone of it) shares one cache handle
/// rather than each clone opening and rewriting its own copy of the file.
#[derive(Debug, Clone)]
pub struct HttpJudgmentProvider {
    /// Endpoint, model, auth, timeout, and retry budget.
    config: HttpProviderConfig,
    /// Connection pool + timeout policy, built once from `config`.
    agent: Agent,
    /// Persistent response cache. `None` means every call is a live HTTP
    /// request (today's behavior, unchanged).
    cache: Option<Arc<ResponseCache>>,
}

impl HttpJudgmentProvider {
    /// Build a provider from `config` with no response cache: every call is
    /// a live HTTP request.
    pub fn new(config: HttpProviderConfig) -> Self {
        Self {
            agent: build_agent(&config),
            config,
            cache: None,
        }
    }

    /// Build a provider from `config`, backed by `cache`. A repeated request
    /// (same endpoint and exact wire-request JSON) is served from `cache`
    /// without an HTTP call; a new request is answered over HTTP and its raw
    /// response body is written through to `cache` before being parsed.
    pub fn with_cache(config: HttpProviderConfig, cache: ResponseCache) -> Self {
        Self {
            agent: build_agent(&config),
            config,
            cache: Some(Arc::new(cache)),
        }
    }

    /// The completions URL: configured endpoint + `CHAT_COMPLETIONS_PATH`.
    fn completions_url(&self) -> String {
        format!("{}{}", self.config.endpoint.0, CHAT_COMPLETIONS_PATH)
    }

    /// Build the typed wire request from the rendered messages.
    fn build_request(&self, messages: Vec<WireMessage>) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: self.config.model.0.clone(),
            temperature: TEMPERATURE_ZERO,
            messages,
            response_format: ResponseFormat {
                kind: RESPONSE_FORMAT_JSON_OBJECT,
            },
        }
    }

    /// Perform one HTTP attempt. Returns the classified `AttemptOutcome` so
    /// the retry loop in `judge` can decide whether to try again.
    ///
    /// `url` is the pre-built completions URL (computed once by `judge`
    /// before the retry loop to avoid reallocating on each attempt).
    /// `body` is the already-serialized request JSON; serializing once outside
    /// the loop avoids re-serializing on every retry.
    fn attempt(&self, url: &str, body: &str) -> AttemptOutcome {
        let mut request = self
            .agent
            .post(url)
            .header("Content-Type", CONTENT_TYPE_JSON);
        if let Some(key) = &self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", key.0));
        }

        match request.send(body) {
            // Transport-level failure (connection refused, timeout, etc.).
            // These are always retryable.
            Err(transport) => AttemptOutcome::RetryableError(transport.to_string()),
            Ok(mut response) => {
                let status = response.status();
                let code = status.as_u16();
                if status.is_success() {
                    // Read the body; a read failure here is a transport-ish
                    // problem, treat it as retryable.
                    match response.body_mut().read_to_string() {
                        Ok(text) => AttemptOutcome::Body(text),
                        Err(read_err) => AttemptOutcome::RetryableError(format!(
                            "failed to read response body: {read_err}"
                        )),
                    }
                } else if status.is_server_error() {
                    // 5xx: retryable.
                    AttemptOutcome::RetryableError(format!("endpoint returned HTTP {code}"))
                } else {
                    // 4xx (and any other non-2xx, non-5xx): not retryable.
                    AttemptOutcome::FatalStatus(format!("endpoint returned HTTP {code}"))
                }
            }
        }
    }
}

/// Build the ureq [`Agent`] for `config`: the per-attempt global timeout,
/// and HTTP-status-as-error turned OFF so the `judge` retry loop can inspect
/// the status code itself (to distinguish retryable 5xx from non-retryable
/// 4xx). Shared by [`HttpJudgmentProvider::new`] and
/// [`HttpJudgmentProvider::with_cache`] so the two constructors cannot drift.
fn build_agent(config: &HttpProviderConfig) -> Agent {
    Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(config.timeout.0)))
        .http_status_as_error(false)
        .build()
        .into()
}

/// Cache key for one request: hex SHA-256 of the endpoint URL followed by
/// the exact serialized wire-request JSON. The rendered messages (and thus
/// any prompt or `PromptVersion` change) are part of that JSON, so a stale
/// entry can never be served without a separate version field to keep in
/// sync by hand, the same versioned-key discipline `talkbank-cache` uses,
/// applied here by folding everything that affects the answer into the key.
fn compute_cache_key(endpoint: &EndpointUrl, body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(endpoint.0.as_bytes());
    hasher.update(body.as_bytes());
    // `Sha256::finalize()`'s output type does not implement `LowerHex`
    // directly; format each byte instead of the whole digest.
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Outcome of a single HTTP attempt, used by the retry loop.
enum AttemptOutcome {
    /// A successful (2xx) response with its body text.
    Body(String),
    /// A retryable failure (transport error, body-read error, or HTTP 5xx),
    /// carrying a human-readable description for the final error message.
    RetryableError(String),
    /// A non-retryable HTTP status (4xx), carrying its description.
    FatalStatus(String),
}

impl JudgmentProvider for HttpJudgmentProvider {
    /// Render the prompt, POST it, retry on retryable failures up to the
    /// configured budget, then parse the model's message content into a
    /// [`HolisticJudgment`]. Never panics: every failure path returns a
    /// typed [`JudgmentError`].
    fn judge(&self, request: &JudgmentRequest) -> Result<HolisticJudgment, JudgmentError> {
        // 1. Build the completions URL once; each retry reuses the same string.
        let url = self.completions_url();

        // 2. Render the transport-neutral prompt and map to wire messages.
        //    `messages` is a local Vec not used after this point, so move
        //    content into each WireMessage rather than cloning.
        let (messages, _version) = render_messages(request);
        let wire_messages: Vec<WireMessage> = messages
            .into_iter()
            .map(|m| WireMessage {
                role: wire_role(m.role),
                content: m.content,
            })
            .collect();

        // 3. Build and serialize the typed request once (reused across retries).
        let wire_request = self.build_request(wire_messages);
        let body = serde_json::to_string(&wire_request)
            .map_err(|e| JudgmentError::Provider(format!("failed to serialize request: {e}")))?;

        // 3b. Cache lookup: key = sha256(endpoint + wire request JSON). The
        //     key is computed once and reused for both the lookup below and
        //     the write-through after a live fetch, so a hit and a
        //     subsequent miss can never disagree on the key. A cache hit
        //     short-circuits straight to the existing parse path, no HTTP
        //     attempt is made.
        let cache_entry = self
            .cache
            .as_ref()
            .map(|cache| (cache, compute_cache_key(&self.config.endpoint, &body)));
        if let Some((cache, key)) = &cache_entry
            && let Some(cached_body) = cache.get(key)
        {
            return parse_completion(&cached_body);
        }

        // 4. Attempt with retries. The loop runs 1 + max_retries times in the
        //    worst case (one initial attempt plus the retry budget).
        let mut last_error = String::new();
        for _ in 0..=self.config.max_retries.0 {
            match self.attempt(&url, &body) {
                AttemptOutcome::Body(text) => {
                    // 4b. Write-through before parsing: a cache-write failure
                    //     is surfaced loudly (never a silent bypass) even
                    //     though a good response was in hand.
                    if let Some((cache, key)) = &cache_entry {
                        cache
                            .put(key, text.clone())
                            .map_err(|e| JudgmentError::Provider(format!("cache: {e}")))?;
                    }
                    return parse_completion(&text);
                }
                AttemptOutcome::FatalStatus(msg) => {
                    // 4xx: do not retry.
                    return Err(JudgmentError::Provider(msg));
                }
                AttemptOutcome::RetryableError(msg) => {
                    // Remember the latest reason and loop to retry (if budget
                    // remains); when the budget is exhausted this becomes the
                    // returned error.
                    last_error = msg;
                }
            }
        }

        Err(JudgmentError::Provider(format!(
            "request failed after {} attempt(s): {last_error}",
            self.config.max_retries.0 + 1
        )))
    }
}

/// Parse a successful completion body into a [`HolisticJudgment`].
///
/// The body is the OpenAI envelope; its first choice's message content is the
/// holistic JSON. An empty `choices` array is a provider problem (the endpoint
/// returned 200 but no usable completion); a content string that does not
/// parse as `HolisticJudgment` is a malformed response.
fn parse_completion(text: &str) -> Result<HolisticJudgment, JudgmentError> {
    let envelope: ChatCompletionResponse = serde_json::from_str(text)
        .map_err(|e| JudgmentError::Provider(format!("malformed completion envelope: {e}")))?;
    let first = envelope
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| JudgmentError::Provider("no choices in response".to_string()))?;
    serde_json::from_str::<HolisticJudgment>(&first.message.content)
        .map_err(|e| JudgmentError::MalformedResponse(e.to_string()))
}

/// Map a transport-neutral [`Role`] to its OpenAI wire string. Exhaustive on
/// purpose: a new `Role` variant must be handled here, not silently defaulted.
fn wire_role(role: Role) -> &'static str {
    match role {
        Role::System => WIRE_ROLE_SYSTEM,
        Role::User => WIRE_ROLE_USER,
    }
}

// ---------------------------------------------------------------------------
// Typed wire structs (serde) -- no hand-built JSON
// ---------------------------------------------------------------------------

/// The OpenAI `/chat/completions` request body.
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    /// Model identifier.
    model: String,
    /// Sampling temperature; always `TEMPERATURE_ZERO`.
    temperature: u8,
    /// Ordered system + user messages.
    messages: Vec<WireMessage>,
    /// Requests a raw JSON-object response from the model.
    response_format: ResponseFormat,
}

/// One outgoing chat message in the OpenAI wire format. The `role` is a
/// known `&'static str` (`system` / `user`) chosen by `wire_role`, so it is
/// borrowed rather than allocated. This is the SEND side only; incoming
/// messages use `ChoiceMessage` (owned `role`) because the endpoint may
/// echo any role string such as `assistant`.
#[derive(Debug, Serialize)]
struct WireMessage {
    /// `system` or `user`.
    role: &'static str,
    /// The message text.
    content: String,
}

/// The OpenAI `response_format` object: `{"type": "json_object"}`.
#[derive(Debug, Serialize)]
struct ResponseFormat {
    /// Serialized as the JSON key `type` (a Rust keyword, hence the rename).
    #[serde(rename = "type")]
    kind: &'static str,
}

/// The OpenAI `/chat/completions` response envelope (only the fields we use).
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    /// Completion choices; we use the first one.
    choices: Vec<Choice>,
}

/// One completion choice.
#[derive(Debug, Deserialize)]
struct Choice {
    /// The assistant message; its `content` is the holistic JSON.
    message: ChoiceMessage,
}

/// An incoming chat message. Only `content` is read; the endpoint-chosen
/// `role` (typically `assistant`) is an unknown field and serde ignores it,
/// so it is not declared here.
#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    /// The holistic JSON the model produced.
    content: String,
}
