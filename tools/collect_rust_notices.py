#!/usr/bin/env python3
"""Package notices shipped by the exact resolved Cargo dependencies."""
import argparse
import json
import pathlib
import subprocess

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version=1", "--manifest-path", str(args.manifest)],
        check=True, stdout=subprocess.PIPE, text=True, encoding="utf-8",
    )
    metadata = json.loads(result.stdout)
    parts = ["Resolved Rust dependency notices\nGenerated from the committed Cargo.lock.\n"]
    notices = 0
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package.get("source") is None:
            continue
        root = pathlib.Path(package["manifest_path"]).parent
        parts.append("\n" + "=" * 72 + "\n" + package["name"] + " " + package["version"] + "\n")
        parts.append("Source: " + (package.get("repository") or package["source"]) + "\n")
        parts.append("License expression: " + (package.get("license") or "see source notice") + "\n")
        files = {p for p in root.iterdir() if p.is_file() and p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"))}
        if package.get("license_file"):
            files.add(pathlib.Path(package["license_file"]))
        for file in sorted(files):
            if file.is_file():
                parts.append("\n--- " + file.name + " ---\n")
                parts.append(file.read_text(encoding="utf-8", errors="replace") + "\n")
                notices += 1
        if not files:
            parts.append("The package does not ship a separate notice file; see its source and license expression.\n")
    sysroot = pathlib.Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    for root in [sysroot, sysroot / "share" / "doc" / "rust"]:
        if not root.is_dir():
            continue
        for file in sorted(root.iterdir()):
            if file.is_file() and file.name.upper().startswith(("LICENSE", "COPYRIGHT")):
                parts.append("\n--- Rust toolchain: " + file.name + " ---\n")
                parts.append(file.read_text(encoding="utf-8", errors="replace") + "\n")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("".join(parts), encoding="utf-8")
    print(f"Collected {notices} dependency notice files")

if __name__ == "__main__":
    main()
