# CHAT Format Overview

**Status:** Reference
**Last updated:** 2026-05-11 21:51 EDT

CHAT (Codes for the Human Analysis of Transcripts) is a standardized transcription format for spoken language data, developed by MacWhinney as part of the [CHILDES](https://childes.talkbank.org/) and [TalkBank](https://talkbank.org/) projects. It is the most widely used format in child language research and conversational analysis.

## File Anatomy

Every CHAT file follows this structure:

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|2;6.||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*MOT:	what do you want ?
%mor:	ADV|what AUX|do PRON|you VERB|want ?
%gra:	1|4|LINK 2|4|AUX 3|4|SUBJ 4|0|ROOT 5|4|PUNCT
*CHI:	I want cookie .
%mor:	PRON|I VERB|want NOUN|cookie .
%gra:	1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
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
- **Terminators**: every utterance ends with a terminator (`.`, `?`, `!`, or special forms like `+...`)
- **Line continuation**: long lines wrap with a tab at the start of continuation lines
- **Speaker codes**: short identifiers; the validator accepts up to seven characters from `A-Z`, `0-9`, `_`, `-`, `'`; three uppercase letters is the convention (e.g., `CHI`, `MOT`, `FAT`, `INV`)
- **Media linking**: timestamps link transcripts to audio/video via bullet markers

## CHAT vs Other Formats

| Feature | CHAT | Praat TextGrid | ELAN EAF |
|---------|------|----------------|----------|
| Morphological tiers | Built-in (%mor, %gra) | No | No |
| Dependency syntax | Built-in (%gra) | No | No |
| Standardized POS | UD-style via %mor | No | No |
| Word-level alignment | %wor tier | Interval-based | Interval-based |
| Error recovery | Tree-sitter GLR | N/A | N/A |

## References

- [CHAT Manual](https://talkbank.org/0info/manuals/CHAT.html): the canonical reference
- [TalkBank](https://talkbank.org/): the data repository
