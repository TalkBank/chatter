//! Stage regular reports; keep streaming destinations explicit.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use super::{ReportDestination, ReportDestinationError};

/// A staged regular report or an admitted streaming handle.
/// Existing regular destinations remain untouched until explicit publication.
pub struct StagedReportDestination(AdmittedDestination);

enum AdmittedDestination {
    Regular(StagedReport),
    Stream(ReportDestination),
}

/// Publication ownership retained until the writer has successfully flushed.
/// Dropping this value discards a staged regular file.
pub struct PendingPublication(Publication);

enum Publication {
    /// A regular file held separately from its final destination.
    Regular(StagedReport),
    /// Streaming bytes cannot be rolled back or renamed.
    Stream,
}

/// Private staging state retained by [`PendingPublication`].
struct StagedReport {
    staged: tempfile::NamedTempFile,
    destination: PathBuf,
    previous: PreviousReport,
}

enum PreviousReport {
    Absent,
    Existing {
        identity: same_file::Handle,
        permissions: std::fs::Permissions,
    },
}

impl StagedReportDestination {
    /// Admit against an already opened source, preserving its identity even if
    /// its original path has since been renamed or replaced.
    pub fn admit_opened(
        path: &Path,
        source_path: &Path,
        source: &same_file::Handle,
    ) -> Result<Self, ReportDestinationError> {
        Self::admit_with(path, |file| {
            let output = same_file::Handle::from_file(file.try_clone()?)?;
            if source == &output {
                return Err(ReportDestinationError::SourceAlias(
                    source_path.to_path_buf(),
                ));
            }
            Ok(ReportDestination(file))
        })
    }

    /// Refuse aliased outputs, including two names for the same existing file
    /// and two spellings of the same not-yet-created destination.
    pub fn ensure_distinct(&self, other: &Self) -> io::Result<()> {
        fn identity(destination: &AdmittedDestination) -> io::Result<Option<same_file::Handle>> {
            match destination {
                AdmittedDestination::Regular(report) => match &report.previous {
                    PreviousReport::Absent => Ok(None),
                    PreviousReport::Existing { identity, .. } => {
                        same_file::Handle::from_file(identity.as_file().try_clone()?).map(Some)
                    }
                },
                AdmittedDestination::Stream(report) => {
                    same_file::Handle::from_file(report.0.try_clone()?).map(Some)
                }
            }
        }
        let same_path = matches!((&self.0, &other.0),
            (AdmittedDestination::Regular(a), AdmittedDestination::Regular(b))
                if a.destination == b.destination);
        let same_identity = match (identity(&self.0)?, identity(&other.0)?) {
            (Some(a), Some(b)) => a == b,
            (None, _) | (Some(_), None) => false,
        };
        if same_path || same_identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "output destinations alias each other; choose separate paths",
            ));
        }
        Ok(())
    }

    /// Admit a report against discovered paths. A new destination needs no input
    /// handles, so an audit can report unreadable inputs. For strict admission of
    /// every input first, use [`super::ReportInputs::staged_destination`].
    pub fn admit<'a>(
        path: &Path,
        inputs: impl IntoIterator<Item = &'a Path>,
    ) -> Result<Self, ReportDestinationError> {
        Self::admit_with(path, |file| ReportDestination::admit_file(file, inputs))
    }

    pub(super) fn admit_with(
        path: &Path,
        admit: impl FnOnce(File) -> Result<ReportDestination, ReportDestinationError>,
    ) -> Result<Self, ReportDestinationError> {
        let metadata = match path.metadata() {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if metadata
            .as_ref()
            .is_some_and(|metadata| !metadata.is_file())
        {
            return admit(std::fs::OpenOptions::new().write(true).open(path)?)
                .map(|destination| Self(AdmittedDestination::Stream(destination)));
        }
        let (destination, previous) = match metadata {
            Some(metadata) => {
                // Preserve symlink target selection, but replace a regular-file
                // directory entry rather than mutating all of its hard links.
                let destination = path.canonicalize()?;
                let file = std::fs::OpenOptions::new().write(true).open(&destination)?;
                let admitted = admit(file)?;
                let identity = same_file::Handle::from_file(admitted.0)?;
                (
                    destination,
                    PreviousReport::Existing {
                        identity,
                        permissions: metadata.permissions(),
                    },
                )
            }
            None => {
                // A dangling symlink is not a vacant publication path.
                match path.symlink_metadata() {
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "report destination is a dangling symlink",
                        )
                        .into());
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
                let parent = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let name = path.file_name().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "report destination needs a filename",
                    )
                })?;
                (parent.canonicalize()?.join(name), PreviousReport::Absent)
            }
        };
        let parent = destination.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "report destination needs a parent",
            )
        })?;
        let staged = tempfile::NamedTempFile::new_in(parent)?;
        Ok(Self(AdmittedDestination::Regular(StagedReport {
            staged,
            destination,
            previous,
        })))
    }

    /// Begin writing; only successful completion can yield publication ownership.
    pub fn into_writer(self) -> io::Result<ReportWriter> {
        let (writer, publication) = match self.0 {
            AdmittedDestination::Regular(staged) => (
                BufWriter::new(staged.staged.as_file().try_clone()?),
                Publication::Regular(staged),
            ),
            AdmittedDestination::Stream(destination) => {
                (destination.into_writer()?, Publication::Stream)
            }
        };
        Ok(ReportWriter {
            writer,
            publication,
        })
    }
}

/// An owned report writer that cannot publish until its final flush succeeds.
pub struct ReportWriter {
    writer: BufWriter<File>,
    publication: Publication,
}

impl Write for ReportWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writer.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl ReportWriter {
    /// Flush and close the writer, admitting its bytes for publication.
    pub fn finish(mut self) -> io::Result<PendingPublication> {
        self.writer.flush()?;
        drop(self.writer);
        Ok(PendingPublication(self.publication))
    }
}

impl PendingPublication {
    /// Publish flushed bytes, refusing a changed destination identity.
    /// This is not a sandbox against concurrent filesystem mutation.
    pub fn publish(self) -> io::Result<()> {
        match self.0 {
            Publication::Stream => Ok(()),
            Publication::Regular(StagedReport {
                staged,
                destination,
                previous,
            }) => {
                staged.as_file().sync_all()?;
                match previous {
                    PreviousReport::Absent => {
                        staged
                            .persist_noclobber(destination)
                            .map_err(|error| error.error)?;
                    }
                    PreviousReport::Existing {
                        identity,
                        permissions,
                    } => {
                        if same_file::Handle::from_path(&destination)? != identity {
                            return Err(io::Error::other(
                                "report destination changed during writing",
                            ));
                        }
                        staged.as_file().set_permissions(permissions)?;
                        staged.persist(destination).map_err(|error| error.error)?;
                    }
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn retained_source_identity_survives_path_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.cha");
        let moved = dir.path().join("moved.cha");
        std::fs::write(&source, "retained source").unwrap();
        let handle = same_file::Handle::from_path(&source).unwrap();
        std::fs::rename(&source, &moved).unwrap();
        std::fs::write(&source, "different file").unwrap();
        assert!(matches!(
            StagedReportDestination::admit_opened(&moved, &source, &handle),
            Err(ReportDestinationError::SourceAlias(_))
        ));
        assert_eq!(std::fs::read_to_string(moved).unwrap(), "retained source");
    }

    #[test]
    fn staged_outputs_refuse_shared_new_names_and_existing_identity() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("report");
        let same = dir.path().join(".").join("report");
        let a = StagedReportDestination::admit(&first, []).unwrap();
        let b = StagedReportDestination::admit(&same, []).unwrap();
        assert!(a.ensure_distinct(&b).is_err());
        drop((a, b));
        std::fs::write(&first, "old").unwrap();
        let other = dir.path().join("alias");
        std::fs::hard_link(&first, &other).unwrap();
        let a = StagedReportDestination::admit(&first, []).unwrap();
        let b = StagedReportDestination::admit(&other, []).unwrap();
        assert!(a.ensure_distinct(&b).is_err());
        let distinct = StagedReportDestination::admit(&dir.path().join("different"), []).unwrap();
        assert!(a.ensure_distinct(&distinct).is_ok());
    }

    fn stage(path: &Path) -> PendingPublication {
        let admitted = StagedReportDestination::admit(path, []).unwrap();
        let mut writer = admitted.into_writer().unwrap();
        writer.write_all(b"new report").unwrap();
        writer.finish().unwrap()
    }

    #[test]
    fn stage_and_abandon_never_changes_final_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.jsonl");
        let pending = stage(&path);
        assert!(!path.exists());
        drop(pending);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        std::fs::write(&path, "old report").unwrap();
        let pending = stage(&path);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "old report");
        drop(pending);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "old report");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn new_destination_race_refuses_without_clobber() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.jsonl");
        let pending = stage(&path);
        std::fs::write(&path, "other writer").unwrap();
        assert!(pending.publish().is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "other writer");
    }

    #[test]
    fn replaced_destination_identity_refuses_publication() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.jsonl");
        std::fs::write(&path, "old report").unwrap();
        let pending = stage(&path);
        std::fs::rename(&path, dir.path().join("old.jsonl")).unwrap();
        std::fs::write(&path, "other writer").unwrap();
        assert!(pending.publish().is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "other writer");
    }

    #[test]
    fn publication_replaces_one_name_without_mutating_sibling_hard_link() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.jsonl");
        let sibling = dir.path().join("sibling.jsonl");
        std::fs::write(&path, "old report").unwrap();
        std::fs::hard_link(&path, &sibling).unwrap();
        stage(&path).publish().unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "new report");
        assert_eq!(std::fs::read_to_string(sibling).unwrap(), "old report");
    }

    #[cfg(unix)]
    #[test]
    fn existing_symlink_targets_are_preserved_and_dangling_links_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.jsonl");
        let alias = dir.path().join("alias.jsonl");
        std::fs::write(&path, "old report").unwrap();
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        stage(&alias).publish().unwrap();
        assert!(alias.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new report");
        std::fs::remove_file(&path).unwrap();
        assert!(StagedReportDestination::admit(&alias, []).is_err());
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn device_output_remains_a_stream() {
        let admitted = StagedReportDestination::admit(Path::new("/dev/null"), []).unwrap();
        let mut writer = admitted.into_writer().unwrap();
        assert!(matches!(writer.publication, Publication::Stream));
        writer.write_all(b"streamed report").unwrap();
        writer.finish().unwrap().publish().unwrap();
    }
}
