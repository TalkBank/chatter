# E242: Unbalanced quotation marks

**Last updated:** 2026-07-19 08:30 EDT

## Description

Quotation marks must be balanced within an utterance.

## Metadata

- **Error Code**: E242
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E242_unbalanced_quotation.cha`
**Trigger**: Unbalanced opening quote; improved recovery now surfaces the quotation-balance check directly
**Expected Error Codes**: E242

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Quotation marks must be balanced
@Comment:	Invalid: '"hello' - Missing closing quote
*CHI:	"hello .
@End
```

## Expected Behavior

The validator rejects this CHAT input and reports E242 "Unbalanced quotation
in word content" at the unbalanced opening quote. On this malformed line the
tree-sitter recovery also surfaces incidental recovery-noise diagnostics
(E305 missing terminator, E306, E342 missing-required-via-recovery); the
characterizing, semantically meaningful code for the construct is E242.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- E242 is emitted during utterance quotation validation (model layer). Historically this fixture could NOT trigger E242 from standalone CHAT input: tree-sitter absorbed the unbalanced `"hello` into a single ERROR node and produced E316 (generic unparsable content) before quotation validation ran, so E316 was the recorded expectation. The TierSeparator refactor (2026-07-19) improved recovery so the malformed line recovers more granularly and the quotation-balance check in `quotation.rs` now fires directly, yielding E242 (plus incidental recovery noise: E305, E306, E342). E316 is no longer emitted on this fixture. The observed code set is {E242, E305, E306, E342}; E242 is the characterizing expectation. This E316 -> E242 reclassification was accepted by maintainer ruling (recovery is not to be dampened).
- Review and enhance this specification as needed
