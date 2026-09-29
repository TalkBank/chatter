//! Public timing-edit transitions over admitted canonical merge inputs.

use super::*;
use talkbank_model::model::Bullet;
use talkbank_transform::transcript_merge::{
    BulletEditError, MergeDraft, merge_chat_files_with_donor_selection_draft,
};

fn reference(path: &str) -> ChatFile {
    let path = talkbank_parser_tests::repo_paths::workspace_root().join(path);
    let source = std::fs::read_to_string(&path).expect("canonical reference");
    let parser = TreeSitterParser::new().expect("parser");
    strict_parse(parser.parse_chat_file(&source))
        .expect("reference parses")
        .validate_with_policy(
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            &NullErrorSink,
            TranscriptName::for_path(&path),
        )
        .expect("reference validates")
        .into_unchecked()
}

fn draft(source: &ChatFile) -> MergeDraft {
    let parents = source
        .utterances()
        .enumerate()
        .map(|(i, _)| DonorIdx::new(UtteranceIdx::new(i)))
        .collect();
    let selection = SourceBoundDonorSelection::bind(source, source, parents)
        .expect("identity source selection");
    merge_chat_files_with_donor_selection_draft(
        source,
        &selection,
        &source.unique_utterance_speakers(),
        &[],
    )
    .expect("retained speech draft")
}

#[test]
fn reference_merge_timing_edits_preserve_original_and_latest_evidence() {
    let source = reference("corpus/reference/content/media-bullets.cha");
    let before = source.to_chat_string();
    let originals: Vec<_> = source
        .utterances()
        .map(|u| u.main.content.bullet.clone().expect("timed reference turn"))
        .collect();
    assert_eq!(originals.len(), 2);
    let mut edited = draft(&source);
    let origins = edited.origins().to_vec();
    // Apply in reverse order: reporting is by output ordinal, not edit order.
    for i in [1, 0] {
        let timing = originals[i].timing;
        edited
            .set_terminal_bullet(i, Bullet::new(timing.start_ms, timing.end_ms + 10))
            .expect("replace existing timing");
    }
    let latest = Bullet::new(
        originals[0].timing.start_ms,
        originals[0].timing.end_ms + 20,
    );
    edited
        .set_terminal_bullet(0, latest.clone())
        .expect("repeat edit");
    let records: Vec<_> = edited.bullet_edits().collect();
    assert_eq!(records.len(), 2);
    for (i, record) in records.iter().enumerate() {
        assert_eq!(record.output(), i);
        assert_eq!(record.assembled(), originals[i].timing);
    }
    assert_eq!(records[0].replacement(), latest.timing);
    assert_eq!(
        records[1].replacement().end_ms,
        originals[1].timing.end_ms + 10
    );
    assert_eq!(edited.origins(), origins);

    // Refusal is transactional even after successful edits.
    let edited_before = edited.file().to_chat_string();
    assert_eq!(
        edited.set_terminal_bullet(usize::MAX, latest),
        Err(BulletEditError::NoSuchUtterance { output: usize::MAX })
    );
    assert_eq!(edited.file().to_chat_string(), edited_before);
    assert_eq!(edited.bullet_edits().count(), 2);

    edited
        .set_terminal_bullet(0, originals[0].clone())
        .expect("restore original");
    assert_eq!(
        edited
            .bullet_edits()
            .map(|edit| edit.output())
            .collect::<Vec<_>>(),
        vec![1]
    );
    let accepted = edited.validate().expect("edited draft still validates");
    assert_eq!(accepted.bullet_edits().len(), 1);
    assert_eq!(accepted.bullet_edits()[0].assembled(), originals[1].timing);
    assert_eq!(
        accepted.bullet_edits()[0].replacement().end_ms,
        originals[1].timing.end_ms + 10
    );
    let reported = accepted.report(|_, _| panic!("retain-all drops no speech"));
    assert_eq!(reported.file().utterances().count(), originals.len());

    let mut restored = draft(&source);
    // Merge assembly preserves opening comments from both sources. Editing
    // must restore that assembled draft, not pretend merging is idempotent.
    let assembled_before = restored.file().to_chat_string();
    restored
        .set_terminal_bullet(
            0,
            Bullet::new(
                originals[0].timing.start_ms,
                originals[0].timing.end_ms + 10,
            ),
        )
        .expect("edit");
    restored
        .set_terminal_bullet(0, originals[0].clone())
        .expect("restore");
    assert_eq!(restored.bullet_edits().count(), 0);
    assert_eq!(restored.file().to_chat_string(), assembled_before);
    assert_eq!(source.to_chat_string(), before, "source is never rewritten");
}

#[test]
fn reference_merge_timing_refusal_preserves_untimed_speech() {
    let source = reference("corpus/reference/core/basic-conversation.cha");
    let timed = reference("corpus/reference/content/media-bullets.cha");
    let replacement = timed
        .utterances()
        .next()
        .expect("timed turn")
        .main
        .content
        .bullet
        .clone()
        .expect("recorded bullet");
    let mut edited = draft(&source);
    let assembled = edited.file().clone();
    let before = edited.file().to_chat_string();
    let origins = edited.origins().to_vec();
    assert_eq!(
        edited.set_terminal_bullet(0, replacement),
        Err(BulletEditError::NoTerminalBullet { output: 0 })
    );
    assert_eq!(edited.file().to_chat_string(), before);
    assert_eq!(edited.origins(), origins);
    assert_eq!(edited.bullet_edits().count(), 0);
    let accepted = edited.validate().expect("refusal preserves validity");
    let reported = accepted.report(|_, _| panic!("no speech dropped"));
    assert!(reported.file().semantic_eq(&assembled));
}
