$ErrorActionPreference = "Stop"

# Install the disk-analysis suite. Memory and agent are still in development.
$modules = @(
    "exhume_ntfs",
    "exhume_exfat",
    "exhume_extfs",
    "exhume_apfs",
    "exhume_partitions",
    "exhume_body",
    "exhume_lvm",
    "exhume_filesystem",
    "exhume_artefacts",
    "exhume_indexer",
    "exhume_grep"
)

Push-Location $PSScriptRoot
try {
    foreach ($module in $modules) {
        cargo install --path "members/$module" --force
        if ($LASTEXITCODE -ne 0) {
            throw "Installation of $module failed (exit code $LASTEXITCODE)."
        }
    }
}
finally {
    Pop-Location
}
