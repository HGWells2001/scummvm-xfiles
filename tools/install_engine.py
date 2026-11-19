#!/usr/bin/env python3
"""Install the experimental engine into the reviewed ScummVM revision."""
import argparse
import pathlib
import shutil
import subprocess

PIN = "55c2973c1df5e8790b4248ae36a6b1b37842ce12"

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("scummvm", type=pathlib.Path)
    args = parser.parse_args()
    root = args.scummvm.resolve()
    if not (root / "engines" / "engine.h").is_file():
        parser.error("Not a ScummVM source checkout")
    revision = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
    if revision != PIN:
        parser.error("Use a separate checkout at " + PIN)
    source = pathlib.Path(__file__).resolve().parents[1] / "engines" / "xfiles"
    destination = root / "engines" / "xfiles"
    if destination.exists():
        expected = {p.name: p.read_bytes() for p in source.iterdir() if p.is_file()}
        actual = {p.name: p.read_bytes() for p in destination.iterdir() if p.is_file()}
        if expected == actual:
            print("Matching engine already installed")
            return
        parser.error("Existing engine differs; use a fresh ScummVM checkout")
    shutil.copytree(source, destination)
    print("Installed experimental database reader. The game is not playable yet.")

if __name__ == "__main__":
    main()
