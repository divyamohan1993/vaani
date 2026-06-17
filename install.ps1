# Vaani one-line installer for Windows 10/11.
#   irm https://raw.githubusercontent.com/divyamohan1993/vaani/main/install.ps1 | iex
# Downloads the latest release and runs the (transparent) setup, which also
# installs Google Chrome automatically if it is missing.
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo = 'divyamohan1993/vaani'
$zip  = Join-Path $env:TEMP 'vaani-windows.zip'
$dir  = Join-Path $env:TEMP 'vaani-install'

Write-Host 'Downloading Vaani (latest release)...' -ForegroundColor Cyan
Invoke-WebRequest "https://github.com/$repo/releases/latest/download/vaani-windows.zip" -OutFile $zip -UseBasicParsing

if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
Expand-Archive -Path $zip -DestinationPath $dir -Force

$cfg = Get-ChildItem $dir -Recurse -Filter 'autoconfig.ps1' | Select-Object -First 1
if (-not $cfg) { throw 'autoconfig.ps1 not found in the release archive.' }
& $cfg.FullName
