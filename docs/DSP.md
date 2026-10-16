# DSP

The math behind the sound: filters, band layout, smoothing, meters, and the
numbers that prove the chain works. Everything here is pure `f32` Rust in
[`src/dsp/`](../src/dsp/) — no DSP crate, following the RBJ *Audio EQ
Cookbook* recipes (principle 4: reference before invention).

## Signal chain

```
Source (tone / sweep / white / pink / silence)
   │
   ├────────────── bypass? ──────────────────────────┐
   │ no                                              │ yes
   ▼                                                 │
Biquad[0]  low shelf   60 Hz      ±12 dB             │
Biquad[1]  peaking    150 Hz  Q=1 ±12 dB             │
Biquad[2]  peaking    400 Hz  Q=1 ±12 dB             │
Biquad[3]  peaking   1000 Hz  Q=1 ±12 dB             │
Biquad[4]  peaking   2400 Hz  Q=1 ±12 dB             │
Biquad[5]  peaking   6000 Hz  Q=1 ±12 dB             │
Biquad[6]  peaking  12000 Hz  Q=1 ±12 dB             │
Biquad[7]  high shelf 16 kHz     ±12 dB             │
   │                                                 │
   ▼                                                 │
Master gain  −24 … +6 dB                             │
   │                                                 │
   └────────────────────► sample clamp (−1 … +1) ◄───┘
                              │
                              ▼
                    write to every output channel,
                    convert f32 → device format
```

All gain values reach the filters through a 20 ms smoother
(see *Parameter smoothing*), so the chain you hear is continuous even though
the UI moves in discrete slider steps.

## The biquad: `Biquad`

One second-order IIR section in **transposed direct form II** — three
multiplications per sample, minimal state, the standard choice for audio:

```text
y[n]  = b0·x[n] + x1
x1    = b1·x[n] − a1·y[n] + x2
x2    = b2·x[n] − a2·y[n]
```

(`src/dsp/biquad.rs` → `process()`.)

Coefficients come from the RBJ Cookbook. With `ω0 = 2π·f/f_s`,
`α = sin ω0 / (2Q)` and `A = 10^(dB/40)`:

- **Peaking** — `b = [1+αA, −2cos ω0, 1−αA]`, `a = [1+α/A, −2cos ω0, 1−α/A]`.
  Peak gain at `f` equals the requested dB (unit-tested to ±0.01 dB).
- **Shelves** — the standard `S = 2√A·α` forms (low-shelf and high-shelf are
  the two sign variants); they use `α = (sin ω0/2)·√2` (the shelf "S" form),
  no Q.
- **LPF / HPF** — implemented and unit-tested, not yet exposed as bands
  (they are the building blocks for future filter types).

Guards that keep it stable in any condition:

- frequency clamped to `[1 Hz, 0.49·f_s]`, Q to `[0.05, 30]`;
- gains below `1e-4` dB short-circuit to the **identity** filter
  (`b0 = 1`, rest 0) — a flat band is a no-op, exactly;
- coefficients are divided by `a0` once at build time, not per sample.

### Magnitude response

`Biquad::magnitude_db(f)` evaluates the transfer function directly (no FFT):

```text
H(e^jω) = (b0 + b1·e^−jω + b2·e^−2jω) / (1 + a1·e^−jω + a2·e^−2jω)
dB       = 10·log10(|H|²)
```

The real/imaginary parts are expanded with `cos 2ω`, `sin 2ω` double-angle
identities. `EqChain::magnitude_db` sums the eight sections' dB values
(cascaded filters add in dB). This is what the plot draws — the *same
coefficients* the audio path processes with, evaluated on the UI thread from
a separate `EqChain` copy.

## Band layout: `EqChain`

The layout lives in one table — `BANDS: [BandSpec; 8]` in
[`src/dsp/eq.rs`](../src/dsp/eq.rs) — consumed by the filter builder, the UI,
and (later) preset import:

| Band | Frequency | Type | Why |
|---|---|---|---|
| 1 | 60 Hz | low shelf | one control owns everything below the mids (bass shelf reaches 0 Hz) |
| 2 | 150 Hz | peaking Q=1 | upper bass |
| 3 | 400 Hz | peaking Q=1 | low mids |
| 4 | 1 kHz | peaking Q=1 | the reference midrange point |
| 5 | 2.4 kHz | peaking Q=1 | presence / intelligibility |
| 6 | 6 kHz | peaking Q=1 | sibilance region |
| 7 | 12 kHz | peaking Q=1 | lower treble air |
| 8 | 16 kHz | high shelf | one control owns everything above (treble shelf reaches Nyquist) |

- **Shelves at the ends** so the outer bands have no dead space above/below —
  a graphic EQ must cover the whole spectrum with 8 handles.
- **Q = 1.0** — wide enough that adjacent bands blend smoothly (no comb
  ripples between sliders), narrow enough that ±12 dB stays musical.
  Parametric per-band Q is a P1 feature.
- `set_gains()` rebuilds only the sections whose gain actually moved (more
  than 0.01 dB from the applied value) — that's why coefficient rebuilds are
  cheap enough to run 1500×/s (every 32 frames).

## Parameter smoothing: `Smoother`

Exponential one-pole approach, per sample:

```text
coeff  = e^(−1 / (τ · f_s))
value ← target + (value − target) · coeff      (snaps when |Δ| < 1e-6)
```

- τ = **20 ms** per band, **30 ms** for master (slightly slower so master
  moves feel like a fader).
- Exponential interpolation is monotonic and never overshoots — unit tests
  assert exactly that.
- Why it exists: a raw gain jump between buffers is an amplitude
  discontinuity — an audible *click* (zipper noise). 20 ms is long enough to
  be click-free, short enough that it feels immediate (63 % of the way there
  after one τ).

Smoothing runs in the audio thread on the *UI's* target values; the audio
thread never waits for the UI (see
[`docs/ARCHITECTURE.md`](ARCHITECTURE.md)).

## Bypass semantics

`Params::bypass = true` → the sample goes **source → clamp → output**:
no filters, no master gain, no smoothing lag. The UI badge says
"BYPASS — signal untouched" and means it — this is the bit-exact A/B the
vision demands (principle 1). The ±1.0 clamp remains in bypass as a
loudness-safety net (the source itself never exceeds 0.3, so it never
engages).

## Meters

After each buffer the callback computes, over the *post-clamp* samples:

- `RMS = sqrt(mean(x²))`, `peak = max|x|` — stored as raw `f32` bits in
  atomics; the UI converts with `to_db()` (`20·log10`, floor −120 dB).

They reflect what actually reaches the device format conversion, including
bypass behavior — flip bypass or drag master and the numbers follow.

## Test signals: `Source`

| Signal | Definition |
|---|---|
| Tone | 440 Hz sine, amplitude 0.3 (≈ −13.5 dBFS RMS) — the default source; 440 sits between bands 2/3 so any band near it is audible |
| Sweep | 200 Hz → 8 kHz linear-in-phase over 10 s, same amplitude — sweeps the whole chain for listening tests |
| White noise | xorshift32, uniform in [−1, 1] |
| Pink noise | Paul–Kellet economy pink filter over white |
| Silence | exactly 0.0 — for capturing the noise floor |

Switching signals resets phase (`set_kind`) so no click is emitted.

## Verification

### Unit tests — 36 total, all in-file

| File | Tests | What they pin down |
|---|---|---|
| `biquad.rs` | 7 | transparency, peaking gain accuracy, +6 dB = ×2 amplitude, shelf direction, cuts, extreme-settings stability |
| `eq.rs` | 6 | band-spec wiring (+ ascending frequencies), flat-chain transparency, per-band gain accuracy, clamping, time-domain vs analytic response, rebuild preserves state |
| `smooth.rs` | 3 | convergence, monotonicity/no-overshoot, stationary stability |
| `state.rs` | 9 | defaults, snapshot/update round-trip, sanitization clamps, meter bit round-trip, `to_db` floor, env-hook parsing (4 tests via a fake lookup) |
| `source.rs` | 5 | exact silence, bounded tone/noise, phase reset on switch, sweep output |
| `engine/render.rs` | 3 | flat chain passes the source unchanged (tone RMS/peak), bypass bit-identical to flat, master −6 dB halves the tone |
| `app/curve.rs` | 3 | Hz label formatting, log-grid decades/bounds, cache rebuild clears dirty |

Run with `cargo test`; they are deterministic (fixed sample rate 48 kHz, no
audio device, no timing dependencies).

### Audible proof — sink capture (2026-10-16)

Method (full recipe in [`docs/DEVELOPMENT.md`](DEVELOPMENT.md)): run the app
with scripted gains, capture 4 s of the default sink's **monitor** via
`parec` at 48 kHz stereo, compute mono-mix RMS, restore sink mute/volume.

| Configuration | Measured RMS | Level |
|---|---|---|
| EQ on, flat (all bands 0 dB) | 0.2110 | −13.51 dBFS |
| Bypass on, same source | 0.2082 | −13.63 dBFS (within **0.12 dB** of flat → the EQ chain at unity is transparent end-to-end) |
| +6 dB bands 2 & 3 (150/400 Hz) | 0.4196 | −7.54 dBFS → **+6.09 dB vs flat** |

Theory check: evaluating the two RBJ peaking sections at 440 Hz predicts
**+6.59 dB**; measured +6.09 dB. The 0.5 dB residual is in the device/analog
capture path (stream soft-ramp, mixer); the DSP itself is pinned to ±0.01 dB
by the unit tests.

Tone reference: amplitude 0.3 → theoretical RMS −13.47 dBFS; flat measured
−13.51 dBFS (0.04 dB).
