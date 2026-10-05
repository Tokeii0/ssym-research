"""Serial, fresh-process SSYM experiment; preserves all raw samples and command lines.

Build examples/symbol_format_bench with the vendor lockfile and --release first.
This writes only inside the explicitly supplied, new output directory. Input images
and existing symbol roots are read-only; exact missing PDBs use StarMem's cache.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import time


def digest(path):
    result = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def summary(values):
    ordered = sorted(values)
    return {"median": statistics.median(ordered), "min": ordered[0], "max": ordered[-1],
            "samples": len(ordered)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", required=True, type=Path)
    parser.add_argument("--image", action="append", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--symbol-path", action="append", default=[], type=Path)
    parser.add_argument("--repetitions", type=int, default=9)
    parser.add_argument("--session-repetitions", type=int, default=5)
    args = parser.parse_args()
    if not (3 <= args.repetitions <= 100 and 3 <= args.session_repetitions <= 100):
        parser.error("repeat counts must be 3..100")
    args.exe = args.exe.resolve(strict=True)
    args.image = [image.resolve(strict=True) for image in args.image]
    output = args.output.resolve()
    if output.exists():
        parser.error("output directory must not exist; use a new run directory")
    if any(output == image or image in output.parents for image in args.image):
        parser.error("output must not alias an input")
    output.mkdir(parents=True)
    calls = []
    rng = random.Random(20261005)

    def run(arguments, label):
        command = [str(args.exe), *map(str, arguments)]
        started = time.perf_counter()
        completed = subprocess.run(command, capture_output=True, encoding="utf-8", errors="strict",
                                   timeout=900, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        calls.append({"label": label, "argv": command, "wall_seconds": time.perf_counter() - started,
                      "exit_code": completed.returncode, "stderr": completed.stderr})
        save(output / "commands.json", calls)
        if completed.returncode:
            raise RuntimeError(f"{label}: exit {completed.returncode}: {completed.stderr[-4000:]}")
        return json.loads(completed.stdout)

    all_results = {"host": {"platform": platform.platform(), "processor": platform.processor(),
                            "logical_cpus": os.cpu_count()}, "executable": str(args.exe),
                   "executable_sha256": digest(args.exe), "seed": 20261005, "threads": 2,
                   "method": "serial fresh processes; warm OS file cache; latency without heap counters; separate counted-heap run; no cache purge",
                   "cases": []}
    for index, image in enumerate(args.image):
        case = output / f"{index + 1:02d}-{image.stem}"
        print(f"Preparing {image.name}", flush=True)
        prepare_args = ["prepare", "--image", image, "--output", case]
        for root in args.symbol_path:
            prepare_args += ["--symbol-path", root]
        manifest = run(prepare_args, f"{index}:prepare")
        backends = {
            "pdb": Path(manifest["pdb"]),
            "json-reader": case / "profile.json",
            "json-slice": case / "profile.json",
            "json-zstd": case / "profile.json.zst",
            "ssym": case / "profile.ssym",
            "ssym-owned": case / "profile.ssym",
        }
        samples = {name: [] for name in backends}
        heaps = {}
        warmups = {}

        def load(name, extra=()):
            result = run([*extra, "load", "--format", name, "--path", backends[name]], f"{index}:load:{name}")
            if result["profile_sha256"] != manifest["profile_sha256"]:
                raise AssertionError(f"{image.name}: {name} changed symbol content")
            return result

        for name in backends:
            warmups[name] = load(name)
            heaps[name] = load(name, ["--measure-heap"])
        lookup_sum = warmups["pdb"]["lookup_sum"]
        for trial in range(args.repetitions):
            order = list(backends)
            rng.shuffle(order)
            for name in order:
                value = load(name)
                if value["lookup_sum"] != lookup_sum:
                    raise AssertionError("lookup results differ")
                samples[name].append(value)
            print(f"  format trial {trial + 1}/{args.repetitions}", flush=True)
            save(case / "load-samples.json", {"samples": samples, "heaps": heaps, "warmups": warmups})
        loads = {name: {"load_us": summary([v["load_us"] for v in values]),
                        "lookup_pair_ns": summary([v["lookup_pair_ns"] for v in values]),
                        **{key: heaps[name][key] for key in ["file_bytes", "peak_heap_bytes", "retained_heap_bytes", "allocation_calls"]}}
                 for name, values in samples.items()}

        sessions = {name: [] for name in ["pdb", "json", "ssym"]}
        session_paths = {"pdb": backends["pdb"], "json": backends["json-reader"], "ssym": backends["ssym"]}
        hashes = set()
        for trial in range(args.session_repetitions + 1):
            order = list(sessions)
            rng.shuffle(order)
            for name in order:
                result = run(["session", "--image", image, "--format", name, "--path", session_paths[name],
                              "--report", case / f"pslist-{name}-{trial}.json"], f"{index}:session:{name}:{trial}")
                hashes.add(result["report_sha256"])
                if trial:
                    sessions[name].append(result)
            if len(hashes) != 1:
                raise AssertionError(f"{image.name}: kernel/pslist reports differ; inspect preserved reports")
            save(case / "session-samples.json", sessions)
            print(f"  session trial {trial}/{args.session_repetitions} (0 is warmup)", flush=True)
        result = {"manifest": manifest, "loads": loads, "sessions": {
            name: {"open_ms": summary([v["open_ms"] for v in values]),
                   "pslist_ms": summary([v["pslist_ms"] for v in values]),
                   "total_ms": summary([v["total_ms"] for v in values]),
                   "records": values[0]["records"], "complete": all(v["complete"] for v in values),
                   "report_sha256": values[0]["report_sha256"]}
            for name, values in sessions.items()}, "symbol_roundtrip_equal": True, "session_reports_equal": True}
        all_results["cases"].append(result)
        save(output / "results.json", all_results)
        print(f"  finished: {sessions['pdb'][0]['records']} processes; complete={sessions['pdb'][0]['complete']}; all reports identical", flush=True)
    save(output / "artifact-hashes.json", [{"path": str(path.relative_to(output)), "sha256": digest(path)}
         for path in sorted(output.rglob("*")) if path.is_file()])
    print(output / "results.json")


if __name__ == "__main__":
    main()
