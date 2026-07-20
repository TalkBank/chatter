//! `%mor` / `%gra` emission, morphology tier handling inside
//! `<w>`, `<tagMarker>`, and `<t>` elements.
//!
//! This submodule owns:
//! - The `<mor type="mor"><mw>…</mw>[<gra/>]</mor>` subtree emission
//!   attached to every main-tier chunk that has a paired `%mor` item.
//! - The `<mw><pos><c>…</c></pos><stem>…</stem><mk type="sfx">…</mk></mw>`
//!   serialization of a single [`MorWord`].
//! - The `<gra type="gra" index=… head=… relation=…/>` serialization
//!   of a single [`GrammaticalRelation`].
//! - The per-utterance tier collector [`UtteranceTiers`] + helper that
//!   separates `%mor` / `%gra` from other dependent tiers (the rest
//!   are staged increments).
//!
//! The `XmlEmitter` instance is owned by `super::writer`; this file
//! extends it via a separate `impl` block.

use quick_xml::events::{BytesEnd, BytesStart, Event};

use talkbank_model::model::WriteChat;
use talkbank_model::model::{DependentTier, GrammaticalRelation, Mor, MorWord, SinTier, WorTier};

use super::error::XmlWriteError;
use super::writer::{XmlEmitter, escape_text};

impl XmlEmitter {
    /// Emit the `<mor type="mor"><mw>...</mw>[<gra.../>]</mor>` subtree
    /// inside a `<w>` or `<tagMarker>`. Post-clitic Mor items and
    /// non-`Mor` MorTierType values are staged increments, fail loud
    /// on encounter so the golden harness points at the missing feature.
    pub(super) fn emit_word_mor_subtree(
        &mut self,
        mor: &Mor,
        gra: Option<&[GrammaticalRelation]>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        let mut mor_start = BytesStart::new("mor");
        mor_start.push_attribute(("type", "mor"));
        self.writer.write_event(Event::Start(mor_start))?;

        // Main morphological word + its %gra edge at the caller-
        // supplied chunk index.
        self.emit_morword(&mor.main)?;
        if let Some(rel) = gra_entry(gra, chunk_index_1based) {
            self.emit_gra(rel)?;
        }

        // Post-clitics (`~aux|be-Fin-...` chained onto the main mor
        // via `~`) each consume one additional `%gra` index.
        // `what's` with mor `pron|what-Int-S1~aux|be-Fin-Ind-Pres-S3`
        // emits the `<aux|be>` post-clitic inside `<mor-post>` with
        // index = main_index + 1.
        for (offset, post_clitic) in mor.post_clitics.iter().enumerate() {
            self.writer
                .write_event(Event::Start(BytesStart::new("mor-post")))?;
            self.emit_morword(post_clitic)?;
            let post_index = chunk_index_1based + offset + 1;
            if let Some(rel) = gra_entry(gra, post_index) {
                self.emit_gra(rel)?;
            }
            self.writer
                .write_event(Event::End(BytesEnd::new("mor-post")))?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("mor")))?;
        Ok(())
    }

    /// Serialize a single [`MorWord`] as `<mw><pos><c>POS</c><s>SUB</s>…</pos>
    /// <stem>LEMMA</stem>[<mk type="sfx">FEATURE</mk>…]</mw>`.
    fn emit_morword(&mut self, word: &MorWord) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("mw")))?;

        // <pos><c>n</c><s>prop</s>…</pos>. The TalkBank schema
        // (schema/xml/talkbank.xsd, element `pos`) defines `pos` as
        // one `<c>` category head followed by zero or more `<s>`
        // subcategory refinements (nonempty strings), so a
        // colon-refined tag like `n:let` must decompose. The model
        // stores it as one string; `PosCategory::category` /
        // `PosCategory::subcategories` own the split.
        self.writer
            .write_event(Event::Start(BytesStart::new("pos")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new("c")))?;
        self.writer
            .write_event(Event::Text(escape_text(word.pos.category())))?;
        self.writer.write_event(Event::End(BytesEnd::new("c")))?;
        for subcategory in word.pos.subcategories() {
            self.writer
                .write_event(Event::Start(BytesStart::new("s")))?;
            self.writer
                .write_event(Event::Text(escape_text(subcategory)))?;
            self.writer.write_event(Event::End(BytesEnd::new("s")))?;
        }
        self.writer.write_event(Event::End(BytesEnd::new("pos")))?;

        // <stem>lemma</stem>
        self.writer
            .write_event(Event::Start(BytesStart::new("stem")))?;
        self.writer
            .write_event(Event::Text(escape_text(word.lemma.as_str())))?;
        self.writer.write_event(Event::End(BytesEnd::new("stem")))?;

        // <mk type="sfx">Feature</mk> per flat feature. Keyed
        // features (key=value form) are a separate TDD increment.
        for feature in word.features.iter() {
            let text = format_mor_feature(feature)?;
            let mut mk = BytesStart::new("mk");
            mk.push_attribute(("type", "sfx"));
            self.writer.write_event(Event::Start(mk))?;
            self.writer.write_event(Event::Text(escape_text(&text)))?;
            self.writer.write_event(Event::End(BytesEnd::new("mk")))?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("mw")))?;
        Ok(())
    }

    /// Serialize a single [`GrammaticalRelation`] as a self-closing
    /// `<gra type="gra" index=… head=… relation=…/>`.
    pub(super) fn emit_gra(&mut self, rel: &GrammaticalRelation) -> Result<(), XmlWriteError> {
        let index_str = rel.index.to_string();
        let head_str = rel.head.to_string();
        let relation_str = rel.relation.as_str().to_owned();
        let mut gra = BytesStart::new("gra");
        gra.push_attribute(("type", "gra"));
        gra.push_attribute(("index", index_str.as_str()));
        gra.push_attribute(("head", head_str.as_str()));
        gra.push_attribute(("relation", relation_str.as_str()));
        self.writer.write_event(Event::Empty(gra))?;
        Ok(())
    }
}

/// Borrowed handles to the `%mor` and `%gra` tiers (if present) on
/// one utterance. Holding references means the emitter walks the
/// original AST in place, no cloning of tier payloads.
pub(super) struct UtteranceTiers<'u> {
    pub(super) mor: Option<&'u talkbank_model::model::MorTier>,
    pub(super) gra: Option<&'u [GrammaticalRelation]>,
    /// `%wor` word-timing sidecar. Captured here alongside `%mor` /
    /// `%gra` so `emit_utterance` can orchestrate element order
    /// (`<wor>` follows the terminator and the utterance-level
    /// `<media>`).
    pub(super) wor: Option<&'u WorTier>,
    /// `%sin` sign-language tier, aligned positionally to main-tier
    /// words. When present, each main-tier word emits wrapped in
    /// `<sg><w>...</w><sw>sin-value</sw></sg>` instead of a bare `<w>`
    ///, matches the TalkBank XML format's schema-driven shape (`<sg>` = sign
    /// group, `<sw>` = sign word). Words that count for TierDomain::Sin
    /// consume one sin item each.
    pub(super) sin: Option<&'u SinTier>,
    /// Text-content "side tiers" (`%act`, `%com`, `%exp`, `%gpx`,
    /// `%sit`, `%xLABEL`). Each becomes an `<a type="…">text</a>`
    /// element emitted after `<wor>` inside `<u>`. See
    /// [`super::deptier`].
    pub(super) side_tiers: Vec<&'u DependentTier>,
}

/// Running cursor state for main-tier content emission.
///
/// Each main-tier item consults the cursor set to find its paired
/// `%mor` / `%gra` / `%sin` partner and advances via the `consume_*`
/// methods, the advance rules (mor items count, post-clitics add to
/// gra, sin-counted words advance sin) live here, not at call sites.
///
/// Does NOT use `AlignmentSet::mor` / `AlignmentSet::sin` pre-computed
/// alignments: XML emission runs on both validated and unvalidated
/// `ChatFile<S>`, so `compute_alignments` may not have populated them.
/// The cursor walk here is equivalent to the model's alignment walk
/// for all well-formed inputs; it diverges only on malformed files
/// that the model's alignment would also flag.
pub(super) struct TierCursors {
    mor: usize,
    gra: usize,
    sin: usize,
}

impl TierCursors {
    pub fn new() -> Self {
        // `%gra` is 1-indexed per CHAT's `gra->target` attribute
        // convention; mor and sin items are 0-indexed.
        Self {
            mor: 0,
            gra: 1,
            sin: 0,
        }
    }

    /// `%mor` index currently pointing at the next item to consume.
    pub fn mor_index(&self) -> usize {
        self.mor
    }

    /// 1-based `%gra` chunk index for the next main `<mw>` (each
    /// `%mor` item with post-clitics contributes more than one chunk,
    /// so mor_index and gra_index diverge).
    pub fn gra_chunk(&self) -> usize {
        self.gra
    }

    /// `%sin` index currently pointing at the next item to consume.
    pub fn sin_index(&self) -> usize {
        self.sin
    }

    /// Advance mor/gra cursors after emitting an item that consumed
    /// one mor chunk with `post_clitics_len` post-clitic trailers.
    /// Call when the item actually consumed a `%mor` item (the caller
    /// already gated on `counts_for_tier(TierDomain::Mor)` or the
    /// separator's tag-marker predicate).
    pub fn consume_mor(&mut self, post_clitics_len: usize) {
        self.mor += 1;
        self.gra += 1 + post_clitics_len;
    }

    /// Advance sin cursor after emitting an item that consumed one
    /// `%sin` item. Call only when `counts_for_tier(TierDomain::Sin)`
    /// returned true for the word AND a `%sin` tier is present.
    pub fn consume_sin(&mut self) {
        self.sin += 1;
    }
}

impl Default for TierCursors {
    fn default() -> Self {
        Self::new()
    }
}

/// Split an utterance's dependent tiers into recognized `%mor` /
/// `%gra` / `%wor` / text-content slots. Phonetic / syllabification
/// tiers (`%pho`, `%mod`, `%phosyl`, `%modsyl`, `%phoaln`) report
/// [`XmlWriteError::PhoneticTierUnsupported`]; these are
/// permanently out of scope, not staged. Any other tier kind reports
/// `FeatureNotImplemented` so the harness surfaces each missing
/// staged feature individually.
pub(super) fn collect_utterance_tiers(
    utterance: &talkbank_model::model::Utterance,
    utterance_index: usize,
) -> Result<UtteranceTiers<'_>, XmlWriteError> {
    let mut out = UtteranceTiers {
        mor: None,
        gra: None,
        wor: None,
        sin: None,
        side_tiers: Vec::new(),
    };
    for entry in utterance.dependent_tiers.iter() {
        let tier = &entry.tier;
        match tier {
            // Text-content tiers with a known TalkBank XML shape
            // (`<a type="…">`) collect into `side_tiers`; emission
            // happens in `super::deptier::emit_side_tiers` after the
            // main-tier block.
            DependentTier::Act(_)
            | DependentTier::Add(_)
            | DependentTier::Cod(_)
            | DependentTier::Com(_)
            | DependentTier::Exp(_)
            | DependentTier::Gpx(_)
            | DependentTier::Int(_)
            | DependentTier::Sit(_)
            | DependentTier::Spa(_)
            | DependentTier::Alt(_)
            | DependentTier::Coh(_)
            | DependentTier::Def(_)
            | DependentTier::Eng(_)
            | DependentTier::Err(_)
            | DependentTier::Fac(_)
            | DependentTier::Flo(_)
            | DependentTier::Gls(_)
            | DependentTier::Ort(_)
            | DependentTier::Par(_)
            | DependentTier::Tim(_)
            | DependentTier::UserDefined(_)
            | DependentTier::Unsupported(_) => {
                out.side_tiers.push(tier);
                continue;
            }
            DependentTier::Mor(m) => {
                if out.mor.is_some() {
                    return Err(XmlWriteError::MultipleStructuredTiers {
                        utterance_index,
                        tier: "%mor",
                    });
                }
                out.mor = Some(m);
            }
            DependentTier::Gra(g) => {
                if out.gra.is_some() {
                    return Err(XmlWriteError::MultipleStructuredTiers {
                        utterance_index,
                        tier: "%gra",
                    });
                }
                out.gra = Some(g.relations());
            }
            DependentTier::Wor(w) => {
                if out.wor.is_some() {
                    return Err(XmlWriteError::MultipleStructuredTiers {
                        utterance_index,
                        tier: "%wor",
                    });
                }
                out.wor = Some(w);
            }
            DependentTier::Pho(_)
            | DependentTier::Mod(_)
            | DependentTier::Modsyl(_)
            | DependentTier::Phosyl(_)
            | DependentTier::Phoaln(_)
            | DependentTier::Xphoint(_) => {
                // Phon-specific phonetic / syllabification tiers are
                // permanently out of scope, see
                // `XmlWriteError::PhoneticTierUnsupported`.
                return Err(XmlWriteError::PhoneticTierUnsupported { utterance_index });
            }
            DependentTier::Sin(sin) => {
                // Structured `<sg><w>...</w><sw>value</sw></sg>` per
                // main-tier word. Emission happens inline in
                // `emit_utterance` via `tiers.sin`.
                out.sin = Some(sin);
                continue;
            }
        }
    }
    Ok(out)
}

/// Look up the grammatical-relation edge for a 1-based chunk index.
/// Returns `None` when `%gra` is absent or the chunk is unaligned; the
/// caller omits the `<gra/>` child in that case.
pub(super) fn gra_entry(
    gra: Option<&[GrammaticalRelation]>,
    chunk_index_1based: usize,
) -> Option<&GrammaticalRelation> {
    let relations = gra?;
    relations.iter().find(|r| r.index == chunk_index_1based)
}

/// Serialize one [`talkbank_model::model::MorFeature`] back to its
/// compact CHAT token (`Fin`, `key=value`, …). This is the content of
/// the `<mk type="sfx">` element. Uses the feature's own `WriteChat`
/// impl so the XML emitter stays in lockstep with the rest of the
/// toolchain's CHAT serialization.
fn format_mor_feature(
    feature: &talkbank_model::model::MorFeature,
) -> Result<String, XmlWriteError> {
    let mut buf = String::new();
    feature
        .write_chat(&mut buf)
        .map_err(|e| XmlWriteError::MissingMetadata {
            what: format!("failed to format MorFeature: {e}"),
        })?;
    Ok(buf)
}

/// Short display name for a [`DependentTier`] variant. Used only in
/// `FeatureNotImplemented` diagnostics, so we keep it as a flat match
/// rather than reaching into each tier's label newtype. Shared with
/// [`super::deptier`].
pub(super) fn tier_kind(tier: &DependentTier) -> &'static str {
    match tier {
        DependentTier::Mor(_) => "%mor",
        DependentTier::Gra(_) => "%gra",
        DependentTier::Wor(_) => "%wor",
        DependentTier::Pho(_) => "%pho",
        DependentTier::Mod(_) => "%mod",
        DependentTier::Sin(_) => "%sin",
        DependentTier::Act(_) => "%act",
        DependentTier::Cod(_) => "%cod",
        DependentTier::Add(_) => "%add",
        DependentTier::Com(_) => "%com",
        DependentTier::Exp(_) => "%exp",
        DependentTier::Gpx(_) => "%gpx",
        DependentTier::Int(_) => "%int",
        DependentTier::Sit(_) => "%sit",
        DependentTier::Spa(_) => "%spa",
        DependentTier::Alt(_) => "%alt",
        DependentTier::Coh(_) => "%coh",
        DependentTier::Def(_) => "%def",
        DependentTier::Eng(_) => "%eng",
        DependentTier::Err(_) => "%err",
        DependentTier::Fac(_) => "%fac",
        DependentTier::Flo(_) => "%flo",
        DependentTier::Modsyl(_) => "%modsyl",
        DependentTier::Phosyl(_) => "%phosyl",
        DependentTier::Phoaln(_) => "%phoaln",
        DependentTier::Xphoint(_) => "%xphoint",
        DependentTier::UserDefined(_) => "%xLABEL",
        DependentTier::Unsupported(_) => "(unsupported tier)",
        _ => "(other dependent tier)",
    }
}

#[cfg(test)]
mod tests {
    use talkbank_model::ParseValidateOptions;

    use super::super::writer::write_chat_xml;

    /// Parse a small CHAT source, emit XML, and collapse the pretty
    /// printer's inter-element whitespace (trim each line, join with
    /// no separator). Containment assertions then check element
    /// adjacency the way the structural golden comparator and
    /// external XML consumers observe it, independent of indentation.
    fn emit_normalized_xml(src: &str) -> String {
        let file =
            crate::parse_and_validate(src, ParseValidateOptions::default().with_validation())
                .expect("inline %mor fixture must parse");
        let xml = write_chat_xml(&file).expect("inline %mor fixture must emit XML");
        xml.lines().map(str::trim).collect()
    }

    /// A subcategorized POS tag (`n:let|shi`) decomposes into a `<c>`
    /// category head plus one `<s>` subcategory, per the TalkBank
    /// schema's `posType` (one `<c>`, zero or more `<s>`). Regression
    /// guard for the bug where the whole `n:let` landed inside `<c>`.
    #[test]
    fn subcategorized_pos_emits_c_plus_s() {
        let src = "@UTF8\n@Begin\n@Languages:\tjpn\n\
            @Participants:\tCHI Child\n\
            @ID:\tjpn|corpus|CHI|||||Child|||\n\
            *CHI:\tshi .\n\
            %mor:\tn:let|shi .\n@End\n";
        let xml = emit_normalized_xml(src);
        assert!(
            xml.contains("<pos><c>n</c><s>let</s></pos>"),
            "expected <c>n</c><s>let</s> decomposition, got:\n{xml}"
        );
    }

    /// Nested subcategories (`pro:poss:det|your`) emit one `<s>` per
    /// refinement segment, in order.
    #[test]
    fn nested_subcategories_emit_multiple_s() {
        let src = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|corpus|CHI|||||Child|||\n\
            *CHI:\tyour .\n\
            %mor:\tpro:poss:det|your .\n@End\n";
        let xml = emit_normalized_xml(src);
        assert!(
            xml.contains("<pos><c>pro</c><s>poss</s><s>det</s></pos>"),
            "expected <c>pro</c><s>poss</s><s>det</s> decomposition, got:\n{xml}"
        );
    }

    /// An unrefined POS tag keeps the bare `<c>`-only shape: no empty
    /// `<s>` may appear (the schema requires `<s>` content nonempty).
    #[test]
    fn unrefined_pos_emits_c_only() {
        let src = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|corpus|CHI|||||Child|||\n\
            *CHI:\tdog .\n\
            %mor:\tn|dog .\n@End\n";
        let xml = emit_normalized_xml(src);
        assert!(
            xml.contains("<pos><c>n</c></pos>"),
            "expected bare <c>n</c> with no <s>, got:\n{xml}"
        );
        assert!(
            !xml.contains("<s>"),
            "no <s> element may appear for an unrefined POS tag:\n{xml}"
        );
    }
}
