//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use crate::ast;
use crate::token::Token;
use talkbank_model::ErrorSink;
use talkbank_model::Span;
use talkbank_model::model::CaOptionEffect;
use talkbank_model::model::content::word::ca::normalize_ca_omissions_in_lines;
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
        .and_then(|t| token_to_terminator(t));

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
        main_tier = main_tier.with_bullet(bullet_from_times(start_time, end_time));
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

    // Language code ([- lang]). The token carries the tag-extracted code
    // directly ("zho"). Declined rather than `expect`ed on the impossible
    // empty case: a parser has no business panicking on input, and the
    // `[- ` lexer rule requires at least one character anyway.
    if let Some(langcode_tok) = &mt.tier_body.langcode
        && let Ok(code) = LanguageCode::new(langcode_tok.text())
    {
        main_tier = main_tier.with_language_code(code);
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
        ast::DependentTierParsed::Wor(wor_parsed) => {
            talkbank_model::model::DependentTier::Wor(wor_tier_to_model(wor_parsed))
        }
        ast::DependentTierParsed::Text { prefix, content } => {
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
                        // The branch condition above is `label.len() >= 2`,
                        // so the label cannot be empty here.
                        label: NonEmptyString::new_unchecked(label),
                        // No fabrication. An empty tier is now representable,
                        // so this reports what it found and lets E756 judge it.
                        // It used to substitute a single space, which tripped
                        // E756 by accident: the right diagnostic for the wrong
                        // reason, and only because the model had no honest way
                        // to say "this line carried nothing".
                        content: NonEmptyString::new(raw_text.as_str()).ok(),
                        span: Span::DUMMY,
                    },
                ));
            }

            // BulletContent tiers. Each arm builds its own payload rather than
            // sharing one `let bc = ...` above the match: only these nine consume
            // it, and hoisting it made every OTHER arm (the ten text tiers, `%tim`,
            // `%wor`, the four Phon tiers, `%x*` and the fallback) build a
            // `BulletContent` (a `Vec`, a `SmolStr` per token, and a normalization
            // pass per segment) only to drop it. Over six figures of corpus files
            // that is the one measurable cost in this function.
            match label {
                "com" => talkbank_model::model::DependentTier::Com(ComTier::new(
                    tokens_to_bullet_content(content),
                )),
                "act" => talkbank_model::model::DependentTier::Act(ActTier::new(
                    tokens_to_bullet_content(content),
                )),
                "exp" => talkbank_model::model::DependentTier::Exp(ExpTier::new(
                    tokens_to_bullet_content(content),
                )),
                "add" => talkbank_model::model::DependentTier::Add(AddTier::new(
                    tokens_to_bullet_content(content),
                )),
                "gpx" => talkbank_model::model::DependentTier::Gpx(GpxTier::new(
                    tokens_to_bullet_content(content),
                )),
                "int" => talkbank_model::model::DependentTier::Int(IntTier::new(
                    tokens_to_bullet_content(content),
                )),
                "spa" => talkbank_model::model::DependentTier::Spa(SpaTier::new(
                    tokens_to_bullet_content(content),
                )),
                "sit" => talkbank_model::model::DependentTier::Sit(SitTier::new(
                    tokens_to_bullet_content(content),
                )),
                "cod" => talkbank_model::model::DependentTier::Cod(CodTier::new(
                    tokens_to_bullet_content(content),
                )),
                // TextTier tiers (plain string content)
                "alt" | "coh" | "def" | "eng" | "err" | "fac" | "flo" | "gls" | "ort" | "par" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    // No fabrication. This used to substitute a single space
                    // for empty content, which made the tier look well formed
                    // and the whole FILE read as valid where tree-sitter
                    // reported errors. Saying the tier is empty is what lets
                    // the validator reject it (E756's rule: a tier whose
                    // content is empty declares nothing).
                    let tt = match NonEmptyString::new(raw_text.as_str()) {
                        Ok(text) => talkbank_model::model::dependent_tier::TextTier::new(text),
                        Err(_) => talkbank_model::model::dependent_tier::TextTier::empty(),
                    };
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
                    // Same fabrication, same cure. `%tim` carries a structured
                    // time, so an empty one is not merely undeclared, it is a
                    // time that is not there; a substituted space made it parse
                    // as though a value had been read.
                    //
                    // It also used to lose the `Tim` identity, lowering an empty
                    // `%tim:` to an unsupported DEPENDENT TIER and so reporting
                    // E605 ("unsupported dependent tier '%tim'") about a tier
                    // name that is perfectly supported. `TimTier` grew the
                    // `Empty` state on 2026-08-16, so the tier keeps its
                    // identity and E756 judges it, agreeing with tree-sitter.
                    //
                    // Emptiness is tested AFTER trimming, not by
                    // `NonEmptyString`: a body of one space is a declaration
                    // that was never made, and treating it as content made
                    // `from_text` call it a non-time and report E603, "Invalid
                    // %tim tier format: ''", ALONGSIDE E756. Two codes for one
                    // fact, and the more specific of them false. Tree-sitter's
                    // separator absorbs that space and reaches `Empty`, so this
                    // is also what makes the two backends agree.
                    //
                    // The content is stored UNTRIMMED. Only the verdict uses
                    // `trim`; trimming the payload too would change the bytes a
                    // `%tim` line roundtrips to, which is a different change
                    // from the one being made here.
                    talkbank_model::model::DependentTier::Tim(
                        match NonEmptyString::new(raw_text.as_str()) {
                            Ok(text) if text.as_str().trim().is_empty() => {
                                talkbank_model::dependent_tier::TimTier::empty()
                            }
                            Ok(text) => talkbank_model::dependent_tier::TimTier::from_text(text),
                            Err(_) => talkbank_model::dependent_tier::TimTier::empty(),
                        },
                    )
                }
                // %wor tier, word tier with timing bullets.
                //
                // This is the FALLBACK path: a `%wor` line reaches here only
                // after `wor_tier_parser` already failed and the line became a
                // text tier, so re-lexing the reconstructed text is a second
                // attempt at something that has failed once. It is kept for now
                // (removing it means passing the original `&[Token]` through,
                // which touches every text-tier branch) but it no longer
                // fabricates: a tier that will not parse propagates `None`
                // exactly as a malformed `%mor` does, rather than becoming a
                // `%wor` tier with no words.
                "wor" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    talkbank_model::model::DependentTier::Wor(crate::convert::wor_tier_from_input(
                        &raw_text,
                    )?)
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
                    // No empty-body guard here, deliberately, and it is worth
                    // saying why since `%xphoint` below has one.
                    // `parse_phoaln_content` is `split_whitespace` into a
                    // `Vec`, so an empty or whitespace-only body already yields
                    // `Ok(vec![])`: an empty `PhoalnTier`, which is what
                    // `PhoalnTier::is_empty` reports and E756 judges. A guard
                    // would be a second place deciding what an empty `%phoaln`
                    // means, agreeing with this one only by inspection.
                    match talkbank_model::dependent_tier::parse_phoaln_content(&raw_text) {
                        Ok(words) => talkbank_model::model::DependentTier::Phoaln(
                            talkbank_model::dependent_tier::PhoalnTier::new(words),
                        ),
                        Err(_) => {
                            // No fabrication: a tier that will not parse says so.
                            let text = NonEmptyString::new(raw_text.as_str()).ok();
                            talkbank_model::model::DependentTier::Unsupported(
                                talkbank_model::model::UserDefinedDependentTier {
                                    label: NonEmptyString::new_unchecked("phoaln"),
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
                    // This guard IS needed, unlike `%phoaln` above:
                    // `parse_xphoint_content("")` returns `Err(EmptyGroup)`, so
                    // without it an absent body is reported as malformed
                    // content and the tier loses its identity to E605.
                    if raw_text.trim().is_empty() {
                        talkbank_model::model::DependentTier::Xphoint(
                            talkbank_model::dependent_tier::XphointTier::new(Vec::new()),
                        )
                    } else {
                        match talkbank_model::dependent_tier::parse_xphoint_content(&raw_text) {
                            Ok(groups) => talkbank_model::model::DependentTier::Xphoint(
                                talkbank_model::dependent_tier::XphointTier::new(groups),
                            ),
                            Err(_) => {
                                // No fabrication: a tier that will not parse says so.
                                let text = NonEmptyString::new(raw_text.as_str()).ok();
                                talkbank_model::model::DependentTier::Unsupported(
                                    talkbank_model::model::UserDefinedDependentTier {
                                        label: NonEmptyString::new_unchecked("xphoint"),
                                        content: text,
                                        span: Span::DUMMY,
                                    },
                                )
                            }
                        }
                    }
                }
                // Fallback: unsupported tier
                _ => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    talkbank_model::model::DependentTier::Unsupported(
                        talkbank_model::model::UserDefinedDependentTier {
                            // An empty label is not a tier named "unknown",
                            // which is what this used to invent: a label no
                            // file contained, reported to the operator as
                            // though it had been read. The fallback arm is
                            // reached only with a label the lexer matched, so
                            // an empty one means the lexer produced something
                            // this conversion cannot describe, and the honest
                            // answer is to say so rather than name it.
                            label: match NonEmptyString::new(label) {
                                Ok(label) => label,
                                Err(_) => return None,
                            },
                            content: NonEmptyString::new(raw_text.as_str()).ok(),
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

/// Lower a parsed re2c AST to the model, reporting the lowering's own
/// diagnostics into `errors`.
///
/// # Why this is a function and not `From`
///
/// It was `impl From<&ast::ChatFile> for ChatFile`, which is infallible by
/// signature and so had nowhere to put a diagnostic. The participant join
/// produces three (E522, E523, E524) and they were dropped on the floor,
/// making the re2c backend silently more permissive than tree-sitter on every
/// file with an inconsistent `@Participants` block. An infallible conversion
/// that discards what it learned is the banned shape; the sink was already
/// available one frame up, at `parse_chat_file_to_model`.
pub fn chat_file_to_model(
    file: &ast::ChatFile<'_>,
    errors: &(impl ErrorSink + ?Sized),
) -> talkbank_model::model::ChatFile {
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
    // The join is the model's, not this backend's. Both parsers ask the
    // same function, so "which speakers appear, in what order" cannot drift
    // between them again; it already did once, and that drift was 445 of
    // the 446 whole-model divergences measured over the corpus.
    //
    // Its diagnostics (E522, E523, E524) go to the caller's sink, which is
    // the only way to reach the map.
    let participants =
        talkbank_model::model::participant::join::build_participants_from_lines(&lines)
            .report_into(errors);

    // `@Options: CA` REINTERPRETS a standalone (word) shortening as a CA
    // omission rather than an error. Not a leniency waiver: the same bytes mean
    // something different. Matches TreeSitterParser's post-parse normalization.
    let ca_mode = lines.iter().any(|line| {
        if let talkbank_model::model::Line::Header { header, .. } = line {
            matches!(header.as_ref(), Header::Options { options }
                if options.iter().any(|opt| opt.has_effect(CaOptionEffect::ParentheticalIsCaOmission)))
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

/// Lower a parsed `%wor` tier to the model.
///
/// The one lowering, shared by the file-level path and by
/// `wor_tier_from_input`. Before 2026-08-08 those were two different
/// conversions and the second one dropped timing bullets, language precodes
/// and terminators on the floor.
pub(crate) fn wor_tier_to_model(parsed: &ast::WorTierParsed<'_>) -> WorTier {
    use talkbank_model::model::dependent_tier::wor::WorItem;

    let wor_items: Vec<WorItem> = parsed
        .items
        .iter()
        .map(|item| match item {
            ast::WorItemParsed::Word { word, bullet } => {
                let mut w = word_from_parsed(word);
                if let Some((start_ms, end_ms)) = bullet {
                    w = w.with_inline_bullet(Bullet::new(*start_ms, *end_ms));
                }
                WorItem::Word(Box::new(w))
            }
            ast::WorItemParsed::Separator(kind) => WorItem::Separator {
                text: kind.chat_text().to_string(),
                span: Span::DUMMY,
            },
        })
        .collect();

    let mut wor = WorTier::new(wor_items);
    // The lexer hands back the code alone (`zho`); the brackets and the `- `
    // are already stripped by the `[- ` rule's tag capture. That rule requires
    // at least one character, so the empty case cannot arrive; it is declined
    // rather than `expect`ed, because a parser has no business panicking on
    // input and the tier's own diagnostics still fire.
    if let Some(tok) = &parsed.langcode
        && let Ok(code) = talkbank_model::model::LanguageCode::new(tok.text())
    {
        wor.language_code = Some(code);
    }
    if let Some(t) = &parsed.terminator {
        wor.terminator = token_to_terminator(t);
    }
    wor
}
