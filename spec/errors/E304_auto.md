# E304: Missing speaker code

**Last modified:** 2026-05-30 19:04 EDT

## Description

Main tier line is missing its speaker code after `*`.

## Metadata

- **Status**: not_implemented
- **Error Code**: E304
- **Category**: Main tier validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `synthetic missing-speaker recovery case`
**Trigger**: Main tier begins with `*:` instead of `*SPK:`
**Expected Error Codes**: E301

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*:	hello .
@End
```

## Expected Behavior

The current full-file tree-sitter recovery path does not reach a distinct E304
diagnostic for this malformed line. Instead it currently reports E301
(`MissingMainTier`) for `*:`-style inputs and E316 for some nearby malformed
variants.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Marked `not_implemented` for full-file spec coverage because current
  tree-sitter recovery does not surface E304 from raw CHAT examples.
- Review and enhance this specification as needed
