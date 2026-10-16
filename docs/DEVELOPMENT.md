# Development

Everything needed to work on this repo: environment, commands, test hooks,
debug recipes, and conventions. Scope and priorities come from
[`PYRAMID-OF-INTENTS.md`](../PYRAMID-OF-INTENTS.md); the actual work items
live in [`TODOS.md`](../TODOS.md).

## Environment (verified 2026-10)

| Piece | Verified value |
|---|---|
| OS | AlmaLinux 10 (RPM family) |
| Rust | rustc / cargo **1.98.1**, edition 2024 |
| Display | Wayland session + XWayland |
| Audio | PipeWire with PulseAudio compatibility layer; default sink `alsa_output.pci-0000_00_1f.3.analog-stereo` @ 48 kHz |
| Capture tools | `parec`, `pactl`, `arecord` present |
| Distro build deps | `pkg-config`, `alsa-lib`, `libxkbcommon`, xcb/X11 dev packages — all already installed |

Install on a fresh Fedora/AlmaLinux box (roughly):

```bash
sudo dnf install pkgconf-pkg-config alsa-lib-devel libxkbcommon-devel \
    libxcb-devel libX11-devel gcc
```

## Commands

```bash
cargo run                              # build + launch (plays 440 Hz tone by default)
cargo run --release                    # optimized build (use for listening tests)
cargo test                             # all unit tests (no audio device needed)
cargo clippy --all-targets -- -D warnings   # lints gate every change
cargo fmt                              # apply formatting
cargo fmt --check                      # verify formatting
cargo tree --prefix none | sort -u | wc -l            # dependency count (182 today)
cargo tree -e normal --prefix none | sort -u | wc -l  # normal deps only (170)
```

**Quality gates** (must all be green before a task is checked off in
`TODOS.md`): `cargo fmt --check` · `cargo clippy --all-targets -- -D warnings`
· `cargo test` · a 10-second window smoke run.

## Environment variables (test hooks)

Read once at app construction in `EqApp::new` — parsing lives in
[`Params::with_env_overrides`](../src/state.rs) so it is unit-testable with a
fake environment lookup — they exist so proof scripts and future CI
can configure the app without synthesizing clicks:

| Variable | Format | Effect |
|---|---|---|
| `EQ_INITIAL_GAINS` | 8 comma-separated floats, e.g. `0,6,6,0,0,0,0,0` | initial band gains in dB (must be exactly 8 values, otherwise ignored) |
| `EQ_INITIAL_BYPASS` | `1` / `true` / `yes` | start in bypass (bit-exact A/B) |

```bash
EQ_INITIAL_GAINS=0,6,6,0,0,0,0,0 cargo run   # +6 dB on bands 2 and 3
EQ_INITIAL_BYPASS=1 cargo run                 # untouched signal
```

Both are read after `SharedState` is created and before the audio stream
starts, so there is no audible jump at launch.

## Tests

36 unit tests, all in-file (`#[cfg(test)] mod tests` beside the code they
pin), all deterministic and device-free:

| File | # | Covers |
|---|---|---|
| `src/dsp/biquad.rs` | 7 | transparency, peaking gain accuracy, +6 dB = ×2 amplitude, shelf direction, cuts, extreme-settings stability |
| `src/dsp/eq.rs` | 6 | band-spec wiring (+ ascending frequencies), flat-chain transparency, per-band gain accuracy, clamping, time-domain vs analytic response, rebuild preserves state |
| `src/dsp/smooth.rs` | 3 | convergence, monotonicity/no-overshoot, stationary stability |
| `src/state.rs` | 9 | defaults, snapshot/update round-trip, sanitization, meter bit round-trip, dB floor, env-hook parsing (4 tests via a fake lookup) |
| `src/dsp/source.rs` | 5 | exact silence, bounded tone/noise, phase reset on switch, sweep output |
| `src/engine/render.rs` | 3 | flat chain passes the source unchanged, bypass bit-identical to flat, master gain scales the tone |
| `src/app/curve.rs` | 3 | Hz label formatting, log-grid decades/bounds, cache rebuild clears dirty |

Add tests next to the code, name them after the property they prove
(`flat_chain_is_transparent`), keep them free of wall-clock and device
dependencies.

## Debug recipes

### Is the app running and streaming?

```bash
./target/debug/rust-egui-audio-eq </dev/null >/tmp/eq.log 2>&1 &
sleep 3
pgrep -af debug/rust-egui-audio-eq          # process alive?
pactl list short sink-inputs                 # a sink input appears (PipeWire ALSA [rust-egui-audio-eq])
kill %1                                      # or pkill -f 'debug/rust-egui-audio-eq'
```

Redirect all three fds (`</dev/null >log 2>&1`) — a backgrounded GUI process
holding the terminal pipe makes shells and CI runners hang.

### Audible proof: capture what reaches the sink

The method behind the numbers in [`DSP.md`](DSP.md):

```bash
# 1. note current mute/volume so you can restore them
pactl get-sink-mute @DEFAULT_SINK@          # e.g. "Mute: yes"
pactl get-sink-volume @DEFAULT_SINK@ | head -1

# 2. unmute — otherwise the monitor captures silence (0% / muted sink = quiet monitor)
pactl set-sink-mute @DEFAULT_SINK@ 0
pactl set-sink-volume @DEFAULT_SINK@ 20%

# 3. run the app with scripted gains, capture the monitor for 4 s
MON=$(pactl get-default-sink).monitor
EQ_INITIAL_GAINS=0,6,6,0,0,0,0,0 ./target/debug/rust-egui-audio-eq \
    </dev/null >/tmp/eq.log 2>&1 &
APP=$!
sleep 3
timeout 4 parec --format=s16le --rate=48000 --channels=2 --device="$MON" --raw \
    > /tmp/eq-boost.raw
kill $APP

# 4. RESTORE — do not leave the user's sink unmuted
pactl set-sink-volume @DEFAULT_SINK@ 20%   # ← your saved values, not these
pactl set-sink-mute @DEFAULT_SINK@ 1       # ← your saved values
```

RMS of the capture (s16le, stereo interleaved, mono mix):

```python
import array, math
a = array.array("h")
data = open("/tmp/eq-boost.raw", "rb").read()
a.frombytes(data[: len(data) // 2 * 2])
s = [(a[i] + a[i + 1]) / 2 / 32768.0 for i in range(0, len(a) - 1, 2)]
rms = math.sqrt(sum(v * v for v in s) / len(s))
print(f"{rms:.4f}  ({20 * math.log10(rms):+.2f} dBFS)")
```

Expected reference values (dev machine, tone = 440 Hz @ 0.3 amp):

| Configuration | dBFS |
|---|---|
| theoretical tone RMS (0.3/√2) | −13.47 |
| flat EQ measured | −13.51 |
| bypass measured | −13.63 (≈ flat → chain is transparent) |
| +6 dB bands 2 & 3 | −7.54 (**+6.09 dB** vs flat; RBJ math predicts +6.59) |

Run captures when nothing else is playing (`pactl list short sink-inputs`
empty) or the result is contaminated.

### No window appears / app exits

- Check the log (`/tmp/eq.log`) — stream errors are strings, not panics.
- `xkbcommon: couldn't find a Compose file` is a harmless locale warning on
  this machine, not an app error.
- If eframe fails hard, `main()` returns `Err` and the process exits non-zero
  with the error printed.

## Conventions

- **Edition 2024 + rustfmt defaults** — import order is rustfmt's (types
  before lowercase items); never hand-sort imports.
- **Doc comments (`///`) on public items**, no narrating inline comments —
  the code should read without them.
- **Errors are strings on the status bar**, never `panic!` in audio or
  startup paths (principle 5).
- **Pure DSP stays pure** — `src/dsp/` may not depend on egui, cpal, or
  `std::sync`; keep it unit-testable without a device.
- **Sanitize at the boundary** — new `Params` fields need clamping in
  `SharedState::update` / `Params::sanitized`.
- **Tags** — every task in `TODOS.md` carries `[Ex/Gx]`; new work gets a tag
  before it gets code.

## How-tos

| Task | Where to look |
|---|---|
| Add a band / change frequencies | `BANDS: [BandSpec; 8]` in `src/dsp/eq.rs`; sliders in `src/app/bands.rs` iterate `BANDS` automatically; env-hook parsing in `src/state.rs` takes exactly 8 values |
| Add a test signal | `SourceKind` enum + `ALL` + `label()` + `next()` arm in `src/dsp/source.rs`; the toolbar row picks it up from `ALL` |
| Retune smoothing | `BAND_TAU_S` / `MASTER_TAU_S` in `src/engine/render.rs` |
| Add a filter type (notch, …) | constructor in `src/dsp/biquad.rs` + a `BandKind` arm in `make_filter` (`src/dsp/eq.rs`) + a test in both |
| Add a UI panel | new file in `src/app/`, call it from `EqApp::ui` in `src/app/mod.rs` |
| Change metering math | `next_sample`/`take_levels` in `src/engine/render.rs`; `to_db()` in `src/state.rs` |
| Debug a coefficient bug | assert `magnitude_db` (analytic) against a sine RMS run — the pattern is already in `biquad.rs` tests (`sine_rms_gain_db` helper) |

## Working agreement (from TODOS.md)

One task in flight at a time; `[~]` marks paused work with a date and
blocker; every completion names its `[Ex/Gx]`; quality gates green before a
task counts as done.
