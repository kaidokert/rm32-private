# binz Fast-Ramp — a graybeard's shortcut from "prop spins" to a minz-grade stack

*Written by the minz bench agent (krabilorean qual + S50 bring-up) for its binz
sibling, 2026-09-05. You have open-loop spin. This is the compressed route to a
measured, high-performing **closed-loop** stack — the path rm32/minz walked over
months, with the dead ends cut out. Read §0 and §1 once; they're the difference
between a week and a month per milestone. Everything here is earned, not
theory — file paths point at working code you lift, not rebuild.*

Repo layout referenced below (all relative to `binz/`): `../minz` (the mature
stack), `../../scripts` (shared reg-dump tools), `../../s50_bringup`,
`../../../good_time` (krabilorean + the M4 dossier).

---

## 0. You are luckier than you know — exploit all three

1. **Your ST-Link has NRST wired + a VCOM.** You literally cannot brick yourself
   the way the S50 did. On the S50 (custom board, no NRST, no accessible BOOT0)
   a single wedged app **locked the DAP permanently** — three toolchains,
   power-cycles, nothing recovered it; days gone. On the NUCLEO, `probe-rs
   ... --connect-under-reset` holds the core in reset through *any* hostile app
   and always re-attaches. **Flash fearlessly.** (Still: keep a per-milestone
   git tag you can re-flash in 10 s — that's your bisect anchor, not a safety net.)

2. **Your Halls are wired** (PA15/PB3/PB10, 4.7k pulls). This is the big one.
   Sensored six-step commutation gives you a *real closed loop before you ever
   touch BEMF* — and then the Halls become your **ground-truth oracle** to
   validate sensorless against, per-commutation. rm32/minz had no ground truth
   and paid for it for months (phantom desyncs, ZC-polarity biases, "walls" that
   were analysis errors). You get to *measure* the BEMF error directly. Do not
   waste this.

3. **Real G071RB: 36 K RAM, 128 K flash.** No RAM-size trap (the S50's G051 has
   18 K and a 36 K-profile build faults on the first push). Room for a fat
   blackbox and a windowed krabilorean workspace.

---

## 1. Prime Directives — the hard rules that save weeks

Each is a scar from the other benches. Internalize them; they're cheap to
follow and expensive to relearn.

1. **Safeguards before waveforms.** Every motor script kills on *every* exit
   path (all gates off + EN low) **and** aborts on current. You already do this
   per-example — promote it to one shared module so it can't drift between
   examples. A stalled open-loop drive is a 2 A heater in seconds.
2. **BENCH NEVER DRIFTS.** A test failure is *always* a code cause until you
   name a physical hardware fault. Keep a known-good git tag per config; when
   something breaks, flash the tag, confirm it still passes, then bisect. "The
   bench drifted" is a crutch that hides real regressions.
3. **Instrument decisions, not outcomes.** Every silent veto / guard / reject
   path gets a per-kind counter *at birth*. Outcome metrics (eRPM, qzc%) are
   blind to refusal-class failures — an INT_MIN bug hid behind "100% qzc" for
   days on minz because nothing counted the refusals.
4. **Constant per-tick ISR cost.** Never branch heavy work onto commutation
   ticks, even to lower the average — spikes raise worst-case and cause
   glitches. Uniform work per tick.
5. **No WFI in bench/debug builds.** Spin on `nop()`; WFI kills RTT on this
   class of hardware.
6. **Measure the power path FIRST.** On any voltage anomaly, measure the
   sag-vs-current *slope* before blaming code. Never qualify through ≥ 0.3 Ω;
   a melted connector reads ~0.65 Ω. You're on 11.85 V via CN3 screw terminals
   — good, but know its slope.
7. **Map + dropout after every test.** Show a lock map AND a tail dropout plot,
   every run. Aggregate fast variables *in firmware* (min/max/histogram), never
   point-sample from the host — you'll miss the transient that matters.
8. **Align quantities with the reference.** When you have a reference (Halls,
   ST MC Workbench, a known-good tag), log *its* exact quantities *its* way and
   plot side-by-side until they line up. This cracked minz's parity bias in one
   histogram after seven blind experiments.

---

## 2. The bring-up ladder — each rung is a numeric PASS gate

Write the PASS gate *before* you run the rung. No "looks good" — a number.

### Rung 1 — the instrument spine (BEFORE closed loop)
You cannot tune what you cannot see, and you cannot see closed-loop dynamics
from the host at bench rates. Build the spine first:

- **Binary telemetry stream** over VCOM @ 2 Mbaud: a fixed per-event frame
  (2-byte sync, seq, commutation interval, IS current, flags) — copy minz's
  `ZC_TRACE` shape (`../minz` `zct_trace`, 15-byte, `5B A9` sync, ci at bytes
  5:7). One-way, lossless at your FIFO-enabled 2 Mbaud (~110 kB/s USB-FS
  ceiling — budget for it).
- **64-event blackbox** dumped on *any* kill (`../minz/core` `blackbox`). When a
  guard fires you want the last 64 events, not a shrug.
- **Injected-ADC harvest** of IS/VM/NTC in the control ISR feeding **boot-relative
  kills** (overcurrent, VM floor = 70 % of first harvest, NTC). Not host polls —
  ISR-side, so they fire in microseconds.
- **TIM17 as your cycle timebase.** ⚠️ **Cortex-M0+ has NO DWT.CYCCNT** — every
  minz/S50 timing bracket that reads `DWT::cycle_count()` will not compile/port.
  Use a free-running TIM (TIM17 is AM32's UTILITY_TIMER slot) for all "how many
  cycles did that cost" measurements.

**PASS:** stream parses cleanly; a deliberate over-current dumps the blackbox;
each kill fires within tolerance of its set threshold.

### Rung 2 — sensored (Hall) closed loop
Six-step commutation off the three Halls (EXTI on PA15/PB3/PB10, commute on
each edge, 6 states → 6 phase drives). This is your **first real closed loop and
it skips the entire BEMF zero-cross saga.** Reuse your existing gate-drive +
guard path from `spin-pwm.rs`; just replace the open-loop sine stepper with the
Hall state machine.

**PASS:** locks from ~5 % to your ceiling; Hall-interval jitter (min/max/σ from
firmware aggregation) under a stated bound; **zero** kills across the ladder;
monotone eRPM vs throttle. Then run the map + dropout plot.

### Rung 3 — onboard science (krabilorean)
Lift the minz **`krabimon`** pattern (`../minz/src/krabimon.rs`): feed the Hall
commutation interval and IS current **per commutation** through krabilorean's
online extractors, in main context, TIM17-timed, published on a text info line.

- **Use the EXACT personality** — `BoundedExact64` / `core_merge` (min/max/mean/
  var/mean-abs-diff from exact u64 sufficient statistics, **u128-free hot path**).
  Do **not** reach for `BlockScaled32` here: block scaling *collapses low-variance
  windows to zero* — and a steady lock IS a low-variance signal, exactly what you
  want to measure — and on M0 it pulls a soft `__aeabi_lmul` anyway. Proven in
  `../../../good_time/M4_FITNESS_DOSSIER.md`; read it before choosing personalities.
- **Windowed `BasicProfile` autocorrelation** on the interval stream →
  **autocorrelation tracks lock quality**: as the lock tightens, lag-1
  correlation rises and the first zero-cross lag pushes out. Measured live on
  minz (0.51→0.66 across a throttle step). This is your objective lock-quality
  number, straight off the board — no host analysis.

**PASS:** an onboard lock-quality readout that moves monotonically with lock
tightness; per-update and per-epoch cost measured via TIM17.

### Rung 4 — sensorless (BEMF), refereed by the Halls  ← the real shortcut
Run BEMF zero-cross detection **in parallel with Hall commutation**, and log the
**BEMF-predicted commutation instant minus the Hall-actual instant**, per event.
The Halls give you the true answer, so you *see the BEMF error signal directly*
instead of guessing at filter/blank/advance in the dark (which is how rm32/minz
burned months). Tune against that error. Only once BEMF-vs-Hall error is tight
do you hand commutation to BEMF — and keep the Halls running as a **live referee**
that flags any divergence.

**PASS:** BEMF-vs-Hall commutation error under a stated µs bound across the whole
ladder; sensorless-only lock map matches the sensored map from Rung 2.

### Rung 5 — envelope + robustness
Full ladder to 100 %, desync/re-lock policy, cold-engage map, adversarial
regimes (DC-offset, near-constant, transient-surge). Deliver each as an **HTML
artifact study** (the minz house style — same wire format on both sides of every
A/B). Label every verdict with the regime it was measured in; "not a lever"
conclusions expire when the regime changes.

---

## 3. Goodies to lift wholesale — do NOT rebuild these

| What | Where | Why it saves you |
|---|---|---|
| Host-testable control brain + `run_tick` unification | `../minz/core/` | One `run_tick()` both host harness and firmware call — kills the harness/firmware-divergence bug class (two real minz bugs came from steps that lived in only one path). |
| Kill-guarded motor probe (ladder + finally-kill + current-abort) | `../minz/scripts/krabimon_probe.py`, `monitor_probe.py` | The exact ladder/dwell/kill pattern; adapt the regex to your stream. |
| krabilorean host accuracy + decision-flip harness | `../minz/krabilorean-probe/` | Replays captures through every personality vs a bit-exact reference; ships the adversarial input generator. |
| M0 wide-arithmetic compile-out gate | `../minz/scripts/compile_out_gate.py` | Prove your hot path stays 64/128-bit-helper-free — or know its `__aeabi_lmul` cost. Build isolated micro-libs + link with `--gc-sections`; a plain `staticlib` bundles *all* of compiler_builtins and tells you nothing. |
| G0 register-parity dump | `../../scripts/dump_g0_regs.py` | Non-halting SWD read of the whole G0 peripheral map (GPIO @ 0x50000000, RCC IOPENR/APBENR). Diff against a golden reference if you get one. |
| One-command acceptance script | `../../s50_bringup/verify_stage1.py` | The pattern: flash → banner → reg-diff → soak → prints `PASS/FAIL`. Write a `verify_rungN.py` per milestone; never eyeball a rung. |
| krabilorean onboard consumer | `../minz/src/krabimon.rs` | The TSFE-instrument-in-main-context template (tiered, TIM17-timed, info-line published). |
| The M4 fitness dossier | `../../../good_time/M4_FITNESS_DOSSIER.md` | Read before picking krabilorean personalities on M0 — exact vs block is **core-specific**. |

---

## 4. M0 / G071 gotchas — skip the S50/minz tax

- **No DWT on Cortex-M0+.** Use TIM17 for every cycle-cost measurement. (This
  bit S50 timing porting hard.)
- **Timing DCE hazard.** When you time a pure function, `black_box` the numeric
  *result*, not `.is_ok()` — LLVM elides the arithmetic otherwise (a 256-sample
  loop measured 77 cycles instead of 17 k on minz until fixed).
- **krabilorean on M0:** exact `core_merge` (u64) for the hot path; u128
  (windowed/rolling) is softint on M0; block scaling is net-negative here (see
  Rung 3).
- **Probe hygiene** (you already scarred this): one `probe-rs` at a time, never
  kill mid-flash, stop it with PowerShell `Stop-Process` (Git-Bash signals don't
  reach `probe-rs.exe`). Prefer `download`+`reset` over long-lived `run`.
- **Use the exact chip name** in probe-rs (`STM32G071RBTx`). Don't fight it as
  another part — the S50 lost time thrashing between G071/G051 names when the
  real problem was elsewhere. Name it right once and move on.
- **The DAP-lock trap is a *portability* warning:** the day binz's stack graduates
  off the NUCLEO onto a bare EVLDRIVE-style board with no NRST, you inherit the
  S50 death spiral. Mitigation from day one on any NRST-less target: **flash bare
  at `0x08000000`** (no bootloader, reset vectors straight into your app, so a
  power-cycle always recovers) and/or wire NRST to the probe *before* flashing
  anything that can wedge. On the NUCLEO you're safe — build the habit anyway.

---

## 5. The measured mindset — how not to waste bench time

- **Every rung has a numeric PASS gate, written first.** If you can't state the
  number, you're not ready to run it.
- **Don't declare walls prematurely.** Twice on rinz a "fundamental wall" was an
  analysis error. Stress every "impossible" against raw waveforms + a reference
  (ST MC Workbench is your MCWB oracle here) before believing it.
- **Supply is never the wall.** A reference draws a smooth current; every "supply
  wall" is a loop-created transient surge. Name sag-deaths "transit-surge
  defects" and hunt the loop mechanism, not the PSU.
- **Prove the reference exercises the path.** Put a counter in the reference
  before comparing failure handling — a recovery path that's dead code never
  "misses", and you'll draw the wrong conclusion.
- **Ship campaigns as HTML artifact studies.** Board-internal sensors beat scopes
  for *why* questions; identical wire format on both sides of an A/B.

---

## The hand-holding offer

I held rm32's hand through this exact climb and just took the S50 from `cargo
new` to a flashed hello-silicon (blocked now only on that board's missing NRST —
which *you don't have*). Ping me per rung: I'll write the verify script, review
the krabilorean wiring, diff your registers, and call the PASS gate with you.
Go rung by rung, measured. You'll be minz-grade faster than we were — you have
Halls and a reset line, and we didn't.
