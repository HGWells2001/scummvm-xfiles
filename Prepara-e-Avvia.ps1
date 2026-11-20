[CmdletBinding()]
param(
    [string]$GameDirectory = 'D:\The X-Files',
    [string]$IsoDirectory = 'D:\',
    [string]$PreparedDirectory = (Join-Path (Split-Path $PSScriptRoot -Parent) 'XFiles-DatiPreparati'),
    [string]$SaveDirectory = (Join-Path $env:LOCALAPPDATA 'ScummVM-XFiles\saves'),
    [switch]$PrepareOnly,
    [switch]$SmokeTest
)
$ErrorActionPreference = 'Stop'

function Test-InstalledData([string]$Directory) {
    if (!(Test-Path -LiteralPath (Join-Path $Directory 'XFILES.HDB') -PathType Leaf)) { return $false }
    if (!(Test-Path -LiteralPath (Join-Path $Directory 'XFILES.GAM') -PathType Leaf)) { return $false }
    $picture = Join-Path $Directory 'X.PFF'
    if (!(Test-Path -LiteralPath $picture -PathType Leaf)) { return $false }
    $stream = [IO.File]::OpenRead($picture)
    try {
        $header = New-Object byte[] 4
        if ($stream.Read($header, 0, 4) -ne 4) { return $false }
        return [Text.Encoding]::ASCII.GetString($header) -eq 'PFF '
    } finally { $stream.Dispose() }
}

try {
    $executable = Join-Path $PSScriptRoot 'scummvm.exe'
    if (!$PrepareOnly -and !(Test-Path -LiteralPath $executable -PathType Leaf)) {
        throw 'Estrai prima la build Windows completa: scummvm.exe deve essere accanto a questo script.'
    }
    if (Test-InstalledData $GameDirectory) {
        $data = (Resolve-Path -LiteralPath $GameDirectory).Path
    } elseif (Test-InstalledData $PreparedDirectory) {
        $data = (Resolve-Path -LiteralPath $PreparedDirectory).Path
    } else {
        $node = Get-Command node.exe -ErrorAction SilentlyContinue
        if (!$node) { throw 'Per preparare i CD serve Node.js 24. Installa Node.js dal suo sito ufficiale, poi riprova.' }
        $extractor = Join-Path $PSScriptRoot 'tools\prepare_data.mjs'
        if (!(Test-Path -LiteralPath $extractor -PathType Leaf)) { throw 'Pacchetto incompleto: manca tools\prepare_data.mjs.' }
        $images = @()
        foreach ($disc in 1..7) {
            $image = Join-Path $IsoDirectory "The_X-Files_-_The_Game_CD$disc.iso"
            if (!(Test-Path -LiteralPath $image -PathType Leaf)) {
                $candidates = @(Get-ChildItem -LiteralPath $IsoDirectory -File -Filter "*X*Files*CD$disc*.iso")
                if ($candidates.Count -ne 1) { throw "Non trovo un unico ISO per il CD $disc in $IsoDirectory." }
                $image = $candidates[0].FullName
            }
            $images += $image
        }
        Write-Host 'Preparo una copia corretta dei sette CD. I file originali non vengono modificati.'
        $log = Join-Path $PSScriptRoot 'preparazione-dati.log'
        & $node.Source $extractor $PreparedDirectory @images *> $log
        if ($LASTEXITCODE -ne 0) { throw "Preparazione interrotta. Dettagli in $log" }
        if (!(Test-InstalledData $PreparedDirectory)) { throw 'La preparazione non ha prodotto i database e le immagini attesi.' }
        $data = (Resolve-Path -LiteralPath $PreparedDirectory).Path
    }
    if ($PrepareOnly) { Write-Host "Dati pronti in: $data"; exit 0 }
    New-Item -ItemType Directory -Path $SaveDirectory -Force | Out-Null
    $config = Join-Path $PSScriptRoot 'scummvm-xfiles.ini'
    Write-Host "Avvio ScummVM con i dati in: $data"
    $arguments = @("--config=$config", "--path=$data", "--savepath=$SaveDirectory")
    if ($SmokeTest) {
        $env:SDL_VIDEODRIVER = 'dummy'
        $env:SDL_AUDIODRIVER = 'dummy'
        $arguments += '--boot-param=-1'
    }
    & $executable @arguments 'xfiles:xfiles'
    exit $LASTEXITCODE
} catch {
    Write-Error $_.Exception.Message -ErrorAction Continue
    exit 1
}
