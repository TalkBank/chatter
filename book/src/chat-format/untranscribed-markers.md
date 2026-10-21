# Untranscribed Markers: `xxx`, `yyy`, `www`

**Status:** Reference
**Last updated:** {{git-dates:page}}

CHAT reserves three short word-level markers for material the human
transcriber cannot or chose not to render as words on the main tier.
Each one has a specific meaning. Tools that emit CHAT, including ASR
pipelines, format converters, and editor heuristics, must respect
those meanings, because every downstream consumer (researchers,
validators, and aggregate-statistics tools like CLAN's `freq`,
`kideval`, `mlu`) reads them at face value.

| Marker | Meaning | Emitter |
|---|---|---|
| `xxx` | Transcriber listened to the audio and **could not make out** what was said. The speech is unintelligible to the human ear at this point. | Human transcriber only. |
| `yyy` | Transcriber heard a discrete utterance but could not write it as ordinary CHAT words. Used when the surface form resists orthography (mumbled, slurred, foreign with no equivalent). The phonetic content typically appears on the `%pho` tier. | Human transcriber only. |
| `www` | Transcriber chose **not to transcribe** this stretch, usually for privacy, off-topic content, or because the segment is irrelevant to the corpus's purpose. | Human transcriber only. |

The shared property: each marker is the human transcriber telling
later readers something specific about *their experience listening to
the audio*. None of them mean "tooling could not process this token".

## Why this matters

When a researcher loads a CHAT corpus and counts `xxx` occurrences, the
result is a *measure of human listening difficulty*: it tells them how
much of the audio resisted human transcription. That number feeds into
methodology decisions ("can we get reliable MLU from this corpus?",
"what's the noise floor on this child's speech?", "should we re-record
in a quieter environment next time?"). It is a load-bearing signal in
language-development research.

If an ASR pipeline emits `xxx` whenever it can't sanitize a token,
for example, substituting `xxx` for any word that fails CHAT
validation under a strict language profile, every `xxx` count in the
corpus becomes a meaningless mixture of "human couldn't tell" and
"pipeline gave up". Researchers then reading those counts are silently
misled. The signal is destroyed for the entire history of that
corpus, because the corruption is indistinguishable from real
unintelligibility once committed.

The same reasoning applies to `yyy` and `www`: substituting any of these
markers for a processing failure changes the transcript's meaning and hides
the distinction between source evidence and tool failure.

## Rules for tooling

1. **Never emit `xxx`, `yyy`, or `www` from a tool to mean "could not
   process".** These markers are reserved for human transcriber
   judgment.
2. **When a token cannot be validated as legal CHAT under the
   declared language**, prefer one of:
   - Pass the token through verbatim and let the CHAT validator
     (or CLAN's `check`) flag it for human review. The transcriber
     listens, decides, and corrects.
   - Fail loud, abort the file rather than emit corrupted output.
   - Apply an explicitly reviewed transformation whose preservation contract
     covers the source format and retain the original plus the change record.
     A quote at a token boundary is not, by itself, evidence that it is stray
     or safe to remove.
3. **Never sanitize a token by replacing it with one of the three
   markers.** That is exactly the corrupting behavior this document
   prohibits.
4. **Never delete a token to "fix" a validation failure.** Deletion
   loses data without any flag.

## What tools synthesizing CHAT should do instead

Any tool that builds CHAT from an external source (ASR output, an
importer, a format converter) should follow the same division of
labor:

1. Preserve the source and distinguish format conversion from transcription
   judgment. Do not silently remove punctuation or replace failed tokens.
2. If the candidate parses but fails validation, retain the candidate and
   diagnostics for review or refuse publication. Successful parsing does not
   establish validity, and a diagnostic does not establish what was spoken.
3. If parsing reports errors, stop the conversion's successful-output path.
   A recovered model can help diagnosis, but it is not permission to publish
   reconstructed CHAT as a faithful transcript. Keep the original evidence.

The division of labor is: tooling reports the failure and applies only reviewed
transformations; the transcriber decides what the recording supports. Chatter
and CLAN CHECK are distinct validators, not interchangeable diagnostic-code
authorities or substitutes for listening.

## Related rules

- Downstream processing must preserve the markers' meanings; this is a
  requirement, not a claim that every NLP implementation has been verified.
- Chatter's current `%mor` membership excludes untranscribed markers;
  `%pho` and `%sin` include them as positions. This structural distinction
  does not supply missing phonetic content.
- The current `%wor` policy, `FilteredLexicalV1`, excludes all three,
  including inside retraces. That generation policy does not prohibit a
  legacy parsed tier from containing them. See
  [`%wor` timing semantics](../architecture/wor-timing.md) for the separate
  correspondence and timing-evidence contracts.
- See [Word Syntax](word-syntax.md) for grammar; this document is the policy
  reference for *who is allowed to emit them and why*.
