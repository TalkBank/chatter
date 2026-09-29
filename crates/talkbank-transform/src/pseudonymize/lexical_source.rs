//! Exact lexical source fields, never a raw-text tokenization fallback.

use talkbank_model::{Word, WordContent};
use talkbank_parser::generated_traversal::{
    AsRawNode, FromNodeKind, ParsedSource, ShorteningNode, SourceSlice, SourceSlotView,
    StandaloneWordNode, WordSegmentNode,
};

/// Sensitive proposed edit to one generated lexical field.
/// A collection of these is not permission to write document output.
pub struct LexicalEdit<'input> {
    source: SourceSlice<'input, 'input>,
    replacement: String,
}

impl LexicalEdit<'_> {
    /// Exact original spelling, including parentheses for a shortening.
    pub fn original(&self) -> &str {
        self.source.text()
    }

    /// Byte range within the producing document, not an independently supplied string.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.source.raw_node().byte_range()
    }

    /// Proposed text; empty means removal of a later lexical piece.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

impl<'input> LexicalEdit<'input> {
    pub(super) fn new(source: SourceSlice<'input, 'input>, replacement: String) -> Self {
        Self {
            source,
            replacement,
        }
    }
}

/// Same-parse association or lexical-piece correspondence could not be proven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LexicalSourceRefusal;

/// Bind only orthographic lexical pieces, indexed in the model's content list.
/// Marker and suffix bytes never become edit candidates. The shortening node
/// owns its entire subtree: its inner segment must not appear as a second edit.
pub(super) fn fields<'input>(
    parsed: &'input ParsedSource<'_>,
    word: &Word,
) -> Result<Vec<(usize, SourceSlice<'input, 'input>)>, LexicalSourceRefusal> {
    let refused = LexicalSourceRefusal;
    let range = word.span.start as usize..word.span.end as usize;
    let mut raw = parsed
        .root_node()
        .named_descendant_for_byte_range(range.start, range.end)
        .ok_or(refused)?;
    // A plain word's segment/body/standalone wrappers can share one range.
    // Ascend only within that exact range to the generated owning word kind.
    loop {
        if raw.byte_range() != range {
            return Err(refused);
        }
        if StandaloneWordNode::from_node(raw).is_some() {
            break;
        }
        raw = raw.parent().ok_or(refused)?;
    }
    let bound = parsed.bind(raw).map_err(|_| refused)?;
    let word_node = bound.typed::<StandaloneWordNode>().ok_or(refused)?;
    if word_node.text() != word.raw_text() {
        return Err(refused);
    }
    let word_node = word_node.extract().map_err(|_| refused)?;
    let body = match word_node.field_child_1().slot().view() {
        SourceSlotView::Present(field) => field.read().map_err(|_| refused)?,
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(_) => {
            return Err(refused);
        }
    };
    let mut model = word
        .content()
        .iter()
        .enumerate()
        .filter(|(_, part)| matches!(part, WordContent::Text(_) | WordContent::Shortening(_)));
    let mut result = Vec::new();
    let mut covered_until = body.raw_node().start_byte();
    for descendant in body.descendants() {
        let source = descendant.map_err(|_| refused)?;
        if source.raw_node().start_byte() < covered_until {
            continue;
        }
        if let Some(shortening) = source.typed::<ShorteningNode>() {
            let shortening = shortening.extract().map_err(|_| refused)?;
            let inner = match shortening.field_child_1().slot().view() {
                SourceSlotView::Present(field) => field.read().map_err(|_| refused)?,
                SourceSlotView::Missing(_)
                | SourceSlotView::Error(_)
                | SourceSlotView::Absent(_) => {
                    return Err(refused);
                }
            };
            let Some((index, WordContent::Shortening(text))) = model.next() else {
                return Err(refused);
            };
            if inner.text() != text.as_ref() {
                return Err(refused);
            }
            covered_until = source.raw_node().end_byte();
            result.push((index, source));
        } else if let Some(segment) = source.typed::<WordSegmentNode>() {
            let Some((index, WordContent::Text(text))) = model.next() else {
                return Err(refused);
            };
            if segment.text() != text.as_ref() {
                return Err(refused);
            }
            covered_until = source.raw_node().end_byte();
            result.push((index, source));
        }
    }
    if model.next().is_some() {
        return Err(refused);
    }
    Ok(result)
}
