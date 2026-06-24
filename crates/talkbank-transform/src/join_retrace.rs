//! Auto-repair of the OBVIOUS subset of E370 ("dangling retrace") errors.
//!
//! E370 fires when an utterance's last main-tier content is a retrace marker
//! with nothing after it, e.g. `*CHI: the dog [/] .`. Empirically these are
//! same-speaker splits: the repeated material starts the NEXT same-speaker
//! utterance. This transform joins the two utterances back into one.
//!
//! Three repair scopes are provided, selected via [`RetraceJoinScope`]:
//!
//! - [`RetraceJoinScope::RepetitionOnly`] (default, Wave 1): joins ONLY
//!   partial-repetition retraces (`[/]`, [`RetraceKind::Partial`]) where the
//!   next same-speaker utterance's leading lexical words repeat the retraced
//!   material. Corrections (`[//]`), multiple retraces (`[///]`), reformulations
//!   (`[/-]`), and non-repeating successors are deliberately left untouched.
//!
//! - [`RetraceJoinScope::RepetitionAndCorrections`] (Wave 3a, opt-in): also
//!   joins correction retraces (`[//]` Full, `[///]` Multiple, `[/-]`
//!   Reformulation). Corrections REPLACE rather than repeat the retraced
//!   material, so there is NO leading-words repeat check; the gate is simply
//!   that a same-speaker successor exists. The retraced material must still be
//!   a pure plain-word sequence (conservative policy, same as for `[/]`).
//!
//! - [`RetraceJoinScope::AllSameSpeakerSuccessor`] (Wave 3b, broadest scope,
//!   opt-in): joins ANY dangling retrace kind, including `[/]` Partial where the
//!   successor does NOT repeat the retraced material, with the immediately-
//!   following same-speaker utterance. No repeat-prefix match is required for
//!   any kind. This covers genuine child-language disfluencies (false starts,
//!   partial words, expansions, fillers) where the transcriber correctly coded a
//!   `[/]` but the successor cannot repeat the abandoned material.
//!
//! # The join
//!
//! The joined utterance is U's content (INCLUDING the trailing retrace marker)
//! followed by V's content, terminated by V's terminator. The two main-tier
//! time bullets are unioned (start = U.start, end = V.end). V is removed as a
//! separate line.
//!
//! # Dependent tiers
//!
//! A naive merge of two `%gra` tiers yields two ROOT relations, which chatter
//! rejects (E723). For Wave 1, if either utterance carried any dependent tier,
//! ALL dependent tiers are DROPPED on the joined utterance and the join is
//! counted as "needs re-morphotag" in the stats. A main tier with no dependent
//! tiers is valid CHAT even when sibling utterances carry them; downstream
//! morphotagging (BA3) regenerates `%mor`/`%gra` later.
//!
//! # References
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope>

use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{
    BracketedContent, BracketedItem, Bullet, ChatFile, Line, MainTier, RetraceKind,
    UtteranceContent,
};
use talkbank_model::validation::ValidationState;

/// Controls which dangling-retrace kinds the join transform handles.
///
/// The default variant preserves the Wave-1 conservative behavior; each
/// broader variant is an opt-in extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RetraceJoinScope {
    /// Join only partial-repetition retraces (`[/]`). The successor must lead
    /// with the retraced material as a prefix (the "OBVIOUS" gate). This is the
    /// conservative default and the only behavior prior to Wave 3a.
    #[default]
    RepetitionOnly,
    /// Join partial-repetition retraces AND correction retraces (`[//]` Full,
    /// `[///]` Multiple, `[/-]` Reformulation). Corrections replace rather than
    /// repeat the retraced material, so the leading-words prefix check is
    /// skipped; the gate is same-speaker successor only.
    RepetitionAndCorrections,
    /// Join ANY dangling retrace kind (including `[/]` Partial) with the
    /// immediately-following same-speaker utterance, with NO repeat-prefix
    /// match required (Wave 3b). This covers genuine child-language
    /// disfluencies where the transcriber correctly coded a `[/]` but the
    /// successor does NOT repeat the retraced material (false starts, partial
    /// words, expansions, fillers). The gate is same-speaker successor only.
    AllSameSpeakerSuccessor,
}

/// Repair summary for one CHAT file.
///
/// Counts are accumulated across every join performed in the file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JoinRetraceStats {
    /// Number of dangling-retrace utterances joined with their successor.
    pub joined_utterances: usize,
    /// Number of joined utterances that had to drop dependent tiers because
    /// either side carried `%mor`/`%gra`/other dependent tiers.
    pub needs_remorphotag: usize,
    /// Total number of dependent tiers dropped across all joins.
    pub dependent_tiers_dropped: usize,
}

impl JoinRetraceStats {
    /// Returns `true` when no join was performed.
    pub fn is_empty(self) -> bool {
        self.joined_utterances == 0
    }
}

/// Join dangling-retrace utterances with their same-speaker successor in place.
///
/// Scans the file in document order. The `scope` parameter selects which
/// retrace kinds are eligible; see [`RetraceJoinScope`] for the exact gate
/// applied per kind. See the module docs for the join, bullet-union, and
/// dependent-tier rules.
pub fn join_dangling_retraces<S: ValidationState>(
    chat: &mut ChatFile<S>,
    scope: RetraceJoinScope,
) -> JoinRetraceStats {
    let mut stats = JoinRetraceStats::default();

    // Index-based scan: a join removes a later line, so we cannot hold a
    // borrow across the mutation. We re-derive indices each iteration.
    let mut i = 0usize;
    while i < chat.lines.len() {
        let Some(j) = obvious_join_target(&chat.lines, i, scope) else {
            i += 1;
            continue;
        };

        perform_join(chat, i, j, &mut stats);
        // Do not advance `i`: the joined utterance now ends in V's content, so
        // it can no longer be a dangling retrace (its last node is V's last
        // content), and re-examining it is cheap and safe.
        i += 1;
    }

    stats
}

/// If the utterance at `start_index` is a qualifying dangling retrace, return
/// the successor's line index.
///
/// The `scope` controls which retrace kinds are eligible:
///
/// - For [`RetraceJoinScope::RepetitionOnly`], only `[/]` qualifies, and only
///   when the successor's leading words repeat the retraced material.
/// - For [`RetraceJoinScope::RepetitionAndCorrections`], `[//]`/`[///]`/`[/-]`
///   also qualify; for those, the prefix check is skipped (corrections replace,
///   not repeat, the retraced material).
///
/// Returns `None` when `start_index` is not a qualifying utterance, when there
/// is no following utterance, when the speakers differ, or (for `[/]` only)
/// when the successor does not lead with exactly the retraced material.
fn obvious_join_target(
    lines: &[Line],
    start_index: usize,
    scope: RetraceJoinScope,
) -> Option<usize> {
    let Line::Utterance(u) = lines.get(start_index)? else {
        return None;
    };

    // U's last main-tier content node must be a dangling retrace of a kind
    // allowed by the current scope.
    let (kind, retrace_material) = dangling_retrace_material(&u.main, scope)?;
    if retrace_material.is_empty() {
        return None;
    }

    // Find the next utterance line (skip interstitial headers / comments).
    let v_index = next_utterance_index(lines, start_index)?;
    let Line::Utterance(v) = &lines[v_index] else {
        return None;
    };

    // Same speaker.
    if v.main.speaker != u.main.speaker {
        return None;
    }

    // For partial repetition under RepetitionOnly or RepetitionAndCorrections:
    // V's leading lexical words must repeat the retraced material as a prefix.
    // Under AllSameSpeakerSuccessor, no prefix match is required for any kind.
    // Corrections always skip the prefix check (they replace, not repeat).
    match kind {
        RetraceKind::Partial => {
            match scope {
                RetraceJoinScope::RepetitionOnly
                | RetraceJoinScope::RepetitionAndCorrections => {
                    if !leading_words_match_prefix(&v.main, &retrace_material) {
                        return None;
                    }
                }
                RetraceJoinScope::AllSameSpeakerSuccessor => {
                    // No prefix check: any same-speaker successor is accepted.
                }
            }
        }
        RetraceKind::Full | RetraceKind::Multiple | RetraceKind::Reformulation => {
            // Corrections: no prefix match required under any corrections-enabled scope.
        }
    }

    Some(v_index)
}

/// Returns the retrace kind and the retraced material's lexical-word
/// `cleaned_text` sequence when the main tier's LAST content node is a
/// dangling retrace of a kind allowed by `scope`; otherwise `None`.
///
/// "Dangling" means the retrace marker is the last content node (nothing
/// after it on the main tier). The case is only treated as OBVIOUS when
/// the retrace material is a pure sequence of plain words (no replaced words,
/// separators, or other markers), keeping both waves strictly conservative.
fn dangling_retrace_material(
    main: &MainTier,
    scope: RetraceJoinScope,
) -> Option<(RetraceKind, Vec<String>)> {
    let last = main.content.content.last()?;
    let UtteranceContent::Retrace(retrace) = last else {
        return None;
    };
    // Check whether this kind is enabled under the current scope.
    match retrace.kind {
        RetraceKind::Partial => {
            // Enabled under all scopes.
        }
        RetraceKind::Full | RetraceKind::Multiple | RetraceKind::Reformulation => {
            // Only enabled when corrections are explicitly opted into.
            match scope {
                RetraceJoinScope::RepetitionOnly => return None,
                RetraceJoinScope::RepetitionAndCorrections
                | RetraceJoinScope::AllSameSpeakerSuccessor => {}
            }
        }
    }

    let words = pure_word_sequence_from_bracketed(&retrace.content)?;
    Some((retrace.kind, words))
}

/// Extract a pure plain-word `cleaned_text` sequence from a retrace's bracketed
/// content, returning `None` if it contains anything other than top-level plain
/// words (nested groups, replaced words, separators, events, etc.).
///
/// Wave 1 is deliberately conservative: only `word [/]` / `<word ...> [/]`
/// repetitions of plain words count as OBVIOUS. Anything richer is left for a
/// later wave, so a single non-`Word` bracketed item disqualifies the case.
fn pure_word_sequence_from_bracketed(content: &BracketedContent) -> Option<Vec<String>> {
    let mut words = Vec::with_capacity(content.content.len());
    for item in content.content.iter() {
        match item {
            BracketedItem::Word(word) => words.push(word.cleaned_text().to_owned()),
            BracketedItem::AnnotatedWord(_)
            | BracketedItem::ReplacedWord(_)
            | BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::Separator(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => return None,
        }
    }
    Some(words)
}

/// Returns `true` when the main tier's leading lexical words (in document
/// order) equal `prefix` as a prefix. The leading run up to `prefix.len()`
/// words must all be plain words (no replaced words / separators), and their
/// `cleaned_text` must match `prefix` exactly in order.
fn leading_words_match_prefix(main: &MainTier, prefix: &[String]) -> bool {
    let mut leading = Vec::with_capacity(prefix.len());
    let mut impure_before_prefix = false;
    walk_words(&main.content.content, None, &mut |item| {
        if leading.len() >= prefix.len() {
            return;
        }
        match item {
            WordItem::Word(word) => leading.push(word.cleaned_text().to_owned()),
            WordItem::ReplacedWord(_) | WordItem::Separator(_) => {
                // A non-plain-word among the leading run makes the successor
                // non-obvious; record it so we reject below.
                impure_before_prefix = true;
            }
        }
    });

    !impure_before_prefix && leading.len() == prefix.len() && leading == prefix
}

/// Find the index of the next `Line::Utterance` strictly after `from_index`,
/// skipping interstitial `Line::Header` (comment) lines.
fn next_utterance_index(lines: &[Line], from_index: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(from_index + 1)
        .find_map(|(idx, line)| match line {
            Line::Utterance(_) => Some(idx),
            Line::Header { .. } => None,
        })
}

/// Perform the join of utterance at `u_index` with utterance at `v_index`.
///
/// Preconditions (established by [`obvious_join_target`]): both indices are
/// `Line::Utterance`, U's last content is a partial retrace, same speaker,
/// V leads with the retraced material. `v_index > u_index`.
fn perform_join<S: ValidationState>(
    chat: &mut ChatFile<S>,
    u_index: usize,
    v_index: usize,
    stats: &mut JoinRetraceStats,
) {
    // Remove V first (higher index) so U's index stays valid.
    let Line::Utterance(v) = chat.lines.remove(v_index) else {
        // Precondition guarantees an utterance; if not, restore nothing and
        // leave the file unchanged. This branch is unreachable in practice.
        return;
    };
    let v = *v;

    let Some(Line::Utterance(u)) = chat.lines.get_mut(u_index) else {
        // Unreachable given preconditions; bail without mutating further.
        return;
    };

    // Count dependent tiers that will be dropped (from BOTH sides). U's own
    // dependent tiers are dropped because the joined main tier no longer
    // aligns with them; V's are dropped for the same reason.
    let dropped = u.dependent_tiers.len() + v.dependent_tiers.len();

    // Union the main-tier time bullets: start from U, end from V.
    let unioned_bullet = union_bullets(
        u.main.content.bullet.as_ref(),
        v.main.content.bullet.as_ref(),
    );

    // Append V's content onto U's (U keeps its trailing retrace marker).
    let mut v_content = v.main.content;
    u.main.content.content.0.append(&mut v_content.content.0);

    // The joined utterance is terminated by V's terminator.
    u.main.content.terminator = v_content.terminator;

    // V's postcodes follow the joined content's terminator.
    u.main.content.postcodes.0.extend(v_content.postcodes.0);

    // Apply the unioned bullet (or clear if neither side had one).
    u.main.content.bullet = unioned_bullet;

    // Drop ALL dependent tiers on the joined utterance (Wave 1 policy).
    if dropped > 0 {
        u.dependent_tiers.clear();
        stats.dependent_tiers_dropped += dropped;
        stats.needs_remorphotag += 1;
    }

    stats.joined_utterances += 1;
}

/// Union two optional main-tier time bullets.
///
/// If both sides carry a bullet, the result spans from U's start to V's end.
/// If only one side has a bullet, that bullet is kept. If neither has one, the
/// result is `None`.
fn union_bullets(u_bullet: Option<&Bullet>, v_bullet: Option<&Bullet>) -> Option<Bullet> {
    match (u_bullet, v_bullet) {
        (Some(u), Some(v)) => Some(Bullet::new(u.timing.start_ms, v.timing.end_ms)),
        (Some(u), None) => Some(u.clone()),
        (None, Some(v)) => Some(v.clone()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{JoinRetraceStats, RetraceJoinScope, join_dangling_retraces};
    use talkbank_model::model::WriteChat;
    use talkbank_parser::TreeSitterParser;

    /// Parse, run the join transform (repetition-only scope), and return the
    /// serialized result and stats.
    fn join(chat: &str) -> (String, JoinRetraceStats) {
        join_with_scope(chat, RetraceJoinScope::RepetitionOnly)
    }

    /// Parse, run the join transform with an explicit scope, and return the
    /// serialized result and stats.
    fn join_with_scope(chat: &str, scope: RetraceJoinScope) -> (String, JoinRetraceStats) {
        let parser = TreeSitterParser::new().expect("parser");
        let mut parsed = parser.parse_chat_file(chat).expect("parse chat");
        let stats = join_dangling_retraces(&mut parsed, scope);
        (parsed.to_chat_string(), stats)
    }

    const HEADER: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n";

    fn doc(body: &str) -> String {
        format!("{HEADER}{body}@End\n")
    }

    /// The OBVIOUS single-word `[/]` case joins into one utterance.
    #[test]
    fn joins_obvious_partial_retrace() {
        let input = doc("*CHI:\tI want and [/] .\n*CHI:\tand the cat .\n");
        let (out, stats) = join(&input);
        assert_eq!(
            stats,
            JoinRetraceStats {
                joined_utterances: 1,
                needs_remorphotag: 0,
                dependent_tiers_dropped: 0,
            }
        );
        assert!(
            out.contains("*CHI:\tI want and [/] and the cat ."),
            "got:\n{out}"
        );
        assert_eq!(out.matches("*CHI:").count(), 1, "got:\n{out}");
    }

    /// A multi-word group retrace `<the dog> [/]` joins when the successor
    /// repeats the full retraced material as a prefix.
    #[test]
    fn joins_group_form_partial_retrace() {
        let input = doc("*CHI:\t<the dog> [/] .\n*CHI:\tthe dog runs .\n");
        let (out, stats) = join(&input);
        assert_eq!(stats.joined_utterances, 1);
        assert!(
            out.contains("*CHI:\t<the dog> [/] the dog runs ."),
            "got:\n{out}"
        );
    }

    /// When either side carried dependent tiers, the join drops them and
    /// reports the utterance as needing re-morphotag.
    #[test]
    fn drops_dependent_tiers_and_flags_remorphotag() {
        let input = doc(
            "*CHI:\t<the dog> [/] .\n%mor:\tdet:art|the noun|dog .\n%gra:\t1|2|DET 2|0|ROOT 3|2|PUNCT\n*CHI:\tthe dog runs .\n%mor:\tdet:art|the noun|dog verb|run-3S .\n%gra:\t1|2|DET 2|3|SUBJ 3|0|ROOT 4|3|PUNCT\n",
        );
        let (out, stats) = join(&input);
        assert_eq!(stats.joined_utterances, 1);
        assert_eq!(stats.needs_remorphotag, 1);
        // Two %mor and two %gra tiers dropped (one of each per side).
        assert_eq!(stats.dependent_tiers_dropped, 4);
        assert!(
            !out.contains("%mor:") && !out.contains("%gra:"),
            "joined utterance must drop dependent tiers, got:\n{out}"
        );
        assert!(
            out.contains("*CHI:\t<the dog> [/] the dog runs ."),
            "got:\n{out}"
        );
    }

    /// Bullets union: start from U, end from V.
    #[test]
    fn unions_main_tier_time_bullets() {
        let input = doc(
            "*CHI:\t<the dog> [/] . \u{0015}0_500\u{0015}\n*CHI:\tthe dog runs . \u{0015}500_1200\u{0015}\n",
        );
        let (out, stats) = join(&input);
        assert_eq!(stats.joined_utterances, 1);
        assert!(out.contains("\u{0015}0_1200\u{0015}"), "got:\n{out}");
    }

    /// Negative (RepetitionOnly): a `[//]` correction is left untouched under
    /// the default scope.
    #[test]
    fn leaves_correction_retrace_untouched_under_repetition_only() {
        let input = doc("*CHI:\tthe cat [//] .\n*CHI:\tthe dog runs .\n");
        let (out, stats) = join(&input);
        assert!(stats.is_empty(), "stats: {stats:?}");
        assert_eq!(out, input);
    }

    /// Negative: a non-repeating successor is left untouched.
    #[test]
    fn leaves_non_repeating_successor_untouched() {
        let input = doc("*CHI:\tthe dog [/] .\n*CHI:\twhat happened next .\n");
        let (out, stats) = join(&input);
        assert!(stats.is_empty(), "stats: {stats:?}");
        assert_eq!(out, input);
    }

    /// Negative: a different-speaker successor is left untouched.
    #[test]
    fn leaves_different_speaker_successor_untouched() {
        let input = doc("*CHI:\tthe dog [/] .\n*MOT:\tthe dog runs .\n");
        let (out, stats) = join(&input);
        assert!(stats.is_empty(), "stats: {stats:?}");
        assert_eq!(out, input);
    }

    // --- Wave 3a: RepetitionAndCorrections scope ---

    /// A dangling `[//]` full-correction retrace joins under
    /// RepetitionAndCorrections, producing `the cat [//] the dog runs .` which
    /// `chatter validate` would accept.
    #[test]
    fn joins_full_correction_retrace_under_corrections_scope() {
        let input = doc("*CHI:\tthe cat [//] .\n*CHI:\tthe dog runs .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::RepetitionAndCorrections);
        assert_eq!(
            stats,
            JoinRetraceStats {
                joined_utterances: 1,
                needs_remorphotag: 0,
                dependent_tiers_dropped: 0,
            }
        );
        assert!(
            out.contains("*CHI:\tthe cat [//] the dog runs ."),
            "expected joined correction retrace, got:\n{out}"
        );
        assert_eq!(out.matches("*CHI:").count(), 1, "got:\n{out}");
    }

    /// The SAME `[//]` dangling case is NOT joined under RepetitionOnly (the
    /// default), confirming the gate is scope-controlled.
    #[test]
    fn does_not_join_full_correction_under_repetition_only() {
        let input = doc("*CHI:\tthe cat [//] .\n*CHI:\tthe dog runs .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::RepetitionOnly);
        assert!(stats.is_empty(), "stats: {stats:?}");
        assert_eq!(out, input);
    }

    /// A `[//]` dangling retrace with a DIFFERENT-speaker successor is NOT
    /// joined even under RepetitionAndCorrections.
    #[test]
    fn does_not_join_correction_different_speaker() {
        let input = doc("*CHI:\tthe cat [//] .\n*MOT:\tthe dog runs .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::RepetitionAndCorrections);
        assert!(stats.is_empty(), "stats: {stats:?}");
        assert_eq!(out, input);
    }

    /// Dependent tiers on a `[//]` join are dropped and flagged as
    /// needing re-morphotag.
    #[test]
    fn drops_dependent_tiers_on_correction_join() {
        let input = doc(
            "*CHI:\tthe cat [//] .\n%mor:\tdet:art|the noun|cat .\n*CHI:\tthe dog runs .\n%mor:\tdet:art|the noun|dog verb|run-3S .\n",
        );
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::RepetitionAndCorrections);
        assert_eq!(stats.joined_utterances, 1);
        assert_eq!(stats.needs_remorphotag, 1);
        assert_eq!(stats.dependent_tiers_dropped, 2);
        assert!(!out.contains("%mor:"), "tiers must be dropped, got:\n{out}");
        assert!(
            out.contains("*CHI:\tthe cat [//] the dog runs ."),
            "got:\n{out}"
        );
    }

    /// A dangling `[///]` multiple-retrace joins under RepetitionAndCorrections.
    #[test]
    fn joins_multiple_retrace_under_corrections_scope() {
        let input = doc("*CHI:\tgoing [///] .\n*CHI:\tI want to go .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::RepetitionAndCorrections);
        assert_eq!(stats.joined_utterances, 1);
        assert!(
            out.contains("*CHI:\tgoing [///] I want to go ."),
            "got:\n{out}"
        );
    }

    /// A dangling `[/-]` reformulation retrace joins under RepetitionAndCorrections.
    #[test]
    fn joins_reformulation_retrace_under_corrections_scope() {
        let input = doc("*CHI:\tand then [/-] .\n*CHI:\tso we went .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::RepetitionAndCorrections);
        assert_eq!(stats.joined_utterances, 1);
        assert!(
            out.contains("*CHI:\tand then [/-] so we went ."),
            "got:\n{out}"
        );
    }

    // --- Wave 3b: AllSameSpeakerSuccessor scope ---

    /// A non-repeating `[/]` successor (the successor does NOT begin with the
    /// retraced material) JOINS under `AllSameSpeakerSuccessor` but NOT under
    /// `RepetitionOnly` or `RepetitionAndCorrections`.
    ///
    /// The fixture uses "要 去 [/]" with successor "我 要 去 公 園": the
    /// retraced material is "要 去" but the successor leads with "我" (not
    /// "要"), so the prefix match fails, confirming a true non-repeat case.
    #[test]
    fn joins_nonrepeat_partial_retrace_under_all_scope() {
        let input = doc("*CHI:\t要 去 [/] .\n*CHI:\t我 要 去 公 園 .\n");
        let (out_rep, stats_rep) = join_with_scope(&input, RetraceJoinScope::RepetitionOnly);
        assert!(stats_rep.is_empty(), "RepetitionOnly must not join: {stats_rep:?}");
        assert_eq!(out_rep, input);

        let (out_cor, stats_cor) =
            join_with_scope(&input, RetraceJoinScope::RepetitionAndCorrections);
        assert!(
            stats_cor.is_empty(),
            "RepetitionAndCorrections must not join a non-repeat [/]: {stats_cor:?}"
        );
        assert_eq!(out_cor, input);

        let (out_all, stats_all) =
            join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert_eq!(
            stats_all.joined_utterances, 1,
            "AllSameSpeakerSuccessor must join: {stats_all:?}"
        );
        assert!(
            out_all.contains("*CHI:\t要 去 [/] 我 要 去 公 園 ."),
            "expected joined non-repeat [/], got:\n{out_all}"
        );
        assert_eq!(out_all.matches("*CHI:").count(), 1, "got:\n{out_all}");
    }

    /// A `[//]` correction still joins under `AllSameSpeakerSuccessor`.
    #[test]
    fn joins_full_correction_retrace_under_all_scope() {
        let input = doc("*CHI:\tthe cat [//] .\n*CHI:\tthe dog runs .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert_eq!(stats.joined_utterances, 1);
        assert!(
            out.contains("*CHI:\tthe cat [//] the dog runs ."),
            "got:\n{out}"
        );
    }

    /// A different-speaker successor is NEVER joined, even under
    /// `AllSameSpeakerSuccessor`.
    #[test]
    fn does_not_join_different_speaker_under_all_scope() {
        let input = doc("*CHI:\t要 去 [/] .\n*MOT:\t要 去 公 園 .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert!(stats.is_empty(), "different speaker must never join: {stats:?}");
        assert_eq!(out, input);
    }

    /// Dependent tiers are dropped and flagged for re-morphotag under
    /// `AllSameSpeakerSuccessor`.
    #[test]
    fn drops_dependent_tiers_under_all_scope() {
        let input = doc(
            "*CHI:\t要 去 [/] .\n%mor:\tverb|要 verb|去 .\n*CHI:\t我 要 去 公 園 .\n%mor:\tpro|我 verb|要 verb|去 noun|公園 .\n",
        );
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert_eq!(stats.joined_utterances, 1);
        assert_eq!(stats.needs_remorphotag, 1);
        assert_eq!(stats.dependent_tiers_dropped, 2);
        assert!(!out.contains("%mor:"), "tiers must be dropped, got:\n{out}");
    }
}
