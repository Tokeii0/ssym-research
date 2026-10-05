# Reference Implementation Snapshot

[中文](README.md) · **English** · [Project home](../README.en.md)

**StarMem source is not public at present.** These excerpts explain this SSYM validation only;
they do not constitute an engine source release or a public SDK.

This directory contains experiment-related source excerpts, dependency snapshots, and an integration
patch. It is not a complete buildable StarMem crate. `Cargo.toml` / `Cargo.lock` record measured
dependencies; do not run Cargo directly in this directory.

Files:

- `src/profile.rs`: measured PDB → `KernelProfile` extractor, including `pub mod compiled`.
- `src/profile/compiled.rs`: SSYM encoding, validation, lookup, and profile materialization.
- `src/profile/compiled/tests.rs`: boundary and equivalence tests.
- `examples/symbol_format_bench.rs`: benchmark through actual StarMem APIs, retaining license checks.
- `integration.patch`: opt-in CLI/session integration, excluding unrelated working-tree changes.

To integrate into a complete StarMem checkout, first compare extractor changes, then add the compiled
module and example, review, and apply the patch. Do not replace an actively developed engine directory.
The patch uses the experimental repository's context; other revisions may require manual adaptation.

`data/source-manifest.json` in the study root records file hashes. Tested executable build IDs are
in the benchmark data. The shared checkout already had uncommitted changes, so the parent Git commit
alone is insufficient to reconstruct the engine. Exact reproduction requires the corresponding full
source snapshot and existing valid licensing environment.
The study root's `.gitattributes` disables line-ending conversion for reference files to preserve
their hashes. The extraction recipe hashes raw `profile.rs` bytes; reformatting or changing line
endings requires SSYM regeneration.

This study distributes the included StarMem code under its Apache-2.0 license option; see
[LICENSE-APACHE](../LICENSE-APACHE). `Cargo.toml` retains the measured source package's original
license expression as provenance, without changing this distribution's Apache-2.0 choice.
Volatility 3 source is not copied here; its comparison invokes official interfaces in a local installation.
