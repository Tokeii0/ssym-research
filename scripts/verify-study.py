"""Verify published source hashes, aggregate statistics and data boundaries offline."""
from __future__ import annotations

import ast
import hashlib
import json
from pathlib import Path
import re
import statistics


ROOT = Path(__file__).resolve().parents[1]


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def check_stats(actual, values):
    assert actual == {"median": statistics.median(values), "min": min(values),
                      "max": max(values), "samples": len(values)}, (actual, values)


def check_documents():
    pairs = 0
    for path in ROOT.rglob("*.md"):
        if path.name.endswith(".en.md"):
            continue
        translated = path.with_name(path.stem + ".en.md")
        assert translated.exists(), f"missing English counterpart: {path}"
        zh = path.read_text(encoding="utf-8")
        en = translated.read_text(encoding="utf-8")
        assert f"]({translated.name})" in zh and f"]({path.name})" in en, path
        # Preserve executable examples and byte-layout equations across translations.
        for language in ["powershell", "text"]:
            pattern = rf"```{language}\s*\n(.*?)```"
            assert re.findall(pattern, zh, re.S) == re.findall(pattern, en, re.S), (path, language)
        # These documents contain the complete measured and binary-layout numeric tables.
        if path.name in {"README.md", "BENCHMARKS.md", "FORMAT.md"} and path.parent == ROOT:
            def numbers(text):
                return [re.findall(r"(?<![A-Za-z0-9_])\d+(?:[.,]\d+)*(?![A-Za-z0-9_])", row)
                        for row in text.splitlines() if row.startswith("|")]
            assert numbers(zh) == numbers(en), f"numeric table mismatch: {path}"
        assert len(re.findall(r"^```mermaid$", zh, re.M)) == len(re.findall(r"^```mermaid$", en, re.M)), path
        pairs += 1
    assert (ROOT / "LICENSE-APACHE").is_file()
    assert not (ROOT / "LICENSE-MIT").exists()
    for path in ROOT.rglob("*.md"):
        assert "LICENSE-MIT" not in path.read_text(encoding="utf-8"), path
    return pairs


def main():
    source = read(ROOT / "data/source-manifest.json")
    for item in source["files"]:
        contents = (ROOT / item["path"]).read_bytes()
        assert len(contents) == item["bytes"]
        assert hashlib.sha256(contents).hexdigest() == item["sha256"], item["path"]
    star = read(ROOT / "data/starmem.json")
    profiles = {}
    for index, case in enumerate(star["cases"]):
        directory = ROOT / f"data/image-{chr(65+index)}"
        manifest = case["manifest"]
        profiles[manifest["image_sha256"]] = case
        assert manifest["recipe_sha256"] == hashlib.sha256((ROOT / "reference/src/profile.rs").read_bytes()).hexdigest()
        loads = read(directory / "load-samples.json")
        checksum = set()
        for name, values in loads["samples"].items():
            summary = case["loads"][name]
            for key in ["load_us", "lookup_pair_ns"]:
                check_stats(summary[key], [v[key] for v in values])
            heap = loads["heaps"][name]
            for key in ["peak_heap_bytes", "retained_heap_bytes", "allocation_calls", "file_bytes"]:
                assert summary[key] == heap[key]
            warmup = loads["warmups"][name]
            for sample in values + [heap, warmup]:
                assert sample["profile_sha256"] == manifest["profile_sha256"]
                checksum.add(sample["lookup_sum"])
            assert all(not v["heap_instrumented"] for v in values)
            assert heap["heap_instrumented"]
        assert len(checksum) == 1
        sessions = read(directory / "session-samples.json")
        status = read(directory / "report-status.json")
        for name, values in sessions.items():
            summary = case["sessions"][name]
            for key in ["open_ms", "pslist_ms", "total_ms"]:
                check_stats(summary[key], [v[key] for v in values])
            for sample in values:
                for key in ["report_sha256", "complete", "records"]:
                    assert sample[key] == status[key] == summary[key]
        assert case["symbol_roundtrip_equal"] and case["session_reports_equal"]

    vol_path = ROOT / "data/vol3.json"
    if vol_path.exists():
        for case in read(vol_path)["cases"]:
            manifest = profiles[case["image_sha256"]]["manifest"]
            conversion = case["conversion"]
            assert conversion["pdb_sha256"] == manifest["pdb_sha256"]
            assert conversion["identity"]["age"] == manifest["identity"]["age"]
            assert conversion["identity"]["GUID"] == manifest["identity"]["guid"].replace("-", "")
            lookup_results = set()
            for variant in case["loads"].values():
                check_stats(variant["load_ms"], [v["load_and_first_lookup_ms"] for v in variant["samples"]])
                for value in variant["samples"] + [variant["first_run"], variant["memory_run"]]:
                    lookup_results.add((value["field_offset"], value["symbol_rva"]))
            assert len(lookup_results) == 1
            timed = [v["cli_pipeline_ms"] for v in case["sessions"][1:] if "cli_pipeline_ms" in v]
            if timed:
                check_stats(case["cli_pipeline_ms"], timed)
            else:
                assert case["cli_pipeline_ms"] is None
                assert case["comparison"]["status"] == "failed"

    rejection = ROOT / "data/rejection-checks.json"
    if rejection.exists():
        assert all(item["passed"] and item["exit_code"] != 0 for item in read(rejection))
    for path in (ROOT / "data").rglob("*.json"):
        text = path.read_text(encoding="utf-8")
        read(path)
        assert not re.search(r"[A-Za-z]:[\\/]", text), f"absolute local path: {path}"
        assert '"embedded_path"' not in text and '"physical_address"' not in text
    for path in (ROOT / "scripts").glob("*.py"):
        ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    for path in ROOT.rglob("*.md"):
        for target in re.findall(r"\]\(([^)]+)\)", path.read_text(encoding="utf-8")):
            if not target.startswith(("http://", "https://", "#")):
                assert (path.parent / target.split("#")[0]).exists(), (path, target)
    forbidden = {".pdb", ".ssym", ".raw", ".vmem", ".mem", ".dmp", ".exe", ".dll", ".pyc"}
    for path in ROOT.rglob("*"):
        assert path.suffix.lower() not in forbidden, path
        assert path.name.lower() != "license.json", path
    pairs = check_documents()
    print(f"Verified {len(star['cases'])} StarMem cases, source hashes, raw-sample aggregates, Vol3 results and publication paths.")
    print(f"Verified {pairs} bilingual document pairs, numeric tables, command examples, and Apache-2.0 distribution license.")
    print("This checks the publication bundle; it does not rerun the engines or establish evidence completeness.")


if __name__ == "__main__":
    main()
