# The X-Files Game (1998) - ScummVM research engine

**Experimental first milestone. The full game is NOT playable.**

This repository contains a native ScummVM detector and a diagnostic engine that
opens the NeoAccess title and game-state databases. It does not run the original
executable and does not yet interpret the game's scripts or implement gameplay.

Implemented:

- Exact detection of the local database pair, using ScummVM's first-5000-byte MD5 convention.
- Bounded big-endian NeoAccess 5.01 class-tree and object-index reader.
- Asset-reference blob lookup, malformed pointer and cycle rejection.
- Standalone database inspector and synthetic parser regression tests.
- Native ScummVM integration that reports database class counts honestly.
- GitHub Actions parser tests and compilation against a pinned ScummVM revision.

## Local edition examined

The supplied directory is laid out like installation media. Its databases are
under `MININST`, rather than at the top level. Add **`D:\The X-Files\MININST`**
to an experimental ScummVM build. Detection also searches MININST/MEDINST from
the parent directory. Language has deliberately been left unknown until verified.

| File | Bytes | MD5 of first 5000 bytes |
| --- | ---: | --- |
| XFILES.HDB | 6042472 | a87bbe0f3b51856303386db083c0321b |
| XFILES.GAM | 154656 | 1de1e4d2a05c5185605232caee341661 |

## Build the research tools

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
ctest --test-dir build --output-on-failure
```

Run `build/xfiles-inspect /path/to/XFILES.HDB` (Windows: `build\xfiles-inspect.exe`).
The tool prints class counts and asset references. It never modifies game files.

## Install the native engine

Use Python 3, Git, and the normal ScummVM build dependencies:

```sh
git clone https://github.com/scummvm/scummvm.git upstream
git -C upstream checkout 55c2973c1df5e8790b4248ae36a6b1b37842ce12
python tools/install_engine.py upstream
cd upstream
./configure --disable-all-engines --enable-engine=xfiles
make -j2
```

The engine is disabled by default. It shows a diagnostic dialog, not a title menu.
The installer refuses to replace a differing engine or use an unreviewed revision.
The portable parser currently uses C++ standard containers; migration to ScummVM's
Common containers is required before proposing upstream inclusion.

## Remaining work

1. Validate every database class and persistent list against the local edition.
2. Implement view hierarchy, navigation, action bytecode and trigger scheduling.
3. Integrate QuickTime video and audio, packed asset storage and asset resolution.
4. Implement hotspots, conversations, inventory, PDA and game-specific callbacks.
5. Implement save/load and verify complete playthroughs, languages and disc changes.

See [STATUS.md](STATUS.md) and [THIRD_PARTY.md](THIRD_PARTY.md).
No original game data, executable or decompiled game code is distributed.
