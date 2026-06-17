# Verify the helper makes the real Chrome window always-on-top + adjusts opacity.
# Reads the live ex-style flags + layered alpha off Chrome's HWND (cross-process).
$exe = "$PSScriptRoot\..\helper\target\debug\vaani.exe"

Add-Type @"
using System;using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h,int i);
  [DllImport("user32.dll")] public static extern bool GetLayeredWindowAttributes(IntPtr h,out uint key,out byte alpha,out uint flags);
}
"@

$proc = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden

function Get-VaaniChromeHwnd {
  $p = Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" |
    Where-Object { $_.CommandLine -like '*Vaani\chrome-profile*' -and $_.CommandLine -like '*--app=*' } |
    Select-Object -First 1
  if (-not $p) { return [IntPtr]::Zero }
  $h = (Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue).MainWindowHandle
  if ($null -eq $h) { return [IntPtr]::Zero }
  return $h
}

$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 40; $i++) {
  $hwnd = Get-VaaniChromeHwnd
  if ($hwnd -ne [IntPtr]::Zero) { break }
  Start-Sleep -Milliseconds 300
}
Write-Output "CHROME_HWND_FOUND=$($hwnd -ne [IntPtr]::Zero)"

if ($hwnd -ne [IntPtr]::Zero) {
  Start-Sleep -Milliseconds 600   # let the helper apply initial style
  $GWL_EXSTYLE = -20
  $ex = [W]::GetWindowLong($hwnd, $GWL_EXSTYLE)
  $topmost = ($ex -band 0x8) -ne 0
  $layered = ($ex -band 0x80000) -ne 0
  Write-Output "INITIAL_TOPMOST=$topmost LAYERED=$layered"

  # Drop opacity to 50%
  try { Invoke-RestMethod 'http://127.0.0.1:17653/window' -Method Post -ContentType 'application/json' -Body '{"alpha":50,"topmost":true}' -TimeoutSec 2 | Out-Null } catch {}
  Start-Sleep -Milliseconds 400
  $key = 0; $alpha = 0; $flags = 0
  $okAttr = [W]::GetLayeredWindowAttributes($hwnd, [ref]$key, [ref]$alpha, [ref]$flags)
  Write-Output "AFTER_50PCT alpha_byte=$alpha (expect ~127) read_ok=$okAttr"
}

# Cleanup (only our profile chrome)
try { Stop-Process -Id $proc.Id -Force } catch {}
Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" |
  Where-Object { $_.CommandLine -like '*Vaani\chrome-profile*' } |
  ForEach-Object { try { Stop-Process -Id $_.ProcessId -Force } catch {} }
Write-Output "DONE"
