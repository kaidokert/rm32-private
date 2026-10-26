# Generic-core refactor log

One entry per step (docs/refactor_execution_plan.md §7). Bench captures are
under `rm32_stm32/captures/<board>/` and gate artifacts under
`captures/gc/<step>/` (both gitignored); the bands are committed as
`docs/gc_bands_s00.json`.

---

## S0 / S1 — reference baseline (2026-10-25)

**Tag** `gc-s00-baseline` (c235afa). **Class** baseline. **Supply** shared
5 A PSU, ~11.65 V rest, both boards on it, separate motors (L431 prop rig
fastened to a heavy box; binz motor in its box).

**Images** (built by `scripts/gc_gate.sh s00`, `BENCH_SUPPLY=psu`):

| board | sha256 | text / data / bss |
|---|---|---|
| L431 (`stm32l431,benchuart`, neutron_l431, reversed) | `c1bb391cc6062c12cee90493663696fa306af91cbf7be97126cb1bc56a9e471f` | 54476 / 1828 / 36196 |
| binz (`stm32g071,benchuart`, binz_drv8304h_g071, forward) | `d9514648567f08bac816714bc241aa1b37fe0ee59fddb5be1a4d600ad68749c7` | 56676 / 1808 / 20764 |

Public builds (text / data / bss): G071 40636 / 1800 / 1144, F051
37652 / 1792 / 1144, L431 37440 / 1792 / 1144, G431 35876 / 1796 / 1140.

**Host gate:** PASS — 376 host tests, clippy, 81/81 vectors on the Rust
harness, AM32 C harness 71/81 with the 10 failures exactly the
`tests/blackbox/c_known_diffs.txt` list, four public builds + two bench
images linked (M0 audit on the thumbv6m links), ISR dumps written.

**binz WCET** (`binz_wcet.py`, 2-wait-state fetch model):

| root | prio | insns | AM32 | ratio | cyc@2ws | µs |
|---|---|---|---|---|---|---|
| TIM14 (COM) | 0 | 549 | 674 | 0.81 | 909 | 14.2 |
| ADC_COMP | 0 | 215 | 131 | 1.64 | 684 | 10.7 |
| TIM6 (tick) | 2 | 2647 | 1637 | 1.62 | 5114 | 79.9 |
| DMA1_CHANNEL1 | 1 | 455 | – | – | 1409 | 22.0 |

**ISR roots** (isr_diff): L431 COMP, DMA1_CH5, EXTI0, EXTI15_10, LPTIM1,
TIM1_UP_TIM16, TIM6_DACUNDER, USART2; binz ADC_COMP, DMA1_CHANNEL1,
EXTI4_15, TIM14, TIM6_DAC_LPTIM1.

**Operator decisions:** 3 runs per test, interleaved L431/binz; bands =
baseline min..max ±5 %, dsy = 0, zero guard kills, zero reboots. Top rungs
(agent's call, measured margin to the 4.5 A cutoff): L431 60 %, binz 70 %.
Exact profiles are in the plan §3 "As run" table.

**Bench results:** 24 band runs (R1–R4 × 3 rounds × 2 boards): every run
dsy = 0, no guard kill, no reboot; `gc_summary.py --check` 24/24 against
its own bands (and a narrowed-band negative control fails 3/3). Bus peak
across all band runs 12.85 V (14 V trip).

- R5 binz coast at 50 %: loop 2070 eHz vs coast 2050 / 2054 eHz
  (+1.0 % / +0.8 %, firmware50's range ≤37.5 % was +0.6–1.8 %). **2 runs**
  (round 3 stopped by the operator).
- R6 idle registers: 2 snapshots per board. Between snapshots only run
  state differs (binz EXTI edge polarity of the last step, DMA NDTRs;
  L431 timer CNTs, TIM16.ARR) — configuration registers identical. Step
  comparisons exclude those run-state registers. **2 runs** (round 3
  stopped by the operator).
- Round-3 R4 runs on the two boards overlapped by ~15 s on the shared PSU
  (a tool background move, not intended). The binz run took its 3-shunt
  current zero while the L431 drew ~2 A on the shared ground and read
  ~250 mA low on every rung (peer review); it was replaced by a solo
  re-run (152204), widening nothing. The L431 overlap run is kept (its
  current sense has no zero capture; values match rounds 1-2).
- `old_routine` samples on starts: L431 6–9, binz 19–25 (polling during
  start-up; logged, not banded).

**Not banded / deferred:** full 60 → 10 % step-down and abrupt stops
(regenerative pumping of the PSU: 17–19.9 V) → battery batch.

**Found while building the baseline** (each committed or logged):
`dir_reversed` never applied on benchuart builds (ba775b6; every earlier
binz qualification ran forward — binz kept forward, 03ce39f); cargo reads
`.cargo/config.toml` from the cwd, so root-level builds (incl. the
pre-commit cross-build hooks) link empty ELFs without the M0 audit (gate
builds from rm32_stm32 and checks text; hooks still open); the AM32 C
harness hung on unknown commands (reply timeout added); CH340 at 2 Mbaud
corrupts dump values (per-row majority vote over whole re-reads); current
unit split (AM32 boards centiamps, bench boards mA; `current_limit * 200`,
EDT `/1000` vs AM32 `/50`) — open bug, out of this no-control-change
increment.

### S1 bands (duty / eHz / mA per segment, settled last 2 s)

**binz/s00-r1-ladder** — runs 144040, 145326, 150102; bus max 12107 mV

| rung | duty band | eHz runs | eHz band | mA runs | mA band |
|---|---|---|---|---|---|
| 10 % | 233..257 | 542 / 544 / 544 | 515..571 | 134 / 151 / 155 | 128..163 |
| 20 % | 418..462 | 1027 / 1030 / 1032 | 976..1083 | 302 / 301 / 310 | 286..326 |
| 30 % | 603..667 | 1441 / 1443 / 1444 | 1369..1516 | 554 / 555 / 588 | 526..618 |
| 40 % | 788..870 | 1787 / 1784 / 1785 | 1695..1876 | 880 / 883 / 908 | 836..953 |
| 50 % | 973..1075 | 2118 / 2129 / 2122 | 2012..2236 | 1400 / 1409 / 1428 | 1330..1499 |
| 60 % | 1158..1280 | 2466 / 2455 / 2468 | 2332..2592 | 2065 / 1988 / 2042 | 1888..2168 |
| 70 % | 1343..1485 | 2762 / 2771 / 2780 | 2624..2919 | 2833 / 2832 / 2864 | 2690..3007 |

**binz/s00-r2-start** — runs 144217, 145438, 150214; bus max 11987 mV

| rung | duty band | eHz runs | eHz band | mA runs | mA band |
|---|---|---|---|---|---|
| 10 % | 232..257 | 544 / 544 / 544 | 517..572 | 139 / 138 / 137 | 130..146 |

**binz/s00-r3-step** — runs 144745, 145537, 150310; bus max 12477 mV

| rung | duty band | eHz runs | eHz band | mA runs | mA band |
|---|---|---|---|---|---|
| 10 % | 233..257 | 544 / 544 / 543 | 516..571 | 158 / 138 / 137 | 130..166 |
| 60 % | 1158..1280 | 2457 / 2462 / 2460 | 2334..2585 | 2057 / 2039 / 2032 | 1930..2160 |
| 40 % | 788..870 | 1823 / 1831 / 1830 | 1732..1922 | 799 / 807 / 814 | 759..854 |
| 25 % | 511..565 | 1263 / 1259 / 1259 | 1196..1326 | 373 / 392 / 404 | 354..424 |
| 15 % | 326..360 | 814 / 817 / 812 | 772..858 | 133 / 202 / 200 | 126..213 |
| 10 % | 233..257 | 544 / 544 / 543 | 516..571 | 152 / 146 / 136 | 129..160 |

**binz/s00-r4-top** — runs 144911, 145651, 152204; bus max 12131 mV

| rung | duty band | eHz runs | eHz band | mA runs | mA band |
|---|---|---|---|---|---|
| 10 % | 233..257 | 543 / 545 / 542 | 514..572 | 167 / 149 / 141 | 134..175 |
| 20 % | 418..462 | 1030 / 1029 / 1025 | 974..1082 | 301 / 298 / 292 | 278..316 |
| 30 % | 603..667 | 1446 / 1445 / 1445 | 1372..1518 | 569 / 569 / 554 | 527..598 |
| 40 % | 788..870 | 1788 / 1782 / 1780 | 1691..1877 | 886 / 910 / 890 | 842..956 |
| 50 % | 973..1075 | 2118 / 2136 / 2111 | 2006..2242 | 1381 / 1367 / 1348 | 1281..1450 |
| 60 % | 1158..1280 | 2467 / 2449 / 2472 | 2327..2595 | 2022 / 2021 / 2003 | 1903..2123 |
| 70 % | 1343..1485 | 2781 / 2784 / 2792 | 2642..2932 | 2853 / 2822 / 2830 | 2681..2995 |

**l431/s00-r1-ladder** — runs 144555, 145243, 150020; bus max 12352 mV

| rung | duty band | eHz runs | eHz band | mA runs | mA band |
|---|---|---|---|---|---|
| 10 % | 224..248 | 557 / 557 / 555 | 527..585 | 54 / 53 / 54 | 50..57 |
| 20 % | 410..454 | 980 / 979 / 978 | 929..1029 | 241 / 241 / 243 | 229..255 |
| 30 % | 597..659 | 1383 / 1389 / 1379 | 1310..1458 | 571 / 544 / 559 | 517..600 |
| 40 % | 783..865 | 1754 / 1774 / 1752 | 1665..1863 | 1021 / 1010 / 1061 | 959..1114 |
| 50 % | 969..1071 | 2071 / 2077 / 2066 | 1963..2180 | 1491 / 1487 / 1500 | 1413..1575 |
| 60 % | 1155..1277 | 2333 / 2338 / 2336 | 2217..2455 | 2086 / 2050 / 2060 | 1947..2191 |

**l431/s00-r2-start** — runs 144641, 145424, 150200; bus max 11755 mV

| rung | duty band | eHz runs | eHz band | mA runs | mA band |
|---|---|---|---|---|---|
| 10 % | 224..248 | 557 / 557 / 557 | 529..585 | 54 / 53 / 53 | 50..56 |

**l431/s00-r3-step** — runs 144520, 145508, 150244; bus max 12847 mV

| rung | duty band | eHz runs | eHz band | mA runs | mA band |
|---|---|---|---|---|---|
| 10 % | 224..248 | 557 / 556 / 557 | 528..585 | 53 / 54 / 56 | 50..58 |
| 60 % | 1155..1277 | 2331 / 2338 / 2340 | 2214..2457 | 2091 / 2054 / 2075 | 1952..2195 |
| 40 % | 783..865 | 1768 / 1781 / 1792 | 1680..1881 | 871 / 857 / 846 | 804..914 |
| 25 % | 504..556 | 1200 / 1210 / 1209 | 1140..1271 | 281 / 265 / 274 | 252..295 |
| 15 % | 317..351 | 786 / 790 / 790 | 747..830 | 95 / 87 / 87 | 83..99 |
| 10 % | 224..248 | 557 / 557 / 556 | 529..585 | 54 / 53 / 53 | 50..56 |

**l431/s00-r4-top** — runs 144829, 145614, 150347; bus max 12698 mV

| rung | duty band | eHz runs | eHz band | mA runs | mA band |
|---|---|---|---|---|---|
| 10 % | 224..248 | 556 / 556 / 557 | 528..585 | 54 / 53 / 53 | 50..57 |
| 20 % | 410..454 | 981 / 980 / 982 | 930..1031 | 247 / 246 / 241 | 229..260 |
| 30 % | 597..659 | 1386 / 1386 / 1382 | 1312..1456 | 566 / 557 / 561 | 529..595 |
| 40 % | 783..865 | 1769 / 1772 / 1760 | 1672..1861 | 1022 / 1018 / 1021 | 967..1073 |
| 50 % | 969..1071 | 2083 / 2063 / 2084 | 1960..2188 | 1485 / 1487 / 1498 | 1411..1573 |
| 60 % | 1155..1277 | 2327 / 2330 / 2312 | 2196..2446 | 2062 / 2081 / 2080 | 1959..2185 |

---

## Bands v4 (operator, 2026-10-25, during S2.1) — commit ac9184f

The S1 mA bands were narrower than the instruments: the L431 current is one
ADC count = 26.7 mA and is software-triggered from main context, so a
layout-only control image (s00 + 8 jumped-over NOPs) reads 80 instead of
54 mA at 10 %; binz's 10 % current spreads 117–170 mA across 35 known-good
segments (per-run 3-shunt zero); the s00 image itself failed the old R3
25 % band. Re-baselined with s00 rounds 4–6 (24 runs, all clean) plus 7
bisect s00 runs = 55 runs. mA band = min..max ±max(5 %, 30 mA); the 0.35 s
walk-down segments keep duty/dsy/kill gates but get no eHz/mA band. eHz
stays ±5 %. `docs/gc_bands_s00.json` (run list inside). Self-check 55/55.

## S2.1 — saturate the interval count (CODE) — tag gc-s02.1-psu

`isr_logic.rs` bemf_polling and bemf_zero_cross: `count().min(u16::MAX as
u32) as u16` (the design doc named only the polling site; the zero-cross
path had the same wrap). Host test with a 70 000-count mock (fails without
the fix: 4464). Functionally dead on both bench chips (16-bit interval
counts) — the bench difference is layout only.

| | L431 | binz |
|---|---|---|
| ELF sha256 | `613b3ab7d6991e0b…` | `d193f0c32bc16e61…` |
| text | 54484 (+8) | 56692 (+16) |

Host gate PASS (s02.1). ISR diff vs s00: only the COMP and TIM6 roots
differ on both chips (the two changed call trees; register allocation
around the new compare). binz WCET: ADC_COMP 684 → 698 cyc @2ws (+5
insns), TIM6 5114 → 5118, TIM14 and DMA unchanged.

Bench (v4 bands): 14 runs, all R1–R4 on both boards, `--check` 14/14,
dsy = 0, no kills, no reboots. Battery: no (no control behaviour change).
Verdict PASS.

## S2.2 — L431 DWT-backed delay_micros (HOST) — tag gc-s02.2-psu

`mcu_l431/system.rs` delay_micros spins on DWT.CYCCNT (enabled at boot on
every L431 build) instead of `asm::delay(us * 80)`: since cortex-m 0.7.8
`asm::delay(n)` is n `subs/bne` iterations (~3 cycles each on the M4).
Callers: startup/beacon tones and the sine stepper (not used on the
bench). Measured with `scripts/boot_timing.py` (reset → boot lines): the
startup tune (nominal 600 ms, as AM32) 1791 ms → 593 ms.

| | L431 | binz |
|---|---|---|
| ELF | text 54452 (−32) | byte-identical to s02.1 |

Host gate PASS (s02.2). ISR diff vs s02.1: identical on both boards
(isr_diff now cuts at symbol size; the padding after a noreturn call had
shown as one extra line). Bench R1–R4 both boards: `--check` 8/8, dsy = 0,
no kills. Battery: no. Verdict PASS.

Open: the G071 (and F051) `asm::delay` ports have the same ~3x
(binz tune 1776 ms); M0+ has no cycle counter — needs a timer delay.

## S2.3 — battery voltage in centivolts everywhere (CODE) — tag gc-s02.3-psu

One internal unit, AM32's own: `CentiVolts` replaces the ambiguous
`MilliVolts` (AM32 boards produced cV, bench boards mV, under one type).
`AdcCount::to_centivolts(voltage_divider_x100)` = pin mV × ratio×100 /
1000. Board yamls state the divider as the true ratio ×100 under a new key
`voltage_divider_x100` (AM32 110 → 1100, 210 → 2100; bench 933 / 1194
unchanged); the old `voltage_divider` key is a build error with a message
(build.rs had no deny_unknown_fields, so a stale key would silently
default). Consumers: KISS sends the value as is (was `/ 10`), EDT `/ 25`
like AM32 dshot.c (was `/ 250`), orbit scaling `> 600` / `816` (same
ratio), voltage-ramp constants renamed `_CV` (values were always AM32 cV),
LVC unchanged (it already assumed cV; on the bench boards it used to count
~31 "cells"). Bench boundary: `rm32_stm32::isr::vbat_mv()` = cV × 10 feeds
the guard, both recorders, the i / b / KILL / loop lines, gecko and SR_MV;
EDT gets cV. Harness: default divider 1100; there is no load_eeprom reset of
the divider in this tree (reviewer's note did not apply). Tests: EDT unit
tests and `edt_voltage_temp.txt` move to cV inputs (12300 → 1230, frames
unchanged); new full-range test vs AM32's formula (rm32's fixed-point
pin-mV truncates up to 1 mV low: ≤ 2 cV at 11:1, pre-existing) and bench
divider tests.

| | L431 | binz |
|---|---|---|
| ELF sha256 | `459b59f7cfd8b85e…` | `a64412126da95800…` |
| text | 54556 | 56812 |

Host gate PASS (s02.3); C harness still exactly the 10 known diffs. ISR
diff vs s02.2: DMA (EDT) and L431 TIM6 (bench SR_MV store); binz WCET
DMA1_CHANNEL1 1409 → 1415 cyc, others unchanged. Meter (operator): bus
11.6–11.8 V vs firmware L431 ≈ 11.65 V, binz ≈ 11.78 V idle — match.
Bench R1–R4 both boards: `--check` 8/8, dsy = 0, no kills; recorder bus
now 10 mV resolution, ranges as before. (A first R1 pair overlapped — a
tool background move — and was discarded and re-run solo.) Battery: no.
Verdict PASS.

## S2.4 — stale doc fixes (HOST) — tag gc-s02.4-psu

- `isr_logic.rs` bemf_zero_cross: the porting note said F051/G071/G431 do
  not pre-ack the COMP EXTI line; now F051 and G431 ack at entry and
  L431/G071 use AM32's half-average-interval gate (ack or camp).
- `main_state.rs`: the REQ-RESET-ON-TIMEOUT doc comment had drifted onto an
  LVC test and cited "Src/and :1917"; moved back onto
  `signal_timeout_armed_requests_reset`, citing AM32 Src/main.c:2388/:2401.
- `main_state.rs` sine-changeover note listed 8 "missing" assignments;
  most are now done (apply_sine_changeover + the tick ISR). Still missing
  against AM32 main.c:2731-2746: SET_INTERVAL_TIMER_COUNT(9000) and the
  stall-protect last_duty_cycle seed (open, not this step).

Host gate PASS (s02.4); ISR diff vs s02.3 identical on both boards; flash
images byte-identical (ELFs differ only in debug info). Bench R1–R4 both
boards 8/8 in the v4 bands, dsy = 0, no kills. Verdict PASS.

**Increment 2 closed** (S2.1–S2.4, tags gc-s02.1-psu … gc-s02.4-psu).
Open from this increment: G071/F051 `asm::delay` ~3x long (binz tune
1776 ms; needs a timer delay), current-unit split (cA vs mA), sine
changeover gaps above, MV_PER_COUNT truncation (≤ 1 pin-mV vs AM32).
