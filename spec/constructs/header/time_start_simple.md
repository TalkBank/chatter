# time_start_simple

`@Time Start` header with strict time syntax.

## Input

```time_start_header
@Time Start:	17:30-18:00
```

## Expected CST

```cst
(time_start_header
  (time_start_prefix)
  (header_sep
    (colon)
    (tab))
  (time_duration_contents
    (strict_time))
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
