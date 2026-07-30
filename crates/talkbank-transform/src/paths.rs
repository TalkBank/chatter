//! CHAT transcript path classification.
//!
//! Lives at the crate root, OUTSIDE the `validation-runner` feature, because
//! path classification has nothing to do with the SQLite result cache: the
//! corpus manifest walk and the CLI-side walks need it on every build,
//! including `default-features = false` consumers that opt out of the
//! validation runner entirely.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::Path;

/// Return `true` when `path` is a CHAT transcript we should collect: a `.cha`
/// file that is not a macOS AppleDouble sidecar (`._name.cha`). Shared by the
/// transform-side directory walks and the CLI-side walk so the two never
/// drift in what they treat as a transcript.
pub fn is_chat_transcript_path(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()) == Some("cha")
        && !path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| name.starts_with("._"))
}
