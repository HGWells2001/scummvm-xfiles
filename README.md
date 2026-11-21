# The X-Files Game (1998) for ScummVM

## Windows: avvio dall'interfaccia di ScummVM

[Scarica la build Windows](https://github.com/HGWells2001/scummvm-xfiles/actions/runs/44956045277/artifacts/14727631187).

1. Estrai tutto il nuovo pacchetto Windows in una cartella per ScummVM.
2. Usa la cartella del gioco, per esempio **D:\Giochi scumm\XFiles** oppure **D:\Giochi scumm\XFiles(Ita)**.
3. Apri scummvm.exe di questa build, scegli **Aggiungi gioco** e seleziona la cartella principale desiderata.
4. Conferma e premi **Gioca**.

Premi **F3** per mostrare o nascondere i sottotitoli presenti nei dati del gioco.

Durante il gioco, **F1** mostra o nasconde le aree cliccabili, inclusi i bersagli
attivi dei filmati. Quando i riquadri sono visibili, la console registra il punto
del clic, la scena e l'eventuale reazione del bersaglio.

Per altre copie dei CD, unisci il contenuto dei sette dischi mantenendo le
sottocartelle MININST, MEDINST, XV, XN, XS, XG, XT e i file NAV*.NMV.
I file dell'installazione come LAYOUT.BIN vengono conservati e non vengono
scambiati per immagini CD.

Questa build legge direttamente sia il layout dei CD sia quello installato.
Il decoder audio necessario e incluso: la prima preparazione avviene nel motore,
senza Node.js, ffprobe, installazioni aggiuntive o avviatori CMD/PowerShell.
Agli avvii successivi un indice dei file e della cache evita di riaprire le
intestazioni di tutti i filmati. Se cambiano dimensioni o date dei media o della
cache, il motore ripete la verifica completa; l'audio conserva anche il controllo
del contenuto quando viene riprodotto. Il primo avvio dopo questo aggiornamento
crea l'indice. La finestra del gioco viene inizializzata al termine del caricamento,
senza la schermata nera con "Preparazione del gioco...".
Premi **F5** per aprire il menu di ScummVM e usare i suoi slot. Puoi salvare
quando la scena e stabile e caricare anche dal launcher. I salvataggi precedenti
restano accessibili dal menu originale di The X-Files.

Su Windows il motore imposta il renderer software prima di inizializzare il video.
Il gioco usa direttamente i file completi della cartella selezionata. Il motore
non cerca ISO, non esegue scansioni per il ripristino e non sostituisce i file
del gioco. Il controllo diagnostico xfiles-folder-check.exe CARTELLA verifica
soltanto il caricamento dei database ed e di sola lettura.

La versione italiana viene identificata dai suoi XFILES.HDB e XFILES.GAM come
**Italiano / Windows**, con un target separato dall'altra edizione. La copia
italiana esaminata contiene gia i dati completi. Audio, testi e filmati restano
quelli dei suoi file originali.

## Game over

I filmati con bersagli temporizzati seguono le reazioni previste dalla scena.
Durante gli altri filmati uno sparo può attivare l'esito previsto dal luogo e
dal punto della storia. Il filmato dell'arresto torna al menu originale.
A Gordon’s Hauling il manifesto avvia il conto alla rovescia di 120 secondi
previsto dagli script del gioco. Un clic sinistro sulla schermata della nota
esegue la sua azione di fine partita. Quando compare «Try again?», la scelta
per riprovare usa il checkpoint della scena e l'altra torna al menu originale.

## Stato e dettagli per sviluppatori

**Experimental integration. A complete playthrough has not been validated.**

The **in-process Shiloh runtime** is under integration and validation.
It links the pinned Rust engine core into ScummVM and routes input, RGB frames
and mixed PCM through ScummVM. It does not start Shiloh's SDL program or the
original executable. This backend has its own CI workflow and remains experimental;
successful compilation alone is not a verified complete playthrough.

The separate C++ research frontend remains available. It reads databases and plays
media but does not execute a game. A runtime build can select it with
`xfiles_research=true` in the game target.
The Rust dependency is pinned to `e3dfde0d098b25135a5243839d44842ccbc5fe91`.
Rust is pinned to 1.100.0 and the resolved crates are recorded in `bridge/Cargo.lock`.
ScummVM slots store the Shiloh game state with standard ScummVM metadata.
The original game's slots and preferences are in the `xfiles-shiloh` subdirectory.

## Older Windows builds and optional preparation scripts

For the older Windows artifact below, extract the entire ZIP and start `Avvia-XFiles.cmd`.
The [runtime package with software video fallback and GUI resources](https://github.com/HGWells2001/scummvm-xfiles/actions/runs/44796666340/artifacts/14659311690)
passed the Windows checks at revision `1fe502c4eb65895e600687106f8291bf653eabd0`.
The Italian launcher uses existing prepared data when available; otherwise it
recovers the data forks from the seven local ISOs. Node.js 24 is needed for
preparation only. It keeps its ScummVM settings separate from other installations.
Paths can be supplied to `Prepara-e-Avvia.ps1` with `-GameDirectory`,
`-IsoDirectory`, `-PreparedDirectory` and `-SaveDirectory`. `-PrepareOnly` prepares
the data without starting ScummVM. No execution-policy setting is changed.
Default paths are resolved after parameter binding, including when the package
directory contains spaces. The Windows CI invokes the launcher under Windows
PowerShell 5.1 with both default and explicit prepared-data directories.
If Windows reports a missing runtime DLL, the launcher asks you to extract the
entire Windows artifact. An unrelated ScummVM executable is not a substitute for
the experimental runtime package.
`-SmokeTest` opens the prepared databases and renders the initial screen through
SDL's dummy drivers, then exits. It is a startup check, not a playthrough or an
audio/visual quality check.

If the window stays black while audio plays, try `Avvia-XFiles-Video-Compatibile.cmd`.
It uses SDL's Windows software renderer in a window, with a separate ScummVM
configuration and `xfiles-video.log` beside the executable. It reuses prepared
data beside the package or in `D:\XFiles-DatiPreparati`; a data directory can also
be dropped onto the CMD file. This is a display compatibility fallback, not a
confirmed diagnosis of every black-screen report. Its settings are process-local.
Windows artifacts include ScummVM's fonts, default theme and translations.

Two build artifacts are intentionally distinct:

| Artifact | Behavior | Validation scope |
| --- | --- | --- |
| ScummVM-XFiles-Shiloh-experimental-Windows | Hosts Shiloh's scene, script, dialogue, inventory, PDA and save logic inside ScummVM; ScummVM supplies input, graphics and audio output | Bridge tests and build checks; gameplay and A/V validation still required |
| ScummVM-XFiles-Windows-preview | Native C++ database inspector and movie browser | Parser tests, original database inspection and compile checks; does not execute gameplay |

Neither artifact starts the original game executable. The original executable and
DLLs may be read as data by Shiloh to obtain game-specific tables and resources.

The native C++ research frontend implements:

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
under `MININST`, rather than at the top level. Add **`D:\The X-Files`**
to this experimental ScummVM build. Detection searches MININST/MEDINST from
the selected directory. Language has deliberately been left unknown until verified.

**Historical media extraction issue:** ISO9660 associated records (flag `0x04`) share
filenames with the real data forks on these hybrid discs. The initial folder had
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

## Legacy scripted QDMC audio preparation

The following helper is retained for older runtime packages. The new GUI build
prepares its own audio natively and ships a minimal FFmpeg QDMC decoder.

The New Game opening movie (XV/64421.XMV), music and several later movies use
QDesign Music (QDMC). The pinned Shiloh core consumes a verified PCM cache for
this codec. Runtime launchers now run tools/prepare_audio.mjs before starting.
Node.js 24 and an existing FFmpeg/ffprobe on PATH are required for this one-time
decode. XF_FFMPEG and XF_FFPROBE may name explicit executables. No decoder or
game file is downloaded by the helper.

The helper scans QuickTime headers, decodes only QDMC sound tracks with
-ignore_editlist 1, and writes PCM WAV plus source size/FNV64 metadata in
%LOCALAPPDATA%/ScummVM-XFiles/saves/audio-cache (or the launcher's selected saves
directory). The core applies the original movie edit list once. Original game
files are never replaced. Cache entries additionally carry a WAV SHA256 and
are reused only when both source identity and cached bytes match. Errors are
reported in preparazione-audio.log rather than being silently ignored.

The local edition has 15 QDMC files. On its first game scene, 120 core ticks
changed from 0 to 175096 nonzero audio samples after preparation; the exported
RGB frame stayed byte-identical. This verifies decoded output, not a complete
playthrough or the subjective quality of every sound.

For an existing Shiloh runtime, download the [small audio patch](https://github.com/HGWells2001/scummvm-xfiles/actions/runs/44796666340/artifacts/14658339270) and extract it into that runtime folder, preserving tools/. Then start Avvia-XFiles-Video-Compatibile.cmd. The first launch prepares the QDMC audio with the installed FFmpeg; later launches reuse the verified cache.
