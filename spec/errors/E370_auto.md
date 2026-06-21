# E370, Structural order error

**Status:** Current
**Last updated:** 2026-06-16 18:11 EDT

## Description

A structural ordering violation in main-tier content. In particular, a retrace
or repetition marker (`[/]`, `[//]`, `[///]`) must be followed by the repeated or
corrected material: per the CHAT manual the marker is necessarily followed by the
material it retraces. A retrace marker followed only by a terminator (e.g.
`<the> [/] .`) has nothing to retrace and is reported as E370.

## Metadata

- **Error Code**: E370
- **Category**: retrace
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: Retrace marker `[/]` with no following material (only a terminator)
**Expected Error Codes**: E370

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: retrace [/] is not followed by repeated material
*CHI:	<the> [/] .
@End
```

## Example 2

**Trigger**: Retrace-with-correction `[//]` with no following corrected material
**Expected Error Codes**: E370

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: retrace [//] is not followed by corrected material
*CHI:	the cat [//] .
@End
```

## Expected Behavior

Validation reports E370 when a retrace/repetition marker is not followed by
substantive content. A valid retrace has the repeated or corrected material after
the marker, for example `<the> [/] the cat .` or `the dog [//] the cat .`.

## CHAT Rule

See the CHAT manual on Retracing and Repetition: the material immediately
following a `[/]`, `[//]`, or `[///]` marker is the repeated or corrected text the
marker refers to, so the marker cannot be the last substantive element of an
utterance.
