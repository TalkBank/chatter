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
//! - [`RetraceJoinScope::RepetitionAndCorrections`] (Wave 3a, opt-in): joins
//!   `[/]` under the SAME verified-repeat gate as `RepetitionOnly` (corrections
//!   never loosens the `[/]` rule), AND additionally joins correction retraces
//!   (`[//]` Full, `[///]` Multiple, `[/-]` Reformulation). Corrections REPLACE
//!   rather than repeat, so for those kinds there is no leading-words repeat
//!   check and no pure-word requirement; the gate is a same-speaker successor.
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
    BracketedContent, BracketedItem, Bullet, ChatFile, Line, MainTier, RetraceKind, TierContent,
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
        // Do NOT advance `i`: re-examine the merged line. If the successor V
        // was ITSELF a dangling retrace (a chain of same-speaker dangling
        // retraces), the merged line is still dangling and must be joined
        // again so the whole chain collapses in one pass. Each join removes
        // exactly one line, so `i` only stays put while joins keep happening;
        // the loop still terminates (a self-join is impossible because the
        // successor is always a strictly later line).
    }

    stats
}

/// If the utterance at `start_index` is a qualifying dangling retrace, return
/// the successor's line index.
///
/// The `scope` controls which retrace kinds are eligible and what material
/// check is required:
///
/// - For [`RetraceJoinScope::RepetitionOnly`], only `[/]` qualifies, and only
///   when the retraced material is a pure plain-word sequence AND the
///   successor's leading words repeat it as a prefix.
/// - For [`RetraceJoinScope::RepetitionAndCorrections`], `[/]` keeps the SAME
///   verified-repeat gate as `RepetitionOnly` (corrections does not loosen the
///   `[/]` rule); additionally `[//]`/`[///]`/`[/-]` qualify, and those need no
///   prefix check (corrections replace rather than repeat).
/// - For [`RetraceJoinScope::AllSameSpeakerSuccessor`], any kind qualifies;
///   neither a material purity check nor a prefix match is required.
///
/// Independent of scope, the join is refused when the successor is not the
/// immediately following line (crossing an interstitial `@`-header), when the
/// speakers differ, or when the successor carries leading linkers or an
/// utterance-scoped language code (which inlining would silently drop).
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
    let (kind, opt_material) = dangling_retrace_kind(&u.main, scope)?;

    // The successor must be the IMMEDIATELY following line and an utterance.
    // Refusing to cross an interstitial `@`-header keeps the repair from
    // silently moving a gem/comment past the content it scoped (see
    // [`immediate_successor_utterance`]).
    let v_index = immediate_successor_utterance(lines, start_index)?;
    let Line::Utterance(v) = &lines[v_index] else {
        return None;
    };

    // Same speaker.
    if v.main.speaker != u.main.speaker {
        return None;
    }

    // Conservative successor-attribute gate: the join appends V's content
    // INLINE after U's retrace marker. V's leading linkers (`++`, `+<`, ...)
    // and utterance-scoped language code (`[- code]`) cannot be re-expressed
    // mid-utterance, so a join would silently drop them (a `[- spa]`
    // continuation would be relabeled to the default language). Refuse the
    // join when V carries either; such cases are left for manual review.
    if v.main.content.language_code.is_some() || !v.main.content.linkers.is_empty() {
        return None;
    }

    // Partial retrace (`[/]`) is a REPETITION marker: it requires a verifiable
    // pure-word repeat under BOTH `RepetitionOnly` and `RepetitionAndCorrections`.
    // `corrections` ADDS the correction kinds; it never loosens the `[/]` rule
    // (that would break the `repetition` ⊂ `corrections` ⊂ `all` ladder, letting
    // `corrections` join a `[/]` that `repetition` refuses). A non-pure-word
    // `[/]` (`opt_material` is None) cannot be verified, so it is deferred to
    // `AllSameSpeakerSuccessor`. Under `all`, any same-speaker successor is
    // accepted regardless of material purity.
    //
    // Corrections (Full / Multiple / Reformulation) REPLACE rather than repeat,
    // so they need neither a prefix match nor pure-word material under any
    // corrections-enabled scope.
    match kind {
        RetraceKind::Partial => match scope {
            RetraceJoinScope::RepetitionOnly | RetraceJoinScope::RepetitionAndCorrections => {
                let material = opt_material?;
                if material.is_empty() {
                    return None;
                }
                if !leading_words_match_prefix(&v.main, &material) {
                    return None;
                }
            }
            RetraceJoinScope::AllSameSpeakerSuccessor => {
                // No prefix check: any same-speaker successor is accepted,
                // even when the retraced material is non-pure-word.
            }
        },
        RetraceKind::Full | RetraceKind::Multiple | RetraceKind::Reformulation => {
            // Corrections: no prefix match required under any corrections-enabled
            // scope; pure-word material is not required either.
        }
    }

    Some(v_index)
}

/// Returns the retrace kind and optionally the retraced material's lexical-word
/// `cleaned_text` sequence when the main tier's LAST content node is a
/// dangling retrace of a kind allowed by `scope`; otherwise `None`.
///
/// "Dangling" means the retrace marker is the last content node (nothing after
/// it on the main tier).
///
/// The material (`Vec<String>`) is extracted only when the retrace content is a
/// PURE plain-word sequence. If the content contains non-word items (pauses,
/// events, error markers, etc.), the material slot is `None`. The caller
/// decides whether a `None` material disqualifies the join: for
/// `RepetitionOnly` it does (prefix-match is impossible); for the broader
/// scopes it does not (those never read the material).
fn dangling_retrace_kind(
    main: &MainTier,
    scope: RetraceJoinScope,
) -> Option<(RetraceKind, Option<Vec<String>>)> {
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

    // Attempt pure-word extraction; a None result is not fatal here.
    let opt_words = pure_word_sequence_from_bracketed(&retrace.content);
    Some((retrace.kind, opt_words))
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

/// Return the index of the utterance IMMEDIATELY following `from_index`.
///
/// The successor must be the very next line AND a `Line::Utterance`. If the
/// next line is a `Line::Header` (any `@`-header: a gem marker `@Bg`/`@Eg`/`@G`,
/// `@Situation`, `@Comment`, ...), the join is REFUSED by returning `None`.
///
/// Joining across a header would silently move that header past the content it
/// scoped, or pull the successor out of a gem region (leaving an empty
/// `@Bg`/`@Eg` pair). Both produce a structurally wrong file that still parses,
/// so `validate`-after would not catch it. A conservative OBVIOUS-only repair
/// never crosses a header boundary; such cases are left for manual review.
///
/// Dependent tiers (`%mor`, `%gra`, `%com`, ...) are NOT `Line`s (they live on
/// `Utterance.dependent_tiers`), so the only thing that can sit between two
/// utterance lines is an `@`-header; refusing to cross one is exactly right.
fn immediate_successor_utterance(lines: &[Line], from_index: usize) -> Option<usize> {
    match lines.get(from_index + 1)? {
        Line::Utterance(_) => Some(from_index + 1),
        Line::Header { .. } => None,
    }
}

/// Perform the join of utterance at `u_index` with utterance at `v_index`.
///
/// Preconditions (established by [`obvious_join_target`]): both indices are
/// `Line::Utterance`, V is the immediate successor of U, U's last content is a
/// dangling retrace allowed by the scope, same speaker, and V carries no
/// leading linkers or language code. `v_index > u_index`. The endpoint kinds
/// are re-checked here so a violated precondition fails closed.
fn perform_join<S: ValidationState>(
    chat: &mut ChatFile<S>,
    u_index: usize,
    v_index: usize,
    stats: &mut JoinRetraceStats,
) {
    // Validate BOTH endpoints are utterances BEFORE any mutation. The
    // preconditions established by `obvious_join_target` make this guard
    // unreachable today, but checking up front means a future refactor that
    // violates them fails CLOSED (a clean no-op) rather than half-applying the
    // join: removing V without merging it into U would be silent data loss.
    let both_utterances = matches!(chat.lines.get(u_index), Some(Line::Utterance(_)))
        && matches!(chat.lines.get(v_index), Some(Line::Utterance(_)));
    if !both_utterances {
        return;
    }

    // Remove V first (higher index) so U's index stays valid. The guard above
    // guarantees this is an utterance.
    let Line::Utterance(v) = chat.lines.remove(v_index) else {
        return;
    };
    let v = *v;

    let Some(Line::Utterance(u)) = chat.lines.get_mut(u_index) else {
        return;
    };

    // Count dependent tiers that will be dropped (from BOTH sides). U's own
    // dependent tiers are dropped because the joined main tier no longer
    // aligns with them; V's are dropped for the same reason. Read before we
    // consume `v.main.content` below.
    let dropped = u.dependent_tiers.len() + v.dependent_tiers.len();

    // Exhaustively destructure V's tier content so any FUTURE `TierContent`
    // field becomes a compile error here instead of another silent drop.
    // `obvious_join_target` guarantees V carries no leading linkers and no
    // language code (a join cannot re-express either mid-utterance), so those
    // are ignored rather than merged; `content_span` is a diagnostic-only span
    // that does not survive serialization.
    let TierContent {
        linkers: _,
        language_code: _,
        content: mut v_items,
        terminator: v_terminator,
        postcodes: v_postcodes,
        bullet: v_bullet,
        content_span: _,
        language_code_span: _,
    } = v.main.content;

    // Union the main-tier time bullets: start from U, end from V.
    let unioned_bullet = union_bullets(u.main.content.bullet.as_ref(), v_bullet.as_ref());

    // Append V's content onto U's (U keeps its trailing retrace marker).
    u.main.content.content.0.append(&mut v_items.0);

    // The joined utterance is terminated by V's terminator.
    u.main.content.terminator = v_terminator;

    // V's postcodes follow the joined content's terminator.
    u.main.content.postcodes.0.extend(v_postcodes.0);

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
        assert!(
            stats_rep.is_empty(),
            "RepetitionOnly must not join: {stats_rep:?}"
        );
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
        assert!(
            stats.is_empty(),
            "different speaker must never join: {stats:?}"
        );
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

    // --- Wave 3c: relaxed pure-word material gate ---

    /// A dangling `[/]` whose retraced material contains an embedded pause
    /// (non-pure-word) JOINS under `AllSameSpeakerSuccessor` but is NOT
    /// joined under `RepetitionOnly` (which needs pure words for prefix-match).
    ///
    /// Fixture: `<the (.) dog> [/]` with successor `the dog runs .`
    /// The embedded `(.)` pause makes `pure_word_sequence_from_bracketed`
    /// return `None`, so the current code skips this in ALL scopes.
    /// After the fix, corrections/all scopes join it; repetition still skips.
    #[test]
    fn joins_partial_retrace_with_pause_material_under_all_scope() {
        let input = doc("*CHI:\t<the (.) dog> [/] .\n*CHI:\tthe dog runs .\n");

        // RepetitionOnly must NOT join: it needs pure-word material for prefix-match.
        let (out_rep, stats_rep) = join_with_scope(&input, RetraceJoinScope::RepetitionOnly);
        assert!(
            stats_rep.is_empty(),
            "RepetitionOnly must not join non-pure-word material: {stats_rep:?}"
        );
        assert_eq!(out_rep, input);

        // AllSameSpeakerSuccessor MUST join: material is irrelevant for this scope.
        let (out_all, stats_all) =
            join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert_eq!(
            stats_all.joined_utterances, 1,
            "AllSameSpeakerSuccessor must join [/] with non-pure-word material: {stats_all:?}"
        );
        assert!(
            out_all.contains("*CHI:\t<the (.) dog> [/] the dog runs ."),
            "expected joined output, got:\n{out_all}"
        );
        assert_eq!(out_all.matches("*CHI:").count(), 1, "got:\n{out_all}");
    }

    /// A dangling `[//]` whose retraced material contains an embedded pause
    /// JOINS under `RepetitionAndCorrections` and `AllSameSpeakerSuccessor`,
    /// but NOT under `RepetitionOnly`.
    ///
    /// Fixture: `<my (.) falled> [//]` with successor `I fell down .`
    #[test]
    fn joins_correction_retrace_with_pause_material_under_corrections_scope() {
        let input = doc("*CHI:\t<my (.) falled> [//] .\n*CHI:\tI fell down .\n");

        // RepetitionOnly must NOT join corrections at all.
        let (out_rep, stats_rep) = join_with_scope(&input, RetraceJoinScope::RepetitionOnly);
        assert!(
            stats_rep.is_empty(),
            "RepetitionOnly must not join [//]: {stats_rep:?}"
        );
        assert_eq!(out_rep, input);

        // RepetitionAndCorrections MUST join: corrections skip the prefix-match
        // and the material gate should not apply.
        let (out_cor, stats_cor) =
            join_with_scope(&input, RetraceJoinScope::RepetitionAndCorrections);
        assert_eq!(
            stats_cor.joined_utterances, 1,
            "RepetitionAndCorrections must join [//] with non-pure-word material: {stats_cor:?}"
        );
        assert!(
            out_cor.contains("*CHI:\t<my (.) falled> [//] I fell down ."),
            "expected joined correction, got:\n{out_cor}"
        );
        assert_eq!(out_cor.matches("*CHI:").count(), 1, "got:\n{out_cor}");

        // AllSameSpeakerSuccessor MUST also join.
        let (out_all, stats_all) =
            join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert_eq!(
            stats_all.joined_utterances, 1,
            "AllSameSpeakerSuccessor must join [//] with non-pure-word material: {stats_all:?}"
        );
        assert!(
            out_all.contains("*CHI:\t<my (.) falled> [//] I fell down ."),
            "got:\n{out_all}"
        );
    }

    /// A different-speaker successor with non-pure-word material is NEVER joined,
    /// even under `AllSameSpeakerSuccessor`.
    #[test]
    fn does_not_join_pause_material_retrace_different_speaker() {
        let input = doc("*CHI:\t<the (.) dog> [/] .\n*MOT:\tthe dog runs .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert!(
            stats.is_empty(),
            "different speaker must never join: {stats:?}"
        );
        assert_eq!(out, input);
    }

    // --- Review fixes (2026-06-24): chain collapse, header boundary,
    //     successor attributes, scope-ladder monotonicity ---

    /// A CHAIN of same-speaker dangling retraces collapses FULLY in one pass.
    /// `the [/]` -> `the dog [/]` -> `the dog runs`: joining the first two
    /// yields `the [/] the dog [/]`, whose last node is STILL a dangling
    /// retrace. The transform must re-examine the merged line and join the
    /// third utterance too, leaving exactly one utterance (no residual E370).
    #[test]
    fn chain_of_dangling_retraces_collapses_in_one_pass() {
        let input = doc("*CHI:\tthe [/] .\n*CHI:\tthe dog [/] .\n*CHI:\tthe dog runs .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert_eq!(
            stats.joined_utterances, 2,
            "both joins must happen in one pass: {stats:?}\n{out}"
        );
        assert_eq!(
            out.matches("*CHI:").count(),
            1,
            "chain must collapse to ONE utterance, got:\n{out}"
        );
        assert!(
            out.contains("*CHI:\tthe [/] the dog [/] the dog runs ."),
            "got:\n{out}"
        );
    }

    /// A repetition chain also collapses under the DEFAULT scope when each
    /// level prefix-matches: `the [/]` -> `the [/]` -> `the dog`.
    #[test]
    fn repetition_chain_collapses_under_default_scope() {
        let input = doc("*CHI:\tthe [/] .\n*CHI:\tthe [/] .\n*CHI:\tthe dog .\n");
        let (out, stats) = join(&input);
        assert_eq!(stats.joined_utterances, 2, "{stats:?}\n{out}");
        assert_eq!(out.matches("*CHI:").count(), 1, "got:\n{out}");
    }

    /// A join is REFUSED across an intervening `@`-header (gem / comment):
    /// the successor must be the IMMEDIATELY following line. Crossing a header
    /// would silently move it past the content it scoped (or pull V out of a
    /// gem), producing a structurally wrong file that still parses.
    #[test]
    fn does_not_join_across_intervening_header() {
        let input = doc("*CHI:\tthe dog [/] .\n@Comment:\tchild paused\n*CHI:\tthe dog runs .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert!(
            stats.is_empty(),
            "must not join across a header: {stats:?}\n{out}"
        );
        assert_eq!(
            out.matches("*CHI:").count(),
            2,
            "both utterances must remain, got:\n{out}"
        );
    }

    /// A join is REFUSED when the successor carries an utterance-scoped
    /// language code (`[- code]`); inlining it after the retrace marker would
    /// silently relabel the continuation's language. Verified under all-scope
    /// so only the successor-attribute gate can block it.
    #[test]
    fn does_not_join_when_successor_has_language_code() {
        let input = doc("*CHI:\tthe dog [/] .\n*CHI:\t[- spa] el perro corre .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert!(
            stats.is_empty(),
            "must not join a [- code] successor: {stats:?}\n{out}"
        );
        assert_eq!(out.matches("*CHI:").count(), 2, "got:\n{out}");
    }

    /// A join is REFUSED when the successor begins with a linker (`++`, `+<`,
    /// ...); inlining it mid-utterance would silently drop the linker.
    #[test]
    fn does_not_join_when_successor_has_linker() {
        let input = doc("*CHI:\tthe dog [/] .\n*CHI:\t++ the dog runs .\n");
        let (out, stats) = join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert!(
            stats.is_empty(),
            "must not join a ++ linker successor: {stats:?}\n{out}"
        );
        assert_eq!(out.matches("*CHI:").count(), 2, "got:\n{out}");
    }

    /// Scope-ladder monotonicity: a NON-pure-word `[/]` (material contains a
    /// pause) with a NON-repeating successor is NOT joined under `corrections`
    /// (corrections does not loosen the `[/]` repeat rule; it only adds the
    /// correction kinds). It joins ONLY under `all`.
    #[test]
    fn nonpureword_partial_not_joined_under_corrections_scope() {
        let input = doc("*CHI:\t<the (.) dog> [/] .\n*CHI:\twhat happened next .\n");

        let (out_cor, stats_cor) =
            join_with_scope(&input, RetraceJoinScope::RepetitionAndCorrections);
        assert!(
            stats_cor.is_empty(),
            "corrections must NOT join a non-pure-word [/]: {stats_cor:?}\n{out_cor}"
        );
        assert_eq!(out_cor, input);

        let (_out_all, stats_all) =
            join_with_scope(&input, RetraceJoinScope::AllSameSpeakerSuccessor);
        assert_eq!(
            stats_all.joined_utterances, 1,
            "all-scope must still join it: {stats_all:?}"
        );
    }
}
