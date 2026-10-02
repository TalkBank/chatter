//! Transcript paths: which files are CHAT transcripts, how a transcript's
//! stored name is resolved, and the one directory walk and command-line
//! expansion every command uses to find its input.
//!
//! - [`is_chat_transcript_path`] classifies a path (`.cha`, not an
//!   AppleDouble `._` sidecar).
//! - [`StoredTranscript`] / [`StoredNameResolver`] resolve the name a
//!   transcript is stored under.
//! - [`walk_files`] walks a directory: every file found with its
//!   root-relative path, and every entry that could not be read ([`Walk`]).
//!   [`walk_transcripts`] does the same for transcripts, each found one
//!   already a [`StoredTranscript`] ([`TranscriptWalk`]): its name came from
//!   the directory listing, so nothing lists the directory again to learn it.
//! - [`expand_transcript_arguments`] expands command-line paths to stored
//!   transcripts ([`ExpandedArguments`]), resolving each file argument's
//!   stored name once.
//!
//! Lives at the crate root, OUTSIDE the `validation-runner` feature: the
//! corpus manifest walk and the CLI-side walks need it on every build,
//! including `default-features = false` consumers that opt out of the
//! validation runner entirely.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::Path;

pub use talkbank_model::{ResolvedDirectory, ResolvedPath};

/// A transcript path whose basename was obtained from its parent directory,
/// rather than trusted from an argument's spelling, with its
/// [`ResolvedPath`]: the identity the cache keys it by, `--force` clears by,
/// and a run counts it once by. Symlink names are retained.
///
/// Every constructor resolves the path when it admits the transcript, so a
/// transcript whose directory cannot be resolved is refused here, as an
/// unreadable input, and no consumer meets the failure later.
#[derive(Debug, Clone)]
pub struct StoredTranscript {
    path: std::path::PathBuf,
    stem: talkbank_model::model::OwnedFileStem,
    resolved: ResolvedPath,
}

impl StoredTranscript {
    /// Resolve a requested spelling to the stored directory entry. Exact names
    /// win on case/normalization-sensitive filesystems. A missing, ambiguous or
    /// non-UTF-8 name is an explicit I/O refusal, never anonymous validation.
    pub fn resolve(requested: &Path) -> std::io::Result<Self> {
        StoredNameResolver::default().resolve(requested)
    }

    /// Admit an actual directory entry without another directory scan.
    pub fn from_entry(entry: std::fs::DirEntry) -> std::io::Result<Self> {
        let path = entry.path();
        let resolved = ResolvedPath::of_file(&path)?;
        Self::admit(path, resolved)
    }

    /// A transcript at `path`, stored under its last component, whose
    /// location is `resolved`.
    fn admit(path: std::path::PathBuf, resolved: ResolvedPath) -> std::io::Result<Self> {
        let stem = Self::stem_of(&path)?;
        Ok(Self {
            path,
            stem,
            resolved,
        })
    }

    /// The stem of a stored name, which validation names the transcript by.
    fn stem_of(path: &Path) -> std::io::Result<talkbank_model::model::OwnedFileStem> {
        talkbank_model::model::OwnedFileStem::from_path(path).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "stored transcript stem is not UTF-8",
            )
        })
    }

    /// The path using the directory's stored basename.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The path using the directory's stored basename, by value.
    pub fn into_path(self) -> std::path::PathBuf {
        self.path
    }

    /// The transcript's location with its directory resolved: one value
    /// for every spelling of the file (`a.cha`, `./a.cha`, an absolute path
    /// through a linked directory). The cache keys and clears by it, and a
    /// run names a transcript once by it.
    pub fn resolved(&self) -> &ResolvedPath {
        &self.resolved
    }

    /// Named validation context borrowing the resolved identity.
    pub fn name(&self) -> talkbank_model::model::TranscriptName<'_> {
        talkbank_model::model::TranscriptName::Named(self.stem.as_stem())
    }
}

struct DirectoryNames {
    modified: std::time::SystemTime,
    /// The directory, resolved once for every transcript found in it.
    resolved: ResolvedDirectory,
    entries: std::collections::HashMap<std::ffi::OsString, std::path::PathBuf>,
}

impl DirectoryNames {
    fn read(parent: &Path, modified: std::time::SystemTime) -> std::io::Result<Self> {
        let entries = std::fs::read_dir(parent)?
            .map(|entry| {
                let entry = entry?;
                Ok((entry.file_name(), entry.path()))
            })
            .collect::<std::io::Result<_>>()?;
        Ok(Self {
            modified,
            resolved: ResolvedDirectory::resolve(parent)?,
            entries,
        })
    }

    /// Admit the stored entry `name` at `path`.
    fn admit(&self, name: &std::ffi::OsStr, path: &Path) -> std::io::Result<StoredTranscript> {
        StoredTranscript::admit(path.to_path_buf(), self.resolved.file(name)?)
    }
}

/// Per-run resolver. Each unchanged parent is listed once, rather than once
/// per transcript; a changed directory timestamp invalidates the snapshot.
#[derive(Default)]
pub struct StoredNameResolver {
    directories: std::collections::HashMap<std::path::PathBuf, DirectoryNames>,
}

impl StoredNameResolver {
    /// Resolve an existing argument spelling without disabling named rules.
    pub fn resolve(&mut self, requested: &Path) -> std::io::Result<StoredTranscript> {
        use std::io::{Error, ErrorKind};
        use unicode_normalization::UnicodeNormalization;
        requested.metadata()?;
        let wanted = requested
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidInput,
                    "transcript filename is missing or is not UTF-8",
                )
            })?;
        let canonical: String = wanted.nfc().collect();
        let parent = requested
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let modified = parent.metadata()?.modified()?;
        // Entry ownership carries the populated snapshot through admission;
        // there is no second lookup with an impossible missing-map state.
        let names = match self.directories.entry(parent.to_path_buf()) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(DirectoryNames::read(parent, modified)?)
            }
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                if slot.get().modified != modified {
                    slot.insert(DirectoryNames::read(parent, modified)?);
                }
                slot.into_mut()
            }
        };
        if let Some(path) = names.entries.get(std::ffi::OsStr::new(wanted)) {
            return names.admit(std::ffi::OsStr::new(wanted), path);
        }
        // Falls back to a spelling that differs in Unicode form or ASCII case,
        // so a transcript is found under the name actually stored. That is
        // resolution, not a verdict: validation then compares the stored name
        // with `@Media`, and a case-only or non-NFC difference is reported
        // there (W110, W109), never absorbed here.
        let mut candidates = names.entries.iter().filter(|(stored, _)| {
            stored.to_str().is_some_and(|name| {
                name.nfc()
                    .collect::<String>()
                    .eq_ignore_ascii_case(&canonical)
            })
        });
        let (name, path) = candidates.next().ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                "stored transcript filename could not be resolved",
            )
        })?;
        if candidates.next().is_some() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "ambiguous stored transcript spelling",
            ));
        }
        names.admit(name, path)
    }
}

/// Return `true` when `path` is a CHAT transcript we should collect: a `.cha`
/// file that is not a macOS AppleDouble sidecar (`._name.cha`). Shared by the
/// transform-side directory walks and the CLI-side walk so the two never
/// drift in what they treat as a transcript.
pub fn is_chat_transcript_path(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()) == Some("cha")
        && !path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| name.starts_with("._"))
}

/// A file a walk found under its root.
///
/// The relative path is built by the walk as it descends (one component per
/// level) and the full path is `root.join(relative)`, so "this file is under
/// the root, at this relative path" is how the value was made rather than a
/// `strip_prefix` that a consumer has to handle failing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    path: std::path::PathBuf,
    relative: std::path::PathBuf,
}

impl FoundFile {
    /// The file's path: the walk's root joined with [`Self::relative`].
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's path relative to the walk's root.
    pub fn relative(&self) -> &Path {
        &self.relative
    }
}

/// A directory, entry or file type a walk could not read. Reported, never
/// skipped silently: a walk that dropped these would present a partial
/// corpus as the whole one.
#[derive(Debug)]
pub struct WalkFailure {
    /// The path that could not be read.
    pub path: std::path::PathBuf,
    /// Why.
    pub error: std::io::Error,
}

impl std::fmt::Display for WalkFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot read {}: {}", self.path.display(), self.error)
    }
}

/// What a walk found, and what it could not read.
///
/// Built only by [`walk_files`], which sorts what it found: the fields are
/// private, so "sorted by relative path" is how the value was made, not a
/// promise in a doc comment.
#[derive(Debug)]
pub struct Walk {
    found: Vec<FoundFile>,
    failures: Vec<WalkFailure>,
}

impl Walk {
    /// The kept files, sorted by relative path.
    pub fn found(&self) -> &[FoundFile] {
        &self.found
    }

    /// Everything the walk could not read, in the order met.
    pub fn failures(&self) -> &[WalkFailure] {
        &self.failures
    }

    /// The found files and the failures, by value.
    pub fn into_parts(self) -> (Vec<FoundFile>, Vec<WalkFailure>) {
        (self.found, self.failures)
    }
}

/// What a walk does with a symbolic link it meets below its root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Links {
    /// Walk a link as what it points to, to a file or a directory: a corpus
    /// may be assembled from links, and reading through one is harmless.
    Follow,
    /// Leave links alone: neither walked into nor offered to `keep`. For a
    /// walk whose results are DELETED (`to-json --prune`): a link is not a
    /// file the tool wrote, and following one reaches outside the tree the
    /// caller owns.
    Skip,
}

/// Walk `root` and keep the files `keep` accepts.
///
/// `keep` is given each entry's FILE NAME, not its full path, so an entry it
/// rejects never has its path built (the name itself is still read from the
/// listing). With [`Links::Follow`], a directory reached a second time (a
/// link cycle, or two links to one directory) is not walked again, so every
/// file is found once, and a link whose target cannot be read is a failure,
/// whatever its name, since it may have been a directory. Every read
/// failure, of the root itself included, is a [`WalkFailure`] in the result: a directory the walk
/// cannot list (an operating system's own folder at a volume root, for
/// example) is reported, never skipped, because nothing can say whether it
/// held transcripts. Walk the corpus directory, not the volume root.
///
/// The type of each entry comes from its directory listing; only a followed
/// link, and each directory (for its identity), costs a `stat`.
pub fn walk_files(root: &Path, links: Links, keep: impl Fn(&Path) -> bool) -> Walk {
    let mut walk = Walk {
        found: Vec::new(),
        failures: Vec::new(),
    };
    let mut visited = std::collections::HashSet::new();
    let policy = WalkPolicy { links, keep: &keep };
    match std::fs::metadata(root) {
        Ok(metadata) => walk_directory(
            root,
            Path::new(""),
            &metadata,
            &policy,
            &mut visited,
            &mut walk,
        ),
        Err(error) => walk.failures.push(WalkFailure {
            path: root.to_path_buf(),
            error,
        }),
    }
    walk.found.sort_by(|a, b| a.relative.cmp(&b.relative));
    walk
}

/// A transcript a walk found: its stored name (from the directory listing)
/// and its path relative to the walk's root.
#[derive(Debug, Clone)]
pub struct FoundTranscript {
    stored: StoredTranscript,
    relative: std::path::PathBuf,
}

impl FoundTranscript {
    /// The transcript under its stored name.
    pub fn stored(&self) -> &StoredTranscript {
        &self.stored
    }

    /// The transcript's path: the walk's root joined with [`Self::relative`].
    pub fn path(&self) -> &Path {
        self.stored.path()
    }

    /// The transcript's path relative to the walk's root.
    pub fn relative(&self) -> &Path {
        &self.relative
    }

    /// The transcript, by value.
    pub fn into_stored(self) -> StoredTranscript {
        self.stored
    }
}

/// The transcripts a walk found, and what it could not read.
///
/// Built only by [`walk_transcripts`]: each found transcript's name came from
/// the listing that found it, and one whose stem is not UTF-8 (so validation
/// could not name it) is a failure here rather than later.
#[derive(Debug)]
pub struct TranscriptWalk {
    found: Vec<FoundTranscript>,
    failures: Vec<WalkFailure>,
}

impl TranscriptWalk {
    /// The transcripts, sorted by relative path.
    pub fn found(&self) -> &[FoundTranscript] {
        &self.found
    }

    /// Everything the walk could not read or name, in the order met.
    pub fn failures(&self) -> &[WalkFailure] {
        &self.failures
    }

    /// The transcripts and the failures, by value.
    pub fn into_parts(self) -> (Vec<FoundTranscript>, Vec<WalkFailure>) {
        (self.found, self.failures)
    }
}

/// Walk `root` for CHAT transcripts ([`is_chat_transcript_path`]),
/// following links, every level down.
pub fn walk_transcripts(root: &Path) -> TranscriptWalk {
    let Walk {
        found,
        mut failures,
    } = walk_files(root, Links::Follow, is_chat_transcript_path);
    let mut transcripts = Vec::with_capacity(found.len());
    // Each directory is resolved once, whatever order its files arrive in
    // (sorting by relative path interleaves a directory's files with its
    // subdirectories').
    let mut directories = std::collections::HashMap::new();
    for FoundFile { path, relative } in found {
        // The path's last component is the listing's own entry name, so it
        // is the stored name: no second listing is needed to learn it.
        match admit_found(&path, &mut directories)
            .and_then(|resolved| StoredTranscript::admit(path.clone(), resolved))
        {
            Ok(stored) => transcripts.push(FoundTranscript { stored, relative }),
            Err(error) => failures.push(WalkFailure { path, error }),
        }
    }
    TranscriptWalk {
        found: transcripts,
        failures,
    }
}

/// The resolved location of a file a walk found, resolving its directory
/// the first time one of its files is admitted.
fn admit_found(
    path: &Path,
    directories: &mut std::collections::HashMap<std::path::PathBuf, ResolvedDirectory>,
) -> std::io::Result<ResolvedPath> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "a found file has no directory or name",
        ));
    };
    match directories.get(parent) {
        Some(resolved) => resolved.file(name),
        None => {
            let resolved = ResolvedDirectory::resolve(parent)?;
            let located = resolved.file(name);
            directories.insert(parent.to_path_buf(), resolved);
            located
        }
    }
}

/// Stored transcripts, each named once by its resolved location: what a
/// validation run takes, so no run validates or counts one file twice.
///
/// Built only by [`DistinctTranscripts::new`], which keeps the first spelling
/// of each location (`dir/a.cha` beside `./dir/a.cha`, `dir/sub/../a.cha`,
/// or a file inside a directory also given).
#[derive(Debug, Clone)]
pub struct DistinctTranscripts(Vec<StoredTranscript>);

impl DistinctTranscripts {
    /// `files`, each location once, under the first spelling given, in the
    /// order given.
    pub fn new(files: impl IntoIterator<Item = StoredTranscript>) -> Self {
        let mut seen = std::collections::HashSet::new();
        Self(
            files
                .into_iter()
                .filter(|file| seen.insert(file.resolved().clone()))
                .collect(),
        )
    }

    /// The transcripts.
    pub fn as_slice(&self) -> &[StoredTranscript] {
        &self.0
    }

    /// How many transcripts there are.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there is none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Sorted by path.
    fn sorted_by_path(mut self) -> Self {
        self.0.sort_by(|a, b| a.path().cmp(b.path()));
        self
    }
}

impl IntoIterator for DistinctTranscripts {
    type Item = StoredTranscript;
    type IntoIter = std::vec::IntoIter<StoredTranscript>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Command-line paths expanded to the transcripts a command reads.
///
/// Built only by [`expand_transcript_arguments`], which sorts the files.
#[derive(Debug)]
pub struct ExpandedArguments {
    files: DistinctTranscripts,
    failures: Vec<WalkFailure>,
}

impl ExpandedArguments {
    /// The transcripts, sorted by path: each file argument under its stored
    /// name, and the transcripts under each directory argument.
    pub fn files(&self) -> &[StoredTranscript] {
        self.files.as_slice()
    }

    /// Every argument or directory entry that could not be read or named, an
    /// argument that does not exist included.
    pub fn failures(&self) -> &[WalkFailure] {
        &self.failures
    }

    /// The transcripts and the failures, by value.
    pub fn into_parts(self) -> (DistinctTranscripts, Vec<WalkFailure>) {
        (self.files, self.failures)
    }
}

/// Expand command-line paths the one way every command does: a file is
/// taken as named (whatever its extension, because the user named it) and
/// resolved to its stored name, and a directory contributes the transcripts
/// under it, recursively. Nothing is dropped silently: an argument that is
/// neither, cannot be read, or whose stored name cannot be resolved, is a
/// failure in the result. One resolver serves every file argument, so each
/// parent directory is listed once. The result names each transcript once.
pub fn expand_transcript_arguments(paths: &[std::path::PathBuf]) -> ExpandedArguments {
    let mut files = Vec::new();
    let mut failures = Vec::new();
    let mut names = StoredNameResolver::default();
    for path in paths {
        match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => {
                let (found, walk_failures) = walk_transcripts(path).into_parts();
                files.extend(found.into_iter().map(FoundTranscript::into_stored));
                failures.extend(walk_failures);
            }
            Ok(_) => match names.resolve(path) {
                Ok(stored) => files.push(stored),
                Err(error) => failures.push(WalkFailure {
                    path: path.clone(),
                    error,
                }),
            },
            Err(error) => failures.push(WalkFailure {
                path: path.clone(),
                error,
            }),
        }
    }
    // A transcript named twice (two spellings of one file argument, or a
    // file inside a directory also given) is one transcript, by its resolved
    // location, under the first spelling given. Then sorted by path.
    ExpandedArguments {
        files: DistinctTranscripts::new(files).sorted_by_path(),
        failures,
    }
}

/// Resolve explicit file paths to stored transcripts with one resolver:
/// each one that resolves, once per location (the first spelling given), and
/// each that does not as a failure. For the runner's
/// `validate_files_streaming`, handed a plain file list.
#[cfg(feature = "validation-runner")]
pub(crate) fn resolve_transcripts(
    paths: impl IntoIterator<Item = std::path::PathBuf>,
) -> (DistinctTranscripts, Vec<WalkFailure>) {
    let mut names = StoredNameResolver::default();
    let mut resolved = Vec::new();
    let mut failures = Vec::new();
    for path in paths {
        match names.resolve(&path) {
            Ok(stored) => resolved.push(stored),
            Err(error) => failures.push(WalkFailure { path, error }),
        }
    }
    (DistinctTranscripts::new(resolved), failures)
}

/// What identifies a directory across the different paths that reach it:
/// device and inode on Unix (from metadata the walk already has), the fully
/// resolved path elsewhere.
#[cfg(unix)]
type DirectoryIdentity = (u64, u64);
#[cfg(not(unix))]
type DirectoryIdentity = std::path::PathBuf;

#[cfg(unix)]
fn directory_identity(
    _path: &Path,
    metadata: &std::fs::Metadata,
) -> std::io::Result<DirectoryIdentity> {
    use std::os::unix::fs::MetadataExt;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn directory_identity(
    path: &Path,
    _metadata: &std::fs::Metadata,
) -> std::io::Result<DirectoryIdentity> {
    std::fs::canonicalize(path)
}

/// What one directory entry is, for the walk: a directory to descend into
/// (with its metadata, for its identity), a file to offer to `keep`, or
/// nothing the walk will touch.
enum EntryKind {
    Directory(std::fs::Metadata),
    File,
    Ignored,
}

/// The settings every level of one walk shares.
struct WalkPolicy<'a, K> {
    links: Links,
    keep: &'a K,
}

/// One directory of [`walk_files`]: `directory` is `root.join(relative)`,
/// and `metadata` is its metadata.
fn walk_directory<K: Fn(&Path) -> bool>(
    directory: &Path,
    relative: &Path,
    metadata: &std::fs::Metadata,
    policy: &WalkPolicy<'_, K>,
    visited: &mut std::collections::HashSet<DirectoryIdentity>,
    walk: &mut Walk,
) {
    let fail = |walk: &mut Walk, path: std::path::PathBuf, error| {
        walk.failures.push(WalkFailure { path, error })
    };
    match directory_identity(directory, metadata) {
        Ok(identity) => {
            // Reached before, through another path or a link cycle.
            if !visited.insert(identity) {
                return;
            }
        }
        Err(error) => return fail(walk, directory.to_path_buf(), error),
    }
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => return fail(walk, directory.to_path_buf(), error),
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                fail(walk, directory.to_path_buf(), error);
                continue;
            }
        };
        // The name is all `keep` sees, so an entry it rejects never has its
        // full path built.
        let name = entry.file_name();
        // The listing's own type: no stat for a plain file or directory.
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                fail(walk, directory.join(&name), error);
                continue;
            }
        };
        let kind = if file_type.is_dir() {
            match entry.metadata() {
                Ok(metadata) => EntryKind::Directory(metadata),
                Err(error) => {
                    fail(walk, directory.join(&name), error);
                    continue;
                }
            }
        } else if file_type.is_symlink() {
            match policy.links {
                Links::Skip => EntryKind::Ignored,
                // A link is walked as what it points to. One whose target
                // cannot be read is a failure whatever its name: nothing
                // says whether it was a file or a directory, and a
                // directory (an unmounted volume's subcorpus) may hold any
                // number of transcripts.
                Links::Follow => match std::fs::metadata(directory.join(&name)) {
                    Ok(metadata) if metadata.is_dir() => EntryKind::Directory(metadata),
                    Ok(_) => EntryKind::File,
                    Err(error) => {
                        fail(walk, directory.join(&name), error);
                        continue;
                    }
                },
            }
        } else {
            EntryKind::File
        };
        match kind {
            EntryKind::Directory(metadata) => walk_directory(
                &directory.join(&name),
                &relative.join(&name),
                &metadata,
                policy,
                visited,
                walk,
            ),
            EntryKind::Ignored => {}
            EntryKind::File => {
                if (policy.keep)(Path::new(&name)) {
                    walk.found.push(FoundFile {
                        path: directory.join(&name),
                        relative: relative.join(&name),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod walk_tests {
    use super::*;

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

    /// A dangling link is a failure whatever its name: a link to an
    /// unmounted subcorpus (`Eng -> /Volumes/gone/Eng`) hides every
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
