// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! End-to-end seam tests: `chatter pipeline --judgment holistic` and
//! `chatter batch --judgment holistic` against a mocked OpenAI-compatible
//! endpoint. Holistic is pending-only: it writes an engine=llm pending entry
//! and produces NO merged file.

use std::fs;
use std::process::Command;

use httpmock::{Method::POST, MockServer};

/// The holistic JSON the mocked model returns as the completion content.
/// PAR1 is the adult interviewer; PAR0 is the child.
const HOLISTIC_JSON: &str = r#"{
  "speaker_mapping": { "PAR0": "CHI", "PAR1": "adult" },
  "adult_roles": { "PAR1": "INV" },
  "sample_type": "confirmed",
  "merge_applicable": true,
  "confidence": { "mapping": 0.95, "roles": 0.9, "merge_applicable": 0.95 },
  "reasoning": "PAR1 produces interviewer prompts; PAR0 gives short child answers."
}"#;

/// Minimal two-speaker donor (anonymous PAR0/PAR1) + a CHI reference, written
/// to temp files. Valid CHAT (4-pipe @ID role field).
const DONOR: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR0 Participant, PAR1 Participant\n@ID:\teng|frog|PAR0|||||Participant|||\n@ID:\teng|frog|PAR1|||||Participant|||\n@Media:\tsmoke, audio\n*PAR0:\twhere did the frog go . \u{15}0_2000\u{15}\n*PAR1:\ttell me about the picture . \u{15}2000_2500\u{15}\n*PAR0:\tthe frog fell in the jar . \u{15}2500_4500\u{15}\n@End\n";
const REFERENCE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|frog|CHI|3;06.||||Target_Child|||\n@Media:\tsmoke, audio\n*CHI:\twhere did the frog go . \u{15}0_2000\u{15}\n*CHI:\tthe frog fell in the jar . \u{15}2500_4500\u{15}\n@End\n";

/// Same donor but PAR0's `@ID` carries an age (`3;06.` = 42 months), so the
/// `@ID`-age fallback path has something to find when the session-context
/// file has no record for the session.
const DONOR_WITH_ID_AGE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR0 Participant, PAR1 Participant\n@ID:\teng|frog|PAR0|3;06.||||Participant|||\n@ID:\teng|frog|PAR1|||||Participant|||\n@Media:\tsmoke, audio\n*PAR0:\twhere did the frog go . \u{15}0_2000\u{15}\n*PAR1:\ttell me about the picture . \u{15}2000_2500\u{15}\n*PAR0:\tthe frog fell in the jar . \u{15}2500_4500\u{15}\n@End\n";

/// Session-context JSON for session `NF201-3` exercising all four optional
/// fields with deliberately free-vocabulary labels (the seam is
/// corpus-agnostic by design; the labels are surfaced verbatim to the LLM).
const SESSION_CONTEXT_JSON: &str = r#"{
  "NF201-3": {
    "sample_type": "clinician interview",
    "declared_roles": ["Investigator"],
    "consent_tier": "video+audio",
    "age_months": 52
  }
}"#;

fn mock_server() -> MockServer {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [ { "message": { "role": "assistant", "content": HOLISTIC_JSON } } ]
    });
    server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });
    server
}

#[test]
fn pipeline_holistic_writes_llm_pending_and_no_merge() {
    let server = mock_server();
    let tmp = tempfile::tempdir().expect("tempdir");
    let donor = tmp.path().join("NF201-3.cha");
    let reference = tmp.path().join("ref.cha");
    let out = tmp.path().join("merged.cha");
    let pending = tmp.path().join("pending.toml");
    fs::write(&donor, DONOR).expect("write donor");
    fs::write(&reference, REFERENCE).expect("write ref");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args([
            "pipeline",
            donor.to_str().unwrap(),
            reference.to_str().unwrap(),
            "--anchor",
            "CHI",
            "--inserted-role",
            "INV:Investigator",
            "--retain",
            "CHI",
            "--judgment",
            "holistic",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--write-pending",
            pending.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run chatter");

    assert!(
        output.status.success(),
        "chatter exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pending_text = fs::read_to_string(&pending).expect("pending written");
    assert!(
        pending_text.contains(r#"engine = "llm""#),
        "pending entry must be engine=llm; got:\n{pending_text}"
    );
    assert!(
        !out.exists(),
        "holistic is pending-only: no merged file may be written"
    );
}

#[test]
fn batch_holistic_writes_llm_pending_for_every_session_no_merge() {
    let server = mock_server();
    let tmp = tempfile::tempdir().expect("tempdir");
    let donor_dir = tmp.path().join("donor");
    let ref_dir = tmp.path().join("ref");
    let out_dir = tmp.path().join("out");
    let pending = tmp.path().join("pending.toml");
    for d in [&donor_dir, &ref_dir, &out_dir] {
        fs::create_dir_all(d).expect("mkdir");
    }
    for s in ["NF201-3", "NF203-2"] {
        fs::write(donor_dir.join(format!("{s}.cha")), DONOR).expect("donor");
        fs::write(ref_dir.join(format!("{s}.cha")), REFERENCE).expect("ref");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args([
            "batch",
            donor_dir.to_str().unwrap(),
            ref_dir.to_str().unwrap(),
            "--anchor",
            "CHI",
            "--inserted-role",
            "INV:Investigator",
            "--retain",
            "CHI",
            "--judgment",
            "holistic",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--write-pending",
            pending.to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run chatter");

    assert!(
        output.status.success(),
        "batch exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pending_text = fs::read_to_string(&pending).expect("pending written");
    let llm_entries = pending_text.matches(r#"engine = "llm""#).count();
    assert_eq!(
        llm_entries, 2,
        "one llm pending entry per session; got:\n{pending_text}"
    );
    let merged: Vec<_> = fs::read_dir(&out_dir).unwrap().flatten().collect();
    assert!(
        merged.is_empty(),
        "holistic is pending-only: no merged files"
    );

    // The summary must tell the same story as the artifacts: nothing was
    // merged; two suggestions await adjudication. (Field regression
    // 2026-07-07: the summary claimed "2 merged, 0 pending adjudication"
    // because exit code 0 was conflated with a merged file existing.)
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let summary_line = combined
        .lines()
        .find(|l| l.contains("batch summary:"))
        .unwrap_or_else(|| panic!("no batch summary line in output:\n{combined}"));
    assert!(
        summary_line.contains("0 merged"),
        "summary must report zero merged files, got: {summary_line}"
    );
    assert!(
        summary_line.contains("2 suggestions awaiting adjudication"),
        "summary must count the written suggestions, got: {summary_line}"
    );
}

/// A mock that only matches when the judgment request body carries every
/// expected substring. If the CLI fails to thread the session-context labels
/// into the prompt, no mock matches, the provider sees an error response,
/// and the assertions on exit status / hit count fail.
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

/// `--session-context FILE`: the record's free-vocabulary labels must reach
/// the judgment request verbatim (sample type, declared roles, consent tier,
/// age in months).
#[test]
fn pipeline_holistic_session_context_labels_reach_judgment_request() {
    let server = mock_server_expecting_body(&[
        "sample_type: clinician interview",
        "declared_adult_roles: Investigator",
        "consent_tier: video+audio",
        "child_age_months: 52",
    ]);
    let tmp = tempfile::tempdir().expect("tempdir");
    let donor = tmp.path().join("NF201-3.cha");
    let reference = tmp.path().join("ref.cha");
    let out = tmp.path().join("merged.cha");
    let pending = tmp.path().join("pending.toml");
    let context = tmp.path().join("session-context.json");
    fs::write(&donor, DONOR).expect("write donor");
    fs::write(&reference, REFERENCE).expect("write ref");
    fs::write(&context, SESSION_CONTEXT_JSON).expect("write context");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args([
            "pipeline",
            donor.to_str().unwrap(),
            reference.to_str().unwrap(),
            "--anchor",
            "CHI",
            "--inserted-role",
            "INV:Investigator",
            "--retain",
            "CHI",
            "--judgment",
            "holistic",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--session-context",
            context.to_str().unwrap(),
            "--write-pending",
            pending.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run chatter");

    assert!(
        output.status.success(),
        "chatter exited non-zero (labels likely missing from the judgment \
         request): stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pending_text = fs::read_to_string(&pending).expect("pending written");
    assert!(
        pending_text.contains(r#"engine = "llm""#),
        "pending entry must be engine=llm; got:\n{pending_text}"
    );
}

/// `CHATTER_SESSION_CONTEXT` env fallback: with no `--session-context` flag
/// the same labels must still reach the judgment request.
#[test]
fn pipeline_holistic_env_fallback_supplies_session_context() {
    let server = mock_server_expecting_body(&[
        "sample_type: clinician interview",
        "consent_tier: video+audio",
    ]);
    let tmp = tempfile::tempdir().expect("tempdir");
    let donor = tmp.path().join("NF201-3.cha");
    let reference = tmp.path().join("ref.cha");
    let out = tmp.path().join("merged.cha");
    let pending = tmp.path().join("pending.toml");
    let context = tmp.path().join("session-context.json");
    fs::write(&donor, DONOR).expect("write donor");
    fs::write(&reference, REFERENCE).expect("write ref");
    fs::write(&context, SESSION_CONTEXT_JSON).expect("write context");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .env("CHATTER_SESSION_CONTEXT", &context)
        .args([
            "pipeline",
            donor.to_str().unwrap(),
            reference.to_str().unwrap(),
            "--anchor",
            "CHI",
            "--inserted-role",
            "INV:Investigator",
            "--retain",
            "CHI",
            "--judgment",
            "holistic",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--write-pending",
            pending.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run chatter");

    assert!(
        output.status.success(),
        "chatter exited non-zero (env-fallback context likely not loaded): \
         stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// File present but session absent: the `@ID` age fallback (pure CHAT) must
/// still apply, and the file-only fields must render as unknown.
#[test]
fn pipeline_holistic_session_absent_uses_id_age_fallback() {
    let server = mock_server_expecting_body(&[
        "sample_type: unknown",
        "consent_tier: unknown",
        "child_age_months: 42",
    ]);
    let tmp = tempfile::tempdir().expect("tempdir");
    // Session ID is the donor basename stem: OTHER-9 has no record in the
    // context file (which only names NF201-3).
    let donor = tmp.path().join("OTHER-9.cha");
    let reference = tmp.path().join("ref.cha");
    let out = tmp.path().join("merged.cha");
    let pending = tmp.path().join("pending.toml");
    let context = tmp.path().join("session-context.json");
    fs::write(&donor, DONOR_WITH_ID_AGE).expect("write donor");
    fs::write(&reference, REFERENCE).expect("write ref");
    fs::write(&context, SESSION_CONTEXT_JSON).expect("write context");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args([
            "pipeline",
            donor.to_str().unwrap(),
            reference.to_str().unwrap(),
            "--anchor",
            "CHI",
            "--inserted-role",
            "INV:Investigator",
            "--retain",
            "CHI",
            "--judgment",
            "holistic",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--session-context",
            context.to_str().unwrap(),
            "--write-pending",
            pending.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run chatter");

    assert!(
        output.status.success(),
        "chatter exited non-zero (@ID-age fallback likely broken): stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `chatter batch` must thread `--session-context` through to every
/// per-session `chatter pipeline` subprocess: both sessions' judgment
/// requests must carry the context labels.
#[test]
fn batch_holistic_threads_session_context_to_subprocesses() {
    let server = mock_server_expecting_body(&["sample_type: clinician interview"]);
    let tmp = tempfile::tempdir().expect("tempdir");
    let donor_dir = tmp.path().join("donor");
    let ref_dir = tmp.path().join("ref");
    let out_dir = tmp.path().join("out");
    let pending = tmp.path().join("pending.toml");
    let context = tmp.path().join("session-context.json");
    for d in [&donor_dir, &ref_dir, &out_dir] {
        fs::create_dir_all(d).expect("mkdir");
    }
    for s in ["NF201-3", "NF203-2"] {
        fs::write(donor_dir.join(format!("{s}.cha")), DONOR).expect("donor");
        fs::write(ref_dir.join(format!("{s}.cha")), REFERENCE).expect("ref");
    }
    // Both sessions carry the same free-vocabulary sample-type label.
    fs::write(
        &context,
        r#"{
  "NF201-3": { "sample_type": "clinician interview" },
  "NF203-2": { "sample_type": "clinician interview" }
}"#,
    )
    .expect("write context");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args([
            "batch",
            donor_dir.to_str().unwrap(),
            ref_dir.to_str().unwrap(),
            "--anchor",
            "CHI",
            "--inserted-role",
            "INV:Investigator",
            "--retain",
            "CHI",
            "--judgment",
            "holistic",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--session-context",
            context.to_str().unwrap(),
            "--write-pending",
            pending.to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run chatter");

    assert!(
        output.status.success(),
        "batch exited non-zero (context labels likely not threaded to \
         subprocesses): stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pending_text = fs::read_to_string(&pending).expect("pending written");
    let llm_entries = pending_text.matches(r#"engine = "llm""#).count();
    assert_eq!(
        llm_entries, 2,
        "one llm pending entry per session; got:\n{pending_text}"
    );
}

/// A malformed session-context file configured via the env fallback must be
/// reported against `CHATTER_SESSION_CONTEXT`, not against the
/// `--session-context` flag the operator never passed. Misattributing the
/// source sends the operator hunting for a flag that is not in their command.
#[test]
fn pipeline_holistic_malformed_env_session_context_names_env_var() {
    let server = mock_server();
    let tmp = tempfile::tempdir().expect("tempdir");
    let donor = tmp.path().join("NF201-3.cha");
    let reference = tmp.path().join("ref.cha");
    let out = tmp.path().join("merged.cha");
    let pending = tmp.path().join("pending.toml");
    let context = tmp.path().join("session-context.json");
    fs::write(&donor, DONOR).expect("write donor");
    fs::write(&reference, REFERENCE).expect("write ref");
    fs::write(&context, "{ this is not JSON").expect("write context");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .env("CHATTER_SESSION_CONTEXT", &context)
        .args([
            "pipeline",
            donor.to_str().unwrap(),
            reference.to_str().unwrap(),
            "--anchor",
            "CHI",
            "--inserted-role",
            "INV:Investigator",
            "--retain",
            "CHI",
            "--judgment",
            "holistic",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--write-pending",
            pending.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run chatter");

    assert!(
        !output.status.success(),
        "chatter must exit nonzero on a malformed env-configured session-context file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CHATTER_SESSION_CONTEXT"),
        "stderr must name the env-var source the path actually came from; got:\n{stderr}"
    );
    assert!(
        !stderr.contains("--session-context"),
        "stderr must NOT blame the --session-context flag (it was not passed); got:\n{stderr}"
    );
}

/// A malformed session-context file is a hard, typed error: the CLI must
/// exit nonzero with a message naming the flag, never silently fall back to
/// all-unknown context.
#[test]
fn pipeline_holistic_malformed_session_context_exits_nonzero() {
    let server = mock_server();
    let tmp = tempfile::tempdir().expect("tempdir");
    let donor = tmp.path().join("NF201-3.cha");
    let reference = tmp.path().join("ref.cha");
    let out = tmp.path().join("merged.cha");
    let pending = tmp.path().join("pending.toml");
    let context = tmp.path().join("session-context.json");
    fs::write(&donor, DONOR).expect("write donor");
    fs::write(&reference, REFERENCE).expect("write ref");
    fs::write(&context, "{ this is not JSON").expect("write context");

    let output = Command::new(env!("CARGO_BIN_EXE_chatter"))
        .args([
            "pipeline",
            donor.to_str().unwrap(),
            reference.to_str().unwrap(),
            "--anchor",
            "CHI",
            "--inserted-role",
            "INV:Investigator",
            "--retain",
            "CHI",
            "--judgment",
            "holistic",
            "--llm-endpoint",
            &format!("{}/v1", server.base_url()),
            "--llm-model",
            "deepseek-v4-flash",
            "--session-context",
            context.to_str().unwrap(),
            "--write-pending",
            pending.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run chatter");

    assert!(
        !output.status.success(),
        "chatter must exit nonzero on a malformed session-context file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("session-context"),
        "stderr must name the session-context input; got:\n{stderr}"
    );
    assert!(
        !pending.exists(),
        "no pending entry may be written on a malformed session-context file"
    );
}
