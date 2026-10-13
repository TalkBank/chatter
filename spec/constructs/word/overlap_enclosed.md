# overlap_enclosed

Word enclosed in overlap markers

## Input

```standalone_word
⌈is⌉
```

## Expected CST

```cst
(standalone_word
  (word_body
    (overlap_point)
    (word_segment)
    (overlap_point)))
```

## Metadata

- **Level**: word
- **Category**: overlap
