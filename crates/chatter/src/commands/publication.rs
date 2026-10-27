//! CHAT publication boundary. No existing destination is opened for writing.
//!
//! Admission -> fully written, metadata-preserved and synchronized staging ->
//! atomic publication. These are ordinary local-filesystem guarantees, not a
//! compare-and-swap against concurrent writers or directory crash durability.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use talkbank_transform::Rewrite;
use tempfile::NamedTempFile;

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[cfg_attr(target_os = "linux", path = "publication/linux.rs")]
#[cfg_attr(windows, path = "publication/windows.rs")]
mod metadata;

/// Publication failures remain distinct from parser/content failures.
#[derive(Debug, thiserror::Error)]
pub(super) enum PublicationError {
    #[error("output must name a file in an existing directory")]
    OutputPath,
    #[error("publication refuses symlink and nonregular output targets")]
    UnsupportedTarget,
    #[error("publication refuses a read-only output file")]
    ReadOnly,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    #[error("publication refuses multiply-linked output files")]
    MultipleLinks,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    #[error("publication refuses unsupported output file flags")]
    FileFlags,
    #[error("output changed before publication")]
    Changed,
    #[error("the prepared edit belongs to a different source")]
    WrongSource,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    #[error("output metadata could not be preserved")]
    MetadataPreservation,
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    #[error(
        "existing-output metadata preservation is not qualified on this platform; use a new output path"
    )]
    UnsupportedPlatform,
    #[error(transparent)]
    Io(#[from] io::Error),
}

type Result<T> = std::result::Result<T, PublicationError>;

/// A destination admitted under the metadata-preserving replacement policy.
/// Only `admit` constructs it; there is no public raw-parts constructor.
enum Destination {
    New(PathBuf),
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    Replace(metadata::ExistingFile),
}

/// Completed staging; it cannot be constructed from incomplete output bytes.
/// `Destination::stage` is its only constructor, after write, metadata and sync.
struct StagedOutput {
    file: NamedTempFile,
    destination: Destination,
}

/// Proof that publication, rather than merely serialization, succeeded.
/// Both constructors are in `StagedOutput::publish`, after successful persistence.
pub(super) struct PublishedOutput(PathBuf);

impl PublishedOutput {
    /// The resolved destination that was published.
    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Destination {
    fn admit(path: &Path) -> Result<Self> {
        let name = path.file_name().ok_or(PublicationError::OutputPath)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let resolved = fs::canonicalize(parent)?.join(name);
        match fs::symlink_metadata(&resolved) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::New(resolved)),
            Err(error) => Err(error.into()),
            Ok(attributes) => {
                if !attributes.is_file() {
                    return Err(PublicationError::UnsupportedTarget);
                }
                if attributes.permissions().readonly() {
                    return Err(PublicationError::ReadOnly);
                }
                #[cfg(any(target_os = "macos", target_os = "linux", windows))]
                {
                    metadata::ExistingFile::admit(resolved, attributes).map(Self::Replace)
                }
                #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
                {
                    Err(PublicationError::UnsupportedPlatform)
                }
            }
        }
    }

    fn path(&self) -> &Path {
        match self {
            Self::New(path) => path,
            #[cfg(any(target_os = "macos", target_os = "linux", windows))]
            Self::Replace(existing) => existing.path(),
        }
    }

    fn stage(self, text: &str) -> Result<StagedOutput> {
        let parent = self.path().parent().ok_or(PublicationError::OutputPath)?;
        let mut builder = tempfile::Builder::new();
        builder.prefix(".chatter-output-");
        // Match ordinary new-file permissions (subject to umask). Replacement
        // stages stay private until the old metadata has been copied.
        #[cfg(unix)]
        match &self {
            Self::New(_) => {
                use std::os::unix::fs::PermissionsExt;
                builder.permissions(fs::Permissions::from_mode(0o666));
            }
            #[cfg(any(target_os = "macos", target_os = "linux", windows))]
            Self::Replace(_) => {}
        }
        let file = builder.tempfile_in(parent)?;
        #[cfg(windows)]
        let file = match &self {
            Self::New(_) => file,
            Self::Replace(existing) => existing.prepare_stage(file)?,
        };
        let mut file = file;
        file.write_all(text.as_bytes())?;
        match &self {
            Self::New(_) => {}
            #[cfg(any(target_os = "macos", target_os = "linux", windows))]
            Self::Replace(existing) => existing.preserve_on(file.as_file())?,
        }
        file.as_file().sync_all()?;
        Ok(StagedOutput {
            file,
            destination: self,
        })
    }
}

impl StagedOutput {
    fn publish(self) -> Result<PublishedOutput> {
        let Self { file, destination } = self;
        match destination {
            Destination::New(path) => {
                file.persist_noclobber(&path).map_err(|error| error.error)?;
                Ok(PublishedOutput(path))
            }
            #[cfg(any(target_os = "macos", target_os = "linux", windows))]
            Destination::Replace(existing) => {
                let path = existing.recheck()?;
                talkbank_transform::file_replacement::FileReplacement::prepare(file, &path)?
                    .publish()?;
                Ok(PublishedOutput(path))
            }
        }
    }
}

/// Consume the filesystem transitions without weakening the rewrite's content guard.
pub(super) fn publish(
    source: &SourceSnapshot,
    path: &Path,
    rewrite: Rewrite,
) -> Result<PublishedOutput> {
    let destination = Destination::admit(path)?;
    source.recheck(source.path())?;
    let staged = destination.stage(rewrite.text())?;
    source.recheck(source.path())?;
    staged.publish()
}

/// The bytes used for planning and the identity of the file they came from.
/// Retaining the handle prevents path replacement from impersonating this source.
pub(super) struct SourceSnapshot {
    path: PathBuf,
    identity: same_file::Handle,
    text: String,
}

impl SourceSnapshot {
    pub(super) fn read(path: &Path) -> Result<Self> {
        let mut file = fs::File::open(path)?;
        if !file.metadata()?.is_file() {
            return Err(PublicationError::UnsupportedTarget);
        }
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        Ok(Self {
            path: path.to_owned(),
            identity: same_file::Handle::from_file(file)?,
            text,
        })
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    fn recheck(&self, destination: &Path) -> Result<()> {
        let mut file = fs::File::open(destination)?;
        let identity = same_file::Handle::from_file(file.try_clone()?)?;
        let mut current = String::new();
        file.read_to_string(&mut current)?;
        if identity != self.identity || current != self.text {
            return Err(PublicationError::Changed);
        }
        Ok(())
    }
}

/// A staged replacement retains its planning source until publication.
struct StagedReplacement<'source> {
    source: &'source SourceSnapshot,
    output: StagedOutput,
}

impl<'source> StagedReplacement<'source> {
    // Private: callers must enter through a content-admitted fix or transform.
    fn stage(source: &'source SourceSnapshot, text: &str) -> Result<Self> {
        let destination = Destination::admit(source.path())?;
        source.recheck(destination.path())?;
        Ok(Self {
            source,
            output: destination.stage(text)?,
        })
    }

    fn publish(self) -> Result<PublishedOutput> {
        self.source.recheck(self.output.destination.path())?;
        self.output.publish()
    }
}

/// Consume a policy-checked fix; a raw string cannot enter this writer.
pub(super) fn publish_fix(
    source: &SourceSnapshot,
    fix: super::fix::PostCheckedFix<'_>,
) -> Result<PublishedOutput> {
    if source.text() != fix.source() {
        return Err(PublicationError::WrongSource);
    }
    StagedReplacement::stage(source, fix.text())?.publish()
}

/// Consume the transform's byte-reproduction admission and edited model.
pub(super) fn publish_transform(edit: super::debug::InPlace) -> Result<PublishedOutput> {
    use talkbank_model::WriteChat;
    let (source, model) = edit.into_parts();
    StagedReplacement::stage(&source, &model.to_chat_string())?.publish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_snapshot_refuses_changed_bytes_and_replaced_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.cha");
        fs::write(&path, "original").unwrap();
        let source = SourceSnapshot::read(&path).unwrap();
        fs::write(&path, "modified").unwrap();
        assert!(matches!(
            source.recheck(&path),
            Err(PublicationError::Changed)
        ));
        fs::rename(&path, directory.path().join("old.cha")).unwrap();
        fs::write(&path, "original").unwrap();
        assert!(matches!(
            source.recheck(&path),
            Err(PublicationError::Changed)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    }

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    #[test]
    fn replacement_refuses_a_source_changed_before_or_after_staging() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.cha");
        fs::write(&path, "original").unwrap();
        let source = SourceSnapshot::read(&path).unwrap();
        fs::write(&path, "changed before staging").unwrap();
        assert!(matches!(
            StagedReplacement::stage(&source, "edit"),
            Err(PublicationError::Changed)
        ));
        let source = SourceSnapshot::read(&path).unwrap();
        let staged = StagedReplacement::stage(&source, "edit").unwrap();
        fs::write(&path, "changed after staging").unwrap();
        assert!(matches!(staged.publish(), Err(PublicationError::Changed)));
        assert_eq!(fs::read_to_string(&path).unwrap(), "changed after staging");
    }

    fn rewrite() -> Rewrite {
        talkbank_transform::normalize_chat(
            include_str!("../../../../corpus/reference/annotation/retrace.cha"),
            talkbank_model::ParseValidateOptions::default(),
        )
        .expect("accepted reference rewrite")
    }

    #[test]
    fn creation_race_never_clobbers_the_winner() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.cha");
        let staged = Destination::admit(&path)
            .unwrap()
            .stage(rewrite().text())
            .unwrap();
        fs::write(&path, b"other writer").unwrap();
        assert!(staged.publish().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"other writer");
    }

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    #[test]
    fn replacement_race_refuses_changed_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.cha");
        fs::write(&path, b"old output").unwrap();
        let staged = Destination::admit(&path)
            .unwrap()
            .stage(rewrite().text())
            .unwrap();
        fs::rename(&path, directory.path().join("kept-old.cha")).unwrap();
        fs::write(&path, b"other writer").unwrap();
        assert!(staged.publish().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"other writer");
        assert_eq!(
            fs::read(directory.path().join("kept-old.cha")).unwrap(),
            b"old output"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn late_rename_failure_keeps_original_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.cha");
        fs::write(&path, b"old output").unwrap();
        let status = std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone deny delete"])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let staged = Destination::admit(&path)
            .unwrap()
            .stage(rewrite().text())
            .unwrap();
        let result = staged.publish();
        // The copied deny-delete ACL can prevent best-effort stage cleanup.
        // Remove only ACLs within this test-owned directory before assertions.
        for entry in fs::read_dir(directory.path()).unwrap() {
            let status = std::process::Command::new("/bin/chmod")
                .arg("-N")
                .arg(entry.unwrap().path())
                .status()
                .unwrap();
            assert!(status.success());
        }
        assert!(
            matches!(result, Err(PublicationError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied)
        );
        assert_eq!(fs::read(&path).unwrap(), b"old output");
    }
}
