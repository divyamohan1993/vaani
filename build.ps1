# Build Vaani from source. Installs the Rust (gnu) toolchain automatically if
# it is missing, reuses an existing mingw linker, and produces a release binary.
# Usage:  powershell -ExecutionPolicy Bypass -File build.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'

Write-Host '== Vaani build ==' -ForegroundColor Cyan

# 1. Rust toolchain (gnu host avoids the multi-GB Visual Studio Build Tools).
if (-not (Test-Path $cargo)) {
  Write-Host 'Installing Rust (x86_64-pc-windows-gnu)...'
  $tmp = Join-Path $env:TEMP 'rustup-init.exe'
  Invoke-WebRequest 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe' -OutFile $tmp -UseBasicParsing
  & $tmp -y --default-host x86_64-pc-windows-gnu --profile minimal | Out-Null
}

# 2. mingw linker (gcc/ld) for the gnu toolchain.
if (-not (Get-Command gcc -ErrorAction SilentlyContinue)) {
  Write-Host 'Installing mingw-w64 (WinLibs) via winget...'
  winget install -e --id BrechtSanders.WinLibs.POSIX.UCRT --accept-source-agreements --accept-package-agreements
  $env:Path = "$env:LOCALAPPDATA\Microsoft\WinGet\Links;" + $env:Path
}

$env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path

# 3. Build.
Write-Host 'Compiling (release)...'
& $cargo build --release --manifest-path (Join-Path $root 'helper\Cargo.toml') --bin vaani
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

Copy-Item (Join-Path $root 'helper\target\release\vaani.exe') (Join-Path $root 'vaani.exe') -Force
Write-Host ("Built: " + (Join-Path $root 'vaani.exe')) -ForegroundColor Green
