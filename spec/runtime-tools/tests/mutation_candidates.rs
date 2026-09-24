//! Policy and byte-preservation checks using canonical CHAT fixtures.
use spec_runtime_tools::mutation::{AdmittedSeed, SeedRejection};
use talkbank_model::RuleSelection;
use talkbank_model::model::{FileStem, TranscriptName};
use talkbank_parser::TreeSitterParser;

#[test]
fn canonical_terminators_delete_only_their_source_bytes() -> anyhow::Result<()> {
    let parser = TreeSitterParser::new()?;
    for (source, expected) in [
        (
            include_str!("../../../corpus/reference/content/terminators-standard.cha"),
            vec![".", "?", "!", "."],
        ),
        (
            include_str!("../../../corpus/reference/content/terminators-continuation.cha"),
            vec!["+...", "+/.", ".", "+//.", "+."],
        ),
        (
            include_str!("../../../corpus/reference/content/terminators-quote-and-special.cha"),
            vec!["+\"/.", ".", "+\"."],
        ),
    ] {
        let name = TranscriptName::Named(FileStem::from_stem("terminators"));
        let rules = RuleSelection::new();
        let seed = AdmittedSeed::admit(&parser, source, name, rules)?;
        assert_eq!(seed.name(), name);
        assert_eq!(seed.rules(), rules);
        assert_eq!(seed.source(), source);
        let candidates = seed.terminator_deletions().collect::<Result<Vec<_>, _>>()?;
        assert_eq!(candidates.len(), expected.len());
        for (candidate, removed) in candidates.iter().zip(expected) {
            assert_eq!(candidate.removed(), removed);
            let range = candidate.range();
            let mut restored = candidate.render();
            restored.insert_str(range.start, removed);
            assert_eq!(restored, source);
        }
    }
    Ok(())
}

#[test]
fn malformed_and_semantically_invalid_seeds_are_refused() -> anyhow::Result<()> {
    let parser = TreeSitterParser::new()?;
    let malformed = include_str!(
        "../../../crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E316_speaker_colon_2.cha"
    );
    assert!(matches!(
        AdmittedSeed::admit(&parser, malformed, TranscriptName::Anonymous, RuleSelection::new()),
        Err(SeedRejection::Parse(diagnostics)) if !diagnostics.is_empty()
    ));
    let media = include_str!(
        "../../../crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E531_2.cha"
    );
    AdmittedSeed::admit(
        &parser,
        media,
        TranscriptName::Named(FileStem::from_stem("media_sample")),
        RuleSelection::new(),
    )?;
    assert!(matches!(
        AdmittedSeed::admit(&parser, media, TranscriptName::Named(FileStem::from_stem("other_sample")), RuleSelection::new()),
        Err(SeedRejection::Validation(diagnostics)) if diagnostics.iter().any(|error| error.code.to_string() == "E531")
    ));
    Ok(())
}

#[test]
fn timed_seed_keeps_bullets_pictures_and_dependent_tiers_intact() -> anyhow::Result<()> {
    let parser = TreeSitterParser::new()?;
    let source = include_str!("../../../corpus/reference/content/media-bullets.cha");
    let seed = AdmittedSeed::admit(
        &parser,
        source,
        TranscriptName::Named(FileStem::from_stem("media-bullets")),
        RuleSelection::new(),
    )?;
    let candidates = seed.terminator_deletions().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(candidates.len(), 2);
    let promoted = [
        include_str!(
            "../../../crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E305_timed_terminators_2.cha"
        ),
        include_str!(
            "../../../crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E305_timed_terminators_3.cha"
        ),
    ];
    assert_eq!(
        source,
        include_str!(
            "../../../crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E305_timed_terminators_1.cha"
        )
    );
    for ((candidate, removed), promoted) in candidates.iter().zip([".", "?"]).zip(promoted) {
        assert_eq!(candidate.removed(), removed);
        assert_eq!(candidate.render(), promoted);
        let mut restored = candidate.render();
        restored.insert_str(candidate.range().start, removed);
        assert_eq!(restored, source);
    }
    Ok(())
}

#[test]
fn cli_retains_seed_provenance_and_refuses_invalid_input_without_output() -> anyhow::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = root.join("corpus/reference/content/terminators-standard.cha");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mutation_candidates"))
        .arg(&input)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let batch: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(batch["seed"], std::fs::read_to_string(&input)?);
    assert_eq!(batch["transcript_stem"], "terminators-standard");
    assert_eq!(batch["strict_linkers"], false);
    let candidates = batch["candidates"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing candidate array"))?;
    assert_eq!(candidates.len(), 4);
    for candidate in candidates {
        assert_eq!(candidate["assessment"], "unreviewed");
        assert_eq!(candidate["mutation"], "delete_main_terminator");
        assert!(candidate.get("expected_error").is_none());
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mutation_candidates"))
        .arg(root.join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E316_speaker_colon_2.cha"))
        .output()?;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("seed parsing was not diagnostic-free")
    );
    Ok(())
}
