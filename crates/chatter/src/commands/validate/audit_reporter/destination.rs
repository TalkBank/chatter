//! CLI wording and discovered-input adapter over shared staged publication.

use std::io;
use std::path::{Path, PathBuf};
use talkbank_transform::paths::ExpandedArguments;
pub(super) use talkbank_transform::report_destination::PendingPublication;
use talkbank_transform::report_destination::{
    ReportDestinationError, ReportWriter, StagedReportDestination,
};

pub(crate) struct AuditDestination(StagedReportDestination);

/// Failure to establish a source-preserving audit destination.
#[derive(Debug, thiserror::Error)]
pub enum AuditDestinationError {
    /// The opened report is one of the discovered inputs.
    #[error("audit output aliases input {}; choose a separate report path", .0.display())]
    SourceAlias(PathBuf),
    /// A discovered input could not be opened for identity admission.
    #[error("cannot establish audit input identity for {}: {source}", path.display())]
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

impl AuditDestination {
    pub(crate) fn admit(
        path: &Path,
        inputs: &ExpandedArguments,
    ) -> Result<Self, AuditDestinationError> {
        StagedReportDestination::admit(path, inputs.files().iter().map(|input| input.path()))
            .map(Self)
            .map_err(map_destination_error)
    }

    pub(super) fn into_writer(self) -> io::Result<ReportWriter> {
        self.0.into_writer()
    }
}

fn map_destination_error(error: ReportDestinationError) -> AuditDestinationError {
    match error {
        ReportDestinationError::SourceAlias(path) => AuditDestinationError::SourceAlias(path),
        ReportDestinationError::SourceIdentity { path, source } => {
            AuditDestinationError::SourceIdentity { path, source }
        }
        ReportDestinationError::Io(source) => AuditDestinationError::Io(source),
    }
}
