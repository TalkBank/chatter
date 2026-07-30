#!/usr/bin/env bash
# Tag and push a chatter release, refusing unless every precondition holds.
#
# This mechanizes the tag step so a release tag can never race its own CI:
# the 2026-07-30 v0.5.0 release tagged a bump commit before that commit's CI
# reported, and the tag-triggered desktop build failed on drift that CI
# flagged three minutes later. The tag is the release trigger, so everything
# a release depends on is verified HERE, fail-closed:
#
#   1. clean working tree;
#   2. HEAD is exactly origin/main (the tagged commit is the pushed commit);
#   3. every literal version copy agrees with the tag (sync-app-version.py
#      --check --release-tag, which also requires the CHANGELOG section);
#   4. the CI and Cross-platform workflows have completed SUCCESSFULLY for
#      this exact commit (not merely started, not an older commit's runs).
#
# Usage: scripts/release-tag.sh X.Y.Z   (no leading v; the tag becomes vX.Y.Z)
set -euo pipefail

version=${1:?usage: release-tag.sh X.Y.Z (no leading v)}
case "$version" in
v*) echo "error: pass X.Y.Z without the leading v (the script adds it)" >&2; exit 2 ;;
esac
tag="v${version}"

cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain)" ]; then
    echo "error: working tree not clean; commit or discard first" >&2
    exit 1
fi

git fetch origin main
head=$(git rev-parse HEAD)
if [ "$head" != "$(git rev-parse origin/main)" ]; then
    echo "error: HEAD is not origin/main; push (or pull) before tagging" >&2
    exit 1
fi

python3 scripts/sync-app-version.py --check --release-tag "$tag"

# Both required workflows must have a completed+successful run for THIS
# commit. gh returns runs newest-first; one line per workflow is enough.
for workflow in "CI" "Cross-platform"; do
    conclusion=$(gh run list --commit "$head" --workflow "$workflow" \
        --limit 1 --json status,conclusion \
        --jq 'if length == 0 then "absent" elif .[0].status != "completed" then .[0].status else .[0].conclusion end')
    if [ "$conclusion" != "success" ]; then
        echo "error: workflow '$workflow' for $head is '$conclusion', not success;" >&2
        echo "       wait for CI on the pushed commit before tagging" >&2
        exit 1
    fi
    echo "workflow '$workflow': success"
done

git tag -a "$tag" -m "chatter $tag"
git push origin "$tag"
echo "tagged and pushed $tag at $head; release workflows are now running"
