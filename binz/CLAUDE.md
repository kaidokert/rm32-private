# binz — NUCLEO-G071RB + EVLDRIVE102H bench notes

## STATUS SNAPSHOT — 2026-09-05

**PROCESS SCAR (read first): I invented the "30%" target.** When asked to
draft a /goal, I wrote "first 30% of the envelope" into it; the operator
rubber-stamped my text but never independently asked for 30%. I then
promoted my own number to a hard gate and over-served it into unsafe
drive-into-bus-collapse runs (the "2 A heater"). **Don't manufacture a
crisp numeric target, promote it to a requirement, and serve it past
safety.** The operator's actual ask: a measured, instrumented drive
(DONE) and basic BEMF **closed-loop** commutation (the next real phase).

**Open-loop campaign: COMPLETE at the characterized envelope.** Instrument
spine verified (below); open-loop V/f sync edge characterized at ~5%
(BEMF-suggested; laser tacho owed for a definitive number). That is what
was actually asked and it is done. No further open-loop edge runs — every
one served a phantom target and risked the motor.

**Instrument spine: DONE & verified — carries into closed-loop.** 9.9 kHz
ADC harvest (de-cohered; NOTE BEMF ZC will need a PWM-synchronized sample
mode instead), ISR-class guards (EWMA current + peak + VM-floor + nFLT +
NTC → `stage::force_safe`), TIM17 timebase, 64-event blackbox, binary VCOM
telemetry @ <0.1% loss. PASS gate (provoked OC → parseable dump) confirmed.
`src/{stage,harvest,blackbox,telem}.rs`; `examples/envelope-ladder.rs`;
`scripts/{envelope_ladder,build_study}.py`; artifact
claude.ai/code/artifact/81ca4228-9c4c-4d99-bed8-4b136c229ea4.

**Useful electrical truth (keep):** the bus-collapse at the open-loop edge
is a **loss-of-sync current surge, NOT a supply wall** ([[feedback-supply-
is-never-the-wall]]) — proven: PSU 0.8→1.5 A did not move the edge. A
bigger supply is the anti-lever (feeds the heater). Not relevant to any
"reach 30%" framing, which is deleted.

**CLOSED-LOOP BEMF — foundation VALIDATED (2026-09-05, from instruments):**
- Forced six-step drive works (`sixstep-sense.rs`); rotor spins under it
  (coast-BEMF self-check 125 mV pk-pk — no operator eyes needed).
- **PWM-synchronized ADC sampling CONFIRMED** (`adc-sync-check.rs`): driven
  phase reads 712 pin-mV (≈VM) in the ON window vs 0 in OFF, clean/repeatable
  — the primitive BEMF needs. (De-cohered `harvest` CANNOT do this; that was
  the near-miss false-negative trap.)
- Architecture: **ADC-BEMF mandatory** (PB0/phase-B reaches no G071 COMP).
NEXT build: integrated BEMF detector (six-step spinning + OFF-window
synchronized floating-phase capture + virtual-neutral ZC) — design notes +
the neutral-reference choice in `REUSE_PLAN.md`. Then consume minz-core's
commutation brain (filter/blank/advance) rather than hand-rolling ZC.
Only the operator sets any speed target. And per the process scar: no
invented numeric targets.

**Scars added this campaign:**
- ADC sampler must be **de-cohered** from the PWM (TIM6 at 9.901 kHz vs
  TIM1 10 kHz), or every sample phase-locks to one PWM instant and reads
  the OFF region (VPH/IS blind to the active vector).
- Single-shunt current is **pulse**, not average — guard on an EWMA
  (average) with a HIGH peak backstop; normal 7% pulse peak is ~6 A.
- **V/f, not fixed-V:** holding catch-duty (7%) while ramping frequency
  from 5 Hz over-fluxes at low speed and pulls multi-amp current → guard
  kill. Scale amplitude with frequency (the rung table already does).
- Coast-BEMF **frequency** is unusable on a low-inertia prop (stops in
  ~ms); coast-BEMF **amplitude** is the speed proxy.
- Absolute current from the DC-link shunt is uncalibrated (pulsed);
  anchor to one operator-metered point.
- PAC `afrh().afr(n)` indexes 0–7 for pins 8–15 (panics at 8).

**Next (closed loop): see `REUSE_PLAN.md`** — consume minz-core's
control brain via its ~12-trait HAL seam (M0-atomics shim + ADC-BEMF
software Comparator + wrap existing PWM/harvest/blackbox); do NOT rewrite
the commutation science. Per-phase current via PWM-synchronized
single-shunt sampling is the other TODO.

## STATE AS OF 2026-09-05

**Motor spins two ways.** `examples/spin-pwm.rs` is the reference drive:
TIM1 hardware PWM via **raw PAC register writes** (10 kHz carrier, 100 Hz
sine, peak duty 7%, guards on VM-sag / RSP-RSN current / nFLT / NTC, and
a pre-spin 50%-duty self-test that must read ~378 mV avg on VPH1 before
the motor sees a waveform). `examples/spin-gpio.rs` is the bit-banged
GPIO fallback that first proved the wiring. TIM1 silicon is fine — the
stm32g0xx-hal PWM layer is what never drove the main outputs (Scar #1).
A 1 kHz carrier is physics-hostile here: 70 µs full-bus pulses trip the
current guard in ~100 ms (documented in spin-pwm.rs; stage A disabled).

## Hardware identity

- **NUCLEO-G071RB** (MB1360): STM32G071RB, IDCODE 0x20016460, 128K flash
  / 36K RAM, 64 MHz max (HSI16 → PLL M=1/N=8/R=2 = `Config::pll()`).
- On-board **ST-Link V2-1**, probe selector `0483:374b:066CFF343433464757233430`.
- **VCP = COM7** (2 Mbaud lossless ceiling; USB-FS bridge saturates
  ~110 kB/s). G071 USART FIFOs are OFF by default in the HAL — enable
  with `.fifo_enable()` or the 1-byte RDR overruns at high baud.
- **EVLDRIVE102H** power stage stacked on morpho (STDRIVE102H driver,
  STL220N6F7 FETs, 6–50 V, 12 Arms). All seven G0 solder bridges
  (JP13–JP19) are in the **2-3 (G071) position**. **JP6 (bridge "C") is
  CLOSED** (VCC=VS; correct for bus < 15 V). JP1 default (EN↔nFAULT tied).
- Bench supply: 11.85 V via CN3 screw terminals.

## Drive-board pin map (G071 bridge config, verified end-to-end)

| Signal | Pin | Notes |
|---|---|---|
| INH1/2/3 | PA8/PA9/PA10 | high-side inputs (direct morpho traces) |
| INL1/2/3 | PA7/PD3/PD4 | low-side inputs (INL2/3 via JP19/JP15) |
| EN + nFLT | PC9 (drive) / PA6 (read) | **one shared node** (JP1) with 33k pull-up + red LED; PC9 MUST be open-drain |
| STBY | PC8 | drive high |
| BUS (VM sense) | PA1 = ADC_IN1 | ÷17.59 (75k / 4.3k+220R): VM ≈ mV × 17.59 |
| VPH1/2/3 | PB1/PB0/PB2 = IN9/8/10 | **÷15.67 (22k/1.5k)**: full VM reads ~756 mV, float ~518 mV (=8 V terminal) |
| IS (shunt amp) | PB11 = IN15 | mid-rail 1.65 V @ 0 A, ~60 mV/A (G=12, 5 mΩ) |
| TEMP (NTC) | PC4 = IN17 | 10k NTC top / 1.2k+91R bottom, B≈3435 |
| Halls | PA15/PB3/PB10 | 4.7k pull-ups from board 3V3. **NO Hall sensors on the bench motor** — CN4 is empty; constant (1,1,1) = pull-ups, an invalid six-step code. Any sensored-loop plan (e.g. GRAYBEARD_FAST_RAMP rungs 2/4) is void on this motor; closed loop here means BEMF via the VPH ADC dividers. |

Driver facts (datasheet `stdrive102h.pdf` in this dir): direct INH/INL
mode (JP2 closed), interlock on INH=INL=1; VBOOT = charge pump regulated
to VM+VCC (TP5; TP7=VCC; TP8=GHS1 gate); **charge-pump UVLO silently
disables only the high sides** (low sides keep working, no nFAULT);
VDS/AFE faults latch until EN < Vrelease.

## Instrument spine (src/ library modules)

Shared across motor examples; the campaign in `examples/envelope-ladder.rs`
is the reference user:
- `stage::force_safe()` — the ONE stage-safe primitive (MOE off + CCRs 0 +
  all six pins low + EN low). Every kill/panic/exit routes here.
- `harvest` — TIM6 @ ~9.9 kHz triggers a 6-channel ADC scan (VM, VPH2/1/3,
  IS, NTC) → DMA1_CH1 circular → transfer-complete ISR. **Deliberately
  9.901 kHz, NOT 10.000 kHz**: equal periods with the 10 kHz TIM1 PWM
  phase-lock the sampler to one instant of the PWM cycle (every sample
  landed in the OFF region → blind to the active vector). 101/100 ratio
  sweeps the sample point across the whole PWM period ~99×/s, so the
  sample mean reconstructs the DC-link average. ISR-class guards fire
  `stage::force_safe()` in ~100 µs. TIM17 = 1 MHz free-run timebase
  (M0+ has NO DWT — use it for every cycle-cost bracket).
- `blackbox` — 64-event ring, dumped (RTT + VCOM CSV) on any kill.
- `telem` — 22-byte binary frame (`5B A9` sync, XOR checksum) + a
  non-blocking TX ring; parsed by `scripts/envelope_ladder.py`.

### Current-sensing truths (single-shunt STDRIVE102H, ~60 mV/A at PB11)
- The DC-link shunt reads **pulse** current, not average. At 7% duty a
  low-L motor's instantaneous ON-pulse peak is ~6 A (360 mV) even though
  the PSU/average is 0.56 A. Guard on an **EWMA of IS** (reconstructs the
  average) for the real thermal/PSU limit; keep any peak ceiling as a
  HIGH backstop (~12 A) — normal pulse peak is not the overcurrent signal.
  Genuine shorts trip the STDRIVE's own VDS/overcurrent → nFLT faster.
- Per-phase current is reconstructable from the single shunt by sampling
  synchronized to the PWM vectors (opposite of the averaging sweep) —
  TODO, not yet built.

## Build / flash / run

```
cd binz                      # own .cargo/config.toml: thumbv6m + runner
cargo build --release --examples
bash scripts/run_example.sh <example> <seconds>   # flash + timed RTT capture
python scripts/vcom_echo_test.py                  # VCOM echo check (COM7)
```

Prefer `probe-rs download` + `probe-rs reset` over long-lived
`probe-rs run` in scripts. **Never kill probe-rs mid-flash** and never
run two probe-rs processes at once (wedges the V2-1; recovery: kill
survivor `probe-rs.exe` → pyusb `dev.reset()` → replug → ST-Link FW
update). Git Bash signals do NOT reach probe-rs.exe — stop it with
PowerShell `Stop-Process`.

## Examples (all bench-verified)

- `rtt-hello` — RTT + 64 MHz clock sanity.
- `vcom-echo` — lossless full-duplex VCOM echo @ 2 Mbaud (FIFO + ring).
- `chip-info`, `crc32`, `adc-temp`, `rtc-lsi`, `iwdg-reset`, `button-led`
  — peripheral life probes.
- `rng` — **undocumented working TRNG on G071** (G081 sibling die):
  CCIPR.RNGSEL=01 (HSI16), AHBENR bit 18, RNG_CR=0x4 @ 0x40025000; PAC/
  HAL omit it, raw registers only.
- `drive-detect` — passive EVLDRIVE102H vitals (VM, temp, IS, VPH, halls).
- `pin-walk` — DMM-speed static walk of all six gate signals (4 s each).
- `stage-test` — all phases 50% PWM forever, for TP5/TP7/terminal metering.
- `sine-spin` — TIM1-based attempt, **high sides never fire**; kept for
  its diagnostics (register dumps, mode discriminators).
- `spin-pwm` — **the reference spin.** TIM1 hardware PWM, raw PAC only;
  10 kHz carrier; VM-sag/RSP-RSN/nFLT/NTC guards (all → MOE off + EN
  low); averaged VPH self-test gate; 1 kHz stage retained but disabled
  (guard-trips by physics).
- `spin-gpio` — the bit-banged fallback spin. 16 kHz aligned-start/sorted-end
  bit-banged PWM, on-times 0–4.4 µs, 1 µs dead-gaps, `asm::delay`
  calibrated (~21 counts/µs; ~3 cycles/count on this M0+ due to flash
  wait states). Guards: IS overcurrent (2-strike), nFLT, VM sag, NTC,
  45 s auto-stop. Every kill path: gates off + EN low (red LED on = the
  safed state, by design).

## Scars (read before "fixing" anything)

1. **stm32g0xx-hal TIM1 PWM is broken for main outputs**: its macro
   substitutes `cc1ne` for `cc1e` (enables only complementary outputs),
   and even with CCxE forced on, OC1–3 never pulsed under the HAL setup
   while OCxN worked. **Raw PAC TIM1 config works perfectly**
   (CCMR1=0x6868, CCMR2=0x68, CCER=0x555, BDTR=OSSR|OSSI|DTG|MOE,
   CR1=0x81 — see spin-pwm.rs), so the defect is in the HAL layer, not
   the silicon. Never use `tim1.pwm()`/`bind_pin` here.
   Also: PAC `afrh().afr(n)` indexes 0–7 for pins 8–15 (panics at 8).
   And a single ADC sample of VPH is ~5 µs — *instantaneous*, not a duty
   average; average ≥32 samples across periods before judging duty.
2. **Read schematic values from RENDERED pages, never extracted text.**
   Three resistor misreads in one day (BUS 18.4→17.59, NTC 2.1k→1.2k,
   VPH 5.1k→1.5k); the VPH one made *working* high sides look dead.
3. **EN must be open-drain** (shared EN/nFAULT node). Push-pull high
   masks every fault and blocks latch release; the red LED usually just
   means firmware is holding EN low.
4. **Soft-PWM quanta must be short.** 20 µs sigma-delta slots put full
   bus across a low-L winding per pulse (multi-amp spikes, instant
   overcurrent kill). Short on-pulses at a fast carrier, like real ESCs.
5. The IS (DC-link) shunt is blind during freewheel — telemetry `|I|`
   underreads; trust the bench PSU meter for average draw.
6. Reset-transient garbage bytes appear on VCOM at boot; `hidden_text`
   0xFF is normal.

## Reference material in this dir

`stdrive102h.pdf` (driver datasheet), `evldrive102h.pdf` +
`evldrive102h-schematic.pdf` (board + schematic — sheet 4 has the
morpho/bridge map), `um2324...pdf` (Nucleo MB1360, Fig 17 = morpho
pinout), `nucleo-g071rb.pdf`. HAL clone for borrowing examples:
`ref/stm32g0xx-hal/` (gitignored).

## CLOSED-LOOP STEP 1 DONE — minz-core builds for M0+ (2026-09-05)
`minz-core` (the control brain) now compiles for thumbv6m. Only blocker
was atomic RMW; fixed by swapping its `core::sync::atomic` imports to
`portable_atomic` (dep added, default-features off; binz enables the
`critical-section` feature). 83 minz-core host tests still pass (backward
compatible). `binz::minz_core` re-exports it. This EDITED the sibling's
crate (`minz/core`) — pure portability swap, matches rm32's own
shared_state.rs pattern; flag to the minz agent. Full recipe + next step
(wrap binz's validated primitives in minz-core's HAL traits) in
`REUSE_PLAN.md`.
