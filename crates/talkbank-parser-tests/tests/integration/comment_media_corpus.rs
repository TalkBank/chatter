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

#[test]
fn rejected_bullet_segments_preserve_source_and_invalidity() {
    use talkbank_model::{ChatFile, ChatParser, ErrorCode};
    fn check(parser: &impl ChatParser, source: &str) {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .expect("recovery file");
        assert!(
            errors
                .to_vec()
                .iter()
                .any(|e| e.code == ErrorCode::InvalidMediaBullet)
        );
        let spelling = &source[source.find('\u{15}').expect("opening delimiter")..];
        let spelling = &spelling[..spelling[1..].find('\u{15}').expect("closing delimiter") + 2];
        fn contents(file: &ChatFile) -> Vec<&talkbank_model::BulletContent> {
            use talkbank_model::DependentTier;
            let mut result: Vec<_> = file
                .headers()
                .filter_map(|h| match h {
                    Header::Comment { content } => Some(content),
                    _ => None,
                })
                .collect();
            for utterance in file.utterances() {
                for entry in &utterance.dependent_tiers {
                    let content = match &entry.tier {
                        DependentTier::Act(t) => &t.content,
                        DependentTier::Cod(t) => &t.content,
                        DependentTier::Add(t) => &t.content,
                        DependentTier::Com(t) => &t.content,
                        DependentTier::Exp(t) => &t.content,
                        DependentTier::Gpx(t) => &t.content,
                        DependentTier::Int(t) => &t.content,
                        DependentTier::Sit(t) => &t.content,
                        DependentTier::Spa(t) => &t.content,
                        _ => continue,
                    };
                    result.push(content);
                }
            }
            result
        }
        let raw = contents(&file)
            .into_iter()
            .flat_map(|c| c.segments.iter())
            .find_map(|s| match s {
                BulletContentSegment::UnparsedBullet(raw) => Some(raw),
                _ => None,
            })
            .expect("explicit rejected bullet");
        assert_eq!(raw.source(), spelling);
        let span = raw.location().span().expect("located source");
        assert_eq!(&source[span.start()..span.end()], spelling);
        assert!(file.to_chat_string().contains(spelling));
        let source_line = source
            .lines()
            .find(|line| line.contains(spelling))
            .expect("bullet line");
        assert!(
            file.to_chat_string().contains(source_line),
            "surrounding content and order retained"
        );
        if source_line.starts_with("@Comment:") {
            let errors = ErrorCollector::new();
            let outcome = parser.parse_header(source_line, 173, &errors);
            assert!(
                errors
                    .to_vec()
                    .iter()
                    .any(|e| e.code == ErrorCode::InvalidMediaBullet)
            );
            // The Tree-sitter header-fragment boundary rejects any lowering
            // diagnostics. Native header recovery retains its explicit payload.
            if let talkbank_model::ParseOutcome::Parsed(header) = outcome {
                let Header::Comment { content } = header else {
                    panic!("comment variant")
                };
                let raw = content
                    .segments
                    .iter()
                    .find_map(|s| match s {
                        BulletContentSegment::UnparsedBullet(raw) => Some(raw),
                        _ => None,
                    })
                    .expect("fragment recovery");
                assert_eq!(raw.source(), spelling);
                let span = raw.location().span().expect("fragment location");
                assert_eq!(
                    span.start(),
                    173 + source_line.find(spelling).expect("bullet position")
                );
                assert_eq!(span.end() - span.start(), spelling.len());
            }
        }
        let json = serde_json::to_string(&file).expect("serialize recovery");
        assert!(json.contains("unparsed_bullet"));
        let decoded: ChatFile = serde_json::from_str(&json).expect("deserialize recovery");
        assert!(decoded.to_chat_string().contains(spelling));
        for document in [&file, &decoded] {
            let errors = ErrorCollector::new();
            document.validate(&errors, TranscriptName::Anonymous);
            assert!(
                errors
                    .to_vec()
                    .iter()
                    .any(|e| e.code == ErrorCode::InvalidMediaBullet),
                "recovery must never validate"
            );
            assert!(
                !contents(document)
                    .into_iter()
                    .flat_map(|c| c.segments.iter())
                    .any(|s| matches!(s, BulletContentSegment::Bullet(_)))
            );
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates")
        .parent()
        .expect("root");
    let specs = talkbank_parser_tests::error_specs::load(root).expect("authored specs");
    let spec = specs
        .iter()
        .find(|s| s.filename == "E360.md")
        .expect("E360");
    let native = talkbank_parser_re2c::Re2cParser::new();
    let tree = TreeSitterParser::new().expect("grammar");
    for title in [
        "Comment header overflow is not a zero timestamp",
        "Overflow only the inline start timestamp",
        "Overflow only the inline end timestamp",
        "An inline dependent-tier bullet rejects the all-zero pair",
    ] {
        let source = &spec
            .examples()
            .iter()
            .find(|e| e.title.as_deref() == Some(title))
            .expect("example")
            .chat;
        let source = source.as_str();
        if source.contains("%com:\t") {
            // The nine structured text tiers share one recovery/validation owner.
            for label in [
                "act", "cod", "add", "com", "exp", "gpx", "int", "sit", "spa",
            ] {
                let source = source.replace("%com:\t", &format!("%{label}:\t"));
                check(&native, &source);
                check(&tree, &source);
            }
        } else {
            check(&native, source);
            check(&tree, source);
        }
    }
}
