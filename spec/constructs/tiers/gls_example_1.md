# gls_example_1

Gloss tier as dependent annotation text.

## Input

```utterance
*CHI:	hello .
%gls:	HELLO
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
  (gls_dependent_tier
    (gls_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (text_with_bullets
      (text_segment))
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
