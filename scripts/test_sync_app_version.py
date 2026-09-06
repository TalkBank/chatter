"""Exercise the release version command against real manifest/lockfile pairs."""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("sync-app-version.py")


class VersionCommandTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "scripts").mkdir()
        shutil.copyfile(SCRIPT, self.root / "scripts/sync-app-version.py")
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.20.0"\n')
        (self.root / "CHANGELOG.md").write_text('## [0.20.0]\n[0.20.0]: https://example.test/release\n')
        desktop = self.root / "apps/chatter-desktop"
        desktop.mkdir(parents=True)
        (desktop / "package.json").write_text(json.dumps({"version": "0.20.0"}, indent=2) + "\n")
        self.lock = desktop / "package-lock.json"
        self.data = {"version": "0.20.0", "lockfileVersion": 3, "packages": {
            "": {"version": "0.20.0"}, "node_modules/example": {"version": "1.2.3", "integrity": "unchanged"}
        }}

    def run_command(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run([sys.executable, str(self.root / "scripts/sync-app-version.py"), *args], capture_output=True, text=True, check=False)

    def test_each_lockfile_version_is_checked_and_fixed_without_changing_dependencies(self) -> None:
        for location in (self.data, self.data["packages"][""]):
            with self.subTest(location=location):
                location["version"] = "0.16.0"
                self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
                result = self.run_command("--check")
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn("package-lock.json", result.stdout)
                result = self.run_command("--fix")
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                actual = json.loads(self.lock.read_text())
                self.assertEqual(actual["version"], "0.20.0")
                self.assertEqual(actual["packages"][""]["version"], "0.20.0")
                self.assertEqual(actual["packages"]["node_modules/example"], self.data["packages"]["node_modules/example"])
                self.assertEqual(self.run_command("--check").returncode, 0)
                location["version"] = "0.20.0"

    def test_bump_updates_both_lockfile_versions(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        result = self.run_command("--bump", "0.21.0")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        actual = json.loads(self.lock.read_text())
        self.assertEqual(actual["version"], "0.21.0")
        self.assertEqual(actual["packages"][""]["version"], "0.21.0")
        self.assertEqual(actual["packages"]["node_modules/example"], self.data["packages"]["node_modules/example"])


if __name__ == "__main__":
    unittest.main()
