//! Tests for this subsystem.
//!

use super::{BulletTextNode, parse_bullet_content};
use crate::TreeSitterParser;
use crate::error::{ErrorCollector, ParseError};
use crate::generated_traversal::{ActDependentTierNode, SourceSlotView};
use std::fs;
use std::path::PathBuf;
use talkbank_model::model::{BulletContent, BulletContentSegment};

/// Parse a real test file and extract the %act tier content
fn parse_test_file(filename: &str) -> Result<(BulletContent, Vec<ParseError>), String> {
    let parser = TreeSitterParser::new().map_err(|err| err.to_string())?;

    // Read test file from local reference corpus.
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates
    path.pop(); // repo root
    path.push("corpus/reference");
    path.push(filename);

    let source = fs::read_to_string(&path)
        .map_err(|err| format!("Could not read test file {:?}: {err}", path))?;

    let parsed = parser
        .parse_source_incremental(&source, None)
        .map_err(|err| err.to_string())?;
    let root = parsed
        .bind(parsed.root_node())
        .map_err(|err| err.to_string())?;
    let mut content_node = None;
    for descendant in root.descendants() {
        let node = descendant.map_err(|err| err.to_string())?;
        if let Some(tier) = node.typed::<ActDependentTierNode>() {
            let children = tier.extract();
            if let Some(body) = children.field_child_2().slot().optional()
                && let SourceSlotView::Present(text) = body.view()
            {
                content_node = Some(BulletTextNode::from(
                    text.read().map_err(|err| err.to_string())?,
                ));
                break;
            }
        }
    }
    let content_node =
        content_node.ok_or_else(|| "Should find a source-bound act content slot".to_string())?;

    let error_sink = ErrorCollector::new();
    let content = parse_bullet_content(content_node, &error_sink);
    Ok((content, error_sink.into_vec()))
}

/// Tests real file with bullets.
#[test]
fn test_real_file_with_bullets() -> Result<(), String> {
    let (content, errors) = parse_test_file("content/media-bullets.cha")?;
    assert!(errors.is_empty(), "Should have no errors: {:?}", errors);

    // From media-bullets.cha line 14: %act:\tfoo 2061689_2062652 bar 2061689_2062652
    // Expected: text("foo "), bullet(2061689, 2062652), text(" bar "), bullet(2061689, 2062652)
    assert!(
        content.segments.len() >= 2,
        "Should have at least 2 segments"
    );

    // Check that we have bullet segments
    let has_bullet = content
        .segments
        .iter()
        .any(|seg| matches!(seg, BulletContentSegment::Bullet(_)));
    assert!(has_bullet, "Should have at least one bullet segment");
    Ok(())
}

/// Tests plain text only.
#[test]
fn test_plain_text_only() {
    // Test with a simple construction for plain text (no real file needed)
    let segments = vec![BulletContentSegment::text("hello world")];
    let content = BulletContent::new(segments);
    assert_eq!(content.segments.len(), 1);
    assert!(!content.is_empty());
}
