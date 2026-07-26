# Claude Code working notes — rm32 / Vimdrones L431 bench

## STATE AS OF 2026-07-26 (branch `am32_sheet`, HEAD ~4c3d98a) — read this first

**The drive envelope is DONE: full 15–100% throttle locked at AM32-clone
parity** (100% = 2347 Hz e @ 4.62 A, sd 9-17 µs at every level; per-level
speeds/currents within 1-2% of the same-protocol clone control). The
authoritative running state ledger lives in the session memory file
`project_rm32_parity_state.md` (rungs 16-19), not in this document.
Key results that OBSOLETE sections below:

- The "100-200 ms chops" investigation below is ancient history. The final
  three defects were: (1) a diagnostic print in the TIM16 ISR delaying
  commutation ~180 µs (observer effect — NEVER print in prio-0 ISRs);
  (2) bench USART2 RX corruption manufacturing phantom throttle values
  (fixed: DMA circular RX, `ore=0`); (3) a desync-echo rm32 invented by
  "fixing" AM32's dead-code `average_interval=5000` reset (reverted —
  match the reference's BEHAVIOR, not its intent).
- KEPT DIVERGENCES (measured + chosen): fast-rotor desyncs stay in
  interrupt mode (`DESYNC_STAY_INTERRUPT_CI` — rm32's 20 kHz tick-grid
  polling cannot re-lock above ~800 Hz e where AM32's main-loop-rate
  polling can; demotion at speed cost 1-2.4 s churn per event) with a
  half-duty kick (`IsrAction::DutyKickHalf`) instead of the full
  min_startup/2 crash (chop softening); AUTO drive mode (comp in
  interrupt mode, diode in polling); polling-through-timer.
- Chop is instrumented: `dsy=` (desync events) in the bench `i` line;
  `scripts/envelope_sweep.py` reports duty-dip chop episodes. Desync
  rate at 70%+ is SUPPLY-SENSITIVE (rail sag deepens demag).
- STALE CLAIMS BELOW: DSHOT150 detection EXISTS (signal.rs bucket +
  prescaler-3 capture config). Bidir auto-detect is now self-validating
  (commits only after 4 consecutive inverted-CRC decode successes —
  bench validation pending a DSHOT source). Non-L431 COMP wrappers
  pre-ack EXTI (task #39 closed).
- Bench: `scripts/envelope_sweep.py` (staircase + --grad walks + chop
  report) and `scripts/spiral_hunt.py` are the standard instruments.
  Bench UART RX is DMA-based; throttle values need two identical
  consecutive sends to apply (host scripts already repeat at 10 Hz).
- NEXT PHASE: DSHOT/Betaflight (bidir bench validation, EDT, configurator
  re-check), long-soak retention at 70-100% on a healthy supply (bench
  power path was being rewired — thicker leads), non-L431 bench bringup.

Everything below is historical context from the May-July campaign; trust
file:line claims only after re-verification.

## Active investigation (May 2026)

**Strategy**: rm32 must reach **1:1 parity with AM32** at three levels — exact register state during operation, identical computational architecture (don't move work between ISR and main loop), and same NVIC priority structure. If rm32 does more work than C, reduce it; never relocate it.

**Current branch**: `bisect_init_changes`. Sequenced commits below.

### Symptom timeline
1. Original rm32 (`main` baseline): motor runs but choppy on the bench. Bidir DSHOT broken — ~50% CRC fail rate. AM32 Configurator passthrough broken.
2. Discovered massive register divergences vs AM32 (clock tree, TIM1/TIM6/TIM16, GPIO, DMA, COMP) and fixed them piece-by-piece — see "AM32 register parity" below.
3. Smooth PWM motor running once TIM16 + register parity + clean bench config were applied.
4. **Currently investigating**: motor chops 100–200 ms at ~40% throttle. Root cause: TIM6 ISR (`ten_khz_tick`) takes ~42–69 µs out of its 50 µs budget; without NVIC priorities, it blocks COMP/TIM16 → commutation timing decays during ISR overrun → BEMF re-lock fails → mode falls back to OldRoutine → chop.

### Diagnostic infra added to firmware (bench-debug only)

All gated on `feature = "debuguart"` (and M4 features where DWT is needed):

| Tool | Purpose | Files |
|---|---|---|
| `dprintln!` macro | RTT + USART1/PB6 mirror | `lib.rs` |
| `[loop n=]` periodic log | every 100k main iters; one line per ~5 s | `bin/main.rs` |
| `cyc_k` (DWT.CYCCNT/1000) | wall-clock timestamp; stalls visible | `bin/main.rs` |
| `dbg_isr_tick` | TIM6-ISR counter (20 kHz); distinguishes "main stalled" from "chip frozen" | `shared_state.rs` |
| `dbg_tim6_last_cyc` / `dbg_tim14_last_cyc` / `dbg_comp_last_cyc` | per-ISR cycle duration of the most recent tick; plain store, single-sample. (Was `*_max_cyc` with `fetch_max`; the LDREX/STREX loop + rprintln heartbeat inflated measurements ~10×.) | `shared_state.rs`, `isr_handlers.rs` |
| `dbg_crc_pass` / `dbg_crc_fail` | DSHOT decode success/fail counters | `shared_state.rs` |
| `dbg_bidir_evt` / `dbg_high_pin_n` | bidir auto-detect telemetry | `shared_state.rs` |
| `dbg_frame_history` ring buffer | last N DSHOT frame buffers with pass/fail flag | `dbg_frame_history.rs` |
| Panic/HardFault → debuguart | crashes now visible in `port_41.log`, with `debug_uart::flush()` before halt | `panic.rs` |
| `feature = "bringup"` | early-jump to `mcu_l431::bringup::run_and_spin()` — AM32-register-parity init then spin loop with IWDG refresh. For dumping/diffing register state. | `bin/main.rs`, `mcu_l431/bringup.rs` |
| Bench-clean config overrides | clears `stuck_rotor_protection`/`stall_protection`/`bi_direction`/`use_sine_start`/`brake_on_stop` after EEPROM load to mirror AM32 Configurator "all complex features off" baseline | `bin/main.rs` |

Reading rate from `[loop n=]`: `Δcyc_k = 410k` → ~5.12 s wall time → main loop iter rate = 20 kHz (== TIM6 ISR rate, since `wfi()` at the bottom of the loop wakes per ISR). Anything significantly larger than that means main is starved by ISRs.

### AM32 register parity (mostly complete)

Captured via `scripts/dump_l431_regs.py` (probe-rs reads ~118 peripheral registers without halting the core) and `scripts/dump_motor_running.py` (6 snapshots while motor spinning, comparing AM32 hex vs rm32 build).

- **Clock tree**: PLLM=2/N=20/R=2 from HSI16 (not HAL's M=1/N=10/R=2). FLASH.ACR = 0x0604 (LATENCY=4, ICEN+DCEN, **PRFTEN intentionally OFF on AM32**). Direct register writes in `mcu_l431/init.rs`, bypasses stm32l4xx-hal `freeze()`.
- **NVIC PRIGROUP=3** (4 preempt / 0 sub bits). Set in init.rs via direct write to SCB.AIRCR.
- **TIM6**: PSC=79, ARR=50 (1 MHz tick → 19.6 kHz update event). NOT PSC=0/ARR=3999.
- **TIM1**: CR1=ARPE+CEN (0x81), CCMR1/2=0x6868 (PWM mode 1 + OCxPE on all 4 channels), CCER=0x1555 (CC1E/2E/3E + complementary N + CC4E), CCR4=0x64 (TRGO trigger), BDTR=DTG|BKP|MOE (0xA02D with `dead_time=45`).
- **TIM16** (commutation): CR1=ARPE+CEN (0x81) free-running from boot. `Tim14Com::set_and_enable()` writes ARR (→ preload via ARPE) then EGR.UG to force-load. Was previously one-shot (disable→reset→enable per commutation); free-running matches AM32 and removes the disable→reload gap that caused choppy commutation.
- **TIM2** (interval timer): ARR=0xFFFF (16-bit wrap, AM32-matched). Was 0xFFFFFFFF.
- **TIM7, TIM15, TIM16, COMP2, DMA1_CH4/CH5, USART1**: per-register parity confirmed via `bringup.rs`.
- **GPIOA/B pin config**: PA2 pull-up + medium-speed + AF14 (TIM15_CH1, DSHOT signal — pull-up matters for bidir telemetry slot when line is undriven). PA7 + PB0/PB1 AFRL=1 (motor PWM-N alternates, MODER stays output for safety idle until armed). PB6 USART1 half-duplex AF7 + pull-up. PB4 PUPDR cleared (was NJTRST reset default 0b01 pull-up).
- **`dead_time` 60 → 45** in `boards/neutron_l431.yaml` (AM32 default for this hardware).
- **TIM16 ISR pattern**: `Tim14Com::new()` writes CR1=ARPE+CEN at boot, so timer is always running. The free-run is what gives smooth motor.

### NVIC priorities — CRITICAL: rm32 was buggy (all 0)

**Bug**: cortex-m's `NVIC::set_priority(irq, n)` writes the raw byte `n` to NVIC IPR. STM32L4 NVIC has **4 priority bits in the UPPER nibble** of each IPR byte. Writing `1` → reg byte `0x01` → effective priority **0** (lower nibble ignored). So all rm32 priorities collapsed to level 0 regardless of the passed value. **AM32 uses CMSIS `NVIC_SetPriority` which automatically shifts `level << 4`**.

**Fix**: pass `level << 4` to `set_priority`. Applied in `mcu_l431/init.rs` (boot priorities) and `mcu_l431/chip.rs::adjust_irq_priorities` (dynamic swap).

**Companion fix (latent bug exposed by the priority change)**: with all IRQs at level 0, same-priority ARM tail-chaining serialized COMP and TIM6 — no preemption, so even an unmasked-and-bouncing comparator output couldn't starve TIM6. Once COMP→0/TIM6→3 priorities are honored, the latent bug surfaces: rm32 leaves `EXTI.IMR1[22]` unmasked outside active commutation. Comparator noise on undriven BEMF pins (Armed-idle, between commutation phases, after a failed startup) storms COMP_IRQ and freezes the firmware in seconds. **AM32 vs rm32 divergence**: AM32 calls `maskPhaseInterrupts()` at *every* stop/timeout site (~15 places in `main.c`); rm32 only masks on the LVC/`IsrAction::AllOff` path. StopMotor/Disarm via stuck-rotor, desync, signal_timeout all left COMP unmasked. **Mitigation applied**: `mcu_l431/interrupts.rs::COMP()` now masks `EXTI.IMR1[22]` on every ISR entry alongside the existing PR1 clear; `commutation_timer_expired` re-unmasks for the next BEMF window. Plus belt-and-suspenders: `control::isr_logic::ten_khz_tick` calls `comp.mask_interrupts()` when `!running`. Other MCU families (G071/F051/G431) have the same latent bug — same fix needed when porting the priority change.

AM32 priorities on L431 (mirror exactly):

| IRQ | Level | Why |
|---|---|---|
| `COMP` | **0** (highest) | BEMF zero-cross — must preempt `tenKhzRoutine` |
| `TIM1_UP_TIM16` | 0 | Commutation timer — same urgency as COMP |
| `DMA1_CH5` | 1 | DSHOT/PWM input capture |
| `EXTI15_10` | 2 | SW-triggered DSHOT frame processing |
| `DMA1_CH4` | 2 | USART1_TX (bench: telemetry off; matters when on) |
| **`TIM6_DACUNDER`** | **3** (lowest) | `tenKhzRoutine` runs long; **must be preempted** by motor-critical IRQs |

Without these priorities, `ten_khz_tick` blocks all motor-critical ISRs for its entire ~45 µs duration. Commutation/BEMF cannot fire mid-tick. Under load (BEMF re-lock cycles), commutation timing degrades → 100–200 ms motor chops.

Verified in hardware via `probe-rs read 0xE000E418` etc. — IPR bytes now show `0x00`/`0x10`/`0x20`/`0x30`.

### ISR pending-bit-clear audit (L431 done, others TODO)

Latent bug class: ISR body has a path that returns without clearing the IRQ source bit. NVIC sees the bit still pending → re-fires the ISR forever → 100% CPU in ISR storm → main starved.

| ISR | Bug | Status |
|---|---|---|
| `COMP` (L431) | `bemf_zero_cross` noise-filter early-return bypasses `comp.mask_interrupts()` which is the only path that clears `EXTI.PR1[22]`. | **Fixed** — ack EXTI line AND mask `IMR1[22]` at ISR entry (next `commutation_timer_expired` re-unmasks). `mcu_l431/interrupts.rs::COMP()`. |
| `DMA1_CH5` (L431) | Only cleared `CGIF5` inside the `if TCIF==1` branch. TEIE is enabled (`CCR5=0x098B`), so a transfer error alone would storm. | **Fixed** — clear `CGIF5` unconditionally at ISR entry. |
| `TIM6_DACUNDER` | Clears SR=0 at top. | OK |
| `TIM1_UP_TIM16` | Clears TIM16.SR at top. (TIM1.UIE is never enabled in our setup, so TIM1.SR doesn't matter.) | OK |
| `EXTI15_10` | Clears `EXTI.PR1[15]` at top. | OK |
| **F051 / G071 / G431** | Same `bemf_zero_cross` early-return path exists; their COMP ISR wrappers don't pre-ack the EXTI line. **Latent bug — same fix needed.** | **Pending** (task #39). |

The contract is documented at the top of `rm32::control::isr_logic::bemf_zero_cross`.

### Current investigation: 100–200 ms motor chops at ~40 % throttle

**The previous "TIM6 overruns its budget" diagnosis was a measurement artifact**. Earlier instrumentation read DWT.CYCCNT at TIM6 entry, called `dbg_isr_tick_inc` (atomic fetch_add) + a `rprintln!` heartbeat every 20000 ticks (with closure formatting + RTT byte writes), built MotorContext, ran `ten_khz_tick`, then called `dbg_tim6_max_cyc_update` (fetch_max LDREX/STREX). The heartbeat ran the timing bracket for 5000+ cycles when it fired, and `fetch_max` retained it as the sticky maximum. So `t6_max` reported 43–96 µs, looking like a budget overrun.

After stripping (rprintln removed; `fetch_max` → `store`; `dbg_isr_tick_inc` moved AFTER the bracket), the actual single-sample TIM6 cost (read as `t6_last` now, not `t6_max`):

```
mode=Armed (idle):    t6_last ≈ 546 cyc =  6.8 µs   (14 % of budget)
mode=Running:         t6_last ≈ 628 cyc =  7.9 µs   (16 % of budget)
mode=OldRoutine:      t6_last ≈ 743 cyc =  9.3 µs   typical
mode=OldRoutine:      t6_last ≈ 1526 cyc = 19.1 µs  occasional spike (ZC detection path)

TIM14 (commutation_timer_expired): 614–651 cyc = 7.7–8.1 µs
COMP  (bemf_zero_cross):           20–75 cyc   = 0.25–0.94 µs
```

**TIM6 has ~40 µs of headroom in every mode.** The motor chop is NOT caused by ISR overrun. The NVIC priority fix and COMP-mask compensating fix remain valid (they kept the firmware alive once COMP could preempt TIM6), but the chop root cause is something else — likely BEMF lock dynamics in `bemf_zero_cross` / `BemfState`, commutation timing in `commutation_timer_expired`, or motor mechanics. After the timing-instrumentation strip, motor visibly reaches Running mode multiple times per sweep where it previously barely flickered Running once.

Next: investigate why Running ↔ OldRoutine ↔ Armed oscillation happens at low/mid throttle. Compare BEMF timing parameters (filter_level, min_bemf_counts, com_timer_delay) against AM32 defaults and bemf_zero_cross arithmetic against C.

### Backlogged DSHOT/bidir work

| Item | Status |
|---|---|
| DSHOT decode buf[0] alignment | **WIP** (commit `2b92824`). NDTR bumped 32→33, decoder picks alignment dynamically (`ft_keep = buf[31]-buf[0]` vs `ft_skip = buf[32]-buf[1]`, smaller = real frame). Not fully validated. |
| DSHOT150 detection | Missing — `signal::detect_input` has buckets for DSHOT600/300, no 150. Fallback path used. |
| Bidir DSHOT auto-detect | Triggers on `hi_pin_n > 100` but spuriously hits non-bidir frames where line idles high briefly → applies CRC inversion to non-bidir frames → all fail. Needs self-validating detection (only commit after N successful inverted-CRC decodes). |
| Bidir GCR response | Encoded but unverified at high success rate. PA2 pull-up fix improved signal stability. |

### Reading port_41.log

Each `[loop n=X cyc_k=Y isr_tick=Z t6_last=A t14_last=B comp_last=C] ...` line:
- `n` increments every 100k main iters
- `cyc_k = CYCCNT/1000`, /80 → µs; wraps every ~53 s at 80 MHz
- `isr_tick` monotonic TIM6 ISR count (20 kHz)
- `t6/t14/comp_last` cycles (`/80` → µs) — SAMPLE from the most recently completed ISR at the time main read the field; plain store, no fetch_max. Read frequently to see distribution; one-off spikes won't be retained.
- normal `Δcyc_k ≈ 410k`, `Δisr_tick ≈ 100k` ; bigger deltas = main loop starved

If `[loop n=]` stops printing AND `dbg_isr_tick` still grows via `probe-rs read 0x200006a8` → main starved by ISRs. If neither grows → chip frozen.

Probe-rs gdb attach workflow (read PC + isr_tick without resetting):
```bash
probe-rs gdb --chip STM32L431KCUx --probe 0483:374f:0037002F3234510836303532 &
arm-none-eabi-gdb -batch \
  -ex "target remote localhost:1337" -ex "monitor halt" \
  -ex "p/x \$pc" -ex "x/wx 0x200006a8" \
  -ex "monitor resume" -ex "quit" \
  rm32_stm32/target/thumbv7em-none-eabihf/release/rm32_firmware
```

## Repo layout

- `rm32/` — portable, host-testable core (motor control, DSHOT/PWM decode, EEPROM config, state machines). No-std, no MCU deps.
- `rm32_stm32/` — STM32 firmware crate. Re-uses `rm32`. Per-MCU dirs `mcu_l431/`, `mcu_g071/`, `mcu_f051/`, `mcu_g431/`. Single point of MCU selection is `rm32_stm32/src/mcu.rs` via `pub use crate::mcu_xxx as active;` — all downstream re-exports go through `active`.
- `tests/blackbox/` — vector-driven blackbox tests. 72 passing + 6 xfailed.
- Board config lives in `rm32_stm32/boards/*.yaml` — build.rs resolves to `BoardConfig` const. 14 boards across 4 MCU families. `bemf_pins` is a required field (compile error if missing).

## Branches

- `origin = github.com/kaidokert/rm32.git`
- `private = github.com/kaidokert/rm32-private.git` — upstream coverage-test agent works here. Pull/push `private/priv_bringup` for active dev.
- Current branch: `priv_bringup`.

## Build & flash (L431, this bench)

```
# L431 with persistent debug UART on PB6 (recommended for bench debug)
cd rm32_stm32
cargo run --release --target thumbv7em-none-eabihf \
    --no-default-features --features stm32l431,debuguart
```

Cargo runner is already configured in `.cargo/config.toml` to invoke `probe-rs run --chip STM32L431KCUx --probe 0483:374f:0037002F3234510836303532`. Bench probe is ST-LINK V3 on USB `0483:374f`. The F411 disco's ST-LINK V2 (`0483:3748`) is the *wrong* probe; always pass `--probe` explicitly to OpenOCD/probe-rs when both are connected.

Other MCU targets build with their own target/features:
- `--target thumbv6m-none-eabi --features stm32g071`
- `--target thumbv6m-none-eabi --features stm32f051`
- `--target thumbv7em-none-eabihf --features stm32g431`

## Pre-commit hooks

`.pre-commit-config.yaml` runs **on every commit** (slow):
- `cargo fmt` — will reformat single-line `if`s into multi-line. If fmt modifies files, the commit *fails* with "files were modified by this hook" and you must `git add` the reformatted files and re-`git commit`.
- `cargo test -p rm32` (host tests, currently 252).
- `cargo clippy` on both crates.
- **Cross-build** for all four MCU targets: G071, F051, L431, G431.

If you only touched L431-specific code, the G071/F051/G431 cross-builds still run and gate the commit. Worth keeping in mind for fast iteration — you can sometimes use `git commit --no-verify` for WIP, but only if you know clippy/fmt would pass.

## Architecture — main loop unification (`run_tick`)

`rm32/src/system.rs::SystemTick::run_tick()` is the single canonical tick orchestration. Both harness and firmware call it. The callback closure handles platform-specific steps (ISR tick inline vs async, ISR→main sync via direct access vs `with_isr_state`).

**Why this matters:** Two bugs (stuck rotor stall rate, desync_check transfer) were caused by orchestration steps existing only in `harness.rs::do_tick` and being forgotten in `rm32_stm32/src/bin/main.rs`. `run_tick` eliminates the class: adding a step to it automatically applies to both paths.

### Cross-context communication pattern

`SharedState` (atomics) bridges ISR↔main. Three patterns:

1. **ISR action requests** (main→ISR, priority-ordered): `IsrAction` enum (`None`, `ResetIntervalTimer`, `AllOff`) stored as `AtomicU8` via `request_isr_action` (fetch_max for priority upgrade). ISR clears via `clear_isr_action`. Used by LVC, stuck rotor, stall handler.

2. **Event flags** (ISR→main, one-shot): `save_settings_flag`, `send_esc_info_flag`, `send_telemetry`. ISR sets, main clears after acting.

3. **State flags** (persistent): `forward`, `prop_brake_active`, `input_set`, `dshot`, etc. Read by either side.

`needs_reset` is a plain `bool` on `MainState` (not atomic) — it's only written and read in main-loop context.

## Board YAML — BEMF pin configuration

BEMF comparator input selection is per-board, configured in YAML:
```yaml
# Simple format (single comparator: L431, G071, F051)
bemf_pins:
  phase_a: PB7   # Symbolic pin name
  phase_b: PA5
  phase_c: PA4
  common: PB4

# Dual-comp format (G431)
bemf_pins:
  phase_a: { comp: 1, inm: PA5, inp: PA1 }
  phase_b: { comp: 2, inm: PA4, inp: PA3 }
  phase_c: { comp: 1, inm: PA0, inp: PA1 }
```

build.rs resolves pin names to MCU-specific packed register values via per-family mapping tables (`l431_comp2_inm`, `g071_comp2_inm`, etc.). The L431 encoding packs both `INMSEL[2:0]` (bits 6:4) and `INMESEL[1:0]` (bits 26:25) — the original port had wrong values selecting DAC channels instead of BEMF pins.

## HAL call counters (test infrastructure)

Both Rust and C harnesses expose `alloff_count`, `fullbrake_count`, `mask_interrupts_count` in state output. These record every safety-relevant HAL call for test assertions:
```
ticks 10002 | throttle=-1 | armed=0 alloff_count>0
```
Run the same vector against both harnesses — if C shows `alloff_count=1` and Rust shows `alloff_count=0`, the missing HAL call is caught. This found 3 missing `allOff()` calls (LVC, stuck rotor, motor restart).

**Harness mock wiring caveat:** `MockPhase` and `MockComp` hold `*mut HalCounts` raw pointers. `reset()` must re-wire them after `*self = Self::new()` because the struct moves. Known tech debt — should be `Rc<RefCell<HalCounts>>`.

## Persistent debug UART (`debuguart` feature)

When `--features debuguart` is on:
- `rm32_stm32::debug_uart` hijacks **PB6 / USART1** as a TX-only plaintext serial log at **115200 8N1**.
- Wire ESC PB6 (the AM32 KISS telemetry pad) → USB-TTL adapter RX, GND ↔ GND.
- The user keeps a continuous capture in `E:/m/robot/esc/port_41.log`. `tail` it to see boot trace + periodic `[loop]` heartbeat without needing probe-rs attached.
- Telemetry HAL is auto-disabled when `debuguart` is on (they fight over USART1).

`rm32_stm32::dprintln!` macro logs to both RTT and (when feature on) the UART.

## Reset semantics on this bench (critical)

The Vimdrones board has `AM32_L431_BOOTLOADER_PA2_V18` at `0x08000000` and rm32 (or AM32) at `0x08001000`. Bootloader has **two paths to the app**:

1. **First-chance** (`checkForSignal`): cumulative-low signal pin counter must exceed 450/4000 over a 40 ms window **AND** `RCC_CSR.SFTRSTF == 0`.
2. **Second-chance** (DFU loop fallback at `main.c:1040`): if `invalid_command > 100` (set when no idle on signal pin for 20 ms), jumps. No SFTRSTF gate.

EEPROM byte 0 at `0x0800F800` must be `0x01` for either jump (`jump()` checks `*(uint8_t*)EEPROM_START_ADD != 0x01 → return`).

Practical consequences:
- `probe-rs run` works reliably — SWD reset + multi-second flash halt makes BF's signal output back off, so the DFU loop times out cleanly and second-chance jump fires.
- **Physical NRST is marginal** — with BF actively driving DSHOT, the signal pin is HIGH ~89% of the time (observed `low_pin_count=451`, just 1 over threshold). Sometimes works, sometimes traps the chip in bootloader DFU forever.
- AM32 (when running) was usually NRST-recoverable because its active telemetry made BF behave differently.

When NRST seems to "hang" — check `port_41.log` first to see if rm32 actually banner'd. **Don't** `probe-rs read` to diagnose: it halts the core while the system timer keeps running, which fires the bootloader's 20 ms idle timeout artificially → chip boots → you've destroyed the evidence you were trying to read.

## Coverage gap audit

All 28 coverage gaps from `COVERAGE_GAPS.md` audited and closed or confirmed present. C-side coverage at 84.9% combined (unit + blackbox). Key fixes:
- BEMF pins (YAML-driven, fixed L431 INMSEL/INMESEL)
- LVC mode 2 (absolute cutoff)
- Signal timeout (armed + unarmed paths)
- Sine-to-BLDC changeover (10 missing state assignments)
- EDT disarm on zero (3 bugs)
- Bidir DShot auto-detect (high_pin_count counter)
- Stuck rotor stall rate (interval timer reset)
- RC-car init overrides
- Servo calibration EEPROM persistence
- Protocol re-confirmation (2-frame validation)
- Slow ramp mode, bidir changeover halving

## Currently working / known-broken

- **PWM, DSHOT300, DSHOT600**: working from cold boot. Live-switching between protocols relies on `signal_timeout > UNARMED` 2-s reset of `input_set`.
- **AM32 Configurator (am32.ca/configurator)** via BF 4way passthrough: working. Firmware self-resets on signal_timeout (`main_state.needs_reset`), bootloader enters DFU.
- **Stuck rotor protection**: working after interval timer reset fix.
- **DSHOT150**: not detected — `signal::detect_input` has buckets only for 300/600/servo.
- **Bidirectional DSHOT**: auto-detect implemented but telemetry response path needs work.

## Known tech debt

1. **Harness mock pointer unsafety** — `*mut HalCounts` raw pointers with manual re-wiring on reset. Should be `Rc<RefCell<HalCounts>>`.
2. **SharedComm trait bloat** — growing bag of methods. Consider sub-trait reorganization.
3. **5 sine xfails** — harness doesn't run the sine stepper loop (it's firmware main-loop specific). Would need Platform trait extension to inline sine stepping in harness.
4. **1 desync_recovery xfail** — may be fixable now that `sync_isr_to_main` is shared.

## Common gotchas this codebase has surfaced

- `signal::detect_input` subtracted DMA samples at u32 width — broken because TIM15 is **16-bit**. Fixed via `u16::wrapping_sub`. Also skip `buf[0]` (stale from previous DMA burst).
- Detection block returned the fast post-detection `CaptureConfig` on a single tentative match. Protocol re-confirmation now requires 2 consecutive matching frames.
- `TransferAction::DshotThrottle` had an EDT-armed gate that silently dropped vanilla DSHOT throttle (BF in plain DSHOT300/600 never sends `EDT_ENABLE`). Removed in firmware `isr_handlers.rs`.
- `BemfState.bad_count` (u8) overflows after 256 ticks with no valid BEMF — causes panic in debug builds. Fixed with `saturating_add`.
- Stuck rotor stall rate: rm32 didn't reset `interval_timer_count` after stall registration. Counter raced at main-loop rate (~8kHz) instead of C's throttled ~44Hz. Fixed via `IsrAction::ResetIntervalTimer`.
- Harness/firmware orchestration divergence: orchestration steps between shared function calls (desync_check transfer, interval timer reset) existed only in `harness.rs::do_tick`. Fixed by `SystemTick::run_tick()` unification.

## Per-MCU PAC quirks (RCC_CSR field names)

| Flag | L431 | G431 | G071 | F051 |
|---|---|---|---|---|
| Low-power | `lpwrstf` | `lpwrrstf` | `lpwrrstf` | `lpwrrstf` |
| Brownout-ish | `borrstf` | `borrstf` | `pwrrstf` | `porrstf` |
| Firewall | `firewallrstf` | *(absent)* | *(absent)* | *(absent)* |

L4's single-r `lpwrstf` is an ST SVD quirk; G0/G4/F0 all use double-r `lpwrrstf`. PAC accessor names captured in each MCU's `system.rs::read_and_clear_reset_cause()`.

## AM32 reference architecture (verified against `E:/m/robot/esc/AM32`)

Facts established when cross-checking rm32 behavior against AM32. All claims have file:line citations — re-verify before acting on them if a long time has passed.

### Four independent rate domains
AM32 deliberately decouples PWM, the control ISR, commutation, and BEMF sensing — they are *not* synchronized. Conflating them causes "where should this work live?" mistakes during parity work.

| Domain | Rate | Mechanism |
|---|---|---|
| Motor PWM carrier | **24 kHz** | TIM1 hardware-only. `TIM1.UIE` is never enabled — CPU never sees the 24 kHz tick. ARR computed per-MCU from `CPU_FREQUENCY_MHZ * 1e6 / NOMINAL_PWM - 1`. |
| Control / housekeeping ISR (`tenKhzRoutine`) | **20 kHz** | TIM6 update IRQ on L431 (`Mcu/l431/Src/peripherals.c:452-453` → `PSC=79`, `ARR = 1000000/LOOP_FREQUENCY_HZ = 50`). `LOOP_FREQUENCY_HZ` default = 20000 (`Inc/targets.h:5315`). |
| Commutation steps | RPM-dependent | TIM16 (`COM_TIMER`) reloaded with next interval at each BEMF ZC. |
| BEMF zero-cross detection | async, edge-triggered | COMP2 output → EXTI line 22, no PWM-phase gating. |

### `tenKhzRoutine` is misnamed — it's 20 kHz
`Src/main.c:1311-1312`:
```c
void tenKhzRoutine()
{ // 20khz as of 2.00 to be renamed
```
Vestigial name from AM32 1.x. All "10 kHz" references in our code and notes should be read as "the slow control-loop ISR @ 20 kHz". Throttle ramp, arming, LVC, stuck-rotor, signal timeout, telemetry counters, sine stepper, polling-mode startup commutation all live here.

### PWM frequency is 24 kHz uniformly
- `Inc/targets.h:5329-5337` — global default: `NOMINAL_PWM 24000U`, `TIM1_AUTORELOAD = CPU_FREQUENCY_MHZ * 1e6 / NOMINAL_PWM - 1`. Both `#ifndef`-guarded.
- **No board in upstream AM32 overrides `NOMINAL_PWM`.** Grepped — only occurrence is the default itself. All Vimdrones L431 variants (`VIMDRONES_L431`, `_CAN`, `_NANO_L431`, `_NANO_L431_CAN`, `_S50_L431`, `_S50_L431_CAN` at `targets.h:89-160`) inherit 24 kHz.
- Per-MCU ARR values (because CPU clock differs): F051/F031=1999, G071/G031=2665, **L431=3332**, F421=4999, G431=5999, CH32V203=1999 (hardcoded). All evaluate to ~24 kHz PWM.
- During slow-ramp startup `Src/main.c:627,630` temporarily divides ARR down then restores; brief, only at spin-up.
- **rm32 implication:** TIM1.ARR should be 3332 on L431 for parity. Existing CLAUDE.md register-parity section covers TIM1 CR1/CCMR/CCER/BDTR but not ARR — confirm `bringup.rs` writes 3332.

### COMP is fully async to PWM on L431
- `Mcu/l431/Src/peripherals.c:202` (COMP1) and `:260` (COMP2): `OutputBlankingSource = LL_COMP_BLANKINGSRC_NONE`. STM32 supports TIM1_OC4/OC5/TIM15_OC1 blanking but AM32 explicitly disables it.
- `Mcu/l431/Src/comparator.c::changeCompInput()` only switches floating-phase input and EXTI edge polarity per step. No TIM1 cross-trigger.
- `Mcu/l431/Src/stm32l4xx_it.c:276` (`COMP_IRQHandler`): the only filter is a software time gate — `INTERVAL_TIMER->CNT > average_interval/2` (elapsed-since-last-ZC), nothing to do with PWM phase. Early edges are swallowed by clearing the EXTI flag only if `getCompOutputLevel() == rising`.
- **Other AM32 MCU ports differ** — e.g. f421 *does* use OutputBlankingSource tied to TIM1. L431 does not.

### BEMF is sensed in all 6 floating windows (not half-rate)
- `Src/main.c:840-856` (`commutate()`): `rising = step % 2` (forward) / `!(step % 2)` (reverse), alternating polarity each step.
- `Mcu/l431/Src/comparator.c::changeCompInput()` phase mapping: step 1/4 → C floating, step 2/5 → A floating, step 3/6 → B floating.
- Result: each of the 3 phases is sensed twice per electrical period — once rising, once falling. 6 ZCs per electrical revolution. Standard "every floating window" scheme, not a half-rate scheme.
- Per single commutation step there is **exactly one** floating phase — two carry current, one is open and routed to COMP_INM. COMP_INP is COMMON_COMP (PB4, star-point virtual neutral).
- `#ifdef INVERTED_EXTI` flips `rising` polarity globally — board-level signal inversion compensation.
