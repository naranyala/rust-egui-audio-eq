# Architecture

How `rust-egui-audio-eq` is put together: threads, data flow, and the rules
that keep the audio callback safe. Parent intent:
[`PYRAMID-OF-INTENTS.md`](../PYRAMID-OF-INTENTS.md) (principles 2 and 5 are
the ones this document implements).

## The one diagram

```
 UI THREAD (eframe/egui)                          AUDIO THREAD (cpal callback)
┌───────────────────────────────────────┐        ┌─────────────────────────────────────┐
│ EqApp (src/app/mod.rs)                │        │ LiveEngine callback (src/engine/    │
│                                       │        │   live.rs) wrapping the Renderer    │
│ panels: toolbar · bands · status      │        │                                     │
│   │  writes local copy of Params      │  Arc   │ 1. state.snapshot()   (once/buffer) │
│   ▼                                  │ Shared  │ 2. renderer.sync()    (set targets) │
│ SharedState::update()  ───────────────┼──State──▶ 3. per 32-frame block:             │
│   (RwLock<Params>, sanitized)        │        │    advance smoothers, rebuild coeffs│
│                                       │        │ 4. per frame:                        │
│ SharedState::meter() ◄────────────────┼───────▶│    renderer.next_sample()           │
│   (RMS/peak atomics)                  │        │ 5. set_meter(renderer.take_levels())│
│                                       │        │                                     │
│ CurveCache: EqChain::magnitude_db()   │        │ samples → ALSA via PipeWire/Pulse   │
│   (UI-side coefficients) → plot       │        └─────────────────────────────────────┘
│                                       │
│ request_repaint_after(33 ms)          │        CREATION / TEARDOWN (UI thread)
└───────────────────────────────────────┘        LiveEngine::start() builds the stream,
                                                 its Drop pauses it.
```

Two threads, one shared state, zero shared locks held while processing.

## Module map

| File | Responsibility | Key exports |
|---|---|---|
| [`src/main.rs`](../src/main.rs) | eframe entry: window options (1100×640, min 860×480), app id, construction | `main()` |
| [`src/app/mod.rs`](../src/app/mod.rs) | UI composition: panel order, state wiring, engine ownership | `EqApp` |
| [`src/app/toolbar.rs`](../src/app/toolbar.rs) | Top panel: source, bypass, master, reset | `show()` |
| [`src/app/bands.rs`](../src/app/bands.rs) | Left panel: eight band sliders | `show()` |
| [`src/app/status.rs`](../src/app/status.rs) | Bottom panel: device label, errors, meters, bypass badge | `show()` |
| [`src/app/curve.rs`](../src/app/curve.rs) | Center panel: cached response curve, log grid | `CurveCache`, `fmt_hz()` |
| [`src/state.rs`](../src/state.rs) | The UI ↔ engine contract: parameter snapshot store, meter atomics, env-hook parsing | `Params`, `SharedState`, `to_db()` |
| [`src/engine/mod.rs`](../src/engine/mod.rs) | The engine contract every backend implements | `EqEngine` |
| [`src/engine/live.rs`](../src/engine/live.rs) | cpal stream lifecycle + the real-time callback | `LiveEngine` |
| [`src/engine/render.rs`](../src/engine/render.rs) | Pure per-sample DSP core shared by all engines | `Renderer` |
| [`src/dsp/biquad.rs`](../src/dsp/biquad.rs) | One RBJ filter: coefficients, `process()`, `magnitude_db()` | `Biquad` |
| [`src/dsp/eq.rs`](../src/dsp/eq.rs) | The 8-band chain: band table, gain → filters, summed response | `EqChain`, `BANDS`, `BandSpec` |
| [`src/dsp/smooth.rs`](../src/dsp/smooth.rs) | One-pole exponential smoother (anti-zipper) | `Smoother` |
| [`src/dsp/source.rs`](../src/dsp/source.rs) | Deterministic test signals | `Source`, `SourceKind` |

Dependency direction: `app → engine → state, dsp`, `app → state, dsp`,
`engine → state, dsp`, `state → dsp`.
`dsp` depends on nothing else — it is pure math, which is why its tests are
fast and deterministic. `engine/render.rs` is deliberately cpal-free so the
offline renderer and mock engines can share it.

## The parameter contract

`state.rs` exists to enforce one rule (**principle 2**): *the UI never
touches the audio thread*.

1. **UI writes** — `EqApp::ui` edits a local `Params` copy (it is `Copy`, so
   no allocation). If any widget reported `changed()`, the frame ends with
   `SharedState::update(|p| *p = params)`, which takes the write lock,
   stores, and **sanitizes** (clamps bands to ±12 dB, master to −24…+6 dB).
2. **Audio reads once** — the cpal callback starts every buffer with
   `state.snapshot()`: one short read-lock, returns `Params` **by value**,
   releases the lock immediately. The lock is never held while filtering.
3. **Meters flow back** — after rendering a buffer the callback stores
   RMS/peak as `f32::to_bits()` into two `AtomicU32`s (relaxed ordering);
   the UI reads them with `SharedState::meter()` without any lock.

Lock poisoning is tolerated, not crashed on: every lock access uses
`unwrap_or_else(|e| e.into_inner())` so a poisoned lock still returns data
instead of panicking in either thread.

The UI additionally holds a *separate* `EqChain` copy (inside `CurveCache`)
purely to evaluate `magnitude_db()` for the plot — it is never the audio
path, it just uses the same `EqChain` code so the curve cannot drift from
the sound. Startup test hooks (`EQ_INITIAL_GAINS`, `EQ_INITIAL_BYPASS`) are
parsed by `Params::with_env_overrides`, which takes an injected lookup
function — that keeps the parsing unit-testable without process-global env
state.

## Audio stream lifecycle

`LiveEngine::start(state)` (UI thread, at app construction):

1. `cpal::default_host()` → `default_output_device()`; failure returns
   `Err(String)`.
2. `device.description()` for the status bar (cpal 0.18 renamed
   `Device::name()`), `default_output_config()` → sample format + `StreamConfig`.
3. Format dispatch: `f32`, `i16`, `u16` each get a monomorphized
   `build_stream::<T>()`; anything else is an error string. The conversion
   `T: SizedSample + FromSample<f32>` lets the DSP always run in `f32` and
   convert on write (`sample.to_sample::<T>()`).
4. The data callback owns a `Renderer` (EqChain + Source + smoothers),
   created with the device's actual sample rate; the error callback stores
   `cpal::Error` text into `last_error` — shown in the status bar on the
   next frame, never a panic.
5. `stream.play()`.

Teardown: `EqApp` holds `Option<Box<dyn EqEngine>>`; dropping `LiveEngine`
pauses the stream. A start failure yields `engine = None` +
`start_error = Some(…)`, which the bottom panel renders in red — the window
still opens.

## Render core walkthrough

[`Renderer`](../src/engine/render.rs) is the pure per-sample core (no cpal,
no locks, no `SharedState`); [`src/engine/live.rs`](../src/engine/live.rs)
adapts it to the callback. Per buffer, the callback does:

1. `state.snapshot()` — one short read-lock, `Params` by value.
2. `renderer.sync(&params)` — copy UI parameters into smoother targets, the
   source kind, and the bypass flag.
3. Per frame: `renderer.next_sample()` —
   - every 32 frames (`BLOCK`, ≈0.67 ms @ 48 kHz): advance all smoothers
     **per sample** (τ_band = 20 ms, τ_master = 30 ms), then rebuild the
     filters whose gain moved (`EqChain::set_gains`) and refresh the master
     linear gain;
   - `source.next(dt)` → if not bypassed, `eq.process(sample)` × master →
     `clamp(−1, 1)` → accumulate RMS/peak;
   - the callback writes the frame to **all** channels (mono test signals
     are copied to every channel) and converts `f32` → device format.
4. `renderer.take_levels()` hands back RMS/peak for the buffer (and restarts
   the metering window); the callback stores them into the atomics.

**Bypass** is a flag on the renderer: the sample goes source → clamp →
output. That makes A/B comparisons bit-exact except for the clipping safety
clamp — and yes, there is a unit test asserting bit-equality
(`bypass_is_bit_identical_to_flat_processing`). See
[`docs/DSP.md`](DSP.md).

## Curve & repaint discipline

- `CurveCache` ([`src/app/curve.rs`](../src/app/curve.rs)) holds 201
  log-spaced points (20 Hz … 20 kHz) of `EqChain::magnitude_db`, rebuilt only
  when `mark_dirty()` was called — i.e., when a widget reported `changed()`.
  Dragging a slider rebuilds once per frame; idle frames rebuild nothing.
- Repaint: `request_repaint_after(33 ms)` at the end of every `ui()` call —
  meters and curve update at ~30 Hz without the event loop spinning
  (principle 5: real-time discipline extends to the UI thread).

## Invariants

These are the rules any future change must not break:

1. **No allocation in the callback** — everything is preallocated
   (`Renderer` built once; `snapshot()` returns `Copy`; buffers come from
   cpal). Error strings are the only allocations and they happen on the
   error path, outside the hot loop.
2. **No locks held while processing** — snapshot in, guard dropped, then
   DSP runs.
3. **No panics in the callback** — invalid lengths return early; filters
   clamp inputs (`clamp_freq`, `clamp_q`); output is clamped to ±1;
   poisoning is absorbed (`into_inner`).
4. **Failures surface, never crash** — no device / bad format → red status
   line; runtime stream error → red status line; the window stays up.
5. **Sanitize at the boundary** — `SharedState::update` clamps whatever the
   UI produced, so the audio thread can trust its snapshot.
6. **Bypass is bit-exact** — no smoothing, no filters, no master gain.

## Extension points (what P1 will touch)

| Planned feature | Where it lands | What changes |
|---|---|---|
| File playback | `src/dsp/source.rs` → a decoder-fed source | `Source` grows a "file" mode (symphonia decode feeding the same `next(dt)` slot), plus transport state in a second store alongside `Params` |
| Offline render + golden files | new `src/engine/offline.rs` | A second `EqEngine` implementation that drives the same `Renderer` into a `Vec<f32>` instead of a cpal stream; conformance tests run against both |
| Device picker | `src/engine/live.rs` + top panel | `LiveEngine::start` takes a device id instead of always `default_output_device()` |
| Per-band Q / filter types | `src/dsp/eq.rs` + `Params` | `Params.gains_db` grows a per-band config struct layered over `BandSpec`; sanitization extends |
| Presets (APO `.txt`, AutoEQ) | new `src/preset.rs` | Parses into `Params`; pure code, no audio or UI changes |

The `EqEngine` contract itself already exists ([`src/engine/mod.rs`](../src/engine/mod.rs))
with `LiveEngine` as its first implementation; the conformance suite that
must pass against live, offline, and mock engines is tracked in `TODOS.md`
under *engine conformance suite*.
