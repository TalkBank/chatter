//! Complete CHAT transcript representation.
//!
//! `ChatFile` preserves the original interleaving of header lines and utterances.
//! This gives deterministic roundtrip behavior and keeps positional context for
//! diagnostics that depend on file order.
//!
//! # CHAT Format Structure
//!
//! Headers and utterances are interleaved in real corpora:
//!
//! ```text
//! @UTF8
//! @Begin
//! @Languages: eng
//! @Participants: CHI Target_Child, MOT Mother
//! *CHI: hello .
//! @Comment: This comment appears between utterances
//! *MOT: hi there !
//! @Comment: Another interleaved comment
//! @End
//! ```
//!
//! # CHAT Format Reference
//!
//! - [CHAT Manual](https://talkbank.org/0info/manuals/CHAT.html)
//! - [File Headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
//! - [Main Lines](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)

mod accessors;
mod core;
pub mod transcript_name;
mod validate;
mod write;

pub use core::{ChatFile, ChatFileLines};

/// Derived presence of main-tier or word-tier timing, not validity or permission
/// to write. Recorded intervals may independently fail timing validation.
#[derive(Debug, Clone, Copy)]
pub enum TranscriptTimingEvidence<'file> {
    /// No timing bullet occurs on either supported surface.
    Absent,
    /// A bullet observed in this borrowed transcript.
    Recorded(RecordedTranscriptTiming<'file>),
}

/// An observation tied to its immutable document. Only the model's shared
/// timing observer constructs it; callers cannot attach a foreign bullet.
///
/// ```compile_fail,E0451
/// # use talkbank_model::model::{Bullet, ChatFile, RecordedTranscriptTiming};
/// # let file = ChatFile::new(vec![]);
/// # let unrelated = Bullet::new(0, 100);
/// let forged = RecordedTranscriptTiming { document: &file, bullet: &unrelated };
/// ```
#[derive(Clone, Copy)]
pub struct RecordedTranscriptTiming<'file> {
    document: &'file ChatFile,
    bullet: &'file crate::model::Bullet,
}

impl std::fmt::Debug for RecordedTranscriptTiming<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Observing one bullet must not dump the entire borrowed transcript.
        formatter
            .debug_struct("RecordedTranscriptTiming")
            .field("bullet", &self.bullet)
            .finish_non_exhaustive()
    }
}

impl<'file> RecordedTranscriptTiming<'file> {
    /// Transcript in which the bullet was observed.
    pub fn document(&self) -> &'file ChatFile {
        self.document
    }

    /// Actual bullet, without any claim that its interval is valid.
    pub fn bullet(&self) -> &'file crate::model::Bullet {
        self.bullet
    }
}

pub use transcript_name::{
    FileStem, FileStemError, OwnedFileStem, OwnedTranscriptName, TranscriptName,
};
