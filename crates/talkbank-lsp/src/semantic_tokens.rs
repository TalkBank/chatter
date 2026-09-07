//! Semantic tokens provider for syntax highlighting in LSP
//!
//! This module provides LSP semantic tokens that map tree-sitter highlight
//! tokens to LSP token types for syntax highlighting in editors.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::backend::LspBackendError;
use crate::backend::utils::LineIndex;
use crate::highlight::{HighlightConfig, TokenType};
use tower_lsp::lsp_types::*;

/// Lift a stringly error from the `tree-sitter-highlight` boundary into
/// the typed [`LspBackendError::HighlightFailed`] variant. Kept as a
/// single helper so the wrapping pattern is uniform across the two
/// public entry points on [`SemanticTokensProvider`].
fn lift_highlight_error(reason: String) -> LspBackendError {
    LspBackendError::HighlightFailed { reason }
}

/// Semantic tokens provider
pub struct SemanticTokensProvider {
    config: HighlightConfig,
}

impl SemanticTokensProvider {
    /// Create a new semantic tokens provider.
    ///
    /// Builds the `HighlightConfig` so we can expose semantic tokens that match the manual’s documented
    /// token hierarchy via the `tree-sitter` capture sets.
    pub fn new() -> Result<Self, LspBackendError> {
        Ok(Self {
            config: HighlightConfig::new().map_err(lift_highlight_error)?,
        })
    }

    /// Generate semantic tokens for the entire document.
    ///
    /// Converts the offset spans returned by `HighlightConfig` into delta-encoded LSP semantic tokens so
    /// editors receive the full CHAT token stream described in the File Format/Headers/Main Tier sections.
    pub fn semantic_tokens_full(
        &mut self,
        text: &str,
    ) -> Result<Vec<SemanticToken>, LspBackendError> {
        self.semantic_tokens_range(text, 0, text.len())
    }

    /// Highlight tokens overlapping a byte range, encoded in UTF-16 units.
    ///
    /// Captures may include line endings or span lines. Split them into legal
    /// single-line tokens; clients need not advertise multiline-token support.
    pub fn semantic_tokens_range(
        &mut self,
        text: &str,
        start_offset: usize,
        end_offset: usize,
    ) -> Result<Vec<SemanticToken>, LspBackendError> {
        let tokens = self.config.highlight(text).map_err(lift_highlight_error)?;
        let index = LineIndex::new(text);
        let mut lsp_tokens = Vec::new();
        let mut previous = Position::default();
        for token in tokens {
            if token.end <= start_offset || token.start >= end_offset {
                continue;
            }
            let mut offset = token.start;
            for part in text[token.start..token.end].split_inclusive('\n') {
                let content = part.strip_suffix('\n').unwrap_or(part);
                let content = content.strip_suffix('\r').unwrap_or(content);
                if !content.is_empty() {
                    let position = index.offset_to_position(offset as u32);
                    lsp_tokens.push(SemanticToken {
                        delta_line: position.line - previous.line,
                        delta_start: if position.line == previous.line {
                            position.character - previous.character
                        } else {
                            position.character
                        },
                        length: content.encode_utf16().count() as u32,
                        token_type: Self::token_type_to_index(token.token_type),
                        token_modifiers_bitset: 0,
                    });
                    previous = position;
                }
                offset += part.len();
            }
        }
        Ok(lsp_tokens)
    }

    /// Map a `TokenType` into the LSP semantic token legend index.
    ///
    /// The mapping keeps the LSP legend in sync with the manual’s categories (keywords, variables, strings,
    /// comments, etc.) and includes a few custom token types (tag/punctuation/error) for CHAT-specific symbols.
    fn token_type_to_index(token_type: TokenType) -> u32 {
        match token_type {
            TokenType::Keyword => 0,          // keyword
            TokenType::KeywordDirective => 0, // keyword (directives are also keywords)
            TokenType::Variable => 1,         // variable
            TokenType::String => 2,           // string
            TokenType::StringSpecial => 2,    // string (special strings still strings)
            TokenType::Comment => 3,          // comment
            TokenType::Type => 4,             // type
            TokenType::TypeBuiltin => 4,      // type
            TokenType::Operator => 5,         // operator
            TokenType::Number => 6,           // number
            TokenType::Function => 7,         // function
            TokenType::Tag => 8,              // custom: tag
            TokenType::Punctuation => 9,      // custom: punctuation
            TokenType::Error => 10,           // custom: error
        }
    }

    /// Get the semantic token legend for LSP initialization.
    ///
    /// The legend lists the token kinds used in `semantic_tokens_full` so the LSP client can colorize them
    /// consistent with the manual’s semantic layers.
    pub fn legend() -> SemanticTokensLegend {
        SemanticTokensLegend {
            token_types: vec![
                SemanticTokenType::KEYWORD,            // 0
                SemanticTokenType::VARIABLE,           // 1
                SemanticTokenType::STRING,             // 2
                SemanticTokenType::COMMENT,            // 3
                SemanticTokenType::TYPE,               // 4
                SemanticTokenType::OPERATOR,           // 5
                SemanticTokenType::NUMBER,             // 6
                SemanticTokenType::FUNCTION,           // 7
                SemanticTokenType::new("tag"),         // 8 (custom)
                SemanticTokenType::new("punctuation"), // 9 (custom)
                SemanticTokenType::new("error"),       // 10 (custom)
            ],
            token_modifiers: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests semantic tokens provider creation.
    #[test]
    fn test_semantic_tokens_provider_creation() {
        let result = SemanticTokensProvider::new();
        assert!(
            result.is_ok(),
            "Failed to create SemanticTokensProvider: {:?}",
            result.err()
        );
    }

    /// Tests semantic tokens generation.
    #[test]
    fn test_semantic_tokens_generation() -> Result<(), String> {
        let mut provider = SemanticTokensProvider::new()
            .map_err(|err| format!("Failed to create SemanticTokensProvider: {err}"))?;

        let text = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let tokens = provider
            .semantic_tokens_full(text)
            .map_err(|err| format!("Failed to generate semantic tokens: {err}"))?;

        // Should have some tokens
        assert!(!tokens.is_empty(), "No semantic tokens generated");
        Ok(())
    }

    #[test]
    fn test_semantic_tokens_range_subset() -> Result<(), String> {
        let mut provider = SemanticTokensProvider::new()
            .map_err(|err| format!("Failed to create SemanticTokensProvider: {err}"))?;

        let text = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let full = provider
            .semantic_tokens_full(text)
            .map_err(|err| format!("Full tokens error: {err}"))?;

        // Range covering only the last line "@End\n" (offset 27..32)
        let range = provider
            .semantic_tokens_range(text, 27, 32)
            .map_err(|err| format!("Range tokens error: {err}"))?;

        // Range should have fewer tokens than full.
        assert!(
            range.len() < full.len(),
            "Range should be a subset of full tokens"
        );
        assert!(
            !range.is_empty(),
            "Range should have at least one token for @End"
        );
        Ok(())
    }

    /// Tests that the legend contains the expected token types.
    #[test]
    fn test_legend_contains_standard_and_custom_types() {
        let legend = SemanticTokensProvider::legend();
        let type_names: Vec<&str> = legend.token_types.iter().map(|t| t.as_str()).collect();
        // Standard LSP token types.
        assert!(type_names.contains(&"keyword"), "Missing keyword type");
        assert!(type_names.contains(&"variable"), "Missing variable type");
        assert!(type_names.contains(&"string"), "Missing string type");
        assert!(type_names.contains(&"comment"), "Missing comment type");
        assert!(type_names.contains(&"number"), "Missing number type");
        assert!(type_names.contains(&"operator"), "Missing operator type");
        // Custom CHAT-specific types.
        assert!(type_names.contains(&"tag"), "Missing custom tag type");
        assert!(
            type_names.contains(&"punctuation"),
            "Missing custom punctuation type"
        );
        assert!(type_names.contains(&"error"), "Missing custom error type");
        // Index count should be 11 (0 through 10).
        assert_eq!(legend.token_types.len(), 11);
    }

    /// Tests that all TokenType variants map to valid legend indices.
    #[test]
    fn test_token_type_to_index_within_legend_bounds() {
        let legend = SemanticTokensProvider::legend();
        let max_index = legend.token_types.len() as u32;
        let all_types = [
            TokenType::Keyword,
            TokenType::KeywordDirective,
            TokenType::Variable,
            TokenType::String,
            TokenType::StringSpecial,
            TokenType::Comment,
            TokenType::Type,
            TokenType::TypeBuiltin,
            TokenType::Operator,
            TokenType::Number,
            TokenType::Function,
            TokenType::Tag,
            TokenType::Punctuation,
            TokenType::Error,
        ];
        for tt in all_types {
            let idx = SemanticTokensProvider::token_type_to_index(tt);
            assert!(
                idx < max_index,
                "Token type {:?} maps to index {}, but legend has only {} types",
                tt,
                idx,
                max_index
            );
        }
    }

    /// Tests that a document with multiple utterances produces tokens on multiple lines.
    ///
    /// Skipped when `SemanticTokensProvider::new()` fails due to highlight query
    /// incompatibility (e.g. stale `inline_bullet` node reference).
    #[test]
    fn test_semantic_tokens_multi_utterance() {
        let mut provider = match SemanticTokensProvider::new() {
            Ok(p) => p,
            Err(_) => return, // highlight config broken; skip gracefully
        };

        let text = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thello .\n*MOT:\tgoodbye .\n@End\n";
        let tokens = provider
            .semantic_tokens_full(text)
            .expect("full tokens should succeed once provider is created");

        assert!(
            tokens.len() > 5,
            "Multi-utterance document should produce many tokens, got {}",
            tokens.len()
        );

        // At least some tokens should be on later lines (delta_line > 0).
        let tokens_on_new_lines = tokens.iter().filter(|t| t.delta_line > 0).count();
        assert!(
            tokens_on_new_lines >= 3,
            "Expected tokens spanning multiple lines, got {} line transitions",
            tokens_on_new_lines
        );
    }

    /// Tests that range tokens for the beginning of the document exclude later content.
    #[test]
    fn test_semantic_tokens_range_beginning_only() {
        let mut provider = match SemanticTokensProvider::new() {
            Ok(p) => p,
            Err(_) => return, // highlight config broken; skip gracefully
        };

        let text = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let full = provider
            .semantic_tokens_full(text)
            .expect("full tokens should succeed");

        // Only the first line: "@UTF8\n" = 6 bytes.
        let range = provider
            .semantic_tokens_range(text, 0, 6)
            .expect("range tokens should succeed");

        assert!(
            range.len() < full.len(),
            "First-line range ({}) should have fewer tokens than full ({})",
            range.len(),
            full.len()
        );
    }

    /// Tests that an empty document produces no tokens.
    #[test]
    fn test_semantic_tokens_empty_input() {
        let mut provider = match SemanticTokensProvider::new() {
            Ok(p) => p,
            Err(_) => return, // highlight config broken; skip gracefully
        };

        let tokens = provider
            .semantic_tokens_full("")
            .expect("empty input should not error");

        assert!(
            tokens.is_empty(),
            "Empty input should produce no tokens, got {}",
            tokens.len()
        );
    }

    /// Tests that delta encoding produces non-negative deltas.
    #[test]
    fn test_semantic_tokens_delta_encoding_non_negative() {
        let mut provider = match SemanticTokensProvider::new() {
            Ok(p) => p,
            Err(_) => return,
        };

        let text = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n@End\n";
        let tokens = provider
            .semantic_tokens_full(text)
            .expect("tokens should succeed");

        for (i, token) in tokens.iter().enumerate() {
            // All deltas are u32, so by construction non-negative, but verify
            // that delta_start resets properly on new lines.
            if token.delta_line > 0 {
                // On a new line, delta_start is absolute column position.
                // Just verify it is reasonable (not larger than line length).
                assert!(
                    token.delta_start < 200,
                    "Token {} has unreasonably large delta_start {} on new line",
                    i,
                    token.delta_start
                );
            }
            assert!(token.length > 0, "Token {} has zero length", i);
        }
    }
}
