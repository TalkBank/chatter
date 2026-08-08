# E377, A retracing marker with no material of its own

**Status:** Current
**Last updated:** 2026-08-07 11:20 EDT

## Description

A retracing marker whose only content is another retracing marker, so it has no
material of its own to retrace. A marker retraces the words immediately to its
left, and a marker is not words.

Two surface spellings, one shape. The lowering folds a marker run into a
left-associative chain, so both produce a retrace whose content is a lone
retrace and one rule catches both:

```text
на [//] [/] на        unbracketed, 105 occurrences
<<a> [/]> [//] b      bracketed, 4 occurrences
```

The combined form IS legal when material sits between the markers, which is how
the CHAT manual presents it:

```text
*CHI:	<the fish is> [//] the [/] the fish are swimming .
```

There the `[/]` refers to the preceding word `the`. Only the bare adjacency is
an error.

## Metadata

- **Error Code**: E377
- **Category**: retrace
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented
- **Kind**: Invalidity

## Example 1

**Trigger**: `[//]` immediately followed by `[/]` on a single word
**Expected Error Codes**: E377

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: the second marker has nothing before it to retrace
*CHI:	the [//] [/] the dog .
@End
```

## Example 2

**Trigger**: two markers after a bracketed group
**Expected Error Codes**: E377

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: the second marker has nothing before it to retrace
*CHI:	<the dog> [/] [//] the cat ran .
@End
```

## Example 3

**Trigger**: the bracketed spelling of the same shape
**Expected Error Codes**: E377

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: the outer marker retraces nothing but the inner marker
*CHI:	<<a> [/]> [//] b .
@End
```

## Expected Behavior

VALIDATION reports E377; parsing is unaffected and keeps both markers, so the
utterance still lowers and still round-trips.

(This paragraph said "Parsing reports E377" until 2026-08-07. It does not: the
fold is in the lowering, the report is a validation pass reached through
`check_retraces`. The distinction matters because a parse-level refusal would
taint the file and silently suppress the rest of validation, which is what
chatter's "recovery is not validity" rule exists to prevent.) The second is discarded because it is invalid, and it is reported,
which is the whole difference from the behaviour this rule replaces.

Before E377 existed the model held one marker per retrace node in a field that a
second assignment simply overwrote, so `на [//] [/] на` parsed clean, validated
clean, and was written back as `на [/] на`. That silently demoted a
retracing-with-correction to a repetition, inside corpora whose subject is
disfluency.

## CHAT Rule

The CHAT manual's Scoped Symbols section states that "there should be no other
material entered between the square brackets and the material to which it
refers", and gives the combined form only with material between the two markers.

Adjudicated directly with the CHAT maintainer on 2026-08-07, who was shown
`*CHI: она на [//] [/] на [//] набросилась на кошку .` from the corpora and
answered "clearly a mistake" and, to the direct question, "It's an error".

## Scope in the corpora

105 occurrences across 46 files at the time the rule landed, 78 of them the
`[//] [/]` shape and 31 of the 46 files in Biling/BiSLI. Four further occurrences exist in the bracketed spelling `<does [/]> [/]`,
where a retrace wraps another retrace with no word of its own. Since the
lowering now folds both spellings into the same shape, one rule catches all
109.
