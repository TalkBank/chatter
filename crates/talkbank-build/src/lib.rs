//! Deterministic fingerprints of a crate's own packaged source tree.

use std::{fmt, fs, io, path::Path};

/// A complete source-tree digest, available only after successful admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceFingerprint(u64);

impl SourceFingerprint {
    /// Hash sorted relative paths and exact file bytes, failing on unreadable
    /// entries or symlinks. The root must be a directory, including when empty.
    pub fn read_tree(root: &Path) -> io::Result<Self> {
        let mut paths = Vec::new();
        collect(root, root, &mut paths)?;
        paths.sort();
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for relative in paths {
            let bytes = fs::read(root.join(&relative))?;
            // Length-prefix each component: source bytes may contain any
            // delimiter, and paths must not merge with adjacent file content.
            for component in [relative.as_bytes(), &bytes] {
                for byte in (component.len() as u64)
                    .to_le_bytes()
                    .iter()
                    .chain(component)
                {
                    hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        Ok(Self(hash))
    }
}

impl fmt::Display for SourceFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

fn collect(root: &Path, directory: &Path, paths: &mut Vec<String>) -> io::Result<()> {
    if !fs::symlink_metadata(directory)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source root must be a directory",
        ));
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_dir() {
            collect(root, &path, paths)?;
        } else if kind.is_file() {
            let relative = path.strip_prefix(root).map_err(io::Error::other)?;
            let relative = relative.to_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "source path is not UTF-8")
            })?;
            paths.push(relative.replace(std::path::MAIN_SEPARATOR, "/"));
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "source entry is not a regular file or directory: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn source_edits_additions_removals_and_relocation() -> std::io::Result<()> {
        let first = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;
        for root in [first.path(), second.path()] {
            fs::create_dir(root.join("nested"))?;
            fs::write(root.join("nested/parser.rs"), "old predicate")?;
        }
        let before = SourceFingerprint::read_tree(first.path())?;
        assert_eq!(before, SourceFingerprint::read_tree(second.path())?);
        fs::write(first.path().join("nested/parser.rs"), "new predicate")?;
        assert_ne!(before, SourceFingerprint::read_tree(first.path())?);
        fs::write(first.path().join("nested/parser.rs"), "old predicate")?;
        fs::write(first.path().join("lexer.re"), "a lexer rule")?;
        assert_ne!(before, SourceFingerprint::read_tree(first.path())?);
        fs::remove_file(first.path().join("lexer.re"))?;
        assert_eq!(before, SourceFingerprint::read_tree(first.path())?);
        fs::rename(
            first.path().join("nested/parser.rs"),
            first.path().join("nested/other.rs"),
        )?;
        assert_ne!(before, SourceFingerprint::read_tree(first.path())?);
        assert!(SourceFingerprint::read_tree(&first.path().join("absent")).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn source_links_cannot_reach_unowned_inputs() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("parser.rs"), "outside source")?;
        std::os::unix::fs::symlink(outside.path(), root.path().join("sibling"))?;
        assert!(SourceFingerprint::read_tree(root.path()).is_err());
        Ok(())
    }
}
