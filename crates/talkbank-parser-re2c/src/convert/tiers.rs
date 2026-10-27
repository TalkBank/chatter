//! Part of the AST→model conversion (see `mod.rs`); split out for file size.

use crate::ast;
use talkbank_model::ErrorSink;
use talkbank_model::model::CaOptionEffect;
use talkbank_model::model::content::word::ca::normalize_ca_omissions_in_lines;
use talkbank_model::model::*;

use super::*;

/// Lower a main tier using the source retained by its line machine.
///
/// A caller cannot substitute unrelated source during conversion:
///
/// ```compile_fail
/// use talkbank_parser_re2c::{convert::main_tier_to_model, ast::LocatedMainTier};
/// use talkbank_parser_re2c::source_text::SourceText;
/// fn replace_source(tier: &LocatedMainTier<'_>, foreign: &str) {
///     main_tier_to_model(tier, SourceText::new(foreign), &talkbank_model::NullErrorSink);
/// }
/// ```
pub fn main_tier_to_model(
    located: &ast::LocatedMainTier<'_>,
    errors: &(impl ErrorSink + ?Sized),
) -> MainTier {
    let source = located.source();
    let mt = located.tier();
    let speaker = SpeakerCode::new(mt.speaker.text());
    let content_items: Vec<UtteranceContent> = mt
        .tier_body
        .contents
        .iter()
        .map(|c| content_item_to_model(c, source))
        .collect();
    let terminator = mt.tier_body.terminator.clone();

    let mut main_tier = MainTier::new(located.locations(), speaker, content_items, terminator);

    // Report the shared model's retained freecode recovery at every depth.
    talkbank_model::alignment::helpers::walk_content(
        &main_tier.content.content,
        None,
        &mut |item| {
            if let talkbank_model::alignment::helpers::ContentItem::Freecode(code) = item {
                code.validate_recovery(errors);
            }
        },
    );

    // Parsed and rejected terminal bullets both occupy the shared slot.
    if let Some(bullet) = mt.tier_body.media_bullet {
        main_tier = main_tier.with_bullet(bullet_from_token(bullet, source));
    }
    main_tier.content.extract_terminal_bullet();

    // The grammar already admitted each linker's meaning and source location.
    main_tier = main_tier.with_linkers(mt.tier_body.linkers.clone());

    // Grammar admission owns both the code and its original location.
    if let Some(precode) = &mt.tier_body.langcode {
        main_tier = main_tier.with_language_code(precode.clone());
    }

    main_tier = main_tier.with_postcodes(
        mt.tier_body
            .postcodes
            .iter()
            .map(|postcode| postcode.to_model(source, errors))
            .collect(),
    );

    main_tier.with_separator(mt.separator)
}

// ═══════════════════════════════════════════════════════════════
// Utterance conversion
// ═══════════════════════════════════════════════════════════════

pub fn utterance_to_model(
    u: &ast::Utterance<'_>,
    errors: &(impl ErrorSink + ?Sized),
) -> talkbank_model::model::Utterance {
    let main = main_tier_to_model(u.main_tier(), errors);
    // Every admitted entry lowers once, retaining its recovery and separator.
    let mut parse_health = talkbank_model::model::ParseHealth::untainted();
    let dep_tiers = u
        .dependent_tiers()
        .iter()
        .map(|entry| {
            let tier = dependent_tier_to_model(entry, errors, &mut parse_health);
            let mut lowered = DependentTierEntry::with_separator(tier, entry.separator());
            if let Some(body) = entry.content_range() {
                lowered.record_content_span(body);
            }
            lowered
        })
        .collect();
    let mut utterance = talkbank_model::model::Utterance::new(main);
    utterance.dependent_tiers = dep_tiers;
    // Admit exactly the recovery state accumulated during this lowering pass.
    parse_health.finish_utterance(utterance)
}

/// Convert a parsed dependent tier to model `DependentTier`, recording on
/// `health` anything the lowering could not build whole.
///
/// Every admitted entry produces exactly one model tier. Rejected content
/// retains its shared recovery variant and taints its alignment domain; no
/// consumer can silently filter a failed conversion out of the utterance.
///
/// Malformed phone alignment and interval bodies retain shared raw-source
/// variants, preserving tier identity and withdrawing their alignment trust.
pub fn dependent_tier_to_model(
    entry: &ast::DependentTierEntryParsed<'_>,
    errors: &(impl ErrorSink + ?Sized),
    health: &mut talkbank_model::model::ParseHealth,
) -> talkbank_model::model::DependentTier {
    use talkbank_model::generated::phon_tier_labels::PhonTierKind;
    use talkbank_model::model::ParseHealthTier;
    let original = entry.source();
    let tier = entry.tier();
    let extent = original.span();
    let source = original.source();
    let location = talkbank_model::Location::Located(extent);

    match tier {
        ast::DependentTierParsed::RejectedMor(tier) => {
            health.taint(ParseHealthTier::Mor);
            talkbank_model::DependentTier::UnparsedMor(tier.clone())
        }
        ast::DependentTierParsed::RejectedPho(tier) => {
            health.taint(ParseHealthTier::Pho);
            talkbank_model::DependentTier::UnparsedPho(tier.clone())
        }
        ast::DependentTierParsed::RejectedMod(tier) => {
            health.taint(ParseHealthTier::Mod);
            talkbank_model::DependentTier::UnparsedMod(tier.clone())
        }
        ast::DependentTierParsed::RejectedSin(tier) => {
            health.taint(ParseHealthTier::Sin);
            talkbank_model::DependentTier::UnparsedSin(tier.clone())
        }
        ast::DependentTierParsed::Mor(mor) => {
            talkbank_model::model::DependentTier::Mor(mor.clone())
        }
        ast::DependentTierParsed::RejectedGra(tier) => {
            health.taint(ParseHealthTier::Gra);
            talkbank_model::DependentTier::UnparsedGra(tier.clone())
        }
        ast::DependentTierParsed::Gra(gra) => talkbank_model::DependentTier::Gra(gra.clone()),
        ast::DependentTierParsed::Pho(pho) => talkbank_model::DependentTier::Pho(pho.clone()),
        ast::DependentTierParsed::Mod(pho) => talkbank_model::DependentTier::Mod(pho.clone()),
        ast::DependentTierParsed::Sin(sin) => talkbank_model::DependentTier::Sin(sin.clone()),
        ast::DependentTierParsed::RejectedWor(tier) => {
            health.taint(ParseHealthTier::Wor);
            talkbank_model::DependentTier::UnparsedWor(tier.clone())
        }
        ast::DependentTierParsed::Wor(wor) => talkbank_model::DependentTier::Wor(wor.clone()),
        ast::DependentTierParsed::RejectedText { label } => {
            let raw = talkbank_model::UnparsedTier::new(extent, original.text());
            match PhonTierKind::from_label(label.as_str()) {
                Some(PhonTierKind::Phoaln) => {
                    health.taint(ParseHealthTier::Phoaln);
                    talkbank_model::DependentTier::UnparsedPhoaln(raw)
                }
                Some(PhonTierKind::Xphoint) => {
                    health.taint(ParseHealthTier::Xphoint);
                    talkbank_model::DependentTier::UnparsedXphoint(raw)
                }
                Some(PhonTierKind::Modsyl | PhonTierKind::Phosyl) | None => {
                    talkbank_model::DependentTier::UnparsedText(
                        talkbank_model::UnparsedTextTier::new(label.nonempty().clone(), raw),
                    )
                }
            }
        }
        ast::DependentTierParsed::Text {
            label: admitted_label,
            content,
        } => {
            let label = admitted_label.as_str();

            // Admit the closed payload family once. The lowering consumes its
            // kind and cannot reach a non-text variant or a panic fallback.
            if let Some(kind) = super::plain_text::PlainTextTierKind::from_label(label) {
                let raw_text: String = content.iter().map(|token| token.text()).collect();
                return kind.lower(location, &raw_text);
            }

            if let Some(kind) = PhonTierKind::from_label(label) {
                return match kind {
                    // Phon project syllabification tiers (with or without x prefix)
                    PhonTierKind::Modsyl => {
                        let raw_text: String = content.iter().map(|t| t.text()).collect();
                        let words = talkbank_model::dependent_tier::parse_syl_content(&raw_text);
                        talkbank_model::model::DependentTier::Modsyl(
                            talkbank_model::dependent_tier::SylTier::new(
                                location,
                                talkbank_model::dependent_tier::SylTierType::Modsyl,
                                words,
                            ),
                        )
                    }
                    PhonTierKind::Phosyl => {
                        let raw_text: String = content.iter().map(|t| t.text()).collect();
                        let words = talkbank_model::dependent_tier::parse_syl_content(&raw_text);
                        talkbank_model::model::DependentTier::Phosyl(
                            talkbank_model::dependent_tier::SylTier::new(
                                location,
                                talkbank_model::dependent_tier::SylTierType::Phosyl,
                                words,
                            ),
                        )
                    }
                    PhonTierKind::Phoaln => {
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
                                talkbank_model::dependent_tier::PhoalnTier::new(location, words),
                            ),
                            Err(error) => {
                                errors.report(talkbank_model::ParseError::at_location(
                                    talkbank_model::ErrorCode::InvalidDependentTier,
                                    talkbank_model::Severity::Error,
                                    location,
                                    format!("malformed %phoaln content: {error}"),
                                ));
                                health.taint(ParseHealthTier::Phoaln);
                                talkbank_model::DependentTier::UnparsedPhoaln(
                                    talkbank_model::UnparsedTier::new(extent, original.text()),
                                )
                            }
                        }
                    }
                    // Phon project per-phone interval tier
                    PhonTierKind::Xphoint => {
                        let raw_text: String = content.iter().map(|t| t.text()).collect();
                        // This guard IS needed, unlike `%phoaln` above:
                        // `parse_xphoint_content("")` returns `Err(EmptyGroup)`, so
                        // without it an absent body is reported as malformed
                        // content and the tier loses its identity to E605.
                        if raw_text.trim().is_empty() {
                            talkbank_model::model::DependentTier::Xphoint(
                                talkbank_model::dependent_tier::XphointTier::new(
                                    location,
                                    Vec::new(),
                                ),
                            )
                        } else {
                            match talkbank_model::dependent_tier::parse_xphoint_content(&raw_text) {
                                Ok(groups) => talkbank_model::model::DependentTier::Xphoint(
                                    talkbank_model::dependent_tier::XphointTier::new(
                                        location, groups,
                                    ),
                                ),
                                Err(error) => {
                                    errors.report(talkbank_model::ParseError::at_location(
                                        talkbank_model::ErrorCode::InvalidDependentTier,
                                        talkbank_model::Severity::Error,
                                        location,
                                        format!("malformed %xphoint content: {error}"),
                                    ));
                                    health.taint(ParseHealthTier::Xphoint);
                                    talkbank_model::DependentTier::UnparsedXphoint(
                                        talkbank_model::UnparsedTier::new(extent, original.text()),
                                    )
                                }
                            }
                        }
                    }
                };
            }

            // Admission owns both the custom syntax and structured exclusions.
            // Other labels continue through structured or unsupported lowering.
            if let Ok(label) = talkbank_model::UserDefinedDependentTierLabel::new(label) {
                let raw_text: String = content.iter().map(|t| t.text()).collect();
                return talkbank_model::model::DependentTier::UserDefined(
                    talkbank_model::model::UserDefinedDependentTier {
                        label,
                        // No fabrication. An empty tier is now representable,
                        // so this reports what it found and lets E756 judge it.
                        // It used to substitute a single space, which tripped
                        // E756 by accident: the right diagnostic for the wrong
                        // reason, and only because the model had no honest way
                        // to say "this line carried nothing".
                        content: RecoveryText::new(raw_text.as_str()).ok(),
                        location,
                    },
                );
            }

            // BulletContent tiers. Each arm builds its own payload rather than
            // sharing one `let bc = ...` above the match: only these nine consume
            // it, and hoisting it made every OTHER arm (`%tim`,
            // `%wor`, the four Phon tiers, `%x*` and the fallback) build a
            // `BulletContent` (a `Vec`, a `SmolStr` per token, and a normalization
            // pass per segment) only to drop it. Over six figures of corpus files
            // that is the one measurable cost in this function.
            match label {
                "com" => talkbank_model::model::DependentTier::Com(ComTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
                "act" => talkbank_model::model::DependentTier::Act(ActTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
                "exp" => talkbank_model::model::DependentTier::Exp(ExpTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
                "add" => talkbank_model::model::DependentTier::Add(AddTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
                "gpx" => talkbank_model::model::DependentTier::Gpx(GpxTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
                "int" => talkbank_model::model::DependentTier::Int(IntTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
                "spa" => talkbank_model::model::DependentTier::Spa(SpaTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
                "sit" => talkbank_model::model::DependentTier::Sit(SitTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
                "cod" => talkbank_model::model::DependentTier::Cod(CodTier::new(
                    location,
                    tokens_to_bullet_content(content, source),
                )),
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
                    // `RecoveryText`: a body of one space is a declaration
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
                        match RecoveryText::new(raw_text.as_str()) {
                            Ok(text) if text.as_str().trim().is_empty() => {
                                talkbank_model::dependent_tier::TimTier::empty(location)
                            }
                            Ok(text) => {
                                talkbank_model::dependent_tier::TimTier::from_text(location, text)
                            }
                            Err(_) => talkbank_model::dependent_tier::TimTier::empty(location),
                        },
                    )
                }
                // Fallback: unsupported tier
                _ => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    talkbank_model::model::DependentTier::Unsupported(
                        talkbank_model::model::UnsupportedDependentTier {
                            label: admitted_label.nonempty().clone().into(),
                            content: RecoveryText::new(raw_text.as_str()).ok(),
                            location,
                        },
                    )
                }
            }
        }
    }
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
        .lines()
        .iter()
        .map(|line| match line {
            ast::Line::Header(header) => talkbank_model::model::Line::Header {
                header: Box::new(crate::convert::header_to_model(header)),
                location: header.span().into(),
                separator: header.separator(),
            },
            ast::Line::Utterance(u) => talkbank_model::model::Line::Utterance(Box::new(
                utterance_to_model(u.as_ref(), errors),
            )),
        })
        .collect();
    // The join is the model's, not this backend's. Both parsers ask the
    // same function, so "which speakers appear, in what order" cannot drift
    // between them again; it already did once, and that drift was 445 of
    // the 446 whole-model divergences measured over the corpus.
    //
    // Its diagnostics (E522, E523, E524) go to the caller's sink, which is
    // required join-reporting boundary before returning the model.
    talkbank_model::model::participant::join::build_participants_from_lines(&lines)
        .report_diagnostics(errors);

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

    talkbank_model::model::ChatFile::new(lines)
}
