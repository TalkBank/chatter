//! Coordinated mutation admission over actual reference morphological tiers.
#![allow(clippy::expect_used)]

use talkbank_model::model::SemanticEq;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

#[test]
fn feature_specs_preserve_main_and_clitic_values_and_independent_diagnostics() {
    use talkbank_model::model::{MorFeature, MorTier, WriteChat};
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_parser_tests::repo_paths::workspace_root;
    let parser = TreeSitterParser::new().expect("parser");
    for (index, expected) in [
        [(None, "Nom"), (None, "Pres")],
        [(Some("Case"), "Nom"), (Some("Tense"), "Pres")],
        [(Some("Case"), ""), (Some("Tense"), "Pres")],
        [(Some("Case"), "Nom"), (Some("Tense"), "")],
        [(Some("Case"), ""), (Some("Tense"), "")],
    ]
    .into_iter()
    .enumerate()
    {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E711_postclitic_features_{}.cha", index + 1,
        ))).expect("canonical feature mutation");
        let file = strict_parse(parser.parse_chat_file(&source))
            .expect("all feature variants retain syntax");
        let utterance = file.utterances().next().expect("authored utterance");
        let mor = utterance.mor_tier().expect("authored morphology");
        assert_eq!(mor.len(), 1);
        assert_eq!(mor.count_chunks(), 3);
        let item = &mor.items()[0];
        assert_eq!(item.post_clitics.len(), 1);
        let words = [&item.main, &item.post_clitics[0]];
        for (word, (key, value)) in words.into_iter().zip(expected) {
            assert_eq!(word.features.len(), 1);
            let feature = &word.features[0];
            assert_eq!(feature.key(), key);
            assert_eq!(feature.value(), value);
            assert_eq!(feature.is_flat(), key.is_none());
            assert_eq!(feature.is_empty(), value.is_empty());
            // Public constructor roundtrip from observed components must not
            // infer a key, normalize spelling, or silently discard emptiness.
            let rebuilt = match feature.key() {
                Some(key) => MorFeature::with_key_value(key, feature.value()),
                None => MorFeature::flat(feature.value()),
            };
            assert!(rebuilt.semantic_eq(feature));
            assert_eq!(
                serde_json::to_value(&rebuilt).expect("rebuilt feature"),
                serde_json::to_value(feature).expect("source feature")
            );
        }
        let expected_errors = expected
            .iter()
            .filter(|(_, value)| value.is_empty())
            .count();
        let errors = ErrorCollector::new();
        mor.validate_content(&errors);
        let diagnostics = errors.into_vec();
        assert_eq!(diagnostics.len(), expected_errors);
        assert!(
            diagnostics
                .iter()
                .all(|error| error.code == ErrorCode::MorEmptyContent)
        );
        assert!(
            diagnostics
                .iter()
                .all(|error| error.location.span == mor.span)
        );
        let wire = serde_json::to_string(mor).expect("serialize morphology");
        let decoded: MorTier = serde_json::from_str(&wire).expect("decode morphology");
        assert!(decoded.semantic_eq(mor));
        let wire_errors = ErrorCollector::new();
        decoded.validate_content(&wire_errors);
        assert_eq!(
            wire_errors.into_vec().len(),
            expected_errors,
            "wire decoding cannot make an empty feature valid"
        );
        assert_eq!(
            file.to_chat_string(),
            source,
            "feature validation does not rewrite invalid values"
        );
    }
}

#[test]
fn reference_clitic_projection_preserves_authored_items_and_dependency_heads() {
    use talkbank_model::alignment::indices::{GraHeadRef, MorItemIndex};
    use talkbank_model::model::MorChunkKind;
    use talkbank_parser_tests::repo_paths::workspace_root;
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/edge-cases/clitics-and-compounds.cha"),
    )
    .expect("canonical clitic reference");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let utterance = file.utterances().next().expect("it's a cookie");
    let mor = utterance.mor_tier().expect("authored morphology");
    let gra = utterance.gra_tier().expect("authored dependencies");
    assert_eq!(mor.count_chunks(), 5);
    // Authored order: it~be a cookie . ; the two first chunks share item 0.
    let expected = [
        (MorChunkKind::Main, Some("it"), Some(0)),
        (MorChunkKind::PostClitic, Some("be"), Some(0)),
        (MorChunkKind::Main, Some("a"), Some(1)),
        (MorChunkKind::Main, Some("cookie"), Some(2)),
        (MorChunkKind::Terminator, None, None),
    ];
    for (index, (kind, lemma, host)) in expected.into_iter().enumerate() {
        let chunk = mor.chunk_at(index).expect("authored chunk");
        assert_eq!(chunk.kind(), kind);
        assert_eq!(chunk.lemma(), lemma);
        assert_eq!(mor.item_index_of_chunk(index), host);
        match host {
            Some(item) => {
                assert!(std::ptr::eq(
                    chunk.host_item().expect("host"),
                    &mor.items()[item]
                ));
                assert!(chunk.terminator().is_none());
            }
            None => {
                assert!(chunk.host_item().is_none());
                assert!(std::ptr::eq(
                    chunk.terminator().expect("terminator"),
                    &mor.terminator
                ));
            }
        }
    }
    for (item, (start, head)) in [(1, 4), (3, 4), (4, 0)].into_iter().enumerate() {
        let item = MorItemIndex::new(item);
        assert_eq!(
            mor.semantic_index_of_item_start(item)
                .expect("item start")
                .as_usize(),
            start
        );
        assert_eq!(
            mor.governing_head_for_item(gra, item)
                .expect("authored head"),
            GraHeadRef::from_raw(head)
        );
    }
    for index in [5, usize::MAX] {
        assert!(mor.chunk_at(index).is_none());
        assert!(mor.item_index_of_chunk(index).is_none());
    }
    for index in [mor.len(), usize::MAX] {
        let item = MorItemIndex::new(index);
        assert!(mor.semantic_index_of_item_start(item).is_none());
        assert!(matches!(mor.governing_head_for_item(gra, item),
            Err(talkbank_model::model::dependent_tier::mor::tier::CoordinatedMutationError::ItemIndexOutOfBounds {
                index: actual, len,
            }) if actual == index && len == mor.len()));
    }
}

#[test]
fn canonical_morphology_projection_refuses_missing_relation_slots() {
    use talkbank_model::ErrorCollector;
    use talkbank_model::alignment::indices::MorItemIndex;
    use talkbank_model::model::dependent_tier::mor::tier::CoordinatedMutationError;
    use talkbank_parser_tests::repo_paths::workspace_root;
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical error specifications");
    let parser = TreeSitterParser::new().expect("parser");
    let mut missing = 0;
    for fixture in corpus.fixtures() {
        let file = parser.parse_chat_file_streaming(fixture.source(), &ErrorCollector::new());
        for utterance in file.utterances() {
            let (Some(mor), Some(gra)) = (utterance.mor_tier(), utterance.gra_tier()) else {
                continue;
            };
            for index in 0..mor.len() {
                let item = MorItemIndex::new(index);
                let start = mor
                    .semantic_index_of_item_start(item)
                    .expect("existing item");
                match mor.governing_head_for_item(gra, item) {
                    Ok(head) => assert_eq!(
                        head,
                        gra.relation_at_semantic_index(start)
                            .expect("present slot")
                            .head_ref()
                    ),
                    Err(CoordinatedMutationError::GraRelationMissing {
                        semantic_index,
                        gra_len,
                    }) => {
                        assert_eq!(semantic_index, start);
                        assert_eq!(gra_len, gra.len());
                        assert!(gra.relation_at_semantic_index(start).is_none());
                        missing += 1;
                    }
                    Err(error) => panic!(
                        "existing item projection: {}: {error}",
                        fixture.path().display()
                    ),
                }
            }
        }
    }
    assert!(
        missing > 0,
        "specs must witness missing relation slots, not just valid projections"
    );
}

/// Parsed tiers admitted by the canonical alignment owner. This is evidence
/// of count/index alignment, not a certificate of dependency-tree validity.
struct AlignedReferencePair<'a> {
    mor: &'a talkbank_model::model::MorTier,
    gra: &'a talkbank_model::model::GraTier,
}

impl<'a> AlignedReferencePair<'a> {
    fn admit(
        mor: &'a talkbank_model::model::MorTier,
        gra: &'a talkbank_model::model::GraTier,
    ) -> Option<Self> {
        talkbank_model::alignment::align_mor_to_gra(mor, gra)
            .errors
            .is_empty()
            .then_some(Self { mor, gra })
    }

    fn exercise_construction(&self) {
        use talkbank_model::alignment::{
            MorGraConstructionError, MorGraTerminatorSlot, try_align_mor_gra,
        };
        let (terminal_relation, item_relations) = self
            .gra
            .relations()
            .split_last()
            .expect("aligned morphology always includes its terminator");
        let terminal = MorGraTerminatorSlot {
            terminator: self.mor.terminator.clone(),
            relation: terminal_relation.clone(),
        };
        let construct = |relations| {
            try_align_mor_gra(
                self.mor.items().to_vec(),
                relations,
                terminal.clone(),
                self.mor.span,
            )
        };
        let (mor, gra) = construct(item_relations.to_vec()).expect("admitted reference pair");
        assert!(mor.semantic_eq(self.mor));
        assert!(gra.semantic_eq(self.gra));
        assert_eq!(mor.span, self.mor.span);
        assert_eq!(
            gra.span, self.mor.span,
            "constructor assigns the supplied shared span"
        );
        assert!(
            talkbank_model::alignment::align_mor_to_gra(&mor, &gra)
                .errors
                .is_empty()
        );

        // Deliberate API error variant: count the existing terminator relation
        // twice. The expected refusal comes from the constructor's contract,
        // not from blessing its current serialized output as golden CHAT.
        assert_eq!(
            construct(self.gra.relations().to_vec()).expect_err("extra relation"),
            MorGraConstructionError::CountMismatch {
                mor_chunks: item_relations.len(),
                gra_relations: item_relations.len() + 1,
            }
        );
        if let Some((_, short)) = item_relations.split_last() {
            assert_eq!(
                construct(short.to_vec()).expect_err("missing relation"),
                MorGraConstructionError::CountMismatch {
                    mor_chunks: item_relations.len(),
                    gra_relations: short.len(),
                }
            );
        }
    }
}

#[test]
fn reference_mor_gra_co_construction_preserves_pairs_and_refuses_count_variants() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut pairs = 0;
    let mut clitic_pairs = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            let (Some(mor), Some(gra)) = (utterance.mor_tier(), utterance.gra_tier()) else {
                continue;
            };
            let Some(pair) = AlignedReferencePair::admit(mor, gra) else {
                continue;
            };
            pair.exercise_construction();
            pairs += 1;
            clitic_pairs += usize::from(mor.items().iter().any(|item| item.count_chunks() > 1));
        }
    }
    assert!(pairs > 0, "reference corpus must supply aligned pairs");
    assert!(
        clitic_pairs > 0,
        "reference corpus must distinguish chunks from items"
    );
}

/// A complete parsed lexical block with only block-relative dependency heads.
/// The constructor retains the admitted tier pair, rather than letting its
/// chunk count or donor relations drift independently.
struct DonorBlock {
    mor: talkbank_model::model::MorTier,
    gra: talkbank_model::model::GraTier,
    chunks: usize,
}

impl DonorBlock {
    fn admit(
        mor: &talkbank_model::model::MorTier,
        gra: &talkbank_model::model::GraTier,
    ) -> Option<Self> {
        let chunks: usize = mor.items().iter().map(|item| item.count_chunks()).sum();
        if chunks == 0
            || !talkbank_model::alignment::align_mor_to_gra(mor, gra)
                .errors
                .is_empty()
        {
            return None;
        }
        let relations = gra.relations().get(..chunks)?;
        if relations.iter().any(|relation| relation.head > chunks) {
            return None;
        }
        Some(Self {
            mor: mor.clone(),
            gra: gra.clone(),
            chunks,
        })
    }
}

#[test]
fn reference_coordinated_replacement_grows_and_shrinks_with_admitted_donors() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut blocks: Vec<DonorBlock> = Vec::new();
    'fixtures: for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            let (Some(mor), Some(gra)) = (utterance.mor_tier(), utterance.gra_tier()) else {
                continue;
            };
            let Some(block) = DonorBlock::admit(mor, gra) else {
                continue;
            };
            if blocks
                .first()
                .is_some_and(|first| first.chunks == block.chunks)
            {
                continue;
            }
            blocks.push(block);
            if blocks.len() == 2 {
                break 'fixtures;
            }
        }
    }
    assert_eq!(
        blocks.len(),
        2,
        "reference corpus must supply distinct block sizes"
    );
    for (host, donor) in [(&blocks[0], &blocks[1]), (&blocks[1], &blocks[0])] {
        let mut mor = host.mor.clone();
        let mut gra = host.gra.clone();
        mor.splice_range_coordinated(
            &mut gra,
            0..host.mor.items().len(),
            donor.mor.items().to_vec(),
            donor.gra.relations()[..donor.chunks].to_vec(),
            Some(0),
        )
        .expect("admitted donor replacement");
        assert_eq!(mor.items(), donor.mor.items());
        assert_eq!(
            gra.relations().len(),
            host.gra.relations().len() - host.chunks + donor.chunks
        );
        assert!(
            talkbank_model::alignment::align_mor_to_gra(&mor, &gra)
                .errors
                .is_empty(),
            "size-changing replacement must preserve mor/gra cardinality and index validity"
        );
        for (index, relation) in gra.relations().iter().enumerate() {
            assert_eq!(relation.index, index + 1);
            if index < donor.chunks {
                assert_eq!(relation.head, donor.gra.relations()[index].head);
            } else {
                let original = &host.gra.relations()[index - donor.chunks + host.chunks];
                let expected = if original.head == 0 {
                    0
                } else if original.head <= host.chunks {
                    1
                } else {
                    original.head - host.chunks + donor.chunks
                };
                assert_eq!(relation.head, expected);
                assert_eq!(relation.relation, original.relation);
            }
        }
    }
}

#[test]
fn reference_single_splices_refuse_unrebased_donor_heads_and_wrong_counts() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut head_refusals = 0;
    let mut count_refusals = 0;
    let mut single_admissions = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            let (Some(original_mor), Some(original_gra)) =
                (utterance.mor_tier(), utterance.gra_tier())
            else {
                continue;
            };
            let Some(first) = original_mor.items().first() else {
                continue;
            };
            let chunks = first.count_chunks();
            let Some(relations) = original_gra.relations().get(..chunks) else {
                continue;
            };
            if relations.iter().any(|relation| relation.head > chunks) {
                let mut mor = original_mor.clone();
                let mut gra = original_gra.clone();
                assert!(
                    mor.splice_coordinated(&mut gra, 0, first.clone(), relations.to_vec(), None)
                        .is_err()
                );
                assert_eq!(&mor, original_mor);
                assert_eq!(&gra, original_gra);
                head_refusals += 1;
            } else if talkbank_model::alignment::align_mor_to_gra(original_mor, original_gra)
                .errors
                .is_empty()
            {
                let mut mor = original_mor.clone();
                let mut gra = original_gra.clone();
                mor.splice_coordinated(&mut gra, 0, first.clone(), relations.to_vec(), None)
                    .expect("reference first-item heads are local to the replacement");
                assert!(mor.semantic_eq(original_mor));
                assert!(
                    talkbank_model::alignment::align_mor_to_gra(&mor, &gra)
                        .errors
                        .is_empty()
                );
                single_admissions += 1;
            }
            // An entire tier's relations cannot be supplied for only its first
            // item when it also contains other chunks (including punctuation).
            if original_gra.relations().len() != chunks {
                let mut mor = original_mor.clone();
                let mut gra = original_gra.clone();
                assert!(
                    mor.splice_coordinated(
                        &mut gra,
                        0,
                        first.clone(),
                        original_gra.relations().to_vec(),
                        None
                    )
                    .is_err()
                );
                assert_eq!(&mor, original_mor);
                assert_eq!(&gra, original_gra);
                count_refusals += 1;
            }
        }
    }
    assert!(
        head_refusals > 0,
        "reference heads must witness refusal of an unrebased block"
    );
    assert!(
        count_refusals > 0,
        "reference relations must witness a mismatched donor count"
    );
    assert!(
        single_admissions > 0,
        "reference blocks must witness single-item admission"
    );
}

#[test]
fn spec_short_grammatical_tiers_refuse_single_splices_atomically() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &talkbank_parser_tests::repo_paths::workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("spec corpus");
    let mut refusals = 0;
    for fixture in corpus.fixtures() {
        let errors = talkbank_model::ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(fixture.source(), &errors);
        for utterance in file.utterances() {
            let (Some(original_mor), Some(original_gra)) =
                (utterance.mor_tier(), utterance.gra_tier())
            else {
                continue;
            };
            let mut end = 0;
            for (index, item) in original_mor.items().iter().enumerate() {
                end += item.count_chunks();
                if end <= original_gra.relations().len() {
                    continue;
                }
                let mut mor = original_mor.clone();
                let mut gra = original_gra.clone();
                assert!(
                    mor.splice_coordinated(
                        &mut gra,
                        index,
                        item.clone(),
                        original_gra
                            .relations()
                            .iter()
                            .take(item.count_chunks())
                            .cloned()
                            .collect(),
                        None
                    )
                    .is_err(),
                    "short host gra admitted: {}",
                    fixture.path().display()
                );
                assert_eq!(&mor, original_mor, "refusal changed mor");
                assert_eq!(&gra, original_gra, "refusal changed gra");
                refusals += 1;
            }
        }
    }
    assert!(refusals > 0, "spec data must witness a short host gra tier");
}

#[test]
fn reference_coordinated_splices_refuse_invalid_ranges_atomically() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut witnesses = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            let (Some(original_mor), Some(original_gra)) =
                (utterance.mor_tier(), utterance.gra_tier())
            else {
                continue;
            };
            let Some(first) = original_mor.items().first() else {
                continue;
            };
            let len = original_mor.items().len();
            for range in [std::ops::Range { start: 1, end: 0 }, len..len, 0..len + 1] {
                let mut mor = original_mor.clone();
                let mut gra = original_gra.clone();
                assert!(
                    mor.splice_range_coordinated(
                        &mut gra,
                        range,
                        vec![first.clone()],
                        original_gra
                            .relations()
                            .iter()
                            .take(first.count_chunks())
                            .cloned()
                            .collect(),
                        None
                    )
                    .is_err(),
                    "invalid replacement range admitted: {}",
                    fixture.path().display()
                );
                assert_eq!(&mor, original_mor, "refusal changed mor");
                assert_eq!(&gra, original_gra, "refusal changed gra");
            }
            // Replacing the lexical block retains its payload and applies the
            // documented collapse policy to outside dependents (the terminator).
            let chunks: usize = original_mor
                .items()
                .iter()
                .map(|item| item.count_chunks())
                .sum();
            let relations: Vec<_> = original_gra
                .relations()
                .iter()
                .take(chunks)
                .cloned()
                .collect();
            if relations.len() == chunks && relations.iter().all(|relation| relation.head <= chunks)
            {
                let mut mor = original_mor.clone();
                let mut gra = original_gra.clone();
                mor.splice_range_coordinated(
                    &mut gra,
                    0..len,
                    original_mor.items().to_vec(),
                    relations,
                    Some(0),
                )
                .expect("complete reference block is admitted");
                assert!(mor.semantic_eq(original_mor));
                assert_eq!(gra.relations().len(), original_gra.relations().len());
                for (index, (actual, original)) in gra
                    .relations()
                    .iter()
                    .zip(original_gra.relations())
                    .enumerate()
                {
                    if index < chunks {
                        assert_eq!(actual.index, index + 1);
                        assert_eq!(actual.head, original.head);
                        assert_eq!(
                            actual.relation.as_str(),
                            if original.head == 0 {
                                "ROOT"
                            } else {
                                original.relation.as_str()
                            }
                        );
                    } else {
                        assert_eq!(actual.index, original.index);
                        assert_eq!(
                            actual.head,
                            if original.head > 0 && original.head <= chunks {
                                1
                            } else {
                                original.head
                            }
                        );
                        assert_eq!(actual.relation, original.relation);
                    }
                }
                witnesses += 1;
            }
        }
    }
    assert!(
        witnesses > 0,
        "reference data must witness successful coordinated replacement"
    );
}
