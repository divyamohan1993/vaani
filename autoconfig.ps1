# Vaani one-step setup for Windows 10/11 — idempotent, transparent, zero-config.
# Double-click autoconfig.bat (which calls this). It will, narrating each step:
#   1. ensure Google Chrome (winget, else direct download),
#   2. fetch the Vaani binary (local, or the latest GitHub Release),
#   3. install to %LOCALAPPDATA%, enable auto-start + shortcuts, and launch.
# Nothing here needs admin rights except (rarely) installing Chrome, where
# Windows shows its own prompt. Re-run any time; it refreshes everything.

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12  # GitHub/Google need TLS 1.2

$Repo       = 'divyamohan1993/vaani'
$ZipName    = 'vaani-windows.zip'
$root       = Split-Path -Parent $MyInvocation.MyCommand.Path
$installDir = Join-Path $env:LOCALAPPDATA 'Vaani'
$target     = Join-Path $installDir 'vaani.exe'

function Say  ($m) { Write-Host "  $m" -ForegroundColor Gray }
function Step ($m) { Write-Host "`n> $m" -ForegroundColor Cyan }
function Ok   ($m) { Write-Host "  OK $m" -ForegroundColor Green }
function Warn ($m) { Write-Host "  ! $m" -ForegroundColor Yellow }

Write-Host ""
Write-Host "  Vaani - speak anywhere, type everywhere" -ForegroundColor White
Write-Host "  Setting things up. This is safe to leave running." -ForegroundColor DarkGray

function Find-Chrome {
  foreach ($p in @(
      "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
      "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe",
      "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe")) {
    if (Test-Path $p) { return $p }
  }
  return $null
}

# ---- 1. Google Chrome (the speech engine) --------------------------------
Step "Checking for Google Chrome (the voice engine)..."
if (Find-Chrome) {
  Ok "Chrome is installed."
} else {
  Warn "Chrome not found. Installing it now - Windows may ask you to approve."
  $installed = $false
  if (Get-Command winget -ErrorAction SilentlyContinue) {
    Say "Using winget..."
    try {
      winget install -e --id Google.Chrome --silent --accept-source-agreements --accept-package-agreements | Out-Null
      if (Find-Chrome) { $installed = $true }
    } catch { Warn "winget route failed, trying a direct download..." }
  }
  if (-not $installed) {
    Say "Downloading Chrome from google.com..."
    $ci = Join-Path $env:TEMP 'chrome_installer.exe'
    Invoke-WebRequest 'https://dl.google.com/chrome/install/latest/chrome_installer.exe' -OutFile $ci -UseBasicParsing
    Say "Running the Chrome installer..."
    Start-Process -FilePath $ci -ArgumentList '/silent', '/install' -Wait
  }
  if (Find-Chrome) { Ok "Chrome installed." }
  else { Warn "Could not install Chrome automatically. Please install it from https://www.google.com/chrome and re-run this." ; Read-Host "Press Enter to exit"; exit 1 }
}

# ---- 2. The Vaani binary -------------------------------------------------
Step "Locating the Vaani app..."
$src = $null
foreach ($c in @((Join-Path $root 'vaani.exe'), (Join-Path $root 'helper\target\release\vaani.exe'))) {
  if (Test-Path $c) { $src = $c; break }
}
if (-not $src) {
  Say "Not bundled here - downloading the latest release..."
  $zip = Join-Path $env:TEMP $ZipName
  $url = "https://github.com/$Repo/releases/latest/download/$ZipName"
  try {
    Invoke-WebRequest $url -OutFile $zip -UseBasicParsing
    $ex = Join-Path $env:TEMP 'vaani-dl'
    if (Test-Path $ex) { Remove-Item $ex -Recurse -Force }
    Expand-Archive -Path $zip -DestinationPath $ex -Force
    $found = Get-ChildItem $ex -Recurse -Filter 'vaani.exe' | Select-Object -First 1
    if ($found) { $src = $found.FullName }
  } catch { Warn "Download failed: $($_.Exception.Message)" }
}
if (-not $src -or -not (Test-Path $src)) {
  Warn "Could not find or download the Vaani binary."
  Say  "Get it from https://github.com/$Repo/releases/latest and run this again."
  Read-Host "Press Enter to exit"; exit 1
}
Ok "App ready."

# ---- 3. Install, auto-start, shortcuts, launch ---------------------------
Step "Installing Vaani..."
Get-Process vaani -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 300
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Copy-Item $src $target -Force
Ok "Installed to $installDir"

Step "Enabling start-on-login + shortcuts..."
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'Vaani' -Value ('"' + $target + '"')
$ws = New-Object -ComObject WScript.Shell
foreach ($lnk in @(
    (Join-Path ([Environment]::GetFolderPath('Programs')) 'Vaani.lnk'),
    (Join-Path ([Environment]::GetFolderPath('Desktop'))  'Vaani.lnk'))) {
  $sc = $ws.CreateShortcut($lnk)
  $sc.TargetPath = $target; $sc.WorkingDirectory = $installDir
  $sc.Description = 'Vaani - speak anywhere, type everywhere'; $sc.Save()
}
Ok "Auto-start + Start Menu/Desktop shortcuts added."

Step "Starting Vaani..."
Start-Process -FilePath $target
Start-Sleep -Seconds 1

Write-Host ""
Write-Host "  All set. A small microphone appears bottom-right." -ForegroundColor Green
Write-Host "  - Click it, or press Ctrl+Alt+Space, then speak (English or Hindi)." -ForegroundColor Gray
Write-Host "  - Right-click it for language, transparency, and quit." -ForegroundColor Gray
Write-Host "  - First time: if Chrome asks to use the microphone, click Allow." -ForegroundColor Gray
Write-Host ""
