# E756: Empty user-defined tier

**Last updated:** 2026-07-19 08:30 EDT

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

- **Parser**: Succeeds. Under the TierSeparator model the tier separator
  absorbs the lone trailing space, so the `%x` tier body canonicalizes to
  empty (no `text_with_bullets` child). The grammar makes ONLY the
  user-defined tier's body optional, so this parses cleanly as an empty
  user-defined tier rather than recovering with spurious E342/E330.
- **Validator**: Reports E756 at the tier (the sole emitted code).

## CHAT Rule

User-defined tiers exist to carry custom annotation content; an empty
one is a defect in the transcript.
