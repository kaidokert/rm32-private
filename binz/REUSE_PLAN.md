# binz — Control-Brain Reuse Plan (don't rewrite the BEMF science)

*The minz bench agent's consumption recipe for lifting the proven sensorless
BLDC control brain instead of re-deriving it. Bottom line: this is a **HAL job,
not a brain rewrite** — ~two weeks, not the from-scratch month. The months of
pain (BEMF zero-cross, commutation timing, desync/re-lock, filter/blank/advance,
ramp, guards, the state machine) are all behind a clean ~12-trait seam. You
write a gate layer you mostly already have, one ADC-BEMF adapter, and an
M0-atomics shim.*

Paths are relative to `binz/`: `../minz/core` (minz-core, standalone brain),
`../rm32` (portable core, same HAL contract), `../rm32_stm32/src/mcu_g071`
(**a complete G0 register-level impl of these traits — your reference**).

---

## The decision to make first

Consume one of two brains; both expose the **same HAL trait contract**
(`../rm32/src/hal.rs`, which `minz-core` copied verbatim):

- **`minz-core`** (`../minz/core`) — clean, zero-dependency, standalone, the
  tight AM32 transliteration. *You* write the G0 trait impls (using mcu_g071 as
  the reference). **Recommended** — binz is a fresh clean stack like minz was.
- **`rm32` core** (`../rm32`) — bigger, more coupled, but its `mcu_g071` port
  already wires these traits to G0 silicon. Less to write, more to inherit.

Either way, **`../rm32_stm32/src/mcu_g071/` is your register reference** for the
G0 wiring. This plan assumes minz-core-as-dep + mcu_g071-as-reference.

---

## The trait checklist (~12 seams, source `../minz/core/src/am32_hal.rs`)

Effort tags: **HAVE** = you already wrote the equivalent, just wrap it ·
**SHIM** = thin adapter · **NEW** = genuinely new integration.

| trait | methods | binz maps to | effort | G0 reference |
|---|---|---|---|---|
| `PwmOutput` | set_duty_all, set_arr, set_psc, set_compare1/2/3, gen_update, dead_time | `spin-pwm.rs` raw-PAC TIM1 | **HAVE** | `mcu_g071/pwm.rs` |
| `PhaseOutput` | com_step, all_off, full_brake, all_pwm, proportional_brake, pulse_toggle | six-step drive + `stage::force_safe` (=all_off) | **HAVE** | `mcu_g071/pwm.rs` |
| `IntervalTimer` | count, set_count | a free-running timer for commutation-interval measure (TIM2) | **HAVE** | `mcu_g071/init.rs` |
| `ComTimer` | set_and_enable, dis/enable_interrupt | one-shot next-commutation timer (**TIM14** = AM32 COM_TIMER slot) | **HAVE** | `mcu_g071` timers |
| `ComTimerExt` | com_set_arr, com_clear_flag | same timer, bare ARR + UIF ack | **HAVE** | — |
| `Recorder` | record, freeze | your `blackbox` module | **HAVE** | — |
| `Cs` | free(f) | `cortex_m::interrupt::free` | **HAVE** | — |
| `LoopTimer` | clear_flag | your TIM6 `harvest` ISR ack | **HAVE** | — |
| `MotorHal` | bundles Pwm/Comp/Phase/Interval/Com | provided `Motor<…>` struct or your own bundle | **HAVE** | — |
| `InjAdc` | inj_read → (phase_a, phase_b, current, vbat) | **adapter** over `harvest` (map VPH/IS/VM into the 4-tuple) | **SHIM** | `mcu_g071/adc.rs` |
| `Comparator` | output_level, set_step(step,rising), change_input, en/mask_interrupts | **software comparator over VPH ADC** (see item 2) | **NEW** | `mcu_g071/comparator.rs` (as HW-comp — adapt) |
| `CompExti` | exti_pending, clear_pending | N/A on the ADC-polling path — stub / software-model | **NEW (minor)** | — |

Nine of twelve are **HAVE** — wrap code that already runs. That's the whole
point of the `run_tick`/trait architecture: a new board is a HAL implementation.

---

## The three integration items (all bounded)

### 1. M0 atomics — you hit this first (~an afternoon)
minz-core uses ~6 atomic **RMW** ops (`fetch_add`/`fetch_max`…) in its cluster
structs (`am32.rs`, `am32_control.rs` ×3, `am32_loop.rs`, `zct_trace.rs`).
Cortex-M0+ (thumbv6m) has **no native atomic RMW** → won't compile as-is.
- **Fix:** add `portable-atomic` with the `critical-section` feature; swap the
  `core::sync::atomic` types for `portable_atomic` equivalents (or gate them).
- **Already solved in the lineage:** rm32 cross-builds for G071 *and* F051 (both
  M0), so the pattern exists — copy it. Not a redesign.
- **Gate:** `cargo build --target thumbv6m-none-eabi` of the brain crate + its
  host `cargo test` both green.

### 2. ADC-BEMF software comparator — the one genuinely-new piece
rm32/minz sense BEMF with a **hardware comparator + EXTI** (`mcu_g071/
comparator.rs`). binz routes VPH through dividers to **ADC** pins (PB1/PB0/PB2),
no comparator. So:
- Implement `Comparator::output_level()` as **`vph_adc[floating] > neutral`**
  (neutral = VM/2 or the measured star-point), `set_step(step, rising)` selects
  the floating-phase channel + edge sense, `change_input` applies it.
- Drive minz-core's **polling commutation path** (`old_routine` /
  `zcfoundroutine`), **not** the EXTI/interrupt path. `CompExti` becomes a stub
  (no hardware pending bit). The entire zero-cross → commutation → desync brain
  *above* this trait is reused unchanged.
- **10-minute check that could shrink this:** can any VPH divider node (or a
  phase pin) reach the G071 internal **COMP1/COMP2** inputs? If yes, use the
  hardware comparator and reuse `mcu_g071/comparator.rs` closer to verbatim.
- **Watch:** your VPH ADC sample is *instantaneous* (Scar #1) and the `harvest`
  sampler is phase-swept for DC-link averaging — BEMF sensing needs the sample
  **synchronized to the PWM OFF window** of the floating phase, a different
  cadence than the averaging sweep. Budget a dedicated BEMF sample point.

### 3. Wrap existing HAL code in the trait shapes (~days)
Your `spin-pwm.rs` / `harvest` / `blackbox` / `force_safe` already *are* the
impls — wrap them in the trait signatures above, build the `Motor<…>` bundle,
thread the `Observer<…>` bundle. Use `mcu_g071/{pwm,adc,init}.rs` as the G0
register reference where your existing code doesn't already cover it.

---

## Suggested order of operations

0. Decide dep source (minz-core recommended). Add it as a path dep.
1. **Make the brain compile for thumbv6m** — resolve item 1 (M0 atomics). Run
   minz-core's host `cargo test` (it's MCU-agnostic; proves the brain is intact
   in your build).
2. **Wrap the HAVE traits** (item 3) — PWM/phase/timers/blackbox/cs/looptimer +
   the InjAdc shim. Build the firmware; it should link.
3. **Implement the ADC-BEMF `Comparator`** (item 2) and select the polling path.
4. **Run the brain on the bench** — spin via the reused commutation, guards from
   your `stage::force_safe`, telemetry via your existing `telem`/`blackbox`.
5. **Validate against the fork laser tacho** (your only external oracle — no
   Halls, no encoder on this motor): reused-brain eRPM vs measured prop RPM.

---

## Caveat — what reuse does and doesn't buy

- **Buys:** the six-step BLDC sensorless brain, host-tested, with every bug we
  already killed (phantom desyncs, ZC-polarity bias, NDTR/one-edge class,
  stuck-rotor stall-rate race, harness/firmware divergence). Months.
- **Doesn't buy FOC.** This brain is AM32 six-step BLDC. If the endgame is
  **FOC on a Maxon PMSM**, that's a different controller — reuse covers the BLDC
  bench path, not FOC. (A Maxon *with an encoder* is still the fast per-commutation
  oracle to validate this brain's BEMF timing — see GRAYBEARD_FAST_RAMP.)

---

*Ping the minz agent (via the operator) to review your `Comparator`/`InjAdc`
adapters against minz-core's expectations before you bench them — the seam
fitting is where a reuse job quietly turns into a rewrite if the contract drifts.*

---

## ADC-BEMF sensing — design requirements (validated architecture, 2026-09-05)

**Hardware comparator is NOT viable for full 3-phase BEMF on this board.**
VPH pins vs G071 COMP inputs: PB2(phase C)→COMP1_INP ✓, PB1(phase A)→
COMP1_INM ✓, but **PB0(phase B) reaches no comparator**. Six-step senses
all three floating phases in turn, so a 2-of-3 comparator path can't do it.
**ADC-BEMF is mandatory** — but it must be built correctly or it produces a
catastrophic false negative ("ADC-BEMF doesn't work here"). Four rules
(graybeard):

1. **Sample the floating phase SYNCHRONIZED to the PWM-OFF window** — NOT
   through `harvest`'s de-cohered sweep (that reconstructs the DC-link
   average, the exact opposite of what BEMF needs; through it BEMF is
   switching noise). Fire the ADC at a fixed mid-OFF point of the PWM
   cycle on the floating phase (e.g. TIM1 CCR-triggered injected conv).
2. **Virtual neutral = mean(VPH channels)** (or mean of the two driven
   phases), NOT VM/2 blindly (drifts under load). ZC = floating phase
   crossing that neutral.
3. **Run detection INSIDE the measured synced band (≤~5% duty).** BEMF ∝
   speed and coast-BEMF was ~180 mV (near ADC noise floor); below the sync
   edge the rotor truly follows forced commutation so BEMF is real.
   Force-commutating above sync is the desync/heater trap again.
4. **Capture the BEMF RAMP, not one threshold hit** (single VPH sample is
   instantaneous noise — Scar #1). Oversample across the step; verify the
   ZC (a) sits ~30° before the next commutation, (b) repeats step-to-step,
   (c) tracks speed. "Saw a crossing" ≠ proof; "right phase, repeatable,
   tracks speed" = proof.

If all four hold, the ADC-BEMF seam is validated and minz-core's
filter/blank/advance handles noise from there — don't hand-roll ZC logic.

**Status:** `examples/sixstep-sense.rs` drives forced six-step (register-
level TIM1 per-phase HIGH/LOW/FLOAT). Its BEMF columns are INVALID (they
read the de-cohered `harvest` VPH — rule #1 violation) — it's spin-verify
only. Open: (a) confirm the rotor spins under forced six-step (near-zero
current at 5% is ambiguous: efficient-sync vs not-driving — needs operator
eyes), then (b) build the PWM-OFF-synchronized ADC BEMF capture per above.

## Validated primitives (2026-09-05) — foundation for the BEMF detector

All determined from instruments, no operator eyes:
- **`examples/sixstep-sense.rs`**: register-level forced six-step drive
  (HIGH=PWM1/CCxE, LOW=force-inactive+CCxNE, FLOAT=CCxE=0/CCxNE=0 w/ OSSR).
  Verified the phases energize (high phase reads 666/493 pin-mV when the
  de-cohered sampler happens to catch ON) and the ROTOR SPINS (coast-BEMF
  self-check = 125 mV pk-pk term after a forced-six-step ramp to ~67 Hz-e).
- **`examples/adc-sync-check.rs`**: minimal software-triggered ADC, timed
  by polling TIM1 CNT. PWM-synchronized sampling CONFIRMED: driven phase A
  reads **712 pin-mV (≈VM) in the ON window vs 0 in the OFF window**, clean
  and repeatable. Two ADC lessons: (1) the ON pulse at low duty (5%=2.5 us)
  is shorter than 160.5-cyc sampling (~10 us) — use SHORT sampling to catch
  ON, LONG (160.5) for the high-Z floating phase in the wide OFF window;
  (2) in the OFF window the driven "HIGH" phase ALSO floats (its INH=INL=0),
  so only the "LOW" phase is a defined 0 V there.

### Next build: integrated BEMF detector — design notes
Combine the two: six-step spinning + OFF-window synchronized reads of the
floating phase, LONG sampling. Open design choice = the **neutral
reference** (graybeard #2): in the OFF window two phases float, so
mean-of-driven isn't clean. Options to try: (a) sample during the ON
window instead (both non-floating phases defined, floating phase shows
BEMF+offset) — AM32's classic scheme; (b) resistor/DAC-generated VM/2
reference; (c) mean of all three VPH taken at the same instant. Capture
the RAMP across the step and verify ZC sits ~30 deg before commutation,
repeatable, tracking speed (graybeard #4). Then hand noise-handling to
minz-core's filter/blank/advance — don't hand-roll ZC logic.

### Integrated detector result (2026-09-05) — `examples/sixstep-bemf.rs`
Six-step spinning + OFF-window synchronized floating-phase capture + mean-
of-3-VPH neutral. Runs safe (current abort armed, never tripped). Captured
1788 floating-vs-neutral samples (`scratchpad/bw.txt`). Finding: the
floating phase DOES swing and cross the neutral each step, BUT the swings
are ±300 pin-mV (≈±5 V term) — far bigger than the ~11 mV of true BEMF at
synced-band speed. So **drive/commutation coupling dominates the raw
signal; the small synced-band BEMF is near the artifact floor.** Two known
causes: (1) in the OFF window the driven "HIGH" phase floats too, so the
mean-of-3-VPH neutral is ill-defined/jumpy; (2) BEMF ∝ speed and the
synced band is slow. **Conclusion: the raw signal exists but a clean ZC
needs filter/blank/advance + a proper neutral — i.e. minz-core's brain,
NOT a hand-rolled threshold.** Next-iteration levers: fixed VM/2 (DAC or
computed) neutral instead of mean-VPH; sample in the ON window with
medium sampling (fits the pulse, star point stiff at VM/2); and feed the
raw stream to minz-core's ZC filter rather than thresholding here.

## STEP 1 DONE (2026-09-05): minz-core builds for thumbv6m (M0+)
The control brain compiles for binz's target. The ONLY blocker was atomic
RMW (fetch_add/swap don't exist on thumbv6m): 7 errors. Fix (mechanical,
backward-compatible, matches rm32's `shared_state.rs`):
- `minz/core/Cargo.toml`: add `portable-atomic = { version="1",
  default-features=false }`.
- Swap `use core::sync::atomic::{Atomic*}` -> `use portable_atomic::{...}`
  in am32.rs / am32_hal.rs / am32_loop.rs / zct_trace.rs, and the
  fully-qualified field types in am32.rs (Ordering-only imports stay core).
- binz enables it: `portable-atomic = { ..., features=["critical-section"] }`
  (binz already has cortex-m critical-section-single-core).
Verified: minz-core's 83 host tests still pass (backward compat), and binz
links minz-core for M0. `binz::minz_core` re-exports it. NOTE: this edited
the sibling's crate — a pure portability swap, no behavior change; flag to
the minz agent for awareness. NEXT (step 2): wrap binz's validated
primitives (six-step drive = PhaseOutput/PwmOutput, adc-sync read = InjAdc/
Comparator adapter, stage::force_safe = all_off, blackbox = Recorder) in
minz-core's HAL traits and thread run_tick.
