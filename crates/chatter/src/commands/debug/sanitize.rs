//! `chatter debug sanitize`, protected-corpus redaction.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use super::*;

/// Sanitize a single CHAT file and write the result to `output_path` (or
/// stdout when `None`).
///
/// Implements `chatter debug sanitize`. Runs the strict sanitization
/// policy from `talkbank-transform::redact`. See the book's sanitizer guide
/// for the retained metadata and manual privacy-review boundary.
///
/// A file that parses with diagnostics but still builds a model (a
/// healthy region alongside a malformed one) is sanitized anyway: the
/// diagnostics are reported on stderr as a warning, not treated as fatal.
/// Only a file that cannot build a model at all (see
/// [`talkbank_parser::ParseProduct::Unbuildable`]) aborts the run.
pub fn run_sanitize(input: &Path, output_path: Option<&Path>) {
    let mut source_handle = File::open(input)
        .and_then(same_file::Handle::from_file)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", input.display())));
    let mut source = String::new();
    source_handle
        .as_file_mut()
        .read_to_string(&mut source)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", input.display())));
    let parser = super::parser();
    let Some(parsed) = parse_or_report(&parser, input, &source) else {
        std::process::exit(1);
    };

    let policy = talkbank_transform::redact::SanitizationPolicy::strict();
    let sanitized = talkbank_transform::redact::sanitize(parsed, &policy)
        .unwrap_or_else(|e| die(&format!("sanitize failed for {}: {e}", input.display())));
    let chat_text = sanitized.to_chat_string();

    match output_path {
        Some(path) => DistinctOutput::open(&source_handle, path)
            .and_then(|output| output.write(chat_text.as_bytes()))
            .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display()))),
        None => std::io::stdout()
            .lock()
            .write_all(chat_text.as_bytes())
            .unwrap_or_else(|e| crate::stdout::closed(e)),
    }
}

/// An opened destination whose identity differs from the retained source.
/// Only this capability can truncate/write; neither operation reopens a path.
struct DistinctOutput(same_file::Handle);

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
    fn open(source: &same_file::Handle, path: &Path) -> Result<Self, OutputError> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let output = same_file::Handle::from_file(file)?;
        if source == &output {
            return Err(OutputError::SourceAlias);
        }
        Ok(Self(output))
    }

    fn write(mut self, bytes: &[u8]) -> Result<(), OutputError> {
        let file = self.0.as_file_mut();
        // Streams/devices have no replaceable length. Preserve their write
        // behavior while truncating ordinary files only after admission.
        if file.metadata()?.is_file() {
            file.set_len(0)?;
        }
        file.write_all(bytes)?;
        file.flush()?;
        Ok(())
    }
}
