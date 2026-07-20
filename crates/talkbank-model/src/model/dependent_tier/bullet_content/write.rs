//! CHAT serialization for bullet-capable dependent-tier text content.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>
//!
//! Serializers here are the single source of truth for inline bullets, so any
//! tier that needs to emit `\u0015…\u0015` markers (including `%act`/`%cod`)
//! should route through this module.

use super::{BulletContent, BulletContentSegment};

impl BulletContent {
    /// Serializes bullet-capable dependent-tier text in CHAT form.
    ///
    /// Spaces are canonical delimiters (the parser stores none): consecutive
    /// content items (text runs, bullets, pictures) are joined by exactly one
    /// space, mirroring the main tier. A continuation (`\n\t`) is a line-wrap
    /// delimiter, so it is emitted with no surrounding space and resets the
    /// join. This is why leading/trailing/multi spaces from the source do not
    /// survive roundtrip: they were never content. Control delimiters
    /// (`U+0015`) mark bullets and pictures.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        // Whether a space is owed before the next content item (true once one
        // content item has been written and no continuation has intervened).
        let mut need_space = false;
        for segment in &self.segments {
            match segment {
                BulletContentSegment::Text(text) => {
                    if need_space {
                        w.write_char(' ')?;
                    }
                    w.write_str(text.text.as_str())?;
                    need_space = true;
                }
                BulletContentSegment::Bullet(bullet) => {
                    if need_space {
                        w.write_char(' ')?;
                    }
                    w.write_char('\u{0015}')?;
                    write!(w, "{}_{}", bullet.start_ms, bullet.end_ms)?;
                    w.write_char('\u{0015}')?;
                    need_space = true;
                }
                BulletContentSegment::Picture(picture) => {
                    if need_space {
                        w.write_char(' ')?;
                    }
                    w.write_char('\u{0015}')?;
                    write!(w, "%pic:\"{}\"", picture.filename)?;
                    w.write_char('\u{0015}')?;
                    need_space = true;
                }
                BulletContentSegment::Continuation => {
                    w.write_str("\n\t")?;
                    need_space = false;
                }
            }
        }
        Ok(())
    }

    /// Allocating convenience wrapper over [`Self::write_chat`].
    ///
    /// Prefer [`Self::write_chat`] when writing into existing buffers to avoid
    /// transient allocation in hot paths.
    pub fn to_chat_string(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }
}
