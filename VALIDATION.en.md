# Validation Record

[中文](VALIDATION.md) · **English** · [Project home](README.en.md)

Date: 2026-10-05. Tested engine build ID:
`1b53c4adc5ec1df68585a5a2912c90b4418c0b92bcfb58d3e1fab79dcb72d994`.
See the [raw aggregate](data/starmem.json) and [source manifest](data/source-manifest.json)
for input identities, executable hashes, and source hashes.

| Check | Result | Scope |
| --- | --- | --- |
| `cargo test ... --release --lib profile::compiled` | 4 passed | Field roundtrips, determinism, every truncation position, single-byte corruption, invalid re-checksummed indices, and identity |
| StarMem native CLI and benchmark example release builds | Passed | Executables remain in the host target; binaries are not distributed |
| Host `cargo check --manifest-path src-tauri/Cargo.toml --locked` | Passed | Does not validate the desktop UI, license server, or every plugin |
| `cargo clippy ... --release --lib --example symbol_format_bench` | Exit code 0, 45 existing engine warnings | No warning located in the new compiled module or example; unrelated modules were not changed to address warnings |
| Native `profile PDB -o kernel.ssym` and `pslist --profile kernel.ssym` | Executed successfully | Sample A produced 63 records with partial-result warnings retained |
| Corrupt SSYM / wrong GUID with valid checksum | Both rejected as expected | [Native command results](data/rejection-checks.json), beyond parser-only checks |
| StarMem, three images and six loading methods | All profile hashes and query checksums match | 9 latency samples; allocations measured separately |
| StarMem, three images and three session methods | Full report hashes match per image | 5 samples per method; A/C are partial |
| Vol3 full ISF from three corresponding PDBs | Conversion and loading passed | 9 loading samples per format; type-graph coverage differs |
| Vol3 native pslist | A empty; B initialization failure; C 161 records | Differences retained; cross-engine correctness is not claimed |
| StarMem vendor source manifest | Verification passed under PowerShell 7 | Includes pre-existing shared-tree changes; does not replace the tested build ID |
| Research integration patch | `git apply --check` passed on parent main.rs / session.rs | Other versions still require context review |

The study covers Windows x64 kernel profiles and pslist only. Other plugins, module PDBs, Linux,
x86 PAE, cold disks, more hardware, and a general full-ISF type graph remain outside its scope.
The SSYM integration does not change the default symbol path.

The publication bundle can be checked offline without images or a license:

```powershell
python -B -X utf8 ssym-research/scripts/verify-study.py
```

This checks source hashes, raw samples and aggregates, JSON/SSYM content equality markers, session
hashes, Vol3 PDB identities, document links, and local absolute paths in published data. It does not
replace rerunning the engine or independently reviewing original evidence.

The bilingual documentation revision does not change measured source or samples and does not rerun
engine benchmarks. Source tests and performance results refer to the experimental build above.
Documentation checks cover language pairs, numeric tables, example commands, and links.

Pairing, numeric tables, and command consistency passed for 9 Chinese/English document pairs.
All 10 Mermaid blocks passed syntax checks and rendered locally with Mermaid 11.12.0 in a headless
browser; key layouts were visually inspected. This verifies local documentation, not publication
or rendering on GitHub's servers. Disk layouts additionally use monospace text diagrams.
The distribution uses [Apache-2.0](LICENSE-APACHE); the full StarMem engine source remains unpublished.
