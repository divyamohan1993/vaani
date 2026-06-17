# Contributing to Vaani

Thanks for helping! Vaani is small and friendly — issues, ideas, and pull requests are all welcome.

## Ways to help

- 🐛 **Report bugs** — use the issue template; include your Windows version and Chrome version.
- 💡 **Suggest features** — keep the mic dot minimal; most ideas belong in the right‑click menu.
- 🖥️ **Port to macOS / Linux** — the most wanted contribution (see below).

## Build & run

```powershell
git clone https://github.com/divyamohan1993/vaani
cd vaani
powershell -ExecutionPolicy Bypass -File build.ps1   # installs the Rust gnu toolchain if missing
powershell -ExecutionPolicy Bypass -File autoconfig.ps1
```

- The Rust helper lives in [`helper/`](helper/) (`windows` crate, `enigo`, `tiny_http`).
- The speech UI lives in [`app/`](app/) (plain HTML/CSS/JS, embedded into the binary at build time).
- Debug build keeps a console; set `VAANI_DEBUG=1` to log to `%LOCALAPPDATA%\Vaani\vaani.log`.

## Code style

- Run `cargo fmt` and `cargo clippy` before opening a PR (CI checks both).
- Keep the helper tiny and dependency‑light. Justify every new crate.
- Comments explain **why**, not what.

## Porting to macOS / Linux

The hard parts are already cross‑platform: recognition runs in Chrome, and typing uses `enigo` (which supports macOS and Linux/X11). What's needed per‑OS:

- a borderless, always‑on‑top, draggable mic window (AppKit on macOS; GTK/X11 on Linux),
- a global hotkey,
- launching Chrome hidden as the engine.

The Windows implementation in [`helper/src/main.rs`](helper/src/main.rs) is a clear reference. Open an issue first so we can coordinate.

## Pull requests

Small, focused PRs. Describe the change and how you tested it. Be kind — see the [Code of Conduct](CODE_OF_CONDUCT.md).
