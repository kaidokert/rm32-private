# rm32 generic core: design for a chip-agnostic `rm32/`

Status: DRAFT r3, 2026-10-25. Not started. Gated on #7 (L431
re-qualification plus the two-board side-by-side). Folds in #2, #3, #4,
#5, #6 and the chip-shaped HAL findings from the 2026-10-24 three-reviewer
audit of `rm32/`. r2 incorporates `docs/generic_core_design_review.md`;
section 12 records what was taken from it and what was rejected.

## 1. Goal

`rm32/` should be clean, canonical embedded Rust that a port to ANY chip
family can drive by implementing traits and supplying constants: another
STM32, a GD32, an RP2040 (PIO), a CH32V / ESP32-C3 (RISC-V), a Microchip
SAM, an nRF. A new port never edits `rm32/`.

The test of "done": a third, deliberately unlike-STM32 port compiles and
passes the blackbox vectors without a single change in `rm32/` (section 9).

### Non-goals

- Changing control behaviour. Every step below is a pure refactor against
  the AM32 oracle (blackbox vectors, C harness counters, bench re-qual).
  Behaviour changes (the 2048 duty scale, #1; dropping rig policy, #6) are
  separate decisions made after this lands.
- Async/RTIC/Embassy. The core stays executor-free: synchronous ISR entry
  points plus a main-loop tick. A port may wrap them in whatever it likes.
- Runtime polymorphism or runtime configuration of static facts. No `dyn`
  on any ISR path; nothing known at build time is stored in RAM or branched
  on at run time.

## 2. Where it stands (audit summary)

What is already right: `rm32/` touches no registers, has no target `cfg`
gates, no target-tied dependencies, and the control logic itself is
portable. The contamination is in the boundary, in hidden assumptions, and
in core logic that leaked OUT into the STM32 port.

| Area | Today | Why it fails on a non-STM32 chip |
|---|---|---|
| PWM / bridge | `PwmOutput` = TIM1 register set (`hal.rs:16`). Every port re-encodes AM32's step → phase map (`rm32_stm32/src/phase.rs`); the `comp_pwm && !old_routine` drive-mode decision lives in the port (`phase.rs:302-324`). | The commutation table and the drive-mode policy are motor control, duplicated per port and untested there. |
| BEMF sensing | `Comparator::set_step / change_input / enable_interrupts / mask_interrupts` (`hal.rs:28`). `BemfPins` holds packed INMSEL/INMESEL values (`board.rs:7`). The half-interval ZC gate, pre-ZC glitch clear, post-ZC "camp" and SWIER re-pend live in the port's COMP ISR (`mcu_l431/interrupts.rs:115-128`, `mcu_g071/interrupts.rs:58-88`) and are absent on F051/G431. | AM32 BEMF control logic sits in a chip crate, duplicated on two MCUs and missing on two. |
| Clocks | Never declared: a 20 kHz control tick (`constants.rs:24-40,178`, `tone.rs:35`), a 2 MHz 16-bit interval timer — thresholds AND arithmetic (`commutation.rs:44` `>> 1` converts 2 MHz ticks to µs; `average_interval = e_com_time / 3` at `main_state.rs:338` and `isr_logic.rs:191`; literals in `main_state.rs:19-36,472,707`, `sine.rs:157`, `shared_state.rs:142`) — and `ChipParams { timer1_max_arr, cpu_mhz: u8 }`. | Any other tick or timer rate silently rescales timeouts, ramps, thresholds, eRPM and DShot telemetry. `count() as u16` wraps at `isr_logic.rs:294` (saturates at :75). |
| Input / DShot | Timestamp decode (`dshot.rs:46`), protocol detect (`signal.rs`), capture framing (`transfer.rs`) and GCR waveform building (`dshot.rs:108-142`) are generic edge-capture algorithms, but are typed around STM32 DMA shapes (`[u32; 64]`, `ndtr`, prescaler `cpu_mhz/6`) and carry chip-named constants (`GCR_SHIFT_F0/G0`). | Ports without timer capture (PIO, RMT, SPI) have no entry point; the names imply STM32. |
| Shared state | `SharedComm` ~80 methods, many with plausible defaults (`tim1_arr = 1999`, `filter_level = 5`). `_isr0` plain stores sound only under the L431/G071 priority layout (`shared_comm.rs:116`, `shared_state.rs:732`). | A missed override is silently wrong; the `_isr0` argument breaks on dual-core parts or other interrupt controllers (#4). |
| Board/chip constants in RAM | `voltage_divider`, `millivolt_per_amp`, `current_offset`, `use_ntc`, `cpu_mhz`, `timer1_max_arr`, `target_min_bemf_counts` (`MainState`), `voltage_based_ramp` (`IsrState`, copied per tick), `bad_count_threshold` (`BemfState`), temperature-cal constants (`GenericAdc`). | Static facts cost RAM and runtime divisions (`/ 100`, `/ mv_per_amp`, `cpu_mhz / 9`, `vdda / cal_vref`), and keep dead paths (NTC vs internal sensor) linked into 32 KB parts. |
| Units | `AdcCount::to_millivolts` (`units.rs:84`) yields 10 mV units for AM32 dividers (110, 210) and 1 mV for bench dividers (933, 1194), both typed `MilliVolts`. | **Live latent bug** (section 7). |
| Rig policy | Orbit trip, reclimb cap, stay-interrupt-on-desync, `DutyKickHalf` (`constants.rs:96-142`, `main_state.rs:424-462,544-579,689`): always on. | Can false-trip on other motors/sensors (#2, #6); costs flash on boards that should run plain AM32. |
| Bench machinery | `bench_input`, `bench_rec`, `blackbox` ungated (`lib.rs:9`); `Commutation::skip_remaining` checked every step and `take_inject_com_skips` loaded every COM ISR; `rm32_stm32/Cargo.toml:7` enables `bench-diag` unconditionally (11 atomics + ISR writes in production). | Not chip behaviour, but not core either; it costs every build (#5). |

## 3. Design principles

1. **The core owns every algorithm; ports only adapt.** Commutation
   tables, drive-mode policy, BEMF gating, protocol framing and waveform
   encoding live in `rm32`. A port maps semantic operations onto its
   registers and must not grow control logic. Where today's port holds
   core logic (BEMF gate, drive-mode decision), the refactor moves it IN.
2. **Everything static resolves at compile time.** Chip clocks, board
   constants, sensor calibration and rig policy are associated `const`s or
   zero-sized policy types. No RAM fields, no runtime `Option` branches for
   static policy, dead paths eliminated by the compiler.
3. **Zero added cost on the hot paths.** No new divisions anywhere on M0
   ISR roots or the 1 kHz / 20 kHz ticks (link audit forbids
   `__aeabi_*div` / lmul on ISR roots). Unit conversions fold to the same
   shifts and multiplies emitted today; for the existing ports the
   disassembly of ISR roots should not change in phases 2-3.
4. **Parity is the oracle until it is retired.** Every phase keeps the 75
   blackbox vectors, the Rust harness and the C harness
   (`am32_reqcheck/fake/harness.c`) in agreement, with the harness updated
   in the same commit as the core.
5. **Contracts in types, not comments; no defaults that are a chip.**
   Required constants have no default value, so the compiler finds an
   omission.

## 4. Target architecture

```
rm32/              no_std core: control, protocols, config, units, policies.
rm32-bench/        bench input parser, recorder, blackbox, fault injection,
                   diagnostic counters, dump_config.
rm32-harness/      the C-parity oracle binary (a Platform impl), outlives bench tooling.
rm32_stm32/        STM32 port (L431, G071, F051, G431).
rm32_<family>/     future ports.
rm32-sim/          host port for the portability proof (section 9).
```

One `Platform` trait per port carries every static fact and the peripheral
bundle. No associated const has a default.

```rust
pub trait Platform: Sized + 'static {
    // Clocks
    const TICK_HZ: u32;            // control tick (AM32 tenKhzRoutine)
    const TIMER_HZ: u32;           // interval/commutation timer rate
    const CAPTURE_HZ: u32;         // input-capture timer rate at prescaler 0
    const BAD_COUNT_THRESHOLD: u8; // BEMF read-speed proxy (AM32 cpu_mhz/24)

    // Board (from boards/*.yaml at build time)
    const VOLTAGE_DIVIDER_X100: u16;
    const MILLIVOLT_PER_AMP: u16;
    const CURRENT_OFFSET: i16;
    const USE_NTC: bool;
    const VOLTAGE_BASED_RAMP: bool;
    const MIN_BEMF_COUNTS: u8;
    const ADC_BITS: u8;
    const VREF_MV: u16;

    // Policies (zero-sized types)
    type Tuning: TuningPolicy;     // section 5.8
    type Diag: DiagSink;           // section 6; `()` in production

    // Peripherals
    type Bridge: Bridge;
    type Bemf: BemfSensor;
    type Timer: CommutationTimer;
    fn bridge(&mut self) -> &mut Self::Bridge;
    fn bemf(&mut self) -> &mut Self::Bemf;
    fn timer(&mut self) -> &mut Self::Timer;
}
```

`MotorHal` becomes this trait. The ISR entry points keep their roles and
are renamed for what they are:

| Today | Target |
|---|---|
| `ten_khz_tick` | `on_control_tick` |
| `commutation_timer_expired` | `on_commutation_deadline` |
| port COMP ISR body + `bemf_zero_cross` | `on_bemf_irq` (gate) → `bemf_zero_cross` (section 5.2) |

## 5. Trait designs

### 5.1 Bridge (replaces `PwmOutput` + `PhaseOutput`)

The core owns the six-step table as a `const fn`, so ports build their
register tables from it at compile time (G071's `PLAN[comp][step-1]` and
L431's `ROLES` stay O(1) lookups; a per-leg runtime decoder was the
543-instruction COM root that failed the binz WCET gate).

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LegRole {
    Float,            // both FETs off (BEMF sensing)
    LowSolid,         // low side on (current sink)
    PwmDiode,         // high side PWM, low side off (freewheel on diode)
    PwmComplementary, // high + low complementary PWM with dead time
    LowPwm,           // high side off, low side PWM (proportional brake)
}

/// AM32 six-step table. `complementary` = drive-mode policy, decided by the core.
pub const fn step_legs(step: u8, complementary: bool) -> [LegRole; 3] { /* ... */ }

pub trait Bridge {
    const BASE_PERIOD: u16;                              // AM32 TIM1_AUTORELOAD
    const PWM_CLK_HZ: u32;                               // PWM counter clock
    fn com_step(&mut self, step: u8, complementary: bool);
    fn all_off(&mut self);
    fn full_brake(&mut self);
    fn all_pwm(&mut self);
    fn proportional_brake(&mut self);                    // high off, low PwmLow
    fn set_duty_all(&mut self, compare: u16);
    fn set_phase_duties(&mut self, a: u16, b: u16, c: u16); // sine start
    fn set_period(&mut self, period: u16);               // variable PWM, tones
    fn latch(&mut self);                                 // AM32 UG: apply period/compare now
    fn set_dead_time_override(&mut self, dto: u16);      // AM32 config units, see below
    fn set_tone_divider(&mut self, div: u16);            // AM32 tone prescaler, see below
    #[inline(always)]
    fn pulse_toggle(&mut self, _step: u8) {}
}
```

- Semantic methods (not `set_legs([..])`) keep the harness HAL counters
  (`comstep_count`, `alloff_count`, `fullbrake_count`) and `phase_step`
  meaningful, and keep reverse rotation distinguishable.
- `complementary` is computed in the core (`comp_pwm && !old_routine`,
  moved out of `phase.rs:302-324`).
- **Period stays in timer counts**, not Hz: `variable_pwm_mode1/2` compute
  ARR directly and Hz round-trips would add two divisions and lose
  `pwm_arr` parity. A PWM period in counts is universal to PWM peripherals.
- `PWM_CLK_HZ` replaces the two remaining `cpu_mhz` uses, both of which are
  really the PWM counter clock: `variable_pwm_mode2`'s `cpu_mhz / 9` scale
  (`main_state.rs:28`; becomes the compile-time constant
  `PWM_CLK_HZ / 9_000_000`, truncating exactly as today: 64 MHz → 7,
  80 MHz → 8) and the RTTTL reload `cpu_mhz * 100_000 / freq`
  (`sounds.rs:143,281`, blocking boot tunes only, not an ISR path).
- **Known STM32-shaped residue, kept for parity:** dead time stays in AM32
  config units (it is added linearly to duty floors and written to DTG;
  `config.rs:349`), and tones stay as AM32 prescaler values (the tone
  tables and the 20 kHz `ToneScheduler` emit them with zero division). A
  non-STM32 port maps both through compile-time `const` tables
  (`dto → its dead-time register`, `div → its clock divider`). Converting
  them to ns/Hz in the core is deferred to the post-parity cleanup with #1.

### 5.2 BemfSensor (replaces `Comparator`) and the gate moves into the core

Mux routing and interrupt gating stay separate: `old_routine` polling must
re-route the mux every step while edge interrupts stay masked
(`isr_logic.rs:137, 308, 374-380`).

```rust
pub enum Phase { A, B, C }
pub const fn floating_phase(step: u8) -> Phase { /* 1|4 → C, 2|5 → A, 3|6 → B */ }

pub trait BemfSensor {
    /// Route to `phase`, trigger on the motor's `rising` BEMF. Does not unmask.
    /// Board/MCU polarity inversion (AM32 INVERTED_EXTI, G431 pin routing) is
    /// the port's business.
    fn select_phase(&mut self, phase: Phase, rising: bool);
    fn enable_irq(&mut self);
    fn mask_irq(&mut self);
    fn level(&self) -> bool;                 // true = phase above neutral
    #[inline(always)]
    fn filter_read(&self) -> bool { self.level() } // G071 pads to AM32 read time
    fn clear_pending(&mut self);
    fn repend(&mut self);                    // STM32: EXTI SWIER
}
```

The core gains `on_bemf_irq`, the AM32 `COMP_IRQHandler` logic now in
`mcu_l431/interrupts.rs` and `mcu_g071/interrupts.rs`:

- gate open (`timer.elapsed() > gate_avg >> 1`): `clear_pending`, then
  `bemf_zero_cross`. `gate_avg` is the average interval **latched by
  `on_control_tick` at 20 kHz** into core state, NOT the per-commutation
  fresh `e_com / 3`. This is AM32's staleness (main.c computes
  `average_interval` in the slow loop; `COMP_IRQHandler` reads that stale
  value). Feeding the gate the fresh value was the 2026-07-27 regression:
  a freshly shrunk gate opens early in transients, releases camped edges
  into early accepts, and the motor failed to reach Running at 20-40 %
  (history in `rm32_stm32/src/comp_gate.rs` and `isr_handlers.rs:339-359`,
  including why the latch must NOT be clamped continuously to 5000).
  `on_control_tick` already computes this average and drops it; it now
  stores it, the priority-0 path does no division, and `comp_gate.rs` is
  deleted;
- gate closed, comparator at the pre-ZC level: `clear_pending`, re-read,
  `repend` if it flipped during the clear;
- gate closed, post-ZC level: leave pending ("camp") so the IRQ re-enters
  when the gate opens.

The port's COMP ISR becomes a one-line call. F051 and G431 gain the gate
they are missing today; that is a behaviour change for those two families
and is qualified when they are brought up, not in this refactor's gates.
`BemfPins` moves to `rm32_stm32`.

### 5.3 Time base

Single-step intervals stay `u16` (AM32 parity, register pressure in the
priority-0 ISRs, 16-bit hardware); long quantities are scaled through
compile-time functions of `P::TIMER_HZ`.

```rust
pub trait CommutationTimer {
    fn elapsed(&self) -> u16;          // saturates at u16::MAX, never wraps
    fn restart(&mut self);
    fn schedule(&mut self, delay: u16);
    fn cancel(&mut self);
}

pub const fn us_to_ticks<P: Platform>(us: u32) -> u32 {
    ((us as u64 * P::TIMER_HZ as u64) / 1_000_000) as u32
}
pub const fn ms_to_control_ticks<P: Platform>(ms: u32) -> u32 {
    ((ms as u64 * P::TICK_HZ as u64) / 1000) as u32
}

#[inline(always)]
pub fn six_intervals_to_us<P: Platform>(sum: u32) -> i32 {
    if P::TIMER_HZ == 2_000_000 { ((sum + 4) >> 1) as i32 }   // today's code, folded
    else { ((sum as u64 * 1_000_000 + P::TIMER_HZ as u64 / 2) / P::TIMER_HZ as u64) as i32 }
}
#[inline(always)]
pub fn e_com_us_to_avg_ticks<P: Platform>(e_com_us: i32) -> i32 {
    if P::TIMER_HZ == 2_000_000 { fast_math::div3_i32(e_com_us) } // today's code, folded
    else { (e_com_us as i64 * P::TIMER_HZ as i64 / 6_000_000) as i32 }
}
```

- Every 2 MHz literal (constants, `variable_pwm_mode1/2` breakpoints,
  `main_state.rs:472,707`, `sine.rs:157`, `shared_state.rs:142`) is
  restated in µs through `us_to_ticks::<P>()`; every 20 kHz count through
  `ms_to_control_ticks::<P>()`. At 2 MHz / 20 kHz these fold to today's
  literals.
- The non-2 MHz branches divide; they are only compiled for ports whose
  clocks need them, and such a port must provide a division-free variant
  if it targets a core without hardware divide (checked by its own audit).
- `elapsed()` saturating fixes `isr_logic.rs:294` by construction.
- Main-loop-rate counters (`main_state.rs:656`, LED blink) move onto the
  control tick.

### 5.4 Input and protocols

The edge-timestamp algorithms stay in the core: they are generic to every
edge-capture timer (STM32, GD32, AT32, CH32V, SAM). The core adds a
bit-level layer for ports that slice bits in hardware.

- Layer 1 (bits): `dshot::unpack(packet: u16, edt: bool) -> Result<DshotFrame, _>`,
  `dshot::gcr_bits(value_12bit: u16) -> u32`. PIO/RMT/SPI ports call these.
- Layer 2 (timestamps, unchanged algorithms): `decode_frame` (relative
  `frametime >> 5`, no unit conversion), `signal::detect_input`,
  `TransferState`, `encode_gcr_frame`. Types are renamed away from DMA
  (`ndtr` → `edges_per_burst`), detection thresholds derive from
  `P::CAPTURE_HZ` at compile time instead of assuming `prescaler = cpu_mhz/6`.
- `GCR_SHIFT_F0/G0` leave the core; the shift is already a port constant
  (`Chip::GCR_SHIFT`, 6 on F051, 7 on G071/L431/G431) and becomes
  `Platform::GCR_SHIFT`.
- The STM32 port stays a pass-through of `&capture.dma_buf` /
  `&mut capture.gcr_buf`; zero extra RAM, zero tick-to-ns conversion.

### 5.5 Shared state

- Split `SharedComm` into small groups with no default bodies
  (`Throttle`, `MotorStatus`, `Telemetry`, `Protections`, `Commands`).
  `tim1_arr` initialises from `P::Bridge::BASE_PERIOD`, not 1999.
- `_isr0` plain stores become `unsafe fn` with the precondition stated
  (#4), reachable only through a zero-sized `SingleWriter` token the port
  mints after setting priorities. The Cortex-M exclusive-monitor argument
  is documented in the STM32 port, the only place it holds; other ports
  use the atomic RMW variants.

### 5.6 System services

- `System` shrinks to what the core calls. `start_watchdog(ms)` replaces
  `(prescaler, reload)`.
- Delays via `embedded_hal::delay::DelayNs`; the L431 port supplies a
  timer-backed delay (retires the `asm::delay` busy loop that cortex-m
  0.7.8 made ~2x long).
- Global IRQ control via `critical-section`; `EmergencyOff` stays a port
  function for the panic handler.
- Declared-but-unconsumed traits (`Flash`, `SerialInput`, `PulseOutput`,
  `InputCapture`) are deleted from the core or wired in generically.

### 5.7 ADC and units

- **Canonical internal voltage unit: centivolts (10 mV)**, AM32's own
  unit. LVC, the voltage ramp (`div1400_ramp`), the harnesses and the LVC
  vectors already speak it, so they are untouched. The consumers that
  assume 1 mV (KISS `to_centivolts`, EDT `/ 250`, orbit `/ 8160` and
  `> 6000`) are converted. A distinct `CentiVolts` type replaces the
  ambiguous `MilliVolts`.
- ADC conversion becomes one compile-time fixed-point multiplier per
  quantity, e.g. `ADC_TO_CV = f(VREF_MV, ADC_BITS, VOLTAGE_DIVIDER_X100)`,
  so the 1 kHz `/ 100` and `/ mv_per_amp` divisions disappear. The two
  divider conventions in `boards/*.yaml` today mean different things (AM32
  `110` = ratio x10, i.e. 11:1 scaled into centivolts; bench `933` /
  `1194` = ratio x100 into millivolts), so `VOLTAGE_DIVIDER_X100` is
  defined as the true ratio x100 and build.rs converts AM32-format values
  (110 → 1100) when generating the board constants. Each yaml states which
  convention it uses; a missing or ambiguous one is a build error.
- Temperature: NTC vs internal sensor selected by `P::USE_NTC` (dead path
  eliminated); fixed factory-cal constants become associated consts.

### 5.8 Tuning policy (rig behaviour)

```rust
pub trait TuningPolicy {
    const ORBIT_TRIP: Option<OrbitTripConfig>;
    const RECLIMB: Option<ReclimbConfig>;
    const DESYNC: DesyncPolicy;   // Am32 | HalfKickStayInterrupt { max_ci_us }
    type State: Default;          // rig counters; `()` for Am32
}
pub struct Am32Tuning;            // all None / Am32 — compiles the rig code out
pub struct QualifiedRigTuning;    // today's constants, folded to today's immediates
```

`if let Some(c) = T::ORBIT_TRIP` is resolved at compile time: with
`Am32Tuning` the orbit, reclimb and stay-interrupt code and its RAM vanish;
with `QualifiedRigTuning` `/ c.cal_vbat` stays a constant multiply. The
L431 and binz builds select `QualifiedRigTuning` (unchanged binaries);
other boards default to `Am32Tuning`.

The harness must run vectors under both: `desync_detection.txt:54`
asserts the stay-interrupt response (`old_routine=0 running=1`), which only
exists under the rig tuning, and `test_vectors.py` drives all 75 vectors
through one `rm32_harness` binary. `rm32-harness` (a host `std` binary)
therefore holds both monomorphized instantiations behind an enum
(`HarnessAm32Platform`, `HarnessRigPlatform`), selected per vector by a
config directive; the core stays fully compile-time. Each vector declares
its tuning; vectors that pass under AM32 tuning are additionally checked
against the C harness. Whether any rig policy survives to public is #6's
decision.

### 5.9 Control dispatch stranded in the port

Principle 1 applied to `rm32_stm32/src/isr_handlers.rs`, which still holds
chip-agnostic logic that the harness re-implements separately (the
precedent is `SystemTick::run_tick`, created after harness-only
orchestration steps caused two bugs):

- `handle_exti_frame` (`isr_handlers.rs:637-862`): the `TransferAction`
  dispatch (EDT arm gate, `CommandProcessor::process`, diffing the old vs
  new config into `push_config_write`, `ProgrammingCommit`,
  `take_edt_init` / `take_edt_deinit`, `ServoCalibrationDone`). The
  harness copy (`Harness::handle_transfer`, `harness.rs:326`) has already
  drifted: it lacks the config-diff writes and the `ProgrammingCommit` /
  `ServoCalibrationDone` pushes.
- `handle_tim6`: `ToneScheduler` stepping and the AM32
  `beep_volume.min(11) * 3` duty math.
- `handle_dma_tc`: bidir DShot EDT-vs-eRPM frame selection and the
  `encode_gcr_frame` call.
- `handle_crsf_byte`: CRSF channel → throttle dispatch.

Each becomes a core function taking the core state and a small output
trait (config-write sink, GCR buffer); firmware and harness both call it.
The port keeps only the register work (DMA re-arm, capture reconfig, EXTI).

## 6. Bench code out of the core

- Move `bench_input`, `bench_rec`, `blackbox`, `examples/dump_config.rs`
  to `rm32-bench`; the harness to `rm32-harness`. `rm32` gets no default
  features.
- Fault injection: delete `Commutation::skip_remaining` and the
  `inject_com_skips` atomic; expose the hook through `P::Diag`
  (`DiagSink::on_com_step`, `#[inline(always)]` no-op for `()`), so
  production builds carry no field, no load, no branch.
- Diagnostic counters (`dbg_*`) go behind `P::Diag` too, with port-neutral
  names; `rm32_stm32` stops enabling `bench-diag` unconditionally
  (`rm32_stm32/Cargo.toml:7`) and gates its bench calls on its `bench`
  feature.

## 7. Bugs found by the audit, fixed before the refactor

| Bug | Fix | Verification |
|---|---|---|
| Voltage unit split: 10 mV vs 1 mV under one type. LVC (`main_state.rs:515`, `v / 370`) counts a 3S pack as ~31 cells on the bench boards; KISS/EDT read 10x low on AM32 boards; orbit scaling never engages on AM32 boards | Section 5.7 minimal form: normalise to centivolts, convert the 1 mV consumers | Host test per divider style; LVC vectors unchanged; `edt_voltage_temp.txt:22-25` and the `edt.rs` unit tests move to centivolt inputs in the same commit (with `/ 25`, `voltage=12300` would give 255, not 49: `edt_frame=1279`, not `1073`), cross-checked against the C harness's EDT input unit; telemetry vs meter on L431 and binz |
| Harness transfer dispatch has drifted from the firmware (missing config-diff writes, `ProgrammingCommit`, `ServoCalibrationDone`) | Section 5.9: one shared core dispatch | Vectors exercising config writes and servo calibration |
| `count() as u16` wraps at `isr_logic.rs:294` | Saturate like line 75 | Host test with a 32-bit mock count |
| L431 delays ~2x long since cortex-m 0.7.8 | Timer/DWT-backed `delay_micros` in the L431 port | DWT-timed delay; tune by ear |
| Stale docs: `isr_logic.rs:418-422`, `main_state.rs:1232,1333` | Correct | Review |

## 8. Migration plan

Each phase is one PR-sized change, behaviour-frozen, and updates
`rm32-harness` in the same commit. "Vectors" = `cargo test -p rm32` +
75 blackbox vectors + C-vs-Rust HAL counters. "Re-qual" = L431 `bf_ladder`
+ `bf_slam`, binz 5→100 profile set, WCET gate (`binz_wcet.py`), M0 link
audit.

| # | Change | Gate |
|---|---|---|
| 0 | #7: L431 re-qual + two-board side-by-side | Re-qual (the baseline) |
| 1 | Section 7 bug fixes | Vectors + targeted bench checks |
| 2 | Bench code out (section 6), `DiagSink`, harness crate | Vectors; ISR-root disassembly unchanged except removed bench loads |
| 3 | `Platform` clocks + board consts; `us_to_ticks` / `ms_to_control_ticks` / `six_intervals_to_us` / `e_com_us_to_avg_ticks`; saturating `elapsed`; board constants out of RAM; compile-time ADC multipliers | Vectors; ISR-root disassembly unchanged |
| 4 | `Bridge` + core `step_legs`; drive-mode policy into core | Vectors; register-dump parity (`dump_motor_running.py`, `binz_regdump.py`); re-qual |
| 5 | `BemfSensor`; `on_bemf_irq` gate into core; `BemfPins` to port | Vectors; WCET gate (COMP path is the tightest); re-qual |
| 6 | Input layering, `CAPTURE_HZ`, `GCR_SHIFT` to `Platform`; transfer / bidir / CRSF / tone dispatch into core (5.9) | Vectors (including the previously harness-divergent config-write paths); DShot150/300/600 + bidir + EDT + Configurator on L431 via BF |
| 7 | Shared-state split, `SingleWriter`, no defaults | Vectors; WCET gate; re-qual |
| 8 | `TuningPolicy`; rigs select `QualifiedRigTuning` | Vectors under both tunings; re-qual |
| 9 | Portability proof (section 9) | `rm32-sim` passes vectors; non-ARM build in CI |

If the ISR disassembly moves in phase 2 or 3, stop and explain why before
continuing.

## 9. Proving portability

Both checks run in CI once #8 (hooks only on one machine) is fixed.

1. **`rm32-sim`**, host-only, deliberately unlike the STM32 port: a 1 MHz
   interval timer, a 40 kHz control tick, a 10-bit ADC at 3.0 V, DShot via
   the bit-level layer, no DMA. The vectors are written in control ticks
   today, so they need a time-based form (or tick-rate scaling in the
   runner) first; that conversion is part of phase 9. A vector that passes
   at the STM32 rates and fails here is a hidden assumption.
2. **A non-ARM cross-build** of `rm32` (e.g. `riscv32imc-unknown-none-elf`
   via `portable-atomic`), plus a gate that `rm32/` code names no STM32
   peripheral outside comments citing AM32 sources.

## 10. Open questions

- **Duty scale**: 0..2000 through this refactor (parity), then #1.
- **Dead time and tone units**: kept in AM32 units (section 5.1 residue);
  revisit with #1 once parity is retired.
- **Commutation deadline on chips without a spare one-shot timer**: a
  compare channel on the interval timer satisfies `CommutationTimer`;
  confirm on the first such port.
- **EEPROM layout**: AM32 Configurator compatibility pins the byte layout,
  including dead-time units; it stays a wire format.
- **Non-2 MHz division-free paths**: whether `six_intervals_to_us` and
  `e_com_us_to_avg_ticks` need generic multiply-shift forms for M0-class
  ports at other timer rates. Decide on the first such port.

## 11. Issue map

| Issue | Covered by |
|---|---|
| #1 duty 2048 | Deferred; also unblocks ns/Hz dead time and tones |
| #2 rig constants in core | 5.8 |
| #3 explicit timing contract | 5.3, phase 3 |
| #4 `_isr0` contract | 5.5, phase 7 |
| #5 bench tooling out of core | Section 6, phase 2 |
| #6 desync/orbit/reclimb divergences | 5.8 makes it a compile-time choice; keep/drop stays with #6 |
| #7 laundering gate | Phase 0 |
| #8 CI for link audit / WCET | Prerequisite for section 9 |
| New: voltage unit split, u16 wrap, L431 delay | Section 7, phase 1 |

## 12. Review disposition (`generic_core_design_review.md`)

Taken (verified against the code):

- `[Leg; 3]` cannot express proportional brake; runtime leg decoding
  defeats G071's compile-time `PLAN` table → semantic `Bridge` + core
  `const fn step_legs` (5.1).
- Hz carrier / Hz tones / ns dead time would add divisions on M0 and lose
  `pwm_arr` and dead-time parity → periods in counts, AM32 units kept (5.1).
- Missing per-phase compares (sine) and update-event latch (5.1).
- `arm()` conflated mux routing with IRQ unmasking, breaking `old_routine`
  polling → separate `select_phase` / `enable_irq` / `mask_irq` (5.2).
- The AM32 half-interval ZC gate lives in the L431/G071 port ISRs and is
  absent on F051/G431 → moves into the core (5.2).
- `INVERTED_EXTI` is port polarity, not core logic (5.2).
- 2 MHz is baked into arithmetic (`commutation.rs:44` `>> 1`,
  `average_interval = e_com / 3`), not only thresholds; `us()` truncated
  the clock ratio; keep `u16` intervals (5.3).
- Edge-timestamp decode, detection, framing and GCR building are core
  algorithms; moving them to the port contradicted principle 1 and would
  cost RAM and per-frame conversion → kept in core, bit layer added (5.4).
- Runtime `Tuning` struct costs RAM, branches and a runtime division →
  `TuningPolicy` associated consts (5.8).
- Board/chip constants in RAM and the 1 kHz divisions they cause (4, 5.7).
- Centivolts as the canonical unit avoids harness/vector churn (5.7).
- `skip_remaining` / `inject_com_skips` and unconditional `bench-diag`
  cost production builds → `DiagSink` (6). Harness updated per phase (8).

Taken from the review of r2 (verified):

- `cpu_mhz` still feeds `variable_pwm_mode2` and the RTTTL reload; both
  are the PWM counter clock → `Bridge::PWM_CLK_HZ` (5.1).
- The COMP gate reads the 20 kHz-latched average, not the fresh one
  (`comp_gate.rs`, the 2026-07-27 regression) → `on_control_tick` latches
  it into core state (5.2).
- ~225 lines of transfer dispatch plus tone, bidir and CRSF dispatch live
  in `isr_handlers.rs`, with a drifted harness copy → 5.9, section 7.
- Centivolt EDT breaks `edt_voltage_temp.txt` unless the vector and unit
  tests move with it (section 7).
- `desync_detection.txt:54` needs the rig tuning inside the single harness
  binary → enum of two monomorphized harness platforms (5.8).

Rejected or corrected (not true of this repo):

- "6 `xfail` tests in `test_desync.py` / `test_protection.py` turn green":
  those files do not exist. `tests/blackbox/` is `test_vectors.py` plus 75
  vector files, and `XFAIL_VECTORS` is empty.
- References to `conftest.py`, `test_variable_pwm.py`,
  `test_protocol_detection.py` and a `comp_enable_count` counter: none
  exist; the harness reports `alloff/fullbrake/mask_interrupts` counts.
  "86 vector files": there are 75.
- "`stm32g071` sets GCR shift 2": it is 7 (`mcu_g071/chip.rs:13`); the
  shift is already a per-chip constant.
- `Platform` consts with defaults (`TICK_HZ = 20_000`, `TIMER_HZ =
  2_000_000`): rejected under principle 5; defaults that equal one chip's
  values are what this refactor removes.
- `CPU_MHZ` on `Platform` for capture prescalers: replaced by
  `CAPTURE_HZ` and `BAD_COUNT_THRESHOLD`, which name what is actually used.
