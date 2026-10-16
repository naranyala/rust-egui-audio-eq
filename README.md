# rust-egui-audio-eq

A native **8-band graphic equalizer for Linux desktops**, written in Rust:
[eframe/egui](https://docs.rs/eframe) for the window and UI,
[cpal](https://docs.rs/cpal) for real-time audio output,
[egui_plot](https://docs.rs/egui_plot) for the response curve.

The app plays its own test signals through a live DSP chain — every slider
drives filters that actually run — and draws the curve from the same math
that renders the audio. It is an *in-app* equalizer: it processes playback it
owns (test signals today, your audio files next), not the whole desktop.
The vision, principles, and explicit non-goals live in
[`PYRAMID-OF-INTENTS.md`](PYRAMID-OF-INTENTS.md).

> **Stage: P0 — working.** Band EQ, test signals, bypass A/B, live meters,
> and the response curve all work, and every claim below is verified. Presets,
> file playback, and additional platforms are planned and tracked in
> [`TODOS.md`](TODOS.md).

## Features

**Working today**

- **8-band graphic EQ** — 60 / 150 / 400 / 1k / 2.4k / 6k / 12k / 16k Hz,
  ±12 dB per band (low shelf on band 1, high shelf on band 8, peaking Q = 1.0)
- **Master gain** (−24 … +6 dB) and a **bit-exact bypass** for honest A/B
- **Test signals** — 440 Hz tone, 200 Hz → 8 kHz sweep, white noise,
  pink noise, silence
- **Response curve** — 20 Hz … 20 kHz on a log axis, recomputed only when a
  parameter changes, drawn at ~30 fps
- **Live meters** — RMS / peak in dB from the audio callback, plus device,
  rate, format, and stream-error status in the status bar
- **Smoothed parameters** — 20 ms one-pole smoothing, no zipper noise on
  slider drags
- **Headless hooks** — `EQ_INITIAL_GAINS` / `EQ_INITIAL_BYPASS` env vars for
  scripted A/B runs

**Planned** (tracked in [`TODOS.md`](TODOS.md)) — Equalizer APO `.txt` preset
import/export, AutoEQ headphone profiles, named presets with persistence,
per-band Q and filter types, file playback (WAV/FLAC/MP3/OGG) with transport,
device picker, FFT spectrum overlay, Windows and macOS builds.

## Verification

| Check | Result |
|---|---|
| `cargo test` | **36 passed** — biquads (7), chain (6), smoothing (3), state (9), sources (5), render core (3), curve (3) |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| 10 s smoke run | window stays up; sink input `PipeWire ALSA [rust-egui-audio-eq]` appears |
| Sink-monitor capture (`parec`, 48 kHz stereo) | bypass ≈ flat within 0.12 dB; +6 dB bands measure **+6.1 dB** at 440 Hz vs +6.6 dB predicted by the RBJ math |

Capture methodology and the full measurement table:
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) → *Audible proof*.

## Getting started

### Prerequisites

- Rust stable (built and tested with **1.98.1**, edition 2024)
- Audio stack: ALSA + PulseAudio/PipeWire (PulseAudio compatibility layer of
  PipeWire is what development ran on)
- Distro packages for building (Fedora/AlmaLinux names): `pkg-config`,
  `alsa-lib`, `libxkbcommon`, and the xcb/X11 development packages

### Build, run, check

```bash
cargo run                              # build + open the window (440 Hz tone plays)
cargo test                             # 36 unit tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Scripted runs without touching the UI:

```bash
EQ_INITIAL_GAINS=0,6,6,0,0,0,0,0 cargo run   # start with +6 dB on bands 2 and 3
EQ_INITIAL_BYPASS=1 cargo run                 # bit-exact bypass from launch
```

## Using the app

| Region | Contents |
|---|---|
| Top bar | source picker (tone / sweep / white / pink / silence), bypass checkbox, master dB slider, reset button |
| Left panel | eight vertical band sliders (±12 dB), each labeled with frequency and filter type (LS / PK / HS) |
| Center | the response curve on a log-frequency axis |
| Bottom bar | device name · sample rate · sample format · channels, stream errors, RMS/peak meters, orange **BYPASS** badge when engaged |

If audio fails to start (no device, unsupported format), the app still opens
and shows the reason in the status bar — it never panics.

## Dependencies

Exactly **three direct crates**; everything else arrives transitively
(182 crates in the full graph, 170 normal dependencies — see
[`docs/DEPENDENCIES.md`](docs/DEPENDENCIES.md) for audit commands):

| Crate | Version | Role |
|---|---|---|
| `eframe` | 0.36.2 | App shell: window + event loop (winit), GPU painter (glow/OpenGL), egui integration |
| `cpal` | 0.18.2 | Real-time output stream; on Linux it drives ALSA (`alsa` → `alsa-sys` → `libasound`) |
| `egui_plot` | 0.37.0 | Log-frequency plot, drawn through egui's own tessellator — no second rendering backend |

**Version pairing.** egui 0.36 and egui_plot 0.37 belong together: egui_plot
0.36 targets egui 0.35, so mixing minor versions fails to compile. cpal 0.18
changed from 0.17 (`SampleRate` is now a plain `u32`, `Device::description()`
replaced `Device::name()`) — migration notes in
[`docs/DEPENDENCIES.md`](docs/DEPENDENCIES.md).

**Trimmed on purpose.** `eframe` is built with `default-features = false`
plus `default_fonts`, `glow`, `wayland`, `x11`: no wgpu renderer, no
D-Bus accessibility bus, no web screen reader, no link-opening — that alone
cut the graph from ~347 crates to 182.

**Deliberately absent:** serde/persistence (presets arrive with P1), file
decoders (P1), logging frameworks, and any DSP crate — the RBJ biquads are
hand-written and unit-tested in [`src/dsp/`](src/dsp/).

## Project layout

```
src/
├── main.rs          eframe entry point (window options, app id)
├── app/             UI — one file per panel, wired to shared state
│   ├── mod.rs       EqApp: frame orchestration + engine wiring
│   ├── toolbar.rs   source / bypass / master / reset (top panel)
│   ├── bands.rs     eight band sliders (left panel)
│   ├── status.rs    device, errors, meters, BYPASS badge (bottom)
│   └── curve.rs     response plot + cached curve (center)
├── engine/          sound production and the engine contract
│   ├── mod.rs       EqEngine trait (live today; offline/mock next)
│   ├── live.rs      LiveEngine: cpal stream + callback
│   └── render.rs    Renderer: pure source → EQ → master core + meters
├── state.rs         Params/SharedState — UI ↔ engine contract, env hooks
└── dsp/             pure math: biquad · eq (BandSpec table) · smooth · source
docs/              architecture · DSP · development · dependencies
TODOS.md           phased tracker (P0 → P1 → P2), every task tagged [Ex/Gx]
PYRAMID-OF-INTENTS.md   vision → principles → goals → epics
```

## Documentation

| Document | Contents |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Threads, data flow, module map, real-time invariants |
| [`docs/DSP.md`](docs/DSP.md) | Filter math, band layout, smoothing, bypass semantics, verification numbers |
| [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) | Setup, commands, env hooks, test inventory, debug recipes (parec capture) |
| [`docs/DEPENDENCIES.md`](docs/DEPENDENCIES.md) | Crate rationale, feature flags, version pairing, supply-chain audit |
| [`TODOS.md`](TODOS.md) | What's done and what's next |
| [`PYRAMID-OF-INTENTS.md`](PYRAMID-OF-INTENTS.md) | Why the project exists; higher levels win arguments |

## Roadmap

- **P1 — trustworthy and interoperable:** APO preset import/export, AutoEQ
  profiles, named presets with persistence, parametric editing (per-band Q and
  filter types), preamp, file playback with transport, device picker, offline
  WAV render, golden vectors, engine conformance suite
- **P2 — shippable:** Windows and macOS builds, FFT spectrum overlay, CI,
  license file, packaging, tested-platform matrix

## License

No LICENSE file yet — GPL-3.0 is proposed. Decide before importing any
GPL-derived code or accepting contributions.
