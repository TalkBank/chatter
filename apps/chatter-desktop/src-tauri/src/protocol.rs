//! Shared desktop command and event contracts.
//!
//! TypeScript mirrors live in `desktop/src/protocol/desktopProtocol.ts`.

use serde::{Deserialize, Serialize};

pub mod commands {
    use super::*;

    pub const VALIDATE: &str = "validate";
    pub const CANCEL_VALIDATION: &str = "cancel_validation";
    pub const CHECK_CLAN_AVAILABLE: &str = "check_clan_available";
    pub const OPEN_IN_CLAN: &str = "open_in_clan";
    pub const EXPORT_RESULTS: &str = "export_results";
    pub const REVEAL_IN_FILE_MANAGER: &str = "reveal_in_file_manager";
    pub const INSTALL_CLI: &str = "install_cli";

    /// Which parser backend to validate with.
    ///
    /// Mirrors `talkbank_transform::validation_runner::ParserKind`, which has no
    /// `serde` derive of its own (it's a pure Rust-side validation-runner type);
    /// this is the serializable DTO the frontend settings panel sends. Convert
    /// with `From`/`Into` at the `validation.rs` boundary, never by hand.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum ParserKindRequest {
        #[default]
        TreeSitter,
        Re2c,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ValidateRequest {
        pub path: String,
        /// Run roundtrip test (serialize -> re-parse -> compare) after validation.
        pub roundtrip: bool,
        /// Which parser backend to use.
        pub parser_kind: ParserKindRequest,
        /// Enable strict cross-utterance linker validation (E351-E355).
        pub strict_linkers: bool,
        /// Number of parallel validation jobs (`None` = use all CPUs).
        pub jobs: Option<u32>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpenInClanRequest {
        pub file: String,
        pub line: i32,
        pub col: i32,
        pub byte_offset: u32,
        pub msg: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum ExportFormat {
        Json,
        Text,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ExportResultsRequest {
        pub results: String,
        pub format: ExportFormat,
        pub path: String,
    }
}

pub mod events {
    pub const VALIDATION: &str = "validation-event";
}
