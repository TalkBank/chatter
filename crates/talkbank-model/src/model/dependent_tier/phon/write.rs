//! Phon tier rendering without fallible `Display` callbacks.

use super::{
    AlignmentPair, PhoalnTier, PhoneInterval, SylTier, WordAlignment, XPHOINT_GROUP_SEP,
    XphointGroup, XphointTier,
};
use crate::model::{ChatWriter, WriteChat};

fn separated<T: WriteChat, W: ChatWriter>(
    items: &[T],
    separator: &str,
    w: &mut W,
) -> Result<(), W::Error> {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            w.write_str(separator)?;
        }
        item.write_chat(w)?;
    }
    Ok(())
}

impl SylTier {
    fn write_content<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        separated(&self.words, " ", w)
    }
}

impl WriteChat for SylTier {
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self.prefix())?;
        w.write_str(":\t")?;
        self.write_content(w)
    }
}

impl WriteChat for AlignmentPair {
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        match &self.source {
            Some(source) => w.write_str(source.as_str())?,
            None => w.write_str("∅")?,
        }
        w.write_char('↔')?;
        match &self.target {
            Some(target) => w.write_str(target.as_str()),
            None => w.write_str("∅"),
        }
    }
}

impl WriteChat for WordAlignment {
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        separated(&self.pairs, ",", w)
    }
}

impl PhoalnTier {
    fn write_content<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        separated(&self.words, " ", w)
    }
}

impl WriteChat for PhoalnTier {
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str("%xphoaln:\t")?;
        self.write_content(w)
    }
}

impl WriteChat for PhoneInterval {
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self.phone.as_str())?;
        w.write_char(' ')?;
        self.bullet.write_chat(w)
    }
}

impl WriteChat for XphointGroup {
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        separated(&self.phones, " ", w)
    }
}

impl XphointTier {
    fn write_content<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        separated(&self.groups, XPHOINT_GROUP_SEP, w)
    }
}

impl WriteChat for XphointTier {
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str("%xphoint:\t")?;
        self.write_content(w)
    }
}

// Tier Display is content-only; WriteChat includes the tier prefix.
macro_rules! content_display {
    ($($ty:ty),+ $(,)?) => {$(
        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.write_content(f)
            }
        }
    )+};
}
content_display!(SylTier, PhoalnTier, XphointTier);

macro_rules! chat_display {
    ($($ty:ty),+ $(,)?) => {$(
        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.write_chat(f)
            }
        }
    )+};
}
chat_display!(AlignmentPair, WordAlignment, PhoneInterval, XphointGroup);
