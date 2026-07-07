//! End-to-end seam test: `chatter speaker-id --judgment holistic` against a
//! mocked OpenAI-compatible endpoint. This is the top-level integration test
//! for the holistic-judgment feature: it drives the CLI subprocess, the
//! prompt rendering, the HTTP provider, and the LLM-stamped pending write.

use std::process::Command;

use httpmock::{Method::POST, MockServer};

/// The holistic JSON the mocked model "returns" as the chat-completion
/// content. Donor speaker PAR1 is the adult interviewer; PAR0 is the child.
/// The donor fixture (ell-conversation.cha) has both PAR0 and PAR1.
const HOLISTIC_JSON: &str = r#"{
  "speaker_mapping": { "PAR0": "CHI", "PAR1": "adult" },
  "adult_roles": { "PAR1": "INV" },
  "sample_type": "confirmed",
  "merge_applicable": true,
  "confidence": { "mapping": 0.95, "roles": 0.9, "merge_applicable": 0.95 },
  "reasoning": "PAR1 produces interviewer prompts; PAR0 gives short child answers."
}"#;

#[test]
fn speaker_id_holistic_writes_llm_stamped_pending() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [ { "message": { "role": "assistant", "content": HOLISTIC_JSON } } ]
    });
    let _mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    // A real two-speaker donor fixture from the reference corpus (PAR0 + PAR1).
    let donor = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/languages/ell-conversation.cha"
    );

    let tmp = tempfile::tempdir().expect("tempdir");
    let pending = tmp.path().join("pending.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args([
            "speaker-id",
            "--judgment",
            "holistic",
            "--anchor",
            "CHI",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--write-pending",
            pending.to_str().expect("utf8 path"),
            donor,
        ])
        .output()
        .expect("run chatter");

    assert!(
        output.status.success(),
        "chatter exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let pending_text = std::fs::read_to_string(&pending).expect("pending written");
    assert!(
        pending_text.contains(r#"engine = "llm""#),
        "pending entry must be stamped engine=llm; got:\n{pending_text}"
    );
    assert!(
        pending_text.contains("kind = \"speaker-id-low-confidence\""),
        "expected a speaker-id-low-confidence pending kind; got:\n{pending_text}"
    );
    assert!(
        pending_text.contains("PAR1") && pending_text.contains("interviewer prompts"),
        "pending entry must carry the suggested mapping and LLM reasoning; got:\n{pending_text}"
    );
}

/// Minimal two-speaker donor (anonymous PAR0/PAR1) for the session-context
/// test; written to a temp file named after the context key. Valid CHAT
/// (4-pipe @ID role field).
const DONOR: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR0 Participant, PAR1 Participant\n@ID:\teng|frog|PAR0|||||Participant|||\n@ID:\teng|frog|PAR1|||||Participant|||\n@Media:\tsmoke, audio\n*PAR0:\twhere did the frog go . \u{15}0_2000\u{15}\n*PAR1:\ttell me about the picture . \u{15}2000_2500\u{15}\n*PAR0:\tthe frog fell in the jar . \u{15}2500_4500\u{15}\n@End\n";

/// Session-context JSON for session `NF201-3` exercising all four optional
/// fields (free vocabulary by design; surfaced verbatim to the LLM).
const SESSION_CONTEXT_JSON: &str = r#"{
  "NF201-3": {
    "sample_type": "clinician interview",
    "declared_roles": ["Investigator"],
    "consent_tier": "video+audio",
    "age_months": 52
  }
}"#;

/// A mock that only matches when the judgment request body carries every
/// expected substring. If `chatter speaker-id` fails to thread the
/// session-context labels into the prompt, no mock matches, the provider
/// sees an error response, and the exit-status assertion fails.
fn mock_server_expecting_body(substrings: &[&str]) -> MockServer {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [ { "message": { "role": "assistant", "content": HOLISTIC_JSON } } ]
    });
    server.mock(|mut when, then| {
        when = when.method(POST).path("/v1/chat/completions");
        for s in substrings {
            when = when.body_includes(*s);
        }
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });
    server
}

/// `--session-context FILE` through the SPEAKER-ID subcommand specifically:
/// the record's free-vocabulary labels must reach the judgment request
/// verbatim (sample type, declared roles, consent tier, age in months).
/// The pipeline/batch seams have their own copies of this contract in
/// `holistic_pipeline_batch_cli.rs`.
#[test]
fn speaker_id_holistic_session_context_labels_reach_judgment_request() {
    let server = mock_server_expecting_body(&[
        "sample_type: clinician interview",
        "declared_adult_roles: Investigator",
        "consent_tier: video+audio",
        "child_age_months: 52",
    ]);

    let tmp = tempfile::tempdir().expect("tempdir");
    // Session ID is the donor basename stem, so the file name must match
    // the context key.
    let donor = tmp.path().join("NF201-3.cha");
    let context = tmp.path().join("session-context.json");
    let pending = tmp.path().join("pending.toml");
    std::fs::write(&donor, DONOR).expect("write donor");
    std::fs::write(&context, SESSION_CONTEXT_JSON).expect("write context");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args([
            "speaker-id",
            "--judgment",
            "holistic",
            "--anchor",
            "CHI",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--session-context",
            context.to_str().expect("utf8 path"),
            "--write-pending",
            pending.to_str().expect("utf8 path"),
            donor.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("run chatter");

    assert!(
        output.status.success(),
        "chatter exited non-zero (labels likely missing from the judgment \
         request): stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pending_text = std::fs::read_to_string(&pending).expect("pending written");
    assert!(
        pending_text.contains(r#"engine = "llm""#),
        "pending entry must be stamped engine=llm; got:\n{pending_text}"
    );
}

/// `--llm-cache FILE`: a second `chatter speaker-id --judgment holistic`
/// invocation for the same donor is answered from the cache, so the mock
/// endpoint sees exactly one request across both runs.
#[test]
fn speaker_id_holistic_llm_cache_flag_avoids_second_request() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [ { "message": { "role": "assistant", "content": HOLISTIC_JSON } } ]
    });
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let donor = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/languages/ell-conversation.cha"
    );

    let tmp = tempfile::tempdir().expect("tempdir");
    let cache_path = tmp.path().join("llm-cache.json");

    let run = |pending_name: &str| {
        let pending = tmp.path().join(pending_name);
        let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
            .args([
                "speaker-id",
                "--judgment",
                "holistic",
                "--anchor",
                "CHI",
                "--llm-endpoint",
                &format!("{}/v1", server.base_url()),
                "--llm-model",
                "deepseek-v4-flash",
                "--llm-cache",
                cache_path.to_str().expect("utf8 path"),
                "--write-pending",
                pending.to_str().expect("utf8 path"),
                donor,
            ])
            .output()
            .expect("run chatter");
        assert!(
            output.status.success(),
            "chatter exited non-zero: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    };

    run("pending-1.toml");
    run("pending-2.toml");

    mock.assert_calls(1); // the second run's identical request was cache-served
}

/// `CHATTER_LLM_CACHE` env fallback: an env-configured cache (no `--llm-cache`
/// flag) is honored the same way the flag is.
#[test]
fn speaker_id_holistic_llm_cache_env_fallback() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [ { "message": { "role": "assistant", "content": HOLISTIC_JSON } } ]
    });
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let donor = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/languages/ell-conversation.cha"
    );

    let tmp = tempfile::tempdir().expect("tempdir");
    let cache_path = tmp.path().join("llm-cache.json");

    let run = |pending_name: &str| {
        let pending = tmp.path().join(pending_name);
        let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
            .env("CHATTER_LLM_CACHE", &cache_path)
            .args([
                "speaker-id",
                "--judgment",
                "holistic",
                "--anchor",
                "CHI",
                "--llm-endpoint",
                &format!("{}/v1", server.base_url()),
                "--llm-model",
                "deepseek-v4-flash",
                "--write-pending",
                pending.to_str().expect("utf8 path"),
                donor,
            ])
            .output()
            .expect("run chatter");
        assert!(
            output.status.success(),
            "chatter exited non-zero: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    };

    run("pending-1.toml");
    run("pending-2.toml");

    mock.assert_calls(1); // CHATTER_LLM_CACHE alone was enough to cache the second run
}
