# Postcodes (`[+ ...]`)

**Status:** Reference
**Last modified:** {{git-dates:page}}

A **postcode** attaches a researcher or analysis tag to an utterance as a
whole. It appears on the main tier after the terminator, not on an individual
word. Adding a tag does not change the utterance's words or itself perform an
analysis operation.

## Syntax and Scope

The reference transcript `corpus/reference/content/postcodes-and-freecodes.cha`
includes this example:

```chat
*CHI:	hello . [+ bch] [+ foo]
```

Multiple postcodes are stored in source order. Preserve that order unless a
reviewed operation explicitly changes it; an open vocabulary does not establish
that every consumer treats ordering as irrelevant.

The payload is text, not a list of CHAT words. That does **not** mean arbitrary
bytes are accepted: the grammar requires content after `[+ ` and excludes a
closing bracket or newline from that content. Parser admission also rejects an
empty payload after trimming trailing whitespace. A recognized token or a
programmatically constructed `Postcode` does not certify the whole document.

## Common Postcodes

The vocabulary is open-ended. The following conventional labels are also offered
by the editor's completion feature; they are suggestions, not a whitelist:

| Postcode | Conventional meaning |
| --- | --- |
| `[+ gram]` | Grammatical utterance |
| `[+ exc]` | Exclude utterance from the relevant analysis |
| `[+ bch]` | Backchannel |
| `[+ trn]` | Translation utterance |

Consult the corpus protocol and the analysis tool's options before interpreting
or changing a tag. Chatter does not automatically exclude an utterance from
every downstream analysis because its postcode is `exc`, nor does an unfamiliar
tag such as `foo` become invalid solely because it is absent from this table.

This guide does not maintain a corpus-frequency census. Historical occurrence
counts are not a vocabulary definition, a current prevalence measurement or
evidence that an unobserved parser path is unreachable.

## What Postcodes Are NOT

| Form | Attachment | Payload |
| --- | --- | --- |
| `[+ ...]` | Whole utterance | Postcode text |
| `[: ...]` | One original word | One or more typed replacement words |
| `[% ...]` | Preceding word or scope | Comment text |
| `[= ...]` | Preceding word or scope | Explanation text |
| `[* ...]` | Preceding word or scope | Error coding |

These forms have distinct grammar and validation rules. Free text is not a
blanket validation exemption, and selecting another annotation to avoid a
diagnostic can change the transcript's meaning. See [Replacements](replacements.md)
for the word-level alternative and [Annotations](annotations.md) for scoped forms.

A comment that says “exclude this” is not automatically an utterance-exclusion
postcode. Conversely, a postcode does not provide a word-level attachment for a
consumer that needs to know which word was annotated.

## Not Postcodes: Quotation Markers

Quotation terminators and linkers such as `+"/.`, `+"/` and `+"` are not
`[+ ...]` tags. They have separate grammar and model owners. See
[Utterances → Terminators](utterances.md#terminators) rather than interpreting
every plus-prefixed form as a postcode.

## Position in the AST

The utterance's `main.content.postcodes` is a `TierPostcodes` collection of
`Postcode` values. A postcode owns its text and source span; it is not a
`ContentAnnotation` on a word. Word walkers therefore are not the interface for
reading or changing the postcode list.

Dependent tiers have their own payload types and grammars. A bracket-shaped
string there is not automatically main-tier postcode metadata: a text tier may
retain it as text, while another tier's grammar may reject it. Consult the
[CHECK assessment](../architecture/errors-and-validation/check-parity-audit.md)
for the documented diagnostic boundary; do not add a second text parser to
imitate another tool's error code.

The model serializer writes `[+ `, the stored payload and `]`. The primary
parser trims trailing payload whitespace, so a parse/serialize cycle is not a
promise of byte-for-byte source preservation. Keep the original when exact
authored spelling or formatting matters.

## Tooling Rules

- Add or inspect utterance tags through `main.content.postcodes`, not the
  word-level annotation list.
- Preserve unknown values and their order during unrelated transformations.
  A supported constructor or serializer is not authority to reinterpret a tag.
- Validate the complete candidate after an intentional edit, and review the
  changed tag against the corpus's coding protocol. Validation cannot decide
  whether an utterance should be excluded from a particular research analysis.
- Completion replaces the selected postcode with an explicit source range;
  the user still chooses whether the offered tag is appropriate.

## References

- [CHAT manual: Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)
- [Model and serializer](https://github.com/TalkBank/chatter/blob/main/crates/talkbank-model/src/model/content/postcode.rs)
- [Primary parser admission](https://github.com/TalkBank/chatter/blob/main/crates/talkbank-parser/src/parser/tree_parsing/postcode/mod.rs)
- [Grammar](https://github.com/TalkBank/chatter/blob/main/grammar/grammar.js): `postcode` and `annotation_content`
