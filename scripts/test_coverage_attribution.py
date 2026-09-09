"""Tests for `coverage_attribution.py`.

Every case here is a mistake the tool actually made against real llvm-cov
output, kept so it cannot make them again. Each one produced a confident wrong
number, and none of them would have been caught by looking at the output: the
tool's self-check against llvm-cov's own per-file summary is what found them,
and these tests are that check applied to a fixture small enough to read.
"""

import unittest
from pathlib import Path

import coverage_attribution as attribution


def export(functions, files):
    """One llvm-cov export, in the shape the real one has."""
    return {"data": [{"functions": functions, "files": files}]}


def function(name, filenames, regions):
    return {"name": name, "filenames": filenames, "regions": regions}


def region(line_start, col_start, line_end, col_end, count, file_id=0):
    """A region in llvm-cov's own field order.

    `[line_start, col_start, line_end, col_end, execution_count, file_id,
    expanded_file_id, kind]`. The tool read index 5 as the KIND for a while; it
    is the file id, and the kind is index 7.
    """
    return [line_start, col_start, line_end, col_end, count, file_id, 0, 0]


def file_summary(name, covered, count):
    return {
        "filename": f"/x/chatter/{name}",
        "summary": {"regions": {"covered": covered, "count": count}},
    }


REPO = Path(__file__).resolve().parent.parent


class MergesInstantiations(unittest.TestCase):
    """A generic function appears once per instantiation, with different hashes.

    Keying rows on a demangled name split `check_id_header` across buckets and
    reported a function llvm-cov says is 160/161 covered as entirely unreached.
    """

    def test_a_region_covered_by_any_instantiation_is_covered(self):
        regions = [region(10, 1, 10, 20, 0)]
        covered = [region(10, 1, 10, 20, 7)]
        rows, disagreements = attribution.build_rows(
            export(
                [
                    function("_RNvA", ["/x/chatter/crates/c/src/a.rs"], regions),
                    function("_RNvB", ["/x/chatter/crates/c/src/a.rs"], covered),
                ],
                [file_summary("crates/c/src/a.rs", 1, 1)],
            ),
            REPO,
            "crates/c",
        )
        self.assertEqual(disagreements, [])
        self.assertEqual(rows, {}, "one instantiation ran it, so it ran")


class DistinguishesRegionsSharingAStart(unittest.TestCase):
    """`if c { a } else { b }` puts two regions at one start position.

    Merging on the start alone under-counted every file in the validator by one
    to five regions, which the self-check caught and nothing else would have.
    """

    def test_two_regions_at_one_start_are_two_regions(self):
        rows, disagreements = attribution.build_rows(
            export(
                [
                    function(
                        "_RNvA",
                        ["/x/chatter/crates/c/src/a.rs"],
                        [region(5, 9, 5, 20, 3), region(5, 9, 7, 4, 0)],
                    )
                ],
                [file_summary("crates/c/src/a.rs", 1, 2)],
            ),
            REPO,
            "crates/c",
        )
        self.assertEqual(disagreements, [])
        self.assertEqual(len(rows), 1)
        self.assertEqual(next(iter(rows.values())).total, 2)


class AttributesARegionToItsOwnFile(unittest.TestCase):
    """A macro expansion puts a function's regions in another file.

    Attributing every region to `filenames[0]` moved coverage between files and
    reported 474/506 where llvm-cov said 451/506.
    """

    def test_a_region_in_another_file_is_not_counted_here(self):
        rows, disagreements = attribution.build_rows(
            export(
                [
                    function(
                        "_RNvA",
                        [
                            "/x/chatter/crates/c/src/a.rs",
                            "/x/chatter/crates/other/src/b.rs",
                        ],
                        [region(5, 1, 5, 9, 1), region(9, 1, 9, 9, 0, file_id=1)],
                    )
                ],
                [file_summary("crates/c/src/a.rs", 1, 1)],
            ),
            REPO,
            "crates/c",
        )
        self.assertEqual(disagreements, [], "the second region belongs to another file")
        self.assertEqual(rows, {})


class ReportsItsOwnDisagreement(unittest.TestCase):
    """The self-check is the thing that found every bug above.

    It must fire, or the tool is a confident wrong number generator.
    """

    def test_a_count_llvm_cov_contradicts_is_reported(self):
        _rows, disagreements = attribution.build_rows(
            export(
                [
                    function(
                        "_RNvA",
                        ["/x/chatter/crates/c/src/a.rs"],
                        [region(5, 1, 5, 9, 1)],
                    )
                ],
                # llvm-cov claims two regions; the functions carry one.
                [file_summary("crates/c/src/a.rs", 1, 2)],
            ),
            REPO,
            "crates/c",
        )
        self.assertEqual(len(disagreements), 1)
        self.assertIn("this tool says 1/1", disagreements[0])
        self.assertIn("llvm-cov says 1/2", disagreements[0])


class ReadsAFunctionPathOutOfAMangledName(unittest.TestCase):
    """Reading segments to the end returns the INSTANTIATION TYPES.

    The first draft reported `ErrorCollector` and `NullErrorSink` as the names
    of the two largest unreached functions in the validator, sending the
    worklist at the sink types rather than at the functions.
    """

    def test_the_generic_arguments_are_not_the_name(self):
        mangled = (
            "_RINvNtNtNtCs44iRkvQ0EKp_14talkbank_model10validation6header"
            "8checkers15check_id_headerINtNtB6_5state13RecordingSinkE"
        )
        self.assertIn("check_id_header", attribution.readable_path(mangled))
        self.assertNotIn("RecordingSink", attribution.readable_path(mangled))


if __name__ == "__main__":
    unittest.main()
