# Headers

**Status:** Reference
**Last updated:** {{git-dates:page}}

Headers are lines beginning with `@` that provide metadata or delimit the
transcript. The metadata preamble follows `@Begin`; changeable headers such as
`@Comment` may also occur between utterances. The `@UTF8`, `@Begin` and `@End`
markers have their own positions, described below.

## Required Headers

### @UTF8

Must be the very first line of every CHAT file. Declares UTF-8 encoding.

```chat
@UTF8
```

### @Begin / @End

Mark the start and end of the transcript body. Every CHAT file must have exactly one `@Begin` and one `@End`.

### @Languages

Declares the language(s) used in the transcript. This is required, not an
optional metadata header; Chatter reports E504 when it is absent.

```chat
@Languages:	eng, fra
```

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

### @Options

`@Options: CA` requests specific legacy parsing/validation exceptions; it is
not required for CA-originated notation and is not a generic processing ban.
`NoAlign` is a distinct application-level alignment opt-out. See
[the CA option](options.md) for actual effects, downstream policies and how to
remove an unnecessary declaration without changing source meaning.

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

Free-form comments may occur in the preamble or between utterances, inside the
`@Begin`/`@End` envelope. They must not interrupt the ordered participant,
options and ID blocks described below.

```chat
@Comment:	child was tired during this session
```

### @Media

Links the transcript to an audio or video file.

```chat
@Media:	session01, audio
```

When the transcript name is known, Chatter checks it against the media name.
A name that differs in more than ASCII letter case is E531. A name that
differs only in letter case (`Session.cha` declaring `@Media: session`) is
W110: CLAN's CHECK accepts it, but a case-sensitive filesystem will not find
the recording, so the names must be made identical. Different Unicode
spellings of the same name, such as
composed and decomposed accents, produce W109 normalization advice rather than
E531 filename mismatch. The warning identifies whether the media name, the
transcript name, or both need normalization; validation does not rename files
or rewrite the header. Identical decomposed spellings warn about both sides.
Disk-based commands use the name stored in the directory, not the spelling
typed at the command line. An unreadable or unusable stored name is reported,
not silently treated as an anonymous transcript.

To normalize only the `@Media` name, first preview
`chatter fix --code W109 <file>`, then add `--apply` after reviewing the proposed
change. This repair changes only the filename token
and never renames the transcript. A remaining file-name warning requires a
separate rename using a tool that preserves NFC (not Finder). Remote media URLs
are opaque and exempt from both the comparison and this repair.

[File Names and @Media](file-names.md) gathers these rules in one place: what
must match, why a Mac hides the difference, and how to fix each diagnostic.

### @Transcriber / @Coder

Identifies who created or coded the transcript.

```chat
@Transcriber:	JDS
@Coder:	ABC
```

## Header Ordering

Use this preamble order. Several of these relationships are enforced, not
merely stylistic:

1. `@UTF8` (required, first line)
2. `@Begin` (required)
3. `@Languages` (required)
4. `@Participants` (required)
5. Optional `@Options`, immediately after `@Participants`
6. The `@ID` block, immediately after `@Participants` or `@Options`
7. Constant participant headers such as `@Birth of CODE`, when present
8. Other metadata (`@Date`, `@Location`, `@Comment`, etc.)

E543 checks participant-header precedence; E551 checks the immediate position
of `@Options`, E548 the ID block, and E547 the constant participant headers.
Do not insert `@Comment` between those blocks. `@End` closes the transcript
after its utterances and any permitted intervening headers.

## Validation

Parsing and model validation together check header syntax and relationships,
including the following requirements:

- `@UTF8` declares encoding at the start of the file
- `@Begin` and `@End` are required and must appear exactly once
- `@Participants` is required and must declare all speakers used in utterances
- `@Languages` is required
- `@ID` participant codes must match `@Participants` declarations
- Age format validation in `@ID` lines
