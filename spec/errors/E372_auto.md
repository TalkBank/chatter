# E372: Nested quotation

Detected at ANY depth, through every container. Until 2026-08-07 the predicate
recursed into annotated groups only, so a quotation inside a retrace, a
phonological group, a sign group, or another quotation was invisible: examples
2 and 3 below reported nothing while example 1 reported E372.

## Description

Nested quotation

## Metadata

- **Error Code**: E372
- **Category**: validation
- **Level**: utterance
- **Layer**: validation
- **Kind**: Invalidity
- **Status**: implemented

## Example 1

**Source**: `validation_gaps/nested-quotation.cha`
**Trigger**: See example below
**Expected Error Codes**: E372

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@ID:	eng|corpus|MOT|30;00.|female|||Mother|||
*MOT:	she said “I told him “go away” yesterday” .
@Comment:	ERROR: Nested quotations - "go away" is inside "I told him..."
@Comment:	A state stack detects this; Rust only counts balance
*CHI:	okay mommy .
*MOT:	he said “hello” and “goodbye” .
@Comment:	VALID: Two separate quotations, not nested
@End
```

## Example 2: nesting below a retrace

**Trigger**: the inner quotation sits inside a retrace, not directly inside the outer quotation
**Expected Error Codes**: E372

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;00.00|male|||Target_Child|||
@Comment:	ERROR: the model is quotation -> retrace -> quotation
*CHI:	“a <“b”> [/] c” .
@End
```

## Example 3: nesting below a phonological group

**Trigger**: the inner quotation sits inside a phonological group
**Expected Error Codes**: E372

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;00.00|male|||Target_Child|||
@Comment:	ERROR: nesting is nesting at any depth, through any container
*CHI:	“a ‹“b”› c” .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
