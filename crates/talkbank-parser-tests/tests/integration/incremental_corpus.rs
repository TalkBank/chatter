//! The editor's incremental entry points must preserve cold-parse semantics.
//! Sources come from the admitted reference and diagnostic corpora. These are
//! API/wire-boundary comparisons, not fabricated ASTs or validity assertions
//! about intentionally invalid error fixtures.

#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;
use tree_sitter::{InputEdit, Point};

/// Successive admitted specimens stand for arbitrary skipped editor revisions.
/// The producer must derive edits itself and preserve cold-parse semantics,
/// including invalid input, Unicode, a complete deletion, and restoration.
#[test]
fn owned_revisions_preserve_corpus_models_and_diagnostics() {
    let parser = TreeSitterParser::new().expect("parser initialization");
    let mut previous = None;
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("admit corpus");
        for fixture in corpus.fixtures() {
            let source = fixture.source();
            let warm_errors = ErrorCollector::new();
            let (warm, revision) =
                parser.parse_chat_file_revision(source.into(), previous.as_ref(), &warm_errors);
            let cold_errors = ErrorCollector::new();
            let (cold, _) =
                parser.parse_chat_file_streaming_incremental(source, None, &cold_errors);
            assert_eq!(warm, cold, "revision model: {}", fixture.path().display());
            assert_eq!(
                warm_errors.to_vec(),
                cold_errors.to_vec(),
                "revision diagnostics: {}",
                fixture.path().display()
            );
            assert_eq!(revision.source(), source);
            previous = Some(revision);
        }
    }
    assert!(previous.is_some(), "nonempty corpus witness");
    const RESTORED: &str =
        include_str!("../../../../corpus/reference/languages/eng-conversation.cha");
    for source in ["", RESTORED, RESTORED] {
        let warm_errors = ErrorCollector::new();
        let (warm, revision) =
            parser.parse_chat_file_revision(source.into(), previous.as_ref(), &warm_errors);
        let cold_errors = ErrorCollector::new();
        let (cold, _) = parser.parse_chat_file_streaming_incremental(source, None, &cold_errors);
        assert_eq!(warm, cold, "delete/restore model");
        assert_eq!(
            warm_errors.to_vec(),
            cold_errors.to_vec(),
            "delete/restore diagnostics"
        );
        // Structural-query callers may edit their detached copy, but cannot
        // alter the owner's next transition, including unchanged-source reuse.
        if let Some(mut detached) = revision.tree() {
            detached.edit(&InputEdit {
                start_byte: 0,
                old_end_byte: 0,
                new_end_byte: 1,
                start_position: Point::new(0, 0),
                old_end_position: Point::new(0, 0),
                new_end_position: Point::new(0, 1),
            });
        }
        previous = Some(revision);
    }
}

/// Finite editor transitions derived from reference CHAT, not diagnostic goldens.
/// Interior edits retain the suffix, unlike prefix typing/backspacing. Tier
/// spans come from the admitted model, not a second tier-label parser.
#[test]
fn reference_dependent_tier_edits_restore_cold_parse_outcomes() {
    assert_reference_interior_edits(
        &[
            (
                "morphology",
                include_str!("../../../../corpus/reference/tiers/mor-gra.cha"),
            ),
            (
                "phonology",
                include_str!("../../../../corpus/reference/tiers/pho-groupings.cha"),
            ),
            (
                "sign",
                include_str!("../../../../corpus/reference/annotation/groups-sign.cha"),
            ),
        ],
        InteriorTarget::DependentTiers,
        &["mor", "gra", "pho", "sin"],
    );
}

/// Source-bound header spans include prefixes, separators and newlines. Their
/// deletion may move following lines into recovery but must not corrupt reuse.
#[test]
fn reference_identity_header_edits_restore_cold_parse_outcomes() {
    assert_reference_interior_edits(
        &[
            (
                "speaker-info",
                include_str!("../../../../corpus/reference/core/headers-speaker-info.cha"),
            ),
            (
                "portuguese-names",
                include_str!("../../../../corpus/reference/languages/por-conversation.cha"),
            ),
        ],
        InteriorTarget::IdentityHeaders,
        &["participants", "languages", "id"],
    );
}

#[derive(Clone, Copy)]
enum InteriorTarget {
    DependentTiers,
    IdentityHeaders,
    MainTiers,
}

/// Interior main-tier edits retain later speech and dependent tiers while
/// nested annotations and media boundaries enter and leave recovery.
#[test]
fn reference_main_tier_edits_restore_cold_parse_outcomes() {
    assert_reference_interior_edits(
        &[
            (
                "nested-groups",
                include_str!("../../../../corpus/reference/annotation/groups-regular.cha"),
            ),
            (
                "retraces",
                include_str!("../../../../corpus/reference/annotation/retrace.cha"),
            ),
            (
                "media",
                include_str!("../../../../corpus/reference/content/media-bullets.cha"),
            ),
        ],
        InteriorTarget::MainTiers,
        &["main"],
    );
}

fn assert_reference_interior_edits(
    fixtures: &[(&str, &str)],
    target: InteriorTarget,
    expected: &[&str],
) {
    let parser = TreeSitterParser::new().expect("parser initialization");
    let mut witnessed = std::collections::BTreeSet::new();
    let mut diagnosed = 0;
    let mut undiagnosed = 0;
    let mut unicode_edits = 0;
    for &(name, source) in fixtures {
        let initial_errors = ErrorCollector::new();
        let (control, initial) =
            parser.parse_chat_file_revision(source.into(), None, &initial_errors);
        assert!(
            initial_errors.to_vec().is_empty(),
            "reference parses: {name}"
        );
        let mut previous = initial;
        let selected: Vec<_> = match target {
            InteriorTarget::MainTiers => control
                .utterances()
                .map(|utterance| (utterance.main.span, "main"))
                .collect(),
            InteriorTarget::DependentTiers => control
                .utterances()
                .flat_map(|u| &u.dependent_tiers)
                .map(|entry| {
                    (
                        entry.content_span().expect("reference tier body span"),
                        entry.kind(),
                    )
                })
                .collect(),
            InteriorTarget::IdentityHeaders => control
                .lines
                .iter()
                .filter_map(|line| {
                    use talkbank_model::model::{Header, Line};
                    let Line::Header { header, span, .. } = line else {
                        return None;
                    };
                    let kind = match header.as_ref() {
                        Header::Participants { .. } => "participants",
                        Header::Languages { .. } => "languages",
                        Header::ID(_) => "id",
                        _ => return None,
                    };
                    Some((*span, kind))
                })
                .collect(),
        };
        for (span, kind) in selected {
            let body = source
                .get(span.start as usize..span.end as usize)
                .expect("source-bound UTF-8 edit target");
            for (offset, scalar) in body.char_indices() {
                witnessed.insert(kind.to_owned());
                let start = span.start as usize + offset;
                let end = start + scalar.len_utf8();
                let mut edited = source.to_owned();
                edited.replace_range(start..end, "");
                unicode_edits += usize::from(!scalar.is_ascii());
                for (text, restored) in [(edited.as_str(), false), (source, true)] {
                    let warm_errors = ErrorCollector::new();
                    let (warm, revision) =
                        parser.parse_chat_file_revision(text.into(), Some(&previous), &warm_errors);
                    let cold_errors = ErrorCollector::new();
                    let (cold, _) =
                        parser.parse_chat_file_streaming_incremental(text, None, &cold_errors);
                    let findings = cold_errors.to_vec();
                    assert_eq!(
                        warm, cold,
                        "{name} at byte {start}, restored={restored}: model"
                    );
                    assert_eq!(
                        warm_errors.to_vec(),
                        findings,
                        "{name} at byte {start}, restored={restored}: diagnostics"
                    );
                    assert!(
                        findings
                            .iter()
                            .all(|e| e.code != talkbank_model::ErrorCode::InternalError),
                        "ordinary editor deletion must not cause a tool failure: {name} at {start}"
                    );
                    assert_eq!(revision.source(), text);
                    if restored {
                        assert_eq!(
                            warm, control,
                            "restoration clears recovery and preserves all payloads"
                        );
                        assert!(findings.is_empty(), "restored reference is clean");
                    } else if findings.is_empty() {
                        undiagnosed += 1;
                    } else {
                        diagnosed += 1;
                    }
                    previous = revision;
                }
            }
        }
    }
    assert_eq!(
        witnessed,
        expected.iter().map(|kind| (*kind).to_owned()).collect()
    );
    assert!(
        diagnosed > 0 && undiagnosed > 0 && unicode_edits > 0,
        "witness diagnosed and undiagnosed deletions, including Unicode boundaries"
    );
    eprintln!(
        "interior edit deck: {diagnosed} diagnosed deletions, {undiagnosed} undiagnosed deletions, {unicode_edits} Unicode deletions; each restored"
    );
}

/// Finite prefix transitions are a separate edit shape from interior deletions.
#[test]
fn reference_typing_and_backspacing_preserve_cold_parse_outcomes() {
    let parser = TreeSitterParser::new().expect("parser initialization");
    for (name, source) in [
        (
            "morphology",
            include_str!("../../../../corpus/reference/edge-cases/clitics-and-compounds.cha"),
        ),
        (
            "unicode",
            include_str!("../../../../corpus/reference/edge-cases/unicode-ipa-content.cha"),
        ),
        (
            "continuation",
            include_str!("../../../../corpus/reference/core/multiline-continuation.cha"),
        ),
    ] {
        assert!(
            parser.parse_chat_file_incremental(source, None).0.is_ok(),
            "complete reference: {name}"
        );
        let boundaries: Vec<_> = source
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(source.len()))
            .collect();
        let mut previous = None;
        for end in boundaries.iter().chain(boundaries.iter().rev()).copied() {
            let prefix = &source[..end];
            let warm_errors = ErrorCollector::new();
            let (warm, revision) =
                parser.parse_chat_file_revision(prefix.into(), previous.as_ref(), &warm_errors);
            let cold_errors = ErrorCollector::new();
            let (cold, _) =
                parser.parse_chat_file_streaming_incremental(prefix, None, &cold_errors);
            assert_eq!(warm, cold, "{name} at byte {end}: model");
            assert_eq!(
                warm_errors.to_vec(),
                cold_errors.to_vec(),
                "{name} at byte {end}: diagnostics"
            );
            assert!(
                cold_errors
                    .to_vec()
                    .iter()
                    .all(|error| error.code != talkbank_model::ErrorCode::InternalError),
                "typing ordinary CHAT must not cause a tool failure: {name} at byte {end}"
            );
            assert_eq!(revision.source(), prefix);
            previous = Some(revision);
        }
    }
}

/// A raw incremental tree is a caller contract, not evidence of readable ranges.
/// Deliberately violating that contract must not turn source association into
/// unchecked indexing. The old source is a retained reference specimen.
#[test]
fn stale_incremental_tree_does_not_certify_readable_source_ranges() {
    use talkbank_parser::generated_traversal::SourceBindingError;

    let parser = TreeSitterParser::new().expect("parser initialization");
    let source = include_str!("../../../../corpus/reference/languages/eng-conversation.cha");
    let old = parser
        .parse_tree_incremental(source, None)
        .expect("reference tree");
    // Simulate a caller deleting all input but forgetting Tree::edit. This is
    // intentionally not a supported incremental edit; it probes the admission
    // boundary rather than asserting CHAT semantics for the resulting tree.
    let stale = parser
        .parse_source_incremental("", Some(&old))
        .expect("runtime completes");
    assert!(matches!(
        stale.root(),
        Err(SourceBindingError::InvalidRange)
    ));

    let fresh = parser
        .parse_source_incremental("", None)
        .expect("cold empty parse");
    assert_eq!(fresh.root().expect("cold range is readable").text(), "");
}

#[test]
fn incremental_entry_points_preserve_corpus_models_and_diagnostics() {
    let parser = TreeSitterParser::new().expect("parser initialization");
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("admit corpus");
        for fixture in corpus.fixtures() {
            let source = fixture.source();
            let path = fixture.path().display();
            let (cold, tree) = parser.parse_chat_file_incremental(source, None);
            let tree = tree.expect("tree-sitter completes fixture parse");
            let (warm, reused) = parser.parse_chat_file_incremental(source, Some(&tree));
            match (&cold, &warm) {
                (Ok(cold), Ok(warm)) => assert_eq!(cold, warm, "strict model mismatch: {path}"),
                (Err(cold), Err(warm)) => assert_eq!(
                    cold.to_error_vec(),
                    warm.to_error_vec(),
                    "strict diagnostics mismatch: {path}"
                ),
                _ => panic!("strict incremental acceptance mismatch: {path}"),
            }
            assert!(reused.is_some(), "incremental tree missing: {path}");

            let cold_errors = ErrorCollector::new();
            let (cold_file, _) =
                parser.parse_chat_file_streaming_incremental(source, None, &cold_errors);
            let warm_errors = ErrorCollector::new();
            let (warm_file, _) =
                parser.parse_chat_file_streaming_incremental(source, Some(&tree), &warm_errors);
            assert_eq!(cold_file, warm_file, "streaming model mismatch: {path}");
            assert_eq!(
                cold_errors.to_vec(),
                warm_errors.to_vec(),
                "streaming diagnostics mismatch: {path}"
            );
            if let Ok(strict_file) = cold {
                assert_eq!(strict_file, cold_file, "strict/streaming mismatch: {path}");
            }

            // Model an editor newline insertion. The old tree must be edited
            // before reuse; compare with a cold parse of exactly those bytes,
            // including source spans and diagnostic multiplicity/order.
            let row = source.bytes().filter(|byte| *byte == b'\n').count();
            let column = source.rsplit('\n').next().expect("last line").len();
            let mut edited_tree = tree;
            edited_tree.edit(&InputEdit {
                start_byte: source.len(),
                old_end_byte: source.len(),
                new_end_byte: source.len() + 1,
                start_position: Point::new(row, column),
                old_end_position: Point::new(row, column),
                new_end_position: Point::new(row + 1, 0),
            });
            let edited_source = format!("{source}\n");
            let fresh_errors = ErrorCollector::new();
            let (fresh, _) =
                parser.parse_chat_file_streaming_incremental(&edited_source, None, &fresh_errors);
            let edited_errors = ErrorCollector::new();
            let (edited, _) = parser.parse_chat_file_streaming_incremental(
                &edited_source,
                Some(&edited_tree),
                &edited_errors,
            );
            assert_eq!(fresh, edited, "edited model mismatch: {path}");
            assert_eq!(
                fresh_errors.to_vec(),
                edited_errors.to_vec(),
                "edited diagnostics mismatch: {path}"
            );
        }
    }
}
