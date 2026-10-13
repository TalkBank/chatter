# sin\_zero\_marker

Zero marker in %sin tier, verifies `0` parses as `sin_word > zero` (not ERROR).

Regression gate for grammar lint finding: `zero` (prec 3) shadows `sin_word`
regex at the DFA level. The shadow is harmless because `sin_word` explicitly
lists `$.zero` as an alternative.

## Input

```utterance
*CHI:	hello .
%sin:	0
```

## Expected CST

```cst
(utterance
  (main_tier
    (star)
    (speaker)
    (colon)
    (tab)
    (tier_body
      (contents
        (content_item
          (base_content_item
            (word_with_optional_annotations
              (standalone_word
                (word_body
                  (word_segment))))))
        (whitespaces))
      (utterance_end
        (period)
        (newline))))
  (sin_dependent_tier
    (sin_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (sin_groups
      (sin_group
        (sin_word
          (zero))))
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
