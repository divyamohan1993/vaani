# Automated core test: serving + /health + /type injection into foreground + /window.
# Does NOT test speech (needs a real mic/voice — covered separately).
$ErrorActionPreference = 'Continue'
$exe = 'd:\todo\helper\target\debug\vaani.exe'
$base = 'http://127.0.0.1:17653'

$proc = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden
Start-Sleep -Milliseconds 600

# Wait for /health
$ver = $null; $ok = $false
for ($i = 0; $i -lt 30; $i++) {
  try { $h = Invoke-RestMethod "$base/health" -TimeoutSec 1; $ver = $h.version; $ok = $true; break }
  catch { Start-Sleep -Milliseconds 400 }
}
Write-Output "HEALTH_OK=$ok VER=$ver"

# Page served?
$page = $false
try { $idx = Invoke-WebRequest "$base/" -TimeoutSec 2 -UseBasicParsing; $page = ($idx.Content -match 'Vaani') } catch {}
Write-Output "PAGE_OK=$page"

# /type injection into a focused textbox
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$form = New-Object Windows.Forms.Form
$form.Text = 'Inject Target'; $form.Width = 560; $form.Height = 200
$form.TopMost = $true; $form.StartPosition = 'CenterScreen'
$tb = New-Object Windows.Forms.TextBox
$tb.Multiline = $true; $tb.Dock = 'Fill'; $tb.Font = New-Object Drawing.Font('Segoe UI', 16)
$form.Controls.Add($tb)
$script:res = $null; $script:t = 0
$timer = New-Object Windows.Forms.Timer; $timer.Interval = 800
$timer.Add_Tick({
  $script:t++
  if ($script:t -eq 1) {
    $form.Activate(); $form.BringToFront(); $tb.Focus() | Out-Null
    try {
      Invoke-RestMethod 'http://127.0.0.1:17653/type' -Method Post -ContentType 'application/json' `
        -Body '{"text":"vaani inject test 123 ","mode":"type"}' -TimeoutSec 2 | Out-Null
    } catch {}
  }
  if ($script:t -ge 4) { $script:res = $tb.Text; $timer.Stop(); $form.Close() }
})
$form.Add_Shown({ $form.Activate(); $tb.Focus() | Out-Null; $timer.Start() })
$form.ShowDialog() | Out-Null
Set-Content 'd:\todo\inject-result.txt' -Value $script:res -Encoding utf8
Write-Output "INJECT_MATCH=$([bool]($script:res -match 'vaani inject test 123')) LEN=$($script:res.Length)"

# /window smoke (must not error)
try {
  Invoke-RestMethod "$base/window" -Method Post -ContentType 'application/json' `
    -Body '{"alpha":70,"topmost":true}' -TimeoutSec 2 | Out-Null
  Write-Output "WINDOW_OK=true"
} catch { Write-Output "WINDOW_OK=false" }

# Cleanup: helper + ONLY our dedicated-profile Chrome (never the user's main Chrome)
try { Stop-Process -Id $proc.Id -Force } catch {}
Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" |
  Where-Object { $_.CommandLine -like '*Vaani\chrome-profile*' } |
  ForEach-Object { try { Stop-Process -Id $_.ProcessId -Force } catch {} }
Write-Output "DONE"
