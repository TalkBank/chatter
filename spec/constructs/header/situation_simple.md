# situation_simple

`@Situation` header with free-text scene description.

## Input

```situation_header
@Situation:	free play at table
```

## Expected CST

```cst
(situation_header
  (situation_prefix)
  (header_sep
    (colon)
    (tab))
  (free_text
    (rest_of_line))
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
