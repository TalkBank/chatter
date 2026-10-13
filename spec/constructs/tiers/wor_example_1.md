# wor_example_1

Example %wor tier

## Input

```wor_dependent_tier
%wor:	word 123_456 .
```

## Expected CST

```cst
(wor_dependent_tier
  (wor_tier_prefix)
  (tier_sep
    (colon)
    (tab))
  (wor_tier_body
    (wor_word_item
      (standalone_word
        (word_body
          (word_segment))))
    (whitespaces)
    (wor_word_item
      (standalone_word
        (word_body
          (word_segment))))
    (whitespaces)
    (period)
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
