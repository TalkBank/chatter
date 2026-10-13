# overlap_utterance

Utterance with a word containing overlap markers

## Input

```main_tier
*CHI:	who ⌈is⌉ ?
```

## Expected CST

```cst
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
                (overlap_point)
                (word_segment)
                (overlap_point))))))
      (whitespaces))
    (utterance_end
      (question)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: overlap
