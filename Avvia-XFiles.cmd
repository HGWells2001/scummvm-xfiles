@echo off
powershell.exe -NoLogo -NoProfile -File "%~dp0Prepara-e-Avvia.ps1" %*
if errorlevel 1 pause
