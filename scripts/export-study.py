"""Copy an allowlisted implementation and sanitized benchmark samples into this study.

Raw evidence stays in the local run directory. No images, PDBs, SSYM files,
licenses, or process records are copied into the publication bundle.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import shutil
import subprocess


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8-sig"))


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--starmem-run", type=Path, required=True)
    parser.add_argument("--vol3-run", type=Path)
    parser.add_argument("--environment", type=Path)
    parser.add_argument("--rejection-results", type=Path)
    args = parser.parse_args()
    repo = args.repository.resolve(strict=True)
    package = Path(__file__).resolve().parents[1]
    vendor = repo / "src-tauri/vendor/starmem"
    star = read(args.starmem_run / "results.json")
    replacements = [(str(args.starmem_run.resolve()), "${STARMEM_RUN}"),
                    (str(Path(star["executable"])), "${STARMEM_BENCH}"),
                    (str(repo), "${REPOSITORY}"), (str(Path.home()), "${USER_HOME}")]
    if args.vol3_run:
        replacements.append((str(args.vol3_run.resolve()), "${VOL3_RUN}"))
    if args.rejection_results:
        replacements.append((str(args.rejection_results.resolve().parent), "${REJECTION_RUN}"))
    for index, case in enumerate(star["cases"]):
        manifest = case["manifest"]
        replacements.extend([(manifest["image"], f"${{IMAGE_{chr(65+index)}}}"),
                             (manifest["pdb"], f"${{PDB_{chr(65+index)}}}"),
                             (str(Path(manifest["artifacts"][0]["path"]).parent), f"${{CASE_{chr(65+index)}}}")])
        image = Path(manifest["image"])
        for suffix in [image.suffix, ".vmss", ".vmsn"]:
            replacements.append((image.with_suffix(suffix).name, f"image-{chr(65+index)}{suffix}"))
    replacements.sort(key=lambda item: len(item[0]), reverse=True)

    def clean(value):
        if isinstance(value, dict):
            return {key: clean(item) for key, item in value.items() if key not in {"embedded_path", "physical_address"}}
        if isinstance(value, list):
            return [clean(item) for item in value]
        if isinstance(value, str):
            for before, after in replacements:
                value = re.sub(re.escape(before), lambda _: after, value, flags=re.I)
                value = re.sub(re.escape(before.replace("\\", "/")), lambda _: after, value, flags=re.I)
            return value
        return value

    save(package / "data/starmem.json", clean(star))
    save(package / "data/starmem-commands.json", clean(read(args.starmem_run / "commands.json")))
    for index, case in enumerate(star["cases"]):
        directory = Path(case["manifest"]["artifacts"][0]["path"]).parent
        for name in ["load-samples", "session-samples"]:
            save(package / f"data/image-{chr(65+index)}/{name}.json", clean(read(directory / f"{name}.json")))
        report = read(directory / "pslist-pdb-1.json")
        processes = report["processes"]
        save(package / f"data/image-{chr(65+index)}/report-status.json", {
            "report_sha256": digest(directory / "pslist-pdb-1.json"),
            "records": len(processes["processes"]), "complete": processes["complete"],
            "stop_reason": processes["stop_reason"],
            "warning_kinds": [warning["kind"] for warning in processes["warnings"]],
            "note": "Only completeness metadata is published; raw process records and warning addresses stay local."})
    if args.vol3_run:
        vol3 = read(args.vol3_run / "results.json")
        # Differences are useful, but individual process identities are not needed in the public study.
        for case in vol3["cases"]:
            comparison = case["comparison"]
            for key in ["only_starmem", "only_vol3"]:
                if key in comparison:
                    comparison[key + "_count"] = len(comparison.pop(key))
        save(package / "data/vol3.json", clean(vol3))
        save(package / "data/vol3-commands.json", clean(read(args.vol3_run / "commands.json")))
    if args.environment:
        save(package / "data/environment.json", clean(read(args.environment)))
    if args.rejection_results:
        save(package / "data/rejection-checks.json", clean(read(args.rejection_results)))

    files = ["src/profile.rs", "src/profile/compiled.rs", "src/profile/compiled/tests.rs",
             "examples/symbol_format_bench.rs", "Cargo.lock", "Cargo.toml"]
    manifest = []
    for name in files:
        source = vendor / name
        destination = package / "reference" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        manifest.append({"path": "reference/" + name, "sha256": digest(source), "bytes": source.stat().st_size})
    # Bilingual research documents are authored in this package; exporting measurements
    # must not overwrite them with the shorter engine integration note.
    shutil.copyfile(vendor / "LICENSE-APACHE", package / "LICENSE-APACHE")

    # Keep only the research hunks, excluding unrelated existing work in this shared checkout.
    patch = subprocess.run(["git", "diff", "--no-ext-diff", "--unified=3", "--",
                            "src-tauri/vendor/starmem/src/session.rs", "src-tauri/vendor/starmem/src/main.rs"],
                           cwd=repo, check=True, capture_output=True, encoding="utf-8").stdout
    selected = []
    for file_diff in re.split(r"(?=^diff --git )", patch, flags=re.M):
        parts = re.split(r"(?=^@@ )", file_diff, flags=re.M)
        hunks = [hunk for hunk in parts[1:] if any(token in hunk for token in [".ssym", "SSYM", "profile::compiled", "let compiled"])]
        if hunks:
            selected.append(parts[0] + "".join(hunks))
    text = "".join(selected).replace("src-tauri/vendor/starmem/", "")
    # Full working-tree blob IDs would be misleading for this intentionally partial patch.
    text = re.sub(r"^index [^\n]+\n", "", text, flags=re.M)
    (package / "reference/integration.patch").write_bytes(text.encode("utf-8"))
    save(package / "data/source-manifest.json", {"files": manifest,
         "rustc": subprocess.check_output(["rustc", "-V"], text=True).strip(),
         "python": platform.python_version(), "parent_git_commit": subprocess.check_output(
             ["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
         "note": "Tested shared working tree includes pre-existing changes; engine build IDs identify the actual source, not the parent commit alone."})
    print(package)


if __name__ == "__main__":
    main()
