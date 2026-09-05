//! Reconcile a document's timing evidence with its `@Media` declaration.
//!
//! A timing-producing transform starts with ordinary [`ChatFile`] and ends in
//! one of two explicit states: the document still has no timing, or it has
//! timing and exactly one usable, linked media declaration. The timed state is
//! the only route that removes `unlinked`, so callers cannot serialize fresh
//! timings while forgetting the header transition.

use talkbank_model::WriteChat;
use talkbank_model::model::{ChatFile, Header, Line, MediaStatus, MediaType, WorTimingEvidence};

/// A document proved to contain no main-tier or `%wor` timing bullets.
#[derive(Debug)]
pub struct UntimedChatFile(ChatFile);

impl UntimedChatFile {
    /// Borrow the proved untimed document.
    pub fn as_chat_file(&self) -> &ChatFile {
        &self.0
    }
}

/// A timed document whose sole media declaration is usable and linked.
#[derive(Debug)]
pub struct LinkedMediaChatFile(ChatFile);

impl LinkedMediaChatFile {
    /// Borrow the proved timed, media-linked document.
    pub fn as_chat_file(&self) -> &ChatFile {
        &self.0
    }
}

/// The two document states that can leave timing reconciliation.
#[derive(Debug)]
pub enum MediaTimingState {
    /// The transform produced no timing evidence, so the media declaration is
    /// preserved exactly as received.
    Untimed(UntimedChatFile),
    /// Timing exists and the media declaration now records linkage.
    Timed(LinkedMediaChatFile),
}

impl MediaTimingState {
    /// Borrow the reconciled document without erasing which state owns it.
    pub fn as_chat_file(&self) -> &ChatFile {
        match self {
            Self::Untimed(file) => file.as_chat_file(),
            Self::Timed(file) => file.as_chat_file(),
        }
    }

    /// Serialize the reconciled document to canonical CHAT text.
    pub fn to_chat_string(&self) -> String {
        self.as_chat_file().to_chat_string()
    }
}

/// Why a timed document could not transition to linked media.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaTimingError {
    /// Timing bullets have no media timeline to index.
    #[error("timed CHAT has no @Media declaration")]
    MissingMedia,
    /// More than one declaration makes the target timeline ambiguous.
    #[error("timed CHAT has {count} @Media declarations; expected exactly one")]
    MultipleMedia {
        /// Number of media headers found.
        count: usize,
    },
    /// The media type itself says no usable recording exists.
    #[error("timed CHAT declares unusable media type {media_type:?}")]
    UnusableMediaType {
        /// Typed media declaration that cannot name a usable timeline.
        media_type: MediaType,
    },
    /// A status other than `unlinked` contradicts successful timing.
    #[error("timed CHAT declares incompatible media status {status:?}")]
    IncompatibleMediaStatus {
        /// Typed status that contradicts successful timing.
        status: MediaStatus,
    },
}

/// Consume a transformed document and reconcile its media/timing state.
///
/// Untimed documents remain byte-semantically unchanged. A timed document must
/// have exactly one audio or video declaration. Its `unlinked` status is
/// consumed by this transition; an already-linked declaration remains linked,
/// while contradictory statuses are refused with a typed error.
pub fn reconcile_media_timing(mut file: ChatFile) -> Result<MediaTimingState, MediaTimingError> {
    if !has_timing_evidence(&file) {
        return Ok(MediaTimingState::Untimed(UntimedChatFile(file)));
    }

    let media_count = file
        .lines
        .iter()
        .filter(|line| {
            matches!(
                line,
                Line::Header { header, .. } if matches!(header.as_ref(), Header::Media(_))
            )
        })
        .count();
    match media_count {
        0 => return Err(MediaTimingError::MissingMedia),
        1 => {}
        count => return Err(MediaTimingError::MultipleMedia { count }),
    }

    let Some(media) = file
        .lines
        .as_mut_slice()
        .iter_mut()
        .find_map(|line| match line {
            Line::Header { header, .. } => match header.as_mut() {
                Header::Media(media) => Some(media),
                _ => None,
            },
            Line::Utterance(_) => None,
        })
    else {
        return Err(MediaTimingError::MissingMedia);
    };

    match &media.media_type {
        MediaType::Audio | MediaType::Video => {}
        MediaType::Missing | MediaType::Unsupported(_) => {
            return Err(MediaTimingError::UnusableMediaType {
                media_type: media.media_type.clone(),
            });
        }
    }

    match media.status.as_ref() {
        None => {}
        Some(MediaStatus::Unlinked) => media.status = None,
        Some(
            status @ (MediaStatus::Missing | MediaStatus::Notrans | MediaStatus::Unsupported(_)),
        ) => {
            return Err(MediaTimingError::IncompatibleMediaStatus {
                status: status.clone(),
            });
        }
    }

    file.media = Some(Box::new(media.clone()));
    Ok(MediaTimingState::Timed(LinkedMediaChatFile(file)))
}

fn has_timing_evidence(file: &ChatFile) -> bool {
    file.utterances().any(|utterance| {
        utterance.main.content.bullet.is_some()
            || utterance.wor_tier().is_some_and(|tier| {
                matches!(tier.timing_evidence(), WorTimingEvidence::Recorded(_))
            })
    })
}
