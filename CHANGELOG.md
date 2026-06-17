# Changelog

All notable changes to Vaani are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versioning is [SemVer](https://semver.org/).

## [0.1.0] — 2026-06-17

### Added
- Native, borderless, always-on-top **microphone dot** drawn by the Rust helper (GDI + color-key transparency). Draggable with position memory; click to toggle; turns red while listening; adjustable opacity.
- Voice recognition via Google Chrome's Web Speech API, English (`en-IN`) and Hindi (`hi-IN`), with continuous auto-restart. Chrome runs hidden off-screen purely as the speech engine.
- Native text injection into the focused application (VS Code, Notepad, Word, browsers, …) using `enigo`; full Unicode incl. Devanagari. Types into the live foreground window (Win+H model), never into Vaani's own dot.
- Global hotkey `Ctrl + Alt + Space` to start/stop dictation from anywhere.
- Right-click menu (on the dot or the tray icon): Language English/हिन्दी, Opacity 100/75/50%, Reset position, Quit. System-tray icon; single-instance (re-launch re-homes the dot).
- Self-contained ~540 KB Rust helper: serves the embedded speech page + a loopback HTTP API (`/health`, `/type`, `/poll`, `/state`, `/show`, `/log`) with CORS + Private Network Access headers so `type.dmj.one` can reach it.
- Clipboard fallback when the helper isn't running.
- One-step installer `autoconfig.bat` (Chrome detection + winget install, install to `%LOCALAPPDATA%`, auto-start, Start-Menu + Desktop shortcuts) and `build.ps1` (auto-installs the Rust gnu toolchain).
- Auto-punctuation (sentence casing) for English; per-app privacy-first logging (off unless `VAANI_DEBUG`).

### Security / Privacy
- Helper binds to `127.0.0.1` only; transcript text is never logged by default.
- Dedicated Chrome profile isolates the microphone permission from the user's main browser.

### Notes
- Opacity uses `WS_EX_LAYERED` only below 100% to avoid GPU-compositor glitches; fully opaque windows stay on the standard render path.
- Verified on Windows 11: build, serving, injection (English + Hindi), always-on-top + transparency, global hotkey, tray, single-instance, and recognizer start in real Chrome. Real-voice transcription is Chrome's cloud engine and requires a microphone.
