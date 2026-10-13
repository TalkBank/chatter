//! Authored CHAT travels from public discovery to named, owned admission.

use std::path::{Path, PathBuf};
use talkbank_model::{NullErrorSink, Severity, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::parse_source_with_parser;
use talkbank_transform::paths::{
    Links, StoredTranscript, expand_transcript_arguments, is_chat_transcript_path, walk_files,
    walk_transcripts,
};

const SOURCE: &str = include_str!("../error_corpus/validation_errors/W109_4.cha");
const NAME: &str = "Schlu\u{0308}ssel.cha";
const EXPLICIT_NAME: &str = "Schlu\u{0308}ssel.txt";

fn admit_discovered(stored: StoredTranscript, parser: &TreeSitterParser) -> PathBuf {
    let source = std::fs::read_to_string(stored.path()).expect("discovered authored CHAT");
    assert_eq!(source, SOURCE);
    let admitted = parse_source_with_parser(parser, &source)
        .admit(stored.name(), &NullErrorSink)
        .expect("canonical equivalence warns, it does not refuse admission");
    assert_eq!(admitted.source(), SOURCE);
    assert_eq!(admitted.document().to_chat_string(), SOURCE);
    let owned = admitted.into_owned();
    drop(source);
    assert_eq!(owned.source(), SOURCE);
    let valid = owned.into_valid_file();
    assert_eq!(
        valid.name(),
        stored.name(),
        "stored spelling reaches admission"
    );
    assert_eq!(valid.to_chat_string(), SOURCE);
    let [warning] = valid.diagnostics() else {
        panic!("the decomposed stored name must retain exactly its W109 warning");
    };
    assert_eq!(warning.code.as_str(), "W109");
    assert_eq!(warning.severity, Severity::Warning);
    stored.into_path()
}

#[test]
fn media_spec_discovery_preserves_named_admission_and_consumes_each_location_once() {
    let directory = tempfile::tempdir().expect("isolated authored-input workflow");
    let root = directory.path().join("inputs");
    let mut physical = vec![
        PathBuf::from(NAME),
        Path::new("sub").join(NAME),
        Path::new("sub/deep").join(NAME),
    ];
    for relative in &physical {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("fixture directories");
        std::fs::write(path, SOURCE).expect("copy authored control unchanged");
    }
    // These contain the same CHAT, but directory selection must not admit
    // sidecars or infer that a non-.cha file was explicitly requested.
    std::fs::write(root.join(format!("._{NAME}")), SOURCE).expect("sidecar control");
    std::fs::write(root.join(EXPLICIT_NAME), SOURCE).expect("explicit-extension control");
    physical.sort();
    let mut followed = physical.clone();
    #[cfg(unix)]
    {
        let outside = directory.path().join("outside");
        std::fs::create_dir_all(&outside).expect("linked fixture directory");
        std::fs::write(outside.join(NAME), SOURCE).expect("authored linked control");
        std::fs::create_dir_all(root.join("links")).expect("link parent");
        std::os::unix::fs::symlink(&outside, root.join("external")).expect("directory link");
        std::os::unix::fs::symlink(root.join(NAME), root.join("links").join(NAME))
            .expect("file link retains its own location");
        std::os::unix::fs::symlink(&root, root.join("sub/loop"))
            .expect("cycle back to the already visited root");
        followed.extend([
            Path::new("external").join(NAME),
            Path::new("links").join(NAME),
        ]);
    }
    followed.sort();

    let skipped = walk_files(&root, Links::Skip, is_chat_transcript_path);
    assert!(skipped.failures().is_empty(), "{:?}", skipped.failures());
    assert_eq!(
        skipped
            .found()
            .iter()
            .map(|f| f.relative().to_path_buf())
            .collect::<Vec<_>>(),
        physical
    );
    let (skipped, failures) = skipped.into_parts();
    assert!(failures.is_empty());
    for found in skipped {
        assert_eq!(found.path(), root.join(found.relative()));
    }

    let walk = walk_transcripts(&root);
    assert!(walk.failures().is_empty(), "{:?}", walk.failures());
    assert_eq!(
        walk.found()
            .iter()
            .map(|f| f.relative().to_path_buf())
            .collect::<Vec<_>>(),
        followed
    );
    for found in walk.found() {
        assert_eq!(found.path(), root.join(found.relative()));
        assert_eq!(found.stored().path(), found.path());
    }

    let expanded = expand_transcript_arguments(&[
        root.join(NAME),
        root.clone(),
        root.join("sub/deep/..").join(NAME),
        root.join(EXPLICIT_NAME),
    ]);
    assert!(expanded.failures().is_empty(), "{:?}", expanded.failures());
    assert_eq!(expanded.files().len(), followed.len() + 1);
    let (distinct, failures) = expanded.into_parts();
    assert!(failures.is_empty());
    assert!(!distinct.is_empty());
    assert_eq!(distinct.len(), distinct.as_slice().len());
    let parser = TreeSitterParser::new().expect("grammar loads");
    let admitted: Vec<_> = distinct
        .into_iter()
        .map(|stored| admit_discovered(stored, &parser))
        .collect();
    let mut expected: Vec<_> = followed
        .into_iter()
        .map(|relative| root.join(relative))
        .collect();
    expected.push(root.join(EXPLICIT_NAME));
    expected.sort();
    assert_eq!(
        admitted, expected,
        "every distinct discovered location is admitted once"
    );
    assert_eq!(
        std::fs::read_to_string(root.join(NAME)).expect("unchanged input"),
        SOURCE
    );
}

fn record_nonfile_refusal(
    label: &str,
    result: std::io::Result<StoredTranscript>,
    failures: &mut Vec<String>,
) {
    match result {
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => failures.push(format!("{label}: wrong refusal category: {error}")),
        Ok(stored) => failures.push(format!("{label}: admitted {}", stored.path().display())),
    }
}

#[test]
fn media_spec_directory_entries_are_not_stored_transcripts() {
    let directory = tempfile::tempdir().expect("isolated authored-input workflow");
    let root = directory.path();
    let regular = root.join(NAME);
    std::fs::write(&regular, SOURCE).expect("unchanged authored regular-file control");
    let not_file = root.join("directory.cha");
    std::fs::create_dir(&not_file).expect("directory with a transcript-like name");
    let entry = std::fs::read_dir(root)
        .expect("directory")
        .map(|entry| entry.expect("read entry"))
        .find(|entry| entry.path() == not_file)
        .expect("native directory entry");
    let mut failures = Vec::new();
    record_nonfile_refusal(
        "direct directory",
        StoredTranscript::resolve(&not_file),
        &mut failures,
    );
    record_nonfile_refusal(
        "directory entry",
        StoredTranscript::from_entry(entry),
        &mut failures,
    );

    // Directory arguments still mean recursive discovery, not file admission.
    let (files, discovery_failures) =
        expand_transcript_arguments(&[root.to_path_buf()]).into_parts();
    assert!(discovery_failures.is_empty(), "{discovery_failures:?}");
    assert_eq!(files.len(), 1);
    let parser = TreeSitterParser::new().expect("grammar loads");
    for stored in files {
        assert_eq!(admit_discovered(stored, &parser), regular);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[cfg(unix)]
#[test]
fn media_spec_special_entries_are_refused_before_content_io() {
    let directory = tempfile::tempdir().expect("isolated authored-input workflow");
    let root = directory.path();
    let regular = root.join(NAME);
    std::fs::write(&regular, SOURCE).expect("unchanged authored regular-file control");
    // Actual kernel entries, never malformed CHAT or a blocking FIFO read.
    let special = root.join("p.cha");
    let _socket = std::os::unix::net::UnixListener::bind(&special).expect("native socket entry");
    let link = root.join("q.cha");
    std::os::unix::fs::symlink(&special, &link).expect("link to non-regular entry");
    let _ignored = std::os::unix::net::UnixListener::bind(root.join("ignored.txt"))
        .expect("filtered non-transcript control");
    let mut failures = Vec::new();
    let mut resolver = talkbank_transform::paths::StoredNameResolver::default();
    for path in [&special, &link] {
        record_nonfile_refusal(
            "direct special entry",
            StoredTranscript::resolve(path),
            &mut failures,
        );
        record_nonfile_refusal(
            "resolver special entry",
            resolver.resolve(path),
            &mut failures,
        );
        let entry = std::fs::read_dir(root)
            .expect("directory")
            .map(|entry| entry.expect("read entry"))
            .find(|entry| entry.path() == *path)
            .expect("native selected entry");
        record_nonfile_refusal(
            "listed special entry",
            StoredTranscript::from_entry(entry),
            &mut failures,
        );
    }
    for (links, mut expected) in [
        (Links::Skip, vec![special.clone()]),
        (Links::Follow, vec![special.clone(), link.clone()]),
    ] {
        let walk = walk_files(root, links, is_chat_transcript_path);
        let found: Vec<_> = walk.found().iter().map(|file| file.path()).collect();
        if found != [&regular] {
            failures.push(format!(
                "{links:?}: non-files reached the file walk: {found:?}"
            ));
        }
        let mut refused: Vec<_> = walk
            .failures()
            .iter()
            .map(|failure| failure.path.clone())
            .collect();
        refused.sort();
        expected.sort();
        if refused != expected {
            failures.push(format!(
                "{links:?}: refusals {refused:?}, expected {expected:?}"
            ));
        }
        for refusal in walk.failures() {
            assert_eq!(refusal.error.kind(), std::io::ErrorKind::InvalidInput);
        }
    }
    let expanded = expand_transcript_arguments(&[special.clone(), link.clone(), regular.clone()]);
    if expanded.failures().len() != 2 {
        failures.push(format!(
            "argument expansion reported {} refusals instead of two",
            expanded.failures().len()
        ));
    }
    let files: Vec<_> = expanded
        .files()
        .iter()
        .map(StoredTranscript::path)
        .collect();
    if files != [&regular] {
        failures.push(format!("argument expansion admitted non-files: {files:?}"));
    }
    let walk = walk_transcripts(root);
    if walk.failures().len() != 2 || walk.found().len() != 1 {
        failures.push(format!(
            "transcript walk: {} files, {} refusals",
            walk.found().len(),
            walk.failures().len()
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let parser = TreeSitterParser::new().expect("grammar loads");
    for found in walk.into_parts().0 {
        assert_eq!(admit_discovered(found.into_stored(), &parser), regular);
    }

    // The same expanded input that the CLI hands off reaches the real runner:
    // refused objects have no CHAT diagnostics, while the valid sibling runs.
    use talkbank_transform::validation_runner::{
        FileStatus, RunEnding, ValidationConfig, ValidationEvent, validate_arguments_streaming,
    };
    let config = ValidationConfig {
        jobs: Some(std::num::NonZeroUsize::MIN),
        ..Default::default()
    };
    let (events, _cancel) = validate_arguments_streaming(
        expanded,
        &talkbank_transform::ValidationRun::uncached(config),
    );
    let mut completed = std::collections::BTreeMap::new();
    let mut finished = false;
    for event in events {
        assert!(!finished, "completion must be terminal");
        match event {
            ValidationEvent::Discovering => {}
            ValidationEvent::Started { total_files } => assert_eq!(total_files, 3),
            ValidationEvent::FileComplete(event) => {
                assert!(completed.insert(event.path, event.status).is_none());
            }
            ValidationEvent::Finished(RunEnding::Complete(stats)) => {
                let snapshot = stats.snapshot();
                assert_eq!(snapshot.total_files().get(), 3);
                assert_eq!(snapshot.valid_files(), 1);
                assert_eq!(snapshot.failed_files(), 2);
                assert_eq!(snapshot.internal_failures(), 0);
                finished = true;
            }
            other => panic!("unexpected discovery/refusal event: {other:?}"),
        }
    }
    assert!(finished);
    assert_eq!(completed.len(), 3);
    assert!(matches!(
        completed.remove(&regular),
        Some(FileStatus::Valid { .. })
    ));
    for path in [special, link] {
        let status = completed
            .remove(&path)
            .expect("one refusal for each native object");
        assert!(status.shown().is_none(), "I/O refusal has no CHAT source");
        assert!(matches!(status, FileStatus::ReadError { message } if !message.is_empty()));
    }
}
