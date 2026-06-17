<div align="center">

# 🎙️ Vaani

### Speak anywhere. Type everywhere.

**Press one key, talk, and your words appear in whatever app you're using — VS&nbsp;Code, Notepad, Word, your browser. English or हिन्दी. No copy‑paste, no switching windows.**

<img src="docs/mic-idle.png" width="76" alt="Vaani idle"> &nbsp;&nbsp; <img src="docs/mic-live.png" width="76" alt="Vaani listening">

![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D6?logo=windows&logoColor=white)
![License: MIT](https://img.shields.io/badge/license-MIT-green)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange?logo=rust&logoColor=white)
![Voice: English | हिन्दी](https://img.shields.io/badge/voice-English%20%7C%20%E0%A4%B9%E0%A4%BF%E0%A4%A8%E0%A5%8D%E0%A4%A6%E0%A5%80-8A2BE2)
![Size: ~530 KB](https://img.shields.io/badge/helper-~530%20KB-blue)

### [⬇️ Download for Windows](https://github.com/divyamohan1993/vaani/releases/latest) · [How it works](#-how-it-works) · [Privacy](#-privacy)

</div>

---

A tiny microphone floats in the corner of your screen. Click it (or press **Ctrl + Alt + Space**), speak, and the text is **typed straight into the app you're already in** — exactly where your cursor is, like a keyboard. It uses Google Chrome's speech engine, the most accurate free dictation there is, and works in **English and Hindi**.

## ✨ Why people like it

- **🎯 It types where your cursor is.** Not into a box you copy from — into your real editor. Hindi (Devanagari) included.
- **⌨️ One shortcut, from anywhere.** `Ctrl + Alt + Space` starts/stops without leaving your work.
- **🫧 Just a mic.** A small, borderless, always‑on‑top dot. Drag it anywhere; fade it; it turns red while listening. No window, no clutter.
- **🪶 Featherweight.** A ~530 KB native helper, a few MB of RAM. No Electron. Chrome runs hidden, only as the engine.
- **🔒 Private by default.** The local helper listens only on `127.0.0.1` and logs nothing about what you say.

## 🚀 Install in 30 seconds

> Works on a fresh **Windows 10 or 11** — it sets up everything for you.

1. **[Download `vaani-windows.zip`](https://github.com/divyamohan1993/vaani/releases/latest)** and unzip it.
2. **Double‑click `autoconfig.bat`.**

That's it. The setup narrates each step and, if Google Chrome isn't already installed, **downloads and installs it for you**. Then a microphone appears in the bottom‑right and starts on every login.

<details>
<summary>Prefer one line in PowerShell?</summary>

```powershell
irm https://raw.githubusercontent.com/divyamohan1993/vaani/main/install.ps1 | iex
```
Downloads the latest release and runs the same setup.
</details>

> The download is an unsigned app, so Windows SmartScreen may say *"Windows protected your PC."* Click **More info → Run anyway** (open‑source, no code‑signing certificate — the source is right here to inspect or build yourself).

## 🗣️ Use it

| Do this | What happens |
|---|---|
| Press **Ctrl + Alt + Space** (or click the dot) | The dot turns red — it's listening |
| Speak — English or हिन्दी | Your words type into the focused app |
| Press again | It stops |
| **Right‑click** the dot | Language · Transparency · Reset position · Quit |
| **Drag** the dot | Move it anywhere; it remembers |

> Pick the language (right‑click → **Language**) before you speak — like all speech engines, it can't read your mind. 🙂

## 🧠 How it works

Google's Web Speech API only works reliably inside **real Google Chrome** — it's disabled in Electron and silently does nothing in Edge/WebView2. So Vaani splits the job:

```
 your voice ─► Chrome (hidden, off‑screen)  ──►  Vaani helper (native, Rust)  ──►  types into
              webkitSpeechRecognition            127.0.0.1 + enigo SendInput        the focused app
              en‑IN / hi‑IN
```

- **The speech page** (`app/`) runs in a Chrome window kept hidden off‑screen, purely as the recognizer.
- **The Rust helper** (`helper/`) draws the native mic dot, runs a tiny loopback HTTP bridge, and types each finished phrase into whatever window is focused — using the live foreground window (never stealing focus), so words land exactly where your caret is.

Recognition happens in Chrome; the dot and the typing are 100% native. A browser tab can't type into other apps — this can.

## 🔒 Privacy

- Audio is transcribed by Chrome's speech service (Google), the same as dictation in Chrome itself. Nothing else sees it.
- The helper binds to **loopback only** and **never logs your transcript** (set `VAANI_DEBUG=1` for verbose local logs while troubleshooting).
- A dedicated Chrome profile under `%LOCALAPPDATA%\Vaani` keeps the microphone permission isolated from your everyday browser.

## 💻 Platforms

| Platform | Status |
|---|---|
| **Windows 10 & 11** | ✅ Supported |
| **macOS / Linux** | 🛣️ Roadmap — the recognizer (Chrome) and the typing layer (`enigo`) are already cross‑platform; the native overlay + hotkey need per‑OS code. **PRs welcome.** |

## 🔧 Build from source

```powershell
git clone https://github.com/divyamohan1993/vaani
cd vaani
powershell -ExecutionPolicy Bypass -File build.ps1   # auto-installs the Rust toolchain if missing
powershell -ExecutionPolicy Bypass -File autoconfig.ps1
```

Rust + the `gnu` toolchain (no Visual Studio needed). See [CONTRIBUTING.md](CONTRIBUTING.md).

## 🙋 FAQ

**Does it cost anything?** No. It uses Chrome's built‑in (free) speech engine.
**Does it need internet?** Yes — Chrome's recognizer is cloud‑based.
**Will it type into games / admin windows?** Anywhere standard text input works. Apps running as administrator may need Vaani run as administrator too.
**Hinglish?** Set language to हिन्दी — it handles mixed Hindi‑English well.

## 📄 License

[MIT](LICENSE) © 2026 [Divya Mohan](https://github.com/divyamohan1993). Built in India 🇮🇳.

<div align="center"><sub>Made for fast, hands‑light typing in English and Hindi.</sub></div>
