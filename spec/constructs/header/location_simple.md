# location_simple

`@Location` header with free-text location value.

## Input

```location_header
@Location:	lab room 1
```

## Expected CST

```cst
(location_header
  (location_prefix)
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
