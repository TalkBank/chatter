# The CA option: legacy exceptions, not notation

**Status:** Reference
**Last modified:** {{git-dates:page}}

`@Options: CA` is not required merely because a transcript uses notation that
originated in Conversation Analysis. Overlap brackets, intonation arrows,
pauses, lengthening and other supported symbols belong to the CHAT grammar;
their origin does not make a file exempt from ordinary processing.

Do not add this header by copying a convention from an older edition or by
looking for CA symbols. The direction for maintained data is to remove the
option where its actual exceptions are unnecessary, preserving the notation
and source meaning. Supporting legacy files does not require perpetuating the
option in newly generated transcripts.

## What Chatter actually changes

The header has several concrete effects. They must be evaluated separately:

| Effect | Current behavior | Consequence when removing CA |
| --- | --- | --- |
| Utterance terminators | The missing-terminator check is waived. | An utterance must have an ordinary CHAT terminator. CA arrows remain separators, not substitute terminators. |
| Standalone parentheticals | `(word)` is interpreted as a CA omission. This changes meaning, rather than merely relaxing validation. | Check the intended interpretation; a successful parse alone does not prove semantic equivalence. |
| Source separator whitespace | The E758 separator check is waived for spaces after the tab on headers, main tiers and dependent tiers. | Review padding on all three line kinds; do not infer an acoustic alignment problem from column layout. |

These are not general permissions to use arbitrary notation or ignore validity.
CA does not disable timing checks: E362, E701 and E704 still apply. It does not
turn malformed syntax, invalid language markers or missing media evidence into
valid CHAT. See [bullet validation](../architecture/bullet-validation.md).

The model names the terminator waiver and parenthetical reinterpretation
separately in `CaOptionEffect`. A broad predicate such as "is this a CA file?"
is inadequate when the caller needs to know one particular effect.

See [CA terminator resolution](../architecture/parser-and-grammar/ca-terminator-resolution.md)
and [contextual parsing](../architecture/parser-backends.md) for the parser
boundaries. The [CHAT manual](https://talkbank.org/0info/manuals/CHAT.html)
documents the optional header and its terminator convention; it does not prove
that a transcript is unsuitable for modern acoustic or morphological analysis.

## Processing conventions are a separate layer

Chatter parses, represents, validates and serializes the header. Downstream
applications have also attached processing policies to it. Those are
application decisions, not consequences of CA notation or Chatter validity.

Batchalign3's (BA3's) source-level policy distinguishes two flags:

- `NoAlign` opts a valid file out of the `align` command.
- `CA` defaults `morphotag` to validated pass-through under `--ca-policy honor`;
  `--ca-policy analyze` requests analysis while retaining the header. CA also
  currently suppresses `%wor` generation during `align`. CA alone does not
  skip the current forced-alignment implementation.

CA additionally selects `WordTimingPlan::Preserve` during BA3 alignment input
admission, rather than the ordinary `PreferRetained` plan. This prevents that
admission step from discarding retained word tiers; it does not promise
byte-identical output after alignment. `NoAlign` instead takes the admitted
unchanged-source path. Neither policy bypasses validation of retained data.
Input admission, whether a command runs, and which timing reaches disk are
separate decisions.

These inherited decisions must not be described as empirical evidence that
CA-marked transcripts cannot be aligned or analyzed. Difficulties with an older
algorithm do not establish a permanent restriction on later algorithms. Assess
current implementations against actual inputs and saved outputs.

In particular, a parser accepting a file, an aligner producing intervals in
memory, and a written CHAT file retaining those intervals are three separate
claims. A header-based output suppression must not make an in-memory coverage
count look like durable word-timing coverage. Processing options and their
actual effective output must be explicit and checked at the serialization
boundary.

See BA3's [Options reference](https://github.com/FranklinChen/talkbank-tools/blob/main/book/src/batchalign/reference/chat-options.md)
for its application-level policies and check the version used by the actual
job. Source inspection does not establish the identity or behavior of an
installed server, qualify a provider, or show that a runtime migration has
occurred. The implementation still supports the legacy behavior described here.

## Removing an unnecessary CA declaration

A source-preserving migration should:

1. Identify whether missing terminators, standalone parentheticals or alignment
   padding actually depend on the option. Preserve intended meaning.
2. Remove the option from a new version and validate with the normal parser and
   full applicable checks, including serialization roundtrip.
3. Compare source content and interpretations. Roundtrip proves stability of
   the new interpretation, not equivalence to the old one.
4. Reassess any downstream command policy and inspect the written artifact.
   Validity does not establish acoustic accuracy or authorize replacing data.

Retain source and transformation provenance. Do not erase CA notation, invent
terminators or rewrite omissions merely to make a checker pass. Conversely,
when a file does not need the exception, do not retain CA simply because an
older derivative carried it.
