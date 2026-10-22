# The Exhume Toolkit

Exhume is a modular toolkit for digital forensics: reading disk images, inspecting
partitions, exploring filesystems, extracting artefacts, searching evidence, and
analysing memory.

The downloads below are the **command-line interface (CLI) versions** of each
module, intended to be run from a terminal. Each module's repository also contains
its source code and library interface.

[Documentation](https://www.forensicxlab.com/docs/exhume) · [Latest stable release](https://github.com/forensicxlab/exhume/releases/latest) · [All releases](https://github.com/forensicxlab/exhume/releases)

## Download CLI executables

Choose your operating system and processor architecture. Module names link to
their repositories.

| Module | Purpose | Linux | macOS | Windows |
| --- | --- | --- | --- | --- |
| [exhume_body](https://github.com/forensicxlab/exhume_body) | Read disk images and evidence sources. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_body-aarch64-pc-windows-msvc.zip) |
| [exhume_partitions](https://github.com/forensicxlab/exhume_partitions) | Inspect GPT, MBR, and extended partition tables. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_partitions-aarch64-pc-windows-msvc.zip) |
| [exhume_lvm](https://github.com/forensicxlab/exhume_lvm) | Inspect LVM metadata and access logical volumes. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_lvm-aarch64-pc-windows-msvc.zip) |
| [exhume_filesystem](https://github.com/forensicxlab/exhume_filesystem) | Browse filesystems and export files, directories, and volumes. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_filesystem-aarch64-pc-windows-msvc.zip) |
| [exhume_apfs](https://github.com/forensicxlab/exhume_apfs) | Read APFS containers, volumes, and filesystem metadata. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_apfs-aarch64-pc-windows-msvc.zip) |
| [exhume_exfat](https://github.com/forensicxlab/exhume_exfat) | Read exFAT files, directories, and metadata. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_exfat-aarch64-pc-windows-msvc.zip) |
| [exhume_extfs](https://github.com/forensicxlab/exhume_extfs) | Read Extended Filesystem files, directories, and metadata. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_extfs-aarch64-pc-windows-msvc.zip) |
| [exhume_ntfs](https://github.com/forensicxlab/exhume_ntfs) | Read NTFS files, directories, and metadata. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_ntfs-aarch64-pc-windows-msvc.zip) |
| [exhume_artefacts](https://github.com/forensicxlab/exhume_artefacts) | Extract structured records from operating-system and application artefacts. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_artefacts-aarch64-pc-windows-msvc.zip) |
| [exhume_indexer](https://github.com/forensicxlab/exhume_indexer) | Index filesystem metadata in SQLite and coordinate artefact extraction. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_indexer-aarch64-pc-windows-msvc.zip) |
| [exhume_grep](https://github.com/forensicxlab/exhume_grep) | Search indexed file contents using literal or regular-expression patterns. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_grep-aarch64-pc-windows-msvc.zip) |
| [exhume_memory](https://github.com/forensicxlab/exhume_memory) | Acquire and analyse volatile memory through memflow connectors. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_memory-aarch64-pc-windows-msvc.zip) |
| [exhume_agent](https://github.com/forensicxlab/exhume_agent) | Use a forensic assistant with Exhume tools from the terminal. | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-x86_64-unknown-linux-gnu.tar.gz) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-aarch64-unknown-linux-gnu.tar.gz) | [Intel](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-x86_64-apple-darwin.tar.gz) · [Apple Silicon](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-aarch64-apple-darwin.tar.gz) | [x64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-x86_64-pc-windows-msvc.zip) · [ARM64](https://github.com/forensicxlab/exhume/releases/latest/download/exhume_agent-aarch64-pc-windows-msvc.zip) |

## Getting started

Download and extract the complete archive for your platform. Keep any bundled
directories, including `lib/`, beside the executable.

From the extracted directory, display the available commands. For example, with
`exhume_body`:

**Linux and macOS**

```sh
./exhume_body --help
```

**Windows PowerShell**

```powershell
.\exhume_body.exe --help
```

Other executables use their module names, except `exhume_agent`, whose executable
is named `exhume-agent`.

See the bundled `RUNTIME.md` for platform requirements and additional setup.
Memory acquisition connectors can require separate hardware drivers or kernel
modules.

Each release includes `SHA256SUMS` for verifying downloaded archives. Full usage
guides and examples are available in the
[Exhume documentation](https://www.forensicxlab.com/docs/exhume).
