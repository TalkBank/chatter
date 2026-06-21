//! Integration tests for the OpenAI-compatible HTTP `JudgmentProvider`.
//!
//! These exercise the real HTTP path against an `httpmock` stub: request
//! shape (URL path, method, JSON body), happy-path parse of a canned model
//! completion into a `HolisticJudgment`, and the typed-error mapping for
//! HTTP 5xx, malformed model content, and empty `choices`. The retry test
//! pins the attempt count via the mock's hit counter.
//!
//! Per the standing rule, these tests are the FIRST thing written for this
//! crate (RED before any implementation).
//!
//! The crate denies `unwrap_used` / `expect_used` / `panic` for long-lived
//! code, but the standing rules allow panicking assertion idioms in test
//! code, so those lints are relaxed for this test target only.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use httpmock::{Method, MockServer};

use talkbank_llm::{ApiKey, HttpJudgmentProvider, HttpProviderConfig, RetryCount, TimeoutSecs};
use talkbank_model::SpeakerCode;
use talkbank_transform::speaker_id::DonorCode;
use talkbank_transform::speaker_id::{
    EndpointUrl, JudgmentError, JudgmentProvider, JudgmentRequest, ModelId, SessionId,
    SpeakerVerdict,
};

/// The canned holistic JSON the stub returns as the assistant message
/// content. This is exactly the shape `HolisticJudgment` deserializes.
const HOLISTIC_JSON: &str = r#"{
  "speaker_mapping": { "PAR0": "CHI", "PAR1": "adult" },
  "adult_roles": { "PAR1": "INV" },
  "sample_type": "confirmed",
  "merge_applicable": true,
  "confidence": { "mapping": 0.9, "roles": 0.8, "merge_applicable": 0.95 },
  "reasoning": "PAR1 prompts; PAR0 answers."
}"#;

/// Model identifier asserted in the request-body test and used in every
/// provider config below.
const TEST_MODEL: &str = "deepseek-v4-flash";

/// The stub completions path. Matches real OpenAI-compatible usage: the
/// provider appends `/chat/completions` to the configured `/v1` endpoint.
const COMPLETIONS_PATH: &str = "/v1/chat/completions";

/// Build a minimal `JudgmentRequest`. Empty samples are enough to exercise
/// the HTTP path; the prompt renderer still produces a non-empty system +
/// user message pair.
fn minimal_request() -> JudgmentRequest {
    JudgmentRequest {
        session_id: SessionId("itest_session".into()),
        sample_type: None,
        declared_roles: Vec::new(),
        consent_tier: None,
        age_months: None,
        anchor: SpeakerCode::new("CHI"),
        samples: Vec::new(),
    }
}

/// Build a provider config pointing at the mock server's `/v1` endpoint with
/// the given retry budget. The mock speaks plain HTTP, so no TLS is needed.
fn config_for(server: &MockServer, max_retries: RetryCount) -> HttpProviderConfig {
    HttpProviderConfig {
        endpoint: EndpointUrl(format!("{}/v1", server.base_url())),
        model: ModelId(TEST_MODEL.into()),
        api_key: None,
        timeout: TimeoutSecs(5),
        max_retries,
    }
}

/// A 200 with a well-formed completion whose content is `HOLISTIC_JSON`
/// parses into a `HolisticJudgment` with the expected mapping.
#[test]
fn judge_parses_canned_completion() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST).path(COMPLETIONS_PATH);
        then.status(200).json_body(serde_json::json!({
            "choices": [
                { "message": { "role": "assistant", "content": HOLISTIC_JSON } }
            ]
        }));
    });

    let provider = HttpJudgmentProvider::new(config_for(&server, RetryCount(0)));
    let judgment = provider
        .judge(&minimal_request())
        .expect("judge should parse the canned completion");

    mock.assert();
    assert!(
        judgment.merge_applicable,
        "merge_applicable must be true per the canned JSON"
    );
    assert_eq!(
        judgment.speaker_mapping.get(&DonorCode("PAR1".into())),
        Some(&SpeakerVerdict::Adult),
        "PAR1 must map to the adult verdict"
    );
}

/// The POSTed request body must carry the configured model, a zero
/// temperature, and a non-empty messages array.
#[test]
fn request_body_has_model_and_temperature_zero() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST)
            .path(COMPLETIONS_PATH)
            // Partial JSON match: model + temperature must be present.
            .json_body_includes(r#"{"model":"deepseek-v4-flash","temperature":0}"#)
            // The serialized request must contain a messages array key.
            .body_includes("\"messages\"");
        then.status(200).json_body(serde_json::json!({
            "choices": [
                { "message": { "role": "assistant", "content": HOLISTIC_JSON } }
            ]
        }));
    });

    let provider = HttpJudgmentProvider::new(config_for(&server, RetryCount(0)));
    provider
        .judge(&minimal_request())
        .expect("judge should succeed with the canned completion");

    // Exactly one request matching the body expectations was made.
    mock.assert_calls(1);
}

/// An HTTP 500 maps to `JudgmentError::Provider`.
#[test]
fn http_500_is_provider_error() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(Method::POST).path(COMPLETIONS_PATH);
        then.status(500).body("upstream exploded");
    });

    let provider = HttpJudgmentProvider::new(config_for(&server, RetryCount(0)));
    let err = provider
        .judge(&minimal_request())
        .expect_err("a 500 must be an error");
    assert!(
        matches!(err, JudgmentError::Provider(_)),
        "a 500 must map to Provider, got {err:?}"
    );
}

/// A 200 whose model content is not valid holistic JSON maps to
/// `JudgmentError::MalformedResponse`.
#[test]
fn malformed_content_is_malformed_response() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(Method::POST).path(COMPLETIONS_PATH);
        then.status(200).json_body(serde_json::json!({
            "choices": [
                { "message": { "role": "assistant", "content": "not json" } }
            ]
        }));
    });

    let provider = HttpJudgmentProvider::new(config_for(&server, RetryCount(0)));
    let err = provider
        .judge(&minimal_request())
        .expect_err("non-JSON content must be an error");
    assert!(
        matches!(err, JudgmentError::MalformedResponse(_)),
        "non-JSON content must map to MalformedResponse, got {err:?}"
    );
}

/// A 200 with an empty `choices` array maps to `JudgmentError::Provider`.
#[test]
fn empty_choices_is_provider_error() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(Method::POST).path(COMPLETIONS_PATH);
        then.status(200)
            .json_body(serde_json::json!({ "choices": [] }));
    });

    let provider = HttpJudgmentProvider::new(config_for(&server, RetryCount(0)));
    let err = provider
        .judge(&minimal_request())
        .expect_err("empty choices must be an error");
    assert!(
        matches!(err, JudgmentError::Provider(_)),
        "empty choices must map to Provider, got {err:?}"
    );
}

/// A persistent 5xx is retried `max_retries` additional times, then fails
/// with a Provider error. The mock must have been hit `1 + max_retries`
/// times total.
#[test]
fn retries_then_fails_on_persistent_5xx() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST).path(COMPLETIONS_PATH);
        then.status(503).body("still down");
    });

    let max_retries = RetryCount(2);
    let provider = HttpJudgmentProvider::new(config_for(&server, max_retries));
    let err = provider
        .judge(&minimal_request())
        .expect_err("persistent 5xx must fail");
    assert!(
        matches!(err, JudgmentError::Provider(_)),
        "persistent 5xx must map to Provider, got {err:?}"
    );

    // One initial attempt plus max_retries retries.
    assert_eq!(
        mock.calls(),
        1 + max_retries.0 as usize,
        "must attempt 1 + max_retries times on persistent 5xx"
    );
}

/// A 4xx client error (bad request, auth failure, etc.) must NOT be retried:
/// the provider returns immediately after a single call. Guards the policy
/// that only transport errors and 5xx are retryable; retrying a 4xx would
/// hammer an endpoint on an unrecoverable client error.
#[test]
fn client_error_4xx_is_not_retried() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST).path(COMPLETIONS_PATH);
        then.status(400).body("bad request");
    });

    let max_retries = RetryCount(3);
    let provider = HttpJudgmentProvider::new(config_for(&server, max_retries));
    let err = provider
        .judge(&minimal_request())
        .expect_err("4xx must fail");
    assert!(
        matches!(err, JudgmentError::Provider(_)),
        "4xx must map to Provider, got {err:?}"
    );

    // 4xx is fatal: exactly one attempt, no retries despite max_retries=3.
    assert_eq!(mock.calls(), 1, "4xx must not be retried");
}

/// When an API key is configured the provider sends an
/// `Authorization: Bearer <key>` header. This is asserted with the header
/// matcher so a regression in auth wiring fails loudly.
#[test]
fn sends_bearer_auth_header_when_key_present() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST)
            .path(COMPLETIONS_PATH)
            .header("authorization", "Bearer sk-test-123");
        then.status(200).json_body(serde_json::json!({
            "choices": [
                { "message": { "role": "assistant", "content": HOLISTIC_JSON } }
            ]
        }));
    });

    let mut config = config_for(&server, RetryCount(0));
    config.api_key = Some(ApiKey("sk-test-123".into()));
    let provider = HttpJudgmentProvider::new(config);
    provider
        .judge(&minimal_request())
        .expect("judge should succeed and carry the auth header");

    mock.assert_calls(1);
}
