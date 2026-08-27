# Annotations

**Status:** Current
**Last updated:** 2026-08-27 18:09 EDT

A **scoped annotation** is a bracketed code written immediately after the thing
it describes: `hello [*]`, `<the dog> [//]`, `bobo [= toy]`, `0 [= ! whining]`.
It is scoped because it attaches to a specific construct rather than to the
utterance as a whole, which is what separates it from a
[postcode](postcodes.md) (utterance-wide, written before the terminator) and
from a [dependent tier](dependent-tiers.md) (a whole line of its own).

This chapter answers three questions the model makes precise: what can carry
annotations, what it means for something to carry none, and why an annotated
construct always carries at least one.

## What can be annotated

Each of these constructs has exactly two spellings. The list is the count; stating a number beside it is one more thing to keep true, and this line said five above six rows.

| Construct | Bare | Annotated |
|---|---|---|
| Word | `Word` | `AnnotatedWord` |
| Group `<...>` | `Group` | `AnnotatedGroup` |
| Quotation | `Quotation` | `AnnotatedQuotation` |
| Event `&=laughs` | `Event` | `AnnotatedEvent` |
| Action `0` | `Action` | `AnnotatedAction` |
| Retrace | `Retrace` | `AnnotatedRetrace` |

Everything else in an utterance is a leaf that takes no scoped annotation:
pauses, separators, overlap points, bullets, freecodes, and the long-feature,
underline and nonvocal delimiters.

Two constructs are worth calling out because they behave unlike their
neighbours. A **replaced word** (`word [: replacement]`) is `ReplacedWord`
rather than an `Annotated<Word>`, because the replacement is part of the word's
identity rather than a comment on it; it carries its own annotations alongside.
And a **retrace's** annotations describe the retrace itself, not the material
inside it, which is why a retrace opens no language scope for the words it
contains.

## Carrying none is a different variant, not an empty list

The bare and annotated spellings are different variants because they are
different things. `hello` is a word; `hello [*]` is a word plus a claim about
it. The model does not represent the first as the second with nothing in it.

This is enforced in the type rather than checked afterwards:

```rust,ignore
// The only public constructor. `None` when the list is empty.
AnnotatedContentAnnotations::new(annotations) -> Option<AnnotatedContentAnnotations>
```

So an annotated wrapper cannot be built without an annotation, and that
`Option` IS the bare-versus-annotated decision. Every place the parser builds
content, it reads:

```rust,ignore
match AnnotatedContentAnnotations::new(scoped) {
    None => UtteranceContent::Event(event),
    Some(scoped) => UtteranceContent::AnnotatedEvent(Annotated::new(event, scoped)),
}
```

`TryFrom<Vec<_>>` applies the same check, `Deserialize` rejects an empty list
off the wire rather than accepting one, and there is deliberately no `Default`.
The type also does not take the crate's collection-newtype macro, whose `take`
and `retain` can empty a collection in place.

## Why this is stated so emphatically

Because the invariant was prose for a long time, and prose does not hold.

Until 2026-08-26 `UtteranceContent` had no bare `Action`, though it had a bare
`Event` sitting two lines away in the same enum. An action with no annotations
therefore had nowhere to go, and the parser wrapped every one of them in an
`Annotated` carrying an empty list. Measured across a 106,000-file corpus that
was **20,184,072 values** claiming to be annotated while carrying nothing,
almost all of them a bare `0` marking silence in daylong audio recordings.
`BracketedItem` had the mirror-image gap: no bare `Group`, so an unannotated
nested group became an `AnnotatedGroup` with an empty list, and the converter
explained itself in a comment because it could do nothing else.

Two error codes were supposed to catch the empty case. Neither could. The full
account is in [Leniency Policy](../architecture/leniency-policy.md), Decision 1:
one code was deliberately disabled because bare `[*]` is valid CHAT, its number
was later reused for a different rule, and that rule was unreachable because an
empty bracket is a parse error and the one genuinely empty construct was never
validated.

Both bare variants exist now, the two content enums are symmetric, and the
empty state is unconstructible. The rule is no longer something a validator
looks for; it is something the compiler refuses.

## What an annotation attaches to when constructs nest

Scoping follows the innermost construct. In `<the big dog> [//] [* m]` both
annotations attach to the group. In `<the [//] dog>` the marker attaches to the
word inside it, because that is what precedes it.

One consequence matters for anything reading language: a `<...> [@s:spa]` group
opens a code-switch scope for the words inside it, and a retrace does not open
one at all. Tools should ask the model for the governing scope rather than
re-deriving it from the annotation list, because the two rules differ and the
difference is invisible if you get it wrong.
