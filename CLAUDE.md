# Claude Code working notes — rm32 / Vimdrones L431 bench

## Repo layout

- `rm32/` — portable, host-testable core (motor control, DSHOT/PWM decode, EEPROM config, state machines). No-std, no MCU deps.
- `rm32_stm32/` — STM32 firmware crate. Re-uses `rm32`. Per-MCU dirs `mcu_l431/`, `mcu_g071/`, `mcu_f051/`, `mcu_g431/`. Single point of MCU selection is `rm32_stm32/src/mcu.rs` via `pub use crate::mcu_xxx as active;` — all downstream re-exports go through `active`.
- `tests/blackbox/` — vector-driven blackbox tests, growing rapidly upstream on `private` remote.

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
- `cargo test -p rm32` (host tests, currently 248+).
- `cargo clippy` on both crates.
- **Cross-build** for all four MCU targets: G071, F051, L431, G431.

If you only touched L431-specific code, the G071/F051/G431 cross-builds still run and gate the commit. Worth keeping in mind for fast iteration — you can sometimes use `git commit --no-verify` for WIP, but only if you know clippy/fmt would pass.

## Persistent debug UART (`debuguart` feature)

When `--features debuguart` is on:
- `rm32_stm32::debug_uart` hijacks **PB6 / USART1** as a TX-only plaintext serial log at **115200 8N1**.
- Wire ESC PB6 (the AM32 KISS telemetry pad) → USB-TTL adapter RX, GND ↔ GND.
- The user keeps a continuous capture in `E:/m/robot/esc/port_41.log`. `tail` it to see boot trace + periodic `[loop]` heartbeat without needing probe-rs attached.
- Telemetry HAL is auto-disabled when `debuguart` is on (they fight over USART1).

`rm32_stm32::dprintln!` macro logs to both RTT and (when feature on) the UART.

## Standalone UART regression bench

`E:/m/robot/esc/uart_test/` (separate crate, not part of rm32 repo) has three examples that isolate the debug-UART module:
- `cargo run --release --example hal_uart` — HAL-driven clocks + HAL serial
- `cargo run --release --example raw_pac_uart` — rm32's verbatim `debug_uart` module on its own
- `cargo run --release --example hal_clocks_raw_uart` — HAL clock setup + raw-PAC UART (matches rm32's startup ordering)

Useful when `debug_uart` is silent in rm32 and you want to know whether the module itself broke or rm32's environment changed.

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

## AM32 bootloader debug knobs

In `E:/m/robot/esc/AM32-bootloader/bootloader/main.c`:
- `#define SERIAL_STATS` (line 37) — enables a `stats` struct (`no_idle`, `no_start`, `bad_start`, `bad_stop`, `good`) in SRAM that counts receive state machine outcomes. Read via probe-rs to diagnose stuck-in-DFU. **Note**: the receive flow only bumps `no_start` after at least one valid byte (`messagereceived=true`), so if `good=0` you'll never see `no_start>0`.
- `#define BOOTLOADER_TEST_STRING` — bootloader spews `"HELLO_WORLD"` on PA2.
- `#define BOOTLOADER_TEST_BKUP` — exercises RTC backup registers.
- Rebuild with `tools/windows/make/bin/make.exe AM32_L431_BOOTLOADER_PA2`. The system GCC 14.2 breaks on `-Werror=array-bounds` — use the bundled GCC 10.3.1.

The bootloader **does not** touch RTC backup registers in production. They're free for rm32 to use as boot-survivor state if useful.

## Per-MCU PAC quirks (RCC_CSR field names)

| Flag | L431 | G431 | G071 | F051 |
|---|---|---|---|---|
| Low-power | `lpwrstf` | `lpwrrstf` | `lpwrrstf` | `lpwrrstf` |
| Brownout-ish | `borrstf` | `borrstf` | `pwrrstf` | `porrstf` |
| Firewall | `firewallrstf` | *(absent)* | *(absent)* | *(absent)* |

L4's single-r `lpwrstf` is an ST SVD quirk; G0/G4/F0 all use double-r `lpwrrstf`. PAC accessor names captured in each MCU's `system.rs::read_and_clear_reset_cause()`.

## Currently working / known-broken

- **PWM, DSHOT300, DSHOT600**: working from cold boot. Live-switching between protocols is iffy (relies on `signal_timeout > UNARMED` 2-s reset of `input_set`).
- **AM32 Configurator (am32.ca/configurator)** via BF 4way passthrough: working. The Configurator talks to the **bootloader**, not the running firmware — every read/write/erase is a BLHeli protocol byte hitting `AM32-bootloader/main.c::receiveBuffer`. The app firmware's only job in this flow is to self-reset when `signal_timeout` fires (BF stops DSHOT during passthrough → signal_timeout grows → `sys.reset()` → SFTRSTF set → bootloader skips first-chance gate → DFU loop activates). See `rm32_stm32/src/bin/main.rs` `if shared.needs_reset()` block.
- **DSHOT150**: not detected — `signal::detect_input` has buckets only for 300/600/servo. Easy add.
- **Bidirectional DSHOT**: handshake didn't complete. `bidir_detected` flow in `transfer.rs` triggers `shared.set_dshot_telemetry(true)` after 100 high-idle frames while unarmed, but telemetry response path or CRC inversion likely needs work. TBD.

## Common gotchas this codebase has surfaced

- `signal::detect_input` originally subtracted DMA samples at u32 width — broken because TIM15 is **16-bit**. Fixed via `u16::wrapping_sub`.
- DMA circular buffer leaves `buf[0]` as the previous burst's last edge — the `buf[0]→buf[1]` delta is an inter-frame gap, not a bit pulse. Skip the first slot.
- Detection block returned the *fast post-detection* `CaptureConfig` on a single tentative match, dropping PSC before re-confirmation could fire. Confirmed-only PSC switch lives in `transfer::process` now.
- `TransferAction::DshotThrottle` had an EDT-armed gate that silently dropped vanilla DSHOT throttle (BF in plain DSHOT300/600 never sends `EDT_ENABLE`). Removed in `isr_handlers.rs:228`; same gate still in `harness.rs:292` for now (host-test parity question).
