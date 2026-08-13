# E255: Whole-utterance language switch should use precode

## Description

Every lexical (and filler/nonword) item in an utterance carries a per-word
`@s` language marker that resolves to the same single non-default language.
When an entire utterance switches language, CHAT provides the utterance
precode `[- LANG]` for exactly this case; tagging every word individually
with `@s` instead of using the precode is flagged so the file is rewritten
into the operationally correct form.

## Metadata

- **Error Code**: E255
- **Category**: main_tier_validation
- **Level**: main_tier
- **Layer**: validation
- **Kind**: Invalidity
- **Status**: implemented

## Example 1: Two-word whole-utterance switch

**Trigger**: Every word in the utterance carries `@s` and resolves to the
same non-default language
**Expected Error Codes**: E255

```chat
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hola@s amiga@s .
@End
```

## Example 2: Single-word switch

**Trigger**: A one-word utterance whose one word carries `@s`. E255
deliberately fires here too (maintainer ruling, 2026-07-30, superseding an
earlier 2-word threshold that was implemented and reverted): linguistically
whether a lone tagged word is an insertion or a whole-utterance switch is a
judgment call that cannot be formalized, so the tiebreak is operational.
The Batchalign morphotag pipeline routes `[- LANG]`-precoded utterances
wholesale to that language's Stanza model, while `@s`-tagged words go
through its L2 splice machinery, which assumes an `@s` span is a proper
SUBSET of the utterance; a whole-utterance `@s` (one word is the degenerate
case) exercises that machinery's unsupported shape. E255 and `chatter debug
fix-s` share the same detection predicate, so both keep the one-word
behavior together.
**Expected Error Codes**: E255

```chat
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	si@s .
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E255, suggesting the utterance be rewritten
  as `[- LANG] ...` with the per-word `@s` markers removed

## CHAT Rule

The `[- LANG]` utterance precode declares whole-utterance language scope
and is the correct notation when every word in the utterance is in the
same non-default language; per-word `@s` is for a genuine sub-utterance
insertion. See CHAT manual on language marking:
<https://talkbank.org/0info/manuals/CHAT.html>

## Notes

- Detection seam: `MainTier::whole_utterance_language_switch_target` in
  `crates/talkbank-model/src/model/content/main_tier/language_switch.rs`,
  shared by this validator and the `chatter debug fix-s` rewrite tool so
  both apply the same rule.
- Emit site: `crates/talkbank-model/src/model/content/main_tier/mod.rs`.
- Collects every word-bearing item (including fillers `&~`/`&-`/`&+` and
  retrace content), not just `%mor`-bearing ones; refuses to fire if any
  uttered word lacks an explicit language attribution.
- Pinned by CLI tests in
  `crates/chatter/tests/integration/command_matrix_tests.rs`
  (`validate_flags_single_word_language_switch_with_e255`,
  `debug_fix_s_rewrites_single_word_switch_to_precode`).
