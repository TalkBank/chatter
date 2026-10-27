//! One same-volume replacement boundary for fully staged files.
//!
//! Callers own content, alias and metadata policy. This boundary owns flushing,
//! synchronization and a consuming rename; it never deletes or copies the target
//! as a fallback. Existing readers may continue reading the replaced file.

use std::io::{self, Write};
use std::path::Path;
use tempfile::NamedTempFile;

#[cfg(windows)]
mod windows;

/// Synchronized staging admitted for one replacement. Dropping it abandons the
/// owned temporary file. It does not authorize overwriting any particular input;
/// that decision belongs to the caller's content and destination policy.
pub struct FileReplacement {
    #[cfg(windows)]
    rename: windows::PreparedRename,
    #[cfg(not(windows))]
    staged: NamedTempFile,
    #[cfg(not(windows))]
    destination: std::path::PathBuf,
}

impl FileReplacement {
    /// Finish staging and acquire the platform's rename capability. The target
    /// may exist; callers requiring exclusive creation must use no-clobber
    /// publication instead. No destination bytes are changed here.
    pub fn prepare(mut staged: NamedTempFile, destination: &Path) -> io::Result<Self> {
        staged.flush()?;
        staged.as_file().sync_all()?;
        #[cfg(windows)]
        {
            windows::PreparedRename::prepare(staged, destination).map(|rename| Self { rename })
        }
        #[cfg(not(windows))]
        {
            Ok(Self {
                staged,
                destination: destination.to_owned(),
            })
        }
    }

    /// Replace one directory entry. Failure leaves the destination in place;
    /// publication is not a multi-file transaction or directory durability proof.
    pub fn publish(self) -> io::Result<()> {
        #[cfg(windows)]
        {
            self.rename.publish()
        }
        #[cfg(not(windows))]
        {
            self.staged
                .persist(self.destination)
                .map(|_| ())
                .map_err(|error| error.error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Read;

    fn prepare(destination: &Path) -> io::Result<(FileReplacement, std::path::PathBuf)> {
        let mut staged = NamedTempFile::new_in(destination.parent().unwrap())?;
        staged.write_all(b"new bytes")?;
        let path = staged.path().to_owned();
        Ok((FileReplacement::prepare(staged, destination)?, path))
    }

    #[test]
    fn replacement_retains_open_reader_and_hard_link_bytes() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        // Exercises UTF-16 length, including a surrogate pair, on Windows.
        let destination = directory.path().join("rapport-\u{e9}-\u{1f4dd}.txt");
        let sibling = directory.path().join("sibling.txt");
        fs::write(&destination, b"old bytes")?;
        fs::hard_link(&destination, &sibling)?;
        let mut reader = fs::File::open(&destination)?;
        let identity = same_file::Handle::from_file(reader.try_clone()?)?;
        let (replacement, temporary) = prepare(&destination)?;
        assert_eq!(fs::read(&destination)?, b"old bytes");
        replacement.publish()?;
        assert!(!temporary.try_exists()?);
        assert_eq!(fs::read(&destination)?, b"new bytes");
        assert_eq!(fs::read(&sibling)?, b"old bytes");
        let mut old = String::new();
        reader.read_to_string(&mut old)?;
        assert_eq!(old, "old bytes");
        assert_ne!(identity, same_file::Handle::from_path(&destination)?);
        assert_eq!(identity, same_file::Handle::from_path(&sibling)?);
        Ok(())
    }

    #[test]
    fn abandonment_and_refusal_clean_only_the_owned_stage() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let destination = directory.path().join("report");
        fs::write(&destination, b"old bytes")?;
        let (replacement, temporary) = prepare(&destination)?;
        drop(replacement);
        assert!(!temporary.try_exists()?);
        assert_eq!(fs::read(&destination)?, b"old bytes");
        fs::remove_file(&destination)?;
        fs::create_dir(&destination)?;
        fs::write(destination.join("retained"), b"old bytes")?;
        let (replacement, temporary) = prepare(&destination)?;
        assert!(replacement.publish().is_err());
        assert!(!temporary.try_exists()?);
        assert_eq!(fs::read(destination.join("retained"))?, b"old bytes");
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn readonly_target_is_not_overridden() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let destination = directory.path().join("readonly.txt");
        fs::write(&destination, b"old bytes")?;
        let writable = fs::metadata(&destination)?.permissions();
        let mut readonly = writable.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&destination, readonly)?;
        let (replacement, temporary) = prepare(&destination)?;
        let result = replacement.publish();
        fs::set_permissions(&destination, writable)?;
        assert!(result.is_err());
        assert!(!temporary.try_exists()?);
        assert_eq!(fs::read(&destination)?, b"old bytes");
        Ok(())
    }
}
