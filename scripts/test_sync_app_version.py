"""Exercise the release version command against real manifest/lockfile pairs."""
from __future__ import annotations

import json
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from enum import Enum
from pathlib import Path

SCRIPT = Path(__file__).with_name("sync-app-version.py")


class WriteLimit(Enum):
    FIRST_FILE = 32
    LAST_FILE = 512


class VersionCommandTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "scripts").mkdir()
        shutil.copyfile(SCRIPT, self.root / "scripts/sync-app-version.py")
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.20.0"\n[workspace.dependencies]\nexample = { path = "crates/example", version = "0.20.0" }\n')
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

    def assert_limited_write_preserves_inputs(self, limit: WriteLimit) -> None:
        self.data["padding"] = "x" * 2048
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        original = self.snapshot()
        wrapper = "\n".join((
            "import resource, runpy, sys",
            "limit = int(sys.argv[1])",
            "resource.setrlimit(resource.RLIMIT_FSIZE, (limit, limit))",
            "sys.argv = sys.argv[2:]",
            "runpy.run_path(sys.argv[0], run_name='__main__')",
        ))
        result = subprocess.run(
            [sys.executable, "-B", "-c", wrapper, str(limit.value),
             str(self.root / "scripts/sync-app-version.py"), "--bump", "0.21.0"],
            capture_output=True, text=True, check=False,
        )
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.snapshot(), original, result.stdout + result.stderr)
        self.assertIn("cannot stage", result.stderr)
        self.assertEqual(list(self.root.rglob(".version-sync-*")), [])

    @unittest.skipIf(sys.platform == "win32", "RLIMIT_FSIZE requires Unix")
    def test_first_write_size_failure_preserves_all_inputs(self) -> None:
        self.assert_limited_write_preserves_inputs(WriteLimit.FIRST_FILE)

    @unittest.skipIf(sys.platform == "win32", "RLIMIT_FSIZE requires Unix")
    def test_last_write_size_failure_preserves_all_inputs(self) -> None:
        self.assert_limited_write_preserves_inputs(WriteLimit.LAST_FILE)

    @unittest.skipIf(sys.platform == "win32", "Unix permission bits")
    def test_replacement_preserves_modes_and_leaves_other_hardlink_unchanged(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        cargo = self.root / "Cargo.toml"
        cargo.chmod(0o640)
        sibling = self.root / "original-cargo"
        sibling.hardlink_to(cargo)
        original = sibling.read_bytes()
        result = self.run_command("--bump", "0.21.0")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(stat.S_IMODE(cargo.stat().st_mode), 0o640)
        self.assertEqual(sibling.read_bytes(), original)
        self.assertIn('version = "0.21.0"', cargo.read_text())
        self.assertEqual(json.loads(self.lock.read_text())["version"], "0.21.0")
        self.assertEqual(list(self.root.rglob(".version-sync-*")), [])

    @unittest.skipIf(sys.platform == "win32", "symlink creation requires Windows privileges")
    def test_symlink_last_destination_refuses_before_any_publication(self) -> None:
        target = self.root / "external-lock.json"
        target.write_text(json.dumps(self.data, indent=2) + "\n")
        self.lock.symlink_to(target)
        original = self.snapshot()
        result = self.run_command("--bump", "0.21.0")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("expected a regular file", result.stderr)
        self.assertEqual(self.snapshot(), original)
        self.assertTrue(self.lock.is_symlink())
        self.assertEqual(list(self.root.rglob(".version-sync-*")), [])

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

    def snapshot(self) -> dict[str, bytes]:
        return {str(path.relative_to(self.root)): path.read_bytes() for path in (
            self.root / "Cargo.toml", self.root / "CHANGELOG.md",
            self.lock.with_name("package.json"), self.lock,
        )}

    def test_invalid_bump_identity_refuses_before_writing(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        original = self.snapshot()
        for version in ("01.2.3", "1.02.3", "1.2.03", "1.2.3\n", "١.٢.٣", "", "1.2.3-rc.1"):
            with self.subTest(version=version):
                for name, data in original.items():
                    (self.root / name).write_bytes(data)
                result = self.run_command("--bump", version)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(self.snapshot(), original)

    def test_conflicting_tag_refuses_before_bump_or_fix_writes(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        self.lock.with_name("package.json").write_text('{"version": "0.1.0"}\n')
        original = self.snapshot()
        for mode in (("--bump", "0.21.0"), ("--fix",)):
            with self.subTest(mode=mode):
                for name, data in original.items():
                    (self.root / name).write_bytes(data)
                result = self.run_command(*mode, "--release-tag", "v0.22.0")
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(self.snapshot(), original)

    def test_coordinated_invalid_canonical_identity_cannot_pass_check(self) -> None:
        version = "01.2.3"
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{version}"\n')
        (self.root / "CHANGELOG.md").write_text(f'## [{version}]\n[{version}]: https://example.test/release\n')
        self.data["version"] = self.data["packages"][""]["version"] = version
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        self.lock.with_name("package.json").write_text(json.dumps({"version": version}) + "\n")
        original = self.snapshot()
        result = self.run_command("--check", "--release-tag", f"v{version}")
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.snapshot(), original)

    def test_normal_versions_and_both_existing_tag_spellings_remain_supported(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        for tag in ("v0.20.0", "0.20.0"):
            with self.subTest(tag=tag):
                original = self.snapshot()
                result = self.run_command("--check", "--release-tag", tag)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(self.snapshot(), original)
        for version in ("0.0.0", "10.20.30"):
            with self.subTest(version=version):
                result = self.run_command("--bump", version, "--release-tag", f"v{version}")
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(json.loads(self.lock.read_text())["version"], version)

    def test_bump_updates_both_lockfile_versions(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        result = self.run_command("--bump", "0.21.0", "--release-tag", "v0.21.0")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        import tomllib
        cargo = tomllib.loads((self.root / "Cargo.toml").read_text())
        self.assertEqual(cargo["workspace"]["package"]["version"], "0.21.0")
        self.assertEqual(cargo["workspace"]["dependencies"]["example"]["version"], "0.21.0")
        self.assertEqual(json.loads(self.lock.with_name("package.json").read_text())["version"], "0.21.0")
        actual = json.loads(self.lock.read_text())
        self.assertEqual(actual["version"], "0.21.0")
        self.assertEqual(actual["packages"][""]["version"], "0.21.0")
        self.assertEqual(actual["packages"]["node_modules/example"], self.data["packages"]["node_modules/example"])

    def test_bump_refuses_missing_or_malformed_canonical_manifest_without_writes(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        cases = (
            '[workspace.package]\nedition = "2024"\n[package]\nversion = "0.20.0"\n',
            '[workspace.package]\nversion = "0.20.0"\nversion = "0.20.0"\n',
            '[workspace.package]\nversion = "0.20.0"\nbroken = [\n',
            '[workspace.package]\nversion = "0.20.0"\n[workspace.dependencies]\nexample = { path = "crates/example" }\n',
            '[workspace.package]\nversion = "0.20.0"\n[workspace.dependencies]\nexample = { path = 12, version = "0.20.0" }\n',
            '[workspace.package]\nnote = \'\'\'\nversion = "0.20.0"\n\'\'\'\nversion = "0.20.0"\n',
            '[workspace]\npackage.version = "0.20.0"\nnote = \'\'\'\n[workspace.package]\nversion = "0.20.0"\n\'\'\'\n',
        )
        for source in cases:
            with self.subTest(source=source):
                (self.root / "Cargo.toml").write_text(source)
                original = self.snapshot()
                result = self.run_command("--bump", "0.21.0")
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(self.snapshot(), original)

    def test_bump_changes_only_owned_fields_and_accepts_toml_spacing_and_order(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        source = '''[workspace.package]
  version = '0.20.0' # canonical
[workspace.dependencies]
example = { version='0.20.0', features = ["feature"], path="crates/example" }
# example = { path = "crates/comment", version = "9.8.7" }
external = { path = "vendor/external", version = "9.8.7" }
[package.metadata]
example = { path = "crates/example", version = "9.8.7" }
'''
        conventional = source.replace("  version = '0.20.0'", 'version = "0.20.0"').replace(
            'version=\'0.20.0\', features = ["feature"], path="crates/example"',
            'path = "crates/example", version = "0.20.0", features = ["feature"]',
        )
        for manifest in (conventional, source, source.replace("\n", "\r\n")):
            with self.subTest(manifest=manifest):
                (self.root / "Cargo.toml").write_bytes(manifest.encode())
                result = self.run_command("--bump", "0.21.0")
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(
                    (self.root / "Cargo.toml").read_bytes(),
                    manifest.replace("0.20.0", "0.21.0").encode(),
                )

    def test_internal_pin_drift_is_checked_and_fixed(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        cargo = self.root / "Cargo.toml"
        original = cargo.read_text()
        cargo.write_text(original.replace('version = "0.20.0" }', 'version = "0.19.0" }'))
        before = self.snapshot()
        result = self.run_command("--check")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("workspace.dependencies.example.version", result.stdout)
        self.assertEqual(self.snapshot(), before)
        result = self.run_command("--fix")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(cargo.read_text(), original)

    def test_invalid_npm_inputs_refuse_all_writes(self) -> None:
        cases = ('{', '[]', '{}', '{"version": 12}',
                 '{"version": "invalid"}',
                 '{"version": "0.20.0", "version": "0.20.0"}',
                 '{"version": "0.20.0", "extra": NaN}',
                 '{"version": "0.20.0", "packages": {}}',
                 '{"version": "0.20.0", "packages": {"": {"version": null}}}')
        for mode in (("--bump", "0.21.0"), ("--fix",)):
            for source in cases:
                with self.subTest(mode=mode, source=source):
                    self.lock.with_name("package.json").write_text('{"version": "0.19.0"}\n')
                    self.lock.write_text(source)
                    original = self.snapshot()
                    result = self.run_command(*mode)
                    self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertEqual(self.snapshot(), original)

    def test_invalid_package_and_missing_changelog_refuse_before_writing(self) -> None:
        self.lock.write_text(json.dumps(self.data, indent=2) + "\n")
        package = self.lock.with_name("package.json")
        package.write_text('{"version": null}\n')
        original = self.snapshot()
        result = self.run_command("--bump", "0.21.0")
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.snapshot(), original)
        package.write_text('{"version": "0.20.0"}\n')
        original = self.snapshot()
        del original["CHANGELOG.md"]
        (self.root / "CHANGELOG.md").unlink()
        result = self.run_command("--bump", "0.21.0")
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual({name: (self.root / name).read_bytes() for name in original}, original)


if __name__ == "__main__":
    unittest.main()
