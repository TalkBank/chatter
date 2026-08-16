# E756: Empty dependent tier

**Last updated:** 2026-08-16 01:22 EDT

## Description

A dependent tier whose content is empty or whitespace-only declares
nothing: the line asserts an annotation that is not there and fails to
make sense.

The rule covers EVERY dependent tier whose grammar body is free text.
That boundary is a grammar fact rather than a list to maintain: a tier
qualifies exactly when its rule in `grammar.js` marks its body
`optional($.text_with_bullets)` (or `optional($.text_with_bullets_and_pics)`,
which only `%com` uses). As of 2026-08-16 no free-text tier body is
required, so the rule reaches all of them: the user-defined `%x*` tiers,
the text-payload tiers, the bullet-payload tiers, and the `%tim` and Phon
tiers that finished the widening.

Structured tiers (`%mor`, `%gra`, `%pho`, `%mod`, `%sin`, `%wor`) are
NOT covered, and that is the whole of the exclusion. Their grammar bodies
are not free text, they parse their payload into typed items, and an
empty one fails earlier and more specifically than "you declared
nothing".

Formerly W601, which carried a warning-prefixed code while firing as a
hard error; the maintainer ruling of 2026-07-16 kept the rejection and
gave it an honest E-number.

WIDENED 2026-08-15 by maintainer ruling, finished 2026-08-16. It had
been wired only to `%x*` tiers, because `TextTier::content` could not
represent an empty standard tier and so a parser meeting `%eng:`
invented a payload. Once the model could say what the file contained, an
empty `%eng:` became representable and unjudged: the re2c backend
reported such a file VALID where tree-sitter rejected it through E330,
an `_auto` stub with no description. The ruling widened this rule rather
than writing E330's spec or adding a third code, on the grounds that "a
tier whose content is empty declares nothing" was never `%x`-specific
and only this code's NAME was. Real CLAN has no analogue. W601 is
retired and not reused.

The widening landed in three groups over two days, each blocked on the
model being able to SAY a tier was empty: the ten text-payload tiers
(`TextTier::content` became an `Option`), the nine bullet-payload tiers
(`BulletContent::is_empty` already answered, so only the grammar and the
lowering had to change), and last the five whose payloads could not
answer at all. `TimTier` needed a third state, `Empty`, because both of
its content variants hold a `NonEmptyString`; the three Phon tiers
derive `is_empty` from the word or group count they already reported.

## Metadata

- **Error Code**: E756
- **Category**: Dependent tier validation
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented
- **Kind**: Invalidity

## Example 1

**Trigger**: whitespace-only content on a custom `%x` tier.

**Expected Error Codes**: E756

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
%xtst:	 
@End
```

## Example 2

**Trigger**: a standard text tier declaring nothing. This is the case the
2026-08-15 widening added; before it, the re2c backend read this file as VALID.

**Expected Error Codes**: E756

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
%eng:	
@End
```

## Example 3

**Trigger**: a bullet-payload tier declaring nothing. Before 2026-08-16 the
tree-sitter grammar required this body, so the line failed to parse and
recovered as E342 while re2c already reported E756: the two backends disagreed
on what the file said.

**Expected Error Codes**: E756

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
%com:	
@End
```

## Example 4

**Trigger**: `%tim` declaring nothing. This is the case that needed a new model
state: both of `TimTier`'s content variants hold a `NonEmptyString`, so re2c had
to lower an empty `%tim:` to an unsupported DEPENDENT TIER and reported E605,
"unsupported dependent tier '%tim'", about a tier name that is perfectly
supported.

**Expected Error Codes**: E756

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
%tim:	
@End
```

## Expected Behavior

- **Parser**: Succeeds, and KEEPS THE TIER. Every free-text tier rule marks
  its body `optional(...)`, so a tier line with nothing after the separator
  parses as a tier whose payload says it is empty, rather than recovering with
  a spurious E342/E330. The tier is pushed into the model, so the line survives
  a roundtrip and `chatter normalize` reproduces it; reporting from the parse
  path instead would drop it. The shared `text_with_bullets` and
  `text_with_bullets_and_pics` rules stay `repeat1`, so an empty `%mor`,
  `%gra` or `@Comment` still does NOT parse.
- **Validator**: Reports E756 at the tier. It is not always the only code:
  Example 1's body is a lone space, which the separator absorbs, so that file
  ALSO earns E758 (illegal trailing space after the separator) on the
  tree-sitter backend. Two true statements about one line, not a double report
  of one fact. Examples 2 to 4 have a bare tab and isolate E756.

## CHAT Rule

A dependent tier exists to carry the annotation it declares; an empty
one is a defect in the transcript. The rule is about what a tier line
ASSERTS, so it applies to every tier kind whose body is free text, not
only to the `%x*` namespace where it was first written. Which tiers those
are is read off the grammar, never off a list in prose: a list would go
wrong the first time a tier was added, and this rule's own spec carried
exactly that defect for a day.
