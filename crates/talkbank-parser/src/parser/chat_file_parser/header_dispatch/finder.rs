//! Locate the Nth header node in a parsed document tree.
//!
//! Used by single-line header APIs that parse inside a synthetic wrapper.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Begin_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#End_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#UTF8_Header>

use crate::generated_traversal::{
    AsRawNode, BeginHeaderNode, EndHeaderNode, FromNodeKind, FullDocumentChild1Choice,
    HeaderChoice, LineNode, SourceBindingError, SourceSlice, Utf8HeaderNode,
};

/// A canonical node admitted by the generated header vocabulary and lookup
/// policy. Private construction prevents a non-header source slice from being
/// passed to fragment admission as though lookup had selected it.
pub(super) struct SelectedHeader<'tree, 'source>(SourceSlice<'tree, 'source>);

impl<'tree, 'source> SelectedHeader<'tree, 'source> {
    fn admit(source: SourceSlice<'tree, 'source>) -> Option<Self> {
        let node = source.raw_node();
        match HeaderChoice::from_node(node) {
            // Deliberate fragment-lookup exclusion, not an omitted kind.
            Some(HeaderChoice::ThumbnailHeader(_)) => None,
            Some(_) => Some(Self(source)),
            None => (FullDocumentChild1Choice::from_node(node).is_some()
                || source.typed::<Utf8HeaderNode<'_>>().is_some()
                || source.typed::<BeginHeaderNode<'_>>().is_some()
                || source.typed::<EndHeaderNode<'_>>().is_some())
            .then_some(Self(source)),
        }
    }

    pub(super) fn source(&self) -> SourceSlice<'tree, 'source> {
        self.0
    }
}

/// Take the first canonical child without dropping a range refusal.
pub(super) fn first_child<'tree, 'source>(
    parent: SourceSlice<'tree, 'source>,
) -> Result<Option<SourceSlice<'tree, 'source>>, SourceBindingError> {
    let mut cursor = parent.raw_node().walk();
    parent.children(&mut cursor).next().transpose()
}

/// Select the Nth header while retaining its producer-bound source.
/// Counting policy, including the intentional Thumbnail exclusion, is unchanged.
pub(super) fn find_header_node_in_tree<'tree, 'source>(
    root: SourceSlice<'tree, 'source>,
    index: usize,
) -> Result<SelectedHeader<'tree, 'source>, HeaderLookupError> {
    let mut found_count = 0;
    let mut consider = |source| {
        let header = SelectedHeader::admit(source)?;
        let ordinal = found_count;
        found_count += 1;
        (ordinal == index).then_some(header)
    };
    let mut cursor = root.raw_node().walk();
    for child in root.children(&mut cursor) {
        let child = child?;
        // `header` and `pre_begin_header` are invisible supertypes in the
        // producer. Their concrete alternatives appear here, or beneath the
        // real `line` wrapper; there is no abstract header wrapper to unwrap.
        if child.typed::<LineNode<'_>>().is_some() {
            let mut cursor = child.raw_node().walk();
            for grandchild in child.children(&mut cursor) {
                let grandchild = grandchild?;
                if let Some(header) = consider(grandchild) {
                    return Ok(header);
                }
            }
        } else if let Some(header) = consider(child) {
            return Ok(header);
        }
    }
    Err(HeaderLookupError::NotFound { index, found_count })
}

/// Lookup absence and source-binding failure remain distinct boundary states.
#[derive(Debug, thiserror::Error)]
pub(super) enum HeaderLookupError {
    #[error(
        "Tier validation error: header at index {index} not found in CST (only {found_count} headers present)"
    )]
    NotFound { index: usize, found_count: usize },
    #[error(transparent)]
    Binding(#[from] SourceBindingError),
}
