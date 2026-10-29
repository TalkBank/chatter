//! Admission from the existing word JSON shape into the structured model.

use super::{
    ConflictingWordSuffixes, FormType, Word, WordCategory, WordContents, WordLanguageMarker,
    WordPosTag, WordSuffix,
};
use serde::Deserialize;

/// External fields are admitted together before a word can be constructed.
/// Computed spelling and legacy locations are never source evidence.
#[derive(Deserialize)]
pub(super) struct WordWire {
    word_id: Option<smol_str::SmolStr>,
    content: WordContents,
    category: Option<WordCategory>,
    form_type: Option<FormType>,
    lang: Option<WordLanguageMarker>,
    part_of_speech: Option<WordPosTag>,
    inline_bullet: Option<crate::Bullet>,
}

impl TryFrom<WordWire> for Word {
    type Error = ConflictingWordSuffixes;

    fn try_from(wire: WordWire) -> Result<Self, Self::Error> {
        let suffix = WordSuffix::from_markers(wire.form_type, wire.lang)?;
        let mut word = Self::from_content(crate::Location::Unlocated, wire.content);
        word.word_id = wire.word_id;
        word.category = wire.category;
        word.suffix = suffix;
        word.part_of_speech = wire.part_of_speech;
        word.inline_bullet = wire.inline_bullet;
        Ok(word)
    }
}
