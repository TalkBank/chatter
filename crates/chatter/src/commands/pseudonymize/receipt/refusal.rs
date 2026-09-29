//! A rejection can write evidence, but cannot produce publication authority.

use super::{Database, ReceiptError, records};
use sqlx::{Connection, Sqlite, Transaction};
use std::path::Path;
use talkbank_model::Span;
use talkbank_transform::pseudonymize::*;

/// Source evidence for a terminal non-publication receipt.
pub(in crate::commands::pseudonymize) enum RejectedDocument<'a> {
    Unmapped(&'a str),
    Admission {
        source: &'a str,
        reason: InputRefusal,
    },
    Plan(&'a OutputRefusal<'a>),
}

pub(in crate::commands::pseudonymize) async fn record_rejection(
    receipt: &Path,
    key: &str,
    rejected: RejectedDocument<'_>,
) -> Result<(), ReceiptError> {
    let (source, status, reason) = match &rejected {
        RejectedDocument::Unmapped(source) => (*source, "unmapped", "no_map_entry".to_owned()),
        RejectedDocument::Admission { source, reason } => {
            (*source, "input_refused", format!("{reason:?}"))
        }
        RejectedDocument::Plan(refusal) => (
            refusal.plan().source(),
            "output_refused",
            format!("{:?}", refusal.reason()),
        ),
    };
    let mut database = Database::create(receipt).await?;
    let mut transaction = database
        .connection
        .begin()
        .await
        .map_err(|_| ReceiptError::Storage)?;
    sqlx::query("INSERT INTO document (id, schema_version, map_key, source_blake3, status, reason) VALUES (1, 1, ?, ?, ?, ?)")
        .bind(key).bind(blake3::hash(source.as_bytes()).to_hex().as_str()).bind(status).bind(reason)
        .execute(&mut *transaction).await.map_err(|_| ReceiptError::Storage)?;
    if let RejectedDocument::Plan(refusal) = rejected {
        records::write_proposals(&mut transaction, refusal.plan()).await?;
    }
    transaction
        .commit()
        .await
        .map_err(|_| ReceiptError::Storage)?;
    database.sync()?;
    database.close().await
}

fn integer(value: Option<usize>) -> Result<Option<i64>, ReceiptError> {
    value
        .map(i64::try_from)
        .transpose()
        .map_err(|_| ReceiptError::Storage)
}

async fn insert(
    transaction: &mut Transaction<'_, Sqlite>,
    domain: &str,
    reason: &str,
    location: Option<WordLocation>,
    span: Option<Span>,
    tier: Option<&str>,
) -> Result<(), ReceiptError> {
    let target = location.and_then(|location| match location.spelling() {
        WordSpelling::Spoken => None,
        WordSpelling::ReplacementTarget(index) => Some(index),
    });
    sqlx::query("INSERT INTO refusal (document_id, domain, reason, utterance, word, target, source_start, source_end, tier) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(domain).bind(reason).bind(integer(location.map(WordLocation::utterance))?)
        .bind(integer(location.map(WordLocation::word))?).bind(integer(target)?)
        .bind(span.map(|span| i64::from(span.start))).bind(span.map(|span| i64::from(span.end)))
        .bind(tier).execute(&mut **transaction).await.map_err(|_| ReceiptError::Storage)?;
    Ok(())
}

pub(super) async fn write_findings(
    transaction: &mut Transaction<'_, Sqlite>,
    plan: &LexicalPlan<'_>,
) -> Result<(), ReceiptError> {
    for word in plan.refusals() {
        insert(
            transaction,
            "word",
            &format!("{:?}", word.reason()),
            Some(word.location()),
            Some(word.span()),
            None,
        )
        .await?;
    }
    for review in plan.morphology() {
        if let MorphologyOutcome::Refused { reason, .. } = review.outcome() {
            insert(
                transaction,
                "mor",
                &format!("{reason:?}"),
                Some(review.location()),
                Some(review.word().span),
                Some("mor"),
            )
            .await?;
        }
    }
    for review in plan.timing() {
        if let TimingOutcome::Refused(reason) = review.outcome() {
            insert(
                transaction,
                "wor",
                &format!("{reason:?}"),
                None,
                Some(review.original().span),
                Some("wor"),
            )
            .await?;
        }
    }
    for review in plan.pronunciation() {
        let reason = match review.evidence() {
            PronunciationEvidence::AlignedItem(_) => "aligned_pronunciation",
            PronunciationEvidence::CompanionReview => "companion_review",
            PronunciationEvidence::MissingAlignment => "missing_alignment",
        };
        insert(
            transaction,
            "pronunciation",
            reason,
            Some(review.location()),
            Some(review.tier().span()),
            Some(review.tier().kind()),
        )
        .await?;
    }
    if plan.header_fields().is_err() {
        insert(transaction, "metadata", "source_binding", None, None, None).await?;
    }
    if plan.free_text().is_err() {
        insert(transaction, "prose", "source_binding", None, None, None).await?;
    }
    Ok(())
}
