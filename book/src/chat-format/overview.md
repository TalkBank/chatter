# CHAT Format Overview

**Status:** Reference
**Last updated:** {{git-dates:page}}

CHAT (Codes for the Human Analysis of Transcripts) is a transcription format
used by [CHILDES](https://childes.talkbank.org/) and
[TalkBank](https://talkbank.org/). These chapters introduce its representation
in Chatter; the CHAT manual remains the format reference. Parser recovery is
an implementation feature, not permission to treat malformed input as valid.

## File Anatomy

A typical transcript has a metadata preamble, speaker turns and optional
dependent annotations:

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|2;6.||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*MOT:	what do you want ?
%mor:	ADV|what AUX|do PRON|you VERB|want ?
%gra:	1|4|OBJ 2|4|AUX 3|4|NSUBJ 4|0|ROOT 5|4|PUNCT
*CHI:	I want cookie .
%mor:	PRON|I VERB|want NOUN|cookie .
%gra:	1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
@End
```

A CHAT file consists of:

1. **`@UTF8`**: required first line, declares UTF-8 encoding
2. **`@Begin`**: marks the start of the transcript
3. **Headers**: lines starting with `@` that provide metadata (participants, languages, IDs, etc.)
4. **Utterances**: blocks consisting of:
   - A **main tier** (line starting with `*SPEAKER:`) containing the transcribed speech
   - Zero or more **dependent tiers** (lines starting with `%tier:`) containing annotations
5. **`@End`**: marks the end of the transcript

## Key Conventions

- **Tab separation**: a tab character separates the tier prefix from its content (e.g., `*CHI:⟶content`)
- **Terminators**: ordinary utterances end with a terminator (`.`, `?`, `!`, or special forms like `+...`). The legacy [CA option](options.md) has a specific missing-terminator exception; CA symbols alone do not enable it.
- **Line continuation**: long lines wrap with a tab at the start of continuation lines
- **Speaker codes**: short identifiers; the validator accepts up to seven characters from `A-Z`, `0-9`, `_`, `-`, `'`; three uppercase letters is the convention (e.g., `CHI`, `MOT`, `FAT`, `INV`)
- **Media linking**: timestamps link transcripts to audio/video via bullet markers

## What validation establishes

Chatter checks syntax and supported semantic rules. A successful parse may
still produce diagnostics, and a recovered model is not evidence that the
original input is valid. Validation does not establish transcription accuracy,
recording identity or acoustic timing accuracy.

Start with [Headers](headers.md), [Utterances](utterances.md) and
[Dependent Tiers](dependent-tiers.md). For a runnable create/validate/repair
exercise, use the [Quick Start](../chatter/user-guide/quick-start.md).

## References

- [CHAT Manual](https://talkbank.org/0info/manuals/CHAT.html): the canonical reference
- [TalkBank](https://talkbank.org/): the data repository
