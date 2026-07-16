# E756: Empty user-defined tier

## Description

A user-defined `%x` tier whose content is empty or whitespace-only
declares nothing: the line asserts an annotation that is not there and
fails to make sense. Formerly W601, which carried a warning-prefixed
code while firing as a hard error (its doc comment even said
"intentionally warning-level"); the maintainer ruling of 2026-07-16
resolved the taxonomy contradiction by keeping the rejection and
giving it an honest E-number. Real CLAN has no analogue (a truly
empty tier draws only structural errors); zero kept files carry the
construct, so the rename has no corpus impact. W601 is retired and
not reused.

## Metadata

- **Error Code**: E756
- **Category**: Dependent tier validation
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: whitespace-only content on a custom `%x` tier.

**Expected Error Codes**: E756

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
%xtst:	 
@End
```

## Expected Behavior

- **Parser**: Succeeds; the user-defined tier parses with whitespace
  content.
- **Validator**: Reports E756 at the tier.

## CHAT Rule

User-defined tiers exist to carry custom annotation content; an empty
one is a defect in the transcript.
