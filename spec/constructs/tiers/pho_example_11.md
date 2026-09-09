# pho_example_11

Example %pho tier with superscript tone digits and a modifier tone letter: ma¹⁵ kjaꜞ

## Input

```pho_dependent_tier
%pho:	ma¹⁵ kjaꜞ
```

## Expected CST

```cst
(pho_dependent_tier
  (pho_tier_prefix)
  (tier_sep
    (colon)
    (tab)
  )
  (pho_groups
    (pho_group
      (pho_words
        (pho_word)
      )
    )
    (whitespaces
      (whitespace
        (space)
      )
    )
    (pho_group
      (pho_words
        (pho_word)
      )
    )
  )
  (newline)
)
```

## Metadata

- **Level**: tier
- **Category**: tiers
