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

//! Data-driven runner for the generated validation corpus.
//!
//! `just spec-gen` emits one `.cha` fixture per error-spec EXAMPLE (every
//! spec, both pipeline stages, since R4 made the corpus total) plus a
//! `manifest.json` recording each fixture's spec code and its CLAIM. This
//! test:
//!
//! 1. Reads the manifest (fails if missing: regenerate with `just spec-gen`).
//! 2. Parses each implemented fixture with streaming diagnostics, then runs
//!    `validate_with_alignment`, collecting BOTH stages' codes.
//! 3. Judges the fixture's claim via the shared `Claim::satisfied_by`,
//!    negative halves included (`legal`, and `subsumed_by`'s own-code-absent
//!    part).
//! 4. Enforces the manifest's per-code coverage gate
//!    (`implemented_codes_without_examples`), so a newly-implemented rule
//!    cannot silently ship with no triggering example anywhere.
use std::fs;
use std::path::PathBuf;
use talkbank_model::model::{FileStem, TranscriptName};

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

// The manifest types are the GENERATOR'S OWN, read from the crate both cargo
// workspaces share. A hand-written mirror of `ValidationFixtureEntry` and
// `ValidationManifest` stood here until 2026-09-08, matched to the generator
// by field name and by nothing else; its own doc recorded that a previous
// local status enum had already drifted, and adding `rules` to the format
// would have meant adding it here too. Moving the wire types into
// `talkbank-spec-vocabulary` deleted both structs and the drift with them.
use talkbank_spec_vocabulary::SpecErrorCode;
use talkbank_spec_vocabulary::paths::RepoRelativePath;
use talkbank_spec_vocabulary::validation_manifest::FixtureTranscriptName;
use talkbank_spec_vocabulary::validation_manifest::ValidationManifest as Manifest;

/// The validation corpus dir under this crate (where the generator writes).
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/error_corpus/validation_errors")
}

/// Independent syntax faults must survive both recovery and typed validation.
#[test]
fn speaker_boundary_specs_report_length_and_ascii_faults_independently() {
    use talkbank_model::ErrorCode;
    let parser = TreeSitterParser::new().expect("parser");
    for (example, length_fault, character_fault) in [
        (1, false, false),
        (2, true, false),
        (3, false, true),
        (4, true, true),
    ] {
        let source =
            fs::read_to_string(corpus_dir().join(format!("E307_boundaries_{example}.cha")))
                .expect("canonical speaker boundary");
        let sink = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &sink);
        file.validate(&sink, TranscriptName::Anonymous);
        let diagnostics = sink.into_vec();
        let mut sites = std::collections::BTreeMap::<(u32, u32), (bool, bool)>::new();
        for error in diagnostics
            .iter()
            .filter(|error| error.code == ErrorCode::InvalidSpeaker)
        {
            let span = error.location.span;
            let faults = sites.entry((span.start, span.end)).or_default();
            if error.message.contains("exceeds maximum length") {
                faults.0 = true;
            } else if error.message.contains("contains invalid character") {
                faults.1 = true;
            } else {
                panic!("unclassified speaker finding: {error:?}");
            }
        }
        assert_eq!(
            !sites.is_empty(),
            length_fault || character_fault,
            "example {example}: {diagnostics:?}"
        );
        for (span, faults) in sites {
            assert_eq!(
                faults,
                (length_fault, character_fault),
                "each retained code needs every applicable finding: example {example}, span {span:?}"
            );
        }
    }
}

/// Named rule counts and source spans survive nested traversal and recovery.
#[test]
fn leading_bullet_specs_preserve_scope_diagnostics_and_timing() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::WriteChat;
    let parser = TreeSitterParser::new().expect("parser");
    for (example, expected) in [
        (1, 1),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 1),
        (6, 0),
        (7, 1),
        (8, 2),
        (9, 0),
    ] {
        let source = fs::read_to_string(corpus_dir().join(format!("E770_{example}.cha")))
            .expect("canonical leading-bullet spec");
        let sink = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &sink);
        let parse = sink.into_vec();
        assert_eq!(
            parse.is_empty(),
            example != 9,
            "example {example}: {parse:?}"
        );
        let sink = ErrorCollector::new();
        file.validate_with_alignment(
            &sink,
            TranscriptName::Named(FileStem::from_stem("leading-bullet")),
        );
        let diagnostics = sink.into_vec();
        let leading: Vec<_> = diagnostics
            .iter()
            .filter(|error| error.code == ErrorCode::TimingBulletBeforeContent)
            .collect();
        assert_eq!(
            leading.len(),
            expected,
            "example {example}: {diagnostics:?}"
        );
        for (index, error) in leading.iter().enumerate() {
            let span = error.location.span;
            assert_eq!(
                &source[span.start as usize..span.end as usize],
                ["\u{15}100_200\u{15}", "\u{15}200_300\u{15}"][index]
            );
        }
        if example != 9 {
            assert_eq!(file.to_chat_string(), source, "example {example}");
        }
    }
}

/// Parsing and serialization preserve each timing scope, including invalid tiers.
#[test]
fn bullet_retention_specs_preserve_each_timing_scope() {
    use talkbank_model::model::{UtteranceContent, WriteChat};
    let parser = TreeSitterParser::new().expect("parser");
    for (example, internal_count, terminal) in [(1, 1, true), (2, 1, true), (3, 1, false)] {
        let source =
            fs::read_to_string(corpus_dir().join(format!("E305_bullet_retention_{example}.cha")))
                .expect("canonical timing specimen");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "example {example}");
        let utterance = file.utterances().next().expect("one utterance");
        let content = &utterance.main.content;
        assert_eq!(
            content
                .content
                .iter()
                .filter(|item| matches!(item, UtteranceContent::InternalBullet(_)))
                .count(),
            internal_count,
            "example {example}"
        );
        assert_eq!(content.bullet.is_some(), terminal, "example {example}");
        assert_eq!(
            file.to_chat_string(),
            source,
            "retain every timing scope: example {example}"
        );
    }
}

/// Named spec inputs preserve normalization-side advice without turning
/// canonical equivalence into a filename mismatch or modifying source bytes.
#[test]
fn media_normalization_specs_identify_the_noncanonical_side() {
    use talkbank_model::model::WriteChat;
    use talkbank_model::{ErrorCode, Severity};
    let dir = corpus_dir();
    let manifest: Manifest = serde_json::from_str(
        &fs::read_to_string(dir.join("manifest.json")).expect("canonical manifest"),
    )
    .expect("typed fixture manifest");
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, expected_message) in [
        (
            "W109_1.cha",
            "The file name uses a nonstandard Unicode spelling (such as a letter plus a separate accent mark); rename it to \"Höchste.cha\" using the standard spelling.",
        ),
        (
            "W109_2.cha",
            "The @Media name uses a nonstandard Unicode spelling (such as a letter plus a separate accent mark); use \"Schlüssel\".",
        ),
        (
            "W109_3.cha",
            "The @Media name uses a nonstandard Unicode spelling (such as a letter plus a separate accent mark); use \"ạ́\". The file name uses a nonstandard Unicode spelling (such as a letter plus a separate accent mark); rename it to \"ạ́.cha\" using the standard spelling.",
        ),
        (
            "W109_5.cha",
            "The @Media name uses a nonstandard Unicode spelling (such as a letter plus a separate accent mark); use \"Schlüssel\". The file name uses a nonstandard Unicode spelling (such as a letter plus a separate accent mark); rename it to \"Schlüssel.cha\" using the standard spelling.",
        ),
    ] {
        let entry = manifest
            .fixtures
            .iter()
            .find(|entry| entry.fixture.as_str() == fixture)
            .expect("canonical normalization example");
        let FixtureTranscriptName::Named(stem) = &entry.transcript_name else {
            panic!("normalization requires the authored transcript name");
        };
        let source = fs::read_to_string(dir.join(fixture)).expect("canonical fixture");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "clean syntax: {fixture}");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Named(FileStem::from_stem(stem)));
        let diagnostics = errors.into_vec();
        assert!(
            !diagnostics
                .iter()
                .any(|error| error.code == ErrorCode::MediaFilenameMismatch)
        );
        let warnings: Vec<_> = diagnostics
            .iter()
            .filter(|error| error.code == ErrorCode::MediaFilenameNonCanonicalUnicode)
            .collect();
        assert_eq!(warnings.len(), 1, "{fixture}: {diagnostics:?}");
        assert_eq!(warnings[0].severity, Severity::Warning);
        assert_eq!(warnings[0].message, expected_message);
        assert_eq!(
            file.to_chat_string(),
            source,
            "normalization advice is not a repair"
        );
    }
}

/// Parse recovery is a refusal, but must not discard healthy sibling tiers.
#[test]
fn free_text_recovery_specs_preserve_unaffected_sibling_tiers() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::WriteChat;
    enum Admission {
        Clean,
        Recovered {
            label: &'static str,
            retained: &'static [&'static str],
        },
    }
    let parser = TreeSitterParser::new().expect("parser");
    for (example, admission) in [
        (4, Admission::Clean),
        (
            5,
            Admission::Recovered {
                label: "%com:",
                retained: &["eng", "com", "xnote"],
            },
        ),
        (
            6,
            Admission::Recovered {
                label: "%xnote:",
                retained: &["eng", "com"],
            },
        ),
    ] {
        let source = fs::read_to_string(corpus_dir().join(format!("E330_{example}.cha")))
            .expect("canonical free-text recovery spec");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        let diagnostics = errors.into_vec();
        let utterance = file.utterances().next().expect("unaffected main tier");
        let labels: Vec<_> = utterance
            .dependent_tiers
            .iter()
            .map(|tier| tier.kind())
            .collect();
        match admission {
            Admission::Clean => {
                assert!(diagnostics.is_empty(), "{diagnostics:?}");
                assert_eq!(labels, ["eng", "com", "xnote"]);
                assert_eq!(file.to_chat_string(), source);
            }
            Admission::Recovered { label, retained } => {
                let mut codes: Vec<_> = diagnostics.iter().map(|error| error.code).collect();
                codes.sort_by_key(|code| code.to_string());
                assert_eq!(
                    codes,
                    [ErrorCode::UnparsableContent, ErrorCode::TreeParsingError]
                );
                assert_eq!(labels, retained);
                let start = source.find(label).expect("authored damaged tier");
                let end = start + source[start..].find('\n').expect("tier newline") + 1;
                assert!(
                    diagnostics
                        .iter()
                        .all(|error| error.location.span.start as usize >= start
                            && error.location.span.end as usize <= end),
                    "{diagnostics:?}"
                );
                for sibling in utterance
                    .dependent_tiers
                    .iter()
                    .filter(|tier| !label.starts_with(&format!("%{}:", tier.kind())))
                {
                    assert!(
                        source.contains(&sibling.to_chat_string()),
                        "recovery changed a healthy sibling"
                    );
                }
            }
        }
    }
}

/// Verify each implemented fixture SATISFIES ITS CLAIM, absences included.
#[test]
fn validation_errors_detected() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let dir = corpus_dir();

    let manifest_text = fs::read_to_string(dir.join("manifest.json")).map_err(|err| {
        TestError::Failure(format!(
            "Failed to read manifest.json in {} (regenerate with `just spec-gen`): {err}",
            dir.display()
        ))
    })?;
    let manifest: Manifest = serde_json::from_str(&manifest_text)
        .map_err(|err| TestError::Failure(format!("Failed to parse manifest.json: {err}")))?;

    if manifest.fixtures.is_empty() {
        return Err(TestError::Failure(
            "Validation manifest has no fixtures!".to_string(),
        ));
    }

    println!("Testing {} manifest fixtures...\n", manifest.fixtures.len());

    let mut failures = Vec::new();
    let mut skipped = 0usize;

    for entry in &manifest.fixtures {
        if entry.status != talkbank_spec_vocabulary::Status::Implemented {
            skipped += 1;
            println!(
                "  ⊘ {} → skipped (status: {:?}, {})",
                entry.fixture, entry.status, entry.source_spec
            );
            continue;
        }

        let content = fs::read_to_string(dir.join(&entry.fixture)).map_err(|err| {
            TestError::Failure(format!("Failed to read fixture {}: {err}", entry.fixture))
        })?;

        // Parse with streaming diagnostics so recovered parser errors are visible,
        // then validate. Collect both parse- and validation-level codes.
        let parse_errors = ErrorCollector::new();
        let parse_result = parser.parse_chat_file_fragment(&content, 0, &parse_errors);
        let mut codes: Vec<String> = parse_errors
            .to_vec()
            .iter()
            .map(|e| e.code.to_string())
            .collect();
        if let ParseOutcome::Parsed(mut chat_file) = parse_result {
            let validation_errors = ErrorCollector::new();
            let transcript_name = match &entry.transcript_name {
                FixtureTranscriptName::Anonymous => TranscriptName::Anonymous,
                FixtureTranscriptName::Named(stem) => {
                    TranscriptName::Named(FileStem::from_stem(stem))
                }
            };
            // The fixture runs under the rules its own code declares. Before
            // the manifest carried them, every fixture ran under the default
            // rule set, so a fixture for an opt-in rule could only ever report
            // nothing; eight such codes were marked `not_implemented` to keep
            // this runner quiet about them.
            chat_file.validate_with_alignment_and_rules(
                entry.rules.selection(),
                &validation_errors,
                transcript_name,
            );
            codes.extend(
                validation_errors
                    .to_vec()
                    .iter()
                    .map(|e| e.code.to_string()),
            );
        }

        // The claim's MEANING lives on the claim (`Claim::satisfied_by`, the
        // one owner in the vocabulary crate); only the rendering of what was
        // wanted is local to this report.
        use talkbank_spec_vocabulary::frontmatter::Claim;
        let satisfied = entry.claim.satisfied_by(&entry.code, |code| {
            codes.iter().any(|got| got == code.as_str())
        });
        let wants = match &entry.claim {
            Claim::Violates => entry.code.as_str().to_owned(),
            Claim::Legal => format!("absence of {}", entry.code.as_str()),
            Claim::SubsumedBy(targets) => format!(
                "{} and absence of {}",
                targets
                    .as_slice()
                    .iter()
                    .map(|target| target.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
                entry.code.as_str()
            ),
        };
        // THE OTHER HALF OF AN OPT-IN CLAIM, and without it the declaration
        // is decoration. A fixture whose code declares an option is run a
        // SECOND time with no options at all, and the code must be absent.
        //
        // Why it has to exist: everything above runs the fixture only with the
        // option ON, so deleting the four `enable_quotation_validation` guards
        // in `validation/cross_utterance/mod.rs` (or flipping the default in
        // `validation/context.rs`) leaves every fixture emitting its code,
        // every claim satisfied, the observation snapshot byte-identical and
        // the gate green, while eight published pages go on telling readers a
        // default run is silent. This is the assertion those pages rest on.
        //
        // Only `violates` is checked: `legal` and `subsumed_by` already assert
        // the code is absent, so asserting it again under weaker rules says
        // nothing new.
        if matches!(entry.claim, Claim::Violates) && !entry.rules.is_default() {
            let default_errors = ErrorCollector::new();
            let mut default_codes: Vec<String> = Vec::new();
            let parse_errors = ErrorCollector::new();
            if let ParseOutcome::Parsed(mut chat_file) =
                parser.parse_chat_file_fragment(&content, 0, &parse_errors)
            {
                chat_file.validate_with_alignment(
                    &default_errors,
                    TranscriptName::for_path(&dir.join(&entry.fixture)),
                );
                default_codes.extend(
                    parse_errors
                        .to_vec()
                        .iter()
                        .chain(default_errors.to_vec().iter())
                        .map(|e| e.code.to_string()),
                );
            }
            if default_codes.iter().any(|got| got == entry.code.as_str()) {
                failures.push(format!(
                    "{}: {} is declared to need {} but a DEFAULT run reports it \
                     anyway (got {:?}). Either the rule is no longer opt-in, in \
                     which case drop `rules` from its registry entry, or a guard \
                     was lost. Every generated page for this code currently \
                     tells readers a default run does not report it. [{}]",
                    entry.fixture,
                    entry.code.as_str(),
                    entry
                        .rules
                        .cli_flag()
                        .unwrap_or("an option this build does not name"),
                    default_codes,
                    entry.source_spec
                ));
            }
        }

        if satisfied {
            println!("  ✓ {} → {} ({})", wants, entry.fixture, codes.join(", "));
        } else {
            failures.push(format!(
                "{} (claim wants {}, got {:?}) [{}]",
                entry.fixture, wants, codes, entry.source_spec
            ));
            println!(
                "  ✗ {} → {:?} (claim wants {}) [{}]",
                entry.fixture, codes, wants, entry.source_spec
            );
        }
    }

    println!("\nskipped (status not `implemented`): {skipped}");

    // Hard coverage gate: every implemented validation spec must contribute at
    // least one example, so a newly-implemented spec cannot silently ship without
    // a test. The generator records any offenders in the manifest; a
    // non-empty list fails the run alongside any fixture mismatches above.
    let coverage_gaps = &manifest.implemented_codes_without_examples;

    // Collect each non-empty failure category as its own section, then join.
    let mut sections = Vec::new();
    if !failures.is_empty() {
        sections.push(format!(
            "{} validation fixtures did not produce their expected codes:\n  {}",
            failures.len(),
            failures.join("\n  ")
        ));
    }
    if !coverage_gaps.is_empty() {
        sections.push(format!(
            "{} implemented codes have no triggering example in any spec (add a triggering \
             example; or Status: not_implemented with a reason; or, only when \
             no CHAT input can reach the rule at all, Status: \
             unreachable_from_chat naming its out-of-corpus test):\n  {}",
            coverage_gaps.len(),
            // Typed now that the manifest is the generator's own struct rather
            // than a `Vec<String>` mirror: rendered here, never re-parsed.
            coverage_gaps
                .iter()
                .map(SpecErrorCode::as_str)
                .collect::<Vec<_>>()
                .join("\n  ")
        ));
    }
    // The converse of the escape hatch: an `unreachable_from_chat` spec that
    // has an example is reachable, so the status is wrong. Without this the
    // new state would be a way to opt any rule out of its fixture.
    let mislabelled = &manifest.unreachable_specs_with_examples;
    if !mislabelled.is_empty() {
        sections.push(format!(
            "{} specs marked unreachable_from_chat carry an example, so CHAT \
             input does reach them and the status is wrong:\n  {}",
            mislabelled.len(),
            mislabelled
                .iter()
                .map(RepoRelativePath::as_str)
                .collect::<Vec<_>>()
                .join("\n  ")
        ));
    }
    if !sections.is_empty() {
        return Err(TestError::Failure(sections.join("\n")));
    }

    println!("\n✓ All implemented validation fixtures produced their expected codes");
    println!("✓ Every implemented validation spec contributes at least one example");
    Ok(())
}
