# Dependencies

Why this project has **three** direct crates, what each one drags in, what
was deliberately switched off, and how to audit the graph. The short version
lives in the [README](../README.md#dependencies).

## Philosophy

1. **Small surface.** Every dependency is a liability (compile time, audit
   time, breaking-change time). Three direct crates, each obviously
   necessary.
2. **No second implementation of what we already have.** Hand-written RBJ
   biquads instead of a DSP crate — the math is 100 lines, fully unit-tested,
   and version-independent.
3. **Add features by enabling features, not by adding crates.** Dead
   machinery gets switched off, not tolerated.
4. **Pin pairings that must match.** egui and egui_plot versions are coupled;
   that's documented below so upgrades don't turn into archaeology.

## Direct dependencies

### `eframe` 0.36.2 — the app shell

One crate re-egui's window, event loop, GPU painter, and platform glue:

| It provides | Via |
|---|---|
| window + input + event loop | `winit` 0.30 |
| GPU painter (OpenGL) | `egui_glow` → `glow` → `glutin`/`glutin-winit` (GL loading + surface) |
| egui itself | `egui` 0.36 → `epaint` (tessellation), `emath`, `ahash` |
| clipboard | `egui-winit` → `arboard` (+ `smithay-clipboard` on Linux, `image` for PNG payloads) |
| window icon | `image` (eframe's own dep) |

**Feature configuration** ([`Cargo.toml`](../Cargo.toml)):

```toml
eframe = { version = "0.36", default-features = false, features = [
    "default_fonts",   # bundled font stack — required for text
    "glow",            # OpenGL painter: enough for egui's immediate-mode UI
    "wayland",         # native Wayland window backend
    "x11",             # X11 window backend (also covers XWayland)
] }
```

What `default-features = false` removes (all absent from `cargo tree`):

| Off by default | Why we don't need it |
|---|---|
| wgpu renderer | glow already renders egui; wgpu would pull a second GPU stack (wgpu-core/wgpu-hal/naga — tens of crates) |
| accessibility bus | eframe's `accesskit` feature wires D-Bus/atspi (`zbus`, `atspi`); egui's accessibility *API types* stay, the bus does not |
| `web_screen_reader`, links/webbrowser | browser-only / link-opening behavior — this is a native audio app |
| persistence | saves app state via serde; presets arrive with P1 (do it there, deliberately) |
| default fonts off/on | we keep them on; turning them off needs a custom font wiring |

Result: **347 → 182 crates** in the full graph (same measurement command,
see *Audit* below).

**Not removable:** `image` and `arboard` (clipboard) are non-optional
dependencies of eframe/egui-winit, and `accesskit` itself is a hard
dependency of `egui` (type definitions). They stay regardless of features.

### `cpal` 0.18.2 — audio output

Cross-platform stream API; on Linux the path is:

```text
cpal → alsa 0.11 → alsa-sys 0.4 → libasound (system ALSA)
     → dasp_sample (sample-format conversion)
     → libc
```

How we use it (details in [`docs/ARCHITECTURE.md`](ARCHITECTURE.md)):

- default host → default output device → default output config;
- dispatch on `SampleFormat` (`f32`, `i16`, `u16` are built; others error
  cleanly to the status bar);
- `build_output_stream(config, data_cb, err_cb, timeout: Option<Duration>)` —
  cpal 0.18 takes the config **by value** and an explicit timeout (we pass
  `None`);
- errors surface as text, never panic.

**0.17 → 0.18 migration notes** (encountered during P0):

| 0.17 | 0.18 |
|---|---|
| `StreamConfig.sample_rate: SampleRate(u32)` | `sample_rate: u32` directly |
| `Device::name()` | `Device::description() -> Result<DeviceDescription, _>`, `.name()` on it |
| `build_output_stream(&config, …)` | `build_output_stream(config, …)` (Copy) |
| error callback type `cpal::StreamError` | error callback type `cpal::Error` |

### `egui_plot` 0.37.0 — the response curve

| It provides | Via |
|---|---|
| `Plot`, `Line`, `PlotPoints`, grid spacers | `egui` + `emath` only — **no** second raster backend |
| log axis, custom tick formatter | built in |

**Version pairing rule** — egui_plot minor versions track egui minor
versions one release apart:

| egui_plot | targets egui |
|---|---|
| 0.37.x | **0.36.x ← this project** |
| 0.36.x | 0.35.x |
| 0.35.x | 0.34.x |

Mixing (e.g. eframe 0.36 + egui_plot 0.36) fails to compile with two-egui-
versions errors. Bump `egui_plot` **first or together with** eframe/egui.

Notably absent: `tiny-skia`, `resvg`, `image` under egui_plot — the plot
draws through egui's tessellator, so the curve costs no extra pipeline.

## Deliberately not dependencies (yet)

| Crate | Comes with | When |
|---|---|---|
| `serde` (+ `ron`/`serde_json`) | preset persistence, app-state saving | P1 — presets |
| `symphonia` (+ codecs) | WAV/FLAC/MP3/OGG decode | P1 — file playback |
| any DSP crate (`biquad`, `dasp` filters, …) | — | never planned: hand-written RBJ is tested and small |
| `tracing` / `log` handlers | structured logging | not needed; errors go to the status bar by design |
| `anyhow` / `thiserror` | error ergonomics | not needed: startup errors are `String`, callback errors are `cpal::Error` text |
| `rayon` / thread pools | — | never: the callback is one thread by design |
| file-dialog crates | open/save dialogs | only when file playback lands (P1); will use eframe/winit-native path then |

## Audit commands

```bash
# direct dependencies (3)
cargo tree -e normal --depth 1

# full graph size (182 with build/proc-macro nodes, 170 normal)
cargo tree --prefix none | sort -u | wc -l
cargo tree -e normal --prefix none | sort -u | wc -l

# who pulls in X?
cargo tree -i image          # eframe (icon) + arboard (clipboard)
cargo tree -i accesskit      # egui (API types)
cargo tree -i alsa-sys       # cpal

# confirm something is really gone after a feature change
cargo tree -i wgpu           # should error: not in graph
```

After any version bump: `cargo test`, `cargo clippy --all-targets -- -D
warnings`, and re-check the counts above — a silent jump in crate count is a
review signal, not progress.

## Version summary (locked today)

| Crate | Version | Source of truth |
|---|---|---|
| eframe | 0.36.2 | `Cargo.toml` → `0.36` |
| egui (transitive) | 0.36.2 | must match eframe's egui |
| cpal | 0.18.2 | `Cargo.toml` → `0.18` |
| egui_plot | 0.37.0 | `Cargo.toml` → `0.37` (egui-0.36 pairing) |
| winit (transitive) | 0.30.13 | eframe's choice |
| alsa (transitive) | 0.11.0 | cpal's choice |

`Cargo.lock` is committed — everyone builds the same graph.
