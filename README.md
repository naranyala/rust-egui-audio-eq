# Audio EQ — Rust + egui starter (Linux first)

Native desktop audio equalizer built with **eframe/egui** and **cpal**: an
in-app EQ processor that plays its own audio (test signals today, your files
next) through a real, tested DSP chain. Not a system-wide EQ — the vision and
its non-goals are spelled out in
[`PYRAMID-OF-INTENTS.md`](PYRAMID-OF-INTENTS.md).

> **Stage: scaffold.** The repo currently holds the intent documents and an
> empty window; DSP, audio, and UI land with the P0 tasks in
> [`TODOS.md`](TODOS.md). Nothing here claims a feature it doesn't have yet.

## Status

| Platform | Tier | State |
|---|---|---|
| Linux | 1 — now | Scaffold; environment verified (ALSA + PipeWire sink, 48 kHz) |
| Windows / macOS | 2 — planned | Targets drop in later — eframe + cpal are already cross-platform |
| Android / iOS | — | Out of scope — the [`dart-flutter-audio-eq`](https://github.com/naranyala/dart-flutter-audio-eq) sibling covers Android |

## Stack

- **eframe 0.36 / egui 0.36** — window + UI
- **cpal 0.18** — audio output
- **egui_plot 0.37** — response curve (0.37 is the egui-0.36 pairing;
  0.36 of egui_plot targets egui 0.35)
- **symphonia** (P1) — file decode
- Hand-written **RBJ biquads**, no DSP crate — tested and independent of
  egui's release cycle

## Quick start

```bash
cargo run                                   # opens the window (scaffold only)
cargo test                                  # DSP tests land with the P0 tasks
cargo clippy --all-targets -- -D warnings
```

Build deps on AlmaLinux/Fedora: `pkg-config`, `alsa-lib`, `libxkbcommon`,
xcb/X11 dev packages. All present on the dev machine — verified 2026-10-15,
no extra installs required.

## Planned features (tracked in [TODOS.md](TODOS.md))

- **8-band graphic EQ** (60/150/400/1k/2.4k/6k/12k/16k Hz, ±12 dB) with a
  bit-exact bypass A/B
- **Test signals** — fixed/sweep tone, white & pink noise, silence
- **Response curve** drawn from the same DSP that renders the audio, plus
  live level meters
- **Equalizer APO `.txt` import/export** — presets move between this app and
  `dart-flutter-audio-eq`
- **AutoEQ headphone profiles** (≥20, attributed) + named presets with
  persistence
- **File playback** (WAV/FLAC/MP3/OGG) with transport: play/pause, seek, loop
- **Tier 2**: Windows + macOS builds with zero UI changes

## Docs

- [`PYRAMID-OF-INTENTS.md`](PYRAMID-OF-INTENTS.md) — vision → principles →
  goals → epics; higher levels win arguments
- [`TODOS.md`](TODOS.md) — phased tracker (P0 make it real → P1 trustworthy
  and interoperable → P2 Tier 2 + shippable), every task tagged `[Ex/Gx]`

## Research behind this project

- [comparative-study-of-audio-eq](https://github.com/naranyala/comparative-study-of-audio-eq) —
  OS hook mechanisms, reference apps (Equalizer314, Resonance, EasyEffects), formats
- [awesome-audio-processing](https://github.com/naranyala/awesome-audio-processing) —
  DSP frameworks, analyzer libraries, EQ Cookbook math

## License

No LICENSE file yet — GPL-3.0 proposed (consistent with the sibling project
and the "free, open" vision). Decide before importing any GPL-derived code or
accepting contributions.
