//! Public construction and wire contracts using parsed reference payloads.
//! Builders produce ordinary models, not validation or source-binding proofs.

use talkbank_model::model::{MainTier, SemanticEq, TierContent, TierContentItems, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::test_error::strict_parse;

#[path = "tier_decoration_corpus.rs"]
mod decoration_contracts;

/// Exercise public text adapters on a parsed lexical value. Reconstructing an
/// open token preserves text, but does not issue a validation/source proof.
fn assert_lexical_adapters<T>(original: &T, text: &str)
where
    T: AsRef<str>
        + std::ops::Deref<Target = str>
        + std::fmt::Display
        + std::fmt::Debug
        + PartialEq
        + WriteChat
        + From<String>
        + for<'a> From<&'a str>
        + serde::Serialize
        + serde::de::DeserializeOwned,
{
    assert_eq!(original.as_ref(), text);
    assert_eq!(&**original, text);
    assert_eq!(original.to_string(), text);
    assert_eq!(original.to_chat_string(), text);
    for rebuilt in [T::from(text), T::from(text.to_owned())] {
        assert_eq!(&rebuilt, original);
        let wire = serde_json::to_value(&rebuilt).expect("lexical JSON");
        assert_eq!(wire, serde_json::Value::String(text.to_owned()));
        let decoded: T = serde_json::from_value(wire).expect("lexical JSON admission");
        assert_eq!(&decoded, original);
    }
}

/// Consumer views preserve authored POS refinements and feature wire spelling;
/// converting these open lexical values does not certify CHAT validity.
#[test]
fn reference_morphology_views_preserve_pos_and_feature_identity() {
    use talkbank_model::model::{MorFeature, MorWord};
    use talkbank_parser_tests::repo_paths::workspace_root;

    let parser = TreeSitterParser::new().expect("parser");
    let mut refined = 0;
    let mut plain = 0;
    let mut feature_spellings = std::collections::BTreeSet::new();
    for path in [
        "corpus/reference/languages/jpn-mlue-postcode.cha",
        "corpus/reference/edge-cases/clitics-and-compounds.cha",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E711_postclitic_features_2.cha",
    ] {
        let source = std::fs::read_to_string(workspace_root().join(path))
            .expect("authored morphology control");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("control parses");
        for utterance in file.utterances() {
            let mor = utterance.mor_tier().expect("authored morphology");
            for item in mor.items() {
                for word in std::iter::once(&item.main).chain(item.post_clitics.iter()) {
                    // Secondary-analysis rollback deliberately demotes only the
                    // selected word's analysis. It is not CHAT admission.
                    let mut fallback = word.clone();
                    fallback.reset_to_l2_placeholder();
                    let constructed = MorWord::l2_placeholder();
                    assert_eq!(fallback, constructed);
                    assert_eq!(fallback.pos.as_str(), "L2");
                    assert_eq!(fallback.lemma.as_str(), "xxx");
                    assert!(fallback.features.is_empty());
                    assert_eq!(fallback.to_chat_string(), "L2|xxx");
                    let pos = &word.pos;
                    if path.ends_with("jpn-mlue-postcode.cha") {
                        assert_eq!(pos.as_str(), "n:let");
                        assert_eq!(pos.category(), "n");
                        assert!(pos.subcategories().eq(["let"]));
                        refined += 1;
                    } else {
                        assert_eq!(pos.category(), pos.as_str());
                        assert!(pos.subcategories().next().is_none());
                        plain += 1;
                    }
                    for feature in &word.features {
                        let text = feature.to_chat_string();
                        feature_spellings.insert(text.clone());
                        for rebuilt in [MorFeature::from(text.as_str()), MorFeature::from(text)] {
                            assert!(rebuilt.semantic_eq(feature));
                            assert_eq!(rebuilt.key(), feature.key());
                            assert_eq!(rebuilt.value(), feature.value());
                            assert_eq!(
                                serde_json::to_value(&rebuilt).expect("feature wire"),
                                serde_json::to_value(feature).expect("original feature wire")
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(
            file.to_chat_string(),
            source,
            "views cannot rewrite the control"
        );
        if path.ends_with("E711_postclitic_features_2.cha") {
            let mor = file
                .utterances()
                .next()
                .expect("utterance")
                .mor_tier()
                .expect("morphology");
            let item = &mor.items()[0];
            let case = &item.main.features[0];
            let tense = &item.post_clitics[0].features[0];
            assert_eq!(case.key(), Some("Case"));
            assert_eq!(tense.key(), Some("Tense"));
            assert!(
                !case.semantic_eq(tense),
                "different authored keys are not equivalent"
            );
            assert!(!tense.semantic_eq(case));
        }
    }
    assert_eq!(refined, 8, "both Japanese utterances are inspected");
    assert!(plain > 0);
    assert_eq!(
        feature_spellings,
        ["Case=Nom", "Past", "Pres", "S3", "Tense=Pres"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

/// Removing the final scoped annotation must leave the annotated type, rather
/// than admitting an empty wrapper through an editor or JSON import.
#[test]
fn reference_scoped_annotations_require_nonempty_constructor_and_wire_admission() {
    use talkbank_model::model::{AnnotatedContentAnnotations, UtteranceContent};
    use talkbank_parser_tests::repo_paths::workspace_root;

    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/annotation/groups-regular.cha"),
    )
    .expect("annotated group reference");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let mut witnessed = 0;
    for utterance in file.utterances() {
        for item in &utterance.main.content.content {
            if let UtteranceContent::AnnotatedGroup(group) = item {
                let original = &group.scoped_annotations;
                let owned: Vec<_> = original.clone().into_iter().collect();
                let admitted = AnnotatedContentAnnotations::try_from(owned)
                    .expect("source-backed nonempty annotations");
                assert_eq!(
                    &admitted, original,
                    "ordered payload survives ownership transfer"
                );
                let wire = serde_json::to_value(&admitted).expect("annotations JSON");
                let restored: AnnotatedContentAnnotations =
                    serde_json::from_value(wire).expect("nonempty annotation import");
                assert!(restored.semantic_eq(original));
                witnessed += 1;
            }
        }
    }
    assert!(witnessed > 0, "actual annotated group required");
    let error = AnnotatedContentAnnotations::try_from(Vec::new())
        .expect_err("last annotation deletion cannot retain an annotated wrapper");
    assert_eq!(
        error.to_string(),
        "an annotated construct must carry at least one scoped annotation"
    );
    for invalid in [
        serde_json::json!([]),
        serde_json::json!(null),
        serde_json::json!({}),
    ] {
        assert!(serde_json::from_value::<AnnotatedContentAnnotations>(invalid).is_err());
    }
}

/// Header text adapters preserve each parsed field without promising semantic
/// admission. Concrete type witnesses keep shared macro coverage nonvacuous.
#[test]
fn reference_header_text_adapters_preserve_authored_payloads() {
    use talkbank_model::model::{Header, Line};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut witnessed = std::collections::BTreeSet::new();
    macro_rules! check {
        ($kind:ident, $value:expr) => {{
            let value = $value;
            let text = value.as_str();
            assert_lexical_adapters(value, text);
            for rebuilt in [
                talkbank_model::model::$kind::new(text),
                talkbank_model::model::$kind::new(text.to_owned()),
            ] {
                assert_eq!(&rebuilt, value);
                assert_eq!(rebuilt.to_chat_string(), text);
            }
            for invalid in [
                serde_json::json!(null),
                serde_json::json!(true),
                serde_json::json!(1),
                serde_json::json!([]),
                serde_json::json!({}),
            ] {
                assert!(serde_json::from_value::<talkbank_model::model::$kind>(invalid).is_err());
            }
            witnessed.insert(stringify!($kind));
        }};
    }
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        let before = file.to_chat_string();
        for line in &file.lines {
            let Line::Header { header, .. } = line else {
                continue;
            };
            match header.as_ref() {
                Header::Participants { entries } => {
                    for entry in entries {
                        if let Some(name) = &entry.name {
                            check!(ParticipantName, name);
                        }
                        check!(ParticipantRole, &entry.role);
                    }
                }
                Header::ID(id) => {
                    check!(CorpusName, &id.corpus);
                    check!(ParticipantRole, &id.role);
                    if let Some(group) = &id.group {
                        check!(GroupName, group);
                    }
                    if let Some(education) = &id.education {
                        check!(EducationDescription, education);
                    }
                    if let Some(custom) = &id.custom_field {
                        check!(CustomIdField, custom);
                    }
                }
                Header::Pid { pid } => check!(PidValue, pid),
                Header::Situation { text } => check!(SituationDescription, text),
                Header::BeginGem { label: Some(label) }
                | Header::EndGem { label: Some(label) }
                | Header::LazyGem { label: Some(label) } => check!(GemLabel, label),
                Header::TapeLocation { location } => check!(TapeLocationDescription, location),
                Header::Location { location } => check!(LocationDescription, location),
                Header::RoomLayout { layout } => check!(RoomLayoutDescription, layout),
                Header::Birthplace { place, .. } => check!(BirthplaceDescription, place),
                Header::Transcriber { transcriber } => check!(TranscriberName, transcriber),
                Header::Warning { text } => check!(WarningText, text),
                Header::Activities { activities } => check!(ActivitiesDescription, activities),
                Header::Bck { bck } => check!(BackgroundDescription, bck),
                Header::Page { page } => check!(PageNumber, page),
                Header::Videos { videos } => check!(VideoSpec, videos),
                Header::T { text } => check!(TDescription, text),
                Header::Font { font } => check!(FontSpec, font),
                Header::Window { geometry } => check!(WindowGeometry, geometry),
                Header::ColorWords { colors } => check!(ColorWordList, colors),
                _ => {} // Structured payloads have separate admission contracts.
            }
        }
        assert_eq!(
            file.to_chat_string(),
            before,
            "adapters cannot mutate the source model"
        );
    }
    assert_eq!(
        witnessed,
        [
            "ParticipantName",
            "ParticipantRole",
            "CorpusName",
            "GroupName",
            "EducationDescription",
            "CustomIdField",
            "PidValue",
            "SituationDescription",
            "GemLabel",
            "TapeLocationDescription",
            "LocationDescription",
            "RoomLayoutDescription",
            "BirthplaceDescription",
            "TranscriberName",
            "WarningText",
            "ActivitiesDescription",
            "BackgroundDescription",
            "PageNumber",
            "VideoSpec",
            "TDescription",
            "FontSpec",
            "WindowGeometry",
            "ColorWordList",
        ]
        .into_iter()
        .collect()
    );
}

/// The SES reference control must remain fully admitted, not merely parseable
/// through a generic header token or represented by a skipped test glob.
#[test]
fn reference_ses_vocabulary_preserves_all_authored_fields() {
    use talkbank_model::model::{Header, Line};
    use talkbank_model::{NullErrorSink, TranscriptName};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/core/headers-ses-vocabulary.cha"
    ));
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(source)).expect("clean reference parsing");
    let proof = file
        .validate_into(&NullErrorSink, TranscriptName::Anonymous)
        .expect("the whole reference model must validate");
    let values: Vec<_> = proof
        .document()
        .lines
        .iter()
        .filter_map(|line| {
            let Line::Header { header, .. } = line else {
                return None;
            };
            let Header::ID(id) = header.as_ref() else {
                return None;
            };
            Some(
                id.ses
                    .as_ref()
                    .expect("authored SES field")
                    .to_chat_string(),
            )
        })
        .collect();
    assert_eq!(
        values,
        [
            "White,UC",
            "Black,MC",
            "Asian,WC",
            "Latino,LI",
            "Pacific",
            "Native",
            "Multiple",
            "Unknown"
        ]
    );
    assert_eq!(proof.to_chat_string(), source);
}

/// Structured header imports preserve field order and refuse wrong JSON types
/// instead of stringifying them into plausible CHAT metadata.
#[test]
fn reference_types_header_preserves_fields_and_refuses_nonstring_json() {
    use serde_json::json;
    use talkbank_model::model::{Header, Line, TypesHeader};
    use talkbank_parser_tests::repo_paths::workspace_root;

    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/headers-time-and-types.cha"),
    )
    .expect("header reference");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let original = file
        .lines
        .iter()
        .find_map(|line| {
            if let Line::Header { header, .. } = line
                && let Header::Types(types) = header.as_ref()
            {
                return Some(types);
            }
            None
        })
        .expect("reference Types header");
    assert_eq!(
        [
            original.design.to_string(),
            original.activity.to_string(),
            original.group.to_string()
        ],
        ["long", "toyplay", "TD"],
    );
    let rebuilt = TypesHeader::new(
        original.design.as_str(),
        original.activity.as_str(),
        original.group.as_str(),
    );
    assert_eq!(&rebuilt, original);
    assert_eq!(rebuilt.to_chat_string(), "@Types:\tlong, toyplay, TD");
    let wire = serde_json::to_value(&rebuilt).expect("header JSON");
    assert_eq!(
        wire,
        json!({"design": "long", "activity": "toyplay", "group": "TD"})
    );
    assert_eq!(
        serde_json::from_value::<TypesHeader>(wire.clone()).expect("header JSON roundtrip"),
        rebuilt
    );
    for field in ["design", "activity", "group"] {
        for invalid in [json!(null), json!(true), json!(1), json!([]), json!({})] {
            let mut corrupted = wire.clone();
            corrupted[field] = invalid;
            assert!(
                serde_json::from_value::<TypesHeader>(corrupted).is_err(),
                "{field} must remain a string at the wire boundary"
            );
        }
    }
}

#[test]
fn reference_tiers_preserve_payload_through_public_construction_and_json() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut witnessed = [false; 8];
    let mut morphology_witnesses = 0;
    let mut phonology_witnesses = [0usize; 2];
    let mut phonology_shapes = [0usize; 2];
    let mut appender_witnesses = [false; 4];
    let mut lexical_witnesses = [0usize; 5];
    let mut sign_shapes = [0usize; 2];
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            use talkbank_model::alignment::helpers::{ContentItem, walk_content};
            walk_content(&utterance.main.content.content, None, &mut |item| {
                let (label, family) = match item {
                    ContentItem::NonvocalBegin(value) => (&value.label, 2),
                    ContentItem::NonvocalEnd(value) => (&value.label, 3),
                    ContentItem::NonvocalSimple(value) => (&value.label, 4),
                    _ => return, // This adapter contract selects nonvocal labels only.
                };
                assert_lexical_adapters(label, label.as_str());
                // Importers may construct from borrowed or owned payloads;
                // neither path may normalize the authored nonvocal label.
                for rebuilt in [
                    talkbank_model::model::NonvocalLabel::new(label.as_str()),
                    talkbank_model::model::NonvocalLabel::new(label.as_str().to_owned()),
                ] {
                    assert_eq!(&rebuilt, label);
                    assert_eq!(rebuilt.to_chat_string(), label.as_str());
                }
                lexical_witnesses[family] += 1;
            });
            // Reassemble parsed payloads in source order. This is ordinary
            // model construction, not transfer of a validation certificate.
            let assembled = utterance.dependent_tiers.iter().fold(
                talkbank_model::model::Utterance::new(utterance.main.clone()),
                |draft, dependent| draft.add_dependent_tier(dependent.tier.clone()),
            );
            assert_eq!(
                assembled.dependent_tiers.len(),
                utterance.dependent_tiers.len()
            );
            for (actual, original) in assembled
                .dependent_tiers
                .iter()
                .zip(&utterance.dependent_tiers)
            {
                assert!(actual.tier.semantic_eq(&original.tier));
                assert_eq!(actual.tier.to_chat_string(), original.tier.to_chat_string());
            }
            assert!(assembled.main.semantic_eq(&utterance.main));

            // The four convenience appenders are projections, deliberately
            // excluding other tier families. Check call order explicitly.
            let mut projected = talkbank_model::model::Utterance::new(utterance.main.clone());
            let mut expected_tiers = Vec::new();
            if let Some(tier) = utterance.mor_tier() {
                projected = projected.with_mor(tier.clone());
                expected_tiers.push(talkbank_model::model::DependentTier::Mor(tier.clone()));
                appender_witnesses[0] = true;
            }
            if let Some(tier) = utterance.gra_tier() {
                let labels: std::collections::HashSet<_> = tier
                    .relations()
                    .iter()
                    .map(|relation| relation.relation.clone())
                    .collect();
                for relation in tier.relations() {
                    let label = &relation.relation;
                    assert_lexical_adapters(label, label.as_str());
                    let borrowed: &str = std::borrow::Borrow::borrow(label);
                    assert_eq!(borrowed, label.as_str());
                    assert!(
                        labels.contains(label.as_str()),
                        "borrowed lookup preserves label identity"
                    );
                    lexical_witnesses[0] += 1;
                }
                projected = projected.with_gra(tier.clone());
                expected_tiers.push(talkbank_model::model::DependentTier::Gra(tier.clone()));
                appender_witnesses[1] = true;
            }
            if let Some(tier) = utterance.sin_tier() {
                use talkbank_model::model::{SinItem, SinTier};
                let structured = SinTier::new(tier.items.to_vec()).with_span(tier.span);
                assert_eq!(
                    &structured, tier,
                    "structured construction preserves groups and provenance"
                );
                assert_eq!(tier.is_empty(), tier.items.is_empty());
                assert_eq!(tier.is_empty(), tier.is_empty());
                let flat: Option<Vec<String>> = tier
                    .items
                    .iter()
                    .map(|item| match item {
                        SinItem::Token(token) => Some(token.as_ref().to_owned()),
                        SinItem::SinGroup(_) => None,
                    })
                    .collect();
                match flat {
                    Some(mut tokens) => {
                        let rebuilt = SinTier::from_tokens(tokens.clone())
                            .expect("nonempty parsed tokens")
                            .with_span(tier.span);
                        assert_eq!(&rebuilt, tier, "flat admission preserves token order");
                        assert_eq!(rebuilt.to_chat_string(), tier.to_chat_string());
                        // A failed imported token rejects the whole construction;
                        // it must not silently shorten an aligned tier.
                        tokens.push(String::new());
                        assert!(SinTier::from_tokens(tokens).is_err());
                        sign_shapes[0] += 1;
                    }
                    None => sign_shapes[1] += 1,
                }
                projected = projected.with_sin(tier.clone());
                expected_tiers.push(talkbank_model::model::DependentTier::Sin(tier.clone()));
                appender_witnesses[2] = true;
            }
            for dependent in &utterance.dependent_tiers {
                if let talkbank_model::model::DependentTier::Com(tier) = &dependent.tier {
                    projected = projected.with_com(tier.clone());
                    expected_tiers.push(talkbank_model::model::DependentTier::Com(tier.clone()));
                    appender_witnesses[3] = true;
                }
            }
            assert_eq!(projected.dependent_tiers.len(), expected_tiers.len());
            for (actual, expected) in projected.dependent_tiers.iter().zip(expected_tiers) {
                assert_eq!(
                    actual.tier, expected,
                    "append order and payload are retained"
                );
            }
            for dependent in &utterance.dependent_tiers {
                use talkbank_model::model::{DependentTier, PhoItem, PhoTier};
                let (original, rebuilt, family) = match &dependent.tier {
                    DependentTier::Pho(tier) => (tier, PhoTier::new_pho(tier.items.to_vec()), 0),
                    DependentTier::Mod(tier) => (tier, PhoTier::new_mod(tier.items.to_vec()), 1),
                    _ => continue, // This contract covers only phonological tiers.
                };
                assert!(rebuilt.span.is_dummy());
                assert_eq!(rebuilt.is_pho(), family == 0);
                assert_eq!(rebuilt.is_mod(), family == 1);
                assert_eq!(rebuilt.len(), original.items.len());
                assert_eq!(rebuilt.is_empty(), original.items.is_empty());
                assert_eq!(rebuilt.to_chat(), original.to_chat());
                assert_eq!(rebuilt.to_content(), original.to_content());
                for item in original.items.iter() {
                    match item {
                        PhoItem::Word(word) => {
                            assert_lexical_adapters(word, word.as_str());
                            lexical_witnesses[1] += 1;
                        }
                        PhoItem::Group(words) => {
                            for word in words.iter() {
                                assert_lexical_adapters(word, word.as_str());
                                lexical_witnesses[1] += 1;
                            }
                        }
                    }
                }
                let tokens: Option<Vec<String>> = original
                    .items
                    .iter()
                    .map(|item| match item {
                        PhoItem::Word(word) => Some(word.as_str().to_owned()),
                        PhoItem::Group(_) => None,
                    })
                    .collect();
                match tokens {
                    Some(tokens) => {
                        let token_tier = PhoTier::from_tokens(original.tier_type, tokens);
                        assert!(token_tier.span.is_dummy());
                        assert!(token_tier.semantic_eq(original));
                        assert_eq!(token_tier.to_chat(), original.to_chat());
                        phonology_shapes[0] += 1;
                    }
                    None => {
                        // A group has no lossless flat-token projection.
                        // Retain the structured constructor result above.
                        phonology_shapes[1] += 1;
                    }
                }
                let bound = rebuilt.with_span(original.span);
                assert_eq!(&bound, original);
                let decoded: PhoTier =
                    serde_json::from_value(serde_json::to_value(&bound).expect("phonology JSON"))
                        .expect("reconstruct phonology");
                assert!(decoded.span.is_dummy());
                assert!(decoded.semantic_eq(original));
                assert_eq!(decoded.to_chat(), original.to_chat());
                phonology_witnesses[family] += 1;
            }
            if let Some(mor) = utterance.mor_tier() {
                let rebuilt = talkbank_model::model::MorTier::new_mor(
                    mor.clone().into_items(),
                    mor.terminator.clone(),
                );
                assert!(
                    rebuilt.span.is_dummy(),
                    "construction does not invent provenance"
                );
                assert!(rebuilt.semantic_eq(mor));
                assert_eq!(rebuilt.to_chat(), mor.to_chat());
                assert_eq!(rebuilt.to_content(), mor.to_content());
                let bound = rebuilt.with_span(mor.span);
                assert_eq!(
                    &bound, mor,
                    "unchanged morphology retains its original span"
                );
                let decoded: talkbank_model::model::MorTier =
                    serde_json::from_value(serde_json::to_value(&bound).expect("morphology JSON"))
                        .expect("reconstruct morphology");
                assert!(decoded.span.is_dummy());
                assert!(decoded.semantic_eq(mor));
                assert_eq!(decoded.to_chat(), mor.to_chat());
                morphology_witnesses += 1;
            }
            let expected = &utterance.main;
            let payload = &expected.content;
            if fixture.path().ends_with("content/language-switching.cha")
                && !payload.linkers.is_empty()
                && payload.language_code.is_some()
            {
                witnessed[7] = true;
                assert_eq!(
                    payload.to_content_string(),
                    "+^ [- zho] 我 要 吃 .",
                    "authored linker/precode interaction retains its separating space"
                );
            }
            // Editor staging is an ordinary model, not a validation proof.
            // Moving typed collections must preserve order and every item.
            let items: Vec<_> = payload.content.clone().into_iter().collect();
            let split = items.len() / 2;
            let mut joined = TierContentItems::new(items[..split].to_vec());
            joined.append(TierContentItems::new(items[split..].to_vec()));
            assert_eq!(joined, payload.content);
            let mut undo = joined.clone();
            for expected_item in items.iter().rev() {
                assert_eq!(undo.pop().as_ref(), Some(expected_item));
            }
            assert!(undo.is_empty());
            assert_eq!(
                undo.pop(),
                None,
                "exhausted undo stack cannot manufacture an item"
            );
            let direct = TierContent::new(items);
            let mut content = TierContent::default();
            assert_eq!(
                content.to_content_string(),
                "",
                "empty editing draft has no invented text"
            );
            content.content = joined;
            assert_eq!(
                content, direct,
                "draft population agrees with direct construction"
            );
            content = content
                .with_linkers(payload.linkers.clone().into_iter().collect())
                .with_postcodes(payload.postcodes.clone().into_iter().collect());
            let mut main = MainTier::new(
                expected.speaker.clone(),
                payload.content.to_vec(),
                payload.terminator.clone(),
            );
            assert!(main.span.is_dummy() && main.speaker_span.is_dummy());
            assert_eq!(content.content_span, None);
            assert_eq!(content.language_code_span, None);
            for linker in &payload.linkers {
                main = main.with_linker(*linker);
                witnessed[0] = true;
            }
            for postcode in &payload.postcodes {
                main = main.with_postcode(postcode.clone());
                witnessed[1] = true;
            }
            if let Some(code) = &payload.language_code {
                content = content.with_language_code(code.clone());
                main = main.with_language_code(code.clone());
                witnessed[2] = true;
            }
            if let Some(terminator) = &payload.terminator {
                content = content.with_terminator(terminator.clone());
                witnessed[3] = true;
            }
            if let Some(bullet) = &payload.bullet {
                content = content.with_bullet(bullet.clone());
                main = main.with_bullet(bullet.clone());
                witnessed[4] = true;
            }
            // Copy only provenance belonging to this unchanged parsed payload.
            if let Some(span) = payload.content_span {
                content = content.with_content_span(span);
                main = main.with_content_span(span);
                witnessed[5] = true;
            }
            if let Some(span) = payload.language_code_span {
                content = content.with_language_code_span(span);
                main = main.with_language_code_span(span);
                witnessed[6] = true;
            }
            main = main
                .with_span(expected.span)
                .with_speaker_span(expected.speaker_span)
                .with_separator(expected.separator);
            assert_eq!(&content, payload, "{}", fixture.path().display());
            assert_eq!(&main, expected, "{}", fixture.path().display());
            assert_eq!(content.to_content_string(), payload.to_content_string());
            assert_eq!(main.to_chat(), expected.to_chat_string());

            // Whole-list replacement and incremental construction must agree.
            let replaced = main
                .with_linkers(payload.linkers.to_vec())
                .with_postcodes(payload.postcodes.to_vec());
            assert_eq!(&replaced, expected, "{}", fixture.path().display());
            let reconstructed: MainTier =
                serde_json::from_str(&serde_json::to_string(&replaced).expect("serialize model"))
                    .expect("reconstruct model");
            assert!(
                reconstructed.semantic_eq(expected),
                "{}",
                fixture.path().display()
            );
            assert_eq!(reconstructed.to_chat(), expected.to_chat_string());
            assert!(reconstructed.span.is_dummy() && reconstructed.speaker_span.is_dummy());
            assert_eq!(reconstructed.content.content_span, None);
            assert_eq!(reconstructed.content.language_code_span, None);
        }
    }
    assert_eq!(
        witnessed, [true; 8],
        "reference witnesses: linker, postcode, language, terminator, bullet, content span, language span, linker/precode interaction"
    );
    assert_eq!(
        appender_witnesses, [true; 4],
        "reference witnesses for each public appender"
    );
    assert!(
        morphology_witnesses > 0,
        "reference morphology must exercise construction"
    );
    assert!(
        sign_shapes.into_iter().all(|count| count > 0),
        "flat and grouped sign tiers need reference witnesses: {sign_shapes:?}"
    );
    assert!(
        lexical_witnesses.into_iter().all(|count| count > 0),
        "relation, phonology and all three nonvocal forms need lexical witnesses: {lexical_witnesses:?}"
    );
    assert!(
        phonology_witnesses.into_iter().all(|count| count > 0),
        "observed and model phonology need reference witnesses: {phonology_witnesses:?}"
    );
    assert!(
        phonology_shapes.into_iter().all(|count| count > 0),
        "plain and grouped phonology need reference witnesses: {phonology_shapes:?}"
    );
}
