# E342: Missing required element

## Description

Missing required element

## Metadata
- **Status**: implemented
- **Last updated**: 2026-08-11 16:20 EDT

- **Error Code**: E342
- **Category**: Word validation
- **Level**: utterance
- **Layer**: parser
- **Kind**: Invalidity

## Example 1

**Source**: `E2xx_word_errors/E211_replacement_missing_corrected.cha`
**Trigger**: Replacement containing 0 (omission marker)
**Expected Error Codes**: E342

Note: `helo [: 0] world .` is malformed at BOTH layers, and this is a
parser-layer spec, so it declares the parser-layer code. The grammar has no
word_segment for the `0`, so tree-sitter recovers with a MISSING node and the
parser reports E342 ("recovery is not validity"). Validation then separately
reports E390 (ReplacementContainsOmission), which is the more informative
diagnosis and is declared by E390's own spec; a parser-layer test cannot see it,
because it only inspects parse diagnostics.

Updated 2026-08-11: this declared E390 alone, and had been unreachable since
whenever E342_auto's status became `implemented` without the generated tests
being regenerated, so four E342 tests sat `#[ignore]`d and nothing noticed the
expectation was for a layer this test does not run.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	helo [: 0] world .
@End
```

## Example 2

**Source**: `E7xx_tier_parsing/E704_empty_mor_pos.cha`
**Trigger**: %mor chunk with empty part-of-speech before pipe
**Expected Error Codes**: E760

Updated 2026-08-11. This declared E316 (unparsable content) and E702 (invalid
morphology format), and emits neither: it now emits E760, which names the
actual defect ("MOR item '|hello' has an empty part-of-speech field") and
carries the rule in its help text. A generic "unparsable content" standing in
for a specific rule is the documented tell of a validator that has not yet been
taught the rule, so replacing it was the improvement; the expectation here was
simply left behind. It also emits E600 as a WARNING, saying why main-to-%mor
alignment was skipped, which is a consequence rather than a second defect and
so is not declared.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	|hello n|world .
@End
```

## Example 3

**Source**: `E7xx_tier_parsing/E703_empty_mor_stem.cha`
**Trigger**: %mor chunk with empty stem after pipe
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	v| n|world .
@End
```

## Example 4

**Source**: `E7xx_tier_parsing/E711_gra_missing_role.cha`
**Trigger**: %gra relation with empty role field
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%gra:	1|2| 2|0|ROOT
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
