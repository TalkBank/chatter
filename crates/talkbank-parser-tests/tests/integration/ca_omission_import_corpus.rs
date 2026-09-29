//! Spec-backed editable-model admission, not new parser-reachability claims.

use super::*;
use talkbank_model::alignment::helpers::{WordItem, WordItemMut, walk_words, walk_words_mut};
use talkbank_model::model::{
    Line, ParseHealthState, WordCategory, WordCompoundMarker, WordContent, WordShortening,
};
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{ErrorCode, ErrorCollector, RuleSelection};

/// An imported draft is not proof of valid CA omission structure. Exercise
/// deletion and typed component substitution without reparsing its CHAT output.
#[test]
fn ca_omission_imports_require_checked_structure_without_silent_normalization() {
    let parser = TreeSitterParser::new().expect("parser");
    let read = |path: &str| {
        let source =
            std::fs::read_to_string(workspace_root().join(path)).expect("canonical source");
        strict_parse(parser.parse_chat_file(&source)).expect("canonical syntax")
    };
    let seed = read("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E212_3.cha");
    let shortening_source =
        read("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E212_2.cha");
    let compound_source =
        read("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E232_2.cha");
    let mut shortening = None;
    let mut compound = None;
    for file in [&shortening_source, &compound_source] {
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                if let WordItem::Word(word) = item {
                    for piece in word.content() {
                        match piece {
                            WordContent::Shortening(value) => shortening = Some(value.clone()),
                            WordContent::CompoundMarker(value) => compound = Some(*value),
                            _ => {}
                        }
                    }
                }
            });
        }
    }
    let shortening = shortening.expect("authored shortening witness");
    let compound = compound.expect("authored compound witness");

    // The variant owns the edit and its expected admission outcome; there is
    // no independent boolean that could disagree with the selected operation.
    enum Edit {
        Preserve,
        Delete,
        ShorteningOnly(WordShortening),
        AppendShortening(WordShortening),
        AppendCompound(WordCompoundMarker),
    }
    for edit in [
        Edit::Preserve,
        Edit::Delete,
        Edit::ShorteningOnly(shortening.clone()),
        Edit::AppendShortening(shortening),
        Edit::AppendCompound(compound),
    ] {
        let mut draft: ChatFile =
            serde_json::from_value(serde_json::to_value(&seed).expect("canonical model wire"))
                .expect("imported draft");
        let mut witnesses = 0;
        for line in &mut draft.lines {
            if let Line::Utterance(utterance) = line {
                assert!(utterance.parse_health().is_unknown());
                walk_words_mut(
                    utterance.main.content.content.as_mut_slice(),
                    None,
                    &mut |item| {
                        if let WordItemMut::Word(word) = item
                            && word.category == Some(WordCategory::CAOmission)
                        {
                            witnesses += 1;
                            let content = match &edit {
                                Edit::Preserve => word.content().to_vec(),
                                Edit::Delete => Vec::new(),
                                Edit::ShorteningOnly(piece) => {
                                    vec![WordContent::Shortening(piece.clone())]
                                }
                                Edit::AppendShortening(piece) => {
                                    let mut content = word.content().to_vec();
                                    content.push(WordContent::Shortening(piece.clone()));
                                    content
                                }
                                Edit::AppendCompound(piece) => {
                                    let mut content = word.content().to_vec();
                                    content.push(WordContent::CompoundMarker(*piece));
                                    content
                                }
                            };
                            *word = word.clone().with_content(content);
                        }
                    },
                );
            }
        }
        assert_eq!(witnesses, 1, "one spec-authored omission must be edited");
        // Roundtrip the *edited* structure through JSON, not CHAT. Wire
        // acceptance remains draft admission and cannot restore parser proof.
        let wire = serde_json::to_string(&draft).expect("edited wire");
        let imported: ChatFile = serde_json::from_str(&wire).expect("editable model import");
        assert!(draft.semantic_eq(&imported));
        assert!(imported.utterances().all(|u| u.parse_health().is_unknown()));
        for alignment in [
            AlignmentValidation::Structure,
            AlignmentValidation::IncludeTierAlignment,
        ] {
            let errors = ErrorCollector::new();
            let result = imported.clone().validate_construction_with_policy(
                ValidationPolicy::new(RuleSelection::new(), alignment),
                &errors,
                TranscriptName::Anonymous,
            );
            match edit {
                Edit::Preserve => {
                    let admitted = result.expect("legal spec control must be admitted");
                    assert!(admitted.document().semantic_eq(&imported));
                    assert!(
                        admitted
                            .document()
                            .utterances()
                            .all(|u| u.parse_health() == ParseHealthState::Constructed)
                    );
                    assert!(
                        !errors
                            .to_vec()
                            .iter()
                            .any(|e| e.code == ErrorCode::InvalidWordFormat)
                    );
                }
                Edit::Delete
                | Edit::ShorteningOnly(_)
                | Edit::AppendShortening(_)
                | Edit::AppendCompound(_) => {
                    let refused = result.expect_err("invalid omission cannot establish validity");
                    assert!(!refused.has_incomplete_parse());
                    assert!(!refused.has_internal_failure());
                    assert!(
                        errors
                            .to_vec()
                            .iter()
                            .any(|e| e.code == ErrorCode::InvalidWordFormat)
                    );
                    assert!(
                        refused.document().semantic_eq(&imported),
                        "do not silently normalize"
                    );
                    assert!(
                        refused
                            .document()
                            .utterances()
                            .all(|u| u.parse_health().is_unknown())
                    );
                }
            }
        }
    }
}
