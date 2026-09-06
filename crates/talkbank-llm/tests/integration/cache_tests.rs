//! `ResponseCache`: persistence across reopen, corrupt-file refusal, and the
//! provider-level guarantee that a cache hit performs no HTTP request.
//!
//! Per the standing rule, these tests are the FIRST thing written for this
//! feature (RED before any implementation).
//!
//! The crate denies `unwrap_used` / `expect_used` / `panic` for long-lived
//! code, but the standing rules allow panicking assertion idioms in test
//! code, so those lints are relaxed for this test target only.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use httpmock::{Method, MockServer};

use talkbank_llm::{CachePath, HttpJudgmentProvider, HttpProviderConfig, ResponseCache};
use talkbank_model::SpeakerCode;
use talkbank_transform::speaker_id::{
    EndpointUrl, JudgmentProvider, JudgmentRequest, ModelId, SessionId,
};

/// The canned holistic JSON the stub returns as the assistant message
/// content. Copied from `http_provider_tests.rs`: test crates do not share
/// code with each other.
const HOLISTIC_JSON: &str = r#"{
  "speaker_mapping": { "PAR0": "CHI", "PAR1": "adult" },
  "adult_roles": { "PAR1": "INV" },
  "sample_type": "confirmed",
  "merge_applicable": true,
  "confidence": { "mapping": 0.9, "roles": 0.8, "merge_applicable": 0.95 },
  "reasoning": "PAR1 prompts; PAR0 answers."
}"#;

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

/// A cache written by one `ResponseCache` handle is readable by a fresh
/// handle opened later against the same path: the write-through-on-put
/// contract, not an in-memory-only cache.
#[test]
fn cache_roundtrips_across_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("llm-cache.json");
    let cache = ResponseCache::open(CachePath(path.clone())).expect("open");
    cache.put("k1", "v1".to_string()).expect("put");
    drop(cache);
    let reopened = ResponseCache::open(CachePath(path)).expect("reopen");
    assert_eq!(reopened.get("k1").as_deref(), Some("v1"));
}

/// A missing cache file is an empty cache, not an error: the common case of
/// a first run with no prior cache must not require the operator to
/// pre-create anything.
#[test]
fn missing_cache_file_is_an_empty_cache() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("does-not-exist.json");
    let cache = ResponseCache::open(CachePath(path)).expect("open on missing file");
    assert_eq!(cache.get("anything"), None);
}

/// A cache file that exists but is not valid JSON is a hard error: silently
/// treating it as empty would re-pay every LLM call without telling the
/// operator why, and silently ignoring it would risk masking a real bug.
#[test]
fn corrupt_cache_file_is_a_hard_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("llm-cache.json");
    std::fs::write(&path, "not json {{{").expect("write junk");
    assert!(ResponseCache::open(CachePath(path)).is_err());
}

/// A second `judge` call for the exact same request is served from the
/// cache: the mock server sees exactly one hit even though `judge` is
/// called twice.
#[test]
fn second_judge_call_is_served_from_cache() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST).path("/v1/chat/completions");
        then.status(200).json_body(serde_json::json!({
            "choices": [{"message": {"role": "assistant",
                                     "content": HOLISTIC_JSON}}]
        }));
    });
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = ResponseCache::open(CachePath(dir.path().join("c.json"))).expect("open");
    let config = HttpProviderConfig::new(
        EndpointUrl(format!("{}/v1", server.base_url())),
        ModelId("test-model".into()),
    );
    let provider = HttpJudgmentProvider::with_cache(config, cache);
    let req = minimal_request();
    let first = provider.judge(&req).expect("first judge");
    let second = provider.judge(&req).expect("second judge");
    assert_eq!(first.merge_applicable, second.merge_applicable);
    mock.assert_calls(1); // the second call never reached the server
}

/// A failed disk write must not turn a later lookup into an uncommitted hit.
#[test]
fn failed_put_preserves_memory_and_existing_disk_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cache.json");
    let retained = dir.path().join("retained.json");
    let cache = ResponseCache::open(CachePath(path.clone())).expect("open");
    cache
        .put("old", "retained response".into())
        .expect("initial put");
    let before = std::fs::read(&path).expect("read old cache");
    // The JSON file is closed between puts. Move only that file, leaving the
    // open lease and staged-file directory in place on every platform. A
    // directory at the destination forces the real publication rename to fail.
    std::fs::rename(&path, &retained).expect("retain old cache");
    std::fs::create_dir(&path).expect("obstruct replacement destination");

    assert!(cache.put("new", "uncommitted response".into()).is_err());
    assert_eq!(
        cache.get("new"),
        None,
        "failed writes cannot publish memory hits"
    );
    assert_eq!(cache.get("old").as_deref(), Some("retained response"));
    assert_eq!(std::fs::read(&retained).unwrap(), before);
    assert!(path.is_dir(), "publication must preserve the obstruction");
    std::fs::remove_dir(&path).expect("remove obstruction");
    std::fs::rename(&retained, &path).expect("restore retained cache");
    cache
        .put("new", "committed response".into())
        .expect("retry");
    drop(cache);
    let reopened = ResponseCache::open(CachePath(path)).expect("reopen");
    assert_eq!(reopened.get("old").as_deref(), Some("retained response"));
    assert_eq!(reopened.get("new").as_deref(), Some("committed response"));
}

/// A second handle must not load a stale snapshot and later overwrite the owner.
#[test]
fn a_second_cache_owner_is_rejected_until_the_first_drops() {
    const PROBE: &str = "TALKBANK_LLM_TEST_LOCK_PROBE";
    if let Some(path) = std::env::var_os(PROBE) {
        assert!(matches!(
            ResponseCache::open(CachePath(path.into())),
            Err(talkbank_llm::CacheError::InUse { .. })
        ));
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cache.json");
    let first = ResponseCache::open(CachePath(path.clone())).expect("first owner");
    assert!(ResponseCache::open(CachePath(path.clone())).is_err());
    let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "cache_tests::a_second_cache_owner_is_rejected_until_the_first_drops",
        ])
        .env(PROBE, &path)
        .status()
        .expect("run independent opener");
    assert!(status.success(), "another process must be rejected too");
    drop(first);
    assert!(ResponseCache::open(CachePath(path)).is_ok());
}

/// Concurrent puts must retain every completed update on disk, in key order.
#[test]
fn simultaneous_writers_publish_every_entry() {
    use std::collections::BTreeMap;
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cache.json");
    let cache = ResponseCache::open(CachePath(path.clone())).expect("open");
    let start = std::sync::Barrier::new(16);
    std::thread::scope(|scope| {
        for index in 0..16 {
            let cache = &cache;
            let start = &start;
            scope.spawn(move || {
                start.wait();
                cache
                    .put(&format!("key-{index:02}"), format!("response-{index}"))
                    .expect("put");
            });
        }
    });
    let expected: BTreeMap<_, _> = (0..16)
        .map(|index| (format!("key-{index:02}"), format!("response-{index}")))
        .collect();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        serde_json::to_string(&expected).unwrap()
    );
    drop(cache);
    let reopened = ResponseCache::open(CachePath(path)).expect("reopen");
    for (key, value) in expected {
        assert_eq!(reopened.get(&key), Some(value));
    }
}
