//! Word-level emission, `<w>`, `<tagMarker>`, `<pause>`, `<t>`, and
//! the `<g>` wrappers for retraces and annotated words.
//!
//! Every function here writes the spoken-text side of a main-tier
//! chunk. `%mor` subtrees inside these elements are delegated to
//! `super::mor` via `emit_word_mor_subtree` so morphology logic stays
//! in one place.
//!
//! Staged features are reported via
//! [`XmlWriteError::FeatureNotImplemented`] with a descriptive
//! `feature:` string. This keeps the golden-XML harness producing
//! single-phenomenon failure diagnostics rather than swallowing
//! unimplemented constructs silently.

mod attrs;
mod parts;

use quick_xml::events::{BytesEnd, BytesStart, Event};

use talkbank_model::model::{
    Annotated, ContentAnnotation, GrammaticalRelation, Mor, Pause, PauseDuration, Retrace,
    Separator, Terminator, Word, WordContent, WordStressMarkerType,
};

use super::error::XmlWriteError;
use super::mor::{UtteranceTiers, gra_entry};
use super::writer::{XmlEmitter, escape_text};

pub(crate) use attrs::*;

impl XmlEmitter {
    /// Emit `<w[ type="…"][ untranscribed="…"]>TEXT[<mor…/>]</w>` for
    /// a main-tier [`Word`]. Form-type suffixes (`@a`, `@b`, …),
    /// language markers (`@s`), and word-internal POS suffixes are
    /// staged, fail loud so the harness picks up the missing
    /// increments.
    pub(super) fn emit_word(
        &mut self,
        word: &Word,
        mor: Option<&Mor>,
        gra: Option<&[GrammaticalRelation]>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        let mut start = BytesStart::new("w");

        // `formType`: `@a`/`@b`/… → XSD enum; `@z:label` → `user-special-form`.
        push_form_type_attrs(&mut start, word.form_type.as_ref());

        if let Some(cat) = &word.category {
            // `CAOmission` (`(parens)`) has no `type=` attribute; it
            // renders as a whole-word shortening (see
            // `emit_word_contents`), not as a `type="omission"` word.
            if let Some(attr) = word_category_attr(cat) {
                start.push_attribute(("type", attr));
            }
        }
        // `word.untranscribed()` is case-insensitive as a Stanza/MOR
        // correctness fix; the XML schema's `untranscribed` attribute
        // is case-sensitive and attaches only to the strictly
        // lowercase placeholders, gate on literal text here.
        if let Some(status) = untranscribed_attribute_for_xml(word) {
            start.push_attribute(("untranscribed", status));
        }

        self.writer.write_event(Event::Start(start))?;

        // `<langs>` child (if present) sits at the start of the
        // word, before any other content per the XSD sequence.
        if let Some(lang) = &word.lang {
            self.emit_langs(lang)?;
        }

        self.emit_word_contents(word)?;

        // Main-tier `$pos` suffix projects onto `<pos><c>tag</c></pos>`
        // as a word child per the XSD. Subcategory `<s>` children
        // aren't represented on the main-tier `Word` (that's %mor's
        // job); we emit just `<c>`.
        if let Some(pos_tag) = &word.part_of_speech {
            self.writer
                .write_event(Event::Start(BytesStart::new("pos")))?;
            self.writer
                .write_event(Event::Start(BytesStart::new("c")))?;
            self.writer
                .write_event(Event::Text(escape_text(pos_tag.as_str())))?;
            self.writer.write_event(Event::End(BytesEnd::new("c")))?;
            self.writer.write_event(Event::End(BytesEnd::new("pos")))?;
        }

        if let Some(mor) = mor {
            self.emit_word_mor_subtree(mor, gra, chunk_index_1based)?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("w")))?;
        Ok(())
    }

    /// Emit `<langs>` child for an `@s:code` marker. The schema
    /// requires a `<single>` / `<multiple>` / `<ambiguous>` child
    /// with at least one concrete language code, so bare `@s`
    /// (`Shortcut`), which toggles between primary/secondary
    /// languages from the file's `@Languages` header without
    /// naming one, projects to no `<langs>` element. This is a
    /// known round-trip gap: re-reading the XML will lose the
    /// `@s`-vs-no-marker distinction. A future enhancement could
    /// resolve the toggle against `@Languages` context and emit
    /// `<single>` with the resolved code, at the cost of losing
    /// the "this was a shortcut" signal.
    fn emit_langs(
        &mut self,
        lang: &talkbank_model::model::WordLanguageMarker,
    ) -> Result<(), XmlWriteError> {
        use talkbank_model::model::WordLanguageMarker;
        match lang {
            // Bare `@s` toggles to the secondary `@Languages` entry.
            // Without a secondary (single-language file), the shortcut
            // has no target, TalkBank XML skips the `<langs>` emission and
            // we match. With one, emit the same shape as
            // `Explicit(secondary)`.
            WordLanguageMarker::Shortcut => {
                let Some(code) = self.secondary_language.clone() else {
                    return Ok(());
                };
                self.emit_langs_single(code.as_str())
            }
            WordLanguageMarker::Explicit(code) => self.emit_langs_single(code.as_str()),
            WordLanguageMarker::Multiple(codes) => self.emit_langs_group("multiple", codes),
            WordLanguageMarker::Ambiguous(codes) => self.emit_langs_group("ambiguous", codes),
        }
    }

    fn emit_langs_single(&mut self, code: &str) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("langs")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new("single")))?;
        self.writer.write_event(Event::Text(escape_text(code)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("single")))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("langs")))?;
        Ok(())
    }

    fn emit_langs_group(
        &mut self,
        child: &'static str,
        codes: &[talkbank_model::model::LanguageCode],
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("langs")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new(child)))?;
        let mut joined = String::new();
        for (i, code) in codes.iter().enumerate() {
            if i > 0 {
                joined.push(' ');
            }
            joined.push_str(code.as_str());
        }
        self.writer.write_event(Event::Text(escape_text(&joined)))?;
        self.writer.write_event(Event::End(BytesEnd::new(child)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("langs")))?;
        Ok(())
    }

    /// Walk the word's `content` vector and emit each segment inline
    /// inside the open `<w>`. Plain text becomes XML text; word-internal
    /// markers (compound `+`, clitic `~`, syllable pause `^`, lengthening
    /// `:`, stress `ˈ`/`ˌ`) become element siblings in the same order
    /// the CHAT source presents them.
    ///
    /// Mapping table (staged increments for the rest):
    ///
    /// | CHAT | Rust variant | XML |
    /// |------|--------------|-----|
    /// | text | [`WordContent::Text`] | (raw text) |
    /// | `+` (compound) | [`WordContent::CompoundMarker`] | `<wk type="cmp"/>` |
    /// | `~` (clitic) | [`WordContent::CliticBoundary`] | `<wk type="cli"/>` |
    /// | `^` (syllable pause) | [`WordContent::SyllablePause`] | `<p type="pause"/>` |
    /// | `:` (lengthening, count N) | [`WordContent::Lengthening`] | N × `<p type="drawl"/>` |
    /// | `ˈ` primary stress | [`WordContent::StressMarker`] (Primary) | `<ca-element type="primary stress"/>` |
    /// | `ˌ` secondary stress | [`WordContent::StressMarker`] (Secondary) | `<ca-element type="secondary stress"/>` |
    ///
    /// Other variants (`(text)` shortenings, CA delimiters/elements,
    /// overlap points, underline markers) fall through to
    /// `FeatureNotImplemented`. Each is a distinct TDD increment.
    fn emit_word_contents(&mut self, word: &Word) -> Result<(), XmlWriteError> {
        // CA delimiters come in pairs (`°…°`, `∆…∆`, …) but the AST
        // stores each `CADelimiter` occurrence on its own, without an
        // explicit begin/end marker. We recover begin/end by
        // toggling a bit per delimiter type during the walk: the
        // first occurrence of a given type sets the bit (`begin`),
        // the second clears it (`end`), and a doubled `°°` pair
        // opens and immediately closes again. The state is local to
        // one word, delimiters don't span word boundaries. A `u16`
        // bitset fits all 15 `CADelimiterType` variants without
        // allocating.
        let mut ca_delim_open: u16 = 0;

        // `CAOmission` (`(fullword)`) renders as a whole-word
        // shortening in the XML schema, `<w><shortening>text</shortening></w>`
        // rather than `<w type="omission">text</w>`. Open the wrapper
        // here and close it after the content loop so every emitted
        // segment lands inside `<shortening>`.
        let ca_omission = matches!(
            word.category,
            Some(talkbank_model::model::WordCategory::CAOmission)
        );
        if ca_omission {
            self.writer
                .write_event(Event::Start(BytesStart::new("shortening")))?;
        }

        // Leading OverlapPoint items were hoisted out as siblings
        // of `<w>` by `emit_leading_overlap_points`; skip them here
        // so they don't emit twice.
        let skip_count = word
            .content
            .iter()
            .take_while(|c| matches!(c, WordContent::OverlapPoint(_)))
            .count();

        for segment in word.content.iter().skip(skip_count) {
            match segment {
                WordContent::Text(text) => {
                    self.writer
                        .write_event(Event::Text(escape_text(text.0.as_str())))?;
                }
                WordContent::CompoundMarker(_) => {
                    let mut tag = BytesStart::new("wk");
                    tag.push_attribute(("type", "cmp"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::CliticBoundary(_) => {
                    let mut tag = BytesStart::new("wk");
                    tag.push_attribute(("type", "cli"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::SyllablePause(_) => {
                    let mut tag = BytesStart::new("p");
                    tag.push_attribute(("type", "pause"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::Lengthening(lengthening) => {
                    // `no::` is `count = 2` → emit two `<p type="drawl"/>`
                    // siblings. `count` is a small positive u8 from the
                    // grammar; zero would be a parser bug.
                    for _ in 0..lengthening.count {
                        let mut tag = BytesStart::new("p");
                        tag.push_attribute(("type", "drawl"));
                        self.writer.write_event(Event::Empty(tag))?;
                    }
                }
                WordContent::StressMarker(marker) => {
                    let type_attr = match marker.marker_type {
                        WordStressMarkerType::Primary => "primary stress",
                        WordStressMarkerType::Secondary => "secondary stress",
                    };
                    let mut tag = BytesStart::new("ca-element");
                    tag.push_attribute(("type", type_attr));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::Shortening(shortening) => {
                    // `(text)` inside a word, a shortened/omitted sound
                    // segment that TalkBank XML preserves as an inline
                    // `<shortening>text</shortening>` wrapper around the
                    // unspoken characters.
                    self.writer
                        .write_event(Event::Start(BytesStart::new("shortening")))?;
                    self.writer
                        .write_event(Event::Text(escape_text(shortening.0.as_str())))?;
                    self.writer
                        .write_event(Event::End(BytesEnd::new("shortening")))?;
                }
                WordContent::CAElement(element) => {
                    let mut tag = BytesStart::new("ca-element");
                    tag.push_attribute(("type", ca_element_label(element)));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::OverlapPoint(point) => {
                    self.emit_overlap_point(point)?;
                }
                WordContent::CADelimiter(delimiter) => {
                    let bit = 1u16 << ca_delimiter_bit_index(delimiter.delimiter_type);
                    let was_open = (ca_delim_open & bit) != 0;
                    ca_delim_open ^= bit;
                    let type_attr = if was_open { "end" } else { "begin" };
                    let mut tag = BytesStart::new("ca-delimiter");
                    tag.push_attribute(("type", type_attr));
                    tag.push_attribute(("label", ca_delimiter_label(delimiter)));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::UnderlineBegin(_) => {
                    let mut tag = BytesStart::new("underline");
                    tag.push_attribute(("type", "begin"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::UnderlineEnd(_) => {
                    let mut tag = BytesStart::new("underline");
                    tag.push_attribute(("type", "end"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
            }
        }

        if ca_omission {
            self.writer
                .write_event(Event::End(BytesEnd::new("shortening")))?;
        }
        Ok(())
    }

    /// Emit `<t type="p|q|e"/>` for an utterance terminator. When the
    /// paired `%mor` tier carries a terminator chunk, nests
    /// `<mor><mt/><gra/></mor>` inside the `<t>` to match the
    /// TalkBank XML output shape. Staged terminators (trailing-off,
    /// interruption, …) fail loud.
    pub(super) fn emit_terminator(
        &mut self,
        terminator: &Terminator,
        tiers: &UtteranceTiers<'_>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        // CA intonation-contour arrows (`⇗ ↗ → ↘ ⇘`) and CA TCU
        // markers (`≋ +≋ ≈ +≈`) are modeled as ``Separator`` variants,
        // not as ``Terminator`` variants, the parser dispatches them
        // through ``non_colon_separator``. So this code path only sees
        // genuine terminators (``Period``, ``Question``, ``+/.`` etc.)
        // and the ``<s>`` peel-off that used to live here for the CA
        // case is now naturally emitted by the separator renderer at
        // the call site that produces ``<s type="…"/>`` from each
        // ``Separator`` variant.

        // `<t type="…"/>`. TalkBank XML uses the short letter code
        // for the three standard sentence terminators and a prose
        // phrase for CA-specific variants.
        let ty = terminator_type_attr(terminator);
        let mut start = BytesStart::new("t");
        start.push_attribute(("type", ty));

        // When %mor is present and carries a terminator chunk, TalkBank
        // XML nests `<mor type="mor"><mt type="X"/><gra.../></mor>`
        // inside `<t>`, making it a non-empty element. Without %mor,
        // `<t>` stays empty (structural comparator folds `<t/>` vs
        // `<t></t>`).
        // MorTier always has a terminator, so presence-check
        // reduces to "is there a %mor tier at all."
        let has_mor_terminator = tiers.mor.is_some();
        if has_mor_terminator {
            self.writer.write_event(Event::Start(start))?;
            let mut mor = BytesStart::new("mor");
            mor.push_attribute(("type", "mor"));
            self.writer.write_event(Event::Start(mor))?;
            let mut mt = BytesStart::new("mt");
            mt.push_attribute(("type", ty));
            self.writer.write_event(Event::Empty(mt))?;
            if let Some(rel) = gra_entry(tiers.gra, chunk_index_1based) {
                self.emit_gra(rel)?;
            }
            self.writer.write_event(Event::End(BytesEnd::new("mor")))?;
            self.writer.write_event(Event::End(BytesEnd::new("t")))?;
        } else {
            self.writer.write_event(Event::Empty(start))?;
        }
        Ok(())
    }

    /// Emit a main-tier separator token. TalkBank XML uses two XML
    /// element shapes here depending on the separator kind:
    ///
    /// - `<tagMarker type="…"/>` for structural separators (`,`,
    ///   `;`, `:`, `„`, `‡`). The tag-marker variants that
    ///   participate in `%mor` alignment (Comma, Tag, Vocative) get
    ///   a nested `<mor>` subtree when tiers are present.
    /// - `<s type="…"/>` for CA intonation / uptake / unmarked-ending
    ///   separators. These are empty elements, no `%mor` alignment.
    ///
    /// `CaContinuation` ([^c]) is a staged increment; its schema
    /// shape differs from both of the above and it doesn't appear in
    /// the reference corpus yet.
    pub(super) fn emit_separator(
        &mut self,
        sep: &Separator,
        mor: Option<&Mor>,
        gra: Option<&[GrammaticalRelation]>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        if let Some(s_type) = s_separator_label(sep) {
            let mut tag = BytesStart::new("s");
            tag.push_attribute(("type", s_type));
            self.writer.write_event(Event::Empty(tag))?;
            return Ok(());
        }

        // Every `Separator` variant routes to either `<s>` (above)
        // or `<tagMarker>` (here); this fallthrough only fires if a
        // new variant lands in the model without updating both
        // helpers. Fail loud rather than silently emit an invalid
        // attribute, the XSD rejects anything that isn't
        // `comma|tag|vocative`.
        let tag_type =
            separator_tag_type(sep).ok_or_else(|| XmlWriteError::FeatureNotImplemented {
                feature: format!("separator variant without XML mapping: {sep:?}"),
            })?;
        let mut start = BytesStart::new("tagMarker");
        start.push_attribute(("type", tag_type));

        match mor {
            Some(mor) => {
                self.writer.write_event(Event::Start(start))?;
                self.emit_word_mor_subtree(mor, gra, chunk_index_1based)?;
                self.writer
                    .write_event(Event::End(BytesEnd::new("tagMarker")))?;
            }
            None => {
                self.writer.write_event(Event::Empty(start))?;
            }
        }
        Ok(())
    }

    /// Emit `<pause symbolic-length="…" [length="…"]/>`. Symbolic
    /// pauses use one of the three XSD enum values; timed pauses
    /// add a numeric `length` attribute while keeping
    /// `symbolic-length="simple"` since that attribute is required
    /// by the schema even when the timing is explicit.
    pub(super) fn emit_pause(&mut self, pause: &Pause) -> Result<(), XmlWriteError> {
        let length_str;
        let (symbolic, numeric) = match &pause.duration {
            PauseDuration::Short => ("simple", None),
            PauseDuration::Medium => ("long", None),
            PauseDuration::Long => ("very long", None),
            PauseDuration::Timed(timed) => {
                use talkbank_model::model::PauseTimedDuration;
                length_str = match timed {
                    PauseTimedDuration::Parsed {
                        seconds, millis, ..
                    } => match millis {
                        // Emit canonical decimal form without
                        // trailing zeros: "(3.4)" round-trips to
                        // `length="3.4"`, not `"3.400"`. The model
                        // stores millis as a 0..999 value, so trim
                        // the 3-digit string representation from the
                        // right. `Some(0)` is equivalent to no
                        // millis and collapses to `seconds`.
                        Some(0) => seconds.to_string(),
                        Some(ms) => {
                            let padded = format!("{ms:03}");
                            let trimmed = padded.trim_end_matches('0');
                            format!("{seconds}.{trimmed}")
                        }
                        None => seconds.to_string(),
                    },
                    PauseTimedDuration::Unsupported(raw) => raw.as_str().to_owned(),
                };
                ("simple", Some(length_str.as_str()))
            }
        };
        let mut start = BytesStart::new("pause");
        start.push_attribute(("symbolic-length", symbolic));
        if let Some(n) = numeric {
            start.push_attribute(("length", n));
        }
        self.writer.write_event(Event::Empty(start))?;
        Ok(())
    }

    /// Emit `<g>…<k type="retracing"/>[annotation…]</g>` for a
    /// retrace. The inner content is walked with NO `%mor` alignment
    /// (retraced text is excluded from `%mor` by CHAT convention).
    /// Scoped annotations attached to the retrace (`[/] [= text]`,
    /// `[/?] [!]`, …) are emitted as sibling children of `<k>` inside
    /// the same `<g>` wrapper, using the same dispatch as
    /// `emit_annotated_word`.
    pub(super) fn emit_retrace(&mut self, retrace: &Retrace) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("g")))?;
        for item in retrace.content.content.iter() {
            self.emit_bracketed_word_only(item)?;
        }
        // Scoped annotations emit before the retrace-kind `<k>` so the
        // retrace marker closes the group, matching `nay [?] [//]`.
        for annotation in retrace.annotations.iter() {
            self.emit_scoped_annotation(annotation)?;
        }
        let kind = retrace_kind_attr(retrace.kind);
        let mut k = BytesStart::new("k");
        k.push_attribute(("type", kind));
        self.writer.write_event(Event::Empty(k))?;
        self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        Ok(())
    }

    /// Emit `<g><w>word[<mor>...]</w><error/></g>` for a word carrying
    /// a single `[*]` error annotation. Richer annotation kinds, `[=]`,
    /// `[+]`, `[!]`, overlap markers etc., each need their own
    /// increment.
    pub(super) fn emit_annotated_word(
        &mut self,
        annotated: &Annotated<Word>,
        mor: Option<&Mor>,
        gra: Option<&[GrammaticalRelation]>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        // TalkBank XML wraps every annotated word in `<g>…</g>` and
        // emits one child per scoped annotation after the `<w>`. We
        // reject unknown annotation kinds up front so the harness
        // reports the missing feature precisely.
        let annotations = annotated.scoped_annotations.as_slice();
        if annotations.is_empty() {
            return Err(XmlWriteError::MissingMetadata {
                what: "annotated word reached emitter without scoped annotations".to_owned(),
            });
        }

        self.writer
            .write_event(Event::Start(BytesStart::new("g")))?;
        self.emit_word(&annotated.inner, mor, gra, chunk_index_1based)?;

        for annotation in annotations {
            self.emit_scoped_annotation(annotation)?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        Ok(())
    }

    /// Emit the XML child element corresponding to a single
    /// scoped-annotation. Mapping per `talkbank.xsd`:
    ///
    /// | CHAT | Rust variant | XML |
    /// |------|--------------|-----|
    /// | `[*]` (no code) | [`ContentAnnotation::Error`] | `<error/>` |
    /// | `[*error_text]` | [`ContentAnnotation::Error`] with code | `<error>text</error>` |
    /// | `[= text]`      | [`ContentAnnotation::Explanation`] | `<ga type="explanation">text</ga>` |
    /// | `[% text]`      | [`ContentAnnotation::PercentComment`] | `<ga type="comments">text</ga>` |
    /// | `[=? text]`     | [`ContentAnnotation::Alternative`] | `<ga type="alternative">text</ga>` |
    /// | `[=! text]`     | [`ContentAnnotation::Paralinguistic`] | `<ga type="paralinguistics">text</ga>` |
    /// | `[!]`           | [`ContentAnnotation::Stressing`] | `<k type="stressing"/>` |
    /// | `[!!]`          | [`ContentAnnotation::ContrastiveStressing`] | `<k type="contrastive stressing"/>` |
    /// | `[?]`           | [`ContentAnnotation::Uncertain`] / `BestGuess` | `<k type="best guess"/>` |
    /// | `[e]`           | [`ContentAnnotation::Exclude`] | `<k type="mor exclude"/>` |
    /// | `[<N]` / `[>N]` | overlap | `<overlap type="…" index="N"/>` |
    fn emit_scoped_annotation(
        &mut self,
        annotation: &ContentAnnotation,
    ) -> Result<(), XmlWriteError> {
        match annotation {
            ContentAnnotation::Error(err) => {
                if let Some(code) = &err.code {
                    self.writer
                        .write_event(Event::Start(BytesStart::new("error")))?;
                    self.writer
                        .write_event(Event::Text(escape_text(code.as_str())))?;
                    self.writer
                        .write_event(Event::End(BytesEnd::new("error")))?;
                } else {
                    self.writer
                        .write_event(Event::Empty(BytesStart::new("error")))?;
                }
                Ok(())
            }
            ContentAnnotation::Explanation(expl) => self.emit_ga("explanation", expl.text.as_str()),
            ContentAnnotation::PercentComment(cmt) => self.emit_ga("comments", cmt.text.as_str()),
            ContentAnnotation::Alternative(alt) => self.emit_ga("alternative", alt.text.as_str()),
            ContentAnnotation::Paralinguistic(para) => {
                self.emit_ga("paralinguistics", para.text.as_str())
            }
            ContentAnnotation::Stressing => self.emit_k("stressing"),
            ContentAnnotation::ContrastiveStressing => self.emit_k("contrastive stressing"),
            ContentAnnotation::Uncertain => self.emit_k("best guess"),
            ContentAnnotation::Exclude => self.emit_k("mor exclude"),
            ContentAnnotation::OverlapBegin(begin) => {
                let mut tag = BytesStart::new("overlap");
                tag.push_attribute(("type", "overlap precedes"));
                if let Some(index) = &begin.index {
                    let s = index.to_string();
                    let mut t2 = tag;
                    t2.push_attribute(("index", s.as_str()));
                    self.writer.write_event(Event::Empty(t2))?;
                } else {
                    self.writer.write_event(Event::Empty(tag))?;
                }
                Ok(())
            }
            ContentAnnotation::OverlapEnd(end) => {
                let mut tag = BytesStart::new("overlap");
                tag.push_attribute(("type", "overlap follows"));
                if let Some(index) = &end.index {
                    let s = index.to_string();
                    let mut t2 = tag;
                    t2.push_attribute(("index", s.as_str()));
                    self.writer.write_event(Event::Empty(t2))?;
                } else {
                    self.writer.write_event(Event::Empty(tag))?;
                }
                Ok(())
            }
            ContentAnnotation::Unknown(unknown) => {
                // Lenient-parse fallback for unrecognised `[…]`
                // annotations. Preserve marker + text as a generic
                // `<ga type="comments">` so the payload survives
                // round-trip; the validator has already flagged the
                // shape.
                let payload = format!("{}{}", unknown.marker, unknown.text);
                self.emit_ga("comments", &payload)
            }
        }
    }

    fn emit_k(&mut self, ty: &'static str) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("k");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }

    fn emit_ga(&mut self, ty: &'static str, text: &str) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("ga");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Start(tag))?;
        self.writer.write_event(Event::Text(escape_text(text)))?;
        self.writer.write_event(Event::End(BytesEnd::new("ga")))?;
        Ok(())
    }
}
