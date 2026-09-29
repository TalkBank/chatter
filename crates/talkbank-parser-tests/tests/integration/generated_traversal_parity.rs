// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Parity tests: validate the generated `extract_*` free functions against
//! real CHAT data.
//!
//! These tests parse the 74-file reference corpus with tree-sitter and exercise
//! the generated extraction functions on every matching node. They verify:
//! 1. Extraction doesn't panic on any real-world CST
//! 2. Required children are Present in well-formed CHAT
//! 3. Speaker text, tier body, headers extract correctly

use std::collections::BTreeMap;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::classify;

use talkbank_parser_tests::generated_traversal::*;

/// Read the UTF-8 text of a leaf wrapper's raw node.
///
/// The OLD backend's leaf wrappers carried an inherent `.text(source)`
/// convenience method; the NEW backend's wrappers do not (verified: zero
/// occurrences of that method in `generated_traversal.rs`), so callers
/// read the wrapped raw node and decode directly, matching the idiom used
/// throughout the already-migrated production parser (e.g.
/// `node.utf8_text(source.as_bytes())` in `tree_parsing/main_tier/structure/
/// contents.rs`).
fn node_text<'s>(node: tree_sitter::Node, source: &'s str) -> &'s str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

/// Walk a tree-sitter tree, calling `callback` on every node.
fn walk_all<'tree, F>(node: tree_sitter::Node<'tree>, callback: &mut F)
where
    F: FnMut(tree_sitter::Node<'tree>),
{
    callback(node);
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_all(cursor.node(), callback);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn parse_chat(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");
    parser.parse(source, None).expect("parse")
}

// ---------------------------------------------------------------------------
// Test 1: Simple utterance, verify every field of main_tier
// ---------------------------------------------------------------------------

#[test]
fn test_main_tier_all_fields_present() {
    let source = "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n*CHI:\thello world .\n@End\n";
    let tree = parse_chat(source);
    let mut found = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "main_tier" {
            let c =
                extract_main_tier(classify::<MainTierNode>(node)).expect("producer reconstruction");
            assert!(
                matches!(c.child_0.slot(), NodeSlot::Present(_)),
                "star: {:?}",
                c.child_0.slot()
            );
            assert!(
                matches!(c.speaker.slot(), NodeSlot::Present(_)),
                "speaker: {:?}",
                c.speaker.slot()
            );
            assert!(
                matches!(c.child_2.slot(), NodeSlot::Present(_)),
                "colon: {:?}",
                c.child_2.slot()
            );
            assert!(
                matches!(c.child_3.slot(), NodeSlot::Present(_)),
                "tab: {:?}",
                c.child_3.slot()
            );
            assert!(
                c.child_4.slot().is_none(),
                "sep_trailing_space (clean tab, no illegal trailing space): {:?}",
                c.child_4.slot()
            );
            assert!(
                matches!(c.child_5.slot(), NodeSlot::Present(_)),
                "tier_body: {:?}",
                c.child_5.slot()
            );

            if let NodeSlot::Present(spk) = c.speaker.slot() {
                assert_eq!(node_text(spk.raw_node(), source), "CHI");
            }
            found = true;
        }
    });
    assert!(found);
}

// ---------------------------------------------------------------------------
// Test 2: Header extraction
// ---------------------------------------------------------------------------

#[test]
fn test_participants_header() {
    let source = "@UTF8\n@Begin\n@Participants:\tCHI Target_Child, MOT Mother\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut found = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "participants_header" {
            let c = extract_participants_header(classify::<ParticipantsHeaderNode>(node))
                .expect("producer reconstruction");
            // Should have all required children present
            assert!(
                matches!(c.child_0.slot(), NodeSlot::Present(_)),
                "prefix: {:?}",
                c.child_0.slot()
            );
            assert!(
                matches!(c.child_1.slot(), NodeSlot::Present(_)),
                "header_sep: {:?}",
                c.child_1.slot()
            );
            assert!(
                matches!(c.child_2.slot(), NodeSlot::Present(_)),
                "contents: {:?}",
                c.child_2.slot()
            );
            found = true;
        }
    });
    assert!(found);
}

#[test]
fn test_date_header() {
    let source =
        "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n@Date:\t01-JAN-2000\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut found = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "date_header" {
            let c = extract_date_header(classify::<DateHeaderNode>(node))
                .expect("producer reconstruction");
            assert!(
                matches!(c.child_0.slot(), NodeSlot::Present(_)),
                "date_prefix: {:?}",
                c.child_0.slot()
            );
            assert!(
                matches!(c.child_1.slot(), NodeSlot::Present(_)),
                "header_sep: {:?}",
                c.child_1.slot()
            );
            assert!(
                matches!(c.child_2.slot(), NodeSlot::Present(_)),
                "date_contents: {:?}",
                c.child_2.slot()
            );

            if let NodeSlot::Present(date) = c.child_2.slot() {
                assert_eq!(node_text(date.raw_node(), source), "01-JAN-2000");
            }
            found = true;
        }
    });
    assert!(found);
}

// ---------------------------------------------------------------------------
// Test 3: Document structure
// ---------------------------------------------------------------------------

#[test]
fn test_full_document_extraction() {
    let source = "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);

    // The root is source_file (CHOICE), its first child is full_document (SEQ)
    let root = tree.root_node();
    assert_eq!(root.kind(), "source_file");
    let full_doc = root.child(0).expect("should have full_document child");
    assert_eq!(full_doc.kind(), "full_document");

    let c = extract_full_document(classify::<FullDocumentNode>(full_doc))
        .expect("producer reconstruction");
    assert!(
        matches!(c.child_0.slot(), Some(NodeSlot::Present(_))),
        "utf8_header: {:?}",
        c.child_0.slot()
    );
}

// ---------------------------------------------------------------------------
// Test 4: Corpus-wide extraction, every node, every method, no panics
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_wide_extraction() {
    let corpus = ChatCorpus::reference().expect("complete reference corpus");

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut files_parsed = 0;

    for fixture in corpus.fixtures() {
        let source = fixture.source();
        let tree = parser.parse(source, None).expect("parse");

        // Walk every node and call the matching extraction function.
        // The key validation: this doesn't panic on any real CST node.
        walk_all(tree.root_node(), &mut |node| {
            let kind = node.kind();
            *kind_counts.entry(kind.to_string()).or_default() += 1;

            // Call extraction for key SEQ rules to verify they work
            match kind {
                "full_document" => {
                    let _ = extract_full_document(classify::<FullDocumentNode>(node))
                        .expect("producer reconstruction");
                }
                "main_tier" => {
                    let _ = extract_main_tier(classify::<MainTierNode>(node))
                        .expect("producer reconstruction");
                }
                "utterance" => {
                    let _ = extract_utterance(classify::<UtteranceNode>(node))
                        .expect("producer reconstruction");
                }
                "tier_body" => {
                    let _ = extract_tier_body(classify::<TierBodyNode>(node))
                        .expect("producer reconstruction");
                }
                "utterance_end" => {
                    let _ = extract_utterance_end(classify::<UtteranceEndNode>(node))
                        .expect("producer reconstruction");
                }
                "participants_header" => {
                    let _ = extract_participants_header(classify::<ParticipantsHeaderNode>(node))
                        .expect("producer reconstruction");
                }
                "languages_header" => {
                    let _ = extract_languages_header(classify::<LanguagesHeaderNode>(node))
                        .expect("producer reconstruction");
                }
                "id_header" => {
                    let _ = extract_id_header(classify::<IdHeaderNode>(node))
                        .expect("producer reconstruction");
                }
                "date_header" => {
                    let _ = extract_date_header(classify::<DateHeaderNode>(node))
                        .expect("producer reconstruction");
                }
                "media_header" => {
                    let _ = extract_media_header(classify::<MediaHeaderNode>(node))
                        .expect("producer reconstruction");
                }
                "comment_header" => {
                    let _ = extract_comment_header(classify::<CommentHeaderNode>(node))
                        .expect("producer reconstruction");
                }
                "mor_dependent_tier" => {
                    let _ = extract_mor_dependent_tier(classify::<MorDependentTierNode>(node))
                        .expect("producer reconstruction");
                }
                "gra_dependent_tier" => {
                    let _ = extract_gra_dependent_tier(classify::<GraDependentTierNode>(node))
                        .expect("producer reconstruction");
                }
                "pho_dependent_tier" => {
                    let _ = extract_pho_dependent_tier(classify::<PhoDependentTierNode>(node))
                        .expect("producer reconstruction");
                }
                "com_dependent_tier" => {
                    let _ = extract_com_dependent_tier(classify::<ComDependentTierNode>(node))
                        .expect("producer reconstruction");
                }
                "word_with_optional_annotations" => {
                    let _ = extract_word_with_optional_annotations(classify::<
                        WordWithOptionalAnnotationsNode,
                    >(node));
                }
                "nonword_with_optional_annotations" => {
                    let _ = extract_nonword_with_optional_annotations(classify::<
                        NonwordWithOptionalAnnotationsNode,
                    >(node));
                }
                "mor_word" => {
                    let _ = extract_mor_word(classify::<MorWordNode>(node))
                        .expect("producer reconstruction");
                }
                "mor_content" => {
                    let _ = extract_mor_content(classify::<MorContentNode>(node))
                        .expect("producer reconstruction");
                }
                "gra_relation" => {
                    let _ = extract_gra_relation(classify::<GraRelationNode>(node))
                        .expect("producer reconstruction");
                }
                "replacement" => {
                    let _ = extract_replacement(classify::<ReplacementNode>(node))
                        .expect("producer reconstruction");
                }
                "group_with_annotations" => {
                    let _ =
                        extract_group_with_annotations(classify::<GroupWithAnnotationsNode>(node))
                            .expect("producer reconstruction");
                }
                "begin_header" => {
                    let _ = extract_begin_header(classify::<BeginHeaderNode>(node))
                        .expect("producer reconstruction");
                }
                "end_header" => {
                    let _ = extract_end_header(classify::<EndHeaderNode>(node))
                        .expect("producer reconstruction");
                }
                "utf8_header" => {
                    let _ = extract_utf8_header(classify::<Utf8HeaderNode>(node))
                        .expect("producer reconstruction");
                }
                _ => {}
            }
        });

        files_parsed += 1;
    }

    assert!(
        files_parsed >= 74,
        "Should parse all 74 files, got {files_parsed}"
    );

    // Print corpus stats
    let total_nodes: usize = kind_counts.values().sum();
    let extracted_kinds = [
        "full_document",
        "main_tier",
        "utterance",
        "tier_body",
        "utterance_end",
        "participants_header",
        "languages_header",
        "id_header",
        "date_header",
        "media_header",
        "comment_header",
        "mor_dependent_tier",
        "gra_dependent_tier",
        "pho_dependent_tier",
        "com_dependent_tier",
        "word_with_optional_annotations",
        "nonword_with_optional_annotations",
        "mor_word",
        "mor_content",
        "gra_relation",
        "replacement",
        "group_with_annotations",
        "begin_header",
        "end_header",
        "utf8_header",
    ];
    let extracted_total: usize = extracted_kinds
        .iter()
        .map(|k| kind_counts.get(*k).copied().unwrap_or(0))
        .sum();

    eprintln!("=== Corpus-wide extraction stats ===");
    eprintln!("Files: {files_parsed}");
    eprintln!("Total CST nodes: {total_nodes}");
    eprintln!("Nodes with extraction methods called: {extracted_total}");
    eprintln!("Key rule counts:");
    for kind in &extracted_kinds {
        if let Some(&count) = kind_counts.get(*kind) {
            eprintln!("  {kind}: {count}");
        }
    }
}

// ---------------------------------------------------------------------------
// Test 5: Speaker extraction parity with hand-written parser
// ---------------------------------------------------------------------------

#[test]
fn test_speaker_parity_with_existing_parser() {
    let corpus = ChatCorpus::reference().expect("complete reference corpus");

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let chat_parser = talkbank_parser::TreeSitterParser::new().expect("grammar loads");

    let mut total_tiers = 0;
    let mut matching_speakers = 0;
    let mut files = 0;

    for fixture in corpus.fixtures() {
        let source = fixture.source();
        let tree = parser.parse(source, None).expect("parse");

        let existing =
            talkbank_parser_tests::test_error::strict_parse(chat_parser.parse_chat_file(source))
                .unwrap_or_else(|error| panic!("{}: {error}", fixture.path().display()));

        // Collect speakers from generated traversal
        let mut gen_speakers = Vec::new();
        walk_all(tree.root_node(), &mut |node| {
            if node.kind() == "main_tier" {
                let c = extract_main_tier(classify::<MainTierNode>(node))
                    .expect("producer reconstruction");
                if let NodeSlot::Present(spk) = c.speaker.slot() {
                    gen_speakers.push(node_text(spk.raw_node(), source).to_string());
                }
            }
        });

        // Compare with existing parser
        let existing_speakers: Vec<String> = existing
            .utterances()
            .map(|u| u.main.speaker.as_str().to_string())
            .collect();

        total_tiers += gen_speakers.len();
        for (g, e) in gen_speakers.iter().zip(existing_speakers.iter()) {
            if g == e {
                matching_speakers += 1;
            }
        }

        files += 1;
    }

    eprintln!("Speaker parity: {matching_speakers}/{total_tiers} match across {files} files");
    assert_eq!(
        matching_speakers, total_tiers,
        "All speakers should match between generated and existing parser"
    );
}

// ---------------------------------------------------------------------------
// Test 6: ALL 113 extraction functions on every node, zero panics
//
// The OLD-backend harness dispatched 116 rule kinds, including the three
// HIDDEN seq rules `_id_demographic_fields` / `_id_identity_fields` /
// `_id_role_fields`. Tree-sitter inlines HIDDEN (underscore-prefixed) rules
// into their parent, so a node of that kind never appears in a real CST
// (confirmed: `walk_all` never visits one; this is the same "hidden rule
// inlining" property `visitor_hidden_rule_inlining.rs` exercises for
// `id_contents`), and the NEW backend correspondingly does NOT generate an
// `extract__id_*` free function for them at all (verified: zero such
// functions in `generated_traversal.rs`). Those three arms were
// already dead code in the OLD harness (never reached); dropping them is not
// a coverage change, only the removal of unreachable arms for functions that
// no longer exist. The remaining 113 real rules are ported 1:1, same set,
// same "no panic" assertion.
// ---------------------------------------------------------------------------

/// Dispatch to the appropriate extraction function based on node kind.
/// Returns true if an extraction was performed.
#[allow(clippy::too_many_lines)]
fn try_extract(node: tree_sitter::Node) -> bool {
    match node.kind() {
        "act_dependent_tier" => {
            let _ = extract_act_dependent_tier(classify::<ActDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "activities_header" => {
            let _ = extract_activities_header(classify::<ActivitiesHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "add_dependent_tier" => {
            let _ = extract_add_dependent_tier(classify::<AddDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "alt_dependent_tier" => {
            let _ = extract_alt_dependent_tier(classify::<AltDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "bck_header" => {
            let _ = extract_bck_header(classify::<BckHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "begin_header" => {
            let _ = extract_begin_header(classify::<BeginHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "bg_header" => {
            let _ =
                extract_bg_header(classify::<BgHeaderNode>(node)).expect("producer reconstruction");
            true
        }
        "birth_of_header" => {
            let _ = extract_birth_of_header(classify::<BirthOfHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "birthplace_of_header" => {
            let _ = extract_birthplace_of_header(classify::<BirthplaceOfHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "blank_header" => {
            let _ = extract_blank_header(classify::<BlankHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "cod_dependent_tier" => {
            let _ = extract_cod_dependent_tier(classify::<CodDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "coh_dependent_tier" => {
            let _ = extract_coh_dependent_tier(classify::<CohDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "color_words_header" => {
            let _ = extract_color_words_header(classify::<ColorWordsHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "com_dependent_tier" => {
            let _ = extract_com_dependent_tier(classify::<ComDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "comment_header" => {
            let _ = extract_comment_header(classify::<CommentHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "date_header" => {
            let _ = extract_date_header(classify::<DateHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "def_dependent_tier" => {
            let _ = extract_def_dependent_tier(classify::<DefDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "full_document" => {
            let _ = extract_full_document(classify::<FullDocumentNode>(node))
                .expect("producer reconstruction");
            true
        }
        "eg_header" => {
            let _ =
                extract_eg_header(classify::<EgHeaderNode>(node)).expect("producer reconstruction");
            true
        }
        "end_header" => {
            let _ = extract_end_header(classify::<EndHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "eng_dependent_tier" => {
            let _ = extract_eng_dependent_tier(classify::<EngDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "err_dependent_tier" => {
            let _ = extract_err_dependent_tier(classify::<ErrDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "event" => {
            let _ = extract_event(classify::<EventNode>(node)).expect("producer reconstruction");
            true
        }
        "exp_dependent_tier" => {
            let _ = extract_exp_dependent_tier(classify::<ExpDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "fac_dependent_tier" => {
            let _ = extract_fac_dependent_tier(classify::<FacDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "flo_dependent_tier" => {
            let _ = extract_flo_dependent_tier(classify::<FloDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "font_header" => {
            let _ = extract_font_header(classify::<FontHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "g_header" => {
            let _ =
                extract_g_header(classify::<GHeaderNode>(node)).expect("producer reconstruction");
            true
        }
        "gls_dependent_tier" => {
            let _ = extract_gls_dependent_tier(classify::<GlsDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "gpx_dependent_tier" => {
            let _ = extract_gpx_dependent_tier(classify::<GpxDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "gra_contents" => {
            let _ = extract_gra_contents(classify::<GraContentsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "gra_dependent_tier" => {
            let _ = extract_gra_dependent_tier(classify::<GraDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "gra_relation" => {
            let _ = extract_gra_relation(classify::<GraRelationNode>(node))
                .expect("producer reconstruction");
            true
        }
        "group_with_annotations" => {
            let _ = extract_group_with_annotations(classify::<GroupWithAnnotationsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "header_sep" => {
            let _ = extract_header_sep(classify::<HeaderSepNode>(node))
                .expect("producer reconstruction");
            true
        }
        "id_contents" => {
            let _ = extract_id_contents(classify::<IdContentsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "id_header" => {
            let _ =
                extract_id_header(classify::<IdHeaderNode>(node)).expect("producer reconstruction");
            true
        }
        "int_dependent_tier" => {
            let _ = extract_int_dependent_tier(classify::<IntDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "l1_of_header" => {
            let _ = extract_l1_of_header(classify::<L1OfHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "languages_contents" => {
            let _ = extract_languages_contents(classify::<LanguagesContentsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "languages_header" => {
            let _ = extract_languages_header(classify::<LanguagesHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "location_header" => {
            let _ = extract_location_header(classify::<LocationHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "long_feature_begin" => {
            let _ = extract_long_feature_begin(classify::<LongFeatureBeginNode>(node))
                .expect("producer reconstruction");
            true
        }
        "long_feature_end" => {
            let _ = extract_long_feature_end(classify::<LongFeatureEndNode>(node))
                .expect("producer reconstruction");
            true
        }
        "main_pho_group" => {
            let _ = extract_main_pho_group(classify::<MainPhoGroupNode>(node))
                .expect("producer reconstruction");
            true
        }
        "main_sin_group" => {
            let _ = extract_main_sin_group(classify::<MainSinGroupNode>(node))
                .expect("producer reconstruction");
            true
        }
        "main_tier" => {
            let _ =
                extract_main_tier(classify::<MainTierNode>(node)).expect("producer reconstruction");
            true
        }
        "media_contents" => {
            let _ = extract_media_contents(classify::<MediaContentsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "media_header" => {
            let _ = extract_media_header(classify::<MediaHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "mod_dependent_tier" => {
            let _ = extract_mod_dependent_tier(classify::<ModDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "modsyl_dependent_tier" => {
            let _ = extract_modsyl_dependent_tier(classify::<ModsylDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "mor_content" => {
            let _ = extract_mor_content(classify::<MorContentNode>(node))
                .expect("producer reconstruction");
            true
        }
        "mor_contents" => {
            let _ = extract_mor_contents(classify::<MorContentsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "mor_dependent_tier" => {
            let _ = extract_mor_dependent_tier(classify::<MorDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "mor_feature" => {
            let _ = extract_mor_feature(classify::<MorFeatureNode>(node))
                .expect("producer reconstruction");
            true
        }
        "mor_post_clitic" => {
            let _ = extract_mor_post_clitic(classify::<MorPostCliticNode>(node))
                .expect("producer reconstruction");
            true
        }
        "mor_word" => {
            let _ =
                extract_mor_word(classify::<MorWordNode>(node)).expect("producer reconstruction");
            true
        }
        "new_episode_header" => {
            let _ = extract_new_episode_header(classify::<NewEpisodeHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "nonvocal_begin" => {
            let _ = extract_nonvocal_begin(classify::<NonvocalBeginNode>(node))
                .expect("producer reconstruction");
            true
        }
        "nonvocal_end" => {
            let _ = extract_nonvocal_end(classify::<NonvocalEndNode>(node))
                .expect("producer reconstruction");
            true
        }
        "nonvocal_simple" => {
            let _ = extract_nonvocal_simple(classify::<NonvocalSimpleNode>(node))
                .expect("producer reconstruction");
            true
        }
        "nonword_with_optional_annotations" => {
            let _ = extract_nonword_with_optional_annotations(classify::<
                NonwordWithOptionalAnnotationsNode,
            >(node))
            .expect("producer reconstruction");
            true
        }
        "number_header" => {
            let _ = extract_number_header(classify::<NumberHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "options_contents" => {
            let _ = extract_options_contents(classify::<OptionsContentsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "options_header" => {
            let _ = extract_options_header(classify::<OptionsHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "ort_dependent_tier" => {
            let _ = extract_ort_dependent_tier(classify::<OrtDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "other_spoken_event" => {
            let _ = extract_other_spoken_event(classify::<OtherSpokenEventNode>(node))
                .expect("producer reconstruction");
            true
        }
        "page_header" => {
            let _ = extract_page_header(classify::<PageHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "par_dependent_tier" => {
            let _ = extract_par_dependent_tier(classify::<ParDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "participant" => {
            let _ = extract_participant(classify::<ParticipantNode>(node))
                .expect("producer reconstruction");
            true
        }
        "participants_contents" => {
            let _ = extract_participants_contents(classify::<ParticipantsContentsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "participants_header" => {
            let _ = extract_participants_header(classify::<ParticipantsHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "pho_dependent_tier" => {
            let _ = extract_pho_dependent_tier(classify::<PhoDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "pho_grouped_content" => {
            let _ = extract_pho_grouped_content(classify::<PhoGroupedContentNode>(node))
                .expect("producer reconstruction");
            true
        }
        "pho_groups" => {
            let _ = extract_pho_groups(classify::<PhoGroupsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "pho_words" => {
            let _ =
                extract_pho_words(classify::<PhoWordsNode>(node)).expect("producer reconstruction");
            true
        }
        "phoaln_dependent_tier" => {
            let _ = extract_phoaln_dependent_tier(classify::<PhoalnDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "phosyl_dependent_tier" => {
            let _ = extract_phosyl_dependent_tier(classify::<PhosylDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "pid_header" => {
            let _ = extract_pid_header(classify::<PidHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "quotation" => {
            let _ = extract_quotation(classify::<QuotationNode>(node))
                .expect("producer reconstruction");
            true
        }
        "recording_quality_header" => {
            let _ = extract_recording_quality_header(classify::<RecordingQualityHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "replacement" => {
            let _ = extract_replacement(classify::<ReplacementNode>(node))
                .expect("producer reconstruction");
            true
        }
        "room_layout_header" => {
            let _ = extract_room_layout_header(classify::<RoomLayoutHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "sin_dependent_tier" => {
            let _ = extract_sin_dependent_tier(classify::<SinDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "sin_grouped_content" => {
            let _ = extract_sin_grouped_content(classify::<SinGroupedContentNode>(node))
                .expect("producer reconstruction");
            true
        }
        "sin_groups" => {
            let _ = extract_sin_groups(classify::<SinGroupsNode>(node))
                .expect("producer reconstruction");
            true
        }
        "sit_dependent_tier" => {
            let _ = extract_sit_dependent_tier(classify::<SitDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "situation_header" => {
            let _ = extract_situation_header(classify::<SituationHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "spa_dependent_tier" => {
            let _ = extract_spa_dependent_tier(classify::<SpaDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "t_header" => {
            let _ =
                extract_t_header(classify::<THeaderNode>(node)).expect("producer reconstruction");
            true
        }
        "tape_location_header" => {
            let _ = extract_tape_location_header(classify::<TapeLocationHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "thumbnail_header" => {
            let _ = extract_thumbnail_header(classify::<ThumbnailHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "tier_body" => {
            let _ =
                extract_tier_body(classify::<TierBodyNode>(node)).expect("producer reconstruction");
            true
        }
        "tier_sep" => {
            let _ =
                extract_tier_sep(classify::<TierSepNode>(node)).expect("producer reconstruction");
            true
        }
        "tim_dependent_tier" => {
            let _ = extract_tim_dependent_tier(classify::<TimDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "time_duration_header" => {
            let _ = extract_time_duration_header(classify::<TimeDurationHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "time_start_header" => {
            let _ = extract_time_start_header(classify::<TimeStartHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "transcriber_header" => {
            let _ = extract_transcriber_header(classify::<TranscriberHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "transcription_header" => {
            let _ = extract_transcription_header(classify::<TranscriptionHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "types_header" => {
            let _ = extract_types_header(classify::<TypesHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "unsupported_dependent_tier" => {
            let _ =
                extract_unsupported_dependent_tier(classify::<UnsupportedDependentTierNode>(node))
                    .expect("producer reconstruction");
            true
        }
        "unsupported_header" => {
            let _ = extract_unsupported_header(classify::<UnsupportedHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "unsupported_line" => {
            let _ = extract_unsupported_line(classify::<UnsupportedLineNode>(node))
                .expect("producer reconstruction");
            true
        }
        "utf8_header" => {
            let _ = extract_utf8_header(classify::<Utf8HeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "utterance" => {
            let _ = extract_utterance(classify::<UtteranceNode>(node))
                .expect("producer reconstruction");
            true
        }
        "utterance_end" => {
            let _ = extract_utterance_end(classify::<UtteranceEndNode>(node))
                .expect("producer reconstruction");
            true
        }
        "videos_header" => {
            let _ = extract_videos_header(classify::<VideosHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "warning_header" => {
            let _ = extract_warning_header(classify::<WarningHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "window_header" => {
            let _ = extract_window_header(classify::<WindowHeaderNode>(node))
                .expect("producer reconstruction");
            true
        }
        "wor_dependent_tier" => {
            let _ = extract_wor_dependent_tier(classify::<WorDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        "wor_tier_body" => {
            let _ = extract_wor_tier_body(classify::<WorTierBodyNode>(node))
                .expect("producer reconstruction");
            true
        }
        "word_with_optional_annotations" => {
            let _ = extract_word_with_optional_annotations(classify::<
                WordWithOptionalAnnotationsNode,
            >(node))
            .expect("producer reconstruction");
            true
        }
        "x_dependent_tier" => {
            let _ = extract_x_dependent_tier(classify::<XDependentTierNode>(node))
                .expect("producer reconstruction");
            true
        }
        _ => false,
    }
}

#[test]
fn test_all_113_extraction_functions_no_panics() {
    let corpus = ChatCorpus::reference().expect("complete reference corpus");

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let mut total_nodes = 0usize;
    let mut extracted_nodes = 0usize;
    let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut files = 0;

    for fixture in corpus.fixtures() {
        let source = fixture.source();
        let tree = parser.parse(source, None).expect("parse");

        walk_all(tree.root_node(), &mut |node| {
            total_nodes += 1;
            if try_extract(node) {
                extracted_nodes += 1;
                *kind_counts.entry(node.kind().to_string()).or_default() += 1;
            }
        });

        files += 1;
    }

    assert!(files >= 74, "Should parse all 74 files");

    let unique_kinds_extracted = kind_counts.len();
    eprintln!("=== ALL 113 FUNCTIONS: Corpus-wide results ===");
    eprintln!("Files: {files}");
    eprintln!("Total CST nodes: {total_nodes}");
    eprintln!("Nodes extracted by generated functions: {extracted_nodes}");
    eprintln!("Unique rule kinds with extraction: {unique_kinds_extracted}/113");
    eprintln!();
    eprintln!("Per-kind counts (top 30):");
    let mut sorted_kinds: Vec<_> = kind_counts.iter().collect();
    sorted_kinds.sort_by(|a, b| b.1.cmp(a.1));
    for (kind, count) in sorted_kinds.iter().take(30) {
        eprintln!("  {kind}: {count}");
    }

    // The key assertion: zero panics across all 37,000+ nodes
    assert!(
        extracted_nodes > 5000,
        "Should extract thousands of nodes, got {extracted_nodes}"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Semantic conversion, @Options header -> ChatOptionFlag
//
// The OLD backend generated an auxiliary `OptionNameValue` "strict +
// catch-all value" enum (`CA` / `NoAlign` / `Other(String)`) purely as a
// traversal-generator convenience; it was never consumed by production
// parsing (verified: the ONLY uses of `OptionNameValue` anywhere in the crate
// were `generated_traversal.rs` itself and this test file). Production has
// always classified `@Options` tokens through the real model type
// `talkbank_model::ChatOptionFlag` (`special.rs`'s `option_flags`, migrated
// onto the NEW free-fn descent in Task B2). The NEW backend does not
// generate this auxiliary value-enum family at all (verified: zero
// "Validated values for" occurrences in `generated_traversal.rs`,
// versus 6 in the OLD module), so this test now exercises the SAME grammar
// classification (`@Options: CA` -> known; `@Options: SomeUnknownOption` ->
// unknown) against the type production actually uses, rather than a
// generated-but-unused stub the NEW backend no longer produces.
// ---------------------------------------------------------------------------

#[test]
fn test_options_header_semantic_conversion() {
    // File with @Options: CA
    let source =
        "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n@Options:\tCA\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut found_option = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "options_header" {
            let children = extract_options_header(classify::<OptionsHeaderNode>(node))
                .expect("producer reconstruction");

            // The payload is options_contents (child_2); its slot already
            // carries the typed `OptionsContentsNode` wrapper, so it is
            // passed straight into `extract_options_contents` with no
            // unwrap-then-rewrap.
            if let NodeSlot::Present(contents_node) = children.child_2.slot() {
                let contents_children =
                    extract_options_contents(*contents_node).expect("producer reconstruction");

                if let NodeSlot::Present(option_node) = contents_children.child_0.slot() {
                    let option_text = node_text(option_node.raw_node(), source);

                    let value = talkbank_model::ChatOptionFlag::from_text(option_text);
                    assert_eq!(value, talkbank_model::ChatOptionFlag::Ca);
                    assert!(!matches!(
                        value,
                        talkbank_model::ChatOptionFlag::Unsupported(_)
                    ));
                    found_option = true;
                }
            }
        }
    });

    assert!(found_option, "Should have found and converted @Options: CA");
}

#[test]
fn test_options_header_unknown_value() {
    // File with @Options: SomeUnknownOption
    let source = "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n@Options:\tSomeUnknownOption\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut found = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "options_header" {
            let children = extract_options_header(classify::<OptionsHeaderNode>(node))
                .expect("producer reconstruction");
            if let NodeSlot::Present(contents_node) = children.child_2.slot() {
                let contents_children =
                    extract_options_contents(*contents_node).expect("producer reconstruction");
                if let NodeSlot::Present(option_node) = contents_children.child_0.slot() {
                    let value = talkbank_model::ChatOptionFlag::from_text(node_text(
                        option_node.raw_node(),
                        source,
                    ));
                    assert!(
                        matches!(value, talkbank_model::ChatOptionFlag::Unsupported(ref s) if s == "SomeUnknownOption"),
                        "Unknown option should be Unsupported, got {value:?}"
                    );
                    found = true;
                }
            }
        }
    });

    assert!(found, "Should have found @Options with unknown value");
}
