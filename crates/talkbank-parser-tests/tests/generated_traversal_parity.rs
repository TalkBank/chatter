//! Parity tests: validate the generated `extract_*` free functions against
//! real CHAT data.
//!
//! These tests parse the 74-file reference corpus with tree-sitter and exercise
//! the generated extraction functions on every matching node. They verify:
//! 1. Extraction doesn't panic on any real-world CST
//! 2. Required children are Present in well-formed CHAT
//! 3. Speaker text, tier body, headers extract correctly

use std::collections::BTreeMap;

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

fn corpus_dir() -> Option<std::path::PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?
        .join("corpus/reference");
    dir.exists().then_some(dir)
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
            let c = extract_main_tier(MainTierNode(node));
            assert!(
                matches!(c.child_0.slot, NodeSlot::Present(_)),
                "star: {:?}",
                c.child_0.slot
            );
            assert!(
                matches!(c.speaker.slot, NodeSlot::Present(_)),
                "speaker: {:?}",
                c.speaker.slot
            );
            assert!(
                matches!(c.child_2.slot, NodeSlot::Present(_)),
                "colon: {:?}",
                c.child_2.slot
            );
            assert!(
                matches!(c.child_3.slot, NodeSlot::Present(_)),
                "tab: {:?}",
                c.child_3.slot
            );
            assert!(
                matches!(c.child_4.slot, NodeSlot::Present(_)),
                "tier_body: {:?}",
                c.child_4.slot
            );

            if let NodeSlot::Present(spk) = &c.speaker.slot {
                assert_eq!(node_text(spk.0, source), "CHI");
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
            let c = extract_participants_header(ParticipantsHeaderNode(node));
            // Should have all required children present
            assert!(
                matches!(c.child_0.slot, NodeSlot::Present(_)),
                "prefix: {:?}",
                c.child_0.slot
            );
            assert!(
                matches!(c.child_1.slot, NodeSlot::Present(_)),
                "header_sep: {:?}",
                c.child_1.slot
            );
            assert!(
                matches!(c.child_2.slot, NodeSlot::Present(_)),
                "contents: {:?}",
                c.child_2.slot
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
            let c = extract_date_header(DateHeaderNode(node));
            assert!(
                matches!(c.child_0.slot, NodeSlot::Present(_)),
                "date_prefix: {:?}",
                c.child_0.slot
            );
            assert!(
                matches!(c.child_1.slot, NodeSlot::Present(_)),
                "header_sep: {:?}",
                c.child_1.slot
            );
            assert!(
                matches!(c.child_2.slot, NodeSlot::Present(_)),
                "date_contents: {:?}",
                c.child_2.slot
            );

            if let NodeSlot::Present(date) = &c.child_2.slot {
                assert_eq!(node_text(date.0, source), "01-JAN-2000");
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

    let c = extract_full_document(FullDocumentNode(full_doc));
    assert!(
        matches!(c.child_0.slot, NodeSlot::Present(_)),
        "utf8_header: {:?}",
        c.child_0.slot
    );
}

// ---------------------------------------------------------------------------
// Test 4: Corpus-wide extraction, every node, every method, no panics
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_wide_extraction() {
    let Some(dir) = corpus_dir() else {
        eprintln!("Skipping: corpus/reference not found");
        return;
    };

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut files_parsed = 0;

    for entry in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
    {
        let source = std::fs::read_to_string(entry.path()).expect("read file");
        let tree = parser.parse(&source, None).expect("parse");

        // Walk every node and call the matching extraction function.
        // The key validation: this doesn't panic on any real CST node.
        walk_all(tree.root_node(), &mut |node| {
            let kind = node.kind();
            *kind_counts.entry(kind.to_string()).or_default() += 1;

            // Call extraction for key SEQ rules to verify they work
            match kind {
                "full_document" => {
                    let _ = extract_full_document(FullDocumentNode(node));
                }
                "main_tier" => {
                    let _ = extract_main_tier(MainTierNode(node));
                }
                "utterance" => {
                    let _ = extract_utterance(UtteranceNode(node));
                }
                "tier_body" => {
                    let _ = extract_tier_body(TierBodyNode(node));
                }
                "utterance_end" => {
                    let _ = extract_utterance_end(UtteranceEndNode(node));
                }
                "participants_header" => {
                    let _ = extract_participants_header(ParticipantsHeaderNode(node));
                }
                "languages_header" => {
                    let _ = extract_languages_header(LanguagesHeaderNode(node));
                }
                "id_header" => {
                    let _ = extract_id_header(IdHeaderNode(node));
                }
                "date_header" => {
                    let _ = extract_date_header(DateHeaderNode(node));
                }
                "media_header" => {
                    let _ = extract_media_header(MediaHeaderNode(node));
                }
                "comment_header" => {
                    let _ = extract_comment_header(CommentHeaderNode(node));
                }
                "mor_dependent_tier" => {
                    let _ = extract_mor_dependent_tier(MorDependentTierNode(node));
                }
                "gra_dependent_tier" => {
                    let _ = extract_gra_dependent_tier(GraDependentTierNode(node));
                }
                "pho_dependent_tier" => {
                    let _ = extract_pho_dependent_tier(PhoDependentTierNode(node));
                }
                "com_dependent_tier" => {
                    let _ = extract_com_dependent_tier(ComDependentTierNode(node));
                }
                "word_with_optional_annotations" => {
                    let _ = extract_word_with_optional_annotations(
                        WordWithOptionalAnnotationsNode(node),
                    );
                }
                "nonword_with_optional_annotations" => {
                    let _ = extract_nonword_with_optional_annotations(
                        NonwordWithOptionalAnnotationsNode(node),
                    );
                }
                "mor_word" => {
                    let _ = extract_mor_word(MorWordNode(node));
                }
                "mor_content" => {
                    let _ = extract_mor_content(MorContentNode(node));
                }
                "gra_relation" => {
                    let _ = extract_gra_relation(GraRelationNode(node));
                }
                "replacement" => {
                    let _ = extract_replacement(ReplacementNode(node));
                }
                "group_with_annotations" => {
                    let _ = extract_group_with_annotations(GroupWithAnnotationsNode(node));
                }
                "begin_header" => {
                    let _ = extract_begin_header(BeginHeaderNode(node));
                }
                "end_header" => {
                    let _ = extract_end_header(EndHeaderNode(node));
                }
                "utf8_header" => {
                    let _ = extract_utf8_header(Utf8HeaderNode(node));
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
    let Some(dir) = corpus_dir() else {
        eprintln!("Skipping: corpus/reference not found");
        return;
    };

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let chat_parser = talkbank_parser::TreeSitterParser::new().expect("grammar loads");

    let mut total_tiers = 0;
    let mut matching_speakers = 0;
    let mut files = 0;

    for entry in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
    {
        let source = std::fs::read_to_string(entry.path()).expect("read file");
        let tree = parser.parse(&source, None).expect("parse");

        // Parse with the Rust parser
        let Ok(existing) = chat_parser.parse_chat_file(&source) else {
            continue; // Skip files that fail to parse
        };

        // Collect speakers from generated traversal
        let mut gen_speakers = Vec::new();
        walk_all(tree.root_node(), &mut |node| {
            if node.kind() == "main_tier" {
                let c = extract_main_tier(MainTierNode(node));
                if let NodeSlot::Present(spk) = &c.speaker.slot {
                    gen_speakers.push(node_text(spk.0, &source).to_string());
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
            let _ = extract_act_dependent_tier(ActDependentTierNode(node));
            true
        }
        "activities_header" => {
            let _ = extract_activities_header(ActivitiesHeaderNode(node));
            true
        }
        "add_dependent_tier" => {
            let _ = extract_add_dependent_tier(AddDependentTierNode(node));
            true
        }
        "alt_dependent_tier" => {
            let _ = extract_alt_dependent_tier(AltDependentTierNode(node));
            true
        }
        "bck_header" => {
            let _ = extract_bck_header(BckHeaderNode(node));
            true
        }
        "begin_header" => {
            let _ = extract_begin_header(BeginHeaderNode(node));
            true
        }
        "bg_header" => {
            let _ = extract_bg_header(BgHeaderNode(node));
            true
        }
        "birth_of_header" => {
            let _ = extract_birth_of_header(BirthOfHeaderNode(node));
            true
        }
        "birthplace_of_header" => {
            let _ = extract_birthplace_of_header(BirthplaceOfHeaderNode(node));
            true
        }
        "blank_header" => {
            let _ = extract_blank_header(BlankHeaderNode(node));
            true
        }
        "cod_dependent_tier" => {
            let _ = extract_cod_dependent_tier(CodDependentTierNode(node));
            true
        }
        "coh_dependent_tier" => {
            let _ = extract_coh_dependent_tier(CohDependentTierNode(node));
            true
        }
        "color_words_header" => {
            let _ = extract_color_words_header(ColorWordsHeaderNode(node));
            true
        }
        "com_dependent_tier" => {
            let _ = extract_com_dependent_tier(ComDependentTierNode(node));
            true
        }
        "comment_header" => {
            let _ = extract_comment_header(CommentHeaderNode(node));
            true
        }
        "date_header" => {
            let _ = extract_date_header(DateHeaderNode(node));
            true
        }
        "def_dependent_tier" => {
            let _ = extract_def_dependent_tier(DefDependentTierNode(node));
            true
        }
        "full_document" => {
            let _ = extract_full_document(FullDocumentNode(node));
            true
        }
        "eg_header" => {
            let _ = extract_eg_header(EgHeaderNode(node));
            true
        }
        "end_header" => {
            let _ = extract_end_header(EndHeaderNode(node));
            true
        }
        "eng_dependent_tier" => {
            let _ = extract_eng_dependent_tier(EngDependentTierNode(node));
            true
        }
        "err_dependent_tier" => {
            let _ = extract_err_dependent_tier(ErrDependentTierNode(node));
            true
        }
        "event" => {
            let _ = extract_event(EventNode(node));
            true
        }
        "exp_dependent_tier" => {
            let _ = extract_exp_dependent_tier(ExpDependentTierNode(node));
            true
        }
        "fac_dependent_tier" => {
            let _ = extract_fac_dependent_tier(FacDependentTierNode(node));
            true
        }
        "flo_dependent_tier" => {
            let _ = extract_flo_dependent_tier(FloDependentTierNode(node));
            true
        }
        "font_header" => {
            let _ = extract_font_header(FontHeaderNode(node));
            true
        }
        "g_header" => {
            let _ = extract_g_header(GHeaderNode(node));
            true
        }
        "gls_dependent_tier" => {
            let _ = extract_gls_dependent_tier(GlsDependentTierNode(node));
            true
        }
        "gpx_dependent_tier" => {
            let _ = extract_gpx_dependent_tier(GpxDependentTierNode(node));
            true
        }
        "gra_contents" => {
            let _ = extract_gra_contents(GraContentsNode(node));
            true
        }
        "gra_dependent_tier" => {
            let _ = extract_gra_dependent_tier(GraDependentTierNode(node));
            true
        }
        "gra_relation" => {
            let _ = extract_gra_relation(GraRelationNode(node));
            true
        }
        "group_with_annotations" => {
            let _ = extract_group_with_annotations(GroupWithAnnotationsNode(node));
            true
        }
        "header_sep" => {
            let _ = extract_header_sep(HeaderSepNode(node));
            true
        }
        "id_contents" => {
            let _ = extract_id_contents(IdContentsNode(node));
            true
        }
        "id_header" => {
            let _ = extract_id_header(IdHeaderNode(node));
            true
        }
        "int_dependent_tier" => {
            let _ = extract_int_dependent_tier(IntDependentTierNode(node));
            true
        }
        "l1_of_header" => {
            let _ = extract_l1_of_header(L1OfHeaderNode(node));
            true
        }
        "languages_contents" => {
            let _ = extract_languages_contents(LanguagesContentsNode(node));
            true
        }
        "languages_header" => {
            let _ = extract_languages_header(LanguagesHeaderNode(node));
            true
        }
        "location_header" => {
            let _ = extract_location_header(LocationHeaderNode(node));
            true
        }
        "long_feature_begin" => {
            let _ = extract_long_feature_begin(LongFeatureBeginNode(node));
            true
        }
        "long_feature_end" => {
            let _ = extract_long_feature_end(LongFeatureEndNode(node));
            true
        }
        "main_pho_group" => {
            let _ = extract_main_pho_group(MainPhoGroupNode(node));
            true
        }
        "main_sin_group" => {
            let _ = extract_main_sin_group(MainSinGroupNode(node));
            true
        }
        "main_tier" => {
            let _ = extract_main_tier(MainTierNode(node));
            true
        }
        "media_contents" => {
            let _ = extract_media_contents(MediaContentsNode(node));
            true
        }
        "media_header" => {
            let _ = extract_media_header(MediaHeaderNode(node));
            true
        }
        "mod_dependent_tier" => {
            let _ = extract_mod_dependent_tier(ModDependentTierNode(node));
            true
        }
        "modsyl_dependent_tier" => {
            let _ = extract_modsyl_dependent_tier(ModsylDependentTierNode(node));
            true
        }
        "mor_content" => {
            let _ = extract_mor_content(MorContentNode(node));
            true
        }
        "mor_contents" => {
            let _ = extract_mor_contents(MorContentsNode(node));
            true
        }
        "mor_dependent_tier" => {
            let _ = extract_mor_dependent_tier(MorDependentTierNode(node));
            true
        }
        "mor_feature" => {
            let _ = extract_mor_feature(MorFeatureNode(node));
            true
        }
        "mor_post_clitic" => {
            let _ = extract_mor_post_clitic(MorPostCliticNode(node));
            true
        }
        "mor_word" => {
            let _ = extract_mor_word(MorWordNode(node));
            true
        }
        "new_episode_header" => {
            let _ = extract_new_episode_header(NewEpisodeHeaderNode(node));
            true
        }
        "nonvocal_begin" => {
            let _ = extract_nonvocal_begin(NonvocalBeginNode(node));
            true
        }
        "nonvocal_end" => {
            let _ = extract_nonvocal_end(NonvocalEndNode(node));
            true
        }
        "nonvocal_simple" => {
            let _ = extract_nonvocal_simple(NonvocalSimpleNode(node));
            true
        }
        "nonword_with_optional_annotations" => {
            let _ =
                extract_nonword_with_optional_annotations(NonwordWithOptionalAnnotationsNode(node));
            true
        }
        "number_header" => {
            let _ = extract_number_header(NumberHeaderNode(node));
            true
        }
        "options_contents" => {
            let _ = extract_options_contents(OptionsContentsNode(node));
            true
        }
        "options_header" => {
            let _ = extract_options_header(OptionsHeaderNode(node));
            true
        }
        "ort_dependent_tier" => {
            let _ = extract_ort_dependent_tier(OrtDependentTierNode(node));
            true
        }
        "other_spoken_event" => {
            let _ = extract_other_spoken_event(OtherSpokenEventNode(node));
            true
        }
        "page_header" => {
            let _ = extract_page_header(PageHeaderNode(node));
            true
        }
        "par_dependent_tier" => {
            let _ = extract_par_dependent_tier(ParDependentTierNode(node));
            true
        }
        "participant" => {
            let _ = extract_participant(ParticipantNode(node));
            true
        }
        "participants_contents" => {
            let _ = extract_participants_contents(ParticipantsContentsNode(node));
            true
        }
        "participants_header" => {
            let _ = extract_participants_header(ParticipantsHeaderNode(node));
            true
        }
        "pho_dependent_tier" => {
            let _ = extract_pho_dependent_tier(PhoDependentTierNode(node));
            true
        }
        "pho_grouped_content" => {
            let _ = extract_pho_grouped_content(PhoGroupedContentNode(node));
            true
        }
        "pho_groups" => {
            let _ = extract_pho_groups(PhoGroupsNode(node));
            true
        }
        "pho_words" => {
            let _ = extract_pho_words(PhoWordsNode(node));
            true
        }
        "phoaln_dependent_tier" => {
            let _ = extract_phoaln_dependent_tier(PhoalnDependentTierNode(node));
            true
        }
        "phosyl_dependent_tier" => {
            let _ = extract_phosyl_dependent_tier(PhosylDependentTierNode(node));
            true
        }
        "pid_header" => {
            let _ = extract_pid_header(PidHeaderNode(node));
            true
        }
        "quotation" => {
            let _ = extract_quotation(QuotationNode(node));
            true
        }
        "recording_quality_header" => {
            let _ = extract_recording_quality_header(RecordingQualityHeaderNode(node));
            true
        }
        "replacement" => {
            let _ = extract_replacement(ReplacementNode(node));
            true
        }
        "room_layout_header" => {
            let _ = extract_room_layout_header(RoomLayoutHeaderNode(node));
            true
        }
        "sin_dependent_tier" => {
            let _ = extract_sin_dependent_tier(SinDependentTierNode(node));
            true
        }
        "sin_grouped_content" => {
            let _ = extract_sin_grouped_content(SinGroupedContentNode(node));
            true
        }
        "sin_groups" => {
            let _ = extract_sin_groups(SinGroupsNode(node));
            true
        }
        "sit_dependent_tier" => {
            let _ = extract_sit_dependent_tier(SitDependentTierNode(node));
            true
        }
        "situation_header" => {
            let _ = extract_situation_header(SituationHeaderNode(node));
            true
        }
        "spa_dependent_tier" => {
            let _ = extract_spa_dependent_tier(SpaDependentTierNode(node));
            true
        }
        "t_header" => {
            let _ = extract_t_header(THeaderNode(node));
            true
        }
        "tape_location_header" => {
            let _ = extract_tape_location_header(TapeLocationHeaderNode(node));
            true
        }
        "thumbnail_header" => {
            let _ = extract_thumbnail_header(ThumbnailHeaderNode(node));
            true
        }
        "tier_body" => {
            let _ = extract_tier_body(TierBodyNode(node));
            true
        }
        "tier_sep" => {
            let _ = extract_tier_sep(TierSepNode(node));
            true
        }
        "tim_dependent_tier" => {
            let _ = extract_tim_dependent_tier(TimDependentTierNode(node));
            true
        }
        "time_duration_header" => {
            let _ = extract_time_duration_header(TimeDurationHeaderNode(node));
            true
        }
        "time_start_header" => {
            let _ = extract_time_start_header(TimeStartHeaderNode(node));
            true
        }
        "transcriber_header" => {
            let _ = extract_transcriber_header(TranscriberHeaderNode(node));
            true
        }
        "transcription_header" => {
            let _ = extract_transcription_header(TranscriptionHeaderNode(node));
            true
        }
        "types_header" => {
            let _ = extract_types_header(TypesHeaderNode(node));
            true
        }
        "unsupported_dependent_tier" => {
            let _ = extract_unsupported_dependent_tier(UnsupportedDependentTierNode(node));
            true
        }
        "unsupported_header" => {
            let _ = extract_unsupported_header(UnsupportedHeaderNode(node));
            true
        }
        "unsupported_line" => {
            let _ = extract_unsupported_line(UnsupportedLineNode(node));
            true
        }
        "utf8_header" => {
            let _ = extract_utf8_header(Utf8HeaderNode(node));
            true
        }
        "utterance" => {
            let _ = extract_utterance(UtteranceNode(node));
            true
        }
        "utterance_end" => {
            let _ = extract_utterance_end(UtteranceEndNode(node));
            true
        }
        "videos_header" => {
            let _ = extract_videos_header(VideosHeaderNode(node));
            true
        }
        "warning_header" => {
            let _ = extract_warning_header(WarningHeaderNode(node));
            true
        }
        "window_header" => {
            let _ = extract_window_header(WindowHeaderNode(node));
            true
        }
        "wor_dependent_tier" => {
            let _ = extract_wor_dependent_tier(WorDependentTierNode(node));
            true
        }
        "wor_tier_body" => {
            let _ = extract_wor_tier_body(WorTierBodyNode(node));
            true
        }
        "word_with_optional_annotations" => {
            let _ = extract_word_with_optional_annotations(WordWithOptionalAnnotationsNode(node));
            true
        }
        "x_dependent_tier" => {
            let _ = extract_x_dependent_tier(XDependentTierNode(node));
            true
        }
        _ => false,
    }
}

#[test]
fn test_all_113_extraction_functions_no_panics() {
    let dir = {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("corpus/reference"));
        match p {
            Some(d) if d.exists() => d,
            _ => {
                eprintln!("Skipping: corpus/reference not found");
                return;
            }
        }
    };

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let mut total_nodes = 0usize;
    let mut extracted_nodes = 0usize;
    let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut files = 0;

    for entry in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
    {
        let source = std::fs::read_to_string(entry.path()).expect("read file");
        let tree = parser.parse(&source, None).expect("parse");

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
            let children = extract_options_header(OptionsHeaderNode(node));

            // The payload is options_contents (child_2); its slot already
            // carries the typed `OptionsContentsNode` wrapper, so it is
            // passed straight into `extract_options_contents` with no
            // unwrap-then-rewrap.
            if let NodeSlot::Present(contents_node) = &children.child_2.slot {
                let contents_children = extract_options_contents(*contents_node);

                if let NodeSlot::Present(option_node) = &contents_children.child_0.slot {
                    let option_text = node_text(option_node.0, source);

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
            let children = extract_options_header(OptionsHeaderNode(node));
            if let NodeSlot::Present(contents_node) = &children.child_2.slot {
                let contents_children = extract_options_contents(*contents_node);
                if let NodeSlot::Present(option_node) = &contents_children.child_0.slot {
                    let value =
                        talkbank_model::ChatOptionFlag::from_text(node_text(option_node.0, source));
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
