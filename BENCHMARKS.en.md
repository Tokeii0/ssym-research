# Benchmark Methods and Results

[中文](BENCHMARKS.md) · **English** · [Project home](README.en.md)

**The measurements support compiled caches, but not a general replacement of ISF with SSYM v1.**

- Relative to repeated PDB extraction, median materialized SSYM loading was **7.2–15.6 times faster**; indexed loading was **11.8–23.3 times faster**.
- Relative to same-content JSON reader, indexed SSYM loading was **1.7–2.1 times faster**, but hot name queries were **2.4–3.1 times slower**.
- Existing plugins still consume `KernelProfile`. After materialization, SSYM was slower than JSON reader on B. Indexed-view gains are not gains already delivered to all plugins.
- PDB / JSON / SSYM full report hashes matched for each image. Only B's StarMem process-chain report was marked complete. A/C retain warnings; format equivalence does not establish complete evidence.

## Separating the questions

The study tests two different benefits:

1. Whether precompilation reduces repeated PDB extraction in new sessions.
2. Loading, allocation, and query costs of indexed binary versus JSON for the same StarMem model.

The Vol3 comparison separately records full ISF and process enumeration. It loads more symbol information,
uses Python, and has different address-space, bootstrap, and plugin implementations. Cross-engine time
differences cannot all be attributed to the SSYM format.

## Environment

- Date: 2026-10-05.
- CPU: Intel Core i5-13600KF, 20 logical processors.
- OS: Windows 11 x64, 10.0.26200, approximately 32 GiB RAM.
- StarMem: jointly developed 0.2.0 snapshot; Rust 1.93.1; release, opt-level=3, thin LTO, 1 codegen unit.
- Volatility 3: local version 2.27.0, Python 3.12.10.
- StarMem uses 2 analysis worker threads; Vol3 explicitly runs serially. This is not an engine contest with equal parallelism.

Source identity, symbol identity, and measurement samples are in `data/`. The parent Git commit excludes
pre-existing working-tree changes; use the tested binary build ID and included source hashes instead.

## Measurement boundaries

StarMem uses fresh processes and a fixed-seed randomized order, with warmups before measured samples.
Vol3 uses fresh processes but always tests JSON before XZ; its cache boundaries are described below.
OS file caches are warm. No cache purge or reboot was performed, and no cold-disk result is claimed.
Allocation instrumentation runs separately from latency measurements.
Each StarMem loading backend has 9 samples; each session method has 5. Tables report medians with
minimum–maximum in brackets. CPU affinity and frequency were not fixed. This exploratory single-machine
study supplies no population confidence interval and retains all high-latency outliers.

| Name | What is measured |
| --- | --- |
| PDB | `build_eprocess_profile`: extract the current runtime model from a local PDB |
| JSON reader | The same compact JSON through `BufReader + serde_json::from_reader` |
| JSON slice | Read the same JSON into a buffer, then `from_slice` |
| JSON + Zstd | Decompress the same JSON encoded with Zstd level 3, then `from_slice` |
| SSYM indexed | Read and validate the entire file, then provide name-indexed queries |
| SSYM materialized | Read and validate the entire file, then reconstruct the existing `KernelProfile` |
| StarMem session | Actual `AnalysisSession::open` + `processes`; SSYM additionally checks the target kernel PE identity |
| Vol3 ISF | `IntermediateSymbolTable(validate=True)`, including the first `_EPROCESS` field and global symbol lookup |
| Vol3 pslist | Native CLI bootstrap, symbol initialization, plugin, and JSON rendering; import/startup overhead also appears in process wall time |

StarMem internal timing excludes licensing, process startup, and report serialization. Vol3 CLI pipeline
timing includes different stages. Command wall times are stored separately.
**Dividing cross-engine numbers does not yield a format speedup.** Rust heap counters track requested
bytes and allocation calls; Python tracemalloc tracks Python memory. Neither is RSS.

Every equal-content backend must produce the same reserialized profile SHA-256. StarMem sessions must
match the hash of the complete serialized kernel context, process report, stop reason, and warnings,
not just row counts. Cross-engine comparison uses sets of PID plus normalized x64 EPROCESS virtual address.

## Samples and identity

Raw images remain in the local evidence directory and are identified publicly as A/B/C. They are not
a distributed public dataset. Full hashes for images, PDBs, profiles, binaries, and the tested engine
are in the [StarMem aggregate](data/starmem.json).

| Sample | Image bytes | PDB bytes | Types / type fields / root fields / symbols | Image and DBI Age / Info Age |
| --- | ---: | ---: | --- | --- |
| A (RAW) | 2,147,483,648 | 3,206,144 | 88 / 1,970 / 145 / 58 | 1 / 4 |
| B (VMEM) | 4,294,967,296 | 7,711,744 | 93 / 2,717 / 226 / 58 | 1 / 2 |
| C (DMP) | 8,588,783,616 | 11,366,400 | 92 / 2,967 / 263 / 56 | 1 / 6 |

| Sample | Image SHA-256 |
| --- | --- |
| A | `037b8a3073a92661d73d8e8e5e02077fddf246fd394b231e1e76ceda8b9ef85b` |
| B | `d27e9ba1612dc083e9323a026711481d60ebc313ab46b33dba7fdd34e3921a12` |
| C | `a83b4e74d44ce351aac81ab3fa44657309561a8f0c61fac9c9ee9d5c677e7272` |

## Equal-content symbol loading

Units: ms. Both SSYM paths include file reading, full SHA-256, structural and string-index checks,
and recipe checking. JSON deserializes into the same Rust model without additional JSON Schema validation.

| Method | A | B | C |
| --- | ---: | ---: | ---: |
| Direct PDB | 4.548 [4.367–7.376] | 13.800 [11.286–19.530] | 13.691 [13.188–18.215] |
| JSON reader | 0.785 [0.765–2.300] | 1.233 [1.021–3.836] | 1.171 [1.137–4.660] |
| JSON slice | 0.749 [0.712–1.248] | 1.739 [0.944–4.617] | 1.027 [0.951–2.842] |
| JSON + Zstd | 0.967 [0.868–1.037] | 1.626 [1.151–5.410] | 1.290 [1.242–2.143] |
| SSYM indexed | **0.386** [0.375–1.348] | **0.593** [0.427–2.352] | **0.691** [0.491–1.561] |
| SSYM materialized | 0.628 [0.588–1.131] | 1.384 [0.723–3.927] | 0.877 [0.801–1.490] |

The implementation suggests a possible explanation: avoiding repeated PDB traversal and expansion of
strings and tree nodes. Materialized SSYM reconstructs those structures, potentially reducing its
advantage; this study does not isolate each cost through ablation. Three samples on one machine with
millisecond-level timings do not establish that SSYM consistently outperforms JSON.

Initial generation also costs time. Single prepare-stage PDB extractions took 4.977 / 21.988 / 13.010 ms
for A/B/C; subsequent DBI Age reading, SSYM encoding, and validation took 1.522 / 3.080 / 2.820 ms.
These are single observations, not medians, and exclude some hashing, download, and output costs.

### File size

Units: bytes. JSON and SSYM below represent the same profile; full ISF is reported separately.

| Encoding | A | B | C |
| --- | ---: | ---: | ---: |
| Compact JSON | 104,159 | 149,953 | 164,980 |
| Indented JSON | 203,558 | 290,094 | 317,384 |
| JSON + Zstd level 3 | **21,532** | **29,899** | **32,752** |
| SSYM v1 | 96,784 | 134,387 | 148,047 |

SSYM is only **7.1–10.4%** smaller than compact JSON, not orders of magnitude smaller.
Compressed JSON is smaller for distribution. This SSYM experiment targets loading allocations and indexed access.

### Memory and hot lookup

Memory is requested heap from one separate instrumented process, in KiB (1,024 bytes). It excludes
image mappings, full-process RSS, and allocator-reserved space. Each query sample averages 10,000 fixed
field/symbol query pairs, including hits and misses; the table takes the median across 9 processes.
This is not an arbitrary plugin workload. All other backend data is in the aggregate.

| Sample / method | Peak KiB | Retained after load KiB | Allocation calls | Query pair ns |
| --- | ---: | ---: | ---: | ---: |
| A / PDB | 1,499.3 | 253.9 | 5,696 | 75.7 |
| A / JSON reader | 287.1 | 279.0 | 2,666 | 63.3 |
| A / SSYM indexed | 131.7 | 131.7 | **21** | 157.8 |
| A / SSYM materialized | 346.1 | 210.2 | 2,670 | 49.7 |
| B / PDB | 2,421.5 | 347.4 | 12,543 | 76.3 |
| B / JSON reader | 385.6 | 377.5 | 3,635 | 73.1 |
| B / SSYM indexed | 259.7 | 259.7 | **22** | 175.6 |
| B / SSYM materialized | 545.4 | 277.1 | 3,593 | 88.2 |
| C / PDB | 2,926.0 | 369.2 | 13,863 | 88.2 |
| C / JSON reader | 417.4 | 409.4 | 3,961 | 76.8 |
| C / SSYM indexed | 256.1 | 256.0 | **19** | 236.1 |
| C / SSYM materialized | 569.2 | 298.8 | 3,902 | 65.7 |

Indexed SSYM reduces thousands of allocation calls to roughly 20. Its current binary search repeatedly
processes UTF-8 strings, however, and hot lookups are slower than existing BTreeMap lookups.
The current `Vec` reader grows its buffer, so retained capacity can exceed file size substantially.
One buffer does not mean zero allocations or mmap. Materialization temporarily retains both the file
and the reconstructed model, so its peak can exceed JSON reader.

## Actual StarMem image sessions

Units: ms. Session opening plus actual pslist, 5 samples per method; process startup, licensing,
and final report serialization are excluded. SSYM uses the plugin-compatible materialized path and
additionally reads the actual kernel PE to check PDB name, GUID, and DBI Age.

| Sample | PDB | JSON | SSYM | Process count / complete flag for all three |
| --- | ---: | ---: | ---: | --- |
| A | 836.310 [787.137–882.706] | 835.714 [796.396–863.751] | 802.512 [794.061–827.399] | 63 / `false` |
| B | 1080.283 [1061.062–1099.360] | 1083.968 [1047.344–1092.435] | 1057.428 [1051.912–1077.452] | 455 / `true` |
| C | 23.980 [21.642–25.187] | **10.090** [9.565–10.813] | 11.033 [9.238–13.793] | 160 / `false` |

- A/B are dominated by session opening and their ranges overlap. These data do not establish a stable few-percent overall improvement from SSYM.
- Precompiled profiles substantially shorten C's new session, but **JSON has the lower total median**. Avoiding repeated PDB extraction and choosing SSYM are separate decisions.
- A/C stop at `list_head` with `head_blink_mismatch`. Complete serialized report hashes, including kernel context, stop reasons, and warnings, match across PDB/JSON/SSYM. The format has not hidden those limitations. B's complete flag means this chain walk completed under engine rules, not that no hidden process exists.

## Volatility 3 full ISF comparison

The same PDB files were converted using local Vol3 2.27.0 `PdbReader`, checking source SHA-256,
GUID, and DBI Age. See the [Vol3 aggregate](data/vol3.json) and [command records](data/vol3-commands.json).
These ISFs cover substantially more types, enums, and symbols than the StarMem projection;
the following is an independent experiment.

| Sample | ISF JSON bytes | JSON.xz bytes | User types / enums / symbols | Single PDB → full ISF conversion, seconds |
| --- | ---: | ---: | --- | ---: |
| A | 1,827,720 | 305,968 | 899 / 118 / 18,979 | 8.561 |
| B | 3,504,001 | 586,740 | 1,551 / 272 / 35,837 | 16.545 |
| C | 4,720,593 | 785,032 | 1,964 / 410 / 48,786 | 21.510 |

Conversion includes `PdbReader` initialization and `get_json()`, excluding compression and output.
This is not evidence that SSYM compresses ISF by tens of times: their information content differs.

### Full ISF loading and first lookup

Units: ms. Every run requests `validate=True`. The 9 measured samples use an existing content validation
cache, so they must not be described as rerunning full schema validation on every invocation.
Each invocation is a fresh Python process.

| Sample | JSON median [range] | JSON.xz median [range] | First JSON invocation without prior validation cache |
| --- | ---: | ---: | ---: |
| A | 178.393 [172.523–182.620] | 199.444 [189.375–262.779] | 2173.277 |
| B | 209.745 [205.881–233.490] | 235.654 [229.406–247.428] | 3825.402 |
| C | 251.423 [241.930–259.735] | 277.728 [272.597–316.527] | 4900.123 |

The first JSON invocation also includes other initial setup costs and was sampled only once per ISF.
The full difference is not attributed to schema validation. XZ runs after JSON and shares the validation
cache for identical decompressed content; its first invocation is not an equivalent cold start.
File caches were never explicitly purged.

| Sample | JSON Python traced peak / retained MiB | JSON.xz Python traced peak / retained MiB |
| --- | ---: | ---: |
| A | 14.76 / 10.98 | 21.57 / 10.98 |
| B | 27.54 / 20.34 | 33.44 / 20.34 |
| C | 37.04 / 27.36 | 41.62 / 27.36 |

MiB = 1,048,576 bytes. These come from separate tracemalloc runs, not RSS measurements.
Subtracting Rust requested heap from them does not establish memory savings attributable to the format.

### Native pslist

Built-in format definitions remain available; Windows kernel ISFs are restricted to the experiment's
own directory. After successful runs, saved configuration is checked to confirm use of the ISF generated
from this exact PDB. CLI pipeline timing includes bootstrap, plugin work, and JSON rendering, unlike
StarMem internal session timing. Successful paths have 1 warmup and 3 measured runs; failed paths retain
their first failure.

| Sample | Vol3 records | StarMem records | Vol3 pipeline median [range] ms | Interpretation |
| --- | ---: | ---: | ---: | --- |
| A | 0 | 63 (partial) | 1075.414 [1058.961–1077.772] | Different sets; exit code 0 does not establish retrieval of equivalent evidence |
| B | Unavailable | 455 | Not counted | Nonzero exit; kernel layer and symbol-table requirements were unmet |
| C | 161 | 160 (partial) | 1692.806 [1639.972–1725.804] | 160 matching PID+EPROCESS keys; 1 additional Vol3 record needs verification |

B also emitted a missing VMSS/VMSN metadata warning. This is an observation, not proof of the sole cause.
C's additional record has not been independently verified; it does not establish that StarMem missed
a real process. None of these three samples achieved complete cross-engine output equivalence.
**No overall “SSYM is this many times faster than Vol3” conclusion is published.**

## Compatibility observations

- PDB Info Ages are 4, 2, and 6, while DBI and image Ages are all 1. SSYM stores both: DBI Age for image matching, Info Age for a lossless profile roundtrip.
- A/C StarMem process chains contain `head_blink_mismatch` and are partial. Vol3's zero-row output is not a passing result merely because the exit code is 0. Different outputs are not described as faster completion of the same task.
- Optional libmagic hung during local import. Vol3 tests explicitly use `--without-libmagic` to trigger its existing extension-based fallback; installed Vol3 files were not modified.

## Applicability

A complete ISF replacement still needs a general type graph, enums, pointers/arrays, module identities,
and compatibility design. These results cover StarMem's current Windows kernel projection.
Removing information absent from that projection is not lossless compression of full ISF.

LovelyMem already reuses sessions, so each plugin in an open session does not incur the entire PDB
initialization cost again. The main targets are new sessions, separate CLI runs, and cross-process
reuse. Image scanning, page-table translation, and evidence reconstruction can dominate total time;
symbol loading speedups cannot be extrapolated to the whole application.

## Further experiments

1. Evaluate exact-identity compiled caches and existing session reuse. Identity should cover PDB GUID, DBI Age, source hash, extraction recipe, and format version. Offline operation, stale caches, concurrent writes, and cancellation require dedicated checks.
2. Resolve field handles during plugin initialization and reuse indices or offsets in hot paths. Acceptance means equal actual output and improved total plugin time, not merely cheaper opening. This is not yet integrated.
3. Implement general graphs, enums, arrays, pointers, aliases, and module references before claiming full ISF replacement. Compare FlatBuffers/rkyv with equal content; do not extrapolate from roughly a hundred projected types.
4. Add public distributable images, more Windows versions, cold caches, more machines, actual RSS, concurrent sessions, and longer plugin chains. mmap, buffer preallocation, and complete type graphs remain unimplemented and unverified here.
