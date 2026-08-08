# E378, A retracing marker over material with no words

**Status:** Current
**Last updated:** 2026-08-07 17:24 EDT

## Description

A retracing marker (`[/]`, `[//]`, `[///]`, `[/-]`) applied to material that
contains no words. A marker retraces the WORDS immediately to its left, and a
laugh is not a word, so there is nothing for it to refer to.

```text
&=laughs [//] water        unbracketed
<&=sigh> [/] &=sigh        bracketed, the form that actually dominates
<(.) &=laughs> [//]        a pause is not a word either
```

The rule is about the ABSENCE OF WORDS, not the presence of an event. Putting
the event inside material that has words is legal, and is the recommended
repair:

```text
*PAR:	<the floor on the &=laughs water> [//] the floor on the xxx .
```

The test recurses, because 205 corpus retraces hold their words one level down
inside an annotated group or a quotation (`<<the dog> [?]> [/] the dog`) and are
perfectly legal.

## Metadata

- **Error Code**: E378
- **Category**: retrace
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented
- **Kind**: Invalidity

## Example 1

**Trigger**: a retracing marker on a bare event
**Expected Error Codes**: E378

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: a laugh is not a word, so the marker retraces nothing
*CHI:	&=laughs [//] water .
@End
```

## Example 2

**Trigger**: a group holding nothing but an event
**Expected Error Codes**: E378

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: the retraced group contains no words
*CHI:	<&=sigh> [/] &=sigh ok .
@End
```

## Example 3

**Trigger**: a pause alongside the event does not supply a word
**Expected Error Codes**: E378

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: neither the pause nor the event is a word
*CHI:	<(.) &=laughs> [//] ok .
@End
```

## Expected Behavior

Validation reports E378 once per retrace whose material has no word beneath it.
Parsing is unaffected: the construct still lowers and still round-trips, so the
transcript remains recoverable and the offending line is reported rather than
rewritten.

Untranscribed material counts as words, deliberately. `xxx`, `yyy` and `www`
lower as words, so `<xxx> [/] xxx` stays valid: retracing speech you could not
make out is legitimate, and only material that is not speech at all is caught.
An omitted lexical word does too: `0det [/] 0det dog` is valid, while a bare
`0` carrying only a paralinguistic annotation (`0 [=! snuffles] [/]`) is not.
The line falls out of what the model already calls a word rather than being
drawn by this rule.

**Reporting is per retrace, and nested wordless retraces each report.**
`<<&=sigh> [/]> [//]` raises E377 once and E378 twice, on the outer retrace and
on the inner one. Every one of the three is true and a single edit fixes all
three, so no suppression is wired between the rules; coupling them would create
exactly the sort of cross-rule dependency that drifts. The shape is unattested
in the corpora.

## CHAT Rule

The CHAT manual's Retracing and Repetition section describes the marker as
referring to the material the speaker said and then repeated or corrected, which
is words.

Adjudicated directly with the CHAT maintainer on 2026-08-07. Shown
`*PAR: <the floor on the> [//] &=laughs [//] water [//] the floor on the xxx .`
from `dementia-data` and asked whether `&=laughs [//]` means anything or is an
error the way adjacent markers are, he answered: "No, not legal. You can't
retrace a laugh." Half an hour earlier in the same thread he gave the legal
alternative, the event inside a group of words, which is why this rule tests for
absent words rather than for a present event.

## Scope in the corpora

**15 instances across 12 files**, measured by running this rule over all
107,376 corpus files. Spread over `childes-other-data` (Biling/Siena, two
Cantonese MOST sessions, Farsi/Minu), `childes-romance-germanic-data`
(Swedish/Lund), `dementia-data` (WLS), `slabank-data` (ESF DutArab),
`psychosis-data` (Tang) and `phon-eng-french-data` (Lyon).

Two earlier figures, 7 and then 14, were each produced by locating candidate
files with `rg` and validating only those. Both were wrong: the first predated
events being lowered as retraces, and the second searched for events, so it
missed `0 [=! skratt] [/]`, a wordless retrace containing no event at all. A
locate is a hypothesis about what a rule catches; only the rule knows. Re-derive
this figure by running `chatter validate` over the corpus, never by searching
for the construct.
