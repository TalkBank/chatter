//! Complete a write in an exclusively owned sibling before replacing a file.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

/// Only a fully written and flushed sibling can be published. Dropping this
/// state removes its owned temporary file without touching the destination.
/// Publication is one rename, not a multi-file transaction or durability proof.
pub(crate) struct PreparedFile {
    temporary: NamedTempFile,
    destination: PathBuf,
}

impl PreparedFile {
    pub(crate) fn prepare(destination: &Path, content: &[u8]) -> io::Result<Self> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::Builder::new()
            .prefix(".chatter-")
            .tempfile_in(parent)?;
        temporary.write_all(content)?;
        temporary.flush()?;
        Ok(Self {
            temporary,
            destination: destination.to_path_buf(),
        })
    }

    pub(crate) fn publish(self) -> io::Result<()> {
        self.temporary
            .persist(self.destination)
            .map(|_| ())
            .map_err(|error| error.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn abandoning_prepared_bytes_preserves_destination_and_cleans_owned_sibling() -> io::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let destination = directory.path().join("record.toml");
        fs::write(&destination, b"original record")?;
        let prepared = PreparedFile::prepare(&destination, b"replacement record")?;
        let temporary = prepared.temporary.path().to_owned();
        assert_eq!(fs::read(&temporary)?, b"replacement record");
        drop(prepared);
        assert!(!temporary.try_exists()?);
        assert_eq!(fs::read(destination)?, b"original record");
        Ok(())
    }

    #[test]
    fn refused_publication_keeps_destination_and_cleans_owned_sibling() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let destination = directory.path().join("record.toml");
        fs::create_dir(&destination)?;
        let retained = destination.join("retained.txt");
        fs::write(&retained, b"original evidence")?;
        let prepared = PreparedFile::prepare(&destination, b"replacement record")?;
        let temporary = prepared.temporary.path().to_owned();
        assert!(prepared.publish().is_err());
        assert!(!temporary.try_exists()?);
        assert_eq!(fs::read(retained)?, b"original evidence");
        Ok(())
    }
}
