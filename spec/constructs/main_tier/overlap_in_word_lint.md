# overlap_in_word_lint

Word with internal overlap markers. The INTERIOR marker (spoken text
glued on BOTH sides, `butt⌈er`) stays inside the word; the TRAILING
marker's outer side touches whitespace, so it is a top-level content
item (whitespace-boundary custody, ideal overlap model 2026-07-11).

Historically this spec also gated a precedence-propagation lint
(prec(6) on `standalone_word` does not propagate through `word_body`);
the interior half of that guarantee is retained here.

## Input

```main_tier
*CHI:	butt⌈er⌉ .
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
                (word_segment)
                (overlap_point)
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
