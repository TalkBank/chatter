//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use crate::ast;
use crate::token::Token;
use talkbank_model::Span;
use talkbank_model::model::*;

use super::*;

pub fn main_tier_to_model(mt: &ast::MainTier<'_>) -> MainTier {
    let speaker = SpeakerCode::new(mt.speaker.text());
    let content_items: Vec<UtteranceContent> = mt
        .tier_body
        .contents
        .iter()
        .map(|c| content_item_to_model(c))
        .collect();
    let terminator = mt
        .tier_body
        .terminator
        .as_ref()
        .map(|t| token_to_terminator(t));

    let mut main_tier = MainTier::new(speaker, content_items, terminator);

    // Extract a terminal bullet that the greedy contents parser left in content.
    main_tier.content.extract_terminal_bullet();

    // Grammar-routed bullet from tier_body.media_bullet takes priority
    // over the extracted one (it's correctly classified by the chumsky parser).
    if let Some(bullet_tok) = &mt.tier_body.media_bullet
        && let Token::MediaBullet {
            start_time,
            end_time,
            ..
        } = bullet_tok
    {
        let start_ms: u64 = start_time.parse().unwrap_or(0);
        let end_ms: u64 = end_time.parse().unwrap_or(0);
        main_tier = main_tier.with_bullet(Bullet::new(start_ms, end_ms));
    }

    // Linkers
    if !mt.tier_body.linkers.is_empty() {
        let linkers: Vec<Linker> = mt
            .tier_body
            .linkers
            .iter()
            .filter_map(|tok| linker_token_to_model(tok))
            .collect();
        main_tier = main_tier.with_linkers(linkers);
    }

    // Language code ([- lang])
    if let Some(ref langcode_tok) = mt.tier_body.langcode {
        // Token carries tag-extracted language code directly (e.g., "zho")
        let code = langcode_tok.text();
        if !code.is_empty() {
            main_tier = main_tier
                .with_language_code(LanguageCode::new(code).expect("checked non-empty above"));
        }
    }

    // Postcodes
    if !mt.tier_body.postcodes.is_empty() {
        let postcodes: Vec<Postcode> = mt
            .tier_body
            .postcodes
            .iter()
            .map(|tok| {
                // Token carries tag-extracted postcode content directly
                Postcode::new(tok.text())
            })
            .collect();
        main_tier = main_tier.with_postcodes(postcodes);
    }

    main_tier
}

// ═══════════════════════════════════════════════════════════════
// Utterance conversion
// ═══════════════════════════════════════════════════════════════

pub fn utterance_to_model(u: &ast::Utterance<'_>) -> talkbank_model::model::Utterance {
    let main = main_tier_to_model(&u.main_tier);
    // Skip tiers whose AST→model conversion failed (e.g. a `%mor:`
    // line with a missing or unrecognized terminator). Cross-tier
    // validators surface the absence as a typed diagnostic.
    let dep_tiers: Vec<talkbank_model::model::DependentTier> = u
        .dependent_tiers
        .iter()
        .filter_map(dependent_tier_to_model)
        .collect();
    talkbank_model::model::Utterance {
        preceding_headers: Default::default(),
        main,
        // re2c does not yet parse E758 separator provenance (Task 3 gives
        // every dependent tier a `DependentTierEntry`; the separator itself
        // stays a follow-up for the re2c oracle, tracked with the rest of
        // the E758 CA-gated rewrite). CLEAN is correct today: re2c reports
        // no illegal trailing space for any dependent tier.
        dependent_tiers: dep_tiers.into_iter().map(DependentTierEntry::new).collect(),
        alignments: None,
        alignment_diagnostics: Vec::new(),
        // re2c's lexer never fails and this runs on a fully-parsed AST utterance;
        // individual unconvertible tiers are dropped above and surfaced by the
        // cross-tier validators. Establish Clean provenance so the alignment
        // checks actually run (an Unknown default makes every cross-tier check
        // skip with an E600 "provenance unknown" warning, leaving re2c with a far
        // weaker validation surface than the tree-sitter parser).
        parse_health: talkbank_model::model::ParseHealthState::Clean,
        utterance_language: Default::default(),
        language_metadata: Default::default(),
    }
}

/// Convert a parsed dependent tier to model `DependentTier`.
///
/// Returns `None` when the AST→model conversion fails for that tier
/// (currently `%mor:` with a missing or unrecognized terminator).
/// Cross-tier validators surface the resulting absence; this layer
/// just declines to construct a `MorTier` from malformed input.
pub fn dependent_tier_to_model(
    tier: &ast::DependentTierParsed<'_>,
) -> Option<talkbank_model::model::DependentTier> {
    Some(match tier {
        ast::DependentTierParsed::Mor(mor) => {
            talkbank_model::model::DependentTier::Mor(MorTier::try_from(mor).ok()?)
        }
        ast::DependentTierParsed::Gra(gra) => {
            talkbank_model::model::DependentTier::Gra(GraTier::from(gra))
        }
        ast::DependentTierParsed::Pho(pho) => {
            talkbank_model::model::DependentTier::Pho(convert_pho_tier(
                pho,
                talkbank_model::model::dependent_tier::pho::PhoTierType::Pho,
            ))
        }
        ast::DependentTierParsed::Mod(pho) => {
            talkbank_model::model::DependentTier::Mod(convert_pho_tier(
                pho,
                talkbank_model::model::dependent_tier::pho::PhoTierType::Mod,
            ))
        }
        ast::DependentTierParsed::Sin(sin) => {
            talkbank_model::model::DependentTier::Sin(convert_sin_tier(sin))
        }
        ast::DependentTierParsed::Wor { items, terminator } => {
            use talkbank_model::model::dependent_tier::wor::WorItem;
            let wor_items: Vec<WorItem> = items
                .iter()
                .map(|item| match item {
                    ast::WorItemParsed::Word { word, bullet } => {
                        let mut w = word_from_parsed(word);
                        if let Some((start_ms, end_ms)) = bullet {
                            w = w.with_inline_bullet(Bullet::new(*start_ms, *end_ms));
                        }
                        WorItem::Word(Box::new(w))
                    }
                    ast::WorItemParsed::Separator(tok) => WorItem::Separator {
                        text: tok.text().to_string(),
                        span: Span::DUMMY,
                    },
                })
                .collect();
            let mut wor = WorTier::new(wor_items);
            if let Some(t) = terminator {
                wor.terminator = Some(token_to_terminator(t));
            }
            talkbank_model::model::DependentTier::Wor(wor)
        }
        ast::DependentTierParsed::Text { prefix, content } => {
            let bc = tokens_to_bullet_content(content);
            let prefix_text = prefix.text();
            // Extract tier label: "%com:\t" → "com", "%xpho:\t" → "xpho"
            let label = prefix_text.trim_start_matches('%').trim_end_matches(":\t");

            // Phon project tiers have x-prefix but are NOT user-defined
            let is_phon_tier = matches!(label, "xmodsyl" | "xphosyl" | "xphoaln" | "xphoint");

            // User-defined tiers: %x* prefix (but not phon project tiers)
            if label.starts_with('x') && label.len() >= 2 && !is_phon_tier {
                let raw_text: String = content.iter().map(|t| t.text()).collect();
                return Some(talkbank_model::model::DependentTier::UserDefined(
                    talkbank_model::model::UserDefinedDependentTier {
                        label: NonEmptyString::new(label)
                            .unwrap_or_else(|| NonEmptyString::new("x").unwrap()),
                        content: NonEmptyString::new(raw_text.as_str())
                            .unwrap_or_else(|| NonEmptyString::new(" ").unwrap()),
                        span: Span::DUMMY,
                    },
                ));
            }

            // BulletContent tiers
            match label {
                "com" => talkbank_model::model::DependentTier::Com(ComTier::new(bc)),
                "act" => talkbank_model::model::DependentTier::Act(ActTier::new(bc)),
                "exp" => talkbank_model::model::DependentTier::Exp(ExpTier::new(bc)),
                "add" => talkbank_model::model::DependentTier::Add(AddTier::new(bc)),
                "gpx" => talkbank_model::model::DependentTier::Gpx(GpxTier::new(bc)),
                "int" => talkbank_model::model::DependentTier::Int(IntTier::new(bc)),
                "spa" => talkbank_model::model::DependentTier::Spa(SpaTier::new(bc)),
                "sit" => talkbank_model::model::DependentTier::Sit(SitTier::new(bc)),
                "cod" => talkbank_model::model::DependentTier::Cod(CodTier::new(bc)),
                // TextTier tiers (plain string content)
                "alt" | "coh" | "def" | "eng" | "err" | "fac" | "flo" | "gls" | "ort" | "par" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let text = NonEmptyString::new(raw_text.as_str())
                        .unwrap_or_else(|| NonEmptyString::new(" ").unwrap());
                    let tt = talkbank_model::model::dependent_tier::TextTier::new(text);
                    match label {
                        "alt" => talkbank_model::model::DependentTier::Alt(tt),
                        "coh" => talkbank_model::model::DependentTier::Coh(tt),
                        "def" => talkbank_model::model::DependentTier::Def(tt),
                        "eng" => talkbank_model::model::DependentTier::Eng(tt),
                        "err" => talkbank_model::model::DependentTier::Err(tt),
                        "fac" => talkbank_model::model::DependentTier::Fac(tt),
                        "flo" => talkbank_model::model::DependentTier::Flo(tt),
                        "gls" => talkbank_model::model::DependentTier::Gls(tt),
                        "ort" => talkbank_model::model::DependentTier::Ort(tt),
                        "par" => talkbank_model::model::DependentTier::Par(tt),
                        _ => unreachable!(),
                    }
                }
                // TimTier (structured time)
                "tim" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let text = NonEmptyString::new(raw_text.as_str())
                        .unwrap_or_else(|| NonEmptyString::new(" ").unwrap());
                    talkbank_model::model::DependentTier::Tim(
                        talkbank_model::dependent_tier::TimTier::from_text(text),
                    )
                }
                // %wor tier, word tier with timing bullets
                "wor" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let wor = crate::convert::wor_tier_from_input(&raw_text);
                    talkbank_model::model::DependentTier::Wor(wor)
                }
                // Phon project syllabification tiers (with or without x prefix)
                "modsyl" | "xmodsyl" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let words = talkbank_model::dependent_tier::parse_syl_content(&raw_text);
                    talkbank_model::model::DependentTier::Modsyl(
                        talkbank_model::dependent_tier::SylTier::new(
                            talkbank_model::dependent_tier::SylTierType::Modsyl,
                            words,
                        ),
                    )
                }
                "phosyl" | "xphosyl" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let words = talkbank_model::dependent_tier::parse_syl_content(&raw_text);
                    talkbank_model::model::DependentTier::Phosyl(
                        talkbank_model::dependent_tier::SylTier::new(
                            talkbank_model::dependent_tier::SylTierType::Phosyl,
                            words,
                        ),
                    )
                }
                "phoaln" | "xphoaln" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    match talkbank_model::dependent_tier::parse_phoaln_content(&raw_text) {
                        Ok(words) => talkbank_model::model::DependentTier::Phoaln(
                            talkbank_model::dependent_tier::PhoalnTier::new(words),
                        ),
                        Err(_) => {
                            let text = NonEmptyString::new(raw_text.as_str())
                                .unwrap_or_else(|| NonEmptyString::new(" ").unwrap());
                            talkbank_model::model::DependentTier::Unsupported(
                                talkbank_model::model::UserDefinedDependentTier {
                                    label: NonEmptyString::new("phoaln").unwrap(),
                                    content: text,
                                    span: Span::DUMMY,
                                },
                            )
                        }
                    }
                }
                // Phon project per-phone interval tier
                "phoint" | "xphoint" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    match talkbank_model::dependent_tier::parse_xphoint_content(&raw_text) {
                        Ok(groups) => talkbank_model::model::DependentTier::Xphoint(
                            talkbank_model::dependent_tier::XphointTier::new(groups),
                        ),
                        Err(_) => {
                            let text = NonEmptyString::new(raw_text.as_str())
                                .unwrap_or_else(|| NonEmptyString::new(" ").unwrap());
                            talkbank_model::model::DependentTier::Unsupported(
                                talkbank_model::model::UserDefinedDependentTier {
                                    label: NonEmptyString::new("xphoint").unwrap(),
                                    content: text,
                                    span: Span::DUMMY,
                                },
                            )
                        }
                    }
                }
                // Fallback: unsupported tier
                _ => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    talkbank_model::model::DependentTier::Unsupported(
                        talkbank_model::model::UserDefinedDependentTier {
                            label: NonEmptyString::new(label)
                                .unwrap_or_else(|| NonEmptyString::new("unknown").unwrap()),
                            content: NonEmptyString::new(raw_text.as_str())
                                .unwrap_or_else(|| NonEmptyString::new(" ").unwrap()),
                            span: Span::DUMMY,
                        },
                    )
                }
            }
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// ChatFile conversion
// ═══════════════════════════════════════════════════════════════

impl<'a> From<&ast::ChatFile<'a>> for talkbank_model::model::ChatFile {
    fn from(file: &ast::ChatFile<'a>) -> Self {
        let lines: Vec<talkbank_model::model::Line> = file
            .lines
            .iter()
            .map(|line| match line {
                ast::Line::Header(h) => talkbank_model::model::Line::Header {
                    header: Box::new(crate::convert::header_to_model(h)),
                    span: Span::DUMMY,
                    separator: TierSeparator::CLEAN,
                },
                ast::Line::Utterance(u) => {
                    talkbank_model::model::Line::Utterance(Box::new(utterance_to_model(u.as_ref())))
                }
            })
            .collect();
        // Build participants map from @ID headers, enriched with
        // @Participants metadata. This matches TreeSitterParser's behavior:
        // only participants with @ID headers appear in the map. The validator
        // detects missing @ID (E522) by comparing @Participants entries
        // against the participants map.
        //
        // First pass: collect @Participants entries by speaker code (for name/role).
        let mut declared: indexmap::IndexMap<
            SpeakerCode,
            (Option<ParticipantName>, ParticipantRole),
        > = indexmap::IndexMap::new();
        for line in &lines {
            if let talkbank_model::model::Line::Header { header, .. } = line
                && let Header::Participants { entries } = header.as_ref()
            {
                for entry in entries.iter() {
                    declared.insert(
                        entry.speaker_code.clone(),
                        (entry.name.clone(), entry.role.clone()),
                    );
                }
            }
        }
        // Second pass: build participants from @ID headers only.
        let mut participants = indexmap::IndexMap::new();
        for line in &lines {
            if let talkbank_model::model::Line::Header { header, .. } = line {
                match header.as_ref() {
                    Header::ID(id_header) => {
                        let code = id_header.speaker.clone();
                        let (name, role) = declared
                            .get(&code)
                            .cloned()
                            .unwrap_or((None, id_header.role.clone()));
                        participants.insert(
                            code.clone(),
                            talkbank_model::model::Participant {
                                code: code.clone(),
                                name,
                                role,
                                id: id_header.clone(),
                                birth_date: None,
                            },
                        );
                    }
                    Header::Birth { participant, date } => {
                        if let Some(p) = participants.get_mut(participant) {
                            p.birth_date = Some(date.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        // CA omission normalization: if @Options includes CA mode,
        // reclassify standalone (word) shortenings as CAOmission category.
        // This matches TreeSitterParser's post-parse normalization.
        let ca_mode = lines.iter().any(|line| {
            if let talkbank_model::model::Line::Header { header, .. } = line {
                matches!(header.as_ref(), Header::Options { options }
                    if options.iter().any(|opt| opt.enables_ca_mode()))
            } else {
                false
            }
        });
        let mut lines = lines;
        if ca_mode {
            normalize_ca_omissions_in_lines(&mut lines);
        }

        talkbank_model::model::ChatFile::with_participants(lines, participants)
    }
}
