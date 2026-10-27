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
    overrides: &'a Path,
}

impl<'a> AdjudicationPaths<'a> {
    pub(super) fn admit(
        pending: &'a Path,
        overrides: &'a Path,
        script: Option<&'a Path>,
    ) -> Result<Self, PathAdmissionError> {
        let pending_identity = identity(pending, FileRole::Pending)?;
        let override_identity = match Handle::from_path(overrides) {
            Ok(handle) => Some(handle),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(PathAdmissionError::Identity {
                    role: FileRole::Overrides,
                    path: overrides.to_path_buf(),
                    source,
                });
            }
        };
        if let Some(handle) = &override_identity {
            distinct(
                &pending_identity,
                FileRole::Pending,
                handle,
                FileRole::Overrides,
            )?;
        }
        if let Some(script) = script {
            let handle = identity(script, FileRole::Script)?;
            distinct(
                &pending_identity,
                FileRole::Pending,
                &handle,
                FileRole::Script,
            )?;
            if let Some(overrides) = &override_identity {
                distinct(overrides, FileRole::Overrides, &handle, FileRole::Script)?;
            }
        }
        Ok(Self { pending, overrides })
    }

    pub(super) fn pending(&self) -> &Path {
        self.pending
    }
    pub(super) fn overrides(&self) -> &Path {
        self.overrides
    }

    /// Preserve the established sequential publication policy. A failure of the
    /// second write does not roll back the first; callers must report it.
    pub(super) fn publish(
        self,
        pending: &PendingAdjudications,
        overrides: &OverrideFile,
    ) -> Result<(), PublicationError> {
        overrides
            .write(self.overrides)
            .map_err(|source| PublicationError::Overrides {
                path: self.overrides.to_path_buf(),
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
