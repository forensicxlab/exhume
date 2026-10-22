#!/usr/bin/env python3
"""Build, package, verify, and publish the pinned Exhume workspace.

Requires Python 3.9+, Cargo, and the native tools installed by verify.yml.
All paths and subprocess arguments are passed without a shell.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import zipfile


ROOT = Path(__file__).resolve().parents[1]
GROUPS = ("core", "memory")


def run(args, cwd=ROOT, capture=False, **kwargs):
    result = subprocess.run(
        [str(arg) for arg in args], cwd=cwd, check=True, text=True,
        stdout=subprocess.PIPE if capture else None, **kwargs,
    )
    return result.stdout.strip() if capture else ""


def digest(path):
    checksum = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def targets():
    return json.loads((ROOT / ".github/release-targets.json").read_text())


def target_config(target):
    return next(item for item in targets() if item["target"] == target)


def metadata(all_dependencies=False):
    args = ["cargo", "metadata", "--locked", "--format-version", "1"]
    if not all_dependencies:
        args.append("--no-deps")
    return json.loads(run(args, capture=True))


def packages(data, group=None):
    members = set(data["workspace_members"])
    result = []
    for package in data["packages"]:
        if package["id"] not in members:
            continue
        current_group = "memory" if package["name"] == "exhume_memory" else "core"
        if group and group != current_group:
            continue
        for binary in package["targets"]:
            if "bin" in binary["kind"]:
                result.append({
                    "module": package["name"],
                    "version": package["version"],
                    "binary": binary["name"],
                    "path": str(Path(package["manifest_path"]).parent.relative_to(ROOT)),
                    "group": current_group,
                })
    return sorted(result, key=lambda item: (item["module"], item["binary"]))


def git_commit(path=ROOT):
    return run(["git", "rev-parse", "HEAD"], cwd=path, capture=True)


def check_sources():
    # Do not strip leading spaces: they distinguish matching gitlinks from drift.
    status = subprocess.check_output(
        ["git", "submodule", "status", "--recursive"], cwd=ROOT, text=True
    )
    if any(line and line[0] != " " for line in status.splitlines()):
        raise RuntimeError("Uninitialized, conflicted, or drifting submodule:\n" + status)
    run(["git", "diff", "--exit-code", "HEAD"])
    data = metadata(all_dependencies=True)
    names = {item["module"] for item in packages(data)}
    members = set(data["workspace_members"])
    wrong = [
        item["id"] for item in data["packages"]
        if item["name"] in names and item["id"] not in members
    ]
    if wrong:
        raise RuntimeError("Exhume dependencies escaped the workspace patches: " + str(wrong))
    for item in packages(data):
        run(["git", "diff", "--exit-code", "HEAD"], cwd=ROOT / item["path"])
    print("All submodule revisions and workspace dependency sources match.")


def binary_arch(path):
    with Path(path).open("rb") as stream:
        header = stream.read(64)
        if header.startswith(b"\x7fELF"):
            machine = struct.unpack_from("<H", header, 18)[0]
            return {62: "x64", 183: "arm64"}.get(machine)
        if header[:4] == b"\xcf\xfa\xed\xfe":
            cpu = struct.unpack_from("<I", header, 4)[0]
            return {0x01000007: "x64", 0x0100000C: "arm64"}.get(cpu)
        if header.startswith(b"MZ"):
            stream.seek(struct.unpack_from("<I", header, 60)[0])
            signature = stream.read(6)
            if signature[:4] == b"PE\0\0":
                return {0x8664: "x64", 0xAA64: "arm64"}.get(
                    struct.unpack_from("<H", signature, 4)[0]
                )
    return None


def copy_linux_license(library, destination):
    for candidate in (library, library.resolve()):
        found = subprocess.run(
            ["dpkg-query", "--search", str(candidate)], text=True,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        if found.returncode:
            continue
        for line in found.stdout.splitlines():
            package = line.split(": ", 1)[0].split(":")[0]
            copyright_file = Path("/usr/share/doc") / package / "copyright"
            if copyright_file.is_file():
                destination.mkdir(exist_ok=True)
                shutil.copy2(copyright_file, destination / (library.name + ".copyright"))
                return
    raise RuntimeError("Cannot locate the runtime library license for " + str(library))


def linux_runtime(binary, stage):
    output = run(["ldd", binary], capture=True)
    if "not found" in output:
        raise RuntimeError("Unresolved runtime dependency:\n" + output)
    # The host provides glibc and its loader. Bundle all other resolved libraries.
    glibc = re.compile(r"lib(c|m|pthread|dl|rt|resolv|util|anl)\.so\.")
    libraries = {}
    for line in output.splitlines():
        match = re.match(r"\s*(\S+)\s+=>\s+(/\S+)\s+\(", line)
        if match and not glibc.match(match[1]):
            libraries[match[1]] = Path(match[2])
    if libraries:
        libdir = stage / "lib"
        libdir.mkdir()
        for name, source in sorted(libraries.items()):
            destination = libdir / name
            shutil.copy2(source, destination)
            run(["patchelf", "--set-rpath", "$ORIGIN", destination])
            copy_linux_license(source, stage / "licenses")
        run(["patchelf", "--set-rpath", "$ORIGIN/lib", binary])
    verified = run(["ldd", binary], capture=True)
    if "not found" in verified:
        raise RuntimeError("Packaged runtime dependencies are unresolved:\n" + verified)
    return {
        "linked_libraries": verified.replace(str(stage), "<archive>").splitlines(),
        "bundled": sorted(libraries),
    }


def macos_dependencies(path):
    lines = run(["otool", "-L", path], capture=True).splitlines()[1:]
    return [line.strip().split(" (", 1)[0] for line in lines]


def macos_runtime(binary, stage):
    pending = [(binary, binary)]
    copied = {}
    while pending:
        destination, original = pending.pop()
        for dependency in macos_dependencies(destination):
            if dependency.startswith(("/usr/lib/", "/System/Library/")):
                continue
            if dependency.startswith("@loader_path/"):
                source = original.parent / dependency[len("@loader_path/"):]
            elif dependency.startswith("/"):
                source = Path(dependency)
            else:
                raise RuntimeError("Cannot resolve non-system macOS dependency " + dependency)
            source = source.resolve()
            if not source.is_file():
                raise RuntimeError("Missing macOS runtime library " + str(source))
            libdir = stage / "lib"
            libdir.mkdir(exist_ok=True)
            name = source.name
            if name in copied and copied[name] != source:
                raise RuntimeError("Conflicting macOS runtime library names: " + name)
            if name not in copied:
                copied[name] = source
                library = libdir / name
                shutil.copy2(source, library)
                run(["install_name_tool", "-id", "@loader_path/" + name, library])
                pending.append((library, source))
            prefix = "@loader_path/lib/" if destination == binary else "@loader_path/"
            run(["install_name_tool", "-change", dependency, prefix + name, destination])
    for library in sorted((stage / "lib").glob("*")):
        run(["codesign", "--force", "--sign", "-", library])
    run(["codesign", "--force", "--sign", "-", binary])
    return {"linked_libraries": macos_dependencies(binary), "bundled": sorted(copied)}


def windows_runtime(binary):
    output = run(["dumpbin", "/DEPENDENTS", binary], capture=True)
    libraries = sorted(set(re.findall(r"^\s+([A-Za-z0-9_.-]+\.dll)\s*$", output, re.M | re.I)))
    # Rust and native dependencies are built with the static MSVC runtime.
    unexpected = [
        name for name in libraries
        if re.match(r"(?i)(vcruntime|msvcp|libssl|libcrypto|libusb)", name)
    ]
    if unexpected:
        raise RuntimeError("Unbundled Windows runtime dependencies: " + ", ".join(unexpected))
    return {"linked_libraries": libraries, "bundled": []}


def runtime_notes(item, config):
    notes = [
        "# " + item["binary"],
        "",
        "Module version: " + item["version"],
        "Target: " + config["target"],
        "",
        "Extract the whole archive and keep any lib/ directory beside the executable.",
    ]
    if config["platform"] == "linux":
        notes += [
            "Requires glibc 2.35 or newer (Ubuntu 22.04 or a compatible distribution).",
            "The archive includes non-glibc shared libraries used by this executable.",
            "TLS connections use the operating system's CA certificates.",
        ]
    elif config["platform"] == "macos":
        notes += [
            "Built and smoke-tested on macOS 15.",
            "The executable has an ad-hoc signature.",
        ]
    else:
        notes += ["Built with the static MSVC runtime and tested on the matching native runner."]
    if item["group"] == "memory":
        notes += [
            "",
            "File-backed memory analysis uses the statically linked LeechCore core.",
            "Live DMA hardware additionally requires its matching vendor/LeechCore driver.",
            "Place optional drivers in leechcore-runtime/ beside the executable, or set",
            "LEECHCORE_RUNTIME_DIR to their directory. Driver binaries are not included.",
            "The KVM connector is available on Linux hosts only and needs its kernel module.",
        ]
    return "\n".join(notes) + "\n"


def safe_extract(archive, destination):
    """Extract only regular files/directories within the archive's own root."""
    def safe(name):
        path = PurePosixPath(name)
        if path.is_absolute() or ".." in path.parts or "\\" in name:
            raise RuntimeError("Unsafe archive path: " + name)
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as stream:
            for member in stream.infolist():
                safe(member.filename)
                if (member.external_attr >> 16) & 0o170000 == 0o120000:
                    raise RuntimeError("Archive contains a symlink")
            stream.extractall(destination)
    else:
        with tarfile.open(archive) as stream:
            for member in stream.getmembers():
                safe(member.name)
                if not (member.isfile() or member.isdir()):
                    raise RuntimeError("Archive contains a link or special file")
            # Every member was checked above; works with Python 3.9 through 3.14.
            stream.extractall(destination, **({"filter": "data"} if sys.version_info >= (3, 12) else {}))


def smoke_archive(archive, record, config):
    with tempfile.TemporaryDirectory(prefix="exhume smoke ") as directory:
        destination = Path(directory)
        safe_extract(archive, destination)
        stage = destination / record["root"]
        binary = stage / record["executable"]
        if binary_arch(binary) != config["arch"]:
            raise RuntimeError("Incorrect executable architecture in " + str(archive))
        manifest = json.loads((stage / "manifest.json").read_text())
        for name, expected in manifest["files"].items():
            if digest(stage / name) != expected:
                raise RuntimeError("Packaged file checksum mismatch: " + name)
        run([binary, "--help"], cwd=destination, capture=True, timeout=60,
            stdin=subprocess.DEVNULL, stderr=subprocess.PIPE)
        if record["module"] == "exhume_body":
            source = destination / "sample raw image"
            exported = destination / "exported raw image"
            source.write_bytes(bytes(range(256)) * 8)
            run([binary, "--body", source, "--format", "raw", "--export", exported],
                cwd=destination, capture=True, timeout=60, stdin=subprocess.DEVNULL)
            if exported.read_bytes() != source.read_bytes():
                raise RuntimeError("Packaged exhume_body RAW round-trip failed")
        print("Smoke passed:", archive.name)


def build(target, group):
    data = metadata()
    selected = sorted({item["module"] for item in packages(data, group)})
    args = ["cargo", "build", "--release", "--locked", "--target", target, "--bins"]
    for package in selected:
        args += ["--package", package]
    run(args)


def package(target, group, output):
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    data = metadata()
    config = target_config(target)
    compiler = run(["rustc", "--version", "--verbose"], capture=True)
    source = {"commit": git_commit(), "cargo_lock_sha256": digest(ROOT / "Cargo.lock")}
    records = []
    for item in packages(data, group):
        stem = item["module"] + "-" + target
        executable = item["binary"] + (".exe" if config["platform"] == "windows" else "")
        built = Path(data["target_directory"]) / target / "release" / executable
        if binary_arch(built) != config["arch"]:
            raise RuntimeError("Build output has the wrong architecture: " + str(built))
        with tempfile.TemporaryDirectory(prefix="exhume package ") as directory:
            stage = Path(directory) / stem
            stage.mkdir()
            binary = stage / executable
            shutil.copy2(built, binary)
            shutil.copy2(ROOT / item["path"] / "LICENSE", stage / "LICENSE")
            (stage / "RUNTIME.md").write_text(runtime_notes(item, config))
            if config["platform"] == "linux":
                runtime = linux_runtime(binary, stage)
            elif config["platform"] == "macos":
                runtime = macos_runtime(binary, stage)
            else:
                runtime = windows_runtime(binary)
            manifest = {
                "schema": 1, **item, "source": source,
                "module_commit": git_commit(ROOT / item["path"]),
                "target": target, "compiler": compiler, "runtime": runtime,
                "rustflags": os.environ.get("RUSTFLAGS", ""),
                "files": {
                    path.relative_to(stage).as_posix(): digest(path)
                    for path in sorted(stage.rglob("*")) if path.is_file()
                },
            }
            write_json(stage / "manifest.json", manifest)
            if config["platform"] == "windows":
                archive = output / (stem + ".zip")
                with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
                    for path in sorted(stage.rglob("*")):
                        if path.is_file():
                            stream.write(path, path.relative_to(stage.parent).as_posix())
            else:
                archive = output / (stem + ".tar.gz")
                with tarfile.open(archive, "w:gz") as stream:
                    stream.add(stage, arcname=stem)
            record = {
                **item, "module_commit": manifest["module_commit"], "target": target,
                "archive": archive.name, "root": stem, "executable": executable,
                "sha256": digest(archive), "size": archive.stat().st_size,
            }
            smoke_archive(archive, record, config)
            records.append(record)
    write_json(output / ("manifest-" + target + "-" + group + ".json"), {
        "schema": 1, "source": source, "compiler": compiler,
        "target": target, "group": group, "packages": records,
    })


def assemble(directory):
    directory = Path(directory)
    data = metadata()
    expected = {
        (item["module"], config["target"]): item
        for item in packages(data) for config in targets()
    }
    source = {"commit": git_commit(), "cargo_lock_sha256": digest(ROOT / "Cargo.lock")}
    seen = set()
    records = []
    for path in sorted(directory.glob("manifest-*.json")):
        manifest = json.loads(path.read_text())
        if manifest["source"] != source:
            raise RuntimeError("Mixed source snapshots in " + str(path))
        for record in manifest["packages"]:
            identity = (record["module"], record["target"])
            if identity not in expected or identity in seen:
                raise RuntimeError("Unexpected or duplicate package: " + str(identity))
            item = expected[identity]
            for field in ("module", "version", "binary", "path", "group"):
                if record[field] != item[field]:
                    raise RuntimeError("Package metadata does not match the source: " + field)
            if record["module_commit"] != git_commit(ROOT / item["path"]):
                raise RuntimeError("Package module commit does not match the source")
            if Path(record["archive"]).name != record["archive"]:
                raise RuntimeError("Invalid archive filename")
            seen.add(identity)
            archive = directory / record["archive"]
            if digest(archive) != record["sha256"] or archive.stat().st_size != record["size"]:
                raise RuntimeError("Archive checksum/size mismatch: " + str(archive))
            records.append(record)
    if seen != set(expected):
        raise RuntimeError("Missing release packages: " + str(sorted(set(expected) - seen)))
    records.sort(key=lambda item: (item["module"], item["target"]))
    write_json(directory / "manifest.json", {
        "schema": 1, "source": source, "targets": targets(), "packages": records,
    })
    checksum_paths = [directory / item["archive"] for item in records] + [directory / "manifest.json"]
    (directory / "SHA256SUMS").write_text("".join(
        digest(path) + "  " + path.name + "\n" for path in sorted(checksum_paths)
    ))
    notes = [
        "Compiled from umbrella commit " + source["commit"] + ".",
        "",
        "Each module is available for Linux, macOS, and Windows on x64 and ARM64.",
        "Download the archive for your platform and extract its entire contents.",
        "SHA256SUMS verifies every archive and manifest.json records all source revisions.",
        "",
        "| Module | Version | Source commit |",
        "| --- | --- | --- |",
    ]
    for item in packages(data):
        notes.append("| {module} | {version} | {commit} |".format(
            **item, commit=git_commit(ROOT / item["path"])
        ))
    notes += [
        "",
        "Linux requires glibc 2.35+. macOS archives are built/tested on macOS 15.",
        "Memory DMA hardware drivers and the Linux KVM kernel module are installed separately.",
        "See RUNTIME.md inside each archive for its runtime requirements.",
    ]
    (directory / "release-notes.md").write_text("\n".join(notes) + "\n")
    print("Verified complete release:", len(records), "archives")


def sync_modules():
    check_sources()
    paths = [
        item.split("\t", 1)[1]
        for item in run(["git", "ls-tree", "-r", "HEAD"], capture=True).splitlines()
        if item.startswith("160000 ")
    ]
    for relative in paths:
        path = ROOT / relative
        before = git_commit(path)
        run(["git", "fetch", "--no-tags", "origin", "main"], cwd=path)
        after = run(["git", "rev-parse", "FETCH_HEAD"], cwd=path, capture=True)
        run(["git", "merge-base", "--is-ancestor", before, after], cwd=path)
        run(["git", "checkout", "--detach", after], cwd=path)
        run(["git", "submodule", "update", "--init", "--recursive"], cwd=path)
    # Update workspace packages and only the dependencies needed by their new versions.
    run(["cargo", "update", "--workspace"])


def publish(directory, channel, tag=None):
    directory = Path(directory)
    assemble(directory)
    commit = git_commit()
    if channel == "stable":
        if not tag or not re.fullmatch(r"toolkit-[A-Za-z0-9][A-Za-z0-9._-]*", tag):
            raise RuntimeError("Stable releases require a toolkit-* tag")
    else:
        tag = "nightly-" + os.environ["GITHUB_RUN_ID"] + "-" + os.environ.get("GITHUB_RUN_ATTEMPT", "1") + "-" + commit[:12]
    # An existing published release is never replaced. Retries may resume a draft.
    found = subprocess.run(
        ["gh", "release", "view", tag, "--json", "isDraft,targetCommitish"],
        cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    )
    if found.returncode == 0:
        existing = json.loads(found.stdout)
        if not existing["isDraft"] or existing["targetCommitish"] != commit:
            raise RuntimeError("Refusing to overwrite an existing release: " + tag)
    else:
        repository = os.environ.get("GITHUB_REPOSITORY") or run(
            ["gh", "repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner"],
            capture=True,
        )
        reference = subprocess.run(
            ["gh", "api", "repos/" + repository + "/commits/" + tag, "--jq", ".sha"],
            cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        if reference.returncode == 0 and reference.stdout.strip() != commit:
            raise RuntimeError("Release tag points at a different source commit")
        args = ["gh", "release", "create", tag, "--target", commit, "--draft",
                "--title", ("Exhume Toolkit " if channel == "stable" else "Latest main build ") + tag,
                "--notes-file", directory / "release-notes.md"]
        if channel == "rolling":
            args += ["--prerelease", "--latest=false"]
        run(args)
    manifest = json.loads((directory / "manifest.json").read_text())
    assets = [directory / item["archive"] for item in manifest["packages"]]
    assets += [directory / "manifest.json", directory / "SHA256SUMS"]
    # Bound command length on Windows and keep uploads resumable while still private.
    for offset in range(0, len(assets), 10):
        run(["gh", "release", "upload", tag, "--clobber", *assets[offset:offset + 10]])
    uploaded = json.loads(run(
        ["gh", "release", "view", tag, "--json", "assets"], capture=True
    ))
    actual = {item["name"]: item["size"] for item in uploaded["assets"]}
    expected = {path.name: path.stat().st_size for path in assets}
    if actual != expected:
        raise RuntimeError("Draft release assets are incomplete; keeping the release private")
    run(["gh", "release", "edit", tag, "--draft=false",
         "--latest=" + ("true" if channel == "stable" else "false")])
    print("Published:", tag)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("check-sources")
    commands.add_parser("matrix")
    commands.add_parser("sync-modules")
    for name in ("build", "package"):
        command = commands.add_parser(name)
        command.add_argument("--target", required=True, choices=[item["target"] for item in targets()])
        command.add_argument("--group", required=True, choices=GROUPS)
        if name == "package":
            command.add_argument("--output", default="dist")
    for name in ("assemble", "publish"):
        command = commands.add_parser(name)
        command.add_argument("--directory", default="dist")
        if name == "publish":
            command.add_argument("--channel", required=True, choices=("stable", "rolling"))
            command.add_argument("--tag")
    args = parser.parse_args()
    if args.command == "matrix":
        print(json.dumps({"include": targets()}, separators=(",", ":")))
    elif args.command == "check-sources":
        check_sources()
    elif args.command == "sync-modules":
        sync_modules()
    elif args.command == "build":
        build(args.target, args.group)
    elif args.command == "package":
        package(args.target, args.group, args.output)
    elif args.command == "assemble":
        assemble(args.directory)
    elif args.command == "publish":
        publish(args.directory, args.channel, args.tag)


if __name__ == "__main__":
    main()
