#!/usr/bin/env bash
#
# Report hooks that `core.hooksPath` has just displaced.
#
# Pointing git at a tracked hooks directory makes it ignore `.git/hooks`
# ENTIRELY. Anything a contributor installed there stops running, with no
# message from git and no failure: the person who most needs to know is the one
# who will not notice.
#
# Prints what is now inert, and whether this repository offers that event a
# `.local` chaining seam to restore it. Never fails the caller: it is a report,
# and a clone with nothing installed is the normal case.
set -euo pipefail

# Whether the tracked hook for this event chains to a `.local` sibling.
#
# Tests the FACT, not a proxy. Checking merely that `$tracked_dir/$name` exists
# would tell a contributor to install a `.local` hook the tracked one never
# execs, the moment an event is added without a chain.
chains_to_local() {
    grep -q "hooks/$1.local" "$tracked_dir/$1" 2> /dev/null
}

hooks_dir="$(git rev-parse --git-dir)/hooks"
tracked_dir="$(git rev-parse --path-format=absolute --git-path hooks)"

displaced=()
for hook in "$hooks_dir"/*; do
    [ -f "$hook" ] || continue
    [ -x "$hook" ] || continue
    name="${hook##*/}"
    case "$name" in
        *.sample | *.local) continue ;;
    esac
    # Already chained: a `.local` sibling exists AND the tracked hook for this
    # event execs it, so this hook's work still happens and there is nothing to
    # report. Without this the script fired on a CORRECTLY configured checkout,
    # which is how a warning gets trained out of the reader it is for.
    if [ -x "$hook.local" ] && chains_to_local "$name"; then
        continue
    fi
    displaced+=("$name")
done

if [ ${#displaced[@]} -eq 0 ]; then
    exit 0
fi

echo ""
echo "NOTE: core.hooksPath now points at $tracked_dir, so git IGNORES .git/hooks."
echo "      These were installed there and will no longer run:"
for name in "${displaced[@]}"; do
    if chains_to_local "$name"; then
        echo "        $name    -> chain it: install it as .git/hooks/$name.local"
    else
        echo "        $name    -> INERT: $tracked_dir/$name offers no .local seam,"
        echo "                        so this event has no way back yet."
    fi
done
echo ""
