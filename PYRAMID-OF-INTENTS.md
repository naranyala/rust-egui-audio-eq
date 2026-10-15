# Pyramid of Intents — rust-egui-audio-eq

How to read this file: top = stable, long-lived intent. Bottom = concrete,
changeable work. Higher levels constrain lower levels — a TODO that conflicts
with a level above it is wrong and must be re-scoped, not the other way round.

```
            L0 VISION (why this exists)
            L1 MISSION + PRINCIPLES (how we behave)
            L2 STRATEGIC GOALS (what winning looks like)
            L3 EPICS / CAPABILITIES (system must do X)
            L4 TASKS (work items) → see TODOS.md
```

---

## L0 — Vision

A free, open, native desktop audio equalizer that a user can trust with their
daily listening: correct sound first, beautiful UI second, every desktop
platform eventually.

- Tier 1 now: **Linux**.
- Tier 2 later: **macOS + Windows** — eframe and cpal are already
  cross-platform; targets drop in without rewrites.
- Mobile out of scope: this is a desktop app (the `dart-flutter-audio-eq`
  sibling covers Android).

Scope: this app processes **its own playback** — test signals today, your
audio files next — through a live cpal stream. It is not a system-wide EQ;
OS-level hooks are a non-goal (see L2).

## L1 — Mission + Principles

Mission: ship the smallest EQ that is **audibly real** on Linux, then widen
platform coverage without forking the app.

Principles (in priority order):

1. **Correctness over features.** An 8-band EQ that truly changes sound beats
   a 31-band mock. Every control must drive DSP that really runs; bypass must
   be a bit-exact A/B. Never present preview state as real.
2. **UI never touches the audio thread.** The UI writes a parameter snapshot;
   the cpal callback reads it once per buffer and processes lock-free. All
   DSP sits behind an `EqEngine`-style contract — the same rule the Flutter
   sibling enforces between `EqController` and `EqEngine`.
3. **Open formats first.** Equalizer APO `.txt` and AutoEQ profiles before
   inventing a native preset format; presets must move between this app and
   `dart-flutter-audio-eq`.
4. **Reference before invention.** Copy proven math (RBJ Audio EQ Cookbook
   biquads) and proven plumbing (cpal, eframe, egui_plot) instead of
   designing new ones.
5. **Real-time discipline.** The audio callback never allocates, never blocks
   on the UI, never panics; failures degrade to a status-bar notice, never a
   crash.
6. **Test the contract.** Pure-DSP unit tests plus golden vectors; one
   conformance suite that every engine implementation (live / offline / mock)
   must pass.

## L2 — Strategic goals

| ID | Goal | Done when |
|----|------|-----------|
| G1 | Audible EQ on Linux | Sliders drive a live cpal stream; bypass is bit-exact A/B; a `parec` capture proves the tone reaches the sink and +6 dB measurably differs from bypass |
| G2 | Trustworthy DSP core | Pure-Rust RBJ biquads with unit tests (flat ≈ 0 dB, +6 dB ≈ ×2 amplitude, shelf direction, clamping respected) plus golden vectors |
| G3 | Preset interop with the Flutter app | APO `.txt` import/export round-trips; a preset exported by `dart-flutter-audio-eq` loads here with an identical response curve |
| G4 | Preset library worth opening the app for | Built-ins (Flat/Bass/Vocal/Treble) plus ≥20 AutoEQ-derived headphone profiles, searchable and attributed |
| G5 | Observable sound | Response curve drawn from the same DSP that renders audio, live level meters; spectrum overlay later |
| G6 | Your files, EQ'd | WAV/FLAC/MP3/OGG playback (symphonia) with full transport (play/pause/seek/loop) and a device picker |
| G7 | Tier-2 ready | macOS and Windows build and pass the same conformance tests with zero UI changes |
| G8 | Trustworthy project | `cargo fmt`/`clippy`/`test` green, CI once the repo is public, LICENSE decided before outside contributions |

Non-goals (explicit): system-wide / OS-hook EQ (PipeWire filter-chains,
WASAPI loopback, Equalizer APO injection), DAW plugin formats
(VST/AU/LV2/CLAP), mobile, REW-style room-correction measurement,
convolution/FIR engines until IIR proves insufficient, accounts / cloud sync
/ subscriptions.

## L3 — Epics / Capabilities

- **E1 App shell & engine abstraction** — eframe entry point, layout, and the
  parameter-store contract (UI writes, callback reads once per buffer);
  `EqEngine` contract with `LiveEngine` (cpal stream), `OfflineEngine`
  (render to WAV), `MockEngine`; one conformance suite run against every
  implementation.
- **E2 DSP core** — RBJ biquads (peaking / low-shelf / high-shelf / LPF /
  HPF / notch), `EqChain` + `magnitude_response`, parameter smoothing,
  preamp, golden-vector validation.
- **E3 Auditioning & playback** — test-signal generator (tone, sweep, white
  and pink noise), file decode via symphonia, transport (play / pause / seek
  / loop), metering.
- **E4 Preset system** — built-ins → APO `.txt` import/export → AutoEQ
  profile bundle → searchable browser → named user presets (CRUD) with serde
  persistence.
- **E5 Visualization** — static response curve (`egui_plot`, free of extra
  permissions) → RMS/peak meters from the callback → live FFT spectrum.
- **E6 Platform plumbing** — device selection and routing, sample-format
  dispatch, buffer/sample-rate negotiation, audio error surfacing, Tier-2
  build targets.
- **E7 Quality gates** — `cargo test` / `clippy` / `fmt`, golden vectors,
  headless smoke runs, capture-based audible proof, CI when the repo goes
  public.
- **E8 Release & distribution** — LICENSE first, then icon/name, Linux
  packaging (one format), Windows/macOS artifacts, versioning scheme.

## L4 — Tasks

Lives in `TODOS.md`. Every task carries its epic + goal tags (e.g.
`[E2/G3]`). If a task has no parent goal, either the pyramid is incomplete or
the task doesn't belong — resolve before coding.
