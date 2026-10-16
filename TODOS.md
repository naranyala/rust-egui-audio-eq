# TODOS — rust-egui-audio-eq

Parent intent for every item: `PYRAMID-OF-INTENTS.md`. Tags `[Ex/Gx]` point at
the epic and goal each task serves. Phase order is intentional — finish P0
before starting P1.

Legend: `[ ]` open · `[x]` done · `[~]` in progress / blocked (note why).

## P0 — Make it real (audible EQ on Linux)

- [x] Environment probe (2026-10-15): toolchain 1.98.1 / edition 2024,
      X11+Wayland display, ALSA + PulseAudio sink (48 kHz), system build deps
      (alsa, xkbcommon, xcb, x11 family, libpulse) all present, crates.io
      reachable — no installs required
- [x] `[E1/G8]` Starter scaffold: pin `eframe 0.36` / `cpal 0.18` /
      `egui_plot 0.37` (0.37 is the egui-0.36 pairing — egui_plot 0.36 wants
      egui 0.35); empty `App` opens a window and exits cleanly — **done**:
      `default-features = false` + `["default_fonts", "glow", "wayland",
      "x11"]` cut deps from 347 → 182 unique crates (no wgpu/accesskit/zbus
      tree); window opens on wlroots session
- [x] `[E2/G2]` RBJ biquads (`src/dsp/biquad.rs`): peaking + low/high shelf
      + LPF/HPF; unit tests for flat transparency, ±6 dB gain, shelf
      direction, stability at extremes — **done**: 7 tests
- [x] `[E2/G2,G5]` `EqChain` (`src/dsp/eq.rs`): 8 bands (60/150/400/1k/
      2.4k/6k/12k/16k Hz — low-shelf first, high-shelf last), per-buffer
      coefficient recompute, `magnitude_response`; flat-at-0-dB test —
      **done**: `PEAKING_Q = 1.0`, coefficient update every 32 frames
      (BLOCK const), 5 tests
- [x] `[E2/G1]` Parameter store + smoothing (`src/state.rs`,
      `src/dsp/smooth.rs`): shared snapshot-per-buffer pattern (`RwLock`
      read once per buffer, never held while processing) + one-pole ~20 ms
      smoother against zipper noise — **done**: band τ = 20 ms, master
      τ = 30 ms; 8 tests total across both files
- [x] `[E6/G1]` cpal output stream (`src/engine/live.rs`): device + config
      selection, `f32`/`i16`/`u16` sample-format dispatch (cpal 0.18 requires
      the `timeout` argument), errors surfaced to the status bar instead of a
      panic, `Stream` owned by the app — **done**: `Device::description()`
      (name() removed in 0.18), `Error` callback → status bar, `Drop` pauses
      the stream
- [x] `[E3/G1]` Test signals (`src/dsp/source.rs`): tone (fixed + sweep),
      white noise, pink noise, silence — **done**: 440 Hz @ 0.3 amp, 200 Hz
      → 8 kHz sweep, Paul-Kellet pink; 5 tests
- [x] `[E1/G1,G5]` Main UI (`src/app/`): 8 band sliders (±12 dB), master
      gain, bypass toggle, source selector, status bar (device, rate, buffer,
      last error) — **done**: egui 0.36 `Panel::left/top/bottom` +
      `eframe::App::ui` (not `update`), BYPASS badge, RMS/peak meters,
      `EQ_INITIAL_GAINS` / `EQ_INITIAL_BYPASS` env hooks for headless runs
- [x] `[E5/G5]` Response curve: `egui_plot` line from
      `EqChain.magnitude_response` on a log-frequency axis (20 Hz–20 kHz),
      recomputed only when parameters change — **done**: 200-pt line,
      `x_grid_spacer` decade grid + kHz labels, `curve_dirty` flag
- [x] `[E5/G5]` Repaint wiring: `request_repaint` on parameter change plus
      ~30 Hz while running, so meters/curve stay live without spinning the
      CPU — **done**: `request_repaint_after(33 ms)` after each frame
- [x] `[E7/G8]` Quality gates green: `cargo test` green,
      `cargo clippy --all-targets -- -D warnings` clean, 10 s window smoke
      run shows sink + sample rate in the status bar with no error —
      **done (2026-10-16)**: 26/26 tests, clippy exit 0, app stayed alive
      with live `pactl list sink-inputs` entry
      `PipeWire ALSA [rust-egui-audio-eq]`, only warning is a harmless
      `xkbcommon` locale compose-file message
- [x] `[E7/G1]` Audible proof: app plays a tone while `parec` captures the
      sink monitor; record RMS for bypass vs +6 dB @ 440 Hz as evidence —
      **done (2026-10-16)**, `alsa_output.pci-0000_00_1f.3.analog-stereo`
      monitor, 4 s × 48 kHz stereo captures, sink restored to muted/0%
      afterwards:
      | config | RMS | level |
      |---|---|---|
      | flat EQ on (all 0 dB) | 0.2110 | −13.51 dBFS |
      | bypass | 0.2082 | −13.63 dBFS (matches flat within 0.12 dB) |
      | +6 dB @ 150 & 400 Hz bands | 0.4196 | −7.54 dBFS (**+6.09 dB** measured vs +6.59 dB RBJ prediction at 440 Hz) |

## P1 — Make it trustworthy, interoperable, observable

- [ ] `[E4/G3]` Equalizer APO `.txt` import/export: preamp + PK/LS/HS, OFF
      skip, round-trip tests; cross-app check — export from
      `dart-flutter-audio-eq`, load here, response curves must match
- [ ] `[E4/G3,G4]` AutoEQ bundle: ≥20 headphone profiles as assets with
      measurement-source attribution shown in the UI
- [ ] `[E4/G4]` Presets: built-ins (Flat/Bass/Vocal/Treble) + named user
      presets (CRUD) persisted via serde (`ron` or `json`); last-used state
      restored on launch
- [ ] `[E2/G2]` Truly parametric editing: per-band Q + filter type
      (PK/LS/HS/notch) in the UI with persistence; `notch` closes the
      cookbook promise; unit tests
- [ ] `[E2/G2]` Preamp gain (−24…+12 dB) + output clamping; smoothing tuning
      tests (no zipper artifacts on slider drags)
- [ ] `[E3/G6]` File playback: symphonia decode (WAV/FLAC/MP3/OGG) into the
      same `EqChain`; transport (play/pause, seek bar, loop), track label
- [ ] `[E6/G1]` Device picker + sample-rate/buffer negotiation; hot-unplug →
      status-bar notice, app stays alive
- [ ] `[E5/G5]` Live meters: RMS/peak dB computed in the callback, shown in
      the status bar
- [ ] `[E5/G5,G1]` `OfflineEngine`: render the current EQ state to WAV +
      golden-file test (doubles as the A/B evidence path)
- [ ] `[E2/G2]` Golden vectors: `tool/golden_vectors.py` generates reference
      biquad responses into `tests/data/`, consumed by Rust tests to catch
      coefficient bugs point tests miss
- [ ] `[E1/G1]` Engine conformance suite: the same gain/enable/clamp/curve
      tests run against `LiveEngine`, `OfflineEngine`, and `MockEngine`
- [ ] `[E7/G8]` Docs: README + `docs/` (architecture, dsp, presets) linking
      both intent files

## P2 — Tier 2 + shippable

- [ ] `[E1/G7]` Windows target: build + smoke (device enumeration, tone
      playback); no UI changes expected
- [ ] `[E1/G7]` macOS target: build + smoke (CoreAudio device enumeration)
- [ ] `[E5/G5]` Live FFT spectrum overlay behind the same plot widget
- [ ] `[E7/G8]` CI (GitHub Actions or equivalent): fmt + clippy + test on
      push, build check on tag, `cargo outdated` monthly
- [ ] `[E8/G8]` LICENSE file (GPL-3.0 proposed, consistent with the
      Equalizer314-based Flutter sibling): decide BEFORE importing any
      GPL-derived code or accepting contributions
- [ ] `[E8/G8]` Release track: icon + display name; Linux packaging (pick
      ONE of deb/rpm/Flatpak) + Windows artifact; versioning scheme
- [ ] `[E7/G8]` Tested-platform matrix: ALSA / PipeWire versions and devices
      actually exercised, recorded in README

## Backlog (do NOT promote without a goal)

- System-wide EQ via PipeWire/Pulse hooks — the `dart-flutter-audio-eq` lane;
  needs a goal first
- DAW plugin formats (VST/AU/LV2/CLAP) — needs a goal first, currently non-goal
- Convolution / FIR engine (CamillaDSP-class) — only if IIR proves insufficient
- Live-stream recording / capture-and-save — needs a goal first
- Per-app profiles, loudness compensation, crossfeed — capture as G/E first
- MIDI / hardware-controller mapping — needs a goal first
- WASM build — egui supports it but cpal does not; conflicts with the audio
  vision, needs its own goal
- URL / streaming-radio playback — needs a goal first
- First-run onboarding — needs a goal first
- i18n — needs a goal first
- Cloud sync / accounts — explicitly out of scope until vision changes

## Working agreement

- One task in flight at a time; check `[~]` with date + blocker when paused.
- Every completion names its `[Ex/Gx]`; pyramid violations get re-scoped, not
  merged.
- Review monthly: promote at most 2 backlog items, retire stale ones.
