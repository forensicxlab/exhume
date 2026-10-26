# Exhume Toolkit toolkit-2026.10.26

Release candidate for the coordinated `exhume_body` 0.7.0 transition.

## Changes

- Propagate AccessData ADCRYPT v1 encrypted-EWF credentials through the image
  tools, filesystem export, indexer, grep, agent, and filesystem artefact example.
  CLIs accept password files or stdin; interactive prompts hide typed passwords.
- Use exactly one `exhume_body` version throughout the workspace. The previous
  0.6.0 pins bypassed the local 0.7.0 workspace patch.
- Include the existing filesystem traversal and indexing work: typed coverage
  reports, recoverable traversal issues, and cancellation-aware consumers.
- Add `exhume_memory --version` so its distributed executable reports the
  manifest version consistently with the other modules.
- Exercise a synthetic two-segment ADCRYPT image in CI and in extracted body and
  partition release packages on every native target. Checks cover exact RAW
  export, opening from E02, partition parity, password input, credential errors,
  and unchanged source bytes.

This release targets ADCRYPT v1 password encryption. It does not add certificate
unlocking or imply support for every encrypted EWF acquisition-tool variant.

## Module versions

| Module | Previous umbrella version | Candidate version |
| --- | --- | --- |
| exhume_body | 0.6.0 | 0.7.0 (already published) |
| exhume_apfs | 0.2.0 | 0.3.0 |
| exhume_exfat | 0.2.0 | 0.3.0 |
| exhume_extfs | 0.2.0 | 0.3.0 |
| exhume_lvm | 0.2.0 | 0.3.0 |
| exhume_ntfs | 0.2.0 | 0.3.0 |
| exhume_partitions | 0.4.0 | 0.5.0 |
| exhume_filesystem | 0.6.0 | 0.7.0 |
| exhume_artefacts | 0.4.0 | 0.5.0 |
| exhume_indexer | 0.3.0 | 0.4.0 |
| exhume_grep | 0.2.0 | 0.3.0 |
| exhume_agent | 0.2.0 | 0.3.0 |
| exhume_memory | 0.1.4 | 0.1.5 |

The shared public Rust types require coordinated minor-version upgrades for the
dependent pre-1.0 crates. Memory is independent and receives a patch release.
Rust 1.92 remains the minimum supported version.

## Downstream compatibility

`Filesystem::walk_fs` now takes `WalkOptions` and a fallible callback, returns a
`WalkReport`, and emits `WalkEvent::Issue` for coverage gaps. The umbrella's
consumers have been migrated together.

Thanatology is a separate consumer and still pins the preceding crate versions,
including a local `exhume_agent = "=0.2.0"` path dependency. It must update those
pins together before consuming this candidate. Its existing uncommitted traversal
integration is not a substitute for updating its dependency graph.

## Validation and release gates

Local validation on macOS ARM64:

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --all-targets --locked`: passed on Rust 1.99.1 and
  Rust 1.92.0.
- `cargo test --workspace --locked`: 473 passed and 8 intentionally ignored on
  each compiler. Ignored tests require external images or run benchmarks.
- `cargo build --workspace --bins --locked`: passed for all 13 executables.
- Release tooling and executable integration tests: 26 passed with
  `EXHUME_TEST_BIN_DIR=target/debug python3 -m unittest discover -s scripts/tests -v`.
- Actionlint 1.7.12: passed for all workflows.
- The resolved dependency graph contains exactly the 13 local Exhume modules,
  including one `exhume_body` at 0.7.0, with no registry copies of older modules.
- Publication dry runs built and verified the packaged APFS, exFAT, extfs, LVM,
  NTFS, and partition crates against the published `exhume_body` 0.7.0.

Outstanding checks:

- `cargo clippy --workspace --all-targets --locked -- -D warnings` fails on
  existing diagnostics in the published body source and memory module, including
  chunk iteration, test `get(0)` calls, a constant-bound comparison, and a large
  connector enum. The already published body 0.7.0 source is kept unchanged.
  Newer Rust also deprecates grep's `fetch_update`; Rust 1.92 still uses that API.
- An all-crate publication dry run cannot resolve the unpublished candidate
  dependency versions (first failure: `exhume_artefacts = "=0.5.0"` for the agent).
  Publish in dependency order and repeat dry runs as each dependency becomes
  available. No upload was performed.
- Six-platform CI, optimized distribution builds, and the extracted native
  packages have not been run for this local candidate. Linux, macOS, and Windows,
  each on x64 and ARM64, remain the six native targets. The release workflow
  requires all 78 module archives plus `manifest.json` and `SHA256SUMS`.

## Publication sequence

The candidate is prepared locally; no crates, remote tags, or releases are
published by this preparation.

1. Push the candidate module commits to their repositories, then push the umbrella
   candidate branch so CI can resolve every pinned submodule commit.
2. Complete the six-target umbrella verification and extracted-package tests.
3. Publish the base crates in this group: `exhume_apfs`, `exhume_exfat`,
   `exhume_extfs`, `exhume_lvm`, `exhume_ntfs`, `exhume_partitions`.
4. Once those versions are available on crates.io, publish `exhume_filesystem`,
   then `exhume_artefacts`, then `exhume_indexer`, then `exhume_grep` and
   `exhume_agent`. Rerun each publication dry run before uploading. `exhume_body`
   0.7.0 is already published and `exhume_memory` is intentionally `publish = false`.
5. Merge the validated umbrella candidate and publish the stable
   `toolkit-2026.10.26` snapshot through the existing release workflow.

Pushing to umbrella `main` automatically publishes a rolling release after CI.
Pushing a `toolkit-*` tag, or dispatching the workflow with `channel=stable` and
the matching tag, publishes a stable release after every required job succeeds.
Use a candidate branch for review and validation before taking those actions.
