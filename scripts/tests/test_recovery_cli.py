"""Decrypt a synthetic Signal database through the exact distributed binaries."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "members/exhume_artefacts/src/recovery/fixtures"
BIN_DIR = os.environ.get("EXHUME_TEST_BIN_DIR")


class RecoveryCase(unittest.TestCase):
    def setUp(self):
        if not BIN_DIR:
            self.skipTest("set EXHUME_TEST_BIN_DIR to test built executables")
        directory = tempfile.TemporaryDirectory(prefix="exhume cipher smoke ")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.profile = self.root / "source/home/alice/.config/Signal"
        (self.profile / "sql").mkdir(parents=True)
        self.database = self.profile / "sql/db.sqlite"
        shutil.copy2(FIXTURES / "signal-linux-encrypted.sqlite", self.database)
        self.before = hashlib.sha256(self.database.read_bytes()).digest()
        self.key = json.loads((FIXTURES / "signal-linux.json").read_text())["database_key"]
        (self.profile / "config.json").write_text(json.dumps({"key": self.key}))

    def tearDown(self):
        if hasattr(self, "database"):
            self.assertEqual(hashlib.sha256(self.database.read_bytes()).digest(), self.before)

    def cli(self, binary, *args):
        suffix = ".exe" if os.name == "nt" else ""
        executable = Path(BIN_DIR).resolve() / (binary + suffix)
        result = subprocess.run([str(executable), *map(str, args)], input=b"",
                                capture_output=True, cwd=self.root, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertNotIn(self.key.encode(), result.stdout + result.stderr)
        return result


class ArtefactRecoveryTests(RecoveryCase):
    def test_distributed_parser_opens_encrypted_signal(self):
        result = self.cli("exhume_artefacts", "--parser", "desktop_signal", "--file",
                          self.database, "--option", "database_key=" + self.key)
        records = [json.loads(line) for line in result.stdout.splitlines()]
        messages = [r for r in records if r.get("record_type") == "message"]
        self.assertEqual(len(messages), 17)


class IndexerRecoveryTests(RecoveryCase):
    def test_distributed_indexer_recovers_encrypted_signal(self):
        index = self.root / "index.sqlite"
        result = self.cli("exhume_indexer", "--body", self.root / "source", "--database", index,
                          "--extract-artefacts", "--json-report", "--no-progress", "--no-recovery-input")
        report = json.loads(result.stdout)
        self.assertEqual(len(report["recovery"]["jobs"]), 1)
        self.assertEqual(report["recovery"]["jobs"][0]["status"], "complete")
        with sqlite3.connect(index) as connection:
            count = connection.execute("SELECT COUNT(*) FROM artifact_objects WHERE parser='desktop_signal' AND kind='mobile.communication.message'").fetchone()[0]
        self.assertEqual(count, 17)


if __name__ == "__main__":
    unittest.main()
