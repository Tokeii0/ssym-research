"""Optional Volatility 3 comparison using the exact PDBs from a StarMem run.

No Volatility source changes. --without-libmagic selects its documented optional-
dependency fallback only in this process (useful for broken Windows libmagic).
Full ISF is not semantically equivalent to StarMem's selected KernelProfile;
results must remain separate from the equal-content JSON/SSYM comparison.
"""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import io
import json
import lzma
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time
import tracemalloc


def digest(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def configure(args):
    if args.without_libmagic:
        # resources.py already handles ImportError and falls back to suffixes.
        sys.modules["magic"] = None
    from volatility3.framework import constants
    constants.OFFLINE = True
    constants.CACHE_PATH = str(Path(args.cache).resolve())
    Path(constants.CACHE_PATH).mkdir(parents=True, exist_ok=True)
    return constants


def worker(args):
    constants = configure(args)
    if args.worker == "convert":
        from volatility3.framework import contexts
        from volatility3.framework.symbols.windows import pdbconv
        path = Path(args.pdb).resolve(strict=True)
        before = digest(path)
        started = time.perf_counter()
        reader = pdbconv.PdbReader(contexts.Context(), path.as_uri(), database_name=path.name)
        data = reader.get_json()
        convert_ms = (time.perf_counter() - started) * 1000
        if digest(path) != before:
            raise RuntimeError("PDB changed during conversion")
        identity = data["metadata"]["windows"]["pdb"]
        destination = Path(args.symbol_root) / "windows" / path.name / f"{identity['GUID']}-{identity['age']}.json"
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.exists():
            raise FileExistsError(destination)
        encoded = json.dumps(data, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        destination.write_bytes(encoded)
        compressed = destination.with_suffix(".json.xz")
        # Keep XZ outside the search root, so pslist has one kernel ISF candidate.
        compressed = Path(args.cache).parent / compressed.name
        compressed.write_bytes(lzma.compress(encoded))
        return {"volatility_version": constants.PACKAGE_VERSION, "pdb_sha256": before,
                "pdb_to_full_isf_ms": convert_ms, "isf": str(destination), "isf_xz": str(compressed),
                "json_bytes": len(encoded), "xz_bytes": compressed.stat().st_size,
                "isf_sha256": digest(destination), "identity": identity,
                "counts": {name: len(data.get(name, {})) for name in ["base_types", "user_types", "enums", "symbols"]}}
    if args.worker == "load":
        from volatility3.framework import contexts
        from volatility3.framework.symbols import intermed
        ctx = contexts.Context()
        if args.measure_memory:
            tracemalloc.start()
        started = time.perf_counter()
        table = intermed.IntermediateSymbolTable(ctx, "benchmark", "kernel",
                                               Path(args.isf).resolve().as_uri(), validate=True)
        # Force the fields and symbol used by process enumeration, not every ISF type.
        field = table.get_type("_EPROCESS").relative_child_offset("UniqueProcessId")
        address = table.get_symbol("PsActiveProcessHead").address
        elapsed_ms = (time.perf_counter() - started) * 1000
        current, peak = tracemalloc.get_traced_memory() if args.measure_memory else (None, None)
        if args.measure_memory:
            tracemalloc.stop()
        return {"load_and_first_lookup_ms": elapsed_ms, "python_traced_live_bytes": current,
                "python_traced_peak_bytes": peak, "field_offset": field, "symbol_rva": address,
                "isf": args.isf, "schema_validation": True, "volatility_version": constants.PACKAGE_VERSION}
    if args.worker == "pslist":
        import volatility3.framework.symbols as builtin_symbols
        # Exclude installed/user PDB collections; retain only built-in formats.
        constants.SYMBOL_BASEPATHS = [str(Path(builtin_symbols.__file__).parent)]
        from volatility3.cli import CommandLine
        config_path = Path(args.report).with_suffix(".config.json")
        sys.argv = ["vol", "-q", "--offline", "--parallelism", "off", "--cache-path", args.cache,
                    "-s", args.symbol_root, "-f", args.image, "--save-config", str(config_path),
                    "-r", "json", "windows.pslist.PsList"]
        stream = io.StringIO()
        started = time.perf_counter()
        with contextlib.redirect_stdout(stream):
            try:
                CommandLine().run()
            except SystemExit as error:
                if error.code:
                    raise
        elapsed_ms = (time.perf_counter() - started) * 1000
        text = stream.getvalue()
        Path(args.report).write_text(text, encoding="utf-8")
        rows = json.loads(text)
        config = json.loads(config_path.read_text(encoding="utf-8"))
        selected = config.get("kernel.symbol_table_name.isf_url", "")
        expected_root = Path(args.symbol_root).resolve().as_uri().lower() + "/"
        if not selected.lower().startswith(expected_root):
            raise RuntimeError(f"Vol3 did not use the case's exact ISF: {selected}")
        return {"cli_pipeline_ms": elapsed_ms, "records": len(rows), "report": args.report,
                "report_sha256": digest(args.report), "config": str(config_path),
                "volatility_version": constants.PACKAGE_VERSION}
    raise ValueError(args.worker)


def stats(values):
    return {"median": statistics.median(values), "min": min(values), "max": max(values), "samples": len(values)}


def orchestrate(args):
    output = Path(args.output).resolve()
    if output.exists():
        raise FileExistsError("use a new output directory")
    output.mkdir(parents=True)
    starmem = json.loads(Path(args.starmem_results).read_text(encoding="utf-8"))
    calls, cases = [], []

    def run(action, arguments, case):
        command = [sys.executable, "-B", "-X", "utf8", str(Path(__file__).resolve()), "--worker", action,
                   "--cache", str(case / "cache")]
        if args.without_libmagic:
            command.append("--without-libmagic")
        command += list(map(str, arguments))
        started = time.perf_counter()
        proc = subprocess.run(command, capture_output=True, encoding="utf-8", timeout=args.timeout,
                              creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        calls.append({"argv": command, "wall_seconds": time.perf_counter()-started,
                      "exit_code": proc.returncode, "stderr": proc.stderr})
        save(output / "commands.json", calls)
        if proc.returncode:
            if action == "pslist":
                return {"status": "failed", "exit_code": proc.returncode,
                        "stderr": proc.stderr, "process_wall_ms": calls[-1]["wall_seconds"] * 1000}
            raise RuntimeError(f"Vol3 {action} failed: {proc.stderr[-4000:]}")
        result = json.loads(proc.stdout)
        result["process_wall_ms"] = calls[-1]["wall_seconds"] * 1000
        return result

    for index, source in enumerate(starmem["cases"]):
        if args.case is not None and index + 1 != args.case:
            continue
        manifest = source["manifest"]
        case = output / f"case-{index + 1:02d}"
        case.mkdir()
        symbol_root = case / "symbols"
        print(f"Vol3: case {index+1}, converting exact PDB", flush=True)
        converted = run("convert", ["--pdb", manifest["pdb"], "--symbol-root", symbol_root], case)
        identity = converted["identity"]
        expected = manifest["identity"]
        if (identity["GUID"].replace("-", "").upper() != expected["guid"].replace("-", "").upper()
                or identity["age"] != expected["age"] or converted["pdb_sha256"] != manifest["pdb_sha256"]):
            raise AssertionError("Vol3 converted a different PDB identity")
        save(case / "conversion.json", converted)
        samples = {}
        for variant, path in [("json", converted["isf"]), ("json_xz", converted["isf_xz"])]:
            cold = run("load", ["--isf", path], case)
            heap = run("load", ["--isf", path, "--measure-memory"], case)
            values = [run("load", ["--isf", path], case) for _ in range(args.repetitions)]
            samples[variant] = {"first_run": cold, "memory_run": heap, "samples": values,
                                "load_ms": stats([v["load_and_first_lookup_ms"] for v in values])}
            save(case / "load-samples.json", samples)
            print(f"  {variant}: {samples[variant]['load_ms']['median']:.3f} ms", flush=True)
        pslist = []
        for trial in range(args.session_repetitions + 1):
            result = run("pslist", ["--image", manifest["image"], "--symbol-root", symbol_root,
                                    "--report", case / f"pslist-{trial}.json"], case)
            pslist.append(result)
            save(case / "session-samples.json", pslist)
            if result.get("status") == "failed":
                print(f"  pslist trial {trial}: failed (exit {result['exit_code']}); preserving failure", flush=True)
                break
            print(f"  pslist trial {trial}/{args.session_repetitions}: {result['records']} processes", flush=True)
        # The StarMem driver preserves the report next to its manifest artifacts.
        baseline_path = Path(manifest["artifacts"][0]["path"]).parent / "pslist-pdb-1.json"
        star_rows = json.loads(baseline_path.read_text(encoding="utf-8"))["processes"]["processes"]
        def number(value):
            return int(value, 0) if isinstance(value, str) else int(value)
        def canonical_x64(value):
            address = number(value)
            if address < (1 << 48) and address & (1 << 47):
                address |= 0xFFFF000000000000
            return address
        star_set = {(number(row["pid"]), canonical_x64(row["eprocess"])) for row in star_rows}
        successful = [sample for sample in pslist if "report" in sample]
        keys = []
        for sample in successful:
            rows = json.loads(Path(sample["report"]).read_text(encoding="utf-8"))
            keys.append({(number(row["PID"]), canonical_x64(row["Offset(V)"])) for row in rows})
        if keys and any(key != keys[0] for key in keys):
            raise AssertionError("Vol3 process identities changed between trials")
        vol_set = keys[-1] if keys else None
        comparison = {"key": "PID + sign-extended x64 EPROCESS virtual address", "starmem_rows": len(star_set),
                      "status": "failed" if vol_set is None else "equal" if star_set == vol_set else "different",
                      "vol3_rows": len(vol_set) if vol_set is not None else None,
                      "equal": star_set == vol_set if vol_set is not None else None}
        if vol_set is not None:
            comparison.update(only_starmem=sorted(star_set-vol_set), only_vol3=sorted(vol_set-star_set))
        timed = [v["cli_pipeline_ms"] for v in pslist[1:] if "cli_pipeline_ms" in v]
        case_result = {"image_sha256": manifest["image_sha256"], "image_bytes": manifest["image_bytes"],
                       "conversion": converted, "loads": samples, "sessions": pslist,
                       "cli_pipeline_ms": stats(timed) if timed else None, "comparison": comparison}
        cases.append(case_result)
        save(output / "results.json", {"cases": cases, "without_libmagic": args.without_libmagic,
                                       "method": "full Vol3 ISF; serial native pslist; offline; warm file and schema caches; not a same-content SSYM comparison"})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", choices=["convert", "load", "pslist"])
    parser.add_argument("--without-libmagic", action="store_true")
    parser.add_argument("--measure-memory", action="store_true")
    for name in ["cache", "pdb", "isf", "symbol-root", "image", "report", "starmem-results", "output"]:
        parser.add_argument(f"--{name}")
    parser.add_argument("--case", type=int)
    parser.add_argument("--repetitions", type=int, default=9)
    parser.add_argument("--session-repetitions", type=int, default=3)
    parser.add_argument("--timeout", type=int, default=600)
    args = parser.parse_args()
    if args.worker:
        print(json.dumps(worker(args), ensure_ascii=False))
    else:
        if not args.starmem_results or not args.output:
            parser.error("--starmem-results and --output are required")
        orchestrate(args)


if __name__ == "__main__":
    main()
