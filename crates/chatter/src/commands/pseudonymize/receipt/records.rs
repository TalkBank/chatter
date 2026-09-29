//! SQL projection of the actual decisions and applied edits, not a second matcher.

use super::ReceiptError;
use sqlx::{Sqlite, Transaction};
use std::ops::Range;
use talkbank_transform::pseudonymize::*;

// Nullable columns are the external SQL wire shape. Only these projections
// construct it; schema checks reject impossible mixtures when queried externally.
struct Location {
    scope: &'static str,
    utterance: Option<usize>,
    word: Option<usize>,
    target: Option<usize>,
    header: Option<usize>,
    tier: Option<usize>,
    item: Option<usize>,
    clitic: Option<usize>,
}

impl Location {
    fn word(scope: &'static str, location: WordLocation) -> Self {
        Self {
            scope,
            utterance: Some(location.utterance()),
            word: Some(location.word()),
            target: match location.spelling() {
                WordSpelling::Spoken => None,
                WordSpelling::ReplacementTarget(index) => Some(index),
            },
            header: None,
            tier: None,
            item: None,
            clitic: None,
        }
    }
    fn header(location: HeaderFieldLocation) -> Self {
        Self::prose_header(
            match location.field() {
                HeaderNameField::Participant => "participant",
                HeaderNameField::Group => "group",
                HeaderNameField::Education => "education",
                HeaderNameField::Custom => "custom",
            },
            location.header(),
        )
    }
    fn prose_header(scope: &'static str, header: usize) -> Self {
        Self {
            scope,
            header: Some(header),
            utterance: None,
            word: None,
            target: None,
            tier: None,
            item: None,
            clitic: None,
        }
    }
    fn prose(location: ProseLocation) -> Self {
        match location {
            ProseLocation::Header(header) => Self::prose_header("header_prose", header),
            ProseLocation::DependentTier { utterance, tier } => Self {
                scope: "tier_prose",
                utterance: Some(utterance),
                tier: Some(tier),
                word: None,
                target: None,
                header: None,
                item: None,
                clitic: None,
            },
        }
    }
    fn lemma(location: LemmaLocation) -> Self {
        let (scope, clitic) = match location.part() {
            LemmaPart::Main => ("lemma_main", None),
            LemmaPart::PostClitic(index) => ("lemma_post_clitic", Some(index)),
        };
        Self {
            scope,
            clitic,
            utterance: Some(location.utterance()),
            item: Some(location.item()),
            word: None,
            target: None,
            header: None,
            tier: None,
        }
    }
    fn edit(origin: EditOrigin) -> Self {
        match origin {
            EditOrigin::MainWord(location) => Self::word("main", location),
            EditOrigin::Morphology(location) => Self::word("mor", location),
            EditOrigin::Timing(location) => Self::word("wor", location),
            EditOrigin::Header(location) => Self::header(location),
            EditOrigin::Prose(location) => Self::prose(location),
        }
    }
}

enum Decision<'a> {
    Proposed(&'a str),
    Applied {
        replacement: &'a str,
        output: Range<usize>,
    },
    CaseNearMiss,
    Possessive,
    UnreplacedLemma,
}

fn integer(value: Option<usize>) -> Result<Option<i64>, ReceiptError> {
    value
        .map(i64::try_from)
        .transpose()
        .map_err(|_| ReceiptError::Storage)
}

async fn insert(
    transaction: &mut Transaction<'_, Sqlite>,
    location: Location,
    source: Option<Range<usize>>,
    original: &str,
    decision: Decision<'_>,
) -> Result<(), ReceiptError> {
    let (decision, replacement, output) = match decision {
        Decision::Proposed(replacement) => ("proposed", Some(replacement), None),
        Decision::Applied {
            replacement,
            output,
        } => ("replace", Some(replacement), Some(output)),
        Decision::CaseNearMiss => ("case_near_miss", None, None),
        Decision::Possessive => ("possessive_review", None, None),
        Decision::UnreplacedLemma => ("unreplaced_lemma", None, None),
    };
    sqlx::query("INSERT INTO event (document_id, decision, scope, utterance, word, target, header, tier, item, clitic, source_start, source_end, output_start, output_end, original, replacement) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(decision).bind(location.scope)
        .bind(integer(location.utterance)?).bind(integer(location.word)?).bind(integer(location.target)?)
        .bind(integer(location.header)?).bind(integer(location.tier)?).bind(integer(location.item)?).bind(integer(location.clitic)?)
        .bind(integer(source.as_ref().map(|range| range.start))?).bind(integer(source.map(|range| range.end))?)
        .bind(integer(output.as_ref().map(|range| range.start))?).bind(integer(output.map(|range| range.end))?)
        .bind(original).bind(replacement).execute(&mut **transaction).await.map_err(|_| ReceiptError::Storage)?;
    Ok(())
}

pub(super) async fn write(
    transaction: &mut Transaction<'_, Sqlite>,
    output: &PseudonymizedDocument<'_>,
) -> Result<(), ReceiptError> {
    for edit in output.edits() {
        insert(
            transaction,
            Location::edit(edit.origin()),
            Some(edit.original_range()),
            edit.original(),
            Decision::Applied {
                replacement: edit.replacement(),
                output: edit.output_range(),
            },
        )
        .await?;
    }
    write_review(transaction, output.plan()).await
}

pub(super) async fn write_review(
    transaction: &mut Transaction<'_, Sqlite>,
    plan: &LexicalPlan<'_>,
) -> Result<(), ReceiptError> {
    for word in plan.previews() {
        for decision in word.decisions() {
            match decision {
                ComponentDecision::CaseNearMiss { original } => {
                    // Word context is exact, but a reconstructed component need
                    // not occupy one contiguous source slice. Do not invent one.
                    insert(
                        transaction,
                        Location::word("main", word.location()),
                        None,
                        original,
                        Decision::CaseNearMiss,
                    )
                    .await?;
                }
                ComponentDecision::Keep | ComponentDecision::Replace { .. } => {}
            }
        }
    }
    if let Ok(fields) = plan.header_fields() {
        for field in fields {
            let decision = match field.decision() {
                FreeTextDecision::Replace(_) => continue,
                FreeTextDecision::CaseNearMiss => Decision::CaseNearMiss,
                FreeTextDecision::PossessiveReview => Decision::Possessive,
            };
            insert(
                transaction,
                Location::header(field.location()),
                Some(field.range()),
                field.original(),
                decision,
            )
            .await?;
        }
    }
    if let Ok(fields) = plan.free_text() {
        for field in fields {
            let decision = match field.decision() {
                FreeTextDecision::Replace(_) => continue,
                FreeTextDecision::CaseNearMiss => Decision::CaseNearMiss,
                FreeTextDecision::PossessiveReview => Decision::Possessive,
            };
            insert(
                transaction,
                Location::prose(field.location()),
                Some(field.range()),
                field.original(),
                decision,
            )
            .await?;
        }
    }
    for finding in plan.lemma_findings() {
        let decision = match finding.kind() {
            LemmaFindingKind::CaseNearMiss => Decision::CaseNearMiss,
            LemmaFindingKind::UnreplacedMatch => Decision::UnreplacedLemma,
        };
        insert(
            transaction,
            Location::lemma(finding.location()),
            None,
            finding.word().lemma.as_ref(),
            decision,
        )
        .await?;
    }
    // Binding failures are retained as refusals, never silently turned into
    // empty findings. This is also the complete safety-refusal inventory.
    super::refusal::write_findings(transaction, plan).await
}

pub(super) async fn write_proposals(
    transaction: &mut Transaction<'_, Sqlite>,
    plan: &LexicalPlan<'_>,
) -> Result<(), ReceiptError> {
    for word in plan.previews() {
        for edit in word.edits() {
            insert(
                transaction,
                Location::word("main", word.location()),
                Some(edit.range()),
                edit.original(),
                Decision::Proposed(edit.replacement()),
            )
            .await?;
        }
    }
    for review in plan.morphology() {
        if let MorphologyOutcome::Proposed {
            source, proposed, ..
        } = review.outcome()
        {
            insert(
                transaction,
                Location::word("mor", review.location()),
                Some(source.range()),
                source.original(),
                Decision::Proposed(proposed.main.lemma.as_ref()),
            )
            .await?;
        }
    }
    for review in plan.timing() {
        if let TimingOutcome::Corroborated(words) = review.outcome() {
            for word in words {
                for edit in word.edits() {
                    insert(
                        transaction,
                        Location::word("wor", word.location()),
                        Some(edit.range()),
                        edit.original(),
                        Decision::Proposed(edit.replacement()),
                    )
                    .await?;
                }
            }
        }
    }
    if let Ok(fields) = plan.header_fields() {
        for field in fields {
            if let FreeTextDecision::Replace(text) = field.decision() {
                insert(
                    transaction,
                    Location::header(field.location()),
                    Some(field.range()),
                    field.original(),
                    Decision::Proposed(text.as_ref()),
                )
                .await?;
            }
        }
    }
    if let Ok(fields) = plan.free_text() {
        for field in fields {
            if let FreeTextDecision::Replace(text) = field.decision() {
                insert(
                    transaction,
                    Location::prose(field.location()),
                    Some(field.range()),
                    field.original(),
                    Decision::Proposed(text.as_ref()),
                )
                .await?;
            }
        }
    }
    write_review(transaction, plan).await
}
