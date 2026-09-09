#!/usr/bin/env python3
"""Turn an llvm-cov export into an attribution worklist.

The completeness criterion in `book/src/contributing/correctness-architecture.md`
asks for one row per uncovered branch, each carrying a VERDICT from a closed
set, and gates on the verdict rather than on a percentage. A percentage is a
number to be gamed; a verdict is a statement a human made, with a reason, that a
later measurement can contradict.

This is the first half: it produces the rows and the discriminator that sorts
them, so the verdicts can be assigned to something concrete. It does not gate.

USAGE

    cargo +nightly llvm-cov --branch -p <crate> --tests --json --output-path cov.json
    python3 scripts/coverage_attribution.py cov.json --scope crates/talkbank-model/src/validation

WHAT THIS DELIBERATELY DOES NOT PRODUCE

`covered_by` and `witnesses`, the fields the design gives each row for naming
WHICH test reaches a region. A single llvm-cov export cannot answer that: it
merges every test binary into one profile, so a covered region knows it was
reached and not by whom. Deriving those needs one instrumented run per test,
which is a different and far more expensive instrument. Saying so here is
cheaper than letting the next reader conclude the export is lossy.

TWO THINGS THE EXPORT GETS WRONG IF YOU READ IT NAIVELY, both measured

1. GENERIC FUNCTIONS APPEAR ONCE PER INSTANTIATION. `check_id_header` is in the
   export twice, once monomorphized over `RecordingSink` and once over
   `ErrorCollector`, with different mangled hashes and independent region
   counts. Counting rows without merging them double-counts the worklist and
   makes the delete-versus-fixture ratio meaningless, because one instantiation
   can be fully covered while its twin is not. Rows are merged on (file,
   demangled path) and a region counts as covered when ANY instantiation covered
   it, which is what "this code ran" means.

2. `--lib` CANNOT SEE THE SPEC SYSTEM. chatter's normative evidence is 436 error
   fixtures, 138 construct tests and 107 reference files, all of which live in
   integration binaries. Under `--lib` the core validation entry points read as
   ZERO covered: `check_id_header`, `check_header` and `validate_alignments` all
   do, measured. And `-p <crate> --tests` is no better in the other direction:
   it reports only that crate's own files, so running the fixture suite that way
   attributes nothing to the validator the fixtures exercise. Neither failure is
   visible in the export, so this tool makes the caller state the command with
   `--produced-by` and records it beside the rows.

AND THE ONE THAT MATTERS MOST, because it moves the number the wrong way

A test that FABRICATES its input still executes the code under it, so it covers
branches. chatter has hundreds of those: `talkbank-model` declares no parser
dependency, so every test in it hand-builds the AST it then judges. Coverage
bought that way is worse than no coverage, because it converts "nobody has shown
this rule firing on a real file" into a green number. Pass `--fabrication-floor`
with an export from those tests ALONE and the report separates the regions only
they reach; that set is not coverage in the sense the criterion means, and it is
a deletion worklist rather than a gap one.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

# Committed files that a generator writes. A mutant, an uncovered branch or a
# surviving fabrication in one of these indicts the GENERATOR, not this
# repository, so they are out of scope for every verdict.
#
# Derived from the one property that distinguishes a generated file from a file
# ABOUT generation: the marker appears in its HEADER. A name-based rule cannot
# make that distinction, and the repository's shell predicate does not:
# `generated_*.rs` also matches `generated_traversal_current.rs`, a
# hand-written test OF the generated traversal.
GENERATED_HEADER_LINES = 8
GENERATED_MARKER = re.compile(r"@generated|DO NOT EDIT|do not edit", re.IGNORECASE)


def is_generated(repo_root: Path, rel: str) -> bool:
    """Whether `rel` says in its own header that a generator wrote it."""
    path = repo_root / rel
    try:
        with path.open(encoding="utf-8", errors="replace") as handle:
            head = "".join(next(handle, "") for _ in range(GENERATED_HEADER_LINES))
    except OSError:
        # A path in the export that is not in the tree is not a generated file;
        # it is a stale export or a path outside the repository, and either way
        # the caller wants to see it rather than have it silently dropped.
        return False
    return bool(GENERATED_MARKER.search(head))


# Rust v0 mangled names are a sequence of length-prefixed identifiers. Full
# demangling needs the whole grammar; the readable PATH is all a worklist needs,
# and it is exactly the length-prefixed run.
_SEGMENT = re.compile(r"(\d+)")


# A v0 symbol for a GENERIC function carries its instantiation types after the
# function path, introduced by a backreference like `B6_`. Reading segments to
# the end of the string therefore returns the type arguments, not the function:
# the first draft of this reported `ErrorCollector` and `NullErrorSink` as the
# names of the two largest unreached functions in the validator, which sent the
# worklist at the sink types instead of at `check_id_header` and
# `ProsodicWord::check`. Truncating at the first backreference is what keeps the
# path.
_BACKREF = re.compile(r"B\d+_")


def readable_path(mangled: str) -> str:
    """The `module::module::function` path inside a v0 mangled symbol.

    Falls back to the raw symbol rather than to a guess: a name this cannot
    parse is still a unique key, and a wrong-but-plausible name in a worklist
    sends someone to the wrong function.
    """
    backref = _BACKREF.search(mangled)
    if backref is not None:
        mangled = mangled[: backref.start()]
    segments: list[str] = []
    index = 0
    while index < len(mangled):
        match = _SEGMENT.search(mangled, index)
        if match is None:
            break
        length = int(match.group(1))
        start = match.end()
        # A leading underscore separates a length from an identifier that would
        # otherwise start with a digit.
        if start < len(mangled) and mangled[start] == "_":
            start += 1
        piece = mangled[start : start + length]
        if piece and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", piece):
            segments.append(piece)
            index = start + length
        else:
            index = match.end()
    # Drop crate-hash noise and leading crate name duplication.
    useful = [s for s in segments if not s.startswith("Cs") and len(s) > 1]
    return "::".join(useful[-3:]) if useful else mangled


@dataclass
class FunctionRow:
    """One function, merged across every instantiation of it."""

    file: str
    path: str
    total: int = 0
    uncovered: int = 0
    # Uncovered branch regions, as (line, column), sorted and deduplicated.
    branches: set[tuple[int, int]] = field(default_factory=set)

    @property
    def ratio(self) -> float:
        """Uncovered over total: 1.0 means nothing in this function ran."""
        return self.uncovered / self.total if self.total else 0.0


# llvm-cov region kinds. 4 is a branch region under --branch.
REGION_KIND_BRANCH = 4


def build_rows(
    export: dict, repo_root: Path, scope: str
) -> tuple[dict[tuple[str, int, int], FunctionRow], list[str]]:
    """Merged rows, plus any file where this disagrees with llvm-cov itself.

    # Why this is keyed on SOURCE POSITION and not on the function name

    The export lists a generic function once per instantiation, and twice more
    when the crate is compiled both as a lib-test target and as a dependency, so
    the same source function appears under several mangled symbols with
    DIFFERENT crate hashes. Keying on a demangled name split `check_id_header`
    across buckets and reported it as entirely unreached, while llvm-cov's own
    summary for that file says 160 of 161 regions are covered. The name is a
    display concern; the identity of a region is where it is in the source.

    So coverage is merged over (file, line, column) with "covered" meaning ANY
    instantiation ran it, which is what "this code ran" means, and the result is
    CHECKED against the export's own per-file summary. A disagreement is
    returned rather than printed, because a measurement that silently disagrees
    with its own source is the thing this repository keeps finding.
    """
    data = export["data"][0]
    # (file, line, col) -> covered by anything
    positions: dict[tuple[str, int, int, int, int], bool] = {}
    # (file, line, col) -> the function span that owns it, for display
    owner: dict[tuple[str, int, int, int, int], tuple[int, int]] = {}
    names: dict[tuple[str, int, int], str] = {}

    for function in data.get("functions", []):
        filenames = [relative(name) for name in function.get("filenames", [])]
        if not filenames or not filenames[0].startswith(scope):
            continue
        file = filenames[0]
        if is_generated(repo_root, file):
            continue
        regions = function.get("regions", [])
        if not regions:
            continue
        span = (min(r[0] for r in regions), max(r[2] for r in regions))
        for region in regions:
            # A region names its OWN file by index into `filenames`, because a
            # macro expansion puts a function's regions in the file the macro
            # was defined in. Attributing every region to `filenames[0]` moved
            # coverage from those files into this one and reported 474/506 where
            # llvm-cov said 451/506. The self-check caught it; nothing else
            # would have.
            region_file = filenames[region[5]] if region[5] < len(filenames) else file
            if not region_file.startswith(scope) or is_generated(repo_root, region_file):
                continue
            # The full span, not the start: `if c { a } else { b }` puts two
            # regions at one start position, and keying on the start merged
            # them. The self-check below is what caught that one.
            at = (region_file, region[0], region[1], region[2], region[3])
            positions[at] = positions.get(at, False) or region[4] > 0
            owner[at] = span
            names.setdefault(
                (region_file, span[0], span[1]), readable_path(function["name"])
            )

    rows: dict[tuple[str, int, int], FunctionRow] = {}
    for at, covered in positions.items():
        file, line, column = at[0], at[1], at[2]
        span = owner[at]
        key = (file, span[0], span[1])
        row = rows.setdefault(
            key, FunctionRow(file=file, path=names.get(key, f"{file}:{span[0]}"))
        )
        row.total += 1
        if not covered:
            row.uncovered += 1
            row.branches.add((line, column))

    # Self-check against llvm-cov's own arithmetic, per file.
    disagreements: list[str] = []
    mine: dict[str, tuple[int, int]] = {}
    for at, covered in positions.items():
        file = at[0]
        got, total = mine.get(file, (0, 0))
        mine[file] = (got + (1 if covered else 0), total + 1)
    for file_entry in data.get("files", []):
        file = relative(file_entry["filename"])
        if not file.startswith(scope) or is_generated(repo_root, file):
            continue
        theirs = file_entry["summary"]["regions"]
        if file not in mine:
            continue
        got, total = mine[file]
        if (got, total) != (theirs["covered"], theirs["count"]):
            disagreements.append(
                f"{file}: this tool says {got}/{total}, llvm-cov says "
                f"{theirs['covered']}/{theirs['count']}"
            )

    return {k: v for k, v in rows.items() if v.uncovered}, disagreements


def covered_positions(
    export: dict,
    repo_root: Path,
    scope: str,
    exclude: set[tuple[str, int, int, int, int]] | None = None,
) -> set[tuple[str, int, int, int, int]]:
    """Every (file, line, column) this export covered, within `scope`.

    Positions rather than counts, because the question is WHICH regions a run
    reaches, and two runs reaching different halves of a function report the
    same count.
    """
    covered: set[tuple[str, int, int, int, int]] = set()
    for function in export["data"][0].get("functions", []):
        filenames = [relative(name) for name in function.get("filenames", [])]
        if not filenames:
            continue
        for region in function.get("regions", []):
            if region[4] <= 0:
                continue
            # The region's OWN file and its FULL span, for the reasons
            # `build_rows` states: `filenames[0]` mis-attributes macro
            # expansions, and a start position alone merges the two arms of an
            # `if`. This function had both bugs after `build_rows` was fixed,
            # and they showed up as a scope reporting 117.5% coverage.
            region_file = filenames[region[5]] if region[5] < len(filenames) else filenames[0]
            if not region_file.startswith(scope) or is_generated(repo_root, region_file):
                continue
            at = (region_file, region[0], region[1], region[2], region[3])
            if exclude is None or at not in exclude:
                covered.add(at)
    return covered


def region_spans(
    export: dict, repo_root: Path, scope: str
) -> tuple[set[tuple[str, int, int, int, int]], set[tuple[str, int, int, int, int]]]:
    """(every region span in `scope`, the covered subset), from ONE walk.

    Numerator and denominator must come from the same pass. Taking the
    denominator from llvm-cov's per-file summary and the numerator from a walk
    of the functions reported 118.4% coverage for the validator, because a
    region can be expanded into an in-scope file from a function defined
    elsewhere and the two counts do not agree about whose it is.
    """
    every: set[tuple[str, int, int, int, int]] = set()
    covered: set[tuple[str, int, int, int, int]] = set()
    for function in export["data"][0].get("functions", []):
        filenames = [relative(name) for name in function.get("filenames", [])]
        if not filenames:
            continue
        for region in function.get("regions", []):
            region_file = filenames[region[5]] if region[5] < len(filenames) else filenames[0]
            if not region_file.startswith(scope) or is_generated(repo_root, region_file):
                continue
            at = (region_file, region[0], region[1], region[2], region[3])
            every.add(at)
            if region[4] > 0:
                covered.add(at)
    return every, covered


def relative(path: str) -> str:
    marker = "/chatter/"
    index = path.find(marker)
    return path[index + len(marker) :] if index >= 0 else path


# An export cannot say how it was produced, and I tried to make it.
#
# The first draft looked for a covered region under a `tests/` path and treated
# its absence as proof of a `--lib` run. Measured: llvm-cov reports no test
# source at all, so a `--tests` export has zero such files too, and the check
# refused every input for the same wrong reason. There is no field in the export
# that distinguishes `--lib` from `--tests`, and inventing one from a heuristic
# is precisely the fabricated measurement this repository keeps finding.
#
# So the caller states it, and the artifact records it. Which run produced a
# number is exactly the provenance an artifact is supposed to carry beside
# itself, and it is the difference between a floor and a figure.
PROVENANCE_HELP = (
    "the exact command that produced the export, recorded in the output. "
    "`--lib` runs cannot see the spec system (436 error fixtures, 138 construct "
    "tests, 107 reference files, all in integration binaries) and `-p <crate>` "
    "runs report only that crate's own files; both produce floors, and a floor "
    "read as a figure turns every verdict into NEEDS_SPEC_EXAMPLE for rules a "
    "fixture already reaches."
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("export", type=Path, help="llvm-cov --json output")
    parser.add_argument(
        "--scope",
        default="crates/",
        help="repository-relative path prefix to report on",
    )
    parser.add_argument("--limit", type=int, default=25, help="rows to print")
    parser.add_argument(
        "--produced-by",
        required=True,
        help=PROVENANCE_HELP,
    )
    parser.add_argument(
        "--fabrication-floor",
        type=Path,
        help=(
            "a second export produced by the FABRICATING tests alone, typically "
            "`-p talkbank-model --lib`. Any region covered there and nowhere "
            "else was reached by a hand-built model rather than by parsing "
            "CHAT, which inflates the number while proving nothing."
        ),
    )
    parser.add_argument(
        "--without-fabrication",
        type=Path,
        help=(
            "a third export: the same suite with the fabricating tests excluded "
            "from the RUN but not the REPORT "
            "(`--workspace --tests --exclude-from-test talkbank-model`). Only "
            "with this is the fabrication-only set exact rather than bounded."
        ),
    )
    parser.add_argument("--json", type=Path, help="write the full row set here")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    export = json.loads(args.export.read_text())

    # "Nothing to report" has two causes and they are opposite verdicts: the
    # scope is fully covered, or the scope is not in this export at all. The
    # first draft of this printed "no uncovered regions" for both and exited 0,
    # which is the vacuity this tool exists to find, in the tool.
    measured = sum(
        1 for file in export["data"][0].get("files", []) if relative(file["filename"]).startswith(args.scope)
    )
    if measured == 0:
        print(
            f"REFUSING: no file under {args.scope} is in this export, so it was\n"
            f"not instrumented and this is not a coverage result about it. An\n"
            f"export produced with `-p <crate>` reports only that crate's own\n"
            f"files, even when its tests exercise half the workspace; use\n"
            f"`--workspace` to attribute the code under test.",
            file=sys.stderr,
        )
        return 2

    fabrication_only: set[tuple[str, int, int]] = set()
    if args.fabrication_floor:
        floor = json.loads(args.fabrication_floor.read_text())
        fabricated = covered_positions(floor, repo_root, args.scope)
        full = covered_positions(export, repo_root, args.scope)
        # Covered in the fabricating run, and covered in the full run ONLY
        # because the fabricating tests are part of it. The full run is a
        # superset, so the discriminator is what the REST of the suite reaches:
        # anything the fabricating tests reach that nothing else does.
        # WHAT TWO EXPORTS CAN AND CANNOT SAY, because the first draft of this
        # got it wrong in the direction that overstates. The full run INCLUDES
        # the fabricating tests, so `full` is a superset of `fabricated` and no
        # subtraction between them isolates what the fabricating tests alone
        # reach. `|fabricated|` is therefore an UPPER BOUND on the inflation,
        # not the figure.
        #
        # The exact set needs a third export: the same suite with the
        # fabricating tests excluded from the RUN but not from the REPORT, and
        # then it is `fabricated - that`. Pass it as --without-fabrication.
        only_rest = full - fabricated
        print(f"FABRICATION EXPOSURE under {args.scope}:")
        print(f"  {len(full)} region(s) covered by the full suite")
        print(
            f"  {len(fabricated)} of them are reached by the fabricating tests, "
            f"an UPPER BOUND of {100 * len(fabricated) / len(full):.1f}% on how "
            f"much of this coverage could rest on a model no parse produces"
        )
        print(
            f"  {len(only_rest)} are reached by the rest of the suite and NOT by "
            f"the fabricating tests, so that much is parse-backed for certain"
        )
        if args.without_fabrication:
            without = covered_positions(
                json.loads(args.without_fabrication.read_text()), repo_root, args.scope
            )
            fabrication_only = fabricated - without
            print(
                f"  {len(fabrication_only)} are reached ONLY by fabricating "
                f"tests, exactly: {100 * len(fabrication_only) / len(full):.1f}% "
                f"of this scope's covered regions prove nothing about CHAT"
            )
        print()

    rows, disagreements = build_rows(export, repo_root, args.scope)
    # THREE BUCKETS, never two. A file this tool and llvm-cov disagree about is
    # neither reported nor silently folded in: it is named and excluded, with
    # its numbers, so a reader can see the size of what is not being claimed.
    # Reporting them anyway would be wrong in an unknown direction; dropping
    # them quietly would be a completeness this method does not have.
    if disagreements:
        excluded = {line.split(":")[0] for line in disagreements}
        rows = {key: row for key, row in rows.items() if row.file not in excluded}
        print(
            f"EXCLUDED, {len(disagreements)} file(s) where this tool's region "
            f"arithmetic disagrees with llvm-cov's own summary. Not reported "
            f"below, and not claimed either way:"
        )
        for line in disagreements:
            print(f"  {line}")
        print()

    if not rows:
        print(f"{measured} file(s) under {args.scope} measured, all fully covered")
        return 0

    dead = [r for r in rows.values() if r.ratio == 1.0]
    partial = [r for r in rows.values() if r.ratio < 1.0]
    print(f"{len(rows)} function(s) with uncovered regions under {args.scope}")
    print(
        f"  {len(dead)} entirely unreached (ratio 1.00): delete candidates, or "
        f"nothing in the suite calls them"
    )
    print(f"  {len(partial)} partially reached: fixture candidates")
    print()

    print("ENTIRELY UNREACHED, largest first. A function nothing runs is the")
    print("cheapest verdict to assign: it is dead code or it is missing a caller.")
    for row in sorted(dead, key=lambda r: -r.total)[: args.limit]:
        print(f"  {row.total:5} regions  {row.path:50} {row.file}")

    print()
    print("PARTIALLY REACHED, most uncovered branches first. These are the")
    print("decidable ones: the guard fired but never declined, or the reverse.")
    for row in sorted(partial, key=lambda r: -len(r.branches))[: args.limit]:
        if not row.branches:
            continue
        where = ", ".join(f"{line}:{column}" for line, column in sorted(row.branches)[:4])
        print(
            f"  {len(row.branches):4} branches  {row.ratio:4.2f}  {row.path:40} "
            f"{row.file}  at {where}"
        )

    if args.json:
        args.json.write_text(
            json.dumps(
                [
                    {
                        "file": row.file,
                        "function": row.path,
                        "function_region_total": row.total,
                        "function_regions_uncovered": row.uncovered,
                        "uncovered_branches": sorted(row.branches),
                        "verdict": None,
                    }
                    for row in sorted(rows.values(), key=lambda r: (-r.ratio, -r.total))
                ],
                indent=2,
            )
            + "\n"
        )
        print(f"\nrows written to {args.json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
