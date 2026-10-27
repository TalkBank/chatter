//! Document link provider for CHAT files.
//!
//! Presents model-owned `@Media` references as remote URLs or local media links.

use std::path::Path;

use tower_lsp::lsp_types::*;

/// Present media references from the current source's shared model.
/// Rejected headers have no `Header::Media` value and cannot become links.
pub(crate) fn document_links(
    uri: &Url,
    analysis: &crate::backend::diagnostics::DocumentAnalysis,
) -> Vec<DocumentLink> {
    use crate::backend::utils::LineIndex;
    use talkbank_model::model::Header;

    let document = analysis.source();
    let file = analysis.file();
    let base_dir = uri
        .to_file_path()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let index = LineIndex::new(document);
    file.headers_with_locations()
        .filter_map(|(header, location)| {
            let Header::Media(media) = header else {
                return None;
            };
            let span = location.span()?;
            let header_source = document.get(span.start()..span.end())?;
            // Locate the already-parsed filename, rather than extracting a new
            // semantic value from delimiters. Require an exact source witness.
            let after_label = header_source.find(':')? + 1;
            let payload = header_source.get(after_label..)?.trim_start();
            let filename = media.filename.as_str();
            payload.strip_prefix(filename)?;
            let start = span.start() + header_source.len() - payload.len();
            let end = start + filename.len();
            let target = if media.filename.is_remote_url() {
                Url::parse(media.filename.unquoted()).ok()
            } else {
                base_dir
                    .as_deref()
                    .and_then(|directory| resolve_media_file(directory, media.filename.unquoted()))
                    .and_then(|path| Url::from_file_path(path).ok())
            };
            Some(DocumentLink {
                range: Range::new(
                    index.offset_to_position(start),
                    index.offset_to_position(end),
                ),
                target,
                tooltip: Some(format!("Open media file: {}", media.filename.unquoted())),
                data: None,
            })
        })
        .collect()
}

/// Try to resolve a media filename to an actual file path.
fn resolve_media_file(base_dir: &Path, filename: &str) -> Option<std::path::PathBuf> {
    // Try the filename as-is first.
    let direct = base_dir.join(filename);
    if direct.exists() {
        return Some(direct);
    }

    // Try common extensions if the filename has none.
    if Path::new(filename).extension().is_none() {
        for ext in &["wav", "mp3", "mp4", "m4a", "m4v", "mov", "ogg", "flac"] {
            let with_ext = base_dir.join(format!("{filename}.{ext}"));
            if with_ext.exists() {
                return Some(with_ext);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // document_links() calls uri.to_file_path() to derive the base
    // directory it resolves @Media filenames against. On Windows,
    // a file:///tmp/... URL has no Windows-valid file path so
    // to_file_path() returns Err(()) and the function early-returns
    // an empty Vec, failing every assertion below. Round-trip via
    // std::env::temp_dir() to get an OS-native path on every host.
    fn temp_cha_uri(name: &str) -> Url {
        Url::from_file_path(std::env::temp_dir().join(name))
            .expect("temp_dir() returns an absolute path")
    }

    #[test]
    fn test_document_links_finds_media_header() {
        let uri = temp_cha_uri("test.cha");
        let doc = "@UTF8\n@Begin\n@Media:\tmyfile, audio\n*CHI:\thello .\n@End\n";
        let analysis = crate::test_fixtures::document_analysis(&uri, doc);
        let links = document_links(&uri, &analysis);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].tooltip.as_deref(), Some("Open media file: myfile"));
        assert_eq!(links[0].range.start.line, 2);
    }

    #[test]
    fn test_no_links_without_media() {
        let uri = temp_cha_uri("test.cha");
        let doc = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let analysis = crate::test_fixtures::document_analysis(&uri, doc);
        let links = document_links(&uri, &analysis);
        assert!(links.is_empty());
    }
    #[test]
    fn document_links_resolve_local_unicode_media() {
        let directory = tempfile::tempdir().unwrap();
        let filename = "Media🦀";
        let recording = directory.path().join(format!("{filename}.wav"));
        std::fs::write(&recording, []).unwrap();
        let uri = Url::from_file_path(directory.path().join("example.cha")).unwrap();
        let source = include_str!("../../../../../corpus/reference/core/headers-media.cha")
            .replace("headers-media,", &format!("{filename},"));
        let analysis = crate::test_fixtures::document_analysis(&uri, &source);
        let links = document_links(&uri, &analysis);
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].target,
            Some(Url::from_file_path(recording).unwrap())
        );
        assert_eq!(
            links[0].range.end.character - links[0].range.start.character,
            filename.encode_utf16().count() as u32
        );
    }
}
