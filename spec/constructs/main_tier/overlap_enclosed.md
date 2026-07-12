# overlap_enclosed

Word enclosed in overlap markers. Custody follows the whitespace-boundary
principle (ideal overlap model, ported 2026-07-11): each marker's OUTER
side touches whitespace (or the utterance edge), so both markers are
TOP-LEVEL content items and the enclosed word is a plain word. The
markers' span relation is derived by the typed model (OverlapGroup /
pairing), not by the grammar.

## Input

```main_tier
*CHI:	⌈is⌉ .
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
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: overlap
