#!/usr/bin/env python3
"""Verify the packaged executable can load without the build machine's DLL paths."""
import argparse
import os
import pathlib
import subprocess

def standalone_environment(source):
    env = dict(source)
    root = next((v for k, v in env.items() if k.upper() == "SYSTEMROOT" and v), None)
    if not root:
        root = next((v for k, v in env.items() if k.upper() == "WINDIR" and v), None)
    if not root or not pathlib.PureWindowsPath(root).is_absolute():
        raise RuntimeError("Windows system directory is unavailable")
    for key in list(env):
        if key.upper() == "PATH":
            del env[key]
    env["PATH"] = str(pathlib.PureWindowsPath(root) / "System32")
    return env

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=pathlib.Path)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("Run this executable-package check on Windows")
    executable = args.directory.resolve() / "scummvm.exe"
    for name in ("fonts.dat", "scummremastered.zip", "translations.dat", "Avvia-XFiles-Video-Compatibile.cmd"):
        if not (executable.parent / name).is_file():
            raise RuntimeError(f"Incomplete Windows package: missing {name}")
    env = standalone_environment(os.environ)
    for option in ["--version", "--list-engines"]:
        result = subprocess.run(
            [str(executable), option], cwd=executable.parent, env=env,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding="utf-8", errors="replace", timeout=30,
        )
        if result.returncode != 0:
            raise RuntimeError(f"Packaged executable failed {option}: {result.returncode}\n{result.stdout}\n{result.stderr}")
        if option == "--list-engines" and "xfiles" not in result.stdout.lower():
            raise RuntimeError("The packaged executable does not register the X-Files engine")
        print(result.stdout.strip())
    print("PASS: standalone DLL loading and X-Files engine registration")

if __name__ == "__main__":
    main()
