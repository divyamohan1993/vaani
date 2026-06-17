# FULL automated end-to-end speech test.
#   TTS wav -> Chrome fake mic -> webkitSpeechRecognition -> Google -> page
#   -> POST /type. Verifies via the debug log (transcript) AND a foreground textbox.
$exe = "$env:LOCALAPPDATA\Vaani\vaani.exe"
$wav = "$env:TEMP\vaani-speech-test.wav"
$phrase = 'this is a test of voice typing'

# 1. Synthesize a ~15s wav (phrase repeated) so audio is present whenever recognition starts.
Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$fmt = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)
$synth.SetOutputToWaveFile($wav, $fmt)
1..5 | ForEach-Object { $synth.Speak($phrase); $synth.Speak('.') }
$synth.SetOutputToNull(); $synth.Dispose()
"WAV_BYTES=$((Get-Item $wav).Length)"

# 2. Stop any running instance (so we can launch one wired to the fake mic).
Get-Process vaani -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.CommandLine -like '*Vaani\chrome-profile*' } | ForEach-Object { try { Stop-Process -Id $_.ProcessId -Force } catch {} }
Start-Sleep -Milliseconds 800
Remove-Item "$env:LOCALAPPDATA\Vaani\vaani.log" -ErrorAction SilentlyContinue

# 3. Launch helper wired to the fake mic (unquoted path; auto-grant mic).
$env:VAANI_DEBUG = '1'
$env:VAANI_CHROME_ARGS = "--use-fake-ui-for-media-stream --use-fake-device-for-media-stream --use-file-for-fake-audio-capture=$wav"
$proc = Start-Process -FilePath $exe -PassThru
$env:VAANI_CHROME_ARGS = $null

for ($i = 0; $i -lt 30; $i++) { try { Invoke-RestMethod 'http://127.0.0.1:17653/health' -TimeoutSec 1 | Out-Null; break } catch { Start-Sleep -Milliseconds 400 } }
Start-Sleep -Seconds 3   # page load + polling

# 4. Foreground textbox + start listening via global hotkey.
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;using System.Runtime.InteropServices;
public class K { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
"@
$form = New-Object Windows.Forms.Form
$form.Text = 'Speech Target'; $form.Width = 640; $form.Height = 240; $form.TopMost = $true; $form.StartPosition = 'CenterScreen'
$tb = New-Object Windows.Forms.TextBox; $tb.Multiline = $true; $tb.Dock = 'Fill'; $tb.Font = New-Object Drawing.Font('Segoe UI', 16)
$form.Controls.Add($tb)
$script:res = $null; $script:t = 0
$timer = New-Object Windows.Forms.Timer; $timer.Interval = 1000
$timer.Add_Tick({
  $script:t++
  if ($script:t -eq 1) {
    $form.Activate(); $tb.Focus() | Out-Null
    [K]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [K]::keybd_event(0x12,0,0,[UIntPtr]::Zero); [K]::keybd_event(0x20,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [K]::keybd_event(0x20,0,2,[UIntPtr]::Zero); [K]::keybd_event(0x12,0,2,[UIntPtr]::Zero); [K]::keybd_event(0x11,0,2,[UIntPtr]::Zero)
    $tb.Focus() | Out-Null
  }
  if ($script:t -ge 14) { $script:res = $tb.Text; $timer.Stop(); $form.Close() }
})
$form.Add_Shown({ $form.Activate(); $tb.Focus() | Out-Null; $timer.Start() })
$form.ShowDialog() | Out-Null

Set-Content "$env:TEMP\vaani-speech-result.txt" -Value $script:res -Encoding utf8
"TEXTBOX_LEN=$($script:res.Length)"
"---- transcript from helper log (/type) ----"
Get-Content "$env:LOCALAPPDATA\Vaani\vaani.log" | Select-String 'PAGE: (onstart|error)|/type'

$env:VAANI_DEBUG = $null
try { Stop-Process -Id $proc.Id -Force } catch {}
Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.CommandLine -like '*Vaani\chrome-profile*' } | ForEach-Object { try { Stop-Process -Id $_.ProcessId -Force } catch {} }
"DONE"
