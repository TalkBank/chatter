# File Names and @Media

**Status:** Current
**Last updated:** {{git-dates:page}}

A CHAT transcript names its recording twice: once in its own file name, and
once in the `@Media` header. A tool that needs the recording then looks for a
file with that name. All three spellings must be the same, character for
character, or the transcript finds its recording on some computers and not on
others.

## The rule

For a transcript `session01.cha`:

```chat
@Media:	session01, audio
```

- The `@Media` name equals the transcript's file name without `.cha`.
- The recording is that name plus its format's extension, in lower case:
  `session01.mp3`, `session01.mp4` or `session01.wav`.
- Names are spelled in Unicode NFC, the standard composed form, where an
  accented letter is one character rather than a letter plus a separate
  accent mark.

A remote URL in `@Media` is exempt: it points at media elsewhere, so there is
no local file name to compare.

## Why "the same" means exactly the same

Filesystems disagree about when two names are the same file:

| Difference | macOS (default) | Linux |
|---|---|---|
| Letter case: `Session01` vs `session01` | same file | different files |
| Unicode form: composed vs decomposed `ü` | same file | different files |

A transcript whose names differ only in one of these ways works on a Mac and
fails on Linux, including a Linux server that publishes recordings. Both kinds
of difference are easy to introduce without noticing: letter case by hand, and
decomposed accents by editors, operating systems and transfer tools that
default to that form. The two spellings look identical on screen.

## What chatter reports

When chatter knows the transcript's file name (`chatter validate` on a file or
directory, `to-json`, the language server), it compares that name with the
`@Media` name:

| Code | Severity | When | Example |
|---|---|---|---|
| [E531](../chatter/user-guide/validation-errors.md#e531-w109-w110-transcript-and-media-names) | error | The names differ in more than ASCII letter case and Unicode form | `session01.cha` with `@Media: session02` |
| W109 | warning | The same name, but one or both spellings are not NFC | `Schlüssel.cha` with a decomposed `ü` in `@Media` |
| W110 | warning | The same name except for ASCII letter case | `Session01.cha` with `@Media: session01` |

E531 compares without regard to ASCII letter case, as CLAN's CHECK does
(its error 157), so a case-only difference is not an E531 error. W110 reports
it instead, because the difference still breaks the lookup on a case-sensitive
filesystem. W109 and W110 are independent: one name can draw both. A
difference in the case of a non-ASCII letter (`É` vs `é`) is E531.

Chatter compares the transcript name as it is stored in the directory, not
as it was typed on the command line.

## Fixing each one

- **E531:** decide which name is right. Usually `@Media` is updated to the file
  name, which the suggestion in the diagnostic spells out.
- **W109:** `chatter fix --code W109 --apply <file>` rewrites the `@Media` name
  in NFC and changes nothing else. A file name that is not NFC must be renamed
  with a tool that keeps NFC; Finder does not.
- **W110:** make the spellings identical: update `@Media`, or rename the
  transcript. There is no automatic fix, because chatter cannot see how the
  recording itself is spelled.

In every case, rename the recording to match as well. Chatter validates the
transcript, not the recording, so a recording spelled differently from its
`@Media` name is invisible to it. Tools that look the recording up report
that; batchalign3 matches recording names exactly on every host.
