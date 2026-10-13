//! Native admission of regular-file kind, not permanent filesystem validity.

/// Only this module can construct evidence that a native type was a file.
/// The proof is consumed by stored-transcript admission; it cannot guarantee
/// that an external actor will not replace the entry after the observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegularFileKind(());

impl RegularFileKind {
    pub(super) fn admit(file_type: std::fs::FileType) -> std::io::Result<Self> {
        if file_type.is_file() {
            Ok(Self(()))
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "input is not a regular file",
            ))
        }
    }
}
