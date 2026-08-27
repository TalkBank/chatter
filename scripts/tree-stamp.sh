#!/usr/bin/env bash
# tree-stamp.sh: a hash of the WORKING TREE'S CONTENT, tracked or not.
#
# The gate stamps and the pre-push hook both need one question answered:
# "did the gate run on the bytes I am about to push?" The previous answer,
# `git status --porcelain | shasum`, hashed the LIST OF MODIFIED PATHS, so a
# gate run on a dirty tree could never match the clean tree that a commit of
# the same content produces. Every gate run before a commit was wasted, and
# the only working flow was commit, gate, push, with a re-gate after any
# amend. On 2026-08-27 that cost six gate runs for one change.
#
# This hashes CONTENT: a throwaway index is filled from the working tree with
# the same ignore rules as a real `git add -A`, and `write-tree` names it.
# Identical bytes give an identical stamp whether they are staged, committed
# or still loose, and the real index is never touched.
set -euo pipefail
# `mktemp` reserves a NAME by creating an empty file, and an empty file is not
# a valid index: `git add -A` refuses it. The name is what is wanted; git
# creates a fresh index at a path that does not exist. The first version of
# this script skipped that step, printed nothing, and the push hook then
# compared an empty stamp to an empty stamp and would have passed anything.
idx=$(mktemp)
rm -f "$idx"
trap 'rm -f "$idx"' EXIT
export GIT_INDEX_FILE="$idx"
git add -A
stamp=$(git write-tree)
case "$stamp" in
    [0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]*) printf '%s\n' "$stamp" ;;
    *) echo "tree-stamp: write-tree produced no tree id" >&2; exit 1 ;;
esac
