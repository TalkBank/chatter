//! Receipt-private staging: no caller outside the receipt owner can publish.

use std::io::Write;
use std::path::{Path, PathBuf};

use talkbank_transform::pseudonymize::PseudonymizedDocument;
use tempfile::NamedTempFile;

/// Value-free filesystem failure: OS messages and paths can disclose names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum PublicationError {
    #[error("invalid private output destination")]
    Destination,
    #[error("cannot stage private output")]
    Stage,
    #[error("cannot publish private output without overwriting a destination")]
    Publish,
}

/// Staged bytes are not published output. Dropping this state removes only its
/// own temporary file; neither the source nor any existing destination is owned.
pub(super) struct PendingOutput {
    staged: NamedTempFile,
    destination: PathBuf,
}

/// Successful publication owns a handle to exactly the file it installed.
/// It deliberately does not delete that file on drop.
pub(super) struct PublishedOutput {
    file: std::fs::File,
    destination: PathBuf,
}

impl PendingOutput {
    /// Resolved private destination, for binding receipt identity before writes.
    pub(super) fn destination(&self) -> &Path {
        &self.destination
    }
    /// Only an admitted pseudonymized document can enter the output path.
    /// Temporary files are created beside the destination for same-filesystem
    /// publication. `NamedTempFile` creates owner-only files on Unix; on other
    /// systems callers must protect the destination directory's inherited ACL.
    pub(super) fn stage(
        output: &PseudonymizedDocument<'_>,
        destination: &Path,
    ) -> Result<Self, PublicationError> {
        let name = destination
            .file_name()
            .ok_or(PublicationError::Destination)?;
        let parent = destination.parent().ok_or(PublicationError::Destination)?;
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        let parent = parent
            .canonicalize()
            .map_err(|_| PublicationError::Destination)?;
        let destination = parent.join(name);
        // This rejects existing files, directories and dangling symlinks. It is
        // an early refusal only: persist_noclobber below closes the race.
        match destination.symlink_metadata() {
            Ok(_) => return Err(PublicationError::Destination),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(PublicationError::Destination),
        }
        let mut staged = NamedTempFile::new_in(&parent).map_err(|_| PublicationError::Stage)?;
        staged
            .write_all(output.text().as_bytes())
            .map_err(|_| PublicationError::Stage)?;
        staged
            .as_file()
            .sync_all()
            .map_err(|_| PublicationError::Stage)?;
        Ok(Self {
            staged,
            destination,
        })
    }

    /// Publish exactly the staged bytes. A destination created since staging
    /// causes refusal; it is never replaced. Receipt orchestration must record
    /// preparation durably before calling this transition.
    pub(super) fn publish(self) -> Result<PublishedOutput, PublicationError> {
        let file = self
            .staged
            .persist_noclobber(&self.destination)
            .map_err(|_| PublicationError::Publish)?;
        Ok(PublishedOutput {
            file,
            destination: self.destination,
        })
    }
}

impl PublishedOutput {
    /// Synchronize the installed file before receipt completion is recorded.
    /// This is not a claim of cross-file transaction or directory durability.
    pub(super) fn sync(&self) -> Result<(), PublicationError> {
        self.file.sync_all().map_err(|_| PublicationError::Publish)
    }

    /// Sensitive destination for the explicit private receipt, never logging.
    pub(super) fn destination(&self) -> &Path {
        &self.destination
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{RuleSelection, model::TranscriptName};
    use talkbank_parser::TreeSitterParser;
    use talkbank_transform::pseudonymize::NameMap;

    const SOURCE: &str =
        include_str!("../../../../../../corpus/reference/word-features/pseudonymizer-source.cha");
    const EXPECTED: &str =
        include_str!("../../../../../../corpus/reference/word-features/pseudonymizer-expected.cha");

    // No new scratch directory: the test owns this one auto-cleaned path in the
    // existing system temporary directory, retaining its cleanup guard.
    fn vacant_destination() -> tempfile::TempPath {
        let path = NamedTempFile::new().unwrap().into_temp_path();
        std::fs::remove_file(&path).unwrap();
        path
    }

    fn with_output(test: impl FnOnce(&PseudonymizedDocument<'_>)) {
        let parser = TreeSitterParser::new().unwrap();
        let map = NameMap::from_toml("version = 1\n[[transcripts]]\nkey = 'sample'\n[[transcripts.names]]\noriginal = 'Rose'\nreplacement = 'PersonA'\n", &parser).unwrap();
        let input = map
            .for_transcript("sample")
            .unwrap()
            .admit_document(
                SOURCE,
                TranscriptName::Anonymous,
                RuleSelection::new(),
                &parser,
            )
            .unwrap();
        let output = input.prepare_output(&parser).unwrap();
        test(&output);
    }

    #[test]
    fn admitted_golden_is_private_until_published_and_survives_handle_drop() {
        with_output(|output| {
            let path = vacant_destination();
            let pending = PendingOutput::stage(output, &path).unwrap();
            assert!(!path.exists());
            let published = pending.publish().unwrap();
            published.sync().unwrap();
            assert_eq!(
                std::fs::read_to_string(published.destination()).unwrap(),
                EXPECTED
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
            drop(published);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), EXPECTED);
        });
    }

    #[test]
    fn refuses_existing_destination_and_a_destination_created_after_staging() {
        with_output(|output| {
            let path = vacant_destination();
            let pending = PendingOutput::stage(output, &path).unwrap();
            std::fs::write(&path, "existing protected bytes").unwrap();
            assert!(matches!(pending.publish(), Err(PublicationError::Publish)));
            assert!(matches!(
                PendingOutput::stage(output, &path),
                Err(PublicationError::Destination)
            ));
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                "existing protected bytes"
            );
        });
    }

    #[test]
    fn abandoned_preparation_publishes_nothing() {
        with_output(|output| {
            let path = vacant_destination();
            let pending = PendingOutput::stage(output, &path).unwrap();
            let staging_path = pending.staged.path().to_owned();
            assert!(staging_path.exists());
            drop(pending);
            assert!(!staging_path.exists());
            assert!(!path.exists());
        });
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_never_followed_or_replaced() {
        with_output(|output| {
            let destination = vacant_destination();
            let target = vacant_destination();
            std::os::unix::fs::symlink(&target, &destination).unwrap();
            assert!(matches!(
                PendingOutput::stage(output, &destination),
                Err(PublicationError::Destination)
            ));
            assert_eq!(
                std::fs::read_link(&destination).unwrap(),
                target.as_ref() as &Path
            );
            assert!(!target.exists());
        });
    }
}
