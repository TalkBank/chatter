#!/usr/bin/env bash
# Sourced by a single gate process; only its successful, unchanged run records a receipt.

gate_begin() {
    GATE_RECEIPT_PATH="$(git rev-parse --git-path gate-passed)"
    # Invalidate a previous receipt before starting a new verification attempt.
    : > "$GATE_RECEIPT_PATH"
    GATE_RECEIPT_TREE="$(bash scripts/tree-stamp.sh)"
}

gate_finish() {
    local final_tree
    final_tree="$(bash scripts/tree-stamp.sh)"
    if [[ "$final_tree" != "$GATE_RECEIPT_TREE" ]]; then
        echo 'gate: content changed during verification; run the gate on the final tree.' >&2
        return 1
    fi
    printf '%s\n' "$final_tree" > "$GATE_RECEIPT_PATH"
}
