# Claude Code working notes — rm32 / Vimdrones L431 bench

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
