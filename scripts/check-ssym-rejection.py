"""Reject corrupt and re-checksummed wrong-identity SSYM through the real CLI."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["exe", "image", "ssym", "output"]:
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        parser.error("output directory must be new")
    source = args.ssym.read_bytes()
    if source[:8] != b"SSYM\r\n\x1a\n":
        parser.error("input is not SSYM")
    output.mkdir(parents=True)
    damaged = bytearray(source)
    damaged[-1] ^= 1
    wrong_guid = bytearray(source)
    type_count, field_count, symbol_count = struct.unpack_from("<III", source, 60)
    strings = 192 + type_count * 24 + field_count * 32 + symbol_count * 16
    guid_offset = strings + struct.unpack_from("<I", source, 32)[0]
    wrong_guid[guid_offset] = ord("B") if wrong_guid[guid_offset] == ord("A") else ord("A")
    wrong_guid[144:176] = hashlib.sha256(wrong_guid[:144] + wrong_guid[176:]).digest()
    results = []
    for name, data, expected in [("corrupt", damaged, "SHA-256"),
                                 ("wrong-guid-valid-checksum", wrong_guid, "与目标内核不匹配")]:
        path = output / f"{name}.ssym"
        path.write_bytes(data)
        command = [str(args.exe.resolve()), "--json", "--threads", "2", "pslist", str(args.image.resolve()),
                   "--profile", str(path)]
        proc = subprocess.run(command, capture_output=True, encoding="utf-8", timeout=120,
                              creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        passed = proc.returncode != 0 and expected in proc.stderr
        results.append({"case": name, "passed": passed, "exit_code": proc.returncode,
                        "error": proc.stderr, "argv": command})
    (output / "results.json").write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(results, ensure_ascii=False, indent=2))
    if not all(value["passed"] for value in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
