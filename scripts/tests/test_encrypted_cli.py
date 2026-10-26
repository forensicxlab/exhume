"""Integration checks for freshly built or extracted Exhume executables.

EXHUME_TEST_BIN_DIR=target/debug python3 -m unittest discover -s scripts/tests -v
"""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).resolve().parent / "fixtures"
PASSWORD = b" fixture password "
BIN_DIR = os.environ.get("EXHUME_TEST_BIN_DIR")


class CliCase(unittest.TestCase):
    def setUp(self):
        if not BIN_DIR:
            self.skipTest("set EXHUME_TEST_BIN_DIR to test built executables")
        self.binary_dir = Path(BIN_DIR).resolve()
        self.directory = tempfile.TemporaryDirectory(prefix="exhume encrypted test ")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.hashes = {}
        for name in ("disk.raw", "encrypted.E01", "encrypted.E02"):
            target = self.root / name
            shutil.copy2(FIXTURES / name, target)
            self.hashes[name] = hashlib.sha256(target.read_bytes()).digest()

    def tearDown(self):
        for name, before in getattr(self, "hashes", {}).items():
            self.assertEqual(hashlib.sha256((self.root / name).read_bytes()).digest(), before)

    def cli(self, binary, *arguments, stdin=b"", success=True):
        executable = self.binary_dir / (binary + (".exe" if os.name == "nt" else ""))
        result = subprocess.run(
            [str(executable), *map(str, arguments)], input=stdin,
            capture_output=True, cwd=self.root, timeout=30,
        )
        if success:
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        else:
            self.assertNotEqual(result.returncode, 0)
        self.assertNotIn(PASSWORD, result.stdout + result.stderr)
        return result


class EncryptedBodyTests(CliCase):
    def test_stdin_password_exports_both_segments_exactly(self):
        output = self.root / "export.raw"
        self.cli("exhume_body", "--body", "encrypted.E01", "--password-stdin",
                 "--export", output, stdin=PASSWORD + b"\n")
        self.assertEqual(output.read_bytes(), (self.root / "disk.raw").read_bytes())

    def test_password_file_preserves_spaces_and_opens_from_second_segment(self):
        password_file = self.root / "password.txt"
        password_file.write_bytes(PASSWORD + b"\r\n")
        output = self.root / "export.raw"
        self.cli("exhume_body", "--body", "encrypted.E02", "--password-file",
                 password_file, "--export", output)
        self.assertEqual(output.read_bytes(), (self.root / "disk.raw").read_bytes())

    def test_missing_password_fails_without_creating_output(self):
        result = self.cli("exhume_body", "--body", "encrypted.E01", "--export",
                          "export.raw", success=False)
        self.assertIn(b"requires a password", result.stderr)
        self.assertFalse((self.root / "export.raw").exists())

    def test_wrong_password_fails_without_creating_output(self):
        result = self.cli("exhume_body", "--body", "encrypted.E01", "--password-stdin",
                          "--export", "export.raw", stdin=b"wrong password\n", success=False)
        self.assertIn(b"verification failed", result.stderr)
        self.assertFalse((self.root / "export.raw").exists())


class EncryptedPartitionTests(CliCase):
    def test_encrypted_and_raw_images_have_the_same_partition_table(self):
        plain = self.cli("exhume_partitions", "--body", "disk.raw", "--json")
        encrypted = self.cli("exhume_partitions", "--body", "encrypted.E01", "--json",
                             "--password-stdin", stdin=PASSWORD + b"\n")
        expected = json.loads(plain.stdout)
        self.assertIsNotNone(expected["mbr"])
        self.assertEqual(json.loads(encrypted.stdout), expected)

    def test_wrong_password_reaches_the_shared_credential_validator(self):
        result = self.cli("exhume_partitions", "--body", "encrypted.E01", "--json",
                          "--password-stdin", stdin=b"wrong password\n", success=False)
        self.assertIn(b"verification failed", result.stderr)
        self.assertEqual(result.stdout, b"")


class ExecutableTests(CliCase):
    def test_every_module_starts_and_reports_its_manifest_version(self):
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
            cwd=ROOT, text=True,
        ))
        for package in metadata["packages"]:
            for target in package["targets"]:
                if "bin" not in target["kind"]:
                    continue
                with self.subTest(module=package["name"]):
                    self.cli(target["name"], "--help")
                    version = self.cli(target["name"], "--version")
                    self.assertIn(package["version"].encode(), version.stdout)


if __name__ == "__main__":
    unittest.main()
