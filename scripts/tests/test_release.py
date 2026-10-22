import importlib.util
import io
import json
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile


spec = importlib.util.spec_from_file_location("release", Path(__file__).parents[1] / "release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ArchiveTests(unittest.TestCase):
    def test_rejects_path_traversal_and_absolute_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("../escaped", "/absolute", "root/../../escaped", r"root\..\escaped", "C:/escaped"):
                with self.subTest(name=name):
                    archive = root / "unsafe.tar.gz"
                    with tarfile.open(archive, "w:gz") as stream:
                        member = tarfile.TarInfo(name)
                        member.size = 4
                        stream.addfile(member, io.BytesIO(b"data"))
                    with self.assertRaisesRegex(RuntimeError, "Unsafe archive path"):
                        release.safe_extract(archive, root / "extract")
            self.assertFalse((root / "escaped").exists())

    def test_rejects_tar_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "link.tar.gz"
            with tarfile.open(archive, "w:gz") as stream:
                member = tarfile.TarInfo("root/link")
                member.type = tarfile.SYMTYPE
                member.linkname = "/outside"
                stream.addfile(member)
            with self.assertRaisesRegex(RuntimeError, "link or special"):
                release.safe_extract(archive, root / "extract")

    def test_rejects_zip_traversal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "unsafe.zip"
            with zipfile.ZipFile(archive, "w") as stream:
                stream.writestr("../escaped", "data")
            with self.assertRaisesRegex(RuntimeError, "Unsafe archive path"):
                release.safe_extract(archive, root / "extract")

    def test_accepts_regular_files_in_both_archive_formats(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            original = root / "sample"
            original.mkdir()
            (original / "binary").write_bytes(b"sample")
            for suffix in (".tar.gz", ".zip"):
                archive = root / ("sample" + suffix)
                if suffix == ".zip":
                    with zipfile.ZipFile(archive, "w") as stream:
                        stream.write(original / "binary", "sample/binary")
                else:
                    with tarfile.open(archive, "w:gz") as stream:
                        stream.add(original, arcname="sample")
                destination = root / ("extract" + suffix)
                release.safe_extract(archive, destination)
                self.assertEqual((destination / "sample/binary").read_bytes(), b"sample")

    def test_detects_all_six_executable_architectures(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "binary"
            for arch, elf, macho, pe in (
                ("x64", 62, 0x01000007, 0x8664),
                ("arm64", 183, 0x0100000C, 0xAA64),
            ):
                with self.subTest(arch=arch):
                    header = bytearray(64)
                    header[:4] = b"\x7fELF"
                    struct.pack_into("<H", header, 18, elf)
                    binary.write_bytes(header)
                    self.assertEqual(release.binary_arch(binary), arch)
                    header[:4] = b"\xcf\xfa\xed\xfe"
                    struct.pack_into("<I", header, 4, macho)
                    binary.write_bytes(header)
                    self.assertEqual(release.binary_arch(binary), arch)
                    header[:2] = b"MZ"
                    struct.pack_into("<I", header, 60, 64)
                    binary.write_bytes(header + b"PE\0\0" + struct.pack("<H", pe))
                    self.assertEqual(release.binary_arch(binary), arch)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.lock").write_text("locked dependencies")
        self.dist = self.root / "dist"
        self.dist.mkdir()
        self.target = {"target": "x86_64-unknown-linux-gnu", "arch": "x64", "platform": "linux"}
        self.item = {
            "module": "exhume_agent", "version": "0.2.0", "binary": "exhume-agent",
            "path": "members/exhume_agent", "group": "core",
        }
        for name, value in (
            ("ROOT", self.root), ("metadata", lambda: {}),
            ("packages", lambda data: [self.item]), ("targets", lambda: [self.target]),
            ("git_commit", lambda *args: "a" * 40),
        ):
            active = patch.object(release, name, value)
            active.start()
            self.addCleanup(active.stop)
        self.source = {
            "commit": "a" * 40, "cargo_lock_sha256": release.digest(self.root / "Cargo.lock")
        }

    def write_manifest(self, source=None, corrupt=False, duplicate=False):
        archive = self.dist / "exhume_agent-test.tar.gz"
        archive.write_bytes(b"validated archive contents")
        record = {
            **self.item, "target": self.target["target"], "archive": archive.name,
            "root": "exhume_agent-test", "executable": "exhume-agent",
            "module_commit": "a" * 40, "sha256": release.digest(archive),
            "size": archive.stat().st_size,
        }
        release.write_json(self.dist / "manifest-test-core.json", {
            "source": source or self.source,
            "packages": [record, record] if duplicate else [record],
        })
        if corrupt:
            archive.write_bytes(b"damaged")

    def test_assembly_requires_every_module_and_target(self):
        with self.assertRaisesRegex(RuntimeError, "Missing release packages"):
            release.assemble(self.dist)

    def test_assembly_rejects_mixed_source_snapshots(self):
        self.write_manifest(source={**self.source, "commit": "b" * 40})
        with self.assertRaisesRegex(RuntimeError, "Mixed source"):
            release.assemble(self.dist)

    def test_assembly_rejects_corrupt_archives(self):
        self.write_manifest(corrupt=True)
        with self.assertRaisesRegex(RuntimeError, "checksum/size mismatch"):
            release.assemble(self.dist)

    def test_assembly_rejects_duplicate_packages(self):
        self.write_manifest(duplicate=True)
        with self.assertRaisesRegex(RuntimeError, "duplicate package"):
            release.assemble(self.dist)

    def test_assembly_rejects_a_version_from_a_different_snapshot(self):
        self.write_manifest()
        path = self.dist / "manifest-test-core.json"
        manifest = json.loads(path.read_text())
        manifest["packages"][0]["version"] = "9.9.9"
        release.write_json(path, manifest)
        with self.assertRaisesRegex(RuntimeError, "metadata does not match"):
            release.assemble(self.dist)

    def test_complete_snapshot_has_checksums_and_source_manifest(self):
        self.write_manifest()
        release.assemble(self.dist)
        result = json.loads((self.dist / "manifest.json").read_text())
        self.assertEqual(result["source"], self.source)
        self.assertEqual(result["packages"][0]["executable"], "exhume-agent")
        checksums = (self.dist / "SHA256SUMS").read_text()
        self.assertIn(release.digest(self.dist / "manifest.json") + "  manifest.json", checksums)
        self.assertEqual(len(checksums.splitlines()), 2)

    def test_incomplete_release_never_calls_github(self):
        with patch.object(release.subprocess, "run") as github:
            with self.assertRaisesRegex(RuntimeError, "Missing release packages"):
                release.publish(self.dist, "stable", "toolkit-test")
            github.assert_not_called()

    def test_published_release_cannot_be_overwritten(self):
        self.write_manifest()
        existing = subprocess.CompletedProcess([], 0, json.dumps({
            "isDraft": False, "targetCommitish": "a" * 40,
        }))
        with patch.object(release.subprocess, "run", return_value=existing), \
                patch.object(release, "run") as external:
            with self.assertRaisesRegex(RuntimeError, "Refusing to overwrite"):
                release.publish(self.dist, "stable", "toolkit-test")
            external.assert_not_called()

    def test_stable_requires_a_toolkit_snapshot_tag(self):
        self.write_manifest()
        with patch.object(release.subprocess, "run") as github:
            with self.assertRaisesRegex(RuntimeError, "toolkit-"):
                release.publish(self.dist, "stable", "main")
            github.assert_not_called()

    def test_upload_failure_keeps_the_release_private(self):
        self.write_manifest()
        missing = subprocess.CompletedProcess([], 1, "")
        def github(args, **kwargs):
            if args[:3] == ["gh", "release", "upload"]:
                raise subprocess.CalledProcessError(1, args)
            return ""
        with patch.dict(release.os.environ, {"GITHUB_REPOSITORY": "owner/repo"}), \
                patch.object(release.subprocess, "run", return_value=missing), \
                patch.object(release, "run", side_effect=github) as external:
            with self.assertRaises(subprocess.CalledProcessError):
                release.publish(self.dist, "stable", "toolkit-test")
            self.assertFalse(any(call.args[0][:3] == ["gh", "release", "edit"]
                                 for call in external.call_args_list))

    def test_missing_uploaded_asset_keeps_the_release_private(self):
        self.write_manifest()
        missing = subprocess.CompletedProcess([], 1, "")
        def github(args, **kwargs):
            return json.dumps({"assets": []}) if args[:3] == ["gh", "release", "view"] else ""
        with patch.dict(release.os.environ, {"GITHUB_REPOSITORY": "owner/repo"}), \
                patch.object(release.subprocess, "run", return_value=missing), \
                patch.object(release, "run", side_effect=github) as external:
            with self.assertRaisesRegex(RuntimeError, "keeping the release private"):
                release.publish(self.dist, "stable", "toolkit-test")
            self.assertFalse(any(call.args[0][:3] == ["gh", "release", "edit"]
                                 for call in external.call_args_list))

    def test_existing_tag_must_point_at_the_compiled_commit(self):
        self.write_manifest()
        responses = [
            subprocess.CompletedProcess([], 1, ""),
            subprocess.CompletedProcess([], 0, "b" * 40),
        ]
        with patch.dict(release.os.environ, {"GITHUB_REPOSITORY": "owner/repo"}), \
                patch.object(release.subprocess, "run", side_effect=responses), \
                patch.object(release, "run") as external:
            with self.assertRaisesRegex(RuntimeError, "different source commit"):
                release.publish(self.dist, "stable", "toolkit-test")
            external.assert_not_called()

    def test_publish_promotes_only_a_complete_uploaded_snapshot(self):
        self.write_manifest()
        missing = subprocess.CompletedProcess([], 1, "")
        def github(args, **kwargs):
            if args[:3] == ["gh", "release", "view"]:
                files = [self.dist / "exhume_agent-test.tar.gz",
                         self.dist / "manifest.json", self.dist / "SHA256SUMS"]
                return json.dumps({"assets": [
                    {"name": path.name, "size": path.stat().st_size} for path in files
                ]})
            return ""
        with patch.dict(release.os.environ, {"GITHUB_REPOSITORY": "owner/repo"}), \
                patch.object(release.subprocess, "run", return_value=missing), \
                patch.object(release, "run", side_effect=github) as external:
            release.publish(self.dist, "stable", "toolkit-test")
            calls = [call.args[0] for call in external.call_args_list]
            self.assertIn("--draft", calls[0])
            self.assertEqual(calls[-1], [
                "gh", "release", "edit", "toolkit-test", "--draft=false", "--latest=true"
            ])


if __name__ == "__main__":
    unittest.main()
