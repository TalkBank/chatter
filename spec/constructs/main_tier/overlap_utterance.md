# overlap_utterance

Utterance with an overlap-wrapped word. Custody follows the
whitespace-boundary principle (ideal overlap model, ported 2026-07-11):
both markers touch whitespace on their outer side, so they are TOP-LEVEL
content items; `is` is a plain word. Span pairing is model-derived.

## Input

```main_tier
*CHI:	who ⌈is⌉ ?
```

## Expected CST

```cst
(main_tier
  (star)
  speaker: (speaker)
  (colon)
  (tab)
  (tier_body
    content: (contents
      (content_item
        (base_content_item
          (word_with_optional_annotations
            word: (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (overlap_point)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            word: (standalone_word
              (word_body
                (word_segment))))))
      (overlap_point)
      (whitespaces))
    ending: (utterance_end
      (question)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: overlap
