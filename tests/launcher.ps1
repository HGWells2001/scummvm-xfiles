[CmdletBinding()]
param([string]$Launcher)
$ErrorActionPreference = 'Stop'
if (!$Launcher) { $Launcher = Join-Path $PSScriptRoot '..\Prepara-e-Avvia.ps1' }
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('XFiles launcher ' + [Guid]::NewGuid())
try {
    $package = Join-Path $testRoot 'package with spaces'
    $prepared = Join-Path $testRoot 'XFiles-DatiPreparati'
    New-Item -ItemType Directory -Path $package, $prepared | Out-Null
    $script = Join-Path $package 'Prepara-e-Avvia.ps1'
    Copy-Item -LiteralPath $Launcher -Destination $script
    # Only the launcher's selection logic is under test; no copyrighted fixtures.
    [IO.File]::WriteAllBytes((Join-Path $prepared 'XFILES.HDB'), [byte[]]@(0))
    [IO.File]::WriteAllBytes((Join-Path $prepared 'XFILES.GAM'), [byte[]]@(0))
    [IO.File]::WriteAllBytes((Join-Path $prepared 'X.PFF'), [Text.Encoding]::ASCII.GetBytes('PFF '))
    $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $missing = Join-Path $testRoot 'absent original'
    foreach ($explicit in @($false, $true)) {
        $arguments = @('-NoProfile', '-File', $script, '-GameDirectory', $missing, '-PrepareOnly')
        if ($explicit) { $arguments += @('-PreparedDirectory', $prepared) }
        $result = & $powershell @arguments 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0 -or !$result.Contains($prepared)) {
            throw "Launcher failed (explicit=$explicit): $result"
        }
    }
    # Exercise the ordinary default-parameter path without starting a GUI.
    $ErrorActionPreference = 'Continue'
    $result = & $powershell -NoProfile -File $script -GameDirectory $prepared 2>&1 | Out-String
    $ErrorActionPreference = 'Stop'
    if ($LASTEXITCODE -ne 1 -or !$result.Contains('scummvm.exe')) {
        throw "Missing executable was not reported correctly: $result"
    }
    Write-Host 'PASS: Windows PowerShell defaults, prepared-data fallback, spaces and missing executable'
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        $resolved = (Resolve-Path -LiteralPath $testRoot).Path
        $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (!$resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Refusing cleanup outside the temporary directory'
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
