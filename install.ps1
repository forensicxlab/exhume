$ErrorActionPreference = "Stop"

cargo install --path members/exhume_ntfs  --force
cargo install --path members/exhume_exfat --force
cargo install --path members/exhume_extfs --force
cargo install --path members/exhume_partitions  --force
cargo install --path members/exhume_body --force
cargo install --path members/exhume_lvm --force
cargo install --path members/exhume_artefacts --force
