# activities_simple

`@Activities` header with simple free-text content.

## Input

```activities_header
@Activities:	play
```

## Expected CST

```cst
(activities_header
  (activities_prefix)
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
