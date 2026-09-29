//! Real SQLite/filesystem boundary tests using the canonical authored golden.

use super::refusal::{RejectedDocument, record_rejection};
use super::*;
use sqlx::{SqliteConnection, sqlite::SqliteConnectOptions};
use talkbank_model::{RuleSelection, model::TranscriptName};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::pseudonymize::NameMap;
use tempfile::{NamedTempFile, TempPath};

const SOURCE: &str =
    include_str!("../../../../../../corpus/reference/word-features/pseudonymizer-source.cha");
const EXPECTED: &str =
    include_str!("../../../../../../corpus/reference/word-features/pseudonymizer-expected.cha");

fn vacant() -> TempPath {
    let path = tempfile::Builder::new()
        .prefix("pseudonymizer-δοκιμή-")
        .tempfile()
        .unwrap()
        .into_temp_path();
    std::fs::remove_file(&path).unwrap();
    path
}

async fn read(path: &Path) -> SqliteConnection {
    SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path).read_only(true))
        .await
        .unwrap()
}

fn names(parser: &TreeSitterParser) -> NameMap {
    NameMap::from_toml("version = 1\n[[transcripts]]\nkey = 'sample'\n[[transcripts.names]]\noriginal = 'Rose'\nreplacement = 'PersonA'\n", parser).unwrap()
}

#[tokio::test]
async fn refused_canonical_plans_record_proposals_but_never_applied_edits() {
    let parser = TreeSitterParser::new().unwrap();
    for (source, name, domain) in [
        (
            include_str!("../../../../../../corpus/reference/tiers/pho.cha"),
            "Mommy",
            "pronunciation",
        ),
        (
            include_str!("../../../../../../corpus/reference/tiers/mor-selective-names.cha"),
            "ice",
            "mor",
        ),
        (
            include_str!("../../../../../../corpus/reference/tiers/wor-drift.cha"),
            "hello",
            "wor",
        ),
        (
            include_str!("../../../../../../corpus/reference/annotation/retrace.cha"),
            "tika",
            "word",
        ),
    ] {
        let map = NameMap::from_toml(&format!("version = 1\n[[transcripts]]\nkey = 'sample'\n[[transcripts.names]]\noriginal = '{name}'\nreplacement = 'PersonA'\n"), &parser).unwrap();
        let input = map
            .for_transcript("sample")
            .unwrap()
            .admit_document(
                source,
                TranscriptName::Anonymous,
                RuleSelection::new(),
                &parser,
            )
            .unwrap();
        let rejected = input.prepare_output(&parser).unwrap_err();
        let receipt = vacant();
        record_rejection(&receipt, "sample", RejectedDocument::Plan(&rejected))
            .await
            .unwrap();
        let mut database = read(&receipt).await;
        let state: (String, Option<String>, Option<String>, i64) = sqlx::query_as(
            "SELECT status, output_path, output_blake3, applied_field_count FROM document_summary",
        )
        .fetch_one(&mut database)
        .await
        .unwrap();
        assert_eq!(state, ("output_refused".to_owned(), None, None, 0));
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM refusal WHERE domain = ?")
            .bind(domain)
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert!(count > 0, "missing refusal domain {domain}");
        if domain != "word" {
            let count: i64 =
                sqlx::query_scalar("SELECT proposed_field_count FROM document_summary")
                    .fetch_one(&mut database)
                    .await
                    .unwrap();
            assert!(
                count > 0,
                "safe proposals must remain reviewable, not applied"
            );
        }
        let invalid: i64 =
            sqlx::query_scalar("SELECT count(*) FROM event WHERE output_start IS NOT NULL")
                .fetch_one(&mut database)
                .await
                .unwrap();
        assert_eq!(invalid, 0);
        database.close().await.unwrap();
    }
}

#[tokio::test]
async fn unmapped_invalid_and_zero_findings_are_distinct_receipts() {
    const CLEAN: &str =
        include_str!("../../../../../../corpus/reference/languages/eng-conversation.cha");
    const INVALID: &str = include_str!(
        "../../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E706_1.cha"
    );
    let parser = TreeSitterParser::new().unwrap();
    let map = names(&parser);
    assert!(map.for_transcript("missing").is_none());
    let missing = vacant();
    record_rejection(&missing, "missing", RejectedDocument::Unmapped(CLEAN))
        .await
        .unwrap();
    let invalid = vacant();
    let reason = match map.for_transcript("sample").unwrap().admit_document(
        INVALID,
        TranscriptName::Anonymous,
        RuleSelection::new(),
        &parser,
    ) {
        Err(reason) => reason,
        Ok(_) => panic!("alignment-invalid reference was admitted"),
    };
    record_rejection(
        &invalid,
        "sample",
        RejectedDocument::Admission {
            source: INVALID,
            reason,
        },
    )
    .await
    .unwrap();
    for (path, expected) in [(&missing, "unmapped"), (&invalid, "input_refused")] {
        let mut database = read(path).await;
        let state: (String, Option<String>, i64) = sqlx::query_as(
            "SELECT status, output_blake3, applied_field_count FROM document_summary",
        )
        .fetch_one(&mut database)
        .await
        .unwrap();
        assert_eq!(state, (expected.to_owned(), None, 0));
        database.close().await.unwrap();
    }
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            CLEAN,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let document = input.prepare_output(&parser).unwrap();
    let output = vacant();
    let receipt = vacant();
    PreparedPublication::prepare(&document, &output, &receipt, "sample")
        .await
        .unwrap()
        .publish()
        .await
        .unwrap();
    let mut database = read(&receipt).await;
    let state: (String, i64, i64) =
        sqlx::query_as("SELECT status, no_findings, applied_field_count FROM document_summary")
            .fetch_one(&mut database)
            .await
            .unwrap();
    assert_eq!(state, ("written".to_owned(), 1, 0));
    assert_eq!(std::fs::read_to_string(&output).unwrap(), CLEAN);
    database.close().await.unwrap();
}

#[tokio::test]
async fn committed_evidence_precedes_publication_and_carries_actual_edits() {
    let parser = TreeSitterParser::new().unwrap();
    let map = names(&parser);
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            SOURCE,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let document = input.prepare_output(&parser).unwrap();
    let output = vacant();
    let receipt = vacant();
    let prepared = PreparedPublication::prepare(&document, &output, &receipt, "sample")
        .await
        .unwrap();
    assert!(!output.exists());
    let mut database = read(&receipt).await;
    let state: String = sqlx::query_scalar("SELECT status FROM document")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(state, "prepared");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event WHERE decision = 'replace'")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 15);
    let possessive: String =
        sqlx::query_scalar("SELECT original FROM event WHERE decision = 'possessive_review'")
            .fetch_one(&mut database)
            .await
            .unwrap();
    assert_eq!(possessive, "rose’s");
    let observed: Vec<(i64, i64, String, String)> = sqlx::query_as("SELECT source_start, source_end, original, replacement FROM event WHERE decision = 'replace' ORDER BY id").fetch_all(&mut database).await.unwrap();
    for (record, edit) in observed.iter().zip(document.edits()) {
        assert_eq!(
            record.0,
            i64::try_from(edit.original_range().start).unwrap()
        );
        assert_eq!(record.1, i64::try_from(edit.original_range().end).unwrap());
        assert_eq!(record.2, edit.original());
        assert_eq!(record.3, edit.replacement());
    }
    database.close().await.unwrap();
    prepared.publish().await.unwrap();
    assert_eq!(std::fs::read_to_string(&output).unwrap(), EXPECTED);
    let mut database = read(&receipt).await;
    let state: String = sqlx::query_scalar("SELECT status FROM document")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(state, "written");
    let digest: String = sqlx::query_scalar("SELECT output_blake3 FROM document")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(digest, blake3::hash(EXPECTED.as_bytes()).to_hex().as_str());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&receipt).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    database.close().await.unwrap();
}

#[tokio::test]
async fn existing_receipt_alias_and_publication_race_do_not_clobber() {
    let parser = TreeSitterParser::new().unwrap();
    let map = names(&parser);
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            SOURCE,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let document = input.prepare_output(&parser).unwrap();
    let output = vacant();
    assert!(matches!(
        PreparedPublication::prepare(&document, &output, &output, "sample").await,
        Err(ReceiptError::Destination)
    ));
    assert!(!output.exists());
    let existing = NamedTempFile::new().unwrap();
    std::fs::write(existing.path(), "protected receipt").unwrap();
    assert!(matches!(
        PreparedPublication::prepare(&document, &output, existing.path(), "sample").await,
        Err(ReceiptError::Destination)
    ));
    assert_eq!(
        std::fs::read_to_string(existing.path()).unwrap(),
        "protected receipt"
    );
    let receipt = vacant();
    let prepared = PreparedPublication::prepare(&document, &output, &receipt, "sample")
        .await
        .unwrap();
    std::fs::write(&output, "protected output").unwrap();
    assert!(matches!(
        prepared.publish().await,
        Err(ReceiptError::Publication)
    ));
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        "protected output"
    );
    let mut database = read(&receipt).await;
    let state: String = sqlx::query_scalar("SELECT status FROM document")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(state, "publication_failed");
    database.close().await.unwrap();
}

#[tokio::test]
async fn abandoning_prepared_output_retains_honest_receipt_without_output() {
    let parser = TreeSitterParser::new().unwrap();
    let map = names(&parser);
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            SOURCE,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let document = input.prepare_output(&parser).unwrap();
    let output = vacant();
    let receipt = vacant();
    drop(
        PreparedPublication::prepare(&document, &output, &receipt, "sample")
            .await
            .unwrap(),
    );
    assert!(!output.exists());
    let mut database = read(&receipt).await;
    let state: String = sqlx::query_scalar("SELECT status FROM document")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(state, "prepared");
    database.close().await.unwrap();
}
