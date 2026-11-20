@echo off
setlocal
rem Settings affect only this launch, not Windows or other ScummVM installations.
set "SDL_VIDEODRIVER=windows"
set "SDL_RENDER_DRIVER=software"
set "XF_GAME_DATA=%~dp0..\XFiles-DatiPreparati"
if not exist "%XF_GAME_DATA%\XFILES.HDB" if exist "D:\XFiles-DatiPreparati\XFILES.HDB" set "XF_GAME_DATA=D:\XFiles-DatiPreparati"
if not "%~1"=="" set "XF_GAME_DATA=%~1"
if not exist "%~dp0scummvm.exe" (
  echo Estrai questo avviatore nella cartella del runtime Shiloh, accanto a scummvm.exe.
  pause
  exit /b 1
)
if not exist "%XF_GAME_DATA%\XFILES.HDB" (
  echo Dati preparati non trovati. Usa prima Avvia-XFiles.cmd per prepararli.
  echo Puoi anche trascinare la cartella dei dati su questo avviatore.
  pause
  exit /b 1
)
if not exist "%LOCALAPPDATA%\ScummVM-XFiles\saves" mkdir "%LOCALAPPDATA%\ScummVM-XFiles\saves"
set "XF_CACHE=%LOCALAPPDATA%\ScummVM-XFiles\saves\audio-cache"
if exist "%~dp0tools\prepare_audio.mjs" (
  echo Controllo audio QDMC. La prima preparazione usa FFmpeg e puo richiedere qualche secondo.
  node.exe "%~dp0tools\prepare_audio.mjs" "%XF_GAME_DATA%" "%XF_CACHE%" >"%~dp0preparazione-audio.log" 2>&1
  if errorlevel 1 echo Audio QDMC non preparato: servono Node.js 24 e FFmpeg con ffprobe nel PATH. Vedi preparazione-audio.log.
)
echo Avvio video software in finestra. Dati: %XF_GAME_DATA%
echo Log: %~dp0xfiles-video.log
"%~dp0scummvm.exe" --config="%~dp0scummvm-xfiles-video.ini" --path="%XF_GAME_DATA%" --savepath="%LOCALAPPDATA%\ScummVM-XFiles\saves" --extrapath="%~dp0." --themepath="%~dp0." --logfile="%~dp0xfiles-video.log" --no-fullscreen --gfx-mode=surfacesdl xfiles:xfiles
if not "%errorlevel%"=="0" (
  echo Avvio terminato con errore. Consulta xfiles-video.log.
  pause
  exit /b 1
)
exit /b 0
