# E311: Failed to parse utterance

## Description

Failed to parse utterance

## Metadata
- **Status**: implemented
- **Status note**: REACHABLE as of 2026-08-11, verified at the CLI. This note previously said E311 was unreachable because tree-sitter recovery wrapped the malformed utterance in an ERROR node and E316 (UnparsableContent) fired first. That is no longer true: the example now emits E311 ("Unclosed replacement bracket") plus E305, and no E316. The spec had stayed `not_implemented` with an example declaring E316, so its generated tests were `#[ignore]`d and the improvement went unchecked.

- **Error Code**: E311
- **Category**: Main tier validation
- **Level**: utterance
- **Layer**: parser
- **Kind**: Invalidity

## Example 1

**Source**: `E3xx_main_tier_errors/E311_failed_parse_utterance.cha`
**Trigger**: Severely malformed utterance that parser cannot handle
**Expected Error Codes**: E311

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	[: unclosed replacement [* error] .
@End
```

## Example 2

**Source**: `E3xx_main_tier_errors/E311_unclosed_replacement_mid_utterance.cha`
**Trigger**: Unclosed replacement bracket AFTER spoken material, rather than at utterance start
**Expected Error Codes**: E311

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [: world .
@End
```

Example 1 puts the unclosed `[:` at the start of the utterance; this one puts
it after a word. The two reach the classifier by different routes, and only the
utterance-initial one was covered, so a change that preserved Example 1 while
silently degrading this case to E316 (the generic "unparsable content"
catch-all) passed every gate. One example per code is coverage of the code, not
of the rule.

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
