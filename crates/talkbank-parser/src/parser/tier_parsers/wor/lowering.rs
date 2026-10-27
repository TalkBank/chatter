//! Source-order association: an untimed word can receive exactly one bullet.
use talkbank_model::model::dependent_tier::WorItem;
use talkbank_model::{Bullet, Word};

/// Completed items are never searched or mutated. The pending word is consumed
/// before timing is attached, so a later bullet cannot replace earlier timing.
pub(super) struct WorLowering {
    items: Vec<WorItem>,
    pending: Option<Box<Word>>,
}

/// The owner retains raw source instead of a partial tier. Diagnostics precede
/// rejection; this marker does not describe a producer or grammar failure.
pub(super) struct WorItemRejected;

impl WorLowering {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            pending: None,
        }
    }
    pub(super) fn push(mut self, item: WorItem) -> Self {
        self.flush();
        match item {
            WorItem::Word(word) => {
                if word.inline_bullet.is_none() {
                    self.pending = Some(word);
                } else {
                    self.items.push(WorItem::Word(word));
                }
            }
            item @ WorItem::Separator { .. } => self.items.push(item),
        }
        self
    }
    pub(super) fn attach(mut self, bullet: Bullet) -> Result<Self, WorItemRejected> {
        let word = self.pending.take().ok_or(WorItemRejected)?;
        self.items
            .push(WorItem::Word(Box::new(word.with_inline_bullet(bullet))));
        Ok(self)
    }
    fn flush(&mut self) {
        if let Some(word) = self.pending.take() {
            self.items.push(WorItem::Word(word));
        }
    }
    pub(super) fn finish(mut self) -> Vec<WorItem> {
        self.flush();
        self.items
    }
}
