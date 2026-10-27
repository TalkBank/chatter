//! `chatter experimental transform redact`, protected-corpus redaction.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use talkbank_transform::report_destination::{ReportDestinationError, StagedReportDestination};

use super::*;

/// Sanitize a single CHAT file and write the result to `output_path` (or
/// stdout when `None`).
///
/// Implements `chatter experimental transform redact`. Runs the strict sanitization
/// policy from `talkbank-transform::redact`. See the book's sanitizer guide
/// for the retained metadata and manual privacy-review boundary.
///
/// Source-bound sanitization checks the unedited model for dropped/reordered
/// source content before redacting it. Recovery diagnostics are reported, but
/// are not themselves fatal. Internal faults, unbuildable parses, source loss
/// and unsupported lexical recovery refuse before opening any destination.
pub fn run_sanitize(input: &Path, output_path: Option<&Path>) -> crate::exit_codes::CommandResult {
    let mut source_handle = File::open(input)
        .and_then(same_file::Handle::from_file)
        .map_err(|e| die(&format!("cannot read {}: {e}", input.display())))?;
    let mut source = String::new();
    source_handle
        .as_file_mut()
        .read_to_string(&mut source)
        .map_err(|e| die(&format!("cannot read {}: {e}", input.display())))?;
    let parser = super::parser()?;
    let errors = talkbank_model::ErrorCollector::new();
    let policy = talkbank_transform::redact::SanitizationPolicy::strict();
    let result =
        talkbank_transform::parse_source_with_parser(&parser, &source).sanitize(&policy, &errors);
    let diagnostics = errors.into_vec();
    if !diagnostics.is_empty() {
        eprintln!(
            "WARNING: {} parse diagnostic(s) for {}: {}",
            diagnostics.len(),
            input.display(),
            talkbank_model::ParseErrors::from(diagnostics)
        );
    }
    let sanitized =
        result.map_err(|e| die(&format!("sanitize failed for {}: {e}", input.display())))?;
    let chat_text = sanitized.to_chat_string();

    match output_path {
        Some(path) => DistinctOutput::open(input, &source_handle, path)
            .and_then(|output| output.write(chat_text.as_bytes()))
            .map_err(|e| die(&format!("cannot write {}: {e}", path.display())))?,
        None => out!("{chat_text}"),
    }
    Ok(())
}

/// A staged destination distinct from the retained source identity.
struct DistinctOutput(StagedReportDestination);

#[derive(Debug, thiserror::Error)]
enum OutputError {
    #[error(
        "refusing to write the sanitized copy over its own input; sanitization is lossy, so name a different -o path"
    )]
    SourceAlias,
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl DistinctOutput {
    fn open(input: &Path, source: &same_file::Handle, path: &Path) -> Result<Self, OutputError> {
        StagedReportDestination::admit_opened(path, input, source)
            .map(Self)
            .map_err(|error| match error {
                ReportDestinationError::SourceAlias(_) => OutputError::SourceAlias,
                ReportDestinationError::Io(error)
                | ReportDestinationError::SourceIdentity { source: error, .. } => {
                    OutputError::Io(error)
                }
            })
    }

    fn write(self, bytes: &[u8]) -> Result<(), OutputError> {
        let mut writer = self.0.into_writer()?;
        writer.write_all(bytes)?;
        writer.finish()?.publish()?;
        Ok(())
    }
}
