# rm32 on the binz bench (NUCLEO-G071RB + BOOSTXL-DRV8304H)

Branch `g071_binz`. Goal: rm32 + rm32_stm32 spinning at low speed on the
bench where binz/firmware50 qualified, with every feedback channel sane
(bus voltage, current, nFAULT, eHz agree with firmware50 at equal duty).

## Build / flash

```
cd rm32_stm32
BOARD=boards/binz_drv8304h_g071.yaml cargo build --release --bin rm32_firmware \
    --no-default-features --features stm32g071,benchuart
probe-rs download --chip STM32G071RBTx --probe 0483:374b:066CFF343433464757233430 \
    target/thumbv6m-none-eabi/release/rm32_firmware
probe-rs reset --chip STM32G071RBTx --probe 0483:374b:066CFF343433464757233430
```

No bootloader on this board: `bootloader: false` links at 0x08000000.
Bench console = FTDI on USART3 PC10/PC11, COM41, 115200 (NOT the ST-LINK
VCOM — PA2/PA3 are COMP2 inputs). `scripts/binz_console.py` (listen / `i`),
`scripts/binz_spin.py` (arm / low-duty spin, kill on every exit path),
`scripts/binz_regdump.py` (idle SWD register snapshot + diff).

Restore firmware50 exactly: `captures/binz/fw50_flash_backup_128k.bin`
(SHA-256 92c7fe2e…6d5e), full 128K read at 0x08000000 before the first rm32
flash.

## Board extensions (rm32_stm32 only — rm32::board untouched)

`boards/*.yaml` gained optional fields, emitted as `BOARD_EXT`
(`src/board_ext.rs`): `bootloader`, `gate_enable_pin` (PD1), `nfault_pin`
(PB14), `current_shunt_channels` ([0, 1, 4] → `cfg(rm32_three_shunt)`),
`bench_dir_reversed`.

- Gate policy: TIM1 MOE and DRV8304 ENABLE follow `armed` (main loop,
  `BoardExt::drive_gate`); only on boards with an ENABLE pin.
- Current: sum of the three low-side CSAs, zero captured per enable after
  1.5 s (driver awake, bridge idle, each CSA within 1600..2500 counts or the
  zero is REJECTED and throttle is held at 0). firmware50's metered
  calibration verbatim, so both report the same mA. `b` line `imean_ma` =
  per-scan mean since the previous `i`.
- Bench guard (`BINZ_BRINGUP`): 500 mA / 20 ms, >10 % sag vs pre-run rest
  peak / 5 ms, 9 V floor, 14 V OV, nFAULT (ignored 2 ms after enable).
- Direction: firmware50 `Wiring = Reverse` walks physical sectors
  4,3,2,1,6,5 on the AM32 sector table = AM32/rm32 `dir_reversed = 1`.

## Stage 1 (motor supply off) — defects found and fixed

All found on the G071 path, which had never run on hardware:

1. **IWDG start hang** (`mcu_g071/system.rs`): waited for SR.PVU/RVU before
   starting the IWDG — never clears; boot hung after the startup tune.
2. **ADC never initialised** (`mcu_g071/adc.rs`): ADVREGEN never set (and CR
   writes cleared it) → ADCAL never completed; the error was discarded.
3. **ADC sequence ignored**: CHSELR written without the RM0444 CCRDY
   handshake → CHSELR = 0 → every slot converted IN0 (all channels read
   shunt A). firmware50 hit the same rule.
4. **TIM1 output compare never configured** (`mcu_g071/pwm.rs`): HAL
   `bind_pin` left CCMR = frozen and CCER = 0x444 — high sides could never
   switch. Now AM32's 0x6868 / 0x555 / ARPE.
5. **Low-side AF**: PA7/PB0/PB1 had no AF2 → "alternate" low side would
   route AF0 (SPI1/TIM14). Now AF2, all gates parked output-low at init.
6. **NVIC priorities unset** (all level 0) → AM32 levels, `<< 6` for the
   M0+'s 2 priority bits.
7. **build.rs**: `rerun-if-env-changed=BOARD` only printed when BOARD was
   set → `BOARD=...` after a default build reused the cached board config.

Register diff vs firmware50 idle (`captures/binz/fw50_idle.txt` vs
`rm32_idle2.txt`): remaining differences are architectural or intended —
24 kHz AM32 carrier vs firmware50 10 kHz startup carrier, DTG 60 (AM32 value)
vs 26, TIM14/TIM2 (rm32) vs TIM16/TIM17 (firmware50) timer roles, software-
vs TIM6-triggered ADC, IWDG 0.5 s vs 50 ms, firmware50's OSSR/OSSI.
COMP2 is identical (0x40000281).

Arm-only check (supply off): armed=1 → en=1, moe=1; kill drops them. Zero
correctly REJECTED with VM off (CSAs unpowered).

## Stage 2 (motor powered, bench PSU 11.7 V, 800 mA clamp) — 2026-10-02

Kill limits: 750 mA / 20 ms (operator raised the PSU clamp to 800 mA),
>10 % sag vs rest, 9 V floor, nFAULT. Startup = AM32 sine start
(`bench_sine_start`, power 3, changeover 5 %), then `binz_spin.py` walks
the bench command 5 % -> setpoint at 1 %/s (`--quiet`: no UART traffic from
changeover to end of hold — TX couples into the comparator on this bench).

Defects found and fixed on the way:

8. **Sine start was dead in firmware**: nothing set `stepper_sine` or put the
   bridge on all-phase PWM. Wired per AM32 setInput / main-loop stepper
   (`3e01b52`). AM32's six-step kick on a stalled rotor (no sine) read
   550-800 mA on this low-resistance motor.
9. **G071 COMP wrapper had no acceptance gate**: acked EXTI at entry and
   evaluated every edge, so early-window ringing was accepted — closed loop
   but ~11 desyncs/s at ~2x the rotor's commutation rate. Ported the L431
   rung-5b wrapper (AM32 `ADC1_COMP_IRQHandler`: half-average gate + camping)
   (`0132dd5`). dsy 116 -> 0 in a 10 s hold.
10. **Current channel aliased**: one ADC scan per 1 kHz dispatch; 64000 mod
    2666 = 16 cycles, so the sample crept through the 24 kHz PWM period and
    sat in the high-side window ~17 ms at a time (~2 A readings), tripping the
    OC kill at a steady, desync-free 504 eHz. Now one scan per 20 kHz tick
    (534-cycle stride), averaged per 1 kHz read; `capture_zero` averages the
    tick's scans (its ADSTART poll starved against the tick -> IWDG reboot).

Same-supply A/B at 15 % duty (firmware50 `DFD4B1DD.env99-cap987` command `9`,
rm32 bench 13 % = duty 304/2000 = 15.2 %), three runs a side, interleaved:

| | firmware50 | rm32 | agree |
|---|---|---|---|
| eHz | 674 / 674 / 674 (coast 669-675) | 732 / 739 / 737 | +7 % (duty-scaled ~+7 %) |
| bus | bus_ref 1212-1214 = 11.66 V | 11.62-11.64 V | <1 % |
| nFAULT | clear | clear (nf=0 every sample) | yes |
| desync | — | dsy=0, zc saturated | |
| hold current (metered fit, pre-run zero) | 138 / 159 / 153 mA | 335 / 213 / 245 mA | **no: +40..+110 %** |
| post-run idle reading (bridge off) | zero_drift -56 mA (run 1) | 204 / 63 / 99 mA (pre-run ~100) | |

Current is instrument-limited on both firmwares: the CSA zero drifts during
a run (firmware50 E079: up to 150 mA-equivalent, erratic). Referencing each
rm32 hold to the mean of its pre/post idle readings gives ~183 / 131 / 145 mA,
near firmware50's hold — suggestive, not a measurement. A PSU-meter reading
during a 15 % hold of each firmware is what settles the current channel.
Stop: every run ends run=0, duty=0, outputs off.

### Stage 2b — carrier, dead time, zero (same day)

11. **Carrier**: firmware50 runs closed loop at 48 kHz; AM32's default 24 kHz
    (variable PWM stays at full ARR at this speed) doubles ripple on this
    low-inductance motor. `bench_pwm_khz: 48` (AM32 `pwm_frequency`): eHz
    +7 % -> +0.5 %, hold current 264 -> ~200 mA.
12. **Dead time 60 -> 26** (firmware50's value, qualified 0-100 % here):
    at 48 kHz, 60 ticks = 9 % of each period on body diodes. Hold ~199 ->
    ~180 mA, eHz unchanged.
13. **Zero from 64 -> 2000 scans** (100 ms): per-enable zero noise was
    +/-25 mA run to run; armed-idle now reads 97-103 mA = the metered fit's
    102.6 mA intercept, i.e. an exact zero.

Final A/B, 15 % duty (rm32 15.2 %), same PSU, firmware50 interleaved:

| | firmware50 (5 runs) | rm32 (5 runs) | delta |
|---|---|---|---|
| eHz | 674 x5 | 691-695 | +0.4..+1.1 % duty-scaled |
| bus | 11.65-11.67 V | 11.63-11.64 V | <1 % |
| nFAULT | clear | clear | — |
| desync | — | dsy=0 every run | — |
| hold current | 138 159 153 118 138 (mean 141, +/-15 %; zero drift -140..+13) | 170 180 189 181 178 (mean 180, +/-5 %) | **+28 %** |

Superseded by 2c below (the +28 % was the instrument, not the motor).

### Stage 2c — current instrument (same day)

14. **Scan timing biased the current**: software-started scans (from the
    TIM6 ISR) cluster around commutations because COMP/TIM14 preempt or
    delay TIM6 — the hold read 180 mA with the start at ISR end and 263 mA
    at ISR entry, same speed. Now firmware50's method: TIM6 TRGO hardware-
    triggers each scan (EXTSEL 0b101, ADC-side circular DMA, DMA re-aligned
    to slot 0 at arm), the tick only drains the finished scan.
15. **Zero tracks the CSA settling**: the amplifiers settle for 4-8 s after
    ENABLE (idle drifted +23 mA after a 1.5 s zero). The zero now follows a
    64 ms EWMA (1/16-count precision) while armed with the bridge idle and
    freezes from the first throttle; armed-idle reads 100-106 mA = the
    metered fit's 102.6 mA intercept.

Final same-supply A/B, 15 % duty (rm32 304/2000 = 15.2 %):

| | firmware50 (7 runs) | rm32 (5 runs) | delta |
|---|---|---|---|
| eHz | 674 x7 (683 scaled to 15.2 %) | 695-697 | +1.8..2.0 % |
| bus | 11.65-11.67 V | 11.63-11.68 V | <1 % |
| nFAULT | clear | clear | — |
| hold current | 138 159 153 118 138 157 148 (mean 144) | 153 155 110 158 139 (mean 143) | -1 % mean; interleaved pairs +0.6 %, -6 % |
| desync / stop | — | dsy=0 every run; run=0 duty=0 outputs off | — |

Both current proxies scatter ~+/-15 % run to run (one low outlier each);
the means agree. A PSU-meter reading remains the absolute anchor.

## Stage 3 — envelope climb, PSU clamp 1 A (2026-10-02, `devel`)

Kill limits: OC 900 mA / 20 ms on the AM32 50 ms moving average, >10 % sag
vs rest, 9 V floor, nFAULT; `w` on every exit. Rungs are APPLIED duty
(`binz_spin.py --rung <tenths>`, inverse sine-start map, cap raised one rung
at a time), 3 quiet 15 s holds each, walking 1 %/s from changeover
(`scripts/binz_rung.sh`). Numbers below are read from each capture's raw
end-of-hold `i`/`b` lines.

**Result: 32.5 % qualified (3/3). Climb stopped at 35.0 %: OC kill 2/2 on
the final image, both locked (dsy=0, zc saturated, ci = avg) — steady
current ~0.75 A + ripple crosses 900 mA (firmware50 draws 834 mA steady at
37.5 % on the same supply).**

| rung | duty | eHz (3 runs) | desync | firmware50 (same supply) | eHz delta |
|---|---|---|---|---|---|
| 15.0 % | 300 | 677 678 680 | 0 | 674 | +0.6 % |
| 17.5 % | 349 | 805 807 807 | 0 | — | — |
| 20.0 % | 399 | 925 925 925 | 0 | 905 (cmd `2`) | +2.4 % |
| 22.5 % | 449 | 1031 1038 1031 | 0 | — | — |
| 25.0 % | 499 | 1153 1145 1149 | 0 | 1118 (cmd `5`) | +3.0 % |
| 27.5 % | 549 | 1262 1267 1248 | 0 | — | — |
| 30.0 % | 600 | 1355 1349 1355 | 0 | — | — |
| 32.5 % | 648 | 1455 1461 1461 (final image) | 0 | 1387 interp. 25 %/37.5 % (1118/1572) | +5.2 % |
| 35.0 % | 700 | 1564 1572 1557 (lattice image); OC kill 2/2 final image | 0 | — | — |

32.5 % hold-only current on the final image: 665 / 658 / 649 mA (50 ms
peaks 906 / 890 / 867); firmware50 interpolated 643 mA (+2 %). Rungs 15-30 %
ran on image `be0e7645` (TIM6-TRGO lattice instrument, identical COM/COMP
code); their `imean` values mixed walk + hold and are not quoted as hold
currents.

### WCET gate (final image `4e21b2ba`, `scripts/binz_wcet.py`)

Method: firmware50's `isr_cycles.py` (copied to `scripts/`, plus a
`--terminal` option so panic paths and the one-time `take_isr_state` lazy
move end a path instead of being costed; both listed for review). Reviewed
loop bounds are in `binz_wcet.py` with their source reasons. Static
instruction counts follow only reachable code.

| root | prio | insns | AM32 | ratio | cyc @0 WS | cyc @2 WS fetch | us |
|---|---|---|---|---|---|---|---|
| TIM14 (COM) | 0 | 1148 | 674 | 1.70 | 1096 | 1442 | 22.5 |
| ADC_COMP | 0 | 165 | 131 | 1.26 | 376 | 466 | 7.3 |
| TIM6 tick | 2 | 2704 | 1637 | 1.65 | 3898 | 5278 | 82.5 |
| DMA1_CH1 | 1 | 455 | — | — | 1179 | 1409 | 22.0 |

COM + COMP = 29.8 us: 26 % of a step at 32.5 % (1460 eHz), 28 % at 35 %;
the < 50 % budget holds to ~2800 eHz. The TIM6 figure is a static bound
over mutually exclusive branches (tones / sine / polling startup) and
exceeds its 50 us period — open review item; TIM6 is priority 2 and cannot
delay COM/COMP.

Code changes made by the gate: `tone::Note` 4-byte aligned (its 6-byte
`.copied()` pulled the 297-instruction generic `memcpy` into the tick);
TIM6 ratio 2.04 -> 1.86 before the dependency fix below.

### Current instrument, climb findings

16. **Lattice aliasing at high duty**: TIM6-TRGO scans sample a 5-phase
    lattice of the 48 kHz carrier (3200 mod 1333 = 534 = 0.40 period) that
    drifts over ~83 ms; at 37.5 % each 1 ms mean held 1 or 2 of 5 points in
    the high-side window — three false OC kills. Now TIM15 triggers the
    scans (EXTSEL 0b100) with its period re-randomised every tick (LFSR,
    37-70 us), scans counted once on EOS. The lattice had also UNDER-read:
    random-phase hold currents agree with firmware50's metered readings at
    32.5-37.5 %; at 15 % the random instrument reads ~+18 % vs firmware50
    (open: low-duty calibration / edge-sample share).
17. **Hold-only current**: one query at hold start resets the per-query
    aggregates; `b` gains `i50max` / `i50over_ms` (peak of the guard's 50 ms
    input and time above 750 mA, firmware-aggregated).
18. **Walk acceleration**: above ~30 % a 1 %/s step's acceleration current
    is several hundred mA; `--walk-step 5` halves it (not needed below 35 %).

### Build finding (open)

**Corrected 2026-10-04 (stage 6 follow-up, item 5): the culprit is
cortex-m 0.7.7, the failing lock had cortex-m-rt 0.7.5; now pinned
`cortex-m >= 0.7.8`.** Original note: the `devel` checkout's gitignored
`rm32_stm32/Cargo.lock` pinned older crates
(e.g. cortex-m-rt 0.7.7) and that G071 image watchdog-looped before the main
loop; the lock from the `g071_binz` worktree (cortex-m-rt 0.7.9 et al.) boots.
The lock is gitignored, so a fresh checkout resolves whatever is current —
bisect the hanging crate before relying on it.

## Stage 4 — envelope climb, PSU clamp 2.5 A (2026-10-03, `devel`)

Image `5bad7b95` (stage-3 final + OC kill 2.2 A). Same protocol: applied-duty
rungs, 3 quiet 15 s holds, 0.5 % walk steps (`WALK=5 scripts/binz_rung.sh`),
one query at hold start (hold-only `imean`, `i50max`), sag / 9 V / nFAULT
kills unchanged; `binz_rung.sh` now reads the latched counters after a kill.

**Result: 57.5 % clean 3/3 (hold 1.83-1.90 A). 60.0 %: all three holds
completed at 2.02-2.03 A — the ~2.0 A stop current — with one SAG kill
(bus 10.32 V vs 11.87 V rest, -13 %, dsy=0) during the end-of-hold query,
i.e. a surge folding the 2.5 A supply (UART TX at that instant is the known
comparator-coupling trigger). Climb stopped there.**

| rung | duty | eHz (3 runs) | hold mA (3 runs) | i50 peak | carrier ARR | dsy |
|---|---|---|---|---|---|---|
| 35.0 % | 700 | 1564 1564 1564 | 750 757 779 | 1061 | 1332 | 0 |
| 37.5 % | 748 | 1633 1650 1666 | 880 865 863 | 1216 | 1325-1332 | 0 |
| 40.0 % | 798 | 1718 1718 1727 | 971 949 946 | 1214 | 1261-1287 | 0 |
| 42.5 % | 850 | 1811 1792 1792 | 1059 1021 1057 | 1264 | 1210-1255 | 0 |
| 45.0 % | 898 | 1862 1851 1872 | 1177 1177 1163 | 1365 | 1159-1210 | 0 |
| 47.5 % | 949 | 1949 1937 1949 | 1244 1308 1328 | 1532 | 1120-1171 | 0 |
| 50.0 % | 999 | 2020 2020 2044 | 1431 1399 1430 | 1635 | 1101-1120 | 0 |
| 52.5 % | 1049 | 2109 2096 2109 | 1553 1574 1556 | 1896 | 1031-1069 | 0 |
| 55.0 % | 1099 | 2178 2164 2192 | 1708 1654 1700 | 1881 | 992-1011 | 0 |
| 57.5 % | 1149 | 2283 2283 2267 | 1832 1899 1888 | 2162 | 960-1005 | 0 |
| 60.0 % | 1198 | 2380 2331 2380 | 2020 2032 2033 | 2159 | 947-979 | 0 |

57.5 % also had an earlier attempt: 2 passes + 1 OC kill 0.18 s into the
hold (2242 mA, after the final walk step: acceleration surge + ripple).

Same-supply firmware50 A/B (`DFD4B1DD.env99-cap987`, climb shell, `l`):

| rung | firmware50 eHz | rm32 eHz (duty-scaled ref) | delta | firmware50 hold | rm32 hold | delta |
|---|---|---|---|---|---|---|
| 37.5 % | 1572 | 1650 (1568) | +5.2 % | 835 mA | 869 mA | +4 % |
| 50.0 % | 1984 | 2028 (1982) | +2.3 % | 1565 mA | 1420 mA | -9 % |

Variable PWM engaged at ci < 200 (~37.5 %): carrier 48 kHz -> ~67 kHz
(ARR 947) at 60 %; the random-phase current instrument kept agreeing with
firmware50 across it (gate 3).

WCET (same COM/COMP code as stage 3): COM + COMP 29.8 us = 37 % of a step
at 50 % (2050 eHz), 41 % at 57.5 %, 42 % at 60 % (2360 eHz); budget < 50 %
holds to ~2800 eHz.

Observation (not a failure): the snapshot `ci` often reads ~1.7x `avg`
(e.g. 300 vs 180) — a long/short sector pattern; averages, zc and dsy stay
clean through 60 %. Worth an interval trace before the next climb.

## Stage 5 — envelope climb, PSU clamp 5 A (2026-10-03, `devel`)

**Result: 77.5 % passes 3/3 at 3.91-3.93 A hold — the ~4.0 A stop current;
climb ended there. No protection fired and no desync on any rung 60-77.5 %.**

Pre-climb changes (all bench/G071; one behaviour-identical core refactor):

19. **Per-sector interval histogram** (`src/sector_hist.rs`, firmware50
    `charz.rs` method): each accept interval against the mean of the
    previous six, per sector, 1 us bins, excursions (>= 1.5x mean), plus a
    SIGNED per-sector mean (`dev_x6`, counts x6). The COMP ISR only pushes
    (interval, step) into a 256-entry ring; the main loop computes (the
    in-ISR version cost +94 instructions / +3.4 us). Reset at the hold-start
    query, dumped only with the bridge stopped.
20. **Split current guard** (`bench_guard.rs`, firmware50's shape): per-ms
    samples feed a slow EWMA (~256 ms) killed at 4.5 A and a fast EWMA
    (~16 ms) killed at 4.8 A (`OCFAST`), replacing one 20 ms window on the
    50 ms moving average. `b` reports `ewf=` / `ews=`.
21. **Fixed 48 kHz carrier** (`bench_fixed_pwm: true` -> AM32
    `variable_pwm = 0`). The 67 kHz seen in stage 4 was AM32's variable PWM
    mapping ARR..ARR/2 at `pwm_frequency = 48` (main.c:2361, :668), not an
    L431-ism.
22. **Atomic G071 commutation writer** (`phase.rs::g071_atomic`): the
    L431-qualified atomic writer as a compile-time table of register images
    per (comp, step); table decoded from the ELF and checked leg by leg (one
    AF leg, one low leg, one floating, no shoot-through state).
23. **`commutation_timer_expired` as exclusive mode branches** (rm32 core,
    behaviour identical, host tests 364/364): the former independent `if`s
    re-read `old_routine` three times and let a static path pay for both
    `set_old_routine` and `transition(BemfLocked)`, which cannot both run.

WCET gate (`binz_wcet.py`, final image `b2e81dea`; COM/COMP SharedState CAS
bounded at 1 for the priority-0 roots — nothing can preempt them between
load and CAS — TIM6 keeps 2):

| root | insns | AM32 | ratio | cyc @2 WS | us |
|---|---|---|---|---|---|
| TIM14 (COM) | 1036 | 674 | 1.54 | 1068 | 16.7 |
| ADC_COMP | 189 | 131 | 1.44 | 522 | 8.2 |
| TIM6 | 2604 | 1637 | 1.59 | 5162 | 80.7 (static bound; open review item) |
| DMA1_CH1 | 455 | — | — | 1409 | 22.0 |

COM + COMP = 24.8 us (was 31.3 before the trims): 45 % of a step at
77.5 % (3000 eHz); the < 50 % budget now holds to ~3360 eHz.

| rung | duty | eHz (3 runs) | hold mA (3 runs) | excursions | sector dev_x6 (2 / 3) |
|---|---|---|---|---|---|
| 60.0 % | 1198 | 2415 2398 2398 | 2323 2201 2188 | 4 | -20 / +24 |
| 62.5 % | 1249 | 2506 2469 2487 | 2388 2380 2413 | 3-5 | -20 / +27 |
| 65.0 % | 1300 | 2583 2544 2564 | 2505 2606 2585 | 2-3 | -18 / +27 |
| 67.5 % | 1348 | 2645 2604 2732 | 2861 2863 2876 | 4-6 | -35 / +46 |
| 70.0 % | 1398 | 2777 2824 2710 | 3145 3129 3137 | 74-78 | -63 / +71 |
| 72.5 % | 1450 | 2754 2849 2873 | 3389 3274 3402 | 8-13 | -63 / +70 |
| 75.0 % | 1498 | 2898 2923 2898 | 3649 3655 3648 | 33-47 | -70 / +77 |
| 77.5 % | 1549 | 2949 3003 2976 | 3909 3932 3916 | ~1510 | -52 / +59 |

Rows 60-70 % (first set) ran on image `7723e703` (64-entry ring, 15-25 %
drops); 70 % was re-qualified 3/3 on `b2e81dea` (256 ring, ~1 % drops at
70 %, ~25 % at 77.5 %) before climbing on. Carrier ARR 1332 (48 kHz) on
every rung. Supply path ~0.14 ohm (rest ~11.78 V, hold ~11.31 V at 3.9 A).

**Sector pattern** (firmware50 agent's prediction confirmed): a static
long/short split, not jitter — at 30 % odd sectors long / even short
(+36 -55 +64 -52 +32 -25); from 60 % sectors 2 and 4 short, 3 long, 1/5/6
near zero, growing with speed to -70 / +77 (~6 us on a 57 us step) at 75 %.
Excursions jump to ~1500 per hold at 77.5 % (still dsy = 0) — the onset to
watch above this current.

Same-supply firmware50 A/B (`DFD4B1DD.env99-cap987`, climb shell, `l`):

| rung | firmware50 eHz | rm32 eHz (duty-scaled ref) | eHz delta | firmware50 hold | rm32 hold |
|---|---|---|---|---|---|
| 62.5 % | 2314 | 2487 (2312) | +7.6 % | 2456 mA | 2394 mA (-2.5 %) |
| 75.0 % | 2645 (x2, repeatable) | 2906 (2641) | +9.9 % raw / +10.0 % scaled — on the bar | 3609-3619 mA | 3651 mA (+0.9 %) |

Both apply the same compare (firmware50 CCR 999, rm32 998 of 1332) and
advance 16. Where firmware50 prints its AM32 oracle, rm32 tracks AM32 and
firmware50 runs slow: 25 % 1186 / 1149 (-3.1 %) / 1118 (-5.7 %); 37.5 %
1650 / 1650 (0 %) / 1572 (-4.7 %); 50 % 2096 / 2028 (-3.2 %) / 1984
(-5.3 %) (oracle / rm32 / firmware50). The growing gap is mostly
firmware50's own deficit (its `speed_within_5pct` fails); rm32 was not
tuned toward firmware50 against AM32 parity.

### Stage 5 addendum — precise speed comparison (supersedes the A/B eHz row above)

The A/B table above compared rm32's snapshot `ehz` (from the integer
single-interval `avg`, ~0.9 % per count) against firmware50's
`ehz_from_sector` (from an integer-truncated mean sector time: 63.99 us ->
63, reading HIGH). Both are too coarse for a 10 % bar. Like-for-like hold
means:

* rm32: new `b` field `ecom10` = mean e_com_time over the hold in 0.1 us
  (sampled every 20 ticks while running; eHz = 1e7 / ecom10).
* firmware50: `BEMFRATE` hold_accepted / 6 / hold_ms.

| rung | duty (rm32 / fw50) | rm32 hold-mean eHz | firmware50 hold eHz | delta |
|---|---|---|---|---|
| 62.5 % | 1249 / 1250 | 2486 (ecom10 4022) | 2287 | **+8.7 %** |
| 75.0 % | 1500 / 1500 (exact; `--rung 751`) | 2899 (ecom10 3450 3449 3449, 3 runs) | 2604 (x2, 2604.5 / 2604.3) | **+11.3 %** |

**The 75 % A/B misses the 10 % bar on precise numbers.** Checked and ruled
out as rm32 config divergences: same applied compare (CCR 998-999 / 1332),
same carrier (48 kHz fixed both), same advance (`advance_level` 26 ->
temp_advance 16, AM32 `advance_level - 10`; firmware50 advance 16),
`auto_advance` = 0 (config byte 47). Current agrees within 1 % at both rungs.
Where firmware50 prints its AM32 oracle, firmware50 itself runs ~5 % below
AM32 (its `speed_within_5pct` fails at 25 / 37.5 / 50 %) while rm32 snapshots
sat within 0-3 % of it; those oracle comparisons used the coarse quantities
and should be redone with hold means before drawing a parity conclusion.
Open: whether rm32 or firmware50 is off AM32 at 75 % (no AM32 oracle above
50 % on this bench) — e.g. zero-cross detection latency / effective advance.

## Stage 6 — expand to the supply limit, then qualify inside it (PSU clamp 5 A, 2026-10-03, `devel`)

Protocol as stage 5: 2.5 % applied-duty rungs, 0.5 % walk, 3 of 3 quiet
15 s holds read from the captures, all guards on (slow EWMA 4.5 A, fast
EWMA 4.8 A, 10 % sag, 9 V floor, 14 V OVOLT, nFAULT), `w` on every exit.
New in this stage: **nothing on the UART while the bridge drives** (see the
watchdog finding). A silent `H` resets the aggregates at hold start, and the
first query after the stop prints the `r` hold snapshot.

**Final image: `167b7ae9`** (sha256 prefix of the ELF). WCET gate on it:

| root | insns | AM32 | ratio | us @ 2 ws |
|---|---|---|---|---|
| TIM14 (COM) | 1049 | 674 | 1.56 | 18.0 |
| ADC_COMP | 214 | 131 | 1.63 | 10.8 |
| TIM6 | 2677 | 1637 | 1.64 | 83.6 (static, all paths) |

COM + COMP (filter 3) = 24.6 us = 45 % of a step at 3040 eHz. PASS.

### Envelope: top rung 80 % on the final image (supply limit at 82.5 %)

| image | rung | holds (imean mA) | hold-mean eHz | dsy | outcome |
|---|---|---|---|---|---|
| `c791788c` | 80.0 % | 3/3, ~4190 | ~3038 | 0 | pass |
| `c791788c` | 82.5 % | 4450 4428 4467 | ~3117 | 0 | pass |
| `c791788c` | 85.0 % | — | — | 0 | OCFAST at the 84.5 % walk step, twice, locked: **supply** |
| **`167b7ae9`** | **80.0 %** | **4159 4178 4163** | **3041** (ecom10 3287-3289) | **0** | **pass 3/3, 0 watchdog resets** |
| `167b7ae9` | 82.5 % | — | — | 0 | OC (slow EWMA) at 4595 mA, 5.8 s into the hold, locked: **supply** |

The 4.5 / 4.8 A kills exist to keep the bridge under the 5 A PSU clamp, so
a locked-rotor current kill is "the supply becomes the limit". Climbing
stopped there and no limit was changed.

**Why the boundary moved down a rung.** The 82.5 % pass on `c791788c` ran
with a persistence filter that turned out to be 1.46x AM32's length. That
filter accepted zero-crosses later, which cost about 4 % less current at the
same speed. At measured AM32 parity (below), 82.5 % draws 4.6-4.7 A at an
unchanged ~3115 eHz: OCFAST at 4673 mA on `f7fee9c0`, OC at 4595 mA on
`167b7ae9`. 75 % at parity: 3606 mA / 2889 eHz, against 3650 mA / 2899 eHz
before.

### Finding: flash prefetch was off (AM32 parity defect, fixed)

AM32 `Mcu/g071/Src/peripherals.c:29` sets `FLASH->ACR |= FLASH_ACR_PRFTEN`.
rm32 never did (ACR read `0x00040602`), so at 64 MHz / 2 wait states every
line fetch paid the full latency. `mcu_g071/init.rs` now sets PRFTEN with a
read-modify-write; a wholesale write would clear DBG_SWEN and kill SWD.
ACR now reads `0x00040702`.

Measured with the new TIM6 instrument (wall time from TIM6's own update, in
CPU cycles):

| condition | prefetch off | prefetch on |
|---|---|---|
| armed idle, TIM6 mean | 1610 cyc | 1320 cyc |
| 30 % hold, TIM6 mean | 2694 cyc | 2199 cyc |
| 30 % hold, TIM6 overruns / 15 s | 141 332 | 6 246 |
| 30 % hold, max main-loop gap | 392 ticks | 71 ticks |

### Finding: sector bias explained; filter set to measured AM32 parity

`mcu_g071/filter_cal.rs` is a bench-only boot calibration (`[bench]
filter-read cyc` line). It times one persistence-filter read on this chip
with interrupts off: AM32's loop copied instruction for instruction from
`AM32_DRV8304H_G071_2.20.elf` (`interruptRoutine` -> `getCompOutputLevel`,
same mod-8 flash alignment) against rm32's loop with a variable NOP pad.
Cycles per read:

| ACR | AM32 | rm32 k=0 | k=12 | k=14 | k=16 |
|---|---|---|---|---|---|
| prefetch on | **30.6** | 12.6 | 28.6 | ~30.6 | 32.6 |
| prefetch off | **30.6** | 12.5 | 38.6 | — | 44.6 |

* AM32's read costs the same with prefetch on or off, because its loop is
  three taken branches plus a flash literal load per read.
* The stage-5 fix (16 NOPs) had been sized from a static estimate with
  prefetch off, where a NOP costs 1.5 cycles. That gave 44.6 cycles, 1.46x
  AM32, not parity.
* Under AM32's own ACR (prefetch on), parity is **14 NOPs**. That is what
  `comparator.rs::filter_read` now uses, documented with these numbers.
* `filter_level` itself was already AM32's: `map(average_interval, 100,
  500, 3, 12)`, 2 below 50, with the same 0.5 us interval units.
* rm32's path from the edge to its first read is longer than AM32's (gate
  computation, `handle_comp` call), so rm32's total window is at least
  AM32's.

Sector-2/3 overflow (|deviation| >= 15 us) at 75 %, by filter state:
unpadded 18.8 % -> k16 / prefetch off 1.8 % -> k16 / prefetch on 5.1 % ->
**k14 / prefetch on 4.8 %**. The bias shrinks monotonically as the window
grows.

**Mechanism, from raw interval traces** (`he` lines in the stopped dump,
the 16 accepts around each of the first four excursions, in step:iv form,
0.5 us counts). At 80 % every excursion has the same shape:
`... 1:103 2:63 3:169 4:97 ...` around a mean of ~110. Sector 2 (phase A
falling) is accepted about 23 us early, ~3-4 us after the half-interval
gate (`CNT > average_interval/2`, ~55-58) opens. Sector 3 then runs long by
the same amount.

* It is not a demag camp: camp re-entries total ~3700 per hold, about one
  per event.
* It is the first phase-A transient after the gate opens that outlasts an
  AM32-length filter. The trigger is a phase-A-specific hardware asymmetry
  (sectors 4 and 6 never show it) whose cause was not isolated.
* An AM32 filter on this board accepts the same transients. The AM32-shaped
  fix is the per-read parity above; nothing beyond it was added.

**The "excursion jump at 77.5 %" is a metric threshold, not a new
mechanism.** An excursion is iv >= 1.5x the 6-step mean. The same gate-edge
accepts exist at 75 % (4.8 % overflow), but their long partner sits at about
1.43-1.49x the mean there. The step shortens with speed, so by 77.5-80 %
the partner crosses 1.5x (164-169 / ~110). Excursions go from 0 at 75 % to
~3550 per 15 s hold at 80 % on the final image, with dsy = 0 throughout.

### Finding: IWDG reset at 82.5 %, then a relaunch hazard (both fixed)

1. **UART TX during drive starves main.** At 82.5 % the first padded-filter
   image reset by IWDG. The bisect showed it was the bench `i` / `b`
   queries: UART TX couples into the comparator (a known bench scar), the
   comparator storm consumes the CPU, and the polled TX starves the main
   loop until the watchdog fires. **Fix (protocol):** no UART traffic while
   driving. Silent `H` at hold start; `r` snapshot (duty, imean, i50max,
   ecom10, slow EWMA, dsy, TIM6 max / overruns / mean / checkpoints, main-loop
   gap, COMP entries / camps) printed after the stop.
2. **After that reset the board re-armed under the still-streaming 82 %
   command and launched from standstill.** It skipped the sine stage and the
   walk, and drew 1.8 A into a SAG kill. Cause: the internal throttle holds
   (no shunt zero yet, latched guard) feed 0 to the input, and AM32's
   arming logic accepts that as "throttle at zero". **Fix:**
   `bench_seen_zero` in `bin/main.rs` disarms until a genuine `0` / `w` is
   received after boot. Verified: 5 s of streaming `820` after a reset left
   `armed=0 en=0 moe=0`. Also `binz_spin.py` now aborts (and kills) on any
   boot banner (`last reset:` / `entering main loop`) in the stream.

### TIM6 WCET, path by path

Static (`isr_cycles.py --terminal --zero-callee`, image `a73ab3b7`, 2 ws,
no prefetch credit): all paths 80.7 us, locked running 50.3 us, polling
67.2 us. Final image, all paths: 83.6 us. These are upper bounds that
cannot exclude mutually exclusive inlined branches, so TIM6 was measured
directly.

**Measured** (bench instrument in `isr_handlers.rs`: TIM6 CNT at exit =
cycles since its update, including entry latency and preemption; UIF set
at exit = overrun; four section checkpoints; all reset by `H`). Final
image, 80 % hold, 3 runs:

| quantity | value |
|---|---|
| TIM6 mean wall time | 2506-2507 cyc (39.2 us of 50) |
| TIM6 max | censored at 3194 (one period): 11.4-11.6 k overruns / 15 s = 3.8 % of ticks |
| entry latency (CNT at entry), max | 1274-1344 cyc (prio-0 COMP / COM and prio-1 DMA ahead of it) |
| `ten_khz_tick` section, max | 3199 (saturates; preempted inside) |
| tone block, max | ~1385 cyc |
| tail (gate latch + tick count), max | ~1236 cyc (preempted) |
| COMP ISR entries | ~45.6 k/s, carrier-rate chatter (48 kHz); camps ~250/s |
| max main-loop gap | 135-149 ticks (~7 ms; IWDG at 2 s) |

Armed idle: mean 1320 cyc, max ~1611, 0-1 overruns. The 80 % wall time is
TIM6's own ~1300-1400 cycles stretched by priority-0 load: comparator
chatter at the carrier rate plus COM, roughly half the CPU at 3 keHz.
firmware50 measured the same chatter class (64 entries/ms peak). Overruns
are serviced one tick late by tail-chaining. Because the max is censored at
one period, whether a tick is ever lost outright (> 1 period late) is not
measured.

### Events on the final image `167b7ae9`

| event | 25 % | 50 % | 75 % | 80 % (top) |
|---|---|---|---|---|
| restart (300 ms coast, re-walk onto the coasting rotor) | pass: re-hold 1161 / 1160 eHz, locked | pass: 2070 eHz | pass: 2889 eHz | pass: 3039 eHz |
| desyncs counted during the restart (cumulative) | 32, 13 | 16 | 4 | 2 |
| desync injection (`K`: next commutation skips a step) | pass: absorbed in-window (1 excursion, dsy 0), re-hold 1160 eHz (0.0 %) | — | pass: absorbed, re-hold 2889 (0.0 %) | pass: absorbed, re-hold 3039 (0.1 %) |

`--probe-phases` (25 / 50 %, where queries don't disturb) puts every restart
desync in the re-walk: 0 in the pre-hold, 0 in the coast. Then run=1,
old=0. This is AM32-shaped behaviour. AM32's zero-input stop
(`main.c` ~1290-1331) sets `running = 0`, `old_routine = 1`,
`zero_crosses = 0`, and with `use_sine_start` re-enters `stepper_sine`. A
re-throttle restarts the sine stepper from home and changes over at
`commutation_interval = 9000` into polling, whatever the coasting rotor's
speed. The desyncs are that mismatch being recovered. Counts vary run to
run: earlier images 3 / 17 / 2 / 1 at 25 / 50 / 75 / 75 %. The injected
skip never needs the desync detector: the loop re-locks within the
2 s window (in-window dsy 0). The inject runs' "desyncs counted" (0 / 41 / 7)
come from their own stop-and-re-walk.

Earlier images in this stage, not re-run on the final image because
neither control path changed in a way that could alter them:

* Disturb 25 / 50 % (UART spam during the hold): 0 desyncs, re-hold within
  0.1 % (`c791788c`).
* Diode-decel step 50 -> 15 % and 75 -> 15 %: 0 desyncs, re-hold 679 eHz.
* Step up 15 -> 50 %: pass.
* Supply-limited, by design of the PSU:
  * damped step down 50 -> 15 % regenerates into OVOLT at 16.98 V (ramped:
    16.94 V; the PSU cannot sink);
  * step up 25 -> 75 % folds the PSU into a SAG kill at 9.97 V, locked.

### Tooling and build changes in this stage

* `binz_wcet.py`: per-root SharedState CAS bound for the priority-0 roots,
  per-rung COM + COMP with AM32's filter_level; `isr_cycles.py --terminal /
  --list-terminal / --zero-callee`.
* `sector_hist.rs`: gate-edge accept counts (`hg`) and raw excursion traces
  (`he`), main-loop only.
* `filter_cal.rs`: per-read filter calibration at boot (bench only).
* TIM6 / COMP instruments: `isr_handlers.rs`, `mcu_g071/interrupts.rs`.
* `binz_events.py`: `restart`, `disturb`, `step` (`--down-pm`,
  `--diode-decel`), `inject` (`K`, `--ref-ehz`), `--probe-phases`.
* `binz_spin.py`: SIGTERM / SIGBREAK -> KeyboardInterrupt (the kill path
  runs). An external `timeout` once killed a script mid-drive and left the
  bridge on. Also aborts on a boot banner. Climb cap `MAX_RUNG_TENTHS = 825`.
* Flash: the binz app region is 62 K. `{:?}` on `InitError` pulled in
  core's Unicode / Debug machinery. `InitError::parts()` instead takes .text
  from 61.6 to 54.9 KB. The final image is 59.3 KB with all bench
  instruments.
* Clippy-clean on `stm32g071,benchuart` except one pre-existing L431
  `debuguart` line. L431 / L431+benchuart / G431 / F051 cross-builds and the
  365 host tests pass.

### Open

* The phase-A asymmetry behind the sector-2 gate-edge accepts is not
  isolated.
* rm32 vs firmware50 at 75 % (addendum above) is still without an AM32
  oracle above 50 %.
* The devel `Cargo.lock` boot-loop: bisected to cortex-m 0.7.7 and pinned out (follow-up item 5).

### Stage 6 follow-up — host-side work on the firmware50 review (2026-10-04, no spinning)

Candidate image after this work: **`2cac8043`**. It is NOT yet
bench-qualified: the qualified image remains `167b7ae9` until the next bench
session reruns 80 % 3/3 and the event set on `2cac8043`. WCET gate on it:
TIM14 1.56x, ADC_COMP 1.63x, TIM6 2707 insns = 1.65x AM32; COM + COMP =
45 % of a step at 3040 eHz.

**1. Physical-phase mapping of the sector asymmetry**
(`scripts/sector_phase_compare.py`, existing captures only). Both firmwares
use the same drive table and floating sequence and tag an interval with the
step current at its accept, so a step names the same crossing in both:
1 C+, 2 A-, 3 B+, 4 C-, 5 A+, 6 B- (+ rising, - falling).

| crossing | rm32 80 % (`167b7ae9`) P(>= 15 us) | firmware50 80 % (`map-800`) | firmware50 90 % |
|---|---|---|---|
| A- (2) | **9.6 %**, signed -3.2 us | **5.8 %** | 17.5 %, -6.3 us |
| B+ (3), partner of 2 | 9.6 %, +3.5 us | 5.9 % | 17.4 %, +7.8 us |
| C- (4) | 0.0 % | **6.5 %** | 12.6 %, -3.1 us |
| A+ (5), partner of 4 | 0.0 % | 6.0 % | 12.4 %, +1.6 us |
| C+ (1), B- (6) | 0 | ~0 | 0 |

The phase-A falling early accept appears in both firmwares with the same
sign (short, then a long partner): it is the board / motor, not rm32.
firmware50 additionally accepts C- early, which rm32 does not: that half
is firmware-specific. B- is clean in both. firmware50's "sectors 2-5 wide,
1/6 narrow" is exactly the two early falling crossings plus their partners.

**2. Control tick: a TIM6 tick-gap instrument, and what it found**
(`isr_handlers.rs`: entry-to-entry gap on a free-running SysTick, max gap,
count over 1.5 periods, count over 2 periods = a tick lost outright; in the
`b` and `r` lines as `gapmax= gaplate= gaplost=`; reset by `H`; modelled
on firmware50's `gap_max_us`).

At idle, motor stopped, before any fix: **gapmax 20 822 cycles (325 us,
6.5 periods), 3 lost ticks per 10 s**. Root cause: `dprintln!` called
`rtt_target::rprintln!`, and rtt-target 0.5.0 runs the whole `core::fmt`
formatting inside `critical_section::with`, i.e. with every interrupt
masked, priority-0 COMP / commutation included (RTT is initialised at boot
in every build). One ~330-character `[loop]` heartbeat held the mask for
~317 us.

**Fix (`lib.rs`):** `dprintln!` now makes one formatting pass outside any
critical section (`dline`). Each formatted fragment goes to RTT under its
own short lock (a buffer copy) and to the debug UART. Idle after the fix:
gapmax 3437 cycles (1.07 periods), 0 late, 0 lost, 0 overruns. Side effect:
each call site formats once instead of twice (RTT + UART), so .text drops by
~4.9 KB on G071. This affects every target, L431 included.

**Reinterpretation, not retested:** every `i` / `b` reply while driving
masked COMP and COM for hundreds of microseconds. That confounds the stage-6
82.5 % watchdog finding ("UART TX couples into the comparator"), the
stage-5 / 6 disturb test (query spam during a hold), and any desync counted
while queries ran. The no-UART-while-driving protocol stays; the 80 % holds
were quiet and are unaffected.

**3. Coast-down rotor speed — built, idle-validated, not yet run**
(`mcu_g071/coast.rs`, `C` key, `scripts/binz_coast.py`). `C` at the end of
a hold:
* zeroes the throttle and requests `AllOff`;
* in one critical section of at most 60 ms: masks the commutation IRQ
  (`AllOff` does not stop an already-armed commutation, which would
  re-drive the bridge), forces the phases off, masks COMP's EXTI line,
  points COMP at phase A;
* times up to 32 debounced half-periods on the SysTick.

TIM14 stays masked until `running == false`. The host compares
1e6 / (2 x median of half-periods 2..9) with the hold's loop eHz. Idle
check on a stationary rotor: one crossing ~32 us after the cut (comparator
settling after the input switch, hence never used), then none; TIM14
re-enabled afterwards (NVIC ISER bit 19 = 1). For reference, firmware50's
own coast at the same duties (`COASTTIMING` in `captures/char/map-*`): loop
above coast by 2.7 % at 80 % (2732 / 2659), 3.5 % at 85 %, 2.4 % at 90 %.

**4. Down-steps on the PSU.** `binz_events.py step` now refuses a
down-step unless `--diode-decel` or `--battery` is given. Both the hard step
and the `--down-pm` ramp pumped the bus to ~17 V, past the 14 V OVOLT stop:
cutting the bridge doesn't stop regeneration, because the body diodes keep
rectifying the BEMF into a supply that cannot sink.

**5. Cargo.lock boot loop — bisected; corrects the stage-3 note.** The
failing lock was **cortex-m 0.7.7 + cortex-m-rt 0.7.5**, not
"cortex-m-rt 0.7.7". The booting lock has cortex-m 0.7.9 + cortex-m-rt
0.7.7, identical to the `g071_binz` worktree's.
* Bisected on the current source by changing one crate in the booting
  lock: cortex-m 0.7.7 alone LOOPS; 0.7.8 and 0.7.9 BOOT.
* Symptom: a reset loop with a ~1.45 s period. Each boot prints up to
  `[rm32] benchuart: ...`, then the next boot reports
  `last reset: indep-watchdog`. Death is at `cortex_m::interrupt::enable()`.
* With SWD halted inside the window, ICSR = `0x00421021`: active vector
  TIM6, TIM6 pending again. The control tick runs back-to-back and the main
  loop never resumes.
* Relevant 0.7.8 change: intrinsics moved from calls into a prebuilt asm
  archive to inline asm, and `asm::delay(n)` went from n/2 to n+1 loop
  iterations. The exact mechanism inside 0.7.7 was not isolated.
* Repro: build with `cargo update -p cortex-m --precise 0.7.7`, then
  `python scripts/binz_bootwatch.py` (byte-safe; `binz_console.py`
  crashes on the pre-boot garbage bytes).
* Guard: `Cargo.toml` now requires `cortex-m = "0.7.8"` (verified: cargo
  refuses `--precise 0.7.7`). The lock stays gitignored, but a fresh
  resolve can no longer pick the looping version.

## Stage 7 — 100 % on the 3S battery (2026-10-06, `devel`)

**Result: 100 % duty qualified on final image `765fcf9a`,** with its
coast, restart, inject and down-step events all on that image.

Limits: firmware50's ENV-98 battery spec, `bench_guard::BINZ_BATTERY` =
8000 mA slow-EWMA allowance, 12 000 mA fast surge ceiling (1.5x, ENV-97),
9 V floor. 10 % relative sag, 14 V OVOLT and nFAULT as before. The relative
sag guard's REFERENCE was corrected (finding 3); its threshold did not change.
Protocol: one quiet 15 s hold per climb rung (0.5 % walk), 3/3 only at
100 %, no UART while driving, `w` on every exit.

### Climb

| rung | image | imean mA | loop eHz | dsy | note |
|---|---|---|---|---|---|
| 80.0 % | `9403f06b` | 4418 | 3086 | 0 | coast: rotor **3066** eHz, loop +1.1 % |
| 82.5 % | `9403f06b` | 4599 | 3156 | 0 | |
| 85.0 % | `9403f06b` | 4932 | 3226 | 0 | |
| 87.5 % | `9403f06b` | 5288 | 3298 | 0 | COM gate next at ~49.7 %: cut (finding 2) |
| 87.5 % | `7375b4c3` | 5255 | 3297 | 0 | re-hold after the COM cut |
| 90.0 % | `7375b4c3` | 5646 | 3375 | 0 | |
| 92.5 % | `7375b4c3` | 6165 | 3463 | 0 | |
| 95.0 % | `7375b4c3` | 6481 | 3515 | 0 | |
| 97.5 % | `7375b4c3` | — | — | 0 | SAG kill: 10.91 V vs 12.13 V rest at 6.87 A, locked (finding 3) |
| 97.5 % | `765fcf9a` | 6765 | 3571 | 0 | |
| **100 %** | **`765fcf9a`** | **7284 7231 7253** | **3638 3634 3627** | **0** | **3/3**, 0 late / 0 lost ticks, pack rest ~12.0 V |

### 100 % events on `765fcf9a` (all pass)

| event | result |
|---|---|
| coast-down speed | rotor **3579** eHz, loop 3621 (+1.2 %); half-periods 139.3 -> 143.1 us |
| restart (300 ms coast, re-walk) x3 | re-hold 3609 / 3597 / 3593 eHz, locked; 2 desyncs during each restart |
| inject (`K`) x3 | absorbed in-window each time (1 excursion, dsy 0); re-hold 3580 / 3574 / 3566 (0.5-0.9 % from 3600) |
| hard down-step 100 -> 25 % (damped, `--battery`) | 0 desyncs, locked 2 s after, quiet re-hold 1141 eHz; no OVOLT |

Against firmware50 on the same pack (ENV-97/98, its `map-1000` coast):

| | rotor speed (coast) | current |
|---|---|---|
| firmware50 | 3311 eHz | 7.2-7.35 A (slow tracker 7.9-8.06 A; folded once at 8 A) |
| rm32 | 3579 eHz (+8 %) | 7.23-7.28 A (slow EWMA max 7.27 A) |

### Findings

1. **The speed advantage is real rotor speed.** The coast check (`C`) puts
   rm32's loop only 1.1-1.2 % above the coasting rotor, below firmware50's
   own loop-over-coast at the same duties (2.7 % at 80 %). That answers the
   firmware50 review: 3041 / 3086 eHz at 80 % was not an estimator artifact.

2. **COM ISR cut to 0.81x AM32 (gate 18.0 -> 14.2 us).** The 50 % gate
   would bind near 3390 eHz (90 %). Changes:
   * On a board without an enable-style bridge, the G071 `com_step` is now
     only the compile-time atomic writer. build.rs emits
     `rm32_bridge_enable`, and the sequential writer is compiled in only for
     such a board. Cost: 542 static instructions, and 128 cycles on the
     longest path.
   * `Commutation::advance` and `record_interval` are inlined.
     `record_interval` keeps a running sum instead of re-summing six slots.
     Its index is masked onto an 8-slot array, removing a panic path.
   * The priority-0-only shared-state writes now use `_isr0` variants (plain
     load + store, as AM32 assigns its globals): `set_old_routine_isr0`,
     `transition_isr0` and `increment_zero_crosses_isr0`. The contract is on
     the `MotorState` / `IsrTiming` traits: highest priority only. On M0
     every lower-priority compare-and-swap runs with interrupts masked; on M4
     exception entry clears the exclusive monitor.
   * Result: TIM14 1049 -> 549 reachable instructions (AM32 674); longest
     path 1150 -> 909 cycles @ 2 ws. COM + COMP = 46 % of a step at
     3700 eHz.
   * Side effects at 87.5 %: TIM6 overruns halved (15.4 k -> 7.4 k per hold)
     and the sector-2/3 bias fell from -29/+32 to -8/+10 (6x dev units),
     because the commutation fires sooner after its scheduled time.

3. **The relative-sag guard was anchored against the operator's spec.**
   rm32 judged the bus against the PRE-RUN REST peak. The operator's binz
   rule says to anchor the 10 % cap to the synced operating point, not
   no-load, because its purpose is the desync current surge. On the battery,
   a locked, healthy 97.5 % hold reached exactly 10 % of rest from the
   pack-plus-leads IR drop alone (~0.18 ohm at 6.87 A) and was killed.
   * Fix: the reference is a ~256 ms average of the bus, updated once per
     ms after each test, and seeded at rest while stopped. This is what
     firmware50 E146 did with ~207 ms.
   * A collapse within milliseconds is still judged against the pre-collapse
     level. Slow droop is the 9 V floor's job. The 10 % threshold and 5 ms
     debounce are unchanged.
   * The kill line now prints `ref_mv=`.

4. **At 100 % duty the comparator chatter disappears.** COMP entries fall
   from ~600 k to ~330 k per 15 s hold and TIM6 overruns from thousands to
   0, since there are no PWM edges at full duty. That confirms the chatter
   is the PWM carrier coupling into the comparator.

5. **Open, unchanged by this stage:** two blackbox vectors,
   `sine_brake_on_stop` and `sine_changeover`, fail (79/81). Verified: they
   fail identically with this stage's COM changes reverted, so they come from
   earlier unstaged work and need a separate look.

### Final image WCET (`765fcf9a`)

| root | insns | AM32 | ratio | us @ 2 ws |
|---|---|---|---|---|
| TIM14 (COM) | 549 | 674 | 0.81 | 14.2 |
| ADC_COMP | 215 | 131 | 1.64 | 10.7 |
| TIM6 | 2275 | 1637 | 1.39 | 77.8 (static) |

Measured TIM6 at 100 %: mean 2295-2298 cycles, max 2810-3033, 0 overruns,
max tick gap 3871-3876 cycles (1.2 periods), 0 late / 0 lost.

## Qualification report (2026-10-06)

**Report: https://claude.ai/artifact/K8n15MFLHaBgu4ZTjx2hXX** (private until
shared). It was generated by `scripts/binz_qual_report.py` from captures in
`captures/binz/` (`profile_*` CSV/JSON from `scripts/binz_profile.py`, plus
the stage-7 holds and events). Report image `4064aef7`: the stage-7 control
code plus a bench time-series recorder (`rm32::bench_rec`, 2048 x 8 B ring,
10 or 50 ms, clocked by SysTick, keys `R` / `Y` / `X`, dumped after the stop).

Runs:
* operating map 5-100 % (5 % steps, 5 s dwells);
* ladder 10 -> 100 -> 10 % (19 rungs);
* steps 10->60->10, 10->90, 10->100, and the 90->10, 100->10, 100->25
  down-steps;
* low end 3/4/5/4/3 % plus lost-tick dwells at 5/10/15 %;
* 3 standard starts and 3 direct starts.

New findings recorded there:
* **10 -> 90 / 100 % slams trip the 10 % sag guard** on acceleration inrush
  (7.9-8.5 A fast average, bus to 10.1 V vs an 11.45 V running reference)
  within 0.3 s, locked. The duty ramp is AM32's G071 default (2 / 6 / 16
  per tick, targets.h:3135-3144). All down-steps and 50 % up-steps hold.
* **Lost control ticks at 5 % throttle:** 9226 per 10 s, worst gap 642 us,
  from comparator camping (the priority-0 COMP handler re-enters while the
  half-interval gate is closed). 48 per 10 s at 10 %, 0 from 15 % up.
* **A direct start into 20 % (no walk-in) failed to lock once in 4.** The
  standard walk-in start locked 3 of 3.
* **5 % throttle hysteresis:** entered from 10 % it runs in interrupt mode
  at ~140 eHz; entered from the sine band it sits in polling.
* The first map run showed the recorder's time axis compressing when
  control ticks were lost. The recorder is now clocked by SysTick and fills
  missed slots.

### Slam test with the sag guard at 15 % (operator, 2026-10-07)

`BINZ_BATTERY.sag_pct` 10 -> 15; every other guard unchanged. Image
`1c7e7935` (report image `4064aef7` plus this constant).

* **10 -> 90 -> 10 %: passes** (`profile_slam15-90_20261007_005104`).
  * Peak 10.0 A (16 ms average) at +50 ms, above 8 A for 70 ms.
  * Bus 13.2 % below the pre-step 11.60 V at +30 ms.
  * 535 -> 3337 eHz settled within +-5 % in 80 ms; 0 desyncs.
* **10 -> 100 -> 10 %: killed by the 12 A surge guard (OCFAST)** at +0.24 s,
  locked (`profile_slam15-100_20261007_005202`).
  * The AM32-default ramp reaches duty 2000 in 30 ms with the rotor at
    2070 eHz.
  * Current goes 3.0 -> 6.8 -> 11.8 A over successive 10 ms samples; bus
    16.0 % down at that point.
  * A full slam to 100 % from 10 % needs more than 12 A and more than ~16 %
    sag on this pack. Under the 1.5x-allowance surge rule it is out of
    envelope; reaching 100 % via a 55 % intermediate step holds (stage 7
    report).
* **Sag at 17.5 % (operator, 2026-10-07; `sag_pct` became `sag_pm` = 175
  to express it).** Image `aa71d91a`. 10 -> 100 % still killed by OCFAST
  (12 A surge) at +0.25 s, bus 9.71 V = 15.5 % below the 11.48 V reference
  (`profile_slam175-100_20261007_005430`). The surge ceiling binds, not the
  sag guard. (An earlier attempt labelled `slam175-100_20261007_005325` ran
  the unchanged 15 % image `1c7e7935` after a failed edit, and gave the same
  kill.)
* **Surge ceiling 12 -> 13 -> 15 A (operator, 2026-10-07), sag 17.5 %.**
  * 13 A (image `4f01bdc1`): OCFAST at +0.24 s; the fast average was
    12.95 A at +40 ms and still rising (`profile_slam13a-100_20261007_010753`).
  * **15 A (image `0cbc3373`): 10 -> 100 -> 10 % passes**
    (`profile_slam15a-100_20261007_010858`).
    * Inrush peak 13.04 A (16 ms average; 50 ms-window peak 13.28 A) at
      +50 ms, above 8 A for 120 ms.
    * Bus minimum 9.71 V at +40 ms, 16.3 % below the pre-step 11.60 V.
    * 535 -> 3625 eHz settled within +-5 % in 90 ms; 0 desyncs; the
      100 -> 10 % step down holds.
  * Margins at this charge state: surge 1.7 A, sag about 1.2 points, 9 V
    floor 0.7 V. The DRV8304H sensing clips near 23 A per phase (70 mV/A
    around VREF/2).
* **Slam from the lowest stable hold, 5 % -> 100 % (sag 17.5 %, surge 15 A,
  image `0cbc3373`): passes** (`profile_slam5-100_20261007_011120`).
  * 5 % entered from 10 % held at 140 eHz in interrupt mode.
  * After the command AM32's staged limits hold duty at the startup cap
    (400) for ~200 ms, with a few polling/interrupt flips, then at the
    low-RPM cap (800) while speed builds. Full duty at +390 ms, 3663 eHz
    by +500 ms.
  * Inrush peak 10.9 A at +420 ms (lower than the 13 A of 10 -> 100),
    bus minimum 10.05 V (13.4 %), 0 desyncs.
* The qualification report gained section 4b with these slams (version 2,
  same link).

## Commit review (2026-10-07)

An independent review of the full change set before the first commit found:

* **Blocker, fixed:** `fast_math::div2000_pwm` `assert!`ed `n <= 16_000_000`
  inside the control ISR. L431 at `pwm_frequency` 8-10 kHz (ARR 8028-10021)
  exceeds that above ~80 % duty. It is now total and exact: products above
  the fast range divide the reduced value by 5 three times with a
  straight-line `div5_u32`. That keeps the M0 link audit green (a plain
  `/ 2000` put `__aeabi_uidiv` on the TIM6 path, and the audit rejected the
  link). Tests cover ARR up to 14164, a dense sweep, and LCG samples.
* **Sine idle braking regression, fixed:** the stepper-sine branch in
  `ten_khz_tick` forced outputs off at idle for every `brake_on_stop` (AM32
  main.c: 1 = proportional brake, 2 = comStep(2), else allOff). Sine idle
  now falls through to the brake chain as at HEAD. The blackbox vector
  `sine_brake_on_stop` caught it.
* `sine_changeover` vector: the stepper reads `adjusted_input`, which the
  input stage publishes after the stepper runs in each pass (firmware and
  harness alike), so the changeover is asserted one pass later. 81/81.
* `.cargo/config.toml`: the M0 link audit (Windows `.cmd` wrapper) applies
  to `thumbv6m` only; L431/G431 link as at HEAD.
* Kept thin LTO + codegen-units 1 (comment in Cargo.toml). Fat LTO inlines
  every ISR callee into its root, and the fail-closed WCET gate then needs
  nine newly reviewed root-loop bounds.
* `rtt-target` pinned `=0.5.0` (`dline` uses its doc-hidden API); the
  `_isr0` contract reworded (L431 priority-swap caveat); misplaced doc
  comments and stale guard numbers fixed; LF line endings restored.

**Final binz image `5565a34c`** (thin LTO, these fixes, battery guards
17.5 % / 15 A).
* WCET: TIM14 549 insns (0.81x AM32), ADC_COMP 1.64x, TIM6 2647 (1.62x);
  COM + COMP = 46 % of a step at 3700 eHz.
* 100 % hold: 6987 mA, 3545 eHz, 0 desyncs, 0 lost ticks.
* 10 -> 100 -> 10 % slam: peak 12.3 A, bus -15.9 %, 3589 eHz in 90 ms,
  0 desyncs (`profile_final-slam-100_20261007_013113`).

**Vimdrones L431:** all L431 configurations build and the 81 blackbox
vectors pass, but the L431 binary changes (shared core, `dprintln!`, thin
LTO, the sine-start path is now live, `bench_seen_zero` arming gate on
benchuart builds). It has NOT been run on the Vimdrones bench. Re-qualify
with `bf_ladder.py` + `bf_slam.py` before relying on this commit there.
