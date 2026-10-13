//! Authored mixed-media comments retain their parsed order through admission.

#![allow(clippy::expect_used)]

use talkbank_model::model::{BulletContentSegment, Header, TranscriptName};
use talkbank_model::{ErrorCollector, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::parse_source_with_parser;

const SOURCE: &str = include_str!("../../../../corpus/reference/tiers/comment-media-segments.cha");

#[test]
fn authored_comments_keep_picture_timing_and_continuation_order() {
    let source = SOURCE.to_owned();
    let parser = TreeSitterParser::new().expect("grammar");
    let errors = ErrorCollector::new();
    let admitted = parse_source_with_parser(&parser, &source)
        .admit(
            TranscriptName::for_path(std::path::Path::new("comment-media-segments.cha")),
            &errors,
        )
        .expect("authored mixed-media reference is valid");
    assert!(errors.is_empty(), "{:?}", errors.to_vec());
    assert_eq!(admitted.document().to_chat_string(), SOURCE);
    let owned = admitted.into_owned();
    drop(source);
    assert_eq!(owned.source(), SOURCE);
    let valid = owned.into_valid_file();
    assert!(valid.diagnostics().is_empty());
    assert_eq!(valid.to_chat_string(), SOURCE);

    let document = valid.document();
    let utterance = document.utterances().next().expect("authored utterance");
    let comment = utterance.com().expect("authored comment tier");
    // Independent semantic checks make an omitted/reclassified picture fail
    // before a new snapshot can be mistaken for an accepted observation.
    assert!(matches!(
        comment.content.segments.first(),
        Some(BulletContentSegment::Picture(picture))
            if picture.filename == "images/frame-01.png"
    ));
    assert_eq!(
        comment
            .content
            .segments
            .iter()
            .filter(|segment| matches!(segment, BulletContentSegment::Picture(_)))
            .count(),
        3
    );
    assert_eq!(
        comment
            .content
            .segments
            .iter()
            .filter(|segment| matches!(segment, BulletContentSegment::Bullet(_)))
            .count(),
        2
    );
    assert_eq!(
        comment
            .content
            .segments
            .iter()
            .filter(|segment| matches!(segment, BulletContentSegment::Continuation))
            .count(),
        1
    );
    let headers: Vec<_> = document
        .headers()
        .filter_map(|header| match header {
            Header::Comment { content, .. } => Some(content),
            _ => None,
        })
        .collect();
    let action = utterance.act().expect("authored action tier");
    insta::assert_json_snapshot!(
        "authored_comment_media_segments",
        (&headers, &comment.content, &action.content)
    );
}
