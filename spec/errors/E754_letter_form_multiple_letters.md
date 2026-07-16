# E754: Letter form `@l` with more than one letter

## Description

The `@l` special form marks a single spoken LETTER (`b@l`, reading a
letter aloud). Multi-character content has its own form, `@k` (letter
sequence) or `@ls` (letter plural), so a stem of more than one
character under `@l` is a mis-marked form: `ab@l` should be `ab@k`.
Replicates CLAN CHECK error 76 ("There should be only one letter
before @l.", check.cpp `check_isOneLetter`), per maintainer ruling
2026-07-14: replicate CHECK's one-character rule now; the deeper
digraph question (Spanish `ch`, Dutch `ij`: one letter
orthographically, two characters) is logged for the corpus authority
and NOT decided here.

Wild-data grounding (2026-07-14 ruling): 98,325 single-letter `@l`
tokens vs 99 multi-letter, and the multi-letter set is confined to
dependent-tier and `[= ...]` gloss contexts that main-tier word
validation does not visit; the one main-tier near-miss
(a stuttered `↫b^↫b@l`) has a one-letter stem outside its repetition
span. This rule flags no kept corpus data (exhaustive typed scan over
7,335 candidate files, 2026-07-15).

## Metadata

- **Error Code**: E754
- **Category**: Main tier words
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: two characters of stem before `@l`.

**Expected Error Codes**: E754

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ab@l .
@Comment:	ERROR: multi-letter stem under the single-letter form
@End
```

## Expected Behavior

- **Parser**: Succeeds; `ab@l` parses as a word with
  `FormType::L`.
- **Validator**: Reports E754 at the word when a main-tier word with
  the `@l` form has a stem of more than one character (Unicode scalar
  count over text parts OUTSIDE segment-repetition spans, matching
  CHECK's UTF-8-aware single-letter scan).
- **Not flagged**: `b@l` (one letter); `abc@k` / `abc@ls`
  (letter-sequence forms); the stuttered letter `↫b^↫b@l`
  (repeated-segment material is not stem; real CLAN CHECK accepts it,
  verified in file mode, and the wild fluency data contains it);
  multi-letter `@l` inside dependent tiers or `[= ...]` glosses
  (outside main-tier word validation, matching CHECK's main-tier
  scope).

## CHAT Rule

`@l` marks one letter; sequences use `@k`/`@ls`. See the CHAT manual
on special form markers. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 76.
