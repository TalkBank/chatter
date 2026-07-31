# E747: Blank line not allowed

## Description

CHAT does not allow blank lines anywhere in the transcript (CLAN CHECK 91).
The grammar represents a blank line as a structural `blank_line` node (a
lone newline at a line boundary), so the parser emits this diagnostic
directly from the tree rather than by scanning the source text.

## Metadata

- **Error Code**: E747
- **Category**: parser
- **Level**: file
- **Layer**: parser
- **Kind**: Invalidity

## Example 1: Blank line between utterances

**Trigger**: A lone blank line separates two `*CHI:` utterances
**Expected Error Codes**: E747

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hi .

*CHI:	bye .
@End
```

## Expected Behavior

- **Parser**: Should succeed (recovers around the blank line; neither
  surrounding utterance is dropped from the model)
- **Validator**: Should report E747 at the blank line's own span, with the
  message "Blank lines are not allowed"

## CHAT Rule

Blank lines are not permitted anywhere in a CHAT transcript. See CHAT
manual on file structure: <https://talkbank.org/0info/manuals/CHAT.html>

## Notes

- Emit site: `crates/talkbank-parser/src/parser/chat_file_parser/chat_file/document_lowering.rs`.
- Only a `blank_line` node that sits at a genuine line boundary (starts the
  file, or is immediately preceded by another newline) is reported; under
  error recovery a `blank_line` node can otherwise cover just the trailing
  newline of a non-blank malformed line, and reporting E747 there would
  point the reader at a blank line that does not exist.
- Pinned end-to-end (exact byte span and message) by
  `blank_line_between_utterances_produces_e747` in
  `crates/talkbank-parser/tests/integration/line_dispatch_characterization.rs`.
