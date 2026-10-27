//! Admission shared by transforms that rewrite participant identities.

use talkbank_model::model::ParticipantDeclarationError;
use talkbank_model::{ChatFile, Header, ParticipantEntries};

/// Why participant identity headers cannot be safely rewritten.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SpeakerRewriteError {
    /// There is no single readable participant declaration.
    #[error(transparent)]
    Declaration(#[from] ParticipantDeclarationError),
    /// A recognized ID contains unreadable identity or demographic fields.
    #[error("cannot rewrite speakers with an unparsed @ID header; repair its syntax first")]
    UnparsedID,
}

/// A borrowed input with one declaration and no recognized unreadable IDs.
/// This is not CHAT validity or proof that every speaker reference is supported.
pub(crate) struct SpeakerRewriteInput<'a> {
    file: &'a ChatFile,
    participants: &'a ParticipantEntries,
}

impl<'a> SpeakerRewriteInput<'a> {
    pub(crate) fn admit(file: &'a ChatFile) -> Result<Self, SpeakerRewriteError> {
        let participants = file.participant_declaration()?;
        if file
            .headers()
            .any(|header| matches!(header, Header::UnparsedID(_)))
        {
            return Err(SpeakerRewriteError::UnparsedID);
        }
        Ok(Self { file, participants })
    }

    pub(crate) fn participants(&self) -> &'a ParticipantEntries {
        self.participants
    }

    pub(crate) fn file(&self) -> &'a ChatFile {
        self.file
    }
}
