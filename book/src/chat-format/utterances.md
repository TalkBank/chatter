# Utterances

**Status:** Reference
**Last updated:** {{git-dates:page}}

An utterance is the fundamental unit of a CHAT transcript. It consists of a main tier (the transcribed speech) followed by zero or more dependent tiers (annotations).

## Main Tier

The main tier begins with `*SPEAKER:` followed by a tab and the utterance content, ending with a terminator.

```chat
*CHI:	I want a cookie .
```

### Speaker Codes

Speaker codes are short identifiers (up to seven characters from `A-Z`, `0-9`, `_`, `-`, `'`; three uppercase letters is the convention) matching a code declared in `@Participants`:

```chat
@Participants:	CHI Target_Child, MOT Mother
*MOT:	what do you want ?
*CHI:	cookie .
```

### Terminators

Ordinary utterances require a terminator. The legacy
[`@Options: CA`](options.md) declaration waives the missing-terminator check;
intonation arrows themselves are not ordinary terminators. Do not add CA merely
to silence a diagnostic: it also changes parenthetical interpretation.

Common terminators include:

| Terminator | Meaning |
|------------|---------|
| `.` | Declarative (period) |
| `?` | Question |
| `!` | Exclamation |
| `+...` | Trailing off |
| `+..?` | Trailing-off question |
| `+/.` | Interruption |
| `+//.` | Self-interruption |
| `+/?` | Interrupted question |
| `+!?` | Broken question |
| `+"/.` | Quotation follows on next line |

### Line Continuation

Long utterances wrap to the next line with a leading tab:

```chat
*MOT:	well I think that we should probably go to
	the store and get some more cookies .
```

## Content Items

The content between `*SPEAKER:` and the terminator consists of content items separated by whitespace:

- **Words**: regular words, potentially with annotations
- **Groups**: bracketed content like `<word word>` for overlap, retrace, etc.
- **Special forms**: pauses `(.)`, events `&=laughs`, fillers `&-uh`
- **Separators**: commas `,` and other punctuation

### Words

Words are the primary content unit. See [Word Syntax](word-syntax.md) for full details.

### Groups

Angle brackets `< >` group words for annotations:

```chat
*CHI:	<I want> [/] I want cookie .
```

Common group annotations:
- `[/]`: partial retrace (speaker repeats the same words)
- `[//]`: full retrace (speaker restarts with different words)
- `[///]`: multiple retracing (multiple false starts)
- `[/-]`: reformulation (speaker rephrases with different structure)
- `[?]`: uncertain transcription

### Special Forms

```chat
*CHI:	um (.) I want &-uh cookie .
```

- `(.)`: short pause
- `(..)`: medium pause
- `(...)`: long pause
- `(1.5)`: timed pause in seconds
- `&=laughs`: paralinguistic event
- `&-uh`: filler

## Media Linking

Utterances can include media timestamps (bullets) that link to audio/video:

```chat
*CHI:	I want cookies . •1234_5678•
```

The numbers represent start and end times in milliseconds. The bullets
delimiting the pair render as `•` in most editors; on disk they are
the NAK control character (`U+0015`). See `grammar/grammar.js` rule
`bullet`.

The visible `•` characters above illustrate the delimiters; typing U+2022
bullets does not create the on-disk U+0015 timing markers. Use a CHAT-aware
editor or a tool that writes the actual delimiters. Format validation checks
their structure and applicable timing rules, not whether the interval matches
the speech in the recording.

## Dependent Tiers

See [Dependent Tiers](dependent-tiers.md) for documentation on `%mor`, `%gra`, `%pho`, `%wor`, and other annotation tiers that follow the main tier.
