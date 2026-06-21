# E253: Empty word content

**Last modified:** 2026-05-30 19:04 EDT

## Description

A parsed Word object has empty content, the word node exists in the CST but contains no text.

## Metadata
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E253
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E253_empty_word_content.cha`
**Trigger**: Utterance with only whitespace before terminator, current parser recovery yields an empty utterance
**Expected Error Codes**: E306

Note: The example produces E306 (EmptyUtterance) rather than E253
(EmptyWordContent, a word-level validation). The current parser recovery
collapses whitespace-only content into a terminator-only utterance, so the
validator sees “no content” at the main-tier level instead of a parsed word
node with empty text.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Word must have content
@Comment:	Invalid: Empty word element
*CHI:	  .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
