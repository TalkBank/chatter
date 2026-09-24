//! CHAT transcript path classification.
//!
//! Lives at the crate root, OUTSIDE the `validation-runner` feature, because
//! path classification has nothing to do with the SQLite result cache: the
//! corpus manifest walk and the CLI-side walks need it on every build,
//! including `default-features = false` consumers that opt out of the
//! validation runner entirely.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::Path;

/// A transcript path whose basename was obtained from its parent directory,
/// rather than trusted from an argument's spelling. Symlink names are retained.
#[derive(Debug, Clone)]
pub struct StoredTranscript {
    path: std::path::PathBuf,
    stem: String,
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
        Self::from_stored_path(entry.path())
    }

    fn from_stored_path(path: std::path::PathBuf) -> std::io::Result<Self> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "stored transcript stem is not UTF-8",
                )
            })?
            .to_owned();
        Ok(Self { path, stem })
    }

    /// The path using the directory's stored basename.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Named validation context borrowing the resolved identity.
    pub fn name(&self) -> talkbank_model::model::TranscriptName<'_> {
        talkbank_model::model::TranscriptName::Named(talkbank_model::model::FileStem::from_stem(
            &self.stem,
        ))
    }
}

struct DirectoryNames {
    modified: std::time::SystemTime,
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
        Ok(Self { modified, entries })
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
            return StoredTranscript::from_stored_path(path.clone());
        }
        let mut candidates = names.entries.iter().filter_map(|(stored, path)| {
            let name = stored.to_str()?;
            name.nfc()
                .collect::<String>()
                .eq_ignore_ascii_case(&canonical)
                .then_some(path)
        });
        let path = candidates.next().ok_or_else(|| {
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
        StoredTranscript::from_stored_path(path.clone())
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
