# E305: Missing terminator

**Last modified:** 2026-05-30 19:04 EDT

## Description

Main tier is missing its required utterance terminator.

## Metadata

- **Error Code**: E305
- **Category**: Main tier validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `E3xx_main_tier_errors/E305_missing_terminator.cha`
**Trigger**: Main tier has spoken content but no terminator
**Expected Error Codes**: E305

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world
@End
```

## Expected Behavior

The parser should recover the utterance content, and validation should report
that the utterance is missing a terminator.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
