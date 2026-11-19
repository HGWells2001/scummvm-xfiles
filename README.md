# The X-Files Game (1998) - ScummVM research engine

**Experimental first milestone. The full game is NOT playable.**

This repository contains a native ScummVM detector, a NeoAccess database reader
and a resource browser with QuickTime playback through ScummVM's own decoder.
It does not run the original executable or yet interpret the game's scripts.

Implemented:

- Exact detection of the local database pair, using ScummVM's first-5000-byte MD5 convention.
- Bounded big-endian NeoAccess 5.01 class-tree and object-index reader.
- Asset-reference parsing with variants, movie time ranges and safe relative paths.
- Persistent ID lists, malformed pointer and cycle rejection.
- All 14,247 local VCAction records structurally decoded, including conditions and named procedure calls.
- Standalone database inspector and synthetic parser regression tests.
- Native ScummVM resource browser, movie playback and the four opening movies.
- Color conversion, original frame dimensions, audio via ScummVM, pause and skip.
- Windows preview compilation and downloadable artifact through GitHub Actions.
- GitHub Actions parser tests and compilation against a pinned ScummVM revision.

## Local edition examined

The supplied directory is laid out like installation media. Its databases are
under `MININST`, rather than at the top level. Add **`D:\The X-Files\MININST`**
to an experimental ScummVM build. Detection also searches MININST/MEDINST from
the parent directory. Language has deliberately been left unknown until verified.

**Media extraction issue found:** ISO9660 associated records (flag `0x04`) share
filenames with the real data forks on these hybrid discs. The supplied folder has
many 286-byte resource forks instead of playable media. For example, the real
`NAV1.NMV` on CD1 is 6,143,233 bytes; `X.PFF` is 44,107,692 bytes.

Recover a separate installed layout from your own seven ISOs with Node.js 24:

```text
node tools/prepare_data.mjs C:\XFiles-Prepared D:\The_X-Files_-_The_Game_CD1.iso D:\The_X-Files_-_The_Game_CD2.iso D:\The_X-Files_-_The_Game_CD3.iso D:\The_X-Files_-_The_Game_CD4.iso D:\The_X-Files_-_The_Game_CD5.iso D:\The_X-Files_-_The_Game_CD6.iso D:\The_X-Files_-_The_Game_CD7.iso
```

The extractor skips associated forks, merges installation directories, rejects
unsafe paths and differing files, and resumes only verified matching outputs.
QuickTime duplicates differing only in creation/modification timestamps and unused
`free` bytes are accepted after comparing every other atom byte, including media
payload and sample tables. Original images and existing game directories are not
modified. A SHA-256 manifest is written beside the recovered data.

Add the prepared directory to the experimental ScummVM build. To run a local
headless startup check with SDL's dummy driver, use `--boot-param=-1` with the
`xfiles:xfiles` target; this checks the research browser, not gameplay.

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

The engine is disabled by default. It opens a research resource browser.
Arrow keys select a movie; PageUp/PageDown move a page; Enter plays it.
`I` plays the four introductory movies resolved from the database. In video,
Space pauses, a mouse button skips the current clip, and Esc returns to the browser.
No savegame is created and no original game file is changed.
The installer refuses to replace a differing engine or use an unreviewed revision.
The portable parser currently uses C++ standard containers; migration to ScummVM's
Common containers is required before proposing upstream inclusion.

## Remaining work

1. Validate every database class and persistent list against the local edition.
2. Implement view hierarchy, navigation, action bytecode and trigger scheduling.
3. Implement packed asset storage and the remaining media variants.
4. Implement hotspots, conversations, inventory, PDA and game-specific callbacks.
5. Implement save/load and verify complete playthroughs, languages and disc changes.

See [STATUS.md](STATUS.md) and [THIRD_PARTY.md](THIRD_PARTY.md).
No original game data, executable or decompiled game code is distributed.
