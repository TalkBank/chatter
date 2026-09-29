//! Shared role assignment for generated `participant_word` sequences.

/// A participant declaration needs a final role token after its speaker code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingParticipantRole;

/// The grammar's ordered participant words partitioned into name and role.
///
/// This is structural assignment only, not validation of role spelling. Both
/// AST lowering and source-bound transforms use it, so the last-word rule has
/// one owner. The speaker code is a separate grammar field and never enters it.
pub struct ParticipantWordRoles<T> {
    names: Vec<T>,
    role: T,
}

impl<T> ParticipantWordRoles<T> {
    /// Assign the final generated word to the role and preceding words to names.
    pub fn from_words(mut words: Vec<T>) -> Result<Self, MissingParticipantRole> {
        let role = words.pop().ok_or(MissingParticipantRole)?;
        Ok(Self { names: words, role })
    }

    /// Name tokens only; may be empty for an unnamed participant.
    pub fn names(&self) -> &[T] {
        &self.names
    }

    /// The required role token, excluded from name processing.
    pub fn role(&self) -> &T {
        &self.role
    }
}
