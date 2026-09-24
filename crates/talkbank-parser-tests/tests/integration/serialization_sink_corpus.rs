//! Writer refusal is an external-boundary contract, exercised on parsed CHAT.
//!
//! This checks propagation and prefix preservation, not the correctness of the
//! canonical spelling (the reference roundtrip tests own that obligation).

use std::fmt::{self, Write};
use talkbank_model::model::WriteChat;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

#[derive(Default)]
struct ObservedWrites {
    output: String,
    calls: usize,
}

impl Write for ObservedWrites {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.calls += 1;
        self.output.push_str(text);
        Ok(())
    }
}

/// Once the sink refuses, it cannot resume accepting output.
enum SinkState<'a> {
    Writable {
        remaining_calls: usize,
        unwritten: &'a str,
    },
    Refused,
}

struct RefusingWriter<'a>(SinkState<'a>);

impl<'a> RefusingWriter<'a> {
    fn after_calls(expected: &'a str, accepted_calls: usize) -> Self {
        Self(SinkState::Writable {
            remaining_calls: accepted_calls,
            unwritten: expected,
        })
    }

    fn assert_refused(self) {
        assert!(matches!(self.0, SinkState::Refused));
    }
}

impl Write for RefusingWriter<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        match &mut self.0 {
            SinkState::Writable {
                remaining_calls: 0, ..
            } => {
                self.0 = SinkState::Refused;
                Err(fmt::Error)
            }
            SinkState::Writable {
                remaining_calls,
                unwritten,
            } => {
                *unwritten = unwritten
                    .strip_prefix(text)
                    .expect("accepted writes preserve the normal output prefix");
                *remaining_calls -= 1;
                Ok(())
            }
            SinkState::Refused => panic!("serializer continued after writer refusal"),
        }
    }
}

#[test]
fn reference_serialization_propagates_every_writer_refusal() {
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let parser = TreeSitterParser::new().expect("parser");
    for fixture in corpus.fixtures() {
        let parsed = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("canonical reference parses");
        assert!(assert_writer_refusals(&parsed, fixture.path()) > 0);
    }
}

#[test]
fn spec_serialization_propagates_writer_refusal_without_certifying_recovery() {
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical specification corpus");
    let parser = TreeSitterParser::new().expect("parser");
    let mut boundaries = [0; 2];
    for fixture in corpus.fixtures() {
        let diagnostics = talkbank_model::ErrorCollector::new();
        let parsed = parser.parse_chat_file_streaming(fixture.source(), &diagnostics);
        // Keep recovery observable, but never interpret its serialization as
        // a validity certificate or as the original source's canonical form.
        let recovered = usize::from(!diagnostics.to_vec().is_empty());
        boundaries[recovered] += assert_writer_refusals(&parsed, fixture.path());
    }
    assert!(
        boundaries.iter().all(|count| *count > 0),
        "both clean and recovered models need real write-boundary witnesses: {boundaries:?}"
    );
    eprintln!(
        "spec writer boundaries: clean={}, recovered={}",
        boundaries[0], boundaries[1]
    );
}

#[test]
fn canonical_admission_outputs_propagate_every_sink_refusal() {
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCollector, NullErrorSink};
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical specification corpus");
    let parser = TreeSitterParser::new().expect("parser");
    let mut boundaries = [0; 3];
    for fixture in corpus.fixtures() {
        let parsed = parser.parse_chat_file_streaming(fixture.source(), &ErrorCollector::new());
        // Model admission is not a certificate for recovered source. Both
        // rejection variants must remain reportable when an output sink fails.
        match parsed.validate_into(&NullErrorSink, TranscriptName::Anonymous) {
            Ok(accepted) => boundaries[0] += assert_writer_refusals(&accepted, fixture.path()),
            Err(failure) => {
                let index = 1 + usize::from(failure.has_incomplete_parse());
                boundaries[index] += assert_display_refusals(&failure, fixture.path());
            }
        }
    }
    assert!(
        boundaries.iter().all(|count| *count > 0),
        "accepted, rejected and incomplete outputs need canonical witnesses: {boundaries:?}"
    );
}

fn assert_display_refusals(value: &impl fmt::Display, path: &std::path::Path) -> usize {
    assert_output_refusals(|writer| write!(writer, "{value}"), path)
}

#[test]
fn canonical_scoped_annotation_display_preserves_chat_and_writer_refusal() {
    use talkbank_model::model::Descend;
    let parser = TreeSitterParser::new().expect("parser");
    let mut boundaries = [0; 2];
    for (index, root) in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ]
    .into_iter()
    .enumerate()
    {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let file = parser.parse_chat_file_streaming(
                fixture.source(),
                &talkbank_model::ErrorCollector::new(),
            );
            // Display is a diagnostic boundary, not recovered-source admission.
            // Use the structural owner so nested and replaced annotations are
            // not silently omitted by a second word-only traversal.
            for utterance in file.utterances() {
                for item in &utterance.main.content.content {
                    item.structure().walk(&mut |structure| {
                        for annotation in structure.scoped_annotations() {
                            let mut chat = String::new();
                            annotation.write_chat(&mut chat).expect("accepting writer");
                            assert_eq!(
                                annotation.to_string(),
                                chat,
                                "{}",
                                fixture.path().display()
                            );
                            boundaries[index] +=
                                assert_display_refusals(annotation, fixture.path());
                        }
                        Descend::Into
                    });
                }
            }
        }
    }
    assert!(
        boundaries.iter().all(|count| *count > 0),
        "reference and error specs must both exercise annotation display: {boundaries:?}"
    );
}

#[test]
fn canonical_replacement_display_preserves_payload_and_writer_refusal() {
    use talkbank_model::model::{ContentStructure, Descend, WordRef};
    let parser = TreeSitterParser::new().expect("parser");
    let mut boundaries = [0; 2];
    let mut multiword = 0;
    let mut annotated = 0;
    for (index, root) in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ]
    .into_iter()
    .enumerate()
    {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let file = parser.parse_chat_file_streaming(
                fixture.source(),
                &talkbank_model::ErrorCollector::new(),
            );
            for utterance in file.utterances() {
                for item in &utterance.main.content.content {
                    item.structure().walk(&mut |structure| {
                        if let ContentStructure::Word(WordRef::Replaced(replaced)) = structure {
                            let mut chat = String::new();
                            replaced.write_chat(&mut chat).expect("accepting writer");
                            assert_eq!(replaced.to_string(), chat, "{}", fixture.path().display());
                            boundaries[index] += assert_display_refusals(replaced, fixture.path());
                            // The payload is independently usable, but the enclosing
                            // word must use this same owner for its bracketed spelling.
                            assert!(
                                assert_output_refusals(
                                    |mut writer| replaced.replacement.write_chat(&mut writer),
                                    fixture.path(),
                                ) > 0
                            );
                            multiword += usize::from(replaced.replacement.words.len() > 1);
                            annotated += usize::from(!replaced.scoped_annotations.is_empty());
                        }
                        Descend::Into
                    });
                }
            }
        }
    }
    assert!(boundaries.iter().all(|count| *count > 0));
    assert!(
        multiword > 0 && annotated > 0,
        "multiword and annotated replacements need canonical witnesses"
    );
}

fn assert_writer_refusals(parsed: &impl WriteChat, path: &std::path::Path) -> usize {
    assert_output_refusals(|mut writer| parsed.write_chat(&mut writer), path)
}

fn assert_output_refusals(
    write_output: impl Fn(&mut dyn Write) -> fmt::Result,
    path: &std::path::Path,
) -> usize {
    let mut observed = ObservedWrites::default();
    write_output(&mut observed).expect("accepting sink");
    // Each actual write boundary is refused once. No fabricated model,
    // arbitrary error transcript, or golden-output update is involved.
    for accepted_calls in 0..observed.calls {
        let mut refusing = RefusingWriter::after_calls(&observed.output, accepted_calls);
        assert!(
            write_output(&mut refusing).is_err(),
            "{}: swallowed refusal after {accepted_calls} writes",
            path.display()
        );
        refusing.assert_refused();
    }
    observed.calls
}

#[test]
fn canonical_morphology_content_output_preserves_prefix_and_sink_refusal() {
    let parser = TreeSitterParser::new().expect("parser");
    let mut tiers = 0;
    let mut empty = 0;
    let mut clitics = 0;
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let file = parser.parse_chat_file_streaming(
                fixture.source(),
                &talkbank_model::ErrorCollector::new(),
            );
            for utterance in file.utterances() {
                let Some(mor) = utterance.mor_tier() else {
                    continue;
                };
                assert!(mor.is_mor());
                let prefix = mor.tier_type.to_chat_string();
                assert_eq!(prefix, "%mor");
                let content = mor.to_content();
                assert_eq!(mor.to_chat(), format!("{prefix}:\t{content}"));
                let mut streamed = String::new();
                mor.write_content(&mut streamed)
                    .expect("accepting content sink");
                assert_eq!(streamed, content);
                assert!(
                    assert_output_refusals(
                        |mut writer| mor.write_content(&mut writer),
                        fixture.path()
                    ) > 0
                );
                tiers += 1;
                empty += usize::from(mor.is_empty());
                clitics +=
                    usize::from(mor.items().iter().any(|item| !item.post_clitics.is_empty()));
            }
        }
    }
    assert!(
        tiers > 0 && empty > 0 && clitics > 0,
        "ordinary, empty and post-clitic tiers need real witnesses: tiers={tiers}, empty={empty}, clitics={clitics}"
    );
}
