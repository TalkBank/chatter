# Headers

**Status:** Reference
**Last updated:** 2026-09-24 00:21 EDT

Headers are lines beginning with `@` that provide metadata about the transcript. They appear between `@Begin` and the first utterance (though some headers like `@Comment` can appear anywhere).

## Required Headers

### @UTF8

Must be the very first line of every CHAT file. Declares UTF-8 encoding.

```chat
@UTF8
```

### @Begin / @End

Mark the start and end of the transcript body. Every CHAT file must have exactly one `@Begin` and one `@End`.

### @Participants

Declares all speakers in the transcript. Format:
`CODE [Name] Role`, comma-separated. The role is required; the name
is optional, so each entry is either `CODE Role` or `CODE Name Role`.

```chat
@Participants:	CHI Target_Child, MOT Mother, FAT Father
@Participants:	CHI Alex Target_Child, MOT Mary Mother
```

In the first line, `Target_Child`, `Mother`, and `Father` are roles,
not names. In the second line, `Alex` and `Mary` are optional names
sitting between the speaker code and the role.

Role labels use their canonical, case-sensitive spelling in both
`@Participants` and the role field of `@ID`. For example, `Mother` is a role;
`Mom` is not an alias, and `Target_child` is not `Target_Child`. E532 can offer
a likely correction, but validation and serialization preserve the original
spelling rather than silently rewriting it. Suggestions are heuristic advice,
not additional entries in the accepted vocabulary.

Speaker codes are short identifiers; the validator accepts up to
seven characters from `A-Z`, `0-9`, `_`, `-`, and `'`. The convention
is three uppercase letters; the most common codes are:

- `CHI`: target child
- `MOT`: mother
- `FAT`: father
- `INV`: investigator
- `OBS`: observer

### @ID

Provides detailed metadata for each participant. One `@ID` line per participant.

```chat
@ID:	eng|corpus|CHI|2;6.||||Target_Child|||
```

Fields (pipe-separated): language, corpus, speaker code, age, sex, group, SES, participant role, education, custom field.

Age format: `years;months.days` (e.g., `2;6.` = 2 years, 6 months).

SES field: ethnicity (`White`, `Black`, `Asian`, `Latino`, `Pacific`, `Native`, `Multiple`, `Unknown`), socioeconomic code (`UC`, `MC`, `WC`, `LI`), or combined with comma separator (e.g., `White,MC`).

## Optional Headers

### @Languages

Declares the language(s) used in the transcript.

```chat
@Languages:	eng, fra
```

### @Date

Recording date in DD-MON-YYYY format.

The day and year require exactly two and four ASCII digits respectively;
numeric signs are not allowed. `@Birth of CODE` uses the same format checks.
E518/E545 report malformed components without rewriting the source value.

```chat
@Date:	15-JAN-2024
```

### @Location

Where the recording took place.

```chat
@Location:	Boston, MA, USA
```

### @Situation

Description of the recording context.

```chat
@Situation:	free play with toys in lab
```

### @Activities

Activities during the recording.

```chat
@Activities:	toyplay, reading
```

### @Comment

Free-form comments. Can appear anywhere in the file (before, between, or after utterances).

```chat
@Comment:	child was tired during this session
```

### @Media

Links the transcript to an audio or video file.

```chat
@Media:	session01, audio
```

When the transcript name is known, Chatter checks it against the media name
(ignoring ASCII case). Different Unicode spellings of the same name, such as
composed and decomposed accents, produce W109 normalization advice rather than
E531 filename mismatch. The warning identifies whether the media name, the
transcript name, or both need normalization; validation does not rename files
or rewrite the header. Identical decomposed spellings warn about both sides.
Disk-based commands use the name stored in the directory, not the spelling
typed at the command line. An unreadable or unusable stored name is reported,
not silently treated as an anonymous transcript.

To normalize only the `@Media` name, run
`chatter fix --code W109 --apply <file>`. This changes only the filename token
and never renames the transcript. A remaining file-name warning requires a
separate rename using a tool that preserves NFC (not Finder). Remote media URLs
are opaque and exempt from both the comparison and this repair.

### @Transcriber / @Coder

Identifies who created or coded the transcript.

```chat
@Transcriber:	JDS
@Coder:	ABC
```

## Header Ordering

Headers should follow this conventional order:
1. `@UTF8` (required, first line)
2. `@Begin` (required)
3. `@Languages`
4. `@Participants` (required)
5. `@ID` lines (one per participant)
6. Other metadata headers (`@Date`, `@Location`, etc.)
7. `@Comment` lines (can also appear later)

## Validation

The parser validates header structure including:
- `@UTF8` must be the first non-empty line
- `@Begin` and `@End` are required and must appear exactly once
- `@Participants` is required and must declare all speakers used in utterances
- `@ID` participant codes must match `@Participants` declarations
- Age format validation in `@ID` lines
