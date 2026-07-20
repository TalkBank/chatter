//! Word-level XML emission (group/bracketed/replaced/event variants),
//! split from `mod.rs` for file size.

use quick_xml::events::{BytesEnd, BytesStart, Event};

use talkbank_model::alignment::TierDomain;
use talkbank_model::alignment::helpers::counts_for_tier;
use talkbank_model::model::{
    Action, Annotated, BracketedItem, Event as CEvent, Linker, LinkerKind, OverlapPointKind,
    ReplacedWord, Word, WordContent,
};

use super::super::error::XmlWriteError;
use super::super::mor::{TierCursors, UtteranceTiers};
use super::super::writer::{XmlEmitter, escape_text};

use super::attrs::*;

impl XmlEmitter {
    /// Emit `<w>original<replacement><w>r1</w><w>r2</w>…</replacement></w>`.
    /// Each replacement word consumes one `%mor` / `%gra` chunk, so
    /// the caller tracks the cursor via the returned
    /// `chunks_consumed` value.
    ///
    /// `%mor` items align to the *replacement* words, not the
    /// original, that matches the TalkBank XML format and the CHAT
    /// convention of aligning morphology to the intended form. A
    /// single-word replacement consumes one chunk; `dunno [: don't
    /// know]` consumes two.
    ///
    /// Returns the number of `%mor`/`%gra` chunks the replacement
    /// consumed. Callers in `emit_utterance` use this to advance
    /// their running cursor.
    ///
    /// Scoped annotations attached to the replaced word are a
    /// staged increment, `[: text] [= explanation]` and similar
    /// patterns require emitting both a `<replacement>` and
    /// annotation siblings inside a `<g>` wrapper, which isn't
    /// wired yet.
    pub(crate) fn emit_replaced_word(
        &mut self,
        rw: &ReplacedWord,
        tiers: &UtteranceTiers<'_>,
        cursors: &mut TierCursors,
    ) -> Result<(), XmlWriteError> {
        // `полетел [: полетела] [*]`, replacement + error, renders
        // as `<g><w>…<replacement>…</replacement></w><error/></g>`.
        // When scoped annotations are present, wrap the whole
        // replaced-word shape in `<g>` and emit each annotation as
        // a sibling after the outer `<w>` closes.
        let wrap_in_g = !rw.scoped_annotations.is_empty();
        if wrap_in_g {
            self.writer
                .write_event(Event::Start(BytesStart::new("g")))?;
        }

        // Outer <w> with the original spoken text, no mor subtree.
        // category / untranscribed on the original carry through so
        // `0word [: replacement]` still emits `type="omission"`.
        // (CAOmission intentionally omits the attribute, see
        // `word_category_attr`.)
        let mut outer = BytesStart::new("w");
        // The original spoken-side word's form_type projects to the
        // outer `<w>`; without it, `wɨspatsing@u [: whispering]` emits
        // `<w>` with no `formType` and loses phonological coding.
        push_form_type_attrs(&mut outer, rw.word.form_type.as_ref());
        if let Some(cat) = &rw.word.category
            && let Some(attr) = word_category_attr(cat)
        {
            outer.push_attribute(("type", attr));
        }
        self.writer.write_event(Event::Start(outer))?;
        // Emit the original word's structured content rather than
        // flattened `cleaned_text()`, preserves compound markers
        // (`<wk type="cmp"/>`), clitic boundaries, stress markers, and
        // other word-internal structure that the replacement's outer
        // `<w>` needs to round-trip through XML. Flat text would fuse
        // `rocking+house` into `rockinghouse`.
        self.emit_word_contents(&rw.word)?;

        self.writer
            .write_event(Event::Start(BytesStart::new("replacement")))?;
        for replacement_word in rw.replacement.words.0.iter() {
            // Each replacement word consumes one Mor item (with its
            // post-clitics inline) plus `1 + post_clitics.len()` `%gra`
            // edges, so a `dunno [: don't know's]`-style expansion
            // stays aligned even if a token carries a clitic chain.
            let mor_for_word = tiers
                .mor
                .as_ref()
                .and_then(|mor| mor.items().get(cursors.mor_index()));
            self.emit_word(
                replacement_word,
                mor_for_word,
                tiers.gra,
                cursors.gra_chunk(),
            )?;
            if tiers.mor.is_some() {
                let post_count = mor_for_word.map(|m| m.post_clitics.len()).unwrap_or(0);
                cursors.consume_mor(post_count);
            }
        }
        self.writer
            .write_event(Event::End(BytesEnd::new("replacement")))?;

        self.writer.write_event(Event::End(BytesEnd::new("w")))?;

        if wrap_in_g {
            for annotation in rw.scoped_annotations.iter() {
                self.emit_scoped_annotation(annotation)?;
            }
            self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        }

        Ok(())
    }

    /// Emit an inline `&=descriptor` event as `<e><happening>text</happening></e>`.
    /// `&=laughs`, `&=rire`, `&=coughs` and similar non-speech event
    /// markers sit in main-tier content outside the word alignment,
    /// they never consume `%mor` chunks.
    pub(crate) fn emit_event(&mut self, event: &CEvent) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("e")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new("happening")))?;
        self.writer
            .write_event(Event::Text(escape_text(event.event_type.as_str())))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("happening")))?;
        self.writer.write_event(Event::End(BytesEnd::new("e")))?;
        Ok(())
    }

    /// Emit an annotated bare-action token as `<e><action/></e>`. The
    /// `Annotated<Action>` wrapper carries scoped annotations in
    /// principle; currently only the empty-annotations case is wired
    /// because that is the only shape exercised by the reference
    /// corpus. Richer cases (`0 [= description]`) fail loud.
    pub(crate) fn emit_annotated_action(
        &mut self,
        annotated: &Annotated<Action>,
    ) -> Result<(), XmlWriteError> {
        // `<e><action/>[annotation…]</e>`. Scoped annotations
        // attach inside `<e>` per the XSD sequence (same as
        // `emit_annotated_event`). The bare `0 .` case has no
        // annotations and renders as `<e><action/></e>`.
        self.writer
            .write_event(Event::Start(BytesStart::new("e")))?;
        self.writer
            .write_event(Event::Empty(BytesStart::new("action")))?;
        for annotation in annotated.scoped_annotations.iter() {
            self.emit_scoped_annotation(annotation)?;
        }
        self.writer.write_event(Event::End(BytesEnd::new("e")))?;
        Ok(())
    }

    /// Emit `<linker type="…"/>` for a discourse linker (`+<`, `++`,
    /// `+≈`, `+≋`, …). Called once per item in
    /// `utterance.main.content.linkers` before any main-tier word is
    /// written. Staged variants (`+<` lazy-overlap, `++` completion)
    /// fail loud so each missing mapping shows up in the harness.
    pub(crate) fn emit_linker(&mut self, linker: &Linker) -> Result<(), XmlWriteError> {
        let ty = match linker.kind {
            LinkerKind::QuotationFollows => "quoted utterance next",
            LinkerKind::QuickUptakeOverlap => "quick uptake",
            LinkerKind::LazyOverlapPrecedes => "lazy overlap mark",
            LinkerKind::SelfCompletion => "self completion",
            LinkerKind::OtherCompletion => "other completion",
            LinkerKind::TcuContinuation => "technical break TCU completion",
            LinkerKind::NoBreakTcuContinuation => "no break TCU completion",
        };
        let mut tag = BytesStart::new("linker");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }

    /// Hoist leading `OverlapPoint` items out of a word's content
    /// as top-level `<overlap-point/>` siblings. The Rust parser
    /// bundles markers at the start of a word (e.g. `⌈` in
    /// `⌈°overlapping+soft⌉°`) into `word.content`; the TalkBank XML
    /// format keeps the *leading* ones outside the `<w>` element, only
    /// internal / trailing overlap points remain inside `<w>`. This
    /// method emits the leading prefix only; `emit_word_contents`
    /// knows to skip them when walking the word body.
    pub(crate) fn emit_leading_overlap_points(&mut self, word: &Word) -> Result<(), XmlWriteError> {
        for item in word.content.iter() {
            match item {
                WordContent::OverlapPoint(point) => {
                    self.emit_overlap_point(point)?;
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Emit `<overlap-point start-end=… top-bottom=… [index=…]/>`.
    /// Shared between three callers: `emit_utterance` for top-level
    /// overlap markers (siblings of `<w>`), `emit_leading_overlap_points`
    /// for hoisted leading markers, and `emit_word_contents` for
    /// word-internal trailer markers. The XML shape is identical in
    /// all three contexts, only the enclosing element differs.
    /// Concurrent overlap regions carry an `index` attribute so
    /// downstream consumers can pair up begin/end markers; absent when
    /// the file uses only one overlap pair at a time.
    pub(crate) fn emit_overlap_point(
        &mut self,
        point: &talkbank_model::model::OverlapPoint,
    ) -> Result<(), XmlWriteError> {
        let (start_end, top_bottom) = match point.kind {
            OverlapPointKind::TopOverlapBegin => ("start", "top"),
            OverlapPointKind::TopOverlapEnd => ("end", "top"),
            OverlapPointKind::BottomOverlapBegin => ("start", "bottom"),
            OverlapPointKind::BottomOverlapEnd => ("end", "bottom"),
        };
        let index_str = point.index.as_ref().map(|i| i.to_string());
        let mut tag = BytesStart::new("overlap-point");
        if let Some(s) = index_str.as_deref() {
            tag.push_attribute(("index", s));
        }
        tag.push_attribute(("start-end", start_end));
        tag.push_attribute(("top-bottom", top_bottom));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }

    /// Emit `<g><w>word1</w>…<annotation/></g>` for a group wrapped
    /// in scoped annotations, the main-tier shape behind
    /// `<lá em casa> [!]`. Each inner `BracketedItem::Word`
    /// consumes one `%mor` item (plus its post-clitic chain) from
    /// the caller-supplied cursors; the advance counts are
    /// returned so `emit_utterance` can thread the cursors forward.
    ///
    /// Staged increments: bracketed items beyond `Word` (Event,
    /// Pause, Separator, AnnotatedWord inside a group) fail loud.
    /// The reference corpus currently only exercises the
    /// `<word word word>` shape inside `AnnotatedGroup`.
    pub(crate) fn emit_annotated_group(
        &mut self,
        annotated: &talkbank_model::model::Annotated<talkbank_model::model::Group>,
        tiers: &UtteranceTiers<'_>,
        cursors: &mut TierCursors,
    ) -> Result<(), XmlWriteError> {
        let annotations = annotated.scoped_annotations.as_slice();
        if annotations.is_empty() {
            return Err(XmlWriteError::MissingMetadata {
                what: "annotated group reached emitter without scoped annotations".to_owned(),
            });
        }

        self.writer
            .write_event(Event::Start(BytesStart::new("g")))?;

        for item in annotated.inner.content.content.iter() {
            match item {
                BracketedItem::Word(word) => {
                    self.emit_aligned_word(word, tiers, cursors)?;
                }
                BracketedItem::AnnotatedGroup(inner) => {
                    // Nested `<…<inner words> [ann]…> [outer-ann]`
                    // recurses with the same cursor so mor alignment
                    // stays correct across the nesting.
                    self.emit_annotated_group(inner, tiers, cursors)?;
                }
                // Other bracketed items (separators, events,
                // actions, quotations, overlaps, underlines) delegate
                // to the generic per-item emitter. They don't consume
                // mor/gra chunks, so cursors don't advance here.
                other => self.emit_bracketed_word_only(other)?,
            }
        }

        for annotation in annotations {
            self.emit_scoped_annotation(annotation)?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        Ok(())
    }

    /// Emit one main-tier word that participates in `%mor` alignment
    /// per the `counts_for_tier` predicate. Words that don't count
    /// (untranscribed `xxx`, omissions, phonological fragments) still
    /// emit as `<w>` but without receiving a `%mor` child and without
    /// advancing the cursors. Shared between `emit_annotated_group`
    /// and `emit_bare_group` so both stay locked to the same predicate.
    pub(crate) fn emit_aligned_word(
        &mut self,
        word: &Word,
        tiers: &UtteranceTiers<'_>,
        cursors: &mut TierCursors,
    ) -> Result<(), XmlWriteError> {
        let counts = counts_for_tier(word, TierDomain::Mor);
        let mor_for_word = if counts {
            tiers
                .mor
                .as_ref()
                .and_then(|mor| mor.items().get(cursors.mor_index()))
        } else {
            None
        };
        self.emit_word(word, mor_for_word, tiers.gra, cursors.gra_chunk())?;
        if counts && tiers.mor.is_some() {
            let post_count = mor_for_word.map(|m| m.post_clitics.len()).unwrap_or(0);
            cursors.consume_mor(post_count);
        }
        Ok(())
    }

    /// Emit a single bracketed-content item inside a retrace or
    /// group. Bare words are the common case; richer bracketed
    /// content (events, pauses, annotated words, replacements,
    /// separators) is emitted with a simple recursive dispatch.
    /// Retrace content is emitted without `%mor` alignment, retraced
    /// text is excluded from `%mor` by CHAT convention.
    pub(crate) fn emit_bracketed_word_only(
        &mut self,
        item: &BracketedItem,
    ) -> Result<(), XmlWriteError> {
        match item {
            BracketedItem::Word(word) => self.emit_word(word, None, None, 0),
            BracketedItem::AnnotatedWord(annotated) => {
                self.emit_annotated_word(annotated, None, None, 0)
            }
            BracketedItem::ReplacedWord(rw) => {
                // Precondition: caller has already opened a `<g>`
                // parent (retrace wrapper or annotated group). Scoped
                // annotations emit as flat siblings of the `<w>`
                // inside that parent, TalkBank XML shape is
                // `<g><w>tika<replacement/>…</w><error/><k/></g>`.
                // Opening a nested `<g>` here would double-wrap.
                let mut outer = BytesStart::new("w");
                push_form_type_attrs(&mut outer, rw.word.form_type.as_ref());
                if let Some(cat) = &rw.word.category
                    && let Some(attr) = word_category_attr(cat)
                {
                    outer.push_attribute(("type", attr));
                }
                self.writer.write_event(Event::Start(outer))?;
                // Same reason as the top-level `emit_replaced_word`:
                // walk the structured content to preserve compound
                // markers and other word-internal structure on the
                // original side of `[: replacement]`.
                self.emit_word_contents(&rw.word)?;
                self.writer
                    .write_event(Event::Start(BytesStart::new("replacement")))?;
                for word in rw.replacement.words.0.iter() {
                    self.emit_word(word, None, None, 0)?;
                }
                self.writer
                    .write_event(Event::End(BytesEnd::new("replacement")))?;
                self.writer.write_event(Event::End(BytesEnd::new("w")))?;

                // Scoped annotations emit flat in the outer `<g>`.
                for annotation in rw.scoped_annotations.iter() {
                    self.emit_scoped_annotation(annotation)?;
                }
                Ok(())
            }
            BracketedItem::Event(event) => self.emit_event(event),
            BracketedItem::AnnotatedEvent(annotated) => self.emit_annotated_event(annotated),
            BracketedItem::Pause(pause) => self.emit_pause(pause),
            BracketedItem::Action(_) => {
                self.writer
                    .write_event(Event::Start(BytesStart::new("e")))?;
                self.writer
                    .write_event(Event::Empty(BytesStart::new("action")))?;
                self.writer.write_event(Event::End(BytesEnd::new("e")))?;
                Ok(())
            }
            BracketedItem::AnnotatedAction(annotated) => self.emit_annotated_action(annotated),
            BracketedItem::Quotation(quotation) => self.emit_quotation(quotation),
            BracketedItem::Freecode(freecode) => self.emit_freecode(freecode),
            BracketedItem::LongFeatureBegin(lf) => {
                self.emit_long_feature("begin", lf.label.as_str())
            }
            BracketedItem::LongFeatureEnd(lf) => self.emit_long_feature("end", lf.label.as_str()),
            BracketedItem::NonvocalBegin(nv) => self.emit_nonvocal("begin", nv.label.as_str()),
            BracketedItem::NonvocalEnd(nv) => self.emit_nonvocal("end", nv.label.as_str()),
            BracketedItem::NonvocalSimple(nv) => self.emit_nonvocal("simple", nv.label.as_str()),
            BracketedItem::Separator(sep) => self.emit_separator(sep, None, None, 0),
            BracketedItem::OverlapPoint(point) => self.emit_overlap_point(point),
            BracketedItem::InternalBullet(bullet) => self.emit_internal_media(bullet),
            BracketedItem::UnderlineBegin(_) => {
                let mut tag = BytesStart::new("underline");
                tag.push_attribute(("type", "begin"));
                self.writer.write_event(Event::Empty(tag))?;
                Ok(())
            }
            BracketedItem::UnderlineEnd(_) => {
                let mut tag = BytesStart::new("underline");
                tag.push_attribute(("type", "end"));
                self.writer.write_event(Event::Empty(tag))?;
                Ok(())
            }
            BracketedItem::OtherSpokenEvent(event) => self.emit_other_spoken_event(event),
            BracketedItem::Retrace(retrace) => {
                // Nested retrace inside a group/retrace: emit
                // without cursor threading (retrace content is
                // excluded from `%mor`).
                self.emit_retrace(retrace)
            }
            BracketedItem::AnnotatedGroup(inner) => {
                // Retrace / bracketed context disables mor alignment,
                // so synthesise empty tiers and emit the group using
                // the normal path. Cursor advances are discarded,
                // retraced content is excluded from `%mor` / `%gra`
                // by CHAT convention.
                let empty = UtteranceTiers {
                    mor: None,
                    gra: None,
                    wor: None,
                    sin: None,
                    side_tiers: Vec::new(),
                };
                let mut scratch = TierCursors::new();
                self.emit_annotated_group(inner, &empty, &mut scratch)
            }
            BracketedItem::PhoGroup(_) | BracketedItem::SinGroup(_) => {
                // Phon-specific payloads share the permanent
                // out-of-scope policy with `%pho` / `%mod`.
                Err(XmlWriteError::PhoneticTierUnsupported {
                    utterance_index: usize::MAX,
                })
            }
        }
    }

    /// Emit `<internal-media start="…" end="…" unit="s"/>` for a
    /// standalone bullet encountered inside a retrace or group.
    /// Shares the seconds formatting with the `%wor` emitter.
    pub(crate) fn emit_internal_media(
        &mut self,
        bullet: &talkbank_model::model::Bullet,
    ) -> Result<(), XmlWriteError> {
        let start = super::super::wor::format_seconds(bullet.timing.start_ms);
        let end = super::super::wor::format_seconds(bullet.timing.end_ms);
        let mut tag = BytesStart::new("internal-media");
        tag.push_attribute(("start", start.as_str()));
        tag.push_attribute(("end", end.as_str()));
        tag.push_attribute(("unit", "s"));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }

    /// Emit `<e><happening>text</happening>[annotation…]</e>` for an
    /// annotated event like `&=laughs [!]`. Annotations attach
    /// *inside* `<e>` per the XSD sequence.
    pub(crate) fn emit_annotated_event(
        &mut self,
        annotated: &Annotated<CEvent>,
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("e")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new("happening")))?;
        self.writer.write_event(Event::Text(escape_text(
            annotated.inner.event_type.as_str(),
        )))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("happening")))?;
        for annotation in annotated.scoped_annotations.iter() {
            self.emit_scoped_annotation(annotation)?;
        }
        self.writer.write_event(Event::End(BytesEnd::new("e")))?;
        Ok(())
    }

    /// Emit `<g>` for a bare (unannotated) `Group`. Each inner Word
    /// consumes one `%mor` chunk, returned as a cursor advance.
    pub(crate) fn emit_bare_group(
        &mut self,
        group: &talkbank_model::model::Group,
        tiers: &UtteranceTiers<'_>,
        cursors: &mut TierCursors,
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("g")))?;
        for item in group.content.content.iter() {
            if let BracketedItem::Word(word) = item {
                self.emit_aligned_word(word, tiers, cursors)?;
            } else {
                self.emit_bracketed_word_only(item)?;
            }
        }
        self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        Ok(())
    }

    /// Emit `<quotation type="begin"/>…<quotation type="end"/>` for
    /// a quotation span. The Rust model holds the whole quoted
    /// content as one `Quotation`; the XSD represents it as two
    /// standalone markers with content between them.
    pub(crate) fn emit_quotation(
        &mut self,
        quotation: &talkbank_model::model::Quotation,
    ) -> Result<(), XmlWriteError> {
        let mut begin = BytesStart::new("quotation");
        begin.push_attribute(("type", "begin"));
        self.writer.write_event(Event::Empty(begin))?;
        for item in quotation.content.content.iter() {
            self.emit_bracketed_word_only(item)?;
        }
        let mut end = BytesStart::new("quotation");
        end.push_attribute(("type", "end"));
        self.writer.write_event(Event::Empty(end))?;
        Ok(())
    }

    /// Emit `<freecode>text</freecode>`.
    pub(crate) fn emit_freecode(
        &mut self,
        freecode: &talkbank_model::model::Freecode,
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("freecode")))?;
        self.writer
            .write_event(Event::Text(escape_text(freecode.text.as_str())))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("freecode")))?;
        Ok(())
    }

    /// Emit `<long-feature type="begin|end">label</long-feature>`.
    pub(crate) fn emit_long_feature(
        &mut self,
        ty: &'static str,
        label: &str,
    ) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("long-feature");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Start(tag))?;
        self.writer.write_event(Event::Text(escape_text(label)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("long-feature")))?;
        Ok(())
    }

    /// Emit `<e><otherSpokenEvent who=".." said=".."/></e>` for a
    /// `&*SPEAKER:text` interposed-speaker event. The `<e>` wrapper
    /// is required by the XSD (it holds the `<action>` / `<happening>`
    /// / `<otherSpokenEvent>` choice).
    pub(crate) fn emit_other_spoken_event(
        &mut self,
        event: &talkbank_model::model::OtherSpokenEvent,
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("e")))?;
        let mut inner = BytesStart::new("otherSpokenEvent");
        inner.push_attribute(("who", event.speaker.as_str()));
        inner.push_attribute(("said", event.text.as_str()));
        self.writer.write_event(Event::Empty(inner))?;
        self.writer.write_event(Event::End(BytesEnd::new("e")))?;
        Ok(())
    }

    /// Emit `<nonvocal type="begin|end|simple">label</nonvocal>`.
    pub(crate) fn emit_nonvocal(
        &mut self,
        ty: &'static str,
        label: &str,
    ) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("nonvocal");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Start(tag))?;
        self.writer.write_event(Event::Text(escape_text(label)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("nonvocal")))?;
        Ok(())
    }
}
