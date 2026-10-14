#!/usr/bin/env sh
set -eu

cd -- "$(dirname -- "$0")"

# Install the disk-analysis suite. Memory and agent are still in development.
cargo install --path members/exhume_ntfs  --force
cargo install --path members/exhume_exfat --force
cargo install --path members/exhume_extfs --force
cargo install --path members/exhume_apfs --force
cargo install --path members/exhume_partitions  --force
cargo install --path members/exhume_body --force
cargo install --path members/exhume_lvm --force
cargo install --path members/exhume_filesystem --force
cargo install --path members/exhume_artefacts --force
cargo install --path members/exhume_indexer --force
cargo install --path members/exhume_grep --force
