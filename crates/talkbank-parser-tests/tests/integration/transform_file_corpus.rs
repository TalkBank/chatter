//! Filesystem boundary contracts, using canonical CHAT rather than synthetic files.

use talkbank_model::ParseValidateOptions;
use talkbank_model::model::{SemanticEq, TranscriptName};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;
use talkbank_transform::{PipelineError, parse_and_validate_named, parse_file_and_validate};

#[test]
fn reference_files_preserve_parsed_models() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical reference corpus");
    for fixture in corpus.fixtures() {
        let expected = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("reference parses cleanly");
        let actual = parse_file_and_validate(fixture.path(), ParseValidateOptions::default())
            .expect("reference file parses cleanly");
        assert!(
            expected.semantic_eq(&actual),
            "{}",
            fixture.path().display()
        );
    }
}

#[test]
fn spec_files_preserve_named_admission_and_refusal_evidence() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical spec corpus");
    let mut outcomes = [0; 3];
    for fixture in corpus.fixtures() {
        // This is the storage-name boundary, not the manifest-authored name
        // used by the separate specification-claim runner.
        let name = TranscriptName::for_path(fixture.path());
        for options in [
            ParseValidateOptions::default(),
            ParseValidateOptions::default().with_validation(),
            ParseValidateOptions::default()
                .with_alignment()
                .with_strict_linkers(),
        ] {
            let expected =
                parse_and_validate_named(&parser, fixture.source(), options.clone(), name);
            let actual = parse_file_and_validate(fixture.path(), options.clone());
            match (expected, actual) {
                (Ok(expected), Ok(actual)) => {
                    assert!(
                        expected.semantic_eq(&actual),
                        "{}",
                        fixture.path().display()
                    );
                    outcomes[0] += 1;
                }
                (Err(PipelineError::Parse(expected)), Err(PipelineError::Parse(actual))) => {
                    assert_eq!(expected.errors, actual.errors);
                    outcomes[1] += 1;
                }
                (
                    Err(PipelineError::Validation(expected)),
                    Err(PipelineError::Validation(actual)),
                ) => {
                    assert_eq!(expected, actual);
                    outcomes[2] += 1;
                }
                (
                    Err(PipelineError::IncompleteValidation(expected)),
                    Err(PipelineError::IncompleteValidation(actual)),
                ) => {
                    assert_eq!(expected.name(), name);
                    assert_eq!(actual.name(), name);
                    assert_eq!(expected.policy(), actual.policy());
                    assert_eq!(expected.diagnostics(), actual.diagnostics());
                    assert_eq!(
                        expected.has_incomplete_parse(),
                        actual.has_incomplete_parse()
                    );
                    assert!(expected.document().semantic_eq(actual.document()));
                    outcomes[2] += 1;
                }
                (expected, actual) => panic!(
                    "file boundary differs: {} {options:?}: {expected:?} / {actual:?}",
                    fixture.path().display()
                ),
            }
        }
    }
    assert!(
        outcomes.iter().all(|count| *count > 0),
        "must witness admission and parse/validation refusals: {outcomes:?}"
    );
}

#[test]
fn fixture_directory_is_an_io_refusal_not_empty_chat() {
    let directory = workspace_root().join("corpus/reference");
    assert!(
        directory.is_dir(),
        "canonical reference directory must exist"
    );
    assert!(matches!(
        parse_file_and_validate(&directory, ParseValidateOptions::default()),
        Err(PipelineError::Io(_))
    ));
}

#[cfg(unix)]
#[test]
fn media_spec_stored_identity_survives_argument_aliases_and_directory_changes() {
    use talkbank_model::model::FileStem;
    use talkbank_transform::paths::{StoredNameResolver, StoredTranscript};
    let source = std::fs::read(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/W109_4.cha"),
    )
    .expect("canonical media control");
    let directory = tempfile::tempdir().expect("isolated filesystem boundary");
    let stored_path = directory.path().join("Schlu\u{0308}ssel.cha");
    std::fs::write(&stored_path, &source).expect("write canonical bytes");
    let entry = std::fs::read_dir(directory.path())
        .expect("directory")
        .next()
        .expect("entry")
        .expect("read entry");
    let admitted = StoredTranscript::from_entry(entry).expect("directory entry admission");
    assert_eq!(admitted.path(), stored_path);
    assert_eq!(
        admitted.name(),
        TranscriptName::Named(FileStem::from_stem("Schlu\u{0308}ssel"))
    );
    // Establish an old directory timestamp without a timing-dependent sleep.
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    std::fs::File::open(directory.path())
        .expect("directory handle")
        .set_times(std::fs::FileTimes::new().set_modified(old))
        .expect("directory timestamp");
    let mut resolver = StoredNameResolver::default();
    for spelling in ["Schlu\u{0308}ssel.cha", "Schlüssel.cha", "SCHLüSSEL.cha"] {
        let requested = directory.path().join(spelling);
        if requested.exists() {
            let resolved = resolver
                .resolve(&requested)
                .expect("existing normalized alias");
            assert_eq!(resolved.path(), stored_path);
            assert_eq!(resolved.name(), admitted.name());
        } else {
            assert_eq!(
                resolver
                    .resolve(&requested)
                    .expect_err("not a filesystem alias")
                    .kind(),
                std::io::ErrorKind::NotFound
            );
        }
    }
    // A differently named file cannot reuse the old directory snapshot.
    let renamed = directory.path().join("other.cha");
    std::fs::rename(&stored_path, &renamed).expect("rename test copy");
    let resolved = resolver
        .resolve(&renamed)
        .expect("refresh changed directory");
    assert_eq!(resolved.path(), renamed);
    assert_eq!(
        resolved.name(),
        TranscriptName::Named(FileStem::from_stem("other"))
    );
    assert_eq!(
        std::fs::read(resolved.path()).expect("unchanged bytes"),
        source
    );
    assert_eq!(
        resolver
            .resolve(&stored_path)
            .expect_err("removed name is not cached admission")
            .kind(),
        std::io::ErrorKind::NotFound
    );
}

#[cfg(target_os = "linux")]
#[test]
fn media_spec_non_utf8_disk_names_refuse_instead_of_becoming_anonymous() {
    use std::os::unix::ffi::OsStringExt;
    use talkbank_transform::paths::StoredTranscript;
    let source = std::fs::read(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/W109_4.cha"),
    )
    .expect("canonical media control");
    let directory = tempfile::tempdir().expect("isolated filesystem boundary");
    let path = directory
        .path()
        .join(std::ffi::OsString::from_vec(b"name\xff.cha".to_vec()));
    std::fs::write(&path, source).expect("Linux permits non-UTF-8 filenames");
    assert_eq!(
        StoredTranscript::resolve(&path)
            .expect_err("requested name refusal")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    let entry = std::fs::read_dir(directory.path())
        .expect("directory")
        .next()
        .expect("entry")
        .expect("read entry");
    assert_eq!(
        StoredTranscript::from_entry(entry)
            .expect_err("stored name refusal")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert!(matches!(
        parse_file_and_validate(&path, ParseValidateOptions::default().with_validation()),
        Err(PipelineError::Io(_))
    ));
}

#[cfg(unix)]
#[test]
fn media_spec_symlink_keeps_its_own_named_validation_context() {
    use talkbank_transform::paths::StoredTranscript;
    let target = workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/W109_4.cha");
    let directory = tempfile::tempdir().expect("isolated filesystem boundary");
    let link = directory.path().join("Schlüssel.cha");
    std::os::unix::fs::symlink(&target, &link).expect("transcript link");
    let stored = StoredTranscript::resolve(&link).expect("resolve link name");
    assert_eq!(
        stored.path(),
        link,
        "do not canonicalize to the target's basename"
    );
    parse_file_and_validate(&link, ParseValidateOptions::default().with_validation())
        .expect("media matches the link name, not W109_4");
    std::fs::remove_file(&link).expect("remove test link only");
    assert!(
        target.exists(),
        "canonical fixture survives removal of the link"
    );
    assert!(matches!(
        parse_file_and_validate(&link, ParseValidateOptions::default()),
        Err(PipelineError::Io(_))
    ));
    assert_eq!(
        StoredTranscript::resolve(std::path::Path::new("/"))
            .expect_err("no transcript basename")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
}
