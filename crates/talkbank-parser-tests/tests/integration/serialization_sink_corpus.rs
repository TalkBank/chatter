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

#[path = "word_spelling_corpus.rs"]
mod word_spelling_contracts;

#[path = "json_sink_corpus.rs"]
mod json_sink_contracts;

#[path = "header_output_corpus.rs"]
mod header_output_contracts;

#[path = "walk_view_corpus.rs"]
mod walk_view_contracts;

#[path = "bullet_body_corpus.rs"]
mod bullet_body_contracts;

#[path = "event_display_corpus.rs"]
mod event_display_contracts;

#[test]
fn reference_group_construction_preserves_payload_spans_and_output() {
    use talkbank_model::model::{
        BracketedItems, ContentStructure, Descend, Group, GroupKind, GroupRef, PhoGroup, Quotation,
        SinGroup,
    };
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut witnessed = [0usize; 4];
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference syntax");
        if fixture.path().file_name().and_then(|name| name.to_str())
            == Some("phon-constituent-controls.cha")
        {
            let errors = talkbank_model::ErrorCollector::new();
            assert!(
                file.clone()
                    .validate_into(&errors, talkbank_model::model::TranscriptName::Anonymous,)
                    .is_ok(),
                "authored constituent control must validate: {:?}",
                errors.into_vec()
            );
        }
        for utterance in file.utterances() {
            for item in &utterance.main.content.content {
                item.structure().walk(&mut |structure| {
                    if let ContentStructure::Group(group) = structure {
                        let content = group.content();
                        // The content-only public writer is distinct from the
                        // enclosing group's spacing/decoration policy.
                        let writes = assert_writer_refusals(content, fixture.path());
                        assert_eq!(writes > 0, !content.is_empty());
                        let transferred = content.content.clone().into_iter().collect();
                        let restored = BracketedItems::new(transferred);
                        assert_eq!(
                            restored, content.content,
                            "owned transfer preserves bracketed items and spans"
                        );
                        let count = content.content.iter().count();
                        match group {
                            GroupRef::Angle(view) => {
                                assert_eq!(group.kind(), GroupKind::Angle);
                                assert_eq!(group.span(), Some(view.span));
                                let original = view.group;
                                let mut rebuilt = Group::new(content.clone())
                                    .with_span(original.span)
                                    .with_trailing_space("");
                                assert!(
                                    rebuilt.trailing_space.is_none(),
                                    "empty decoration adds no whitespace"
                                );
                                if let Some(space) = &original.trailing_space {
                                    rebuilt = rebuilt.with_trailing_space(space.clone());
                                }
                                assert_eq!(rebuilt.span, original.span);
                                assert_eq!(rebuilt.len(), count);
                                assert_eq!(rebuilt.is_empty(), count == 0);
                                assert_eq!(rebuilt.to_chat_string(), original.to_chat_string());
                                assert_writer_refusals(&rebuilt, fixture.path());
                                witnessed[0] += 1;
                            }
                            GroupRef::Quotation(view) => {
                                assert_eq!(group.kind(), GroupKind::Quotation);
                                assert_eq!(group.span(), Some(view.span));
                                let rebuilt = Quotation::new(content.clone());
                                let located =
                                    Quotation::with_span(content.clone(), view.quotation.span);
                                assert_eq!(located.span, view.quotation.span);
                                assert_eq!(rebuilt.len(), count);
                                assert_eq!(rebuilt.is_empty(), count == 0);
                                assert_eq!(
                                    rebuilt.to_chat_string(),
                                    view.quotation.to_chat_string()
                                );
                                assert_eq!(located.to_chat_string(), rebuilt.to_chat_string());
                                assert_writer_refusals(&rebuilt, fixture.path());
                                witnessed[1] += 1;
                            }
                            GroupRef::Pho(original) => {
                                assert_eq!(group.kind(), GroupKind::Pho);
                                assert_eq!(group.span(), None);
                                let rebuilt = PhoGroup::new(content.clone());
                                assert_eq!(rebuilt.len(), count);
                                assert_eq!(rebuilt.is_empty(), count == 0);
                                assert_eq!(rebuilt.to_chat_string(), original.to_chat_string());
                                assert_writer_refusals(&rebuilt, fixture.path());
                                witnessed[2] += 1;
                            }
                            GroupRef::Sin(original) => {
                                assert_eq!(group.kind(), GroupKind::Sin);
                                assert_eq!(group.span(), None);
                                let rebuilt = SinGroup::new(content.clone());
                                assert_eq!(rebuilt.len(), count);
                                assert_eq!(rebuilt.is_empty(), count == 0);
                                assert_eq!(rebuilt.to_chat_string(), original.to_chat_string());
                                assert_writer_refusals(&rebuilt, fixture.path());
                                witnessed[3] += 1;
                            }
                        }
                    }
                    Descend::Into
                });
            }
        }
    }
    assert!(
        witnessed.iter().all(|count| *count > 0),
        "all four group families must have reference witnesses: {witnessed:?}"
    );
}

/// Mutable traversal is an ownership view, not permission to discard annotations.
#[test]
fn reference_mutable_container_views_preserve_structure_and_annotations() {
    use talkbank_model::model::{ContainerMut, ContentStructure, GroupKind, Line};

    fn compare(
        expected: ContentStructure<'_>,
        actual: Option<ContainerMut<'_>>,
        witnessed: &mut [usize; 6],
    ) {
        let content = match (expected, actual) {
            (ContentStructure::Group(group), Some(ContainerMut::Group(mutable))) => {
                let (kind, annotations, content) = mutable.into_parts();
                assert_eq!(kind, group.kind());
                assert_eq!(annotations, group.scoped_annotations());
                assert_eq!(&*content, group.content());
                let index = match kind {
                    GroupKind::Angle => 0,
                    GroupKind::Quotation => 1,
                    GroupKind::Pho => 2,
                    GroupKind::Sin => 3,
                };
                witnessed[index] += 1;
                content
            }
            (ContentStructure::Retrace(retrace), Some(ContainerMut::Retrace(content))) => {
                assert_eq!(&*content, &retrace.inner().content);
                witnessed[4] += 1;
                content
            }
            (ContentStructure::Word(_) | ContentStructure::Leaf(_), None) => {
                witnessed[5] += 1;
                return;
            }
            _ => panic!("read-only and mutable container views disagree"),
        };
        for item in &mut content.content {
            let original = item.clone();
            compare(original.structure(), item.container_mut(), witnessed);
        }
    }

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut witnessed = [0; 6];
    let mut separators = 0;
    for fixture in corpus.fixtures() {
        let mut file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference syntax");
        let before = file.to_chat_string();
        for line in &mut file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            separators += walk_view_contracts::assert_walk_views(
                utterance.main.content.content.as_mut_slice(),
                fixture.source(),
                fixture.path(),
            );
            for item in &mut utterance.main.content.content {
                let original = item.clone();
                compare(original.structure(), item.container_mut(), &mut witnessed);
            }
        }
        assert_eq!(file.to_chat_string(), before, "inspection preserves CHAT");
    }
    assert!(
        witnessed.iter().all(|count| *count > 0),
        "all container and leaf families need witnesses: {witnessed:?}"
    );
    assert!(
        separators > 0,
        "separator adapters require reference witnesses"
    );
}

#[test]
fn reference_syllable_codes_preserve_annotated_words_and_display() {
    use talkbank_model::model::DependentTier;
    use talkbank_model::model::dependent_tier::{
        PositionCode, SylToken, SylWordKind, classify_syl_word,
    };
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut codes = std::collections::BTreeSet::new();
    let mut pauses = 0;
    let mut non_ascii_phones = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference syntax");
        for utterance in file.utterances() {
            for entry in &utterance.dependent_tiers {
                let tier = match &entry.tier {
                    DependentTier::Modsyl(tier) | DependentTier::Phosyl(tier) => tier,
                    _ => continue,
                };
                for word in &tier.words {
                    let SylWordKind::Units(tokens) =
                        classify_syl_word(word.as_str()).expect("reference syllabification")
                    else {
                        continue;
                    };
                    let mut annotated = String::new();
                    for token in tokens {
                        match token {
                            SylToken::Unit(unit) => {
                                let code = unit.code.as_char();
                                assert_eq!(PositionCode::try_from(code), Ok(unit.code));
                                assert_eq!(unit.code.to_string(), code.to_string());
                                assert_display_refusals(&unit.code, fixture.path());
                                write!(&mut annotated, "{}:{}", unit.phone, unit.code)
                                    .expect("string sink");
                                codes.insert(code);
                                non_ascii_phones += usize::from(!unit.phone.as_str().is_ascii());
                            }
                            SylToken::IntraWordPause => {
                                annotated.push('^');
                                pauses += 1;
                            }
                        }
                    }
                    assert_eq!(
                        annotated,
                        word.as_str(),
                        "typed token display retains annotated spelling"
                    );
                }
            }
        }
    }
    assert_eq!(
        codes,
        std::collections::BTreeSet::from(['A', 'C', 'D', 'E', 'L', 'N', 'O', 'R', 'U'])
    );
    assert!(
        pauses > 0 && non_ascii_phones > 0,
        "pause and Unicode witnesses must execute"
    );
}

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
    use talkbank_model::model::Line;
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let parser = TreeSitterParser::new().expect("parser");
    let mut utterance_witnesses = 0;
    let mut line_witnesses = [0; 2];
    let mut headers = header_output_contracts::HeaderWitnesses::default();
    for fixture in corpus.fixtures() {
        let parsed = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("canonical reference parses");
        assert!(assert_writer_refusals(&parsed, fixture.path()) > 0);
        headers.observe(&parsed, fixture.path());
        // Public line dispatch must preserve the owned payload's CHAT output
        // and propagate sink failure for both variants, not merely work when
        // a document writer bypasses this layer and visits the payload itself.
        for line in &parsed.lines {
            let (kind, expected) = match line {
                Line::Header { header, .. } => (0, header.to_chat_string()),
                Line::Utterance(utterance) => (1, utterance.to_chat_string()),
            };
            assert_eq!(
                line.to_chat_string(),
                expected,
                "{}",
                fixture.path().display()
            );
            assert!(assert_writer_refusals(line, fixture.path()) > 0);
            line_witnesses[kind] += 1;
        }
        for utterance in parsed.utterances() {
            let output = utterance.to_chat_string();
            assert_eq!(utterance.to_chat(), output);
            assert_eq!(utterance.to_string(), output);
            assert!(assert_display_refusals(utterance, fixture.path()) > 0);
            utterance_witnesses += 1;
        }
    }
    assert!(
        utterance_witnesses > 0,
        "utterance output requires source witnesses"
    );
    headers.assert_reference_witnesses();
    assert!(line_witnesses.into_iter().all(|count| count > 0));
}

#[test]
fn spec_serialization_propagates_writer_refusal_without_certifying_recovery() {
    use talkbank_model::model::{ContentStructure, Descend};
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical specification corpus");
    let parser = TreeSitterParser::new().expect("parser");
    let mut boundaries = [0; 2];
    let mut diagnostic_boundaries = 0;
    let mut group_boundaries = [0; 2];
    let mut headers = header_output_contracts::HeaderWitnesses::default();
    for fixture in corpus.fixtures() {
        let diagnostics = talkbank_model::ErrorCollector::new();
        let parsed = parser.parse_chat_file_streaming(fixture.source(), &diagnostics);
        headers.observe(&parsed, fixture.path());
        // Keep recovery observable, but never interpret its serialization as
        // a validity certificate or as the original source's canonical form.
        let diagnostics = talkbank_model::ParseErrors::from(diagnostics.into_vec());
        let recovered = usize::from(!diagnostics.is_empty());
        boundaries[recovered] += assert_writer_refusals(&parsed, fixture.path());
        for utterance in parsed.utterances() {
            for item in &utterance.main.content.content {
                item.structure().walk(&mut |structure| {
                    if let ContentStructure::Group(group) = structure {
                        let content = group.content();
                        let writes = assert_writer_refusals(content, fixture.path());
                        assert_eq!(writes > 0, !content.is_empty());
                        group_boundaries[recovered] += writes;
                    }
                    Descend::Into
                });
            }
        }
        let expected: String = diagnostics
            .errors
            .iter()
            .map(|error| format!("{error}\n"))
            .collect();
        assert_eq!(
            diagnostics.to_string(),
            expected,
            "diagnostic display retains order and line boundaries"
        );
        diagnostic_boundaries += assert_display_refusals(&diagnostics, fixture.path());
    }
    assert!(
        boundaries.iter().all(|count| *count > 0),
        "both clean and recovered models need real write-boundary witnesses: {boundaries:?}"
    );
    assert!(
        diagnostic_boundaries > 0,
        "spec diagnostics must exercise failing output sinks"
    );
    headers.assert_spec_witnesses();
    assert!(
        group_boundaries.into_iter().all(|count| count > 0),
        "clean and recovered spec models must both exercise group content sinks"
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
    use talkbank_model::model::annotation::{ReplacedWordAnnotations, ReplacementWords};
    use talkbank_model::model::{ContentStructure, Descend, Replacement, WordRef};
    let parser = TreeSitterParser::new().expect("parser");
    let mut boundaries = [0; 2];
    let mut multiword = 0;
    let mut annotated = 0;
    let mut singleword = 0;
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
                            // Editors transfer parsed payloads through owned collections.
                            // Preserve order and spans, without upgrading recovery models
                            // into validated or source-bound values.
                            let words = ReplacementWords::new(
                                replaced.replacement.words.clone().into_vec(),
                            )
                            .expect("parsed replacement is nonempty");
                            assert_eq!(words, replaced.replacement.words);
                            if let [word] = words.as_slice() {
                                let single = Replacement::from_word(word.clone());
                                assert_eq!(single, replaced.replacement);
                                let mut rebuilt_chat = String::new();
                                single
                                    .write_chat(&mut rebuilt_chat)
                                    .expect("replacement output");
                                let mut original_chat = String::new();
                                replaced
                                    .replacement
                                    .write_chat(&mut original_chat)
                                    .expect("source replacement output");
                                assert_eq!(rebuilt_chat, original_chat);
                                singleword += 1;
                            }
                            let mut annotations = ReplacedWordAnnotations::new(
                                replaced.scoped_annotations.clone().into_iter().collect(),
                            );
                            assert_eq!(annotations, replaced.scoped_annotations);
                            let borrowed: Vec<_> = (&mut annotations).into_iter().collect();
                            assert_eq!(borrowed.len(), replaced.scoped_annotations.len());
                            for (actual, expected) in
                                borrowed.into_iter().zip(&replaced.scoped_annotations)
                            {
                                assert_eq!(actual, expected);
                            }
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
        singleword > 0 && multiword > 0 && annotated > 0,
        "single-word, multiword and annotated replacements need canonical witnesses"
    );
}

fn assert_writer_refusals(parsed: &impl WriteChat, path: &std::path::Path) -> usize {
    assert_output_refusals(|mut writer| parsed.write_chat(&mut writer), path)
}

pub(super) fn assert_output_refusals(
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
                // Generic output consumers use the trait, while MorTier also
                // exposes an inherent writer. Both must preserve the full line
                // and propagate refusal, including for recovered spec models.
                let mut generic = String::new();
                WriteChat::write_chat(mor, &mut generic).expect("generic tier sink");
                assert_eq!(generic, mor.to_chat());
                assert!(assert_writer_refusals(mor, fixture.path()) > 0);
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

/// Content-only consumers must agree with full-tier serialization and must
/// propagate sink failures for both phonology and grammatical relations.
/// Recovered spec models are serialization inputs here, not validity proofs.
#[test]
fn canonical_phonology_and_dependency_content_preserve_prefix_and_sink_refusal() {
    use talkbank_model::model::{DependentTier, GraTier, PhoTier, SemanticEq};
    let parser = TreeSitterParser::new().expect("parser");
    let mut families = [0; 3];
    let mut writes = 0;
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
                for entry in &utterance.dependent_tiers {
                    match &entry.tier {
                        DependentTier::Pho(tier) | DependentTier::Mod(tier) => {
                            let family = match &entry.tier {
                                DependentTier::Pho(_) => 0,
                                DependentTier::Mod(_) => 1,
                                _ => unreachable!("selected phonology variants"),
                            };
                            let prefix = ["%pho", "%mod"][family];
                            assert_eq!(tier.is_pho(), family == 0);
                            assert_eq!(tier.is_mod(), family == 1);
                            let content = tier.to_content();
                            assert_eq!(tier.to_chat(), format!("{prefix}:\t{content}"));
                            let mut streamed = String::new();
                            tier.write_content(&mut streamed)
                                .expect("accepting phonology sink");
                            assert_eq!(streamed, content);
                            writes += assert_output_refusals(
                                |mut writer| tier.write_content(&mut writer),
                                fixture.path(),
                            );
                            let wire = serde_json::to_string(tier).expect("phonology wire");
                            let restored: PhoTier =
                                serde_json::from_str(&wire).expect("phonology decode");
                            assert!(restored.semantic_eq(tier));
                            assert_eq!(restored.to_content(), content);
                            assert_eq!(restored.to_chat(), tier.to_chat());
                            families[family] += 1;
                        }
                        DependentTier::Gra(tier) => {
                            assert!(tier.is_gra());
                            for relation in tier.relations() {
                                let position = relation
                                    .index_as_semantic()
                                    .expect("parsed dependents have positive semantic indices");
                                assert_eq!(position.as_usize(), relation.index);
                                assert_eq!(relation.to_string(), relation.to_chat_string());
                                writes += assert_display_refusals(relation, fixture.path());
                            }
                            let content = tier.to_content();
                            assert_eq!(tier.to_chat(), format!("%gra:\t{content}"));
                            let mut streamed = String::new();
                            tier.write_content(&mut streamed)
                                .expect("accepting dependency sink");
                            assert_eq!(streamed, content);
                            writes += assert_output_refusals(
                                |mut writer| tier.write_content(&mut writer),
                                fixture.path(),
                            );
                            let wire = serde_json::to_string(tier).expect("dependency wire");
                            let restored: GraTier =
                                serde_json::from_str(&wire).expect("dependency decode");
                            assert!(restored.semantic_eq(tier));
                            assert_eq!(restored.to_content(), content);
                            assert_eq!(restored.to_chat(), tier.to_chat());
                            // A wire roundtrip retains content, not the parser's
                            // completeness provenance.
                            assert!(matches!(
                                restored.completeness(),
                                talkbank_model::model::GraCompleteness::Unknown
                            ));
                            families[2] += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    assert!(families.into_iter().all(|count| count > 0));
    assert!(
        writes > 0,
        "actual content writes must exercise sink refusal"
    );
}
