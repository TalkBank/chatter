# interruption_terminator

Main tier ending with interrupted terminator `+/.'`.

## Input

```main_tier
*CHI:	hello +/.
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
      (whitespaces))
    (utterance_end
      (interruption)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
