//! Utterance-level XML emission, extracted from the document module.
//!
//! Owns `emit_utterance` (the main-tier orchestration that threads the
//! `%mor`/`%gra`/`%sin`/`%wor` cursors) and its `emit_sin_word` helper.

use quick_xml::events::{BytesEnd, BytesStart, Event};

use talkbank_model::alignment::TierDomain;
use talkbank_model::alignment::helpers::{counts_for_tier, is_tag_marker_separator};
use talkbank_model::model::{SinItem, UtteranceContent};

use super::super::error::XmlWriteError;
use super::super::mor::{TierCursors, collect_utterance_tiers};
use super::super::word::separator_terminator_type_attr;
use super::super::writer::{XmlEmitter, escape_text};

impl XmlEmitter {
    /// Emit a single `<u who=… uID=…>…<t/>…</u>` utterance. This is
    /// the orchestration point that walks main-tier content in parallel
    /// with the `%mor` cursor, dispatching each content item to the
    /// appropriate word-level emitter. The terminator closes the
    /// utterance and, if `%mor` carries a terminator chunk, picks up
    /// the matching `<gra/>` from chunk index `n+1`.
    pub(super) fn emit_utterance(
        &mut self,
        utterance: &talkbank_model::model::Utterance,
    ) -> Result<(), XmlWriteError> {
        // Pre-begin headers attached to this utterance.
        for header in utterance.preceding_headers.iter() {
            self.emit_header_if_body(header)?;
        }

        // Split dependent tiers into recognized Mor / Gra / other.
        // Phonetic / syllabification tiers (%pho, %mod, %phosyl,
        // %modsyl, %phoaln) are permanently unsupported, see
        // `XmlWriteError::PhoneticTierUnsupported`. Other staged
        // tiers surface via `FeatureNotImplemented` one at a time.
        let tiers = collect_utterance_tiers(utterance, self.next_utterance_id as usize)?;

        let uid = format!("u{}", self.next_utterance_id);
        self.next_utterance_id += 1;

        let mut start = BytesStart::new("u");
        start.push_attribute(("who", utterance.main.speaker.as_str()));
        start.push_attribute(("uID", uid.as_str()));
        // `[- LANG]` pre-code promotes the utterance's baseline
        // language to a tier-scoped override; TalkBank XML projects
        // that onto `<u xml:lang="LANG">`. The grammar populates
        // `main.content.language_code` directly when it parses the
        // pre-code, so we read it here rather than going through
        // the computed `utterance_language` state, the latter is
        // only populated when the caller invokes
        // `compute_language_metadata` (e.g. during the alignment
        // pipeline), but XML emission runs on the bare parse too.
        if let Some(code) = utterance.main.content.language_code.as_ref() {
            start.push_attribute(("xml:lang", code.as_str()));
        }
        self.writer.write_event(Event::Start(start))?;

        // Discourse linkers (`+<`, `++`, `+≈`, `+≋`, …) sit at the
        // very start of tier content and render as `<linker
        // type="…"/>` children of `<u>` ahead of any `<w>` content.
        for linker in utterance.main.content.linkers.0.iter() {
            self.emit_linker(linker)?;
        }

        // Per-content-arm logic reads cursors and calls
        // `cursors.consume_*` to advance. Advance rules live on
        // `TierCursors` (see its rustdoc).
        let mut cursors = TierCursors::new();
        let content_items = &utterance.main.content.content;
        let last_content_index = content_items.len().checked_sub(1);
        for (item_index, item) in content_items.iter().enumerate() {
            match item {
                UtteranceContent::Word(word) => {
                    // Leading overlap markers (`⌈`, `⌊`) attached to
                    // the front of a word get hoisted out as
                    // top-level `<overlap-point/>` siblings before
                    // the `<w>`. The Rust parser bundles them into
                    // `word.content`; TalkBank XML emits them
                    // outside. Peel them here so the word body starts
                    // with its actual first lexical segment.
                    self.emit_leading_overlap_points(word)?;

                    // Nonword (`&~`), filler (`&-`), phonological
                    // fragment (`&+`), and untranscribed (`xxx` /
                    // `yyy` / `www`) tokens appear on the main tier
                    // but have no corresponding `%mor` item, so we
                    // pass `None` for `mor` and keep the cursor
                    // where it is. Using the model's canonical
                    // `counts_for_tier(TierDomain::Mor)` predicate
                    // keeps this check aligned with validation logic.
                    let counts_mor = counts_for_tier(word, TierDomain::Mor);
                    let mor_for_word = if counts_mor {
                        tiers
                            .mor
                            .as_ref()
                            .and_then(|mor| mor.items().get(cursors.mor_index()))
                    } else {
                        None
                    };

                    // `%sin` attaches one sign-word per main-tier
                    // word that counts for `TierDomain::Sin`, wrapping
                    // the whole pair in `<sg><w>...</w><sw>sin</sw></sg>`
                    // per the TalkBank XML schema. `%sin` includes more
                    // token kinds than `%mor` (fragments, untranscribed
                    // all participate), so the gate is separate.
                    let counts_sin = tiers.sin.is_some() && counts_for_tier(word, TierDomain::Sin);
                    let sin_item = if counts_sin {
                        tiers
                            .sin
                            .as_ref()
                            .and_then(|sin| sin.items.0.get(cursors.sin_index()))
                    } else {
                        None
                    };

                    if sin_item.is_some() {
                        self.writer
                            .write_event(Event::Start(BytesStart::new("sg")))?;
                    }
                    self.emit_word(word, mor_for_word, tiers.gra, cursors.gra_chunk())?;
                    if let Some(item) = sin_item {
                        self.emit_sin_word(item)?;
                        self.writer.write_event(Event::End(BytesEnd::new("sg")))?;
                    }

                    if counts_mor && tiers.mor.is_some() {
                        let post_count = mor_for_word.map(|m| m.post_clitics.len()).unwrap_or(0);
                        cursors.consume_mor(post_count);
                    }
                    if counts_sin {
                        cursors.consume_sin();
                    }
                }
                UtteranceContent::Separator(sep) => {
                    if utterance.main.content.terminator.is_none()
                        && Some(item_index) == last_content_index
                        && separator_terminator_type_attr(sep).is_some()
                    {
                        continue;
                    }

                    // Only Comma / Tag / Vocative separators
                    // participate in `%mor` alignment (they produce
                    // `cm|cm`, `end|end`, `beg|beg` mor items). CA
                    // intonation markers and other structural
                    // separators render to `<s>` / `<tagMarker>`
                    // without consuming mor chunks.
                    let counts_for_mor = is_tag_marker_separator(sep);
                    let mor_for_sep = if counts_for_mor {
                        tiers
                            .mor
                            .as_ref()
                            .and_then(|mor| mor.items().get(cursors.mor_index()))
                    } else {
                        None
                    };
                    self.emit_separator(sep, mor_for_sep, tiers.gra, cursors.gra_chunk())?;
                    if counts_for_mor && tiers.mor.is_some() {
                        let post_count = mor_for_sep.map(|m| m.post_clitics.len()).unwrap_or(0);
                        cursors.consume_mor(post_count);
                    }
                }
                UtteranceContent::Pause(pause) => {
                    self.emit_pause(pause)?;
                }
                UtteranceContent::Retrace(retrace) => {
                    self.emit_retrace(retrace)?;
                }
                UtteranceContent::AnnotatedWord(annotated) => {
                    let mor_for_chunk = tiers
                        .mor
                        .and_then(|mor| mor.items().get(cursors.mor_index()));
                    self.emit_annotated_word(
                        annotated,
                        mor_for_chunk,
                        tiers.gra,
                        cursors.gra_chunk(),
                    )?;
                    if tiers.mor.is_some() {
                        let post_count = mor_for_chunk.map(|m| m.post_clitics.len()).unwrap_or(0);
                        cursors.consume_mor(post_count);
                    }
                }
                UtteranceContent::ReplacedWord(rw) => {
                    // `emit_replaced_word` consumes N mor items +
                    // their post-clitic `%gra` edges internally via
                    // the shared `cursors`.
                    self.emit_replaced_word(rw, &tiers, &mut cursors)?;
                }
                UtteranceContent::Event(event) => {
                    // Inline `&=descriptor` event in main-tier content
                    // (e.g. `&=laughs`). Events don't consume mor/gra
                    // chunks; they're outside the word alignment.
                    self.emit_event(event)?;
                }
                UtteranceContent::AnnotatedAction(annotated) => {
                    // Bare main-tier action (`0 .` utterance or `0`
                    // token), scoped annotations on the action are a
                    // separate increment; the bare `<e><action/></e>`
                    // shape is all the reference corpus uses.
                    self.emit_annotated_action(annotated)?;
                }
                UtteranceContent::OverlapPoint(point) => {
                    // Top-level overlap markers (`⌈` / `⌉` / `⌊` /
                    // `⌋` appearing outside a word) render as
                    // `<overlap-point/>` children of `<u>`.
                    self.emit_overlap_point(point)?;
                }
                UtteranceContent::AnnotatedGroup(annotated) => {
                    // `<word1 word2> [annotation]` →
                    // `<g><w>word1</w><w>word2</w><annotation/></g>`.
                    // `emit_annotated_group` advances cursors inline.
                    self.emit_annotated_group(annotated, &tiers, &mut cursors)?;
                }
                UtteranceContent::AnnotatedEvent(annotated) => {
                    // `&=descriptor [!]` → `<e><happening>text</happening>
                    // <k type="stressing"/></e>`. Annotations on an
                    // event attach *inside* `<e>` rather than wrapping
                    // it in `<g>`, per the XSD `<e>` choice sequence.
                    self.emit_annotated_event(annotated)?;
                }
                UtteranceContent::Group(group) => {
                    // `<word word>` without scoped annotations, same
                    // shape as `AnnotatedGroup` minus the sibling
                    // annotations. Cursors advance inside.
                    self.emit_bare_group(group, &tiers, &mut cursors)?;
                }
                UtteranceContent::Quotation(quotation) => {
                    self.emit_quotation(quotation)?;
                }
                UtteranceContent::Freecode(freecode) => {
                    self.emit_freecode(freecode)?;
                }
                UtteranceContent::LongFeatureBegin(lf) => {
                    self.emit_long_feature("begin", lf.label.as_str())?;
                }
                UtteranceContent::LongFeatureEnd(lf) => {
                    self.emit_long_feature("end", lf.label.as_str())?;
                }
                UtteranceContent::NonvocalBegin(nv) => {
                    self.emit_nonvocal("begin", nv.label.as_str())?;
                }
                UtteranceContent::NonvocalEnd(nv) => {
                    self.emit_nonvocal("end", nv.label.as_str())?;
                }
                UtteranceContent::NonvocalSimple(nv) => {
                    self.emit_nonvocal("simple", nv.label.as_str())?;
                }
                UtteranceContent::UnderlineBegin(_) => {
                    let mut tag = quick_xml::events::BytesStart::new("underline");
                    tag.push_attribute(("type", "begin"));
                    self.writer
                        .write_event(quick_xml::events::Event::Empty(tag))?;
                }
                UtteranceContent::UnderlineEnd(_) => {
                    let mut tag = quick_xml::events::BytesStart::new("underline");
                    tag.push_attribute(("type", "end"));
                    self.writer
                        .write_event(quick_xml::events::Event::Empty(tag))?;
                }
                UtteranceContent::InternalBullet(bullet) => {
                    // Standalone bullet inside main content (rare;
                    // usually bullets attach to a word), emit as
                    // `<internal-media>` using the same seconds
                    // formatting as `%wor` bullets.
                    self.emit_internal_media(bullet)?;
                }
                UtteranceContent::OtherSpokenEvent(event) => {
                    // `&*WHO=word` interposed-speaker marker. Per the
                    // XSD, `<otherSpokenEvent>` nests inside `<e>`
                    // alongside `<action>` and `<happening>`.
                    self.emit_other_spoken_event(event)?;
                }
                UtteranceContent::PhoGroup(_) | UtteranceContent::SinGroup(_) => {
                    // `<pg>` / `<sg>` are Phon-specific structured
                    // payloads. Permanently out of scope (same
                    // policy as `%pho` / `%mod` tiers), surface as
                    // `PhoneticTierUnsupported` at the utterance
                    // level rather than an open-ended
                    // `FeatureNotImplemented`.
                    return Err(XmlWriteError::PhoneticTierUnsupported {
                        utterance_index: self.next_utterance_id.saturating_sub(1) as usize,
                    });
                }
            }
        }

        // %mor always emits exactly one extra item (the terminator
        // chunk); we feed that index to the terminator emission so
        // its `<mor>` subtree picks up the matching `<gra>`.
        match utterance.main.content.terminator.as_ref() {
            Some(terminator) => {
                self.emit_terminator(terminator, &tiers, cursors.gra_chunk())?;
            }
            None => {
                let mut tag = quick_xml::events::BytesStart::new("t");
                let type_attr = utterance
                    .main
                    .content
                    .content
                    .last()
                    .and_then(|item| match item {
                        UtteranceContent::Separator(sep) => separator_terminator_type_attr(sep),
                        _ => None,
                    })
                    .unwrap_or("missing CA terminator");
                tag.push_attribute(("type", type_attr));
                self.writer
                    .write_event(quick_xml::events::Event::Empty(tag))?;
            }
        }

        // Utterance-level `<media>` element: the main tier's trailing
        // bullet (`· start_end ·` after the terminator) becomes a
        // `<media start="s.sss" end="s.sss" unit="s"/>` sibling of
        // `<t>` in the TalkBank XML output. Emission order is:
        // main-tier words → `<t>` → `<media>` → `<wor>`.
        if let Some(bullet) = utterance.main.content.bullet.as_ref() {
            self.emit_utterance_media(bullet)?;
        }

        // `[+ code]` postcodes, one `<postcode>` element per code
        // in source order. TalkBank XML emits these directly after
        // `<t/>` / `<media/>` and before `<wor>` / dependent-tier
        // annotations. The model stores them on
        // `main.content.postcodes`, separate from inline content.
        for postcode in utterance.main.content.postcodes.iter() {
            self.writer
                .write_event(Event::Start(BytesStart::new("postcode")))?;
            self.writer
                .write_event(Event::Text(escape_text(postcode.text.as_str())))?;
            self.writer
                .write_event(Event::End(BytesEnd::new("postcode")))?;
        }

        // `<wor>`, the word-level timing sidecar. Emitted only when
        // the utterance carried a `%wor` tier.
        if let Some(wor) = tiers.wor {
            self.emit_wor(wor)?;
        }

        // Text-content "side tiers" (`%act`, `%com`, `%exp`, `%gpx`,
        // `%sit`, `%xLABEL`) become `<a type="…">text</a>` children
        // of `<u>` per the TalkBank XML shape.
        if !tiers.side_tiers.is_empty() {
            self.emit_side_tiers(&tiers.side_tiers)?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("u")))?;
        Ok(())
    }

    /// Emit one `%sin` item as a `<sw>…</sw>` child of the surrounding
    /// `<sg>` group. Tokens render as their raw text (including the
    /// `0` "no-gesture" sentinel, which round-trips as `<sw>0</sw>` per
    /// the TalkBank XML golden output). `SinGroup(…)`, multi-gesture
    /// items enclosed in `〔…〕` on CHAT, renders its joined gesture
    /// text; richer structured emission for sin-groups would go here
    /// later if the XSD requires it.
    fn emit_sin_word(&mut self, item: &SinItem) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("sw")))?;
        match item {
            SinItem::Token(token) => {
                self.writer
                    .write_event(Event::Text(escape_text(token.as_ref())))?;
            }
            SinItem::SinGroup(gestures) => {
                // Flatten `〔g1 g2〕` as space-separated for `<sw>`.
                let mut buf = String::new();
                for (i, gesture) in gestures.0.iter().enumerate() {
                    if i > 0 {
                        buf.push(' ');
                    }
                    buf.push_str(gesture.as_ref());
                }
                self.writer.write_event(Event::Text(escape_text(&buf)))?;
            }
        }
        self.writer.write_event(Event::End(BytesEnd::new("sw")))?;
        Ok(())
    }
}
