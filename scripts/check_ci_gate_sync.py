#!/usr/bin/env python3
"""Assert that CI and the local gate cannot disagree about what must pass.

# The problem this exists to end

This repository has had four independent lists of "what must pass": the CI
workflows, `just gate`, `just test-all`, and a per-machine `.git/hooks/pre-push`
that printed "fast gate passed" after running two of twenty checks. Every one
could drift from every other, and on 2026-08-12 all four had: a push went out
with a broken doctest and a 997-line stale `parser.c`, having been told it
passed.

Writing a fifth, more carefully worded list does not fix that. The cure is the
one this project's own doctrine names for duplicated knowledge: ONE OWNER, and
a check that fails when anything else claims to be it.

# The invariant

**Every check CI runs is a `just` recipe, and `just gate` runs all of them.**

The justfile owns the commands. The workflows own triggers, matrix, caching and
provisioning, which is what YAML is actually for. That makes "the gate is what
CI runs" true by construction rather than by someone having compared two lists
correctly today.

Two directions are checked, because a one-directional check is how the last
four lists drifted:

- a `run:` step in a workflow that is neither provisioning nor `just <recipe>`
  FAILS: CI would be running a command nothing local owns;
- a recipe CI invokes that `gate` does not run FAILS, unless it is exempt for
  a stated reason (below).

# Exemptions are named, not implied

`EXEMPT_FROM_GATE` holds the checks that genuinely cannot run on one developer
machine, each with the reason. That list is deliberately short and each entry
has to justify itself; "it is slow" is not a reason, because the gate's whole
purpose is to be slower than a mistake.

Exit codes: 0 in sync, 1 drifted.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = REPO_ROOT / ".github" / "workflows"

#: Workflows whose steps must be `just` recipes. Others (release automation,
#: scheduled drift probes) are not part of the "must pass to push" contract.
GATED_WORKFLOWS = ("ci.yml", "book.yml", "crates-io-foundation.yml", "cross-platform.yml")

#: Workflow steps that still run a raw command, recorded so they cannot grow.
#:
#: A ratchet, following `scripts/doc-dates-baseline.txt` and the `UNPROTECTED`
#: list in `content_catch_alls.rs`: the entries here are the debt as measured on
#: 2026-08-13, the check FAILS on anything not listed, and it equally fails on a
#: listed entry that has been converted but left in the file. So the list can
#: only shrink, and the day it is empty the justfile owns every command CI runs.
#:
#: Keyed by "workflow:first line of the command" so a converted step stops
#: matching and is reported as stale.
BASELINE = REPO_ROOT / "scripts" / "ci-gate-baseline.txt"

#: The composite action that owns the Tauri Linux system-library list, and the
#: marker proving a workflow is naming those libraries itself instead.
#:
#: Provisioning is exempt from the raw-command ratchet above, since installing a
#: tool is not a check. That exemption left the PROVISIONING half of CI entirely
#: ungoverned, and it bit on 2026-08-27: the list sat inline in four workflows,
#: three of them carrying a comment saying "same list as ci.yml", and a fifth
#: Linux workflow (`release-lint.yml`) was written without it. It failed on the
#: v0.16.0 release tag, and no local gate could have caught it, because CI is
#: Linux and development is macOS.
#:
#: What this checks, exactly: the libraries are named in ONE place. What it does
#: NOT check, stated so nobody trusts it for more: whether a Linux job that
#: NEEDS them remembered to call the action at all. That one is not expressible
#: as a list without becoming the hand-maintained mirror this file exists to
#: argue against, because six Linux jobs run `just` and legitimately do not need
#: them. The root fix is ci.yml's own standing proposal: take chatter-desktop
#: out of the default workspace build.
TAURI_DEPS_ACTION = ".github/actions/tauri-linux-deps"
TAURI_DEPS_MARKER = "libwebkit2gtk"

#: Recipes CI runs that `gate` deliberately does not, each with its reason.
EXEMPT_FROM_GATE: dict[str, str] = {
    # Empty on purpose. Nothing has yet earned an exemption: every command CI
    # runs is either already a recipe the gate runs, or still raw and recorded
    # in the baseline. An entry here must name a check that CANNOT run on one
    # machine, and "it is slow" is not that reason.
}

#: `run:` bodies that are provisioning rather than checks. These legitimately
#: stay as raw commands: they install the tools the recipes then use.
PROVISIONING = re.compile(
    r"^\s*(sudo\s+apt|apt-get|npm\s+(ci|install)|cargo\s+install|"
    r"rustup\s|pip\s+install|brew\s+install|echo\s|mkdir\s)"
)

_RUN_BLOCK = re.compile(r"^(\s*)-?\s*run:\s*(\|[-+]?)?\s*(.*)$")
_JUST_CALL = re.compile(r"\bjust\s+([a-z0-9][a-z0-9-]*)")


def workflow_run_steps(path: Path) -> list[tuple[int, str]]:
    """Every `run:` step body in `path`, as (line number, body)."""
    steps: list[tuple[int, str]] = []
    lines = path.read_text(encoding="utf-8").splitlines()
    i = 0
    while i < len(lines):
        match = _RUN_BLOCK.match(lines[i])
        if not match:
            i += 1
            continue
        indent, block, inline = match.group(1), match.group(2), match.group(3)
        start = i + 1
        if not block:
            steps.append((start, inline))
            i += 1
            continue
        # Block scalar: take the more-indented lines that follow.
        body: list[str] = []
        i += 1
        while i < len(lines):
            line = lines[i]
            if line.strip() and not line.startswith(indent + " "):
                break
            body.append(line)
            i += 1
        steps.append((start, "\n".join(body)))
    return steps


def workflow_jobs(path: Path) -> list[tuple[str, list[str]]]:
    """Each job in `path`, as (name, its lines)."""
    lines = path.read_text(encoding="utf-8").splitlines()
    starts = [
        i for i, line in enumerate(lines) if re.match(r"^  [a-z][a-z0-9-]*:\s*$", line)
    ]
    jobs = []
    for a, b in zip(starts, starts[1:] + [len(lines)]):
        jobs.append((lines[a].strip().rstrip(":"), lines[a:b]))
    return jobs


def gate_recipes() -> set[str]:
    """Every recipe `gate` runs, following dependencies transitively.

    `gate` was once `gate: gate-fast gate-slow` with no body of its own, so
    reading only its body found nothing, and an earlier cut reported
    "justfile has no `gate` recipe": failed closed, correctly, for the wrong
    reason. A recipe's work is its body PLUS its dependencies, and this walks
    both, so `gate` may be a single recipe or a tree of them.
    """
    text = (REPO_ROOT / "justfile").read_text(encoding="utf-8")
    bodies: dict[str, tuple[list[str], str]] = {}
    for match in re.finditer(
        r"^([a-z][a-z0-9-]*)\s*(\*?[A-Za-z_]*)?:([^\n]*)\n((?:    [^\n]*\n|\n)*)",
        text,
        re.M,
    ):
        name, deps, body = match.group(1), match.group(3), match.group(4)
        bodies[name] = (deps.split(), body)

    if "gate" not in bodies:
        raise SystemExit("justfile has no `gate` recipe; this checker is stale")

    seen: set[str] = set()
    found: set[str] = set()
    stack = ["gate"]
    while stack:
        name = stack.pop()
        if name in seen or name not in bodies:
            continue
        seen.add(name)
        deps, body = bodies[name]
        found.update(_JUST_CALL.findall(body))
        for dep in deps:
            found.add(dep)
            stack.append(dep)
    return found


def read_baseline() -> set[str]:
    """Recorded raw-command steps, one `workflow:command` per line."""
    if not BASELINE.exists():
        return set()
    return {
        line.strip()
        for line in BASELINE.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.startswith("#")
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="accepted for symmetry; always checks"
    )
    parser.add_argument(
        "--write-baseline",
        action="store_true",
        help="record the current raw-command steps as the debt to shrink",
    )
    args = parser.parse_args()

    gate = gate_recipes()
    failures: list[str] = []
    ci_recipes: set[str] = set()
    raw: set[str] = set()
    raw_detail: dict[str, int] = {}

    for name in GATED_WORKFLOWS:
        path = WORKFLOWS / name
        if not path.exists():
            failures.append(f"{name}: listed in GATED_WORKFLOWS but does not exist")
            continue
        for line, body in workflow_run_steps(path):
            stripped = body.strip()
            if not stripped:
                continue
            called = _JUST_CALL.findall(body)
            if called:
                ci_recipes.update(called)
                continue
            if PROVISIONING.match(stripped):
                continue
            first = stripped.splitlines()[0].strip()
            raw.add(f"{name}:{first}")
            raw_detail[f"{name}:{first}"] = line

    # The Tauri system libraries are named in exactly one place. Every
    # workflow, not just the gated ones: the workflow that broke was
    # `release-lint.yml`, which GATED_WORKFLOWS does not list.
    owner = REPO_ROOT / TAURI_DEPS_ACTION / "action.yml"
    if not owner.exists():
        failures.append(
            f"{TAURI_DEPS_ACTION}/action.yml is missing; it is the one owner "
            f"of the Tauri Linux system-library list."
        )
    for path in sorted(WORKFLOWS.glob("*.yml")):
        if TAURI_DEPS_MARKER not in path.read_text(encoding="utf-8"):
            continue
        failures.append(
            f"{path.name}: names the Tauri system libraries inline. There is "
            f"one owner:\n"
            f"      uses: ./{TAURI_DEPS_ACTION}"
        )

    # A job that calls `just` must install it. GitHub's runners do not ship
    # `just`, so converting a step to `just <recipe>` without adding the install
    # turns the job red with "command not found" -- which is a CI-only failure
    # the local gate cannot possibly catch, and I made it converting nine steps.
    for name in GATED_WORKFLOWS:
        path = WORKFLOWS / name
        if not path.exists():
            continue
        for job, block in workflow_jobs(path):
            if not any("run: just " in line for line in block):
                continue
            if not any("tool: just" in line for line in block):
                failures.append(
                    f"{name}: job `{job}` runs `just` but never installs it. "
                    f"Add a `taiki-e/install-action@v2` step with `tool: just`."
                )

    for recipe in sorted(ci_recipes - gate):
        if recipe in EXEMPT_FROM_GATE:
            continue
        failures.append(
            f"CI runs `just {recipe}` and `just gate` does not.\n"
            f"      Add it to `gate`, or to EXEMPT_FROM_GATE with the reason "
            f"one machine cannot run it."
        )

    for recipe in sorted(EXEMPT_FROM_GATE):
        if recipe not in ci_recipes:
            failures.append(
                f"EXEMPT_FROM_GATE lists `{recipe}`, which no gated workflow "
                f"runs. Delete the entry; a stale exemption covers nothing."
            )

    if args.write_baseline:
        BASELINE.write_text(
            "# Workflow steps still running a raw command instead of a `just`\n"
            "# recipe. This list may only SHRINK: a new raw command fails the\n"
            "# check, and so does an entry here that has been converted.\n"
            "#\n"
            "# Empty means the justfile owns every command CI runs, which is\n"
            "# the point: `just gate` is then CI by construction.\n"
            + "".join(f"{entry}\n" for entry in sorted(raw)),
            encoding="utf-8",
        )
        print(f"recorded {len(raw)} raw-command step(s) in {BASELINE.name}")
        return 0

    accepted = read_baseline()
    for entry in sorted(raw - accepted):
        failures.append(
            f"{entry.split(':', 1)[0]}:{raw_detail[entry]}: runs a command the "
            f"justfile does not own:\n"
            f"      {entry.split(':', 1)[1][:72]}\n"
            f"      Give it a `just` recipe and call that, so `just gate` runs "
            f"it too."
        )
    for entry in sorted(accepted - raw):
        failures.append(
            f"baseline entry no longer applies, delete it: {entry[:80]}"
        )

    if failures:
        print("CI and `just gate` have drifted:\n", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        print(
            f"\n{len(failures)} problem(s). The justfile owns the commands; "
            f"workflows own triggers, matrix and provisioning.",
            file=sys.stderr,
        )
        return 1

    print(
        f"ci/gate sync: ok ({len(ci_recipes)} recipe(s) shared, "
        f"{len(raw)} raw command(s) still in the baseline, "
        f"{len(EXEMPT_FROM_GATE)} exempt)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
