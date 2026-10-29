//! Operational filesystem contracts; empty files are not CHAT specimens.

mod discovery {
    use std::path::Path;
    use talkbank_transform::paths::{
        FoundFile, FoundTranscript, Links, StoredTranscript, expand_transcript_arguments,
        is_chat_transcript_path, walk_files, walk_transcripts,
    };

    /// The walk keeps transcripts at every depth with their relative paths
    /// and skips AppleDouble sidecars.
    #[test]
    fn walk_finds_transcripts_with_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub/deeper")).unwrap();
        for name in [
            "a.cha",
            "._a.cha",
            "notes.txt",
            "sub/b.cha",
            "sub/deeper/c.cha",
        ] {
            std::fs::write(root.join(name), "").unwrap();
        }
        let walk = walk_transcripts(root);
        assert!(walk.failures().is_empty(), "{:?}", walk.failures());
        let relative: Vec<_> = walk.found().iter().map(FoundTranscript::relative).collect();
        assert_eq!(
            relative,
            [
                Path::new("a.cha"),
                Path::new("sub/b.cha"),
                Path::new("sub/deeper/c.cha")
            ]
        );
        for found in walk.found() {
            assert_eq!(found.path(), root.join(found.relative()));
        }
    }

    /// A root that cannot be read is a failure in the result, not an empty
    /// corpus.
    #[test]
    fn an_unreadable_root_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let walk = walk_transcripts(&dir.path().join("missing"));
        assert!(walk.found().is_empty());
        assert_eq!(walk.failures().len(), 1);
    }

    /// A file argument is kept whatever its extension, a directory argument
    /// contributes its transcripts, and a missing argument is a failure.
    #[test]
    fn arguments_expand_files_directories_and_report_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("corpus")).unwrap();
        std::fs::write(root.join("corpus/a.cha"), "").unwrap();
        std::fs::write(root.join("named.txt"), "").unwrap();
        let expanded = expand_transcript_arguments(&[
            root.join("named.txt"),
            root.join("corpus"),
            root.join("missing.cha"),
        ]);
        let files: Vec<_> = expanded
            .files()
            .iter()
            .map(StoredTranscript::path)
            .collect();
        assert_eq!(files, [root.join("corpus/a.cha"), root.join("named.txt")]);
        assert_eq!(expanded.failures().len(), 1);
        assert_eq!(expanded.failures()[0].path, root.join("missing.cha"));
    }

    /// A transcript named by a file argument and again by its directory is
    /// one transcript, however the argument spells it (`corpus/sub/../a.cha`
    /// included): the dedup compares resolved locations, not spellings.
    #[test]
    fn a_transcript_named_twice_is_expanded_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("corpus/sub")).unwrap();
        std::fs::write(root.join("corpus/a.cha"), "").unwrap();
        let expanded = expand_transcript_arguments(&[
            root.join("corpus/a.cha"),
            root.join("corpus"),
            root.join("corpus/sub/../a.cha"),
        ]);
        let files: Vec<_> = expanded
            .files()
            .iter()
            .map(StoredTranscript::path)
            .collect();
        assert_eq!(files, [root.join("corpus/a.cha")]);
    }

    /// A link cycle is walked once, and a linked transcript is found.
    #[cfg(unix)]
    #[test]
    fn a_link_cycle_is_walked_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/a.cha"), "").unwrap();
        std::os::unix::fs::symlink(root, root.join("sub/loop")).unwrap();
        let walk = walk_transcripts(root);
        assert!(walk.failures().is_empty(), "{:?}", walk.failures());
        assert_eq!(walk.found().len(), 1);
    }

    /// `Links::Skip` neither walks into a linked directory nor keeps a
    /// linked file; `Links::Follow` does both.
    #[cfg(unix)]
    #[test]
    fn skipped_links_are_neither_walked_nor_kept() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("x.cha"), "").unwrap();
        std::fs::write(root.join("own.cha"), "").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
        std::os::unix::fs::symlink(outside.join("x.cha"), root.join("y.cha")).unwrap();
        let skipped = walk_files(&root, Links::Skip, is_chat_transcript_path);
        let skipped: Vec<_> = skipped.found().iter().map(FoundFile::relative).collect();
        assert_eq!(skipped, [Path::new("own.cha")]);
        let followed = walk_transcripts(&root);
        let followed: Vec<_> = followed
            .found()
            .iter()
            .map(FoundTranscript::relative)
            .collect();
        assert_eq!(
            followed,
            [
                Path::new("linked/x.cha"),
                Path::new("own.cha"),
                Path::new("y.cha")
            ]
        );
    }

    /// Root policy applies before directory lookup, including spellings that
    /// would make the operating system follow a final directory link.
    #[cfg(unix)]
    #[test]
    fn root_links_are_skipped_before_traversal_but_followed_when_requested() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("a.cha"), "").unwrap();
        let linked = dir.path().join("linked");
        std::os::unix::fs::symlink(&target, &linked).unwrap();
        for root in [linked.clone(), linked.join(""), linked.join(".")] {
            let calls = std::cell::Cell::new(0);
            let skipped = walk_files(&root, Links::Skip, |_| {
                calls.set(calls.get() + 1);
                true
            });
            assert!(skipped.found().is_empty());
            assert!(skipped.failures().is_empty());
            assert_eq!(calls.get(), 0, "skipping must not offer target entries");
            let followed = walk_files(&root, Links::Follow, is_chat_transcript_path);
            assert!(followed.failures().is_empty());
            assert_eq!(followed.found().len(), 1);
            assert_eq!(followed.found()[0].relative(), Path::new("a.cha"));
        }
        for (name, target) in [
            ("file-link", target.join("a.cha")),
            ("dangling", dir.path().join("missing")),
        ] {
            let root = dir.path().join(name);
            std::os::unix::fs::symlink(target, &root).unwrap();
            let skipped = walk_files(&root, Links::Skip, |_| panic!("skipped root"));
            assert!(skipped.found().is_empty());
            assert!(skipped.failures().is_empty());
            let followed = walk_files(&root, Links::Follow, |_| true);
            assert!(followed.found().is_empty());
            assert_eq!(followed.failures().len(), 1);
        }
    }

    /// A dangling link is a failure whatever its name: a link to an
    /// unmounted subcorpus (`Eng -> /unavailable/Eng`) hides every
    /// transcript under it, so it can never vanish from a run in silence.
    #[cfg(unix)]
    #[test]
    fn a_dangling_link_fails_whatever_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::os::unix::fs::symlink(root.join("gone/Eng"), root.join("Eng")).unwrap();
        std::os::unix::fs::symlink(root.join("gone.cha"), root.join("a.cha")).unwrap();
        let walk = walk_transcripts(root);
        let mut failed: Vec<_> = walk
            .failures()
            .iter()
            .map(|failure| &failure.path)
            .collect();
        failed.sort();
        assert_eq!(failed, [&root.join("Eng"), &root.join("a.cha")]);
    }
}

mod resolved_identity {
    use std::ffi::OsStr;
    use talkbank_model::{ResolvedDirectory, ResolvedPath, ResolvedPrefix};

    /// Native component iteration normalizes trailing separators and `.`.
    /// That normalization must not admit a directory spelling as a file name.
    #[test]
    fn file_identity_recovery_refuses_normalized_non_leaf_names() {
        let directory = tempfile::tempdir().unwrap();
        let resolved = ResolvedDirectory::resolve(directory.path()).unwrap();
        let mut admitted = Vec::new();
        for name in ["", ".", "..", "a/b", "a/", "a//", "a/."] {
            match resolved.file(OsStr::new(name)) {
                Ok(path) => admitted.push((name, path)),
                Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput),
            }
        }
        assert!(
            admitted.is_empty(),
            "non-leaf spellings admitted: {admitted:?}"
        );
        let name = OsStr::new("transcript.cha");
        assert_eq!(
            resolved.file(name).unwrap().as_path(),
            resolved.as_path().join(name)
        );
        for suffix in ["/", "//", "/."] {
            let mut spelling = directory.path().join(name).into_os_string();
            spelling.push(suffix);
            let path = std::path::Path::new(&spelling);
            assert_eq!(
                ResolvedPath::of_file(path).unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput,
                "directory spelling must not become a file location: {path:?}"
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let name = OsStr::from_bytes(b"transcript-\xff.cha");
            assert_eq!(
                ResolvedPath::of_file(&directory.path().join(name)).unwrap(),
                resolved.file(name).unwrap(),
                "native file names do not require Unicode"
            );
        }
        #[cfg(windows)]
        for suffix in ["\\", "\\\\", "\\."] {
            let mut spelling = directory.path().join(name).into_os_string();
            spelling.push(suffix);
            assert_eq!(
                ResolvedPath::of_file(std::path::Path::new(&spelling))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
    }

    /// A real transcript is a file, never evidence for a directory owner.
    /// The same refusal applies to a link whose target is that file.
    #[test]
    fn file_identity_recovery_refuses_file_directory_owners() {
        let directory = tempfile::tempdir().unwrap();
        let transcript = directory.path().join("basic-conversation.cha");
        std::fs::copy(
            talkbank_parser_tests::repo_paths::workspace_root()
                .join("corpus/reference/core/basic-conversation.cha"),
            &transcript,
        )
        .unwrap();
        let mut paths = vec![transcript.clone()];
        #[cfg(unix)]
        {
            let link = directory.path().join("linked.cha");
            std::os::unix::fs::symlink(&transcript, &link).unwrap();
            paths.push(link);
        }
        let mut admitted = Vec::new();
        for path in paths {
            match ResolvedDirectory::resolve(&path) {
                Ok(value) => admitted.push(value),
                Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotADirectory),
            }
        }
        assert!(
            admitted.is_empty(),
            "file directory owners admitted: {admitted:?}"
        );
        assert!(ResolvedPath::of_file(&transcript).is_ok());
        assert!(ResolvedPrefix::of(&transcript).is_ok());
    }

    /// Every spelling of one file resolves to one path: absolute, through
    /// `..`, and through a linked directory. Its own name is kept, so a link
    /// to a file is a different location.
    #[test]
    fn spellings_of_one_file_resolve_to_one_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("corpus/sub")).unwrap();
        std::fs::write(root.join("corpus/a.cha"), "").unwrap();
        let direct = ResolvedPath::of_file(&root.join("corpus/a.cha")).unwrap();
        assert_eq!(
            ResolvedPath::of_file(&root.join("corpus/sub/../a.cha")).unwrap(),
            direct
        );
        assert!(direct.as_path().is_absolute());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("corpus"), root.join("linked")).unwrap();
            assert_eq!(
                ResolvedPath::of_file(&root.join("linked/a.cha")).unwrap(),
                direct
            );
            std::os::unix::fs::symlink(root.join("corpus/a.cha"), root.join("corpus/b.cha"))
                .unwrap();
            let link = ResolvedPath::of_file(&root.join("corpus/b.cha")).unwrap();
            assert_ne!(link, direct, "a link keeps its own name");
            assert_eq!(link.as_path().file_name(), Some(OsStr::new("b.cha")));
        }
    }

    /// A prefix names an existing directory (resolved whole) or anything
    /// else (resolved as a file), and a missing directory is resolved
    /// through its deepest existing ancestor.
    #[test]
    fn prefixes_resolve_directories_files_and_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("corpus")).unwrap();
        let resolved_root = ResolvedDirectory::resolve(root).unwrap();
        assert_eq!(
            ResolvedPrefix::of(&root.join("corpus")).unwrap().as_path(),
            resolved_root.as_path().join("corpus")
        );
        assert_eq!(
            ResolvedPrefix::of(&root.join("gone/deeper"))
                .unwrap()
                .as_path(),
            resolved_root.as_path().join("gone/deeper")
        );
        for spelling in ["gone/deeper/", "gone/deeper//", "gone/deeper/."] {
            assert_eq!(
                ResolvedPrefix::of(&root.join(spelling)).unwrap().as_path(),
                resolved_root.as_path().join("gone/deeper"),
                "missing directory prefixes retain their intended cache scope"
            );
        }
        assert!(resolved_root.file(OsStr::new("a/b")).is_err());
        assert!(resolved_root.file(OsStr::new("..")).is_err());
    }

    /// Native path semantics are external facts, not guaranteed by a proof
    /// type: POSIX retains a missing `..`; ordinary Windows paths normalize it.
    #[test]
    fn missing_parent_components_follow_native_path_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gone").join("..").join("x");
        #[cfg(unix)]
        assert!(ResolvedPrefix::of(&path).is_err());
        #[cfg(windows)]
        assert_eq!(
            ResolvedPrefix::of(&path).unwrap().as_path(),
            ResolvedDirectory::resolve(dir.path())
                .unwrap()
                .as_path()
                .join("x")
        );
    }
}
