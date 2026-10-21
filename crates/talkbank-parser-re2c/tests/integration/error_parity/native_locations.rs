//! Native diagnostic locations are a wire contract, not cross-backend identity.

use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, Span};

/// Recovery preserves the written scope without inventing a retrace marker.
/// Serialization and diagnostic retention are separate external contracts.
#[test]
fn unannotated_groups_preserve_content_and_errors() {
    use talkbank_model::Severity;
    use talkbank_model::model::WriteChat;

    #[derive(Clone, Copy, Debug)]
    enum Api {
        File,
        Utterance,
        Main,
    }

    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    for (filename, index, invalid) in [
        ("E342.md", 0, true),
        ("E342_nested_group.md", 0, false),
        ("E342_nested_group.md", 1, true),
    ] {
        let spec = specs.iter().find(|spec| spec.filename == filename).unwrap();
        let source = spec.examples()[index].chat.as_str();
        let main_source = source.lines().find(|line| line.starts_with('*')).unwrap();
        for origin in [0, "Préface café\n".len()] {
            for api in [Api::File, Api::Utterance, Api::Main] {
                let sink = ErrorCollector::new();
                let main = match api {
                    Api::File => {
                        let ParseOutcome::Parsed(file) =
                            parser.parse_chat_file(source, origin, &sink)
                        else {
                            panic!("expected retained file content");
                        };
                        file.utterances().next().unwrap().main.clone()
                    }
                    Api::Utterance => {
                        let ParseOutcome::Parsed(utterance) =
                            parser.parse_utterance(main_source, origin, &sink)
                        else {
                            panic!("expected retained utterance content");
                        };
                        utterance.main
                    }
                    Api::Main => {
                        let ParseOutcome::Parsed(main) =
                            parser.parse_main_tier(main_source, origin, &sink)
                        else {
                            panic!("expected retained main-tier content");
                        };
                        main
                    }
                };
                let diagnostics = sink.into_vec();
                if !invalid {
                    assert!(diagnostics.is_empty(), "legal control: {diagnostics:?}");
                }
                assert_eq!(
                    diagnostics.iter().any(|error| {
                        error.code == ErrorCode::MissingRequiredElement
                            && error.severity == Severity::Error
                    }),
                    invalid,
                    "{filename}/{index} {api:?}: {diagnostics:?}"
                );
                assert_eq!(
                    main.to_chat_string().trim_end(),
                    main_source,
                    "{filename}/{index} {api:?}: preserve written scope and contents"
                );
            }
        }
    }
}

/// Existing authored faults retain truthful token/line locations under every
/// supported line ending and a nonzero UTF-8 caller origin.
#[test]
fn file_recovery_diagnostics_use_their_original_lexer_regions() {
    let root = talkbank_parser_tests::repo_paths::workspace_root();
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    let mut failures = Vec::new();
    let mut measured = 0;
    for (filename, index, code, needle, whitespace, count) in [
        (
            "E748.md",
            0,
            ErrorCode::LeadingZeroBulletTime,
            "\u{15}012_200\u{15}",
            None,
            1,
        ),
        (
            "E748.md",
            1,
            ErrorCode::LeadingZeroBulletTime,
            "\u{15}100_012\u{15}",
            None,
            1,
        ),
        (
            "E748.md",
            3,
            ErrorCode::LeadingZeroBulletTime,
            "\u{15}012_0200\u{15}",
            None,
            2,
        ),
        (
            "E750.md",
            0,
            ErrorCode::SpaceInsideAngleGroup,
            "< dog>",
            Some(1),
            1,
        ),
        (
            "E750.md",
            1,
            ErrorCode::SpaceInsideAngleGroup,
            "<dog >",
            Some(4),
            1,
        ),
        ("E256.md", 0, ErrorCode::IllegalCurlyQuote, "’", None, 1),
        (
            "E766.md",
            0,
            ErrorCode::LinkerNotUtteranceInitial,
            "+\"",
            None,
            1,
        ),
        (
            "E766.md",
            1,
            ErrorCode::LinkerNotUtteranceInitial,
            "++",
            None,
            1,
        ),
        (
            "E766.md",
            2,
            ErrorCode::LinkerNotUtteranceInitial,
            "+<",
            None,
            1,
        ),
        (
            "E342.md",
            0,
            ErrorCode::MissingRequiredElement,
            "*PAR:\t<I don't> &-uh I know xxx .\n",
            None,
            1,
        ),
        (
            "E404.md",
            0,
            ErrorCode::UnparsableLine,
            "%mor:\tco|hello .\n",
            None,
            1,
        ),
    ] {
        let spec = specs.iter().find(|spec| spec.filename == filename).unwrap();
        for ending in ["\n", "\r\n", "\r"] {
            let source = spec.examples()[index].chat.as_str().replace('\n', ending);
            let needle = needle.replace('\n', ending);
            assert_eq!(
                source.match_indices(&needle).count(),
                1,
                "unique authored region: {filename}/{index}"
            );
            let base = source.find(&needle).unwrap();
            let region = match whitespace {
                Some(offset) => base + offset..base + offset + 1,
                None => base..base + needle.len(),
            };
            for origin in [0, "Préface café\n".len()] {
                let sink = ErrorCollector::new();
                assert!(matches!(
                    parser.parse_chat_file(&source, origin, &sink),
                    ParseOutcome::Parsed(_)
                ));
                let actual: Vec<_> = sink
                    .into_vec()
                    .into_iter()
                    .filter(|error| error.code == code)
                    .map(|error| error.location.span().expect("located diagnostic"))
                    .collect();
                let expected = vec![Span::new(origin + region.start, origin + region.end); count];
                if actual != expected {
                    failures.push(format!("{filename}/{index} ending {ending:?} origin {origin}: expected {expected:?}, got {actual:?}"));
                }
                measured += 1;
            }
        }
    }
    for (filename, index, code) in [
        ("E748.md", 2, ErrorCode::LeadingZeroBulletTime),
        ("E256.md", 1, ErrorCode::IllegalCurlyQuote),
        ("E342.md", 5, ErrorCode::MissingRequiredElement),
    ] {
        let spec = specs.iter().find(|spec| spec.filename == filename).unwrap();
        for ending in ["\n", "\r\n", "\r"] {
            let source = spec.examples()[index].chat.as_str().replace('\n', ending);
            for origin in [0, "Préface café\n".len()] {
                let sink = ErrorCollector::new();
                assert!(matches!(
                    parser.parse_chat_file(&source, origin, &sink),
                    ParseOutcome::Parsed(_)
                ));
                assert!(
                    sink.into_vec().iter().all(|error| error.code != code),
                    "legal control {filename}/{index}"
                );
                measured += 1;
            }
        }
    }
    assert_eq!(measured, 84);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
