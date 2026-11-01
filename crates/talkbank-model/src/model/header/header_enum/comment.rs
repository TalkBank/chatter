//! The payload of an `@Comment` header: bullet-aware text proven to write
//! something after the separator.
//!
//! `@Comment` is the one header whose content has bullets and pictures, so
//! its payload is a [`BulletContent`] rather than a string. A bare
//! `BulletContent` can hold nothing at all (no segments, or only blank text
//! and continuations), and written after `@Comment:` and its tab that is the
//! empty header (E553) wearing a payload. [`CommentContent`] is the
//! `BulletContent` that cannot be blank: every route in (the parsers'
//! lowering, [`CommentContent::try_from_bullets`], JSON) refuses a payload
//! whose CHAT form carries no non-whitespace character, with the same
//! [`BlankContent`] every other header payload refuses blank text with.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>

use super::empty::{BlankContent, HeaderContent, HeaderText};
use crate::model::{BulletContent, ChatWriter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// The content of an `@Comment` header: text with media bullets and
/// pictures, proven to write at least one non-whitespace character.
///
/// Built from content the one emptiness decision admitted
/// ([`Self::from_content`]), or from bullet content that writes something
/// ([`Self::try_from_bullets`]); there is no unchecked route, and the JSON
/// form (the same object as [`BulletContent`]) is admitted through
/// `try_from_bullets`.
///
/// ```compile_fail
/// # use talkbank_model::model::{BulletContent, CommentContent};
/// let forged = CommentContent(BulletContent::empty());
/// ```
#[derive(Clone, Debug, PartialEq, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "BulletContent")]
pub struct CommentContent(BulletContent);

impl CommentContent {
    /// A plain-text comment from content the one emptiness decision
    /// admitted, kept exactly as written.
    pub fn from_content(text: HeaderText<'_>) -> Self {
        Self(BulletContent::from_text(text.as_str()))
    }

    /// Admits plain text as a comment, refusing blank text.
    ///
    /// # Errors
    ///
    /// [`BlankContent`] when `text` is empty or only whitespace.
    pub fn try_from_text(text: impl AsRef<str>) -> Result<Self, BlankContent> {
        HeaderContent::admit(text.as_ref()).map(Self::from_content)
    }

    /// Admits bullet content as a comment when its CHAT form carries a
    /// non-whitespace character: a text run with one, a bullet or a picture.
    /// Continuations and blank text alone write nothing a reader can see,
    /// and would reparse as the empty header.
    ///
    /// The test is [`BulletContent::is_empty`], the writer run against a
    /// probe that stops at the first fragment [`HeaderContent::classify`]
    /// calls text, so "what this writes" and "whether it is blank" cannot
    /// drift apart, and the `%com` tier's emptiness (E756) is the same
    /// judgement.
    ///
    /// # Errors
    ///
    /// [`BlankContent`] when the content writes only whitespace, or nothing.
    pub fn try_from_bullets(content: BulletContent) -> Result<Self, BlankContent> {
        if content.is_empty() {
            Err(BlankContent)
        } else {
            Ok(Self(content))
        }
    }

    /// The bullet content.
    pub fn as_bullet_content(&self) -> &BulletContent {
        &self.0
    }

    /// Writes the content in CHAT form.
    pub fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        self.0.write_chat(w)
    }
}

impl Serialize for CommentContent {
    /// Writes the bullet content's own object, borrowed: no copy per comment.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl TryFrom<BulletContent> for CommentContent {
    type Error = BlankContent;

    /// Admits bullet content through [`CommentContent::try_from_bullets`].
    fn try_from(content: BulletContent) -> Result<Self, Self::Error> {
        Self::try_from_bullets(content)
    }
}

impl From<CommentContent> for BulletContent {
    /// Releases the bullet content; consuming the proof cannot break it.
    fn from(comment: CommentContent) -> Self {
        comment.0
    }
}

impl JsonSchema for CommentContent {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "CommentContent".into()
    }

    /// The bullet content's own object; the nonblank rule is stated in the
    /// description, since a schema cannot express "some segment writes a
    /// non-whitespace character".
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let content = generator.subschema_for::<BulletContent>();
        schemars::json_schema!({
            "description": "@Comment content: bullet content whose CHAT form carries at least one non-whitespace character (a text run with one, a bullet or a picture). Blank content is refused.",
            "allOf": [content]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BulletContentSegment, Header};

    /// Content admitted exactly when what it writes classifies as text.
    #[test]
    fn comment_content_admits_exactly_what_writes_text() {
        let admitted = [
            BulletContent::from_text("x"),
            BulletContent::new(vec![BulletContentSegment::bullet(0, 10)]),
            BulletContent::new(vec![BulletContentSegment::picture("a.jpg")]),
            BulletContent::new(vec![
                BulletContentSegment::continuation(),
                BulletContentSegment::text("late"),
            ]),
        ];
        let refused = [
            BulletContent::empty(),
            BulletContent::from_text(""),
            BulletContent::from_text(" \t"),
            BulletContent::new(vec![BulletContentSegment::continuation()]),
            BulletContent::new(vec![
                BulletContentSegment::text("   "),
                BulletContentSegment::continuation(),
            ]),
        ];
        for content in admitted {
            let written = content.to_chat_string();
            assert!(
                matches!(HeaderContent::classify(&written), HeaderContent::Text(_)),
                "{written:?}"
            );
            assert!(
                CommentContent::try_from_bullets(content).is_ok(),
                "{written:?}"
            );
        }
        for content in refused {
            let written = content.to_chat_string();
            assert_eq!(
                HeaderContent::classify(&written),
                HeaderContent::Blank,
                "{written:?}"
            );
            assert_eq!(
                CommentContent::try_from_bullets(content),
                Err(BlankContent),
                "{written:?}"
            );
        }
        assert_eq!(CommentContent::try_from_text(" "), Err(BlankContent));
    }

    /// JSON refuses a blank comment, which would write `@Comment:` and a tab
    /// and reparse as the empty header; a comment with content roundtrips.
    #[test]
    fn comment_json_refuses_blank_content_and_roundtrips() {
        for wire in [
            r#"{"type":"comment","content":{"segments":[]}}"#,
            r#"{"type":"comment","content":{"segments":[{"type":"text","text":"  "}]}}"#,
            r#"{"type":"comment","content":{"segments":[{"type":"continuation"}]}}"#,
        ] {
            assert!(serde_json::from_str::<Header>(wire).is_err(), "{wire}");
        }
        let header = Header::Comment {
            content: CommentContent::try_from_text("see 0_10").expect("content"),
        };
        let wire = serde_json::to_string(&header).expect("serialize");
        assert_eq!(
            wire,
            r#"{"type":"comment","content":{"segments":[{"type":"text","text":"see 0_10"}]}}"#
        );
        assert_eq!(
            serde_json::from_str::<Header>(&wire).expect("admit"),
            header
        );
        assert_eq!(header.to_chat(), "@Comment:\tsee 0_10");
    }
}
