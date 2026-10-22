# The Exhume Toolkit

Exhume is a modular Rust toolkit for digital forensics, from reading evidence
sources to indexing, searching, and analysing their contents.

The Exhume Toolkit documentation is maintained at [forensicxlab.com](https://www.forensicxlab.com/docs/exhume).

## Download executables

[Latest stable release](https://github.com/forensicxlab/exhume/releases/latest) · [Latest main builds and release history](https://github.com/forensicxlab/exhume/releases)

Stable downloads are linked below. Main builds are published as separate immutable
nightly prereleases after all checks pass. Each release contains all 13 modules
for all six native targets, SHA256SUMS, and a source manifest. Module versions are
independent; an umbrella release identifies the complete tested source snapshot.

Extract the whole archive, including its lib/ directory when present. The agent
executable is named exhume-agent; the other executables use their module names.

| Module | Linux | macOS | Windows |
| --- | --- | --- | --- |
| exhume_agent | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-aarch64-pc-windows-msvc.zip) |
| exhume_apfs | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-aarch64-pc-windows-msvc.zip) |
| exhume_artefacts | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-aarch64-pc-windows-msvc.zip) |
| exhume_body | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-aarch64-pc-windows-msvc.zip) |
| exhume_exfat | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-aarch64-pc-windows-msvc.zip) |
| exhume_extfs | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-aarch64-pc-windows-msvc.zip) |
| exhume_filesystem | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-aarch64-pc-windows-msvc.zip) |
| exhume_grep | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-aarch64-pc-windows-msvc.zip) |
| exhume_indexer | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-aarch64-pc-windows-msvc.zip) |
| exhume_lvm | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-aarch64-pc-windows-msvc.zip) |
| exhume_memory | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-aarch64-pc-windows-msvc.zip) |
| exhume_ntfs | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-aarch64-pc-windows-msvc.zip) |
| exhume_partitions | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-x86_64-unknown-linux-gnu.tar.gz) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-x86_64-pc-windows-msvc.zip) · [arm64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-aarch64-pc-windows-msvc.zip) |

Linux archives require glibc 2.35 or newer and include their non-glibc shared
libraries. macOS archives are built and tested on macOS 15 with ad-hoc signatures.
Windows archives use the static MSVC runtime and are tested on native x64/ARM64
runners. Each archive includes RUNTIME.md with its specific requirements.

Memory file analysis includes the statically linked LeechCore core. Live DMA
hardware needs its matching drivers in leechcore-runtime/ beside the executable
or in LEECHCORE_RUNTIME_DIR. The Linux KVM connector additionally needs its kernel
module. Hardware drivers and the KVM kernel module are installed separately.

Verify an extracted release checksum list with sha256sum --check SHA256SUMS on
Linux or shasum -a 256 --check SHA256SUMS on macOS. Windows provides Get-FileHash
-Algorithm SHA256 for checking individual downloads against SHA256SUMS.

## Rust support

All Exhume crates require Rust 1.92 or newer. Each member declares this minimum
in its own `Cargo.toml` so it also applies when the repository is used standalone.
The umbrella workspace uses Cargo resolver 3 to prefer dependencies compatible
with the declared Rust version.

To validate the minimum supported Rust version from the workspace root:

```sh
rustup toolchain install 1.92.0 --profile minimal
cargo +1.92.0 check --workspace --all-targets --locked
cargo +1.92.0 test --workspace --locked
```

The umbrella CI runs these checks on Linux with both Rust 1.92.0 and stable.
Release executables use Rust 1.92.0, default crate features, and the committed
workspace lockfile. To build all executables from the pinned workspace:

~~~sh
git submodule update --init --recursive
cargo +1.92.0 build --release --workspace --bins --locked
~~~

## Release automation

- Every push and pull request verifies the workspace and builds/packages all six
  native targets. The memory build runs separately from the other modules.
- Successful main pushes publish a new nightly prerelease. Failed builds leave
  existing releases available.
- Pushing a toolkit-* tag publishes a stable snapshot after the same checks.
  A stable release can also be requested from main:

~~~sh
gh workflow run rust.yml --ref main -f channel=stable -f tag=toolkit-2026.10.22
~~~

The publisher stages all assets in a draft, verifies the complete snapshot, and
then publishes it. Existing published releases are never overwritten. Stable
snapshots appear at releases/latest; nightly prereleases retain separate tags.

The daily updater runs at 04:23 UTC. It fast-forwards module pointers to their
published main branches, refreshes the workspace lockfile, and runs the complete
verification/build/package suite before opening or updating one pull request.
Merge that pull request to refresh the main downloads. Incompatible snapshots
fail verification and do not produce a new release.

The updater needs the repository setting "Allow GitHub Actions to create and
approve pull requests" under Settings > Actions > General > Workflow permissions.
GitHub couples creation and review-approval permission in this setting. The
updater uses PR creation and commit statuses. Keep default workflow token
permissions read-only; individual jobs request the permissions they need.
The updater records its verification result on the exact candidate commit, so
its tests do not depend on a second workflow being triggered by GITHUB_TOKEN-created
pull requests. No publishing credentials or crates.io token are required.

The target matrix lives in .github/release-targets.json. Packaging and release
validation are shared by scripts/release.py. To test the release tooling locally:

~~~sh
python3 -m unittest discover -s scripts/tests -v
~~~

## Modules

| Module | Description |
| --- | --- |
| [exhume_body](https://github.com/forensicxlab/exhume_body) | Reads disk images and other evidence sources through a format-independent interface. |
| [exhume_partitions](https://github.com/forensicxlab/exhume_partitions) | Parses GPT, MBR, and extended partition tables to locate partitions within evidence. |
| [exhume_lvm](https://github.com/forensicxlab/exhume_lvm) | Parses LVM metadata and provides access to logical volumes. |
| [exhume_extfs](https://github.com/forensicxlab/exhume_extfs) | Reads files, directories, and metadata from Extended Filesystems. |
| [exhume_exfat](https://github.com/forensicxlab/exhume_exfat) | Reads files, directories, and metadata from exFAT filesystems. |
| [exhume_ntfs](https://github.com/forensicxlab/exhume_ntfs) | Reads files, directories, and metadata from NTFS filesystems. |
| [exhume_apfs](https://github.com/forensicxlab/exhume_apfs) | Parses APFS containers and volumes to access files and filesystem metadata. |
| [exhume_filesystem](https://github.com/forensicxlab/exhume_filesystem) | Provides a common API for browsing filesystems and exporting files, directories, and volumes. |
| [exhume_artefacts](https://github.com/forensicxlab/exhume_artefacts) | Parses operating-system and application artefacts into structured forensic records. |
| [exhume_indexer](https://github.com/forensicxlab/exhume_indexer) | Indexes filesystem metadata in SQLite and coordinates artefact extraction. |
| [exhume_grep](https://github.com/forensicxlab/exhume_grep) | Searches indexed file contents with literal or regex patterns and optional highlighted matching lines. |
| [exhume_memory](https://github.com/forensicxlab/exhume_memory) | Provides volatile memory acquisition and analysis through memflow connectors. |
| [exhume_agent](https://github.com/forensicxlab/exhume_agent) | Provides an embeddable forensic assistant with Exhume tools and a terminal interface. |
