# Verify the global hotkey (Ctrl+Alt+Space) reaches the helper and is exposed via /poll.
$exe = "$PSScriptRoot\..\helper\target\debug\vaani.exe"
$base = 'http://127.0.0.1:17653'

Add-Type @"
using System;using System.Runtime.InteropServices;
public class K {
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
}
"@
$VK_CONTROL = 0x11; $VK_MENU = 0x12; $VK_SPACE = 0x20; $KEYUP = 0x2

$proc = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden
$ok = $false
for ($i = 0; $i -lt 30; $i++) { try { Invoke-RestMethod "$base/health" -TimeoutSec 1 | Out-Null; $ok = $true; break } catch { Start-Sleep -Milliseconds 400 } }
Write-Output "HEALTH_OK=$ok"

# Drain any pending command
try { Invoke-RestMethod "$base/poll" -TimeoutSec 1 | Out-Null } catch {}

# Inject Ctrl+Alt+Space (global hotkey is grabbed system-wide by the helper)
[K]::keybd_event($VK_CONTROL, 0, 0, [UIntPtr]::Zero)
[K]::keybd_event($VK_MENU, 0, 0, [UIntPtr]::Zero)
[K]::keybd_event($VK_SPACE, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 60
[K]::keybd_event($VK_SPACE, 0, $KEYUP, [UIntPtr]::Zero)
[K]::keybd_event($VK_MENU, 0, $KEYUP, [UIntPtr]::Zero)
[K]::keybd_event($VK_CONTROL, 0, $KEYUP, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 400

$action = $null
try { $p = Invoke-RestMethod "$base/poll" -TimeoutSec 2; $action = $p.action } catch {}
Write-Output "POLL_ACTION=$action (expect: toggle)"

# Process still alive (message loop running)?
$alive = -not $proc.HasExited
Write-Output "HELPER_ALIVE=$alive"

try { Stop-Process -Id $proc.Id -Force } catch {}
Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" |
  Where-Object { $_.CommandLine -like '*Vaani\chrome-profile*' } |
  ForEach-Object { try { Stop-Process -Id $_.ProcessId -Force } catch {} }
Write-Output "DONE"
