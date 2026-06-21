# E722: GRA has no ROOT

**Last modified:** 2026-05-30 19:04 EDT

**Last updated:** 2026-04-04 08:15 EDT

## Description

`%gra` tier has no ROOT relation. Every `%gra` tier must have exactly one relation with `head=0` or `head=self` (the ROOT of the dependency tree).

## Metadata

- **Error Code**: E722
- **Category**: validation
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Source**: `error_corpus/validation_errors/E722_gra_no_root.cha`
**Trigger**: `%gra` has no non-terminator ROOT relation while `%mor/%gra` counts still match
**Expected Error Codes**: E722

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I want .
%mor:	pro|I v|want .
%gra:	1|2|SUBJ 2|3|OBJ 3|0|PUNCT
@End
```

## Expected Behavior

The validator should report E722 because the only root relation is the final
terminator `PUNCT`, which is excluded from the non-terminator ROOT count.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on dependent tier formats (%mor, %gra, %pho, etc.). Each tier type has specific syntax requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The example must keep `%mor/%gra` counts aligned; otherwise E720 masks the
  rootless-graph condition before E722 can fire.
- A rootless `%gra` tier without a cycle is only possible when the terminator
  `PUNCT` relation is the sole `head=0` relation.
- E722 is emitted as a Warning (not Error) since 2026-02-14 due to non-conforming corpus data.
- Review and enhance this specification as needed
