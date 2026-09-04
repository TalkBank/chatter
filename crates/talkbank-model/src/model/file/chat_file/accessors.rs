//! Read-oriented accessors over `ChatFile` structure and participants.
//!
//! These helpers give downstream readers deterministic iteration order without
//! exposing the internal `Line` enum. `get_participant` and `all_participants`
//! reuse the canonical participant map to avoid re-parsing `@Participants`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>

use crate::model::header::IDHeader;
use crate::model::{DeclaredSpeaker, Participant, Utterance};
use crate::{Header, Span, WriteChat};
use tracing::{debug, info};

use super::ChatFile;

impl ChatFile {
    /// Iterates header lines in original file order.
    ///
    /// Order preservation matters because CHAT allows headers to appear between
    /// utterances (for example `@Comment` lines mid-file).
    pub fn headers(&self) -> impl Iterator<Item = &Header> {
        self.lines.iter().filter_map(|line| line.as_header())
    }

    /// Iterates the file's `@ID` headers in original file order.
    ///
    /// Convenience over [`headers`](Self::headers) for the common case of
    /// "give me the typed `@ID` rows", every CHAT reader that filters
    /// header lines by variant would otherwise hand-roll the same
    /// `matches!(_, Header::ID(_))` extraction.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::ChatFile;
    /// # let chat_file = ChatFile::new(vec![]);
    /// for id in chat_file.id_headers() {
    ///     println!("speaker {} role {}", id.speaker, id.role);
    /// }
    /// ```
    pub fn id_headers(&self) -> impl Iterator<Item = &IDHeader> {
        self.headers().filter_map(|h| match h {
            Header::ID(id) => Some(id),
            _ => None,
        })
    }

    /// Iterates header lines with source spans in file order.
    ///
    /// Useful for diagnostics or transforms that need both typed header values
    /// and their byte locations in the source transcript.
    pub fn headers_with_spans(&self) -> impl Iterator<Item = (&Header, crate::Span)> {
        self.lines.iter().filter_map(|line| match line {
            crate::model::Line::Header { header, span, .. } => Some((header.as_ref(), *span)),
            _ => None,
        })
    }

    /// Iterates utterance lines in original file order.
    ///
    /// Returned items exclude header lines but keep relative utterance ordering unchanged.
    pub fn utterances(&self) -> impl Iterator<Item = &Utterance> {
        self.lines.iter().filter_map(|line| line.as_utterance())
    }

    /// Finds the utterance whose main tier or a dependent tier contains `offset`.
    ///
    /// Containment is HALF-OPEN (`start <= offset < end`), matching [`Span`]'s
    /// documented semantics, so an offset sitting exactly at one utterance's
    /// end belongs to the next utterance, never to both. This is the shared
    /// home for "which utterance is byte N in": callers that want inclusive-end
    /// behaviour (for example an editor cursor resting at the end of a word)
    /// must widen the offset themselves and document why.
    pub fn utterance_containing(&self, offset: u32) -> Option<&Utterance> {
        self.utterances().find(|utterance| {
            span_contains_half_open(utterance.main.span, offset)
                || utterance
                    .dependent_tiers
                    .iter()
                    .any(|entry| span_contains_half_open(entry.tier.span(), offset))
        })
    }

    /// Returns the number of header lines in `self.lines`.
    ///
    /// This is computed on demand from line variants instead of cached metadata.
    pub fn header_count(&self) -> usize {
        self.lines.iter().filter(|line| line.is_header()).count()
    }

    /// Returns the number of utterance lines in `self.lines`.
    ///
    /// This is computed on demand from line variants instead of cached metadata.
    pub fn utterance_count(&self) -> usize {
        self.lines.iter().filter(|line| line.is_utterance()).count()
    }

    /// Returns the distinct speaker codes appearing on utterance main
    /// tiers, in document order of first appearance.
    ///
    /// Useful for transforms that need the actual speaker set of the
    /// utterance body, distinct from `id_headers()` (declared @ID
    /// speakers, which may include speakers with no utterances or
    /// omit speakers present only via ad-hoc *XXX: lines).
    pub fn unique_utterance_speakers(&self) -> Vec<crate::SpeakerCode> {
        let mut out: Vec<crate::SpeakerCode> = Vec::new();
        let mut seen: std::collections::HashSet<crate::SpeakerCode> =
            std::collections::HashSet::new();
        for utt in self.utterances() {
            if seen.insert(utt.main.speaker.clone()) {
                out.push(utt.main.speaker.clone());
            }
        }
        out
    }

    /// Returns participant metadata for a speaker code, if present.
    ///
    /// Lookups are exact and case-sensitive, matching canonical CHAT speaker codes.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::ChatFile;
    /// # let chat_file = ChatFile::new(vec![]);
    /// if let Some(chi) = chat_file.get_participant("CHI") {
    ///     println!("CHI's age: {:?}", chi.age());
    /// }
    /// ```
    pub fn get_participant(&self, code: &str) -> Option<&Participant> {
        self.participants.get(code)
    }

    /// Returns all participants from the internal participant map.
    ///
    /// Order follows map iteration and should not be assumed stable for UI ordering.
    ///
    /// **This is the `@ID` join, not the roster.** A speaker declared in
    /// `@Participants` with no `@ID` header is absent here. For "who is in
    /// this transcript", use
    /// [`declared_speakers`](Self::declared_speakers) instead.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::ChatFile;
    /// # let chat_file = ChatFile::new(vec![]);
    /// for participant in chat_file.all_participants() {
    ///     println!("{}: {}", participant.code, participant.role);
    /// }
    /// ```
    pub fn all_participants(&self) -> Vec<&Participant> {
        self.participants.values().collect()
    }

    /// Iterates every speaker DECLARED in `@Participants`, in declaration
    /// order, each enriched with its `@ID` metadata when that header exists.
    ///
    /// **Prefer this to [`all_participants`](Self::all_participants) for any
    /// question about who is in the transcript.** The two differ exactly when
    /// the file is invalid, and this one is the roster.
    /// [`participants`](Self::participants) is keyed and populated from the
    /// `@ID` join, so a speaker declared without an `@ID` raises E522 and is
    /// then simply absent from the map: a consumer iterating the map sees
    /// fewer speakers than the file declares, with nothing to say so. Here
    /// that speaker is present with
    /// [`id_metadata`](crate::model::DeclaredSpeaker::id_metadata) `None`.
    ///
    /// Returns an empty iterator when the file has no `@Participants` header,
    /// which is itself invalid CHAT.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::ChatFile;
    /// # let chat_file = ChatFile::new(vec![]);
    /// for speaker in chat_file.declared_speakers() {
    ///     match speaker.id_metadata() {
    ///         Some(meta) => println!("{} ({}) age {:?}", speaker.code(), speaker.role(), meta.age()),
    ///         None => println!("{} ({}) has no @ID header", speaker.code(), speaker.role()),
    ///     }
    /// }
    /// ```
    pub fn declared_speakers(&self) -> impl Iterator<Item = DeclaredSpeaker<'_>> {
        self.participant_entries()
            .into_iter()
            .flatten()
            .map(|entry| DeclaredSpeaker::new(entry, self.participants.get(&entry.speaker_code)))
    }

    /// The `@Participants` payload, in declared order, if the file has one.
    ///
    /// Sibling of [`id_headers`](Self::id_headers), and there for the same
    /// stated reason: a reader that wants the declaration list should not
    /// hand-roll the `Header::Participants` match.
    pub fn participant_entries(&self) -> Option<&crate::model::ParticipantEntries> {
        self.headers().find_map(|header| match header {
            Header::Participants { entries } => Some(entries),
            _ => None,
        })
    }

    /// Returns number of participant entries currently materialized.
    ///
    /// This reflects parsed/validated participant state, not a separate header reparse.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Serializes the full file to an owned CHAT string.
    ///
    /// Instrumentation fields capture line/header/utterance counts so tracing
    /// backends can correlate serialization cost with transcript size.
    ///
    /// Header/utterance ordering is preserved so serialization roundtrips can be
    /// verified against `ChatFile::lines`.
    #[tracing::instrument(skip(self), fields(lines = self.lines.len()))]
    pub fn to_chat(&self) -> String {
        let header_count = self.header_count();
        let utterance_count = self.utterance_count();
        debug!(
            "Serializing CHAT file ({} lines: {} headers, {} utterances)",
            self.lines.len(),
            header_count,
            utterance_count
        );
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        info!("Serialized to {} bytes", s.len());
        s
    }
}

/// Half-open containment: `start <= offset < end`.
///
/// A [`Span::DUMMY`]/[`Span::is_dummy`] span never contains anything: it
/// marks "no real source location," not a zero-width span at byte 0.
fn span_contains_half_open(span: Span, offset: u32) -> bool {
    !span.is_dummy() && offset >= span.start && offset < span.end
}
