//! One owner for a word's optional `@` suffix.

use super::{FormType, WordLanguageMarker};
use crate::model::{ChatWriter, WriteChat};
use talkbank_derive::{SemanticEq, SpanShift};

/// Two independently supplied `@` markers cannot occupy one word suffix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("a word may carry only one '@' suffix; form and language markers conflict")]
pub struct ConflictingWordSuffixes;

/// A word carries a form marker or a language marker in its single `@` slot.
///
/// Rejected marker text remains explicit in [`FormType::Undeclared`]. This
/// enum prevents stacking independently valid form and language markers; it
/// does not promote a recovery value to valid CHAT.
///
/// ```
/// use talkbank_model::{FormType, Location, Word, WordSuffix, WordText};
/// let word = Word::new(Location::Unlocated, WordText::new("word")?)
///     .with_suffix(WordSuffix::Form(FormType::B));
/// assert_eq!(word.to_chat(), "word@b");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Independent mutable marker slots no longer exist:
///
/// ```compile_fail
/// use talkbank_model::{FormType, Location, Word, WordText};
/// let mut word = Word::new(Location::Unlocated, WordText::new("word").unwrap());
/// word.form_type = Some(FormType::B);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, SemanticEq, SpanShift)]
pub enum WordSuffix {
    /// A declared form marker or explicit undeclared-marker recovery.
    Form(FormType),
    /// A word-level language marker.
    Language(WordLanguageMarker),
}

impl WordSuffix {
    /// Admit separate external fields into the single suffix slot.
    ///
    /// JSON and parser producers share this transition. Two present fields
    /// reject; neither marker is silently discarded or given precedence.
    pub fn from_markers(
        form: Option<FormType>,
        language: Option<WordLanguageMarker>,
    ) -> Result<Option<Self>, ConflictingWordSuffixes> {
        match (form, language) {
            (None, None) => Ok(None),
            (Some(form), None) => Ok(Some(Self::Form(form))),
            (None, Some(language)) => Ok(Some(Self::Language(language))),
            (Some(_), Some(_)) => Err(ConflictingWordSuffixes),
        }
    }

    /// Borrow the form marker, if this suffix names a form.
    pub fn form_type(&self) -> Option<&FormType> {
        match self {
            Self::Form(form) => Some(form),
            Self::Language(_) => None,
        }
    }

    /// Borrow the language marker, if this suffix names a language.
    pub fn language_marker(&self) -> Option<&WordLanguageMarker> {
        match self {
            Self::Form(_) => None,
            Self::Language(language) => Some(language),
        }
    }

    /// Mutate an existing language payload without creating another suffix.
    pub fn language_marker_mut(&mut self) -> Option<&mut WordLanguageMarker> {
        match self {
            Self::Form(_) => None,
            Self::Language(language) => Some(language),
        }
    }
}

impl WriteChat for WordSuffix {
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        match self {
            Self::Form(form) => {
                w.write_char('@')?;
                form.write_chat(w)
            }
            Self::Language(marker) => {
                w.write_str("@s")?;
                match marker {
                    WordLanguageMarker::Shortcut => Ok(()),
                    WordLanguageMarker::Explicit(code) => {
                        w.write_char(':')?;
                        code.write_chat(w)
                    }
                    WordLanguageMarker::Multiple(codes) => write_codes(w, codes, '+'),
                    WordLanguageMarker::Ambiguous(codes) => write_codes(w, codes, '&'),
                }
            }
        }
    }
}

fn write_codes<W: ChatWriter>(
    w: &mut W,
    codes: &[crate::LanguageCode],
    separator: char,
) -> Result<(), W::Error> {
    w.write_char(':')?;
    for (index, code) in codes.iter().enumerate() {
        if index > 0 {
            w.write_char(separator)?;
        }
        code.write_chat(w)?;
    }
    Ok(())
}
