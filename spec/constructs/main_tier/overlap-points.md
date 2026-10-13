# Overlap Points

Overlap points mark the beginning and end of overlapping speech in CHAT transcripts.

## Action Within Overlap

When an action marker `0` appears within overlap markers, they should be parsed as separate elements:

- `⌈` - overlap point (top begin)
- `0` - action (omitted action)
- content (e.g., events, words)
- `⌉` - overlap point (top end)

### Example

```chat
*CHI:	⌈0 &=laughter⌉ .
```

**Expected parse:**
1. `⌈` - overlap_point (top begin marker)
2. `0` - nonword_with_optional_annotations containing the omitted-action marker
3. `&=laughter` - nonword_with_optional_annotations containing the event
4. `⌉` - overlap_point (top end marker)
5. `.` - terminator

**NOT** parsed as:
- ❌ `⌈0` as a single word with overlap inside

## Expected CST

```cst
(main_tier
  (star)
  (speaker)
  (colon)
  (tab)
  (tier_body
    (contents
      (overlap_point)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (zero)))))
      (whitespaces)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (event
                (event_marker)
                (event_segment))))))
      (overlap_point)
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier

## References

- [CHAT Manual - Overlaps](https://talkbank.org/0info/manuals/CHAT.html#_Toc110793493)
- [CHAT Manual - Actions](https://talkbank.org/0info/manuals/CHAT.html#OmittedWord_Code)
