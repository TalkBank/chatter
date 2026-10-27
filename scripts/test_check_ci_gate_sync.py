"""Pure regression contracts for transitive gate recipe ownership."""

import unittest
from unittest.mock import patch

import check_ci_gate_sync


class GateRecipesTests(unittest.TestCase):
    def recipes(self, source: str) -> set[str]:
        with patch("pathlib.Path.read_text", return_value=source):
            return check_ci_gate_sync.gate_recipes()

    def test_follows_body_calls_and_dependencies(self):
        recipes = self.recipes(
            "gate:\n    just suite\n\n"
            "suite: workspace\n    just docs\n\n"
            "workspace:\n    cargo test --workspace\n\n"
            "docs: links\n    cargo test --doc\n\n"
            "links:\n    check-links\n"
        )
        self.assertEqual(recipes, {"suite", "workspace", "docs", "links"})

    def test_cycles_terminate_without_admitting_unreachable_recipes(self):
        recipes = self.recipes(
            "gate:\n    just suite\n\n"
            "suite:\n    just gate\n\n"
            "unreachable:\n    just ignored\n"
        )
        self.assertEqual(recipes, {"suite", "gate"})

    def test_missing_gate_is_refused(self):
        with self.assertRaises(SystemExit):
            self.recipes("suite:\n    cargo test\n")



class ListedPackagesTests(unittest.TestCase):
    def test_reads_package_lines_and_skips_comments_and_blanks(self):
        listed = check_ci_gate_sync.listed_packages(
            "# owner of the list\nlibwebkit2gtk-4.1-dev\n\npkg-config\n"
        )
        self.assertEqual(listed, ["libwebkit2gtk-4.1-dev", "pkg-config"])

    def test_a_commented_out_package_is_not_listed(self):
        listed = check_ci_gate_sync.listed_packages("# libwebkit2gtk-4.1-dev\npkg-config\n")
        self.assertEqual(listed, ["pkg-config"])

if __name__ == "__main__":
    unittest.main()
