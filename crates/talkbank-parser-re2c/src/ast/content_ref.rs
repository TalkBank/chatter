//! Borrowed traversal across unrestricted content and admitted marker chains.

use super::{Chain, ContentItem};

/// An item in a native content walk, without copying its recursive payload.
#[derive(Clone, Copy)]
pub enum ContentRef<'item, 'source> {
    /// An ordinary tier or enclosed item.
    Item(&'item ContentItem<'source>),
    /// The single admitted item wrapped by a retrace.
    Chain(&'item Chain<'source>),
}

impl<'item, 'source> ContentRef<'item, 'source> {
    /// Immediate children, including the item inside a single retrace.
    pub fn children(self) -> ContentChildren<'item, 'source> {
        match self {
            Self::Item(item) => item.children(),
            Self::Chain(chain) => match chain {
                Chain::Group(_, items, _) => ContentChildren::items(items),
                Chain::Quotation(quotation) => ContentChildren::items(&quotation.contents),
                Chain::Wrapped(retrace) => retrace.body.items(),
                Chain::Word(_) | Chain::Event(..) | Chain::Action(..) => {
                    ContentChildren::items(&[])
                }
            },
        }
    }
}

/// Allocation-free children of either native container shape.
pub enum ContentChildren<'item, 'source> {
    /// A tier or enclosed sequence.
    Items(std::slice::Iter<'item, ContentItem<'source>>),
    /// The one admitted marker-chain item.
    Single(std::option::IntoIter<&'item Chain<'source>>),
}

impl<'item, 'source> ContentChildren<'item, 'source> {
    pub(super) fn items(items: &'item [ContentItem<'source>]) -> Self {
        Self::Items(items.iter())
    }
}

impl<'item, 'source> Iterator for ContentChildren<'item, 'source> {
    type Item = ContentRef<'item, 'source>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Items(items) => items.next().map(ContentRef::Item),
            Self::Single(item) => item.next().map(ContentRef::Chain),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Items(items) => items.size_hint(),
            Self::Single(item) => item.size_hint(),
        }
    }
}

impl ExactSizeIterator for ContentChildren<'_, '_> {}
