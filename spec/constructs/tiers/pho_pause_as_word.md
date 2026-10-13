# pho_pause_as_word

In %pho and %mod tiers, `(..)` is a phonological word, NOT a pause.
The `pho_word` regex in grammar.js includes parentheses and dots in its
character class. Pauses are main-tier constructs only.

Found in: phon-eng-french-data/Eng-NA/Goad (227+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```utterance
*CHI:	hello there .
%pho:	wow (..) kijz .
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
        (whitespaces)
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
  (pho_dependent_tier
    (pho_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (pho_groups
      (pho_group
        (pho_words
          (pho_word)))
      (whitespaces)
      (pho_group
        (pho_words
          (pho_word)))
      (whitespaces)
      (pho_group
        (pho_words
          (pho_word)))
      (whitespaces)
      (pho_group
        (pho_words
          (pho_word))))
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
