# The Exhume Toolkit

Exhume is a modular Rust toolkit for digital forensics, from reading evidence
sources to indexing, searching, and analysing their contents.

The Exhume Toolkit documentation is maintained at [forensicxlab.com](https://www.forensicxlab.com/docs/exhume).

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
