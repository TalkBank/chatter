#!/usr/bin/env bash
# doc-date-triage.sh: turn the doc-dates backlog into a work list.
#
# `check_doc_dates.py` says WHICH pages claim a date older than their last
# commit. It cannot say WHY, and the why decides the work:
#
#   SQUASH-ONLY  the only commits after the claimed date are history rewrites
#                that touched the file without changing its bytes. The page may
#                well still be correct; the claim is wrong about the published
#                history. Verify the content, then stamp.
#   CONTENT      real commits changed the bytes after the claimed date. The
#                page must be READ against those diffs before any stamp.
#   NO-DATE      the page carries no date header at all.
#
# Prints one line per baseline entry: CLASS, commits-after-date, lines, path.
# Reading only; it never edits a page, because stamping without reading is the
# exact failure the ratchet exists to prevent.
set -euo pipefail

BASELINE=${1:-scripts/doc-dates-baseline.txt}
# Commits that rewrote history without authoring content. Extend deliberately.
SQUASHES='Initial public release'

while IFS= read -r doc; do
    case "$doc" in ''|'#'*) continue ;; esac
    [ -f "$doc" ] || { printf 'MISSING   %5s %6s %s\n' - - "$doc"; continue; }

    stated=$(grep -m1 -E '^\*\*Last (modified|updated):\*\*' "$doc" \
        | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' || true)
    if [ -z "$stated" ]; then
        printf 'NO-DATE   %5s %6s %s\n' - "$(wc -l < "$doc" | tr -d ' ')" "$doc"
        continue
    fi

    # Commits touching the file strictly after the claimed day. `--after` is
    # inclusive of the day, so ask for the day after to avoid counting edits
    # made on the very day the page claims.
    after=$(git log --since="$stated 23:59:59" --format='%s' -- "$doc" || true)
    total=$(printf '%s' "$after" | grep -c . || true)
    real=$(printf '%s' "$after" | grep -vcE "$SQUASHES" || true)

    if [ "$total" -eq 0 ]; then
        class=CURRENT
    elif [ "$real" -eq 0 ]; then
        class=SQUASH-ONLY
    else
        class=CONTENT
    fi
    printf '%-9s %5s %6s %s\n' "$class" "$real" "$(wc -l < "$doc" | tr -d ' ')" "$doc"
done < "$BASELINE"
