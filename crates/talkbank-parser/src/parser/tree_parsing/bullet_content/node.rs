//! The two grammar carriers that own bullet-capable free text.

use crate::generated_traversal::{SourceBound, TextWithBulletsAndPicsNode, TextWithBulletsNode};

/// Retains whether the grammar permits pictures, without a separate flag.
#[derive(Debug, Clone, Copy)]
pub enum BulletTextNode<'tree, 'source> {
    Text(SourceBound<'tree, 'source, TextWithBulletsNode<'tree>>),
    Pictures(SourceBound<'tree, 'source, TextWithBulletsAndPicsNode<'tree>>),
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, TextWithBulletsNode<'tree>>>
    for BulletTextNode<'tree, 'source>
{
    fn from(node: SourceBound<'tree, 'source, TextWithBulletsNode<'tree>>) -> Self {
        Self::Text(node)
    }
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, TextWithBulletsAndPicsNode<'tree>>>
    for BulletTextNode<'tree, 'source>
{
    fn from(node: SourceBound<'tree, 'source, TextWithBulletsAndPicsNode<'tree>>) -> Self {
        Self::Pictures(node)
    }
}
