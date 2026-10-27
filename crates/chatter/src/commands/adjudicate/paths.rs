//! Admit all adjudication file roles before prompting or publishing.
//!
//! This detects aliases at admission, not concurrent filesystem replacement.

use std::io;
use std::path::{Path, PathBuf};

use same_file::Handle;
use talkbank_transform::adjudication::{AdjudicationError, PendingAdjudications};
use talkbank_transform::speaker_id::{OverrideFile, OverrideFileError};

/// Roles are distinct even when the supplied paths accidentally are not.
#[derive(Clone, Copy, Debug)]
pub(super) enum FileRole {
    Pending,
    Overrides,
    Script,
}

impl std::fmt::Display for FileRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Pending => "pending-adjudications",
            Self::Overrides => "override-file",
            Self::Script => "scripted-decisions",
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum PathAdmissionError {
    #[error("cannot establish {role} file identity for {}: {source}", path.display())]
    Identity {
        role: FileRole,
        path: PathBuf,
        source: io::Error,
    },
    #[error("{first} and {second} paths alias the same file; choose separate files")]
    Alias { first: FileRole, second: FileRole },
}

#[derive(Debug, thiserror::Error)]
pub(super) enum PublicationError {
    #[error("override-file {}: {source}", path.display())]
    Overrides {
        path: PathBuf,
        source: OverrideFileError,
    },
    #[error("pending-adjudications {}: {source}", path.display())]
    Pending {
        path: PathBuf,
        source: Box<AdjudicationError>,
    },
}

/// The only command publication route owns the three admitted path roles.
/// The missing override file is permitted; pending and script inputs must exist.
pub(super) struct AdjudicationPaths<'a> {
    pending: &'a Path,
    overrides: OverrideDestination<'a>,
}

/// Absence is admitted only under an existing directory. Windows reports a
/// non-directory ancestor as NotFound too; that is not a vacant file slot.
enum OverrideDestination<'a> {
    Existing { path: &'a Path, identity: Handle },
    Vacant { path: &'a Path },
}

impl<'a> OverrideDestination<'a> {
    fn admit(path: &'a Path) -> Result<Self, PathAdmissionError> {
        let admit = || -> io::Result<Self> {
            match Handle::from_path(path) {
                Ok(identity) => Ok(Self::Existing { path, identity }),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    // A dangling symlink is not a vacant directory entry.
                    match path.symlink_metadata() {
                        Ok(_) => {
                            return Err(io::Error::new(
                                io::ErrorKind::AlreadyExists,
                                "unresolved output entry",
                            ));
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                    let parent = path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(Path::new("."));
                    if !parent.metadata()?.is_dir() {
                        return Err(io::Error::new(
                            io::ErrorKind::NotADirectory,
                            "override parent is not a directory",
                        ));
                    }
                    Ok(Self::Vacant { path })
                }
                Err(error) => Err(error),
            }
        };
        admit().map_err(|source| PathAdmissionError::Identity {
            role: FileRole::Overrides,
            path: path.to_path_buf(),
            source,
        })
    }

    fn path(&self) -> &'a Path {
        match self {
            Self::Existing { path, .. } | Self::Vacant { path } => path,
        }
    }

    fn distinct_from(&self, other: &Handle, role: FileRole) -> Result<(), PathAdmissionError> {
        match self {
            Self::Existing { identity, .. } => distinct(identity, FileRole::Overrides, other, role),
            Self::Vacant { .. } => Ok(()),
        }
    }
}

impl<'a> AdjudicationPaths<'a> {
    pub(super) fn admit(
        pending: &'a Path,
        overrides: &'a Path,
        script: Option<&'a Path>,
    ) -> Result<Self, PathAdmissionError> {
        let pending_identity = identity(pending, FileRole::Pending)?;
        let overrides = OverrideDestination::admit(overrides)?;
        overrides.distinct_from(&pending_identity, FileRole::Pending)?;
        if let Some(script) = script {
            let handle = identity(script, FileRole::Script)?;
            distinct(
                &pending_identity,
                FileRole::Pending,
                &handle,
                FileRole::Script,
            )?;
            overrides.distinct_from(&handle, FileRole::Script)?;
        }
        Ok(Self { pending, overrides })
    }

    pub(super) fn pending(&self) -> &Path {
        self.pending
    }
    pub(super) fn overrides(&self) -> &Path {
        self.overrides.path()
    }

    /// Preserve the established sequential publication policy. A failure of the
    /// second write does not roll back the first; callers must report it.
    pub(super) fn publish(
        self,
        pending: &PendingAdjudications,
        overrides: &OverrideFile,
    ) -> Result<(), PublicationError> {
        overrides
            .write(self.overrides.path())
            .map_err(|source| PublicationError::Overrides {
                path: self.overrides.path().to_path_buf(),
                source,
            })?;
        pending
            .write(self.pending)
            .map_err(|source| PublicationError::Pending {
                path: self.pending.to_path_buf(),
                source: Box::new(source),
            })?;
        Ok(())
    }
}

fn identity(path: &Path, role: FileRole) -> Result<Handle, PathAdmissionError> {
    Handle::from_path(path).map_err(|source| PathAdmissionError::Identity {
        role,
        path: path.to_path_buf(),
        source,
    })
}

fn distinct(
    first: &Handle,
    first_role: FileRole,
    second: &Handle,
    second_role: FileRole,
) -> Result<(), PathAdmissionError> {
    if first == second {
        return Err(PathAdmissionError::Alias {
            first: first_role,
            second: second_role,
        });
    }
    Ok(())
}
