//! Admit the opened report file before any operation may truncate it.

mod staged;
pub use staged::{PendingPublication, ReportWriter, StagedReportDestination};

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

/// An opened destination distinct from every discovered transcript input.
/// Writing consumes the same handle that passed identity admission.
pub struct ReportDestination(File);

/// Failure to establish a source-preserving report destination.
#[derive(Debug, thiserror::Error)]
pub enum ReportDestinationError {
    /// The opened report is one of the discovered inputs.
    #[error("output aliases input {}; choose a separate report path", .0.display())]
    SourceAlias(PathBuf),
    /// A discovered input could not be opened for identity admission.
    #[error("cannot establish input identity for {}: {source}", path.display())]
    SourceIdentity {
        /// The input whose identity could not be established.
        path: PathBuf,
        /// The filesystem failure.
        source: io::Error,
    },
    /// Opening or preparing the report failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Indexed identities of a discovered input population. Handles are closed after
/// indexing, avoiding a descriptor per input. Inputs must not be replaced during
/// the operation; this is not a filesystem snapshot or a race-proof sandbox.
/// Hashes only select candidates: equality is checked against opened handles.
pub struct ReportInputs(HashMap<u64, Vec<PathBuf>>);

impl ReportInputs {
    /// Establish every input identity before allowing any output mutation.
    pub fn discover<'a>(
        inputs: impl IntoIterator<Item = &'a Path>,
    ) -> Result<Self, ReportDestinationError> {
        let mut indexed = HashMap::<u64, Vec<PathBuf>>::new();
        for input in inputs {
            let handle = Self::open(input)?;
            indexed
                .entry(Self::key(&handle))
                .or_default()
                .push(input.to_path_buf());
        }
        Ok(Self(indexed))
    }

    fn open(input: &Path) -> Result<same_file::Handle, ReportDestinationError> {
        same_file::Handle::from_path(input).map_err(|source| {
            ReportDestinationError::SourceIdentity {
                path: input.to_path_buf(),
                source,
            }
        })
    }

    fn key(handle: &same_file::Handle) -> u64 {
        let mut hasher = DefaultHasher::new();
        handle.hash(&mut hasher);
        hasher.finish()
    }

    fn admit(&self, file: File) -> Result<ReportDestination, ReportDestinationError> {
        let destination = same_file::Handle::from_file(file.try_clone()?)?;
        if let Some(candidates) = self.0.get(&Self::key(&destination)) {
            for input in candidates {
                if Self::open(input)? == destination {
                    return Err(ReportDestinationError::SourceAlias(input.clone()));
                }
            }
        }
        Ok(ReportDestination(file))
    }

    /// Admit an existing output without creating a file for a failed conversion.
    /// Only a missing path yields `None`; other open failures remain errors.
    pub fn existing(
        &self,
        path: &Path,
    ) -> Result<Option<ReportDestination>, ReportDestinationError> {
        match OpenOptions::new().write(true).open(path) {
            Ok(file) => self.admit(file).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Stage a report after all source identities have been established.
    /// Existing reports remain untouched until explicit publication succeeds.
    pub fn staged_destination(
        &self,
        path: &Path,
    ) -> Result<StagedReportDestination, ReportDestinationError> {
        StagedReportDestination::admit_with(path, |file| self.admit(file))
    }

    /// Stage only an existing destination. Missing parents are permitted until
    /// conversion succeeds and the caller prepares a new destination.
    pub fn existing_staged(
        &self,
        path: &Path,
    ) -> Result<Option<StagedReportDestination>, ReportDestinationError> {
        match path.symlink_metadata() {
            Ok(_) => self.staged_destination(path).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Open or create an output, then admit its retained handle before truncation.
    pub fn destination(&self, path: &Path) -> Result<ReportDestination, ReportDestinationError> {
        self.admit(
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)?,
        )
    }
}

impl ReportDestination {
    /// Open without truncation and compare identities with every source.
    /// Failure to inspect any source refuses admission. Symlinks and hard links
    /// are compared by filesystem identity, not path spelling.
    /// This does not promise protection against concurrent source replacement.
    pub fn admit<'a>(
        path: &Path,
        inputs: impl IntoIterator<Item = &'a Path>,
    ) -> Result<Self, ReportDestinationError> {
        // Opening must not change existing bytes: the destination may be a
        // differently named hard link or symlink to a transcript.
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        Self::admit_file(file, inputs)
    }

    fn admit_file<'a>(
        file: File,
        inputs: impl IntoIterator<Item = &'a Path>,
    ) -> Result<Self, ReportDestinationError> {
        let destination = same_file::Handle::from_file(file.try_clone()?)?;
        for input in inputs {
            let source = same_file::Handle::from_path(input).map_err(|source| {
                ReportDestinationError::SourceIdentity {
                    path: input.to_path_buf(),
                    source,
                }
            })?;
            if source == destination {
                return Err(ReportDestinationError::SourceAlias(input.to_path_buf()));
            }
        }
        Ok(Self(file))
    }

    /// Refuse two opened destinations that refer to the same file. Compare the
    /// retained handles, not paths that might name another file by publication.
    pub fn ensure_distinct(&self, other: &Self) -> io::Result<()> {
        let first = same_file::Handle::from_file(self.0.try_clone()?)?;
        let second = same_file::Handle::from_file(other.0.try_clone()?)?;
        if first == second {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "output destinations alias each other; choose separate paths",
            ));
        }
        Ok(())
    }

    /// Consume admission and prepare the same handle for report replacement.
    /// Callers must flush explicitly to observe buffered write failures.
    pub fn into_writer(mut self) -> io::Result<BufWriter<File>> {
        let file = &mut self.0;
        // Preserve streaming destinations; ordinary reports retain the
        // existing replacement policy only after source identity admission.
        if file.metadata()?.is_file() {
            file.set_len(0)?;
        }
        Ok(BufWriter::new(self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_hash_collision_does_not_reject_a_distinct_destination()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = tempfile::NamedTempFile::new()?;
        let destination = tempfile::NamedTempFile::new()?;
        let handle = same_file::Handle::from_path(destination.path())?;
        // Force a candidate collision; the opened identities still differ.
        let inputs = ReportInputs(HashMap::from([(
            ReportInputs::key(&handle),
            vec![source.path().to_path_buf()],
        )]));
        inputs.destination(destination.path())?;
        Ok(())
    }
}
