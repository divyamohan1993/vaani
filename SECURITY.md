# Security Policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

- Preferred: open a [private security advisory](https://github.com/divyamohan1993/vaani/security/advisories/new).
- Or email **divyamohan1993@gmail.com**.

You'll get a response as soon as possible. Thank you for helping keep users safe.

## How Vaani handles your data

- **Audio** is transcribed by Google Chrome's built‑in Web Speech service (the same engine as dictation in Chrome). Vaani itself never stores or transmits your audio.
- **The local helper** binds only to `127.0.0.1` (loopback) and is not reachable from the network.
- **Transcript text** is never written to disk or logs by default. Verbose logging (`VAANI_DEBUG=1`) is opt‑in and local‑only.
- **Microphone permission** lives in a dedicated Chrome profile under `%LOCALAPPDATA%\Vaani`, isolated from your everyday browser.

## What the helper can do (by design)

Vaani types into the focused application using OS keyboard simulation (`SendInput` via `enigo`) and registers a global hotkey. These are the same capabilities as any dictation or text‑expander tool. The full source is here for inspection, and you can build it yourself with `build.ps1`.

## Distribution note

Released binaries are **not code‑signed** (no certificate), so Windows SmartScreen may warn on first run. Verify the download by building from source if you prefer, or check the release was produced by this repository's GitHub Actions.
