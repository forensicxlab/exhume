# The Exhume Toolkit

Exhume is a modular Rust toolkit for digital forensics, from reading evidence
sources to indexing, searching, and analysing their contents.

The Exhume Toolkit documentation is maintained at [forensicxlab.com](https://www.forensicxlab.com/docs/exhume).

## Modules

| Module | Description |
| --- | --- |
| [exhume_body](members/exhume_body) | Reads disk images and other evidence sources through a format-independent interface. |
| [exhume_partitions](members/exhume_partitions) | Parses GPT, MBR, and extended partition tables to locate partitions within evidence. |
| [exhume_lvm](members/exhume_lvm) | Parses LVM metadata and provides access to logical volumes. |
| [exhume_extfs](members/exhume_extfs) | Reads files, directories, and metadata from Extended Filesystems. |
| [exhume_exfat](members/exhume_exfat) | Reads files, directories, and metadata from exFAT filesystems. |
| [exhume_ntfs](members/exhume_ntfs) | Reads files, directories, and metadata from NTFS filesystems. |
| [exhume_apfs](members/exhume_apfs) | Parses APFS containers and volumes to access files and filesystem metadata. |
| [exhume_filesystem](members/exhume_filesystem) | Provides a common API for browsing filesystems and exporting files, directories, and volumes. |
| [exhume_artefacts](members/exhume_artefacts) | Parses operating-system and application artefacts into structured forensic records. |
| [exhume_indexer](members/exhume_indexer) | Indexes filesystem metadata in SQLite and coordinates artefact extraction. |
| [exhume_grep](members/exhume_grep) | Searches indexed file contents with literal or regex patterns and optional highlighted matching lines. |
| [exhume_memory](members/exhume_memory) | Provides volatile memory acquisition and analysis through memflow connectors. |
| [exhume_agent](members/exhume_agent) | Provides an embeddable forensic assistant with Exhume tools and a terminal interface. |
