//! A file's location with every directory resolved: the identity under which
//! a transcript is cached, cleared and counted once.
//!
//! Two spellings of one file (`a.cha` in `/tmp/c`, `/tmp/c/a.cha`,
//! `/tmp/c/sub/../a.cha`, a path through a linked directory) name the same
//! stored transcript, and a path made merely absolute keeps them apart:
//! `/tmp` and `/private/tmp` are one directory on macOS, and a relative path
//! made absolute against the current directory takes its physical spelling
//! while an absolute argument keeps its logical one. A [`ResolvedPath`] joins
//! the CANONICAL parent directory (links and `..` resolved by the operating
//! system) to the file's own name, kept as stored: a link to a transcript is
//! a transcript under its own name, which validation compares with
//! `@Media`, so the last component is never resolved.
//!
//! The only constructors read the filesystem, so no caller can mint one from
//! a spelling of its choice. A directory is resolved once into a
//! [`ResolvedDirectory`], and each file in it is [`ResolvedDirectory::file`],
//! so a walk resolves a directory once, not once per file.

use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path, PathBuf};

/// A directory with every component resolved by the operating system
/// (`std::fs::canonicalize`), or, for a directory that no longer exists,
/// its deepest existing ancestor resolved and the missing rest appended as
/// written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResolvedDirectory(PathBuf);

impl ResolvedDirectory {
    /// Resolve `path`. A missing directory is resolved through its deepest
    /// existing ancestor (what `cache clear --prefix` needs for a corpus
    /// already deleted). Native absolute-path conversion runs first: Windows
    /// normalizes `..` in ordinary paths; POSIX retains it. A `..` still among
    /// missing components cannot be resolved and is refused, as is an empty path.
    pub fn resolve(path: &Path) -> io::Result<Self> {
        let mut existing = std::path::absolute(path)?;
        let mut missing = Vec::new();
        loop {
            match std::fs::canonicalize(&existing) {
                Ok(mut resolved) => {
                    // Admission is about a directory location, not just an
                    // existing path. Never append missing children to a file.
                    if !std::fs::metadata(&resolved)?.is_dir() {
                        return Err(io::Error::new(
                            io::ErrorKind::NotADirectory,
                            "the resolved ancestor is not a directory",
                        ));
                    }
                    resolved.extend(missing.iter().rev());
                    return Ok(Self(resolved));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    // `file_name` is `None` for a `..` (or the root): a
                    // missing `..` has nothing on disk to resolve against.
                    let name = existing.file_name().ok_or(error)?.to_owned();
                    missing.push(name);
                    if !existing.pop() {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            "no ancestor of the path exists",
                        ));
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// The file named `name` in this directory. `name` must be one plain
    /// path component (no separator, no `.` or `..`).
    pub fn file(&self, name: &OsStr) -> io::Result<ResolvedPath> {
        let mut components = Path::new(name).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(component)), None) if component == name => {
                Ok(ResolvedPath(self.0.join(name)))
            }
            (Some(_), _) | (None, _) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a file name must be one plain path component",
            )),
        }
    }

    /// The directory's resolved path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A file's location: its [`ResolvedDirectory`] joined with its own name
/// as stored (see the module documentation).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResolvedPath(PathBuf);

impl ResolvedPath {
    /// Resolve the file at `path`: its parent directory (the current
    /// directory for a bare name), then its own name, unresolved. A path
    /// with no file name (`/`, `..`) or a trailing directory separator or
    /// `.` component is refused, even when component iteration normalizes it.
    pub fn of_file(path: &Path) -> io::Result<Self> {
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path names no file"))?;
        // `file_name` ignores trailing separators and `.` components. A file
        // location must end with that actual name, not a normalized directory
        // spelling. Compare the native encoding without requiring Unicode.
        if !path
            .as_os_str()
            .as_encoded_bytes()
            .ends_with(name.as_encoded_bytes())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a file path must end with its file name",
            ));
        }
        let parent = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            Some(_) | None => Path::new("."),
        };
        ResolvedDirectory::resolve(parent)?.file(name)
    }

    /// The resolved path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A prefix of file locations: a resolved directory (every file under it)
/// or a resolved file location (that file). Its own type, so a
/// [`ResolvedPath`] always names a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedPrefix(PathBuf);

impl ResolvedPrefix {
    /// Resolve a path that may name a directory or a file: an existing
    /// directory is resolved whole, another existing entry as a file
    /// ([`ResolvedPath::of_file`]). A missing path retains its name beneath
    /// its deepest existing ancestor, including a deleted directory spelling.
    pub fn of(path: &Path) -> io::Result<Self> {
        match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => ResolvedDirectory::resolve(path).map(Self::from),
            Ok(_) => ResolvedPath::of_file(path).map(Self::from),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A missing prefix may be a deleted directory, including an
                // explicit trailing separator. Resolve its existing ancestry
                // without admitting the spelling as a file location.
                ResolvedDirectory::resolve(path).map(Self::from)
            }
            Err(error) => Err(error),
        }
    }

    /// The resolved prefix.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A resolved directory is the prefix of every file under it.
impl From<ResolvedDirectory> for ResolvedPrefix {
    fn from(directory: ResolvedDirectory) -> Self {
        Self(directory.0)
    }
}

/// A resolved file location is the prefix of that one file.
impl From<ResolvedPath> for ResolvedPrefix {
    fn from(file: ResolvedPath) -> Self {
        Self(file.0)
    }
}
