# Data and Provenance Guide

[中文](README.md) · **English** · [Project home](../README.en.md)

This directory contains statistics and provenance from the 2026-10-05 experiment. It excludes images,
PDBs, full symbol contents, process records, and license files. Local paths are replaced with identifiers
such as `${IMAGE_A}`, `${PDB_A}`, and `${CASE_A}`. These are sanitization placeholders, not runnable paths.

| File | Contents | Interpretation |
| --- | --- | --- |
| [starmem.json](starmem.json) | Three sample identities, artifact hashes, aggregates, and engine build ID | `manifest` includes single preparation timings, not repeated-sample medians |
| [starmem-commands.json](starmem-commands.json) | Arguments, exit codes, stderr, and command wall time | Wall time includes startup and differs from internal loading time |
| `image-A/`, `image-B/`, `image-C/` | load-samples, session-samples, report-status | Full reports are not distributed; hashes and status are retained |
| [vol3.json](vol3.json) | Full ISF conversion, loads, sessions, and set-difference counts | Different semantic scope; failure or zero records does not establish equivalent completion |
| [vol3-commands.json](vol3-commands.json) | Commands and diagnostics | Failures retained; local paths and original filenames replaced |
| [environment.json](environment.json) | Date, hardware, and OS | One machine, not a cross-platform result |
| [source-manifest.json](source-manifest.json) | Reference file SHA-256 hashes and parent commit | The parent commit excludes pre-existing working-tree changes |
| [rejection-checks.json](rejection-checks.json) | Native-session rejection of invalid SSYM | `passed=true` means expected rejection, not successful forensic analysis |

## Units and status

- `load_us` is microseconds; report tables convert to milliseconds. `lookup_pair_ns` is nanoseconds per field/symbol query pair.
- `open_ms`, `pslist_ms`, and `total_ms` are internal timings. `process_wall_ms` or command wall time includes process overhead.
- `peak_heap_bytes` / `retained_heap_bytes` are Rust requested heap; `python_traced_*` are tracemalloc observations. Neither is RSS.
- `median/min/max/samples` summarize all measured samples. `heaps` and `warmups` are excluded from latency aggregates.
- `complete=false` means the engine marked the report incomplete. `report-status.json` publishes warning kinds without addresses or full messages.
- `session_reports_equal=true` only means identical full serialized reports across symbol paths for the same image.
- Vol3 `equal=null` with `status=failed` means no comparable output was obtained, not a zero-row result.

## Raw data and bilingual documentation

JSON keys, original diagnostics, and numeric values retain their recorded meaning. A translated second
measurement dataset is not created. The Chinese [benchmark report](../BENCHMARKS.md) and
[English report](../BENCHMARKS.en.md) explain the same data. Original source package metadata is
preserved by hash; the research distribution uses [Apache-2.0](../LICENSE-APACHE).

Run `python -B -X utf8 scripts/verify-study.py` from the study root to check samples, aggregates,
hashes, documentation pairing, and data paths. It does not read original images or establish evidence
completeness, and it does not reproduce the engine execution.
