//! Generic dependent-tier parsing entrypoint.
//!
//! This parser extracts a `%label:\tcontent` line into `UserDefinedTier` without
//! knowing the tier semantics ahead of time. Typed tier parsers can then consume
//! the result when appropriate.
//!
//! ## Implementation Strategy
//!
//! Uses the **synthesis pattern**: wraps the tier in a minimal valid CHAT file,
//! parses with tree-sitter to get proper CST structure, then extracts the tier node.
//! This avoids all text hacking while providing granular parsing APIs.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use crate::generated_traversal::{
    AsRawNode, DependentTierChoice, NodeSlot, extract_dependent_tier,
};
use talkbank_model::ParseOutcome;
use talkbank_model::model::UserDefinedTier;
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Converts a dependent-tier line using the **synthesis pattern**.
///
/// Wraps the tier in a minimal valid CHAT file, parses with tree-sitter to get proper
/// CST structure, then extracts the tier node. This provides granular parsing without text hacking.
///
/// # Format
/// `%tiertype:\tcontent` (tab after colon)
///
/// # Common tier types
/// - `%mor`: Morphological analysis
/// - `%pho`: Phonological transcription
/// - `%gra`: Grammatical relations
/// - `%com`: Comment
/// - `%act`: Action descriptions
///
/// # Examples
///
/// ```ignore
/// use talkbank_model::ErrorCollector;
/// use talkbank_model::ParseOutcome;
///
/// let errors = ErrorCollector::new();
/// if let ParseOutcome::Parsed(tier) = parse_dependent_tier("%mor:\tpro|I v|be&1S .", &errors) {
///     assert_eq!(tier.label, "mor");
/// }
///
/// if let ParseOutcome::Parsed(tier) = parse_dependent_tier("%pho:\ta b c", &errors) {
///     assert_eq!(tier.label, "pho");
///     assert_eq!(tier.content, "a b c");
/// }
/// ```
///
/// # Implementation
///
/// Uses tree-sitter synthesis pattern:
/// 1. Wrap tier in minimal CHAT file
/// 2. Parse entire file with tree-sitter
/// 3. Extract dependent tier node from CST
/// 4. Parse node using existing CST traversal code
/// 5. Stream errors via ErrorSink (never fail-fast)
pub fn parse_dependent_tier(input: &str, errors: &impl ErrorSink) -> ParseOutcome<UserDefinedTier> {
    // Trim leading whitespace: CHAT requires '%' at column 0 for dependent tiers,
    // so leading spaces would prevent tree-sitter from recognizing the line.
    let trimmed = input.trim_start();

    // Synthesize minimal valid CHAT file with the tier
    let wrapped = format!(
        "@UTF8\n\
         @Begin\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n\
         *CHI:\tdummy .\n\
         {}\n\
         @End\n",
        trimmed
    );

    // Parse with tree-sitter to get proper CST structure
    let mut parser = tree_sitter::Parser::new();
    if let Err(e) = parser.set_language(&tree_sitter_talkbank::LANGUAGE.into()) {
        errors.report(ParseError::new(
            ErrorCode::ParseFailed,
            Severity::Error,
            SourceLocation::at_offset(0),
            ErrorContext::new(input, 0..input.len(), input),
            format!("Failed to set tree-sitter language: {}", e),
        ));
        return ParseOutcome::rejected();
    }

    let Some(tree) = parser.parse(&wrapped, None) else {
        errors.report(ParseError::new(
            ErrorCode::ParseFailed,
            Severity::Error,
            SourceLocation::at_offset(0),
            ErrorContext::new(input, 0..input.len(), input),
            "Tree-sitter failed to parse synthesized CHAT file",
        ));
        return ParseOutcome::rejected();
    };

    // Find the dependent tier node in the CST (the first tier after the main
    // tier), already classified into a typed `DependentTierChoice`.
    let root = tree.root_node();
    let Some(tier_choice) = find_first_dependent_tier(root) else {
        errors.report(ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::at_offset(0),
            ErrorContext::new(input, 0..input.len(), input),
            "No dependent tier found in parsed structure",
        ));
        return ParseOutcome::rejected();
    };

    // Extract tier type and content from the typed choice (no text hacking!)
    extract_tier_from_node(tier_choice, &wrapped, input, errors)
}

/// Find the first dependent tier in the CST and classify it into a typed
/// [`DependentTierChoice`].
///
/// Replaces the removed `is_dependent_tier(root.kind())` `node.kind()` check with
/// the generated supertype classifier [`extract_dependent_tier`], which maps a
/// node to `NodeSlot::Present(DependentTierChoice)` iff the node is a concrete
/// dependent tier (and `Unexpected` otherwise). Recurses in document order,
/// returning the first node the classifier accepts.
fn find_first_dependent_tier<'tree>(
    root: tree_sitter::Node<'tree>,
) -> Option<DependentTierChoice<'tree>> {
    if let NodeSlot::Present(choice) = extract_dependent_tier(root).content.slot {
        return Some(choice);
    }

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if let Some(found) = find_first_dependent_tier(child) {
            return Some(found);
        }
    }

    None
}

/// Extract a generic `UserDefinedTier { label, content }` from a typed
/// [`DependentTierChoice`], without fabricated defaults.
///
/// The `match child.kind()` classification the removed hand-walk used is gone:
/// the tier is already classified as a `DependentTierChoice`, and its children
/// are read POSITIONALLY (a structural read, not a `node.kind()` dispatch), which
/// the grammar guarantees for every dependent tier: `child_0` is the
/// `*_tier_prefix` (the label), `child_1` is the `tier_sep`, and the LAST child
/// is the `newline` (except `%wor`, whose grammar rule has no trailing newline).
/// The label and content bytes are BYTE-IDENTICAL to the removed walk:
///
/// - label = `child_0`'s text with the leading `%` stripped.
/// - content = the children between `tier_sep` and the trailing newline, joined
///   with a single space between non-empty parts. For a tier whose body is a
///   single node this is that node's text; for a recovered tier (e.g. a `%pho`
///   line with trailing whitespace that tree-sitter recovers as an `ERROR`
///   sibling of `pho_groups`) the body node and the `ERROR` node are BOTH content
///   children, joined, exactly as the removed walk did.
fn extract_tier_from_node(
    choice: DependentTierChoice,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UserDefinedTier> {
    let node = choice.raw_node();

    // `%wor` is the one dependent tier whose grammar rule has no trailing
    // `newline` child; for every other tier the last child is the newline, which
    // is a structural delimiter, not content.
    let has_trailing_newline = !matches!(choice, DependentTierChoice::WorDependentTier(_));

    let mut cursor = node.walk();
    let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

    // Label: child_0 is invariantly the tier's `*_tier_prefix`.
    let Some(prefix) = children.first() else {
        errors.report(ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(original_input, 0..original_input.len(), original_input),
            "Could not extract dependent tier label from parsed structure",
        ));
        return ParseOutcome::rejected();
    };
    let tier_type = match prefix.utf8_text(source.as_bytes()) {
        Ok(text) => match text.strip_prefix('%') {
            Some(label) if !label.is_empty() => label.to_string(),
            Some(_) => {
                errors.report(ParseError::new(
                    ErrorCode::InvalidDependentTier,
                    Severity::Error,
                    SourceLocation::from_offsets(prefix.start_byte(), prefix.end_byte()),
                    ErrorContext::new(original_input, 0..original_input.len(), original_input),
                    "Dependent tier label cannot be empty",
                ));
                return ParseOutcome::rejected();
            }
            None => {
                errors.report(ParseError::new(
                    ErrorCode::InvalidDependentTier,
                    Severity::Error,
                    SourceLocation::from_offsets(prefix.start_byte(), prefix.end_byte()),
                    ErrorContext::new(original_input, 0..original_input.len(), original_input),
                    format!(
                        "Dependent tier prefix '{}' is missing leading '%' marker",
                        text
                    ),
                ));
                return ParseOutcome::rejected();
            }
        },
        Err(_) => {
            errors.report(ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(prefix.start_byte(), prefix.end_byte()),
                ErrorContext::new(original_input, 0..original_input.len(), original_input),
                "Unparsable content: dependent tier prefix is not valid UTF-8",
            ));
            return ParseOutcome::rejected();
        }
    };

    // Content: the children strictly between the `tier_sep` (child_1) and the
    // trailing newline (dropped for non-`%wor`). Every such child is a content
    // node in the removed walk's sense (it is neither the prefix nor a structural
    // delimiter), joined with the same "single space between non-empty parts"
    // rule.
    let content_end = if has_trailing_newline {
        children.len().saturating_sub(1)
    } else {
        children.len()
    };
    // Content begins after child_0 (prefix) and child_1 (tier_sep); the `.min`
    // keeps the range valid for a degenerate tier with fewer children.
    const FIRST_CONTENT_CHILD: usize = 2;
    let content_start = FIRST_CONTENT_CHILD.min(content_end);
    let content_children = &children[content_start..content_end];

    let mut saw_content_node = false;
    let mut content = String::new();
    for child in content_children {
        let text = match child.utf8_text(source.as_bytes()) {
            Ok(text) => text,
            Err(_) => {
                errors.report(ParseError::new(
                    ErrorCode::UnparsableContent,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(original_input, 0..original_input.len(), original_input),
                    "Unparsable content: dependent tier content is not valid UTF-8",
                ));
                return ParseOutcome::rejected();
            }
        };
        saw_content_node = true;
        if !content.is_empty() && !text.is_empty() {
            content.push(' ');
        }
        content.push_str(text);
    }

    if !saw_content_node {
        errors.report(ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(original_input, 0..original_input.len(), original_input),
            "Could not extract dependent tier content from parsed structure",
        ));
        return ParseOutcome::rejected();
    }

    if content.is_empty() {
        errors.report(ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(original_input, 0..original_input.len(), original_input),
            "Dependent tier content cannot be empty",
        ));
        return ParseOutcome::rejected();
    }

    ParseOutcome::parsed(UserDefinedTier::new(tier_type, content))
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use talkbank_model::{ErrorCode, ErrorCollector};

    /// Verifies known dependent tiers parse with label/content preserved verbatim.
    #[test]
    fn parses_known_tier_label_and_content_without_fabrication() -> Result<(), String> {
        let input = "%mor:\tpro|I v|go .";
        let errors = ErrorCollector::new();
        let result = parse_dependent_tier(input, &errors);
        let tier = result.ok_or_else(|| {
            format!(
                "Expected %mor tier to parse, got errors: {:?}",
                errors.to_vec()
            )
        })?;
        assert_eq!(tier.label.as_str(), "mor");
        assert_eq!(tier.content, "pro|I v|go .");
        assert!(
            errors.is_empty(),
            "Unexpected diagnostics: {:?}",
            errors.into_vec()
        );
        Ok(())
    }

    /// Verifies `%x...` tiers parse without losing custom labels.
    #[test]
    fn parses_x_tier_label_and_content_without_fabrication() -> Result<(), String> {
        let input = "%xfoo:\tcustom value";
        let errors = ErrorCollector::new();
        let result = parse_dependent_tier(input, &errors);
        let tier = result.ok_or_else(|| {
            format!(
                "Expected %xfoo tier to parse, got errors: {:?}",
                errors.to_vec()
            )
        })?;
        assert_eq!(tier.label.as_str(), "xfoo");
        assert_eq!(tier.content, "custom value");
        assert!(
            errors.is_empty(),
            "Unexpected diagnostics: {:?}",
            errors.into_vec()
        );
        Ok(())
    }

    /// Verifies empty dependent-tier content is rejected without placeholder fabrication.
    #[test]
    fn empty_content_is_rejected_without_placeholder_values() {
        let input = "%com:\t";
        let errors = ErrorCollector::new();
        let result = parse_dependent_tier(input, &errors);
        assert!(
            result.is_none(),
            "Expected empty dependent tier content to fail"
        );
        assert!(
            errors
                .to_vec()
                .iter()
                .any(|err| err.code == ErrorCode::InvalidDependentTier),
            "Expected InvalidDependentTier diagnostic, got: {:?}",
            errors.to_vec()
        );
    }

    /// Verifies leading/trailing line whitespace does not prevent tier extraction.
    #[test]
    fn tier_with_leading_trailing_whitespace() -> Result<(), String> {
        let input = "  %pho:\ta b c  ";
        let errors = ErrorCollector::new();
        let result = parse_dependent_tier(input, &errors);
        let tier = result.ok_or_else(|| {
            format!(
                "Expected whitespace-padded tier to parse, got errors: {:?}",
                errors.to_vec()
            )
        })?;
        assert_eq!(tier.label.as_str(), "pho");
        // 3 trailing spaces: "a b c" from pho_groups + join separator + "  " from ERROR node
        // (trailing spaces in pho content are a tree-sitter ERROR, collected by synthesis pattern)
        assert_eq!(tier.content, "a b c   ");
        assert!(
            errors.is_empty(),
            "Unexpected diagnostics: {:?}",
            errors.into_vec()
        );
        Ok(())
    }
}
