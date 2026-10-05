# Research Tools

[中文](README.md) · **English** · [Project home](../README.en.md)

These scripts implement one research workflow. Complete commands are in [Reproduction](../REPRODUCE.en.md).
The scripts use Python's standard library; the Vol3 script additionally requires a working local
Volatility 3 environment.

| Tool | Input | Output and purpose |
| --- | --- | --- |
| [benchmark-starmem.py](benchmark-starmem.py) | Licensed benchmark EXE, images, optional symbol roots | Serial fresh processes; equal-content files; warmup, allocation and latency runs; reports and statistics |
| [benchmark-vol3.py](benchmark-vol3.py) | Original StarMem results.json and local Vol3 | Full ISF from the same PDB; isolated symbols; loading and native pslist; failures retained |
| [check-ssym-rejection.py](check-ssym-rejection.py) | Native CLI, image, valid SSYM | Corrupt and wrong-GUID files, verifying rejection through real sessions |
| [export-study.py](export-study.py) | Experimental checkout and local run directories | Allowlisted source/statistics export, sanitized paths, Apache-2.0 license, no document overwrite |
| [verify-study.py](verify-study.py) | Current publication directory | Offline source hashes, statistics, language pairs, numeric tables, commands, links, and data paths |

Benchmarks and rejection checks require new output directories to avoid mixing experiments.
`export-study.py` updates published data and reference source; run it only when inputs belong to the
same tested revision. Translating documentation does not require re-export or new benchmark runs.
Raw stderr is useful for diagnosis, but still needs review for machine- or sample-specific information before publication.

Flowchart source is stored in Markdown Mermaid blocks; byte layouts use monospace text diagrams.
Cross-check format changes against the [specification](../FORMAT.en.md) and reference implementation,
rather than inferring new field semantics from an illustration. Code and data retain original identifiers;
explanatory documents are available in both languages.
