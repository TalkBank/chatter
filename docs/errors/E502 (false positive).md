# E502 (false positive): E502 false positive: %wor parse error cascades to entire file

> ✅ Active; This check is active in the validator.

**Severity**: error

**Status**: ✅ Active

## Description

When a %wor tier contains invalid content (e.g., an action marker like &=head:no) AND the %wor line has 7+ words after the error, tree-sitter's error recovery fails catastrophically: instead of isolating the ERROR to the %wor tier, the entire file becomes one ERROR node. This causes:

## How to Fix



