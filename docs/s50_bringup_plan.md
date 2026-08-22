# S50 Rust bring-up plan — staged, register-parity, two wires

Vimdrones S50 ESC, second bench. Goal: rm32 running on this board,
validated against the stock-AM32 reference card measured on the same
hardware. Method: minz-style *staged* bring-up (thin scaffold first,
register parity against the live AM32 reference), NOT a full clone —
the portable control core is already bench-proven on L431.

## NAMING TRAP (read first)

The silicon is **STM32G051G8U6**. Everything in the AM32 world calls
this board's build "G071" (`VIMDRONES_S50_G071`, `Mcu/g071/`,
`HARDWARE_GROUP_G0_A`) — that is a *software family name*; the G071
build runs on the G051 die. Consequences:

- **RAM is 18 KB** (G051), not the G071's 36 KB. Stack ceiling
  0x20004800. Factory app uses SP=0x20004000 — mirror that. A build
  with a 36 KB RAM profile (SP 0x20009000) faults on the first push:
  instant lockup, pre-diagnosed 2026-08-22 from the chip marking.
- Flash: 64 KB (flash-size reg confirms). EEPROM page at 0x0800F800.
- PAC for the bringup crate: `stm32g0` with feature `stm32g051`
  (register truth for THIS die). rm32_stm32's existing G071 target
  uses the G071 PAC — register-compatible for the AM32-exercised
  subset; verifying that IS one purpose of the bring-up.

## Board + bench identity (as verified on silicon)

- Chip: STM32G051G8U6, UFQFPN28. Marking: `G0518 GQ21V14 9RCHNZ 609`.
  DBGMCU IDCODE 0x10016456 (dev_id 0x456 = G05x/6x, rev 0x1001).
  Flash 64 KB, RAM 18 KB, RDP level 0 (option bytes @0x1FFF7800:
  `fffffeaa 00000155 ffffffff 00000000`).
- Firmware stack ON THE BOARD (2026-08-22, all locally built +
  verified): bootloader = `AM32_G071_BOOTLOADER_PB4_64K` (stock
  source ae0de56 in E:/m/robot/esc/AM32-bootloader; bench-hardcode
  unconditional-jump variant available at 9ef0c06); app =
  `AM32_VIMDRONES_S50_G071_2.20` (E:/m/robot/esc/AM32; bench study
  switches gated `MCU_L431` so G0 builds are stock).
- Probes:
  - **J-Link Ultra: `1366:1020:000506004154`**, SWD.
    Reads: `--chip STM32G071GBUx` works. **Flashing: use
    `--chip STM32G051C8`** (G051G8 not in probe-rs list; C8 = same
    64 KB family/flash algorithm).
  - ST-LINK V3 `0483:374f:0037002F3234510836303532` belongs to the
    L431 bench — do not confuse.
- FC: Betaflight F411 on **COM42** (115200 CLI/MSP). Config:
  DSHOT300, `dshot_bidir=ON`, `motor_poles=14`, `dshot_edt=ON`.
- USB-TTL adapter: **COM41**, 115200 (KISS / future debug UART).
- Power: currently bench PSU — **duty capped at 50%** until the
  battery lead is rebuilt (XT30/XT60 + 16 AWG incoming; the old
  SM/22AWG lead melted at ~0.65 ohm). Acceptance for the new lead:
  sag-vs-current slope < 0.1 ohm.
- Backups: `E:/m/robot/esc/s50_backup_factory/` — full 64 K dumps
  (2026-08-20 + 08-22), bootloader/app splits, VIRGIN eeprom page
  reconstruction, option bytes, README with restore commands.
  Restore-all: `probe-rs download --chip STM32G051C8 --probe
  1366:1020:000506004154 --protocol swd --binary-format bin
  --base-address 0x08000000 s50_factory_full_64k.bin`
- SAFETY INVARIANT: EEPROM `dir_reversed[17] = 1` (motor pushes DOWN
  on the bench). Every config write and every EEPROM restore must
  preserve it. The virgin-factory page has it 0 — re-set after any
  virgin restore.

## Pin map / peripheral roster (from AM32 HARDWARE_GROUP_G0_A + S50)

Only TWO signal pins are available on the bench, plus SWD. All
instrumentation must fit on these wires:

| Wire | Pin | Role |
|---|---|---|
| Signal (from FC motor pad) | **PB4** | DSHOT300 in: TIM3_CH1 input capture, DMA1_CH1, DMAMUX req TIM3_CH1. Also the bootloader's input pin, and bidir DSHOT response out. Host→ESC path (throttle, DSHOT commands, 4-way/bootloader protocol). |
| Telemetry (to USB-TTL RX = COM41) | **PB6** | USART1 TX (AF0), DMA1_CH3 (DMAMUX USART1_TX), 115200 8N1. KISS telemetry on stock AM32; becomes rm32's debuguart/bench print channel. ESC→host path. |
| SWD | PA13/PA14 + GND + 3V3 sense | J-Link: flash, RAM liveness, register dumps, gdb. |

Motor/analog (fixed by the board, listed for init parity):

- TIM1 = motor PWM, CCR1/2/3; complementary bridge pins:
  A: low PB1, high PA10; B: low PB0, high PA9; C: low PA7, high PA8.
  DEAD_TIME = 120 (timer ticks).
- Comparator: **COMP2 only** (`active_COMP = COMP2`), INP = common;
  INM per phase: A = IO2 (PB7), B = IO1 (PB3), C = IO3 (PA2).
  COMP1→EXTI line 17, COMP2→**EXTI line 18** (IT handler services
  line 18 rising+falling).
- EXTI line 15: software-triggered (SWIER1) frame-processing IRQ,
  same pattern as L431's EXTI15_10.
- Timer roster (targets.h G0 section): INTERVAL_TIMER=**TIM2**,
  TEN_KHZ_TIMER=**TIM6**, COM_TIMER=**TIM14**, UTILITY_TIMER=TIM17.
- ADC: voltage PA6 = ch 6 (divider 110), current PA5 = ch 5
  (**10 mV/A**, offset 3).
- Board YAML deltas vs our generic `gen_64k_g071.yaml` (which is a
  DIFFERENT ESC — do not reuse): current ch 5 not 4; 10 mV/A not 20;
  offset 3; dead_time **120** not 60; comparator A/B swapped
  (S50: A=IO2/PB7, B=IO1/PB3 — generic file has A=PB3, B=PB7).

## Two-wire science model

- ESC→host: everything prints on PB6 → COM41 (banner, heartbeats,
  counters, frame snapshots). One-way, 115200.
- Host→ESC: only via PB4 = the DSHOT stream (BF MSP_SET_MOTOR for
  throttle, DSHOT commands for levers) or the 4-way/bootloader
  protocol (`scripts/bf_4way.py` — read AND write EEPROM). No
  soft-UART pin on this board; config experiments go through 4-way.
- SWD covers the rest: register dump/diff, RAM-liveness
  (`probe-rs read` twice + diff), force-jump diagnostics, reflash.
- Host instruments already proven on this board:
  `scripts/bf_4way.py` (ID + full EEPROM read/write, preserves
  dir_reversed), `scripts/s50_ladder.py`, `scripts/s50_fullqual.py`
  (MSP drive + bidir rpm/invalid% + KISS volt/current with
  median-of-5 filtering — raw KISS corrupts ~11% under motor load,
  CRC8 leaks ~1/256).

## Reference card to beat (stock AM32, this exact board+motor)

- Ladder 10→100%: eRPM 4.7k→22,236 (~3,177 rpm mech), 0.00% invalid,
  monotone to ~86% (top flattening = old resistive supply path).
- Low end: cold-engage reliable ≥6.0%; **dead notch at 5.0%** (0/9,
  accelerates to ~3,400 eRPM, never locks); patchy 4.0–5.5%;
  hold floor 4.0% (632 eRPM); dropout 3.8%.
- The 5.0% notch is a sharp parity probe: rm32 reproducing it (or
  not) is information either way.

## Stage 0 — scaffold (desk)

```
cd E:/m/robot/esc
cargo new --lib s50_bringup
cd s50_bringup
```

- `Cargo.toml`: `stm32g0 = { version = "*", features = ["stm32g051",
  "rt"] }`, `cortex-m`, `cortex-m-rt`, `panic-halt`. `[[bin]]` or
  examples per stage. Profile: `opt-level = "s"`, LTO fine (M0 has
  no DWT; the L431 LTO/stack scars still apply — check `sub sp` vs
  RAM if boot loops).
- `memory.x`: `FLASH : ORIGIN = 0x08001000, LENGTH = 59K` (app slot
  under the bootloader + EEPROM page), `RAM : ORIGIN = 0x20000000,
  LENGTH = 16K` (SP = 0x20004000, mirroring factory; 2 K margin
  under the 18 K physical ceiling).
- `.cargo/config.toml`: target `thumbv6m-none-eabi`, runner
  `probe-rs run --chip STM32G051C8 --probe 1366:1020:000506004154
  --protocol swd`.
- Keep the AM32 bootloader resident: we flash apps at 0x08001000
  only; jump gate = EEPROM byte0 == 0x01 (already set). For rapid
  iteration the bench-hardcode bootloader (9ef0c06 build for
  G071_64K/PB4) can replace stock — rebuild from source, never trust
  obj/ binaries (L431 scar).

## Golden snapshot — BEFORE any Rust flashes

Adapt `scripts/dump_l431_regs.py` → `dump_g0_regs.py`: with stock
AM32 running (armed idle, BF streaming), read via J-Link without
halting: RCC (clock tree), GPIOA/B (MODER/AFR/PUPDR/OSPEEDR), TIM1
(CR1/CCMR/CCER/BDTR/ARR/PSC), TIM2/TIM3/TIM6/TIM14/TIM17, DMA1 +
DMAMUX channels, ADC1 (CFGR/SMPR/CHSELR/CALFACT), COMP1/COMP2 CSR,
EXTI (IMR1/FTSR1/RTSR1), USART1, NVIC IPR/ISER, SCB VTOR/AIRCR.
Save as `s50_golden_regs_am32.txt`. This is the answer key for every
later stage. (L431 lesson: register parity beats symptom debugging.)

## Stage 1 — hello silicon

Thin binary: clock to 64 MHz (mirror AM32's RCC golden values
exactly), USART1/PB6 TX at 115200, banner + 1 Hz heartbeat with a
loop counter. FETs: before anything else, configure all six phase
pins GPIO-output-LOW (bridge safe). No IWDG yet.

Pass: banner on COM41; register diff vs golden ≈ clock+GPIO+USART
sections match; SP sane over SWD; survives 10 min.

Failure toolkit: SWD RAM-liveness diff, gdb PC read, boot-banner
count (crash loop), `sub sp` vs 16 K.

## Stage 2 — ears before muscles

Add TIM3_CH1 + DMA1_CH1 capture on PB4 (32-edge frames, NDTR=32 —
the frame-lock rule; a 33-edge window slides and never re-locks,
bench-bisected on L431, #65), EXTI15 SW-trigger processing, DSHOT300
decode. Heartbeat gains `crc_pass/crc_fail/frames` counters + one
frame's edge deltas (the [snap] pattern). Motor outputs stay
GPIO-low the whole stage.

Pass: with BF streaming, crc_pass climbs, crc_fail ~0 (reference:
stock link measured 0.00% invalid all week); prescaler/edge timing
matches the L431-derived expectations (46/87 ticks at DSHOT300 with
psc=1 @64 MHz → recompute for 64 MHz: verify against captured
deltas, don't assume).

Pass criterion 2: NVIC priorities honored (M0: 2-bit, levels 0-3 —
mirror AM32's G0 assignment from the golden NVIC dump).

## Stage 3 — full rm32

Switch to the real stack: `rm32_stm32` `stm32g071` feature +
- new `boards/vimdrones_s50_g071.yaml` (deltas listed above),
- G051 memory profile in build.rs (16 K RAM, SP 0x20004000 —
  **not** the G071's 36 K),
- debuguart ported to G0 (USART1/PB6 — same pin as KISS; they own
  it exclusively, exactly like the L431's PB6 arrangement),
- bench instrumentation: dsy/crc/[sr] counters work as-is (no DWT
  needed); DWT cycle-timing brackets and bench_guard's CYCCNT
  timebase DON'T EXIST on M0+ — stub them or use TIM17, defer.

Sequence: flash → banner → detection/decode counters (Stage 2
criteria met inside the full stack) → arm → 10–25% spins →
`s50_ladder.py` small qual vs the stock card → low-end card
(hold floor, engage notch) → full qual after the battery-lead
rebuild.

Every EEPROM interaction preserves `dir_reversed=1`. Duty ≤50%
until the new battery lead passes the <0.1 ohm acceptance test.

## Restore paths (fearless flashing)

- App only: reflash `AM32_VIMDRONES_S50_G071_2.20.bin` at
  0x08001000 (verified working 2026-08-22, spins 6,871 eRPM @15%).
- Everything: `s50_factory_full_64k.bin` at 0x08000000 (then re-set
  dir_reversed if the virgin-eeprom variant was used).
- The 4-way path (`bf_4way.py`) works through ANY app state as long
  as the bootloader region is intact — SWD-less recovery exists.
