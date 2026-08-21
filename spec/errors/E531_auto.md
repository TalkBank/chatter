+++
code = 'E531'
name = 'Media filename mismatch'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'error_corpus/validation_errors/E531_media_filename_mismatch.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	different, audio
@Comment:	ERROR: Media filename must match transcript
*CHI:	hello .
@End
'''
+++

## Description

The filename in the `@Media` header does not match the name of the CHAT file
being parsed (case-insensitive comparison). For example, if `foo.cha` contains
`@Media: bar, audio`, E531 is reported because `bar` does not match `foo`.

E531 requires the validator to be invoked with the file's name: the check in
`crates/talkbank-model/src/model/file/chat_file/validate.rs` compares the
`@Media` filename against the file's own stem. The manifest-driven validation
runner passes each fixture's stem to `validate_with_alignment`, so the check
fires: `@Media: different, audio` in a file not named `different.cha` triggers
E531.

## Expected Behavior

The validator should report E531 when the `@Media` header filename does not
match the CHAT file name (case-insensitive). The check exists in
`crates/talkbank-model/src/model/file/chat_file/validate.rs`.

**Trigger conditions**: `@Media` header contains a filename that differs from
the stem of the `.cha` file being validated. The validator must be invoked with
the file path to enable this check.

## CHAT Rule

See CHAT manual on the `@Media` header. The media filename should match the
transcript filename. The CHAT manual is available at:
https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The check requires the transcript's name at validation time. Both runners now
  supply it: the manifest-driven fixture runner passes each fixture's stem, and
  the spec-example runner passes the stem of this example's own `**Source**`
  line (`E531_media_filename_mismatch`), against which `@Media: different` is a
  mismatch. Until 2026-08-11 the latter passed `None`, which silently disabled
  every rule about the file's own name, so this spec could not be verified
  there and was reported as failing rather than as untestable. The name is a
  `TranscriptName` now, not an `Option<&str>`, so a caller with no name says
  `Anonymous` rather than leaving a reader to work out what `None` switched off.
- This example also emits E544 (`@Media` declares linkage but the transcript
  carries no timing evidence), which is correct and independent: it has no
  bullets. Extra emitted codes never fail a claim, so E544 is not declared
  here; it is declared by E544's own spec.
