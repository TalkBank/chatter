# E766: linker not utterance-initial

## Description

Linkers (`+"`, `++`, `+<`, `+^`, `+,`, `+≈`, `+≋`) connect an utterance to the
PREVIOUS utterance, so they are utterance-initial by definition:

```
*CHI:	+" yes I do .
```

A linker placed after content is meaningless: there is nothing for it to link.
Before this rule, the construct did not parse at all and surfaced as generic
unparsable content (E316), a message that gives the transcriber nothing to act
on (the generic-code-instead-of-a-named-rule shape the adjudication policy
flags as a validator defect; IISRP residue finding 5, 2026-07-30).

The grammar now deliberately parses a misplaced linker into the CST as a
content item (the strict+catch-all pattern, same as `illegal_curly_quote`), so
this rule can name the construct and locate the exact token. The misplaced
linker produces no model element: the item is rejected, the surrounding
content parses normally, and the file is invalid.

Linkers in the legal utterance-initial run (including several in a row) are
unaffected. A linker after the terminator is not a content item and stays on
the generic unparsable path in both parser front ends.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-07-30 08:47 EDT

- **Error Code**: E766
- **Category**: Main tier structure
- **Level**: utterance
- **Layer**: parser
- **Kind**: Invalidity

## Example 1

**Trigger**: a quotation-follows linker after content.

**Expected Error Codes**: E766

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	yeah that go +" okay .
@Comment:	ERROR: the linker must open the utterance
@End
```

## Example 2

**Trigger**: a quick-uptake linker between words.

**Expected Error Codes**: E766

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I know ++ and then .
@Comment:	ERROR: the linker must open the utterance
@End
```

## Example 3

**Trigger**: a lazy-overlap linker between words.

**Expected Error Codes**: E766

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello +< there .
@Comment:	ERROR: the linker must open the utterance
@End
```

## Expected Behavior

- **Parser (tree-sitter)**: the grammar parses the misplaced linker as a
  `content_item` alternative (`prec(-1, $.linker)`; the initial run keeps
  reducing into the `linkers` field via `prec.right`). The lowering reports
  E766 at the linker's span and rejects that item only; no model element is
  produced and the surrounding content parses normally.
- **Parser (re2c)**: the token front end reports E766 for each linker token
  found after the first non-linker item on a main tier line (stopping at the
  terminator) and strips it before the combinator parse, the same
  report-and-strip treatment as `illegal_curly_quote`, so no generic E321
  co-fires.
- **Validator**: nothing additional; the diagnostic is parser-emitted, and
  parse rejection taints the file invalid.

## CHAT Rule

Utterance linkers are defined as utterance-initial marks: they tie the
current utterance to the previous one (completion, uptake, quotation). CHAT
provides no meaning for a linker in any other position, so a non-initial
linker fails to make sense and is invalid.
