//! Filesystem-boundary tests: admission must never turn missing evidence into
//! an empty or partial successful measurement.

use talkbank_parser_tests::chat_corpus::ChatCorpus;

#[test]
fn missing_and_empty_corpora_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    assert!(ChatCorpus::read(&root.path().join("missing")).is_err());
    assert!(ChatCorpus::read(root.path()).is_err());
    std::fs::write(root.path().join("README.md"), "No CHAT fixtures")?;
    assert!(ChatCorpus::read(root.path()).is_err());
    Ok(())
}

#[test]
fn unreadable_source_rejects_the_whole_corpus() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("a.cha"), "readable source")?;
    std::fs::write(root.path().join("b.cha"), [0xff])?;
    let error = match ChatCorpus::read(root.path()) {
        Ok(_) => return Err("invalid UTF-8 admitted as a partial corpus".into()),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("b.cha"), "{error}");
    Ok(())
}

#[test]
fn nested_sources_are_bound_to_sorted_paths() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    std::fs::create_dir(root.path().join("nested"))?;
    std::fs::write(root.path().join("nested/b.CHA"), "second")?;
    std::fs::write(root.path().join("a.cha"), "first")?;
    let corpus = ChatCorpus::read(root.path())?;
    let fixtures = corpus.fixtures();
    assert_eq!(fixtures.len(), 2);
    assert_eq!(fixtures[0].path(), root.path().join("a.cha"));
    assert_eq!(fixtures[0].source(), "first");
    assert_eq!(fixtures[1].path(), root.path().join("nested/b.CHA"));
    assert_eq!(fixtures[1].source(), "second");
    Ok(())
}
