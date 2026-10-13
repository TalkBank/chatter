//! Lexical identity evidence follows spoken originals, not replacement targets.

use talkbank_model::{ChatFile, Line, ParseValidateOptions, SpeakerCode, WriteChat};
use talkbank_transform::speaker_id::{
    ConfidenceMargin, ConfidenceThreshold, RecordedSpeakerIdentificationAttempt, identify_mapping,
};

/// Retain authored utterances and headers; do not manufacture lexical AST nodes.
fn select_utterance(mut chat: ChatFile, selected: usize) -> ChatFile {
    let mut ordinal = 0;
    chat.lines = chat
        .lines
        .into_iter()
        .filter(|line| match line {
            Line::Utterance(_) => {
                let retain = ordinal == 1 || ordinal == selected;
                ordinal += 1;
                retain
            }
            Line::Header { .. } => true,
        })
        .collect::<Vec<_>>()
        .into();
    assert_eq!(
        chat.utterances().count(),
        2,
        "both authored speakers remain"
    );
    chat
}

#[test]
fn replaced_speech_matches_its_original_without_counting_replacement_targets() {
    let chat = talkbank_transform::parse_and_validate(
        include_str!("../../../../corpus/reference/annotation/errors-and-replacements.cha"),
        ParseValidateOptions::default().with_validation(),
    )
    .expect("admitted replacement reference CHAT");
    // Authored controls: the donor says "this is child [= a+er b] speaking";
    // the references say child [: a+er b] and child [: children] [e].
    // All have four spoken tokens; replacement targets add no lexical weight.
    let donor = select_utterance(chat.clone(), 4);
    let donor_before = donor.to_chat_string();
    let child = SpeakerCode::new("CHI");
    let mother = SpeakerCode::new("MOT");
    for selected in [5, 8] {
        let reference = select_utterance(chat.clone(), selected);
        let reference_before = reference.to_chat_string();
        let report = identify_mapping(&reference, &child, &donor, ConfidenceThreshold::DEFAULT)
            .expect("spoken originals agree with the authored donor");
        assert_eq!(report.winner(), &child);
        let evidence = report.evidence_for(&child).expect("child support");
        assert_eq!(evidence.reference_tokens(), 4);
        assert_eq!(evidence.donor_tokens(), 4);
        assert_eq!(evidence.shared_tokens(), 4);
        assert_eq!(evidence.union_tokens(), 4);
        assert_eq!(evidence.score().value(), 1.0);
        assert_eq!(
            report
                .evidence_for(&mother)
                .expect("mother support")
                .shared_tokens(),
            0
        );
        assert!(matches!(report.margin(), ConfidenceMargin::Unbounded));
        let wire = serde_json::to_value(RecordedSpeakerIdentificationAttempt::accepted(
            &report,
            ConfidenceThreshold::DEFAULT,
        ))
        .expect("replacement support wire");
        assert_eq!(wire["outcome"], "accepted");
        assert_eq!(wire["match_report"]["speakers"]["CHI"]["shared_tokens"], 4);
        let reversed = identify_mapping(&donor, &child, &reference, ConfidenceThreshold::DEFAULT)
            .expect("input replacements follow the same spoken-form policy");
        assert_eq!(reversed.evidence_for(&child), report.evidence_for(&child));
        assert_eq!(reference.to_chat_string(), reference_before);
        assert_eq!(donor.to_chat_string(), donor_before);
    }
}
