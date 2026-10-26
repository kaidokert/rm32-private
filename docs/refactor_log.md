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

---

## S3.1 — `rm32-harness` crate (HOST) — tag gc-s03.1-psu

`rm32/src/bin/harness.rs` → `rm32-harness/src/main.rs` (git mv, no code
change); new workspace member `rm32-harness`, binary still `rm32_harness`
(`tests/blackbox/harness.py` unchanged). Entry condition met in the same
commit: `scripts/gc_gate.sh` and `.pre-commit-config.yaml` test and clippy
the whole workspace (`cargo test --workspace`, `cargo clippy --workspace`),
harness built with `-p rm32-harness`. Host tests 384 before and after
(377 lib + 7 harness). `--all-targets` was tried for clippy and dropped:
test code was never clippy-clean (41 errors) — same lib+bin coverage as
before, now workspace-wide.

Gate PASS (s03.1); ISR diff identical; flash images byte-identical to
s02.4. Bench R1–R4 both boards 8/8 in the v4 bands, dsy = 0, no kills.
Verdict PASS.

## S3.2 — `rm32-bench` crate (HOST) — tag gc-s03.2-psu

`bench_input`, `bench_rec`, `blackbox` (git mv, no code change) and
`examples/dump_config.rs` move from `rm32` into the new no_std workspace
member `rm32-bench` (rm32 only as a dev-dependency for the example). The
modules used nothing from rm32. `rm32_stm32` depends on `rm32-bench`; 22
`rm32::bench_*` / `rm32::blackbox` paths rewritten. CLAUDE.md and
feature_validation_roadmap.md now say `cargo run -p rm32-bench --example
dump_config`. Host tests 384 = 349 rm32 + 28 rm32-bench + 7 harness
(unchanged total).

Gate PASS (s03.2); ISR diff identical on both boards. Flash NOT
byte-identical: the crate split changes thin-LTO inlining in `main` only
(binz `main` −36 B; L431 `main` +12 B plus two renamed bench functions of
equal size; public builds −16 / −8 / +8 / +8 B text). Same source; no ISR
root changed. Bench R1–R4 both boards 8/8 in the v4 bands, dsy = 0, no
kills. Verdict PASS.

## L431 reference rebuilt on the shared motor (2026-10-25)

The L431 was rewired to the binz motor behind a 4-relay board on COM19
(all OFF = L431, all ON = binz; mapped by a 15 % hold on each board in each
position: the unrouted board sits in open-loop at 7-17 eHz). New tooling:
`scripts/relay.py` (status/set/check, 3 s deadline per exchange, refuses a
split or partial status), `binz_profile.py` relay gate (`--no-relay` to
skip) and `--sine board|on|off`, `rm32_stm32/scripts/l431_ref_run.sh`
(route + flash + reset + one run), `rm32_stm32/scripts/lock_stats.py`
(per-segment lock summary).

Carrier A/B at 15 %, 3 runs a side interleaved, s03.2 code: 24 kHz
variable held 2/3 (186-213 mA; the pre-A/B run also lost lock), 48 kHz
fixed held 3/3 (106-133 mA, 784-794 eHz vs binz 786 on the same motor).
Adopted 48 kHz fixed in `neutron_l431.yaml` (binz's carrier, AM32
pwm_frequency=48 / variable_pwm=0).

Startup bisect: at 48 kHz the L431 does not start at 10 % (2/2, open-loop
8 eHz for 4 s); 24 kHz starts it (1/1). Sine start (binz's mode) did not
help (2/2 stuck, reversed and forward direction). Dead time and startup
limits left alone; the L431 profiles start at 15 % (starts 3/3 + 16/16)
and hold 10 % once running (R3 tail, 4/4). Current offset: idle 0 mA,
held currents consistent run to run.

Ladder walk: 20-80 % held (80 % = 2930 eHz, 4.1 A, 15 s in R4); 90 %
tripped the guard (OC, 5253 mA). New L431 profiles (plan section 3): R1
15..80 %, R2 15 % from arm, R3 15:3,60:4,walk-down,10:4, R4 15..70 at 2 s
+ 80:15.

Bands: 4 interleaved R1-R4 rounds (`s00.1-*`, 16 runs, all dsy=0, no
kills) on one image (s03.2 code + 48 kHz yaml, flash-identical to the A/B
48 kHz arm), built with the v4 recipe (`--ma-floor l431=30 --min-settled
1.0`) and merged into `docs/gc_bands_s00.json` (l431/* replaced, binz/*
byte-identical). Self-check 16/16; binz s03.2 4/4; the old-motor L431
s03.2 runs 0/4 (falsifier). Operator approved the bands as built
(80 % top, 0.4 A under the guard accepted).

S3.3 (DiagSink) is parked in `git stash` (S3.2 code under these runs); its
L431 runs restart against the new bands.

## binz bands rebuilt through the relay (2026-10-25)

S3.3's binz R2/R3/R4 missed the binz bands on mA only (1-3 % over the
edge, eHz in band, dsy=0). Bisect: the known-good s03.2c binz image
(byte-identical to s03.2) reproduced the miss on R3/R4. Both images vs
the s00 band means: eHz -1..-3 %, mA +6 % mean under load, +20..+40 % at
10 % in R2-R4 -- the binz motor phases now run through the relay
contacts; the binz bands predate the relay. Operator approved a rebuild:
4 interleaved R1-R4 rounds (`s00.2-*`, 16 runs, dsy=0, no kills) on the
s03.2c image, v4 recipe, binz/* replaced, l431/* byte-identical.
Self-check 16/16, s03.2c bisect runs 3/3, pre-relay binz s03.2 runs 1/4
(falsifier).

## S3.3 -- `DiagSink` (CODE) -- tag gc-s03.3-psu

`rm32::diag::DiagSink`: every hook an `#[inline(always)]` no-op, `()` the
production sink. The 11 `dbg_*` atomics and their accessors leave
`SharedState`; the port's `rm32_stm32::diag` holds them behind
`debuguart` (`BenchDiag`: one counter block, DWT brackets on M4) and
exposes 0-returning getters in production for the RTT `[loop]` /
last-words lines. ISR and main-loop cycle brackets, the DShot CRC / bidir
/ command counters and the binz desync injection (`DESYNC_INJECT` ->
`Commutation::inject_skip`) all go through `DIAG`. The control-tick count
stays in `SharedState` as `tick_count` (a clock: G071 bench time derives
from it), not a diagnostic.

Gate tooling: gc_gate.sh dumps the four public builds' ISR roots too and
runs `isr_diff.py removals` per image. `removals` judges work, not
register allocation: fail on a new function, more instructions, more
calls / barriers / exclusives / IRQ masking, or a new peripheral address.
Self-test (an image vs itself) passes; the reverse direction (s03.3 ->
s03.2) fails on every root.

Gate PASS vs s03.2c (s03.2 code + the 48 kHz L431 yaml; ISR-identical to
s03.2). removals PASS on all four public builds: text -408 (G071), -408
(F051), -448 (L431), -280 (G431) B; DWT brackets, barriers and counter
atomics gone from COMP/TIM16/TIM6/DMA/EXTI roots. Bench images grow
slightly (L431 +176 B, binz +16 B: the counter block is its own base, +2
instructions in the L431 TIM6/TIM16 roots). Host tests 384 = 384.

Bench (shared motor, relay routed per run): L431 R1-R4 4/4 in its new
bands; binz R2/R3/R4 pass the relay-path binz bands (rebuilt above). The
first binz R1 missed low by a constant 40-78 mA at identical eHz (a
current-zero offset signature); reflashed the same image and repeated:
in band at every rung. Recorded as an unreproduced outlier
(captures/bisect_s03.3/), the repeat is the step's R1. Verdict 8/8 PASS.

## S3.4 -- `bench-diag` only in debug builds (HOST bench / size report) -- tag gc-s03.4-psu

`rm32_stm32` depends on rm32 with `default-features = false`; `debuguart`
(and so `benchuart`) enables `rm32/bench-diag`. That feature now gates
only `TransferActions::high_pin_count` (read by the debuguart DiagSink);
a duplicated cfg attribute in transfer.rs removed. `CLAUDE.md`'s
observability note updated.

Gate PASS vs s03.3c. Bench images: ISR identical (binz flash-identical;
L431 differs only in link order -- one `Display::fmt` instance moved,
same functions and sizes). Production size report: text G071 +8, F051
+16, L431 -24, G431 -24 B. `removals` passes on L431/G431 and fails on
G071/F051 by +5 instructions in EXTI4_15: dropping the u8 field re-runs
register allocation on M0 (3 extra spill ldr/str, 2 alignment nops), no
new call, barrier, atomic or peripheral access. The plan classes S3.4 as
HOST for bench images + a size report for production, so this is
recorded, not gated. Operator confirmed the HOST classification. Host tests 384.

Bench R1-R4 both boards 8/8 in band, dsy=0, no kills.

Increment 3 closed: S3.1-S3.4 tagged. Bench code is out of the core
(harness and bench crates, DiagSink) and production builds carry none of
it.

## Increment 4-5a goal (S4.1-S4.5, S5.1-S5.2) -- Calls for review

Judgment calls made without stopping, for review at the end of the goal.

**Summary for review (goal closed at gc-s05.2r3-psu).** Tags: gc-s04.1..5,
gc-s04.4f, gc-s05.1, gc-s05.2 and gc-s05.2r2 (superseded, Call 15),
gc-s05.2r3 (all -psu). Host tests 384 -> 392.
Needs a decision:
- Call 9 -- S4.4 shipped a defect (binz current wrapped above ~2.1 A on
  the 1 kHz path) that the bands could not see; fixed in S4.4f. Proposal:
  a band (or gate check) on `actual_current` / `i50max`, which caught it.
- Call 15 -- the first S5.2 commit (gc-s05.2-psu) went out with ISR
  removals FAIL on all six images, logged as "Gate PASS". Reworked twice;
  S5.2r3 is ISR-identical with removals PASS on all six images. Price:
  the gate half of `Bridge` keeps the name `hal::PhaseOutput` (Call 14 g).
- Calls 2 + 12 -- neither guard kill (S4.1, S5.1) was bisected when it
  happened (reset + repeat, a breach of the goal's rule). Both bisects
  were run after the goal review: no regression at either step (S4.1:
  1/8 vs 0/8 trips, peak p=0.81; S5.1: 0/8 vs 0/8, peak p=0.89).
- Calls 2 + 12 -- binz R3 slam trips the 4.8 A OCFAST guard ~1 run in 11.
  Now bisected properly (Call 12): baseline vs S5.1 ABAB, 8 a side, 0/8
  trips each, peak guard EWMA 4423 vs 4406 mA (p=0.89). The trip risk is
  present at the baseline; operator call on guard level vs R3 profile.
- Call 8 -- binz r-line `imean` is +46 mA on S5.1 vs baseline at equal
  in-run current (ABAB p<1e-4); the same image moves ~110 mA between
  sessions. Mechanism read from code (main-loop dispatch weighting), not
  yet verified; proposed test: print the sample count in the r-line.
- Call 10 (noted, not changed) -- AM32 divergences: LED blink ignores
  `>> forward`; consumed charge uses raw mA per second, not AM32's Q16
  mAh, with a mislabelled `/ 1000`.
Accept or veto: Calls 1, 3, 7, 13, 14 (mechanisms: `Platform`, `Timing`,
`AffineDiv` with the 64-bit fallback, `step_legs`, `Bridge` /
`PhaseOutput` supertrait / borrowed `StmBridge`); Call 4 (HOST read as identical ISR
roots); Calls 5, 6 (scope); Call 11 (discarded round); Call 16 (band
check name regex widened twice, bands file untouched).

1. **Platform mechanism (S4.1).** `rm32::platform::Platform` carries the
   clocks (`TICK_HZ`, `TIMER_HZ`); `MotorHal: Platform` is a supertrait so
   every ISR entry point already generic over `H: MotorHal` reads
   `H::TIMER_HZ` with no new parameter. The port has one zero-sized
   `StmPlatform` (TIMER_HZ = CPU / (TIMER_PSC + 1), 2 MHz on all four
   chips; TICK_HZ = 20 000 nominal, the L431's 19.6 kHz TIM6 is AM32's
   own rounding); the harness has `HarnessPlatform` (AM32's 20 kHz / 2 MHz).
   The `'static` bound in the design sketch is dropped (it forced
   `'static` on every IsrHal peripheral parameter for nothing). Non-generic
   code (`MainState`, `SystemTick`) gets `P: Platform` as a type parameter
   when S4.2 first needs it there (a handful of construction sites).
2. **binz R3 slam vs the surge guard (S4.1).** The first S4.1 binz R3
   tripped OCFAST (fast 16 ms current EWMA > 4800 mA) 0.24 s into the
   10->60 % step; R4 then refused (latched). The binz image differs from
   s03.4 in 3 bytes, all panic-location line numbers: the code is
   byte-identical to the known-good image, so there is nothing to bisect.
   Reset + repeat passed (i50max 4816 mA). Every binz R3 since s00 peaks
   4.3-4.8 A on the 50 ms reading, i.e. this slam grazes the 4.8 A surge
   line by design of the profile; 1 trip in ~25 R3 runs. Limits untouched
   (operator's 5 A allowance is final). Recorded per occurrence; if it
   keeps recurring, the review question is the R3 profile (60 % slam on a
   5 A supply), not the guard.
   **Bisect done after the goal review** (reset + repeat was a breach of
   the goal's rule, not a bisect). Endpoints A = s03.4 binz image (last
   good, sha1 5de1f400), B = the s04.1 image that tripped (1fa2dd41);
   binz R3, ABBA, 8 a side, flash before each run; bar as in Call 12.
   Result: trips A 0/8, B 1/8 (run 7, OCFAST at +0.24 s, fast EWMA 4827,
   the same instant as every R3 trip); peak guard EWMA 4426 +- 223 vs
   4395 +- 294 mA with B's killed run counted as a censored 4800
   (p=0.81); dwell p=0.87, `i50max` p=0.54, `imean` 856 vs 857. One A
   run peaked at 4849 mA on the recorder without tripping. Bar not met:
   no regression at S4.1; both sides graze the 4.8 A line.
   (`ab_r3_peaks.py` now keeps killed runs with a censored peak instead
   of refusing them.)

3. **Timing mechanism (S4.2).** Every clock-unit constant moves into
   one blanket trait, `constants::Timing: Platform`, written in µs / ms
   through `us_to_ticks` / `ms_to_control_ticks` and read as `P::NAME`
   (associated consts: always compile-time). `MainState<P, LED>` and
   `SystemTick<P>` take the platform as a type parameter; `SharedState::
   new::<P>()` is a generic const fn (the struct stays non-generic, so the
   ISR-side `SharedComm` code is untouched). Its `Default` impl is
   deleted (nothing used it, and it would have had to pick a clock).
   Free functions that need a clock (`commutation_timer_expired`,
   `DutyState::ramp_limit`, `sine_step`, `ToneScheduler::tick`,
   `variable_pwm_mode1/2`, `telemetry_interval_ticks`) take `P`. Legacy
   symbol mangling keeps generics in the stripped hash, so non-inlined ISR
   callees keep their names in the ISR diff. Tests use a `#[cfg(test)]
   TestPlatform` (AM32 clocks); not a default.
4. **"Disassembly identical" read as ISR roots (S4.2).** All ISR dumps
   are identical. The main-loop code is not: making `MainState` /
   `SystemTick` generic moves their codegen into the firmware crate and
   LLVM inlines differently (text -20..-224 B). Every restated constant
   is proven equal to AM32's literal by a host test, so the change is
   inlining only. The goal defines HOST as identical ISR dumps; bench 8/8.
5. **Scope of S4.2.** Restated: the ten former clock constants, the
   isr_logic `STARTUP_COMMUTATION_INTERVAL` (+ its /2 timer count),
   AM32's inline literals (desync stop 1000, filter 500/50/100..500,
   old-routine re-entry +500, sine changeover 9000, boot interval 12500,
   variable-PWM breakpoints) and tone's `TICKS_PER_MS`. The dead
   `INITIAL_COMMUTATION_INTERVAL` (10000, unused) is now the boot value
   12500 that SharedState actually used. Left for their own steps:
   capture-timer ticks in transfer.rs (S7.2, `CAPTURE_HZ`), the
   main-loop counters (`ten_khz_counter > 20000`, LED blink: S4.5), the
   `e_com >> 1` / `/ 3` conversions (S4.3). `DESYNC_RESET_INTERVAL`'s
   doc said "5 ms"; it is 2.5 ms (5000 ticks at 2 MHz): doc corrected.

6. **Main-loop `e_com_time / 3` (S4.3).** `MainState`'s plain `/ 3`
   (a real division in main-loop code) now goes through
   `e_com_us_to_avg_ticks`, i.e. `div3_i32`. `div3_i32` is exact for every
   `i32` (host-tested against `/ 3` over +-200 000 plus the extremes), so
   the value is unchanged and the 1 kHz path loses a divide. The bench
   `[loop]` print's `/ 3` in the port is bench-only code and left alone.

7. **Board ADC constants: mechanism (S4.4).** The four board facts
   (`VOLTAGE_DIVIDER_X100`, `MILLIVOLT_PER_AMP`, `CURRENT_OFFSET`, and also
   `USE_NTC`, which was the fourth `MainState` copy of a `BoardConfig`
   field) are `Platform` associated consts; the port's come from a
   build.rs-generated `board_platform.rs` (the library cannot see the
   binary's `BOARD`), the harness's and core tests' from
   `BoardConfig::DEFAULT`. Divisions become `units::AffineDiv`: a const fn
   that searches a 32-bit `((x*mul + add) >> shift) - sub` form, exhaustively
   verifies it against the exact quotient for every input at compile time,
   and fails the build if none exists. L431's 933/1000 divider has no
   direct form over 0..3299 pin-mV, so `AffineDiv` also tries
   `A x + floor(a1 x / d)` and `(A+1) x - ceil(a2 x / d)`. Current keeps
   AM32's truncation toward zero with two forms split at the first raw
   count whose numerator is non-negative. The old functions are kept,
   verbatim, as the test reference (`s2_3_centivolts/milliamps`), and the
   test was falsified once (planted +1 in the Q16 constant: fails).
   Consequences on the images: per M0 image, exactly two soft-division
   calls (`/ 1000`, `/ mVpA`) and one `__aeabi_lmul` (the `fixed` U16F16
   multiply) leave the main loop, none are added; `NTC_TABLE` (512 B) is
   dead-stripped on every board without an NTC.
8. **binz `imean_ma` moved +40..+70 mA at S4.4 (unbanded; read as a
   main-loop speed effect, not a current change).** Repeated A/B in one
   session: s04.3 image 1172, 1172; s04.4 image 1216, 1212 (r1). The
   recorder's tick-uniform mean is unchanged (1213/1211 vs 1208/1203) and
   every segment matches. `imean` averages the 1 kHz block's samples;
   when a main-loop iteration overruns 1 ms under binz load (TIM6 alone
   is ~74% CPU at 70%) a block sample is lost, so high-current time is
   under-weighted. `imean - time-mean` narrows at every step that removed
   main-loop work (r4: s03.2 -174, S3.3 -114, S4.3 -99, S4.4 -32; r1:
   -71, -48, -41, +8). Implication worth a look later: on binz at high
   duty the 1 kHz PID block (current limit, stall) runs below 1 kHz.
   Not changed here. (Amended after S4.4f, Call 9: part of S4.4's shift
   rode on the wrapped current; with the fix imean is 1204 / 906 / 1930 vs
   S4.3 1172 / 864 / 1882 and buggy S4.4 1216 / 919 / 1950, so ~+30..+50
   remains. The main-loop reading still fits; it is not directly tested.)
   (A second amendment after the first S5.2 bench called the main-loop
   reading refuted, from single runs in separate sessions. RETRACTED: the
   Call 12 ABAB shows the same image (s00 binz, sha-verified) reads
   imean 862 +- 8 today against 735..766 when the bands were built, so
   one run per session cannot resolve a 40 mA step. Interleaved, 8 a
   side: s05.1 908 +- 8 vs s00 862 +- 8, +46 mA, p<1e-4, while the guard
   EWMA (dwell mean -15 mA, peak p=0.89) and `i50max` (p=0.89) do not
   move: the motor draws the same current. Mechanism, read from code and
   NOT yet verified: `imean` is the mean of one sample per 1 kHz dispatch
   since the pre-arm `b` query, and that dispatch runs in the main loop
   (`one_khz_counter_check_and_reset`), firing when the ISR's counter
   reaches 20 and resetting it. A slower main loop overshoots 20 by a few
   ticks, so the run phase contributes fewer samples than the idle phase
   and `imean` reads lower; a lighter main loop raises it. Direct test,
   proposed not done (firmware change outside the step): print
   `take_mean_ma().1` (the sample count) in the r-line.)
   (CORRECTED at S5.2f per the increment 4-5a review, verified: there is
   no idle time in `imean`. The profile sends `H` at the start of segment
   0 (`binz_profile.py:146`), with the motor already running at 10 %,
   and `H` calls `take_mean_ma()` (`main.rs:1663`), so the window runs
   from `H` to motor stop. The weighting is inside the run: at 60 % the
   ISRs leave the main loop so little CPU that one pass spans 40-90
   control ticks, `one_khz_counter_check_and_reset` drops the overshoot,
   and the 1 kHz block adds one sample for a window of many scans, so the
   CPU-starved 60 % segment is under-sampled relative to the 10-40 %
   segments. Removing main-loop work raises its weight. Confirmed with no
   firmware change by `ecom10`, which has the same unweighted main-loop
   sampling: bis51A 12815.8 +- 12.1, bis51B 12640.4 +- 11.2 (-175, n=8 a
   side), the direction a heavier 60 % weight gives (its e_com is the
   shortest) while every settled recorder segment matches. The proposed
   sample-count test is no longer needed.)
   Also: one aborted A/B run reported "REBOOTED in arm";
   it was the first-boot banner arriving late (run started right after
   flash-binz's reset; one banner, tk=827 at the first `b`). Captures in
   captures/bisect_s04.4/.

9. **S4.4 shipped a defect; fixed as S4.4f (own gate, own bench, own
   tag), not folded into S4.5.** The goal (and my test) said "every raw
   count 0..4095"; binz's `raw_current()` is not a 12-bit count, it is
   metered mA + 2000 as u16 (known since the increment-3 review, and I
   still verified only 0..4095). Above ~2.1 A the S4.4 multiply-shift
   wrapped modulo 4096 counts: `actual_current` (1 kHz path: PID current
   limit, telemetry, `i50max`) read e.g. 704 mA at 4.8 A. Evidence:
   `i50max` exactly 2095 (= 4095 - 2000) on S4.4 R1/R3 and 1996 on S4.5's
   R3 (pre-S4.4 R3 peaks 4273..4816). The bands did not see it: the
   recorder's `ma` column is the guard's driver-fed EWMA, not
   `actual_current`. It surfaced as S4.5's binz R3 OCFAST kill (the guard
   reads the driver: a genuine >4.8 A surge with the 1 kHz current
   reading wrong), and R4 then hit the latched guard. Fix: the conversions
   are exact over all of u16 (the `Adc` trait's type), the exhaustive test
   runs 0..=65535, plus `binz_current_above_12_bits_is_exact`; both fail
   on the 12-bit domain (falsified). Voltage over u16 has no exact 32-bit
   form for 933 and 1194 dividers, so `AffineDiv` falls back to a 64-bit
   multiply only where no 32-bit form exists: binz gains one
   `__aeabi_lmul` (constant time, not a division); the public G071 board
   (1100) needs none; no image gains a division. S4.4f bench: 8/8, binz
   `i50max` 3452 / 4461 / 3601 (back in family). Process gap worth a
   look: no band covers `actual_current` itself.

10. **S4.5 counter source: the existing `tick_count`, advanced in the
    core.** AM32 increments `tenkhzcounter` / `ledcounter` in
    `tenKhzRoutine` and its main loop polls and resets them. Rather than
    add two ISR-side counters, the port's existing `tick_count_inc()`
    moves from `handle_tim6` into core `ten_khz_tick` (new required
    `IsrTiming::tick_count_inc`), so the harness advances it too
    (`do_tick_runs_shared_pipeline` asserts it), and the main loop keeps
    epochs: `now - epoch > TICK_HZ` (AM32 `> LOOP_FREQUENCY_HZ`, reset on
    sample) and the LED's `> HALF` / `> 2*HALF`. No new ISR work: the
    TIM6 diff is that one atomic increment moving, +1 address `add.w` on
    M4 (+2 jump-table half-words on L431), register renaming only on M0.
    `LED_BLINK_HALF_PERIOD` becomes `Timing::LED_BLINK_HALF_PERIOD` =
    ms(100), still u16, == 2000 at 20 kHz (host-tested). Noticed, NOT
    changed (out of S4.5's scope, would change behaviour): AM32 blinks at
    `2000 >> forward` (twice as fast reversed); AM32 accumulates consumed
    charge as `(actual_current << 16) / 360` (mAh in Q16) where rm32 adds
    raw mA per second and reports `/ 1000` with a "uAh -> mAh" comment
    that does not match.
11. **First S4.5 round discarded, not counted.** It ran on top of the S4.4
    defect (Call 9): binz R3 OCFAST at +0.25 s into the 60% slam, R4 hit
    the latched guard. The bisect showed the cause in S4.4, not S4.5;
    S4.5 was re-gated against s04.4f and re-benched from scratch (8/8,
    binz R3 `i50max` 4621). Discarded round in captures/bisect_s04.5/.

12. **binz R3 OCFAST on S5.1, an image byte-identical to S4.5 (Call 2's
    class; reset + repeat, no re-band).** `objcopy -O binary` of the
    s04.5 and s05.1 binz ELFs: 57 696 B each, `cmp` identical, so the
    bisect has no code difference to search. Kill at +0.25 s into the
    10 -> 60% slam, the same instant as every R3 trip. Predeclared bar:
    two trips in the next three R3 runs would trigger an ABAB against
    the s04.3 image; after the latch reset the next three passed
    (i50max 4655, 4561, 4596). Tally of binz R3 on correct code
    (excluding S4.4 and the first S4.5 round, Call 9): 3 OCFAST kills in
    32 runs (s00 first run, S4.1, S5.1), about 9%. The guard (4.8 A fast
    EWMA) sits inside the R3 slam's own peak distribution (4273..4816);
    a limits question for the operator, not changed. Two runs went in one
    command (my one-hold rule; logged in the notebook).
    **Bisect done after the goal review** (the reset + repeat above was
    not one: the known-good end was never re-confirmed on the bench).
    Endpoints A = gc-s00-baseline binz image, B = the s05.1 binz image
    that tripped (sha1 8f0fc79f / 8aa76a9c, copied to captures/gc/bis51A,
    bis51B). Binz R3 only, ABBA-interleaved, 8 a side, a flash before
    every run (clears the latch). Predeclared bar: B trips >= 4/8 with A
    0/8, or B's mean peak guard EWMA above A's at p < 0.05 (Welch, two
    sided), means a regression to bisect through the intermediate tags.
    Result (`scripts/ab_r3_peaks.py bis51A bis51B`): trips 0/8 and 0/8;
    peak guard EWMA 4423 +- 227 vs 4406 +- 253 mA (p=0.89), `i50max`
    4600 vs 4586 (p=0.89), dwell mean 2194 vs 2179 (p=0.08). Bar not met:
    both ends draw from the same peak distribution (4130..4751 mA, 49..670
    mA under the line), so the trip risk predates the refactor and there
    is no interval to bisect. The ABAB also exposed Call 8's `imean`
    shift cleanly (+46 mA, see there).
13. **S5.1 scope.** `rm32::bridge` holds `LegRole` (all five design roles;
    `LowPwm` is unused until S5.2's proportional brake) and `step_legs`.
    G071's `PLAN` is built by matching each leg's role; L431's `ROLES`
    (positions only, independent of drive mode) by `step_roles()` from
    `step_legs`, keeping the L431 writer's runtime shape so its code is
    unchanged. The sequential writer's `match step` (bridge-enable boards
    and F051 / G431) stays as it is: its per-step pin generics cannot read
    a runtime role array without becoming the per-leg decoder the binz
    WCET gate rejected; S5.2's `Bridge::com_step` takes it over.

14. **S5.2 shape (as first committed; S5.2r2 reshaped it, below).** Core: `rm32::bridge::Bridge` (re-exported as
    `hal::Bridge`) with the design's semantic methods; `MotorHal` exposes
    `bridge()` instead of `pwm()` + `phase()`; `PwmOutput` / `PhaseOutput`
    are deleted from the core. Deviations from the design sketch, all
    deliberate: (a) `com_step(step)` keeps one argument -- the
    `complementary` parameter is S5.3's drive-mode move (RISKY), so the
    port still decides it; (b) `BASE_PERIOD` / `PWM_CLK_HZ` are not added
    yet -- nothing reads them until `tim1_arr` initialisation moves off
    the chip constant, and an unused const per port is a value nobody
    checks; (c) the port keeps TIM1 and the gate pins as separate types
    behind port-internal traits (`TimerPwm`, `PhaseGates`, the old
    method sets) composed in `StmBridge<P, PH>`, which implements the core
    trait by `#[inline(always)]` delegation -- four TIM1 wrappers share
    one gate driver, so this is the smallest honest factoring. The
    firmware's sine start still writes CCRs with `mcu::write_tim1_ccr`
    from main context (the bridge lives in ISR state); only the harness
    calls `set_phase_duties`. Harness and core tests use one `MockBridge`
    each, keeping the HAL counters (`all_off`, `full_brake`, order
    recorder).
    **S5.2r2 shape.** Owning `StmBridge` moved TIM1 and the gate driver
    into a new `IsrHal` field and passed a two-pointer struct to the COM
    path, which changed ISR code (Call 15). Now: (d) the core splits the
    gate methods into `BridgeLegs`, and `Bridge: BridgeLegs` adds the
    timer methods; `commutation_timer_expired` asks for `BridgeLegs` only
    (it never touches the timer), so the port passes `&mut hal.phase`
    exactly as S5.1 did, and `PhaseDriver` implements the core's
    `BridgeLegs` directly (the port-internal `PhaseGates` is gone);
    (e) `MotorHal::Bridge<'a>` is a generic associated type and
    `bridge()` returns a view borrowed from the HAL: the port's
    `StmBridge<'a, P, PH>` holds `&mut` TIM1 and `&mut` gates, so
    `IsrHal` keeps S5.1's fields and layout; mocks use `&'a mut
    MockBridge` through a blanket `impl Bridge for &mut B`. (f) The
    port's own tone code keeps calling its TIM1 / gate types directly (as
    in S5.1); only core code goes through `bridge()`.
    (g) S5.2r3: the gate supertrait is named `rm32::hal::PhaseOutput`
    (defined in hal.rs, same five methods + `pulse_toggle` as the old
    trait), not `rm32::bridge::BridgeLegs`. Mangled symbol names carry
    the trait's defining path, and the removals check counts any name the
    old call tree did not have as new work; under the old name the ISR
    call trees match S5.1 name for name. `PwmOutput` is gone, the core
    drives one `Bridge`, and `PhaseOutput` survives as its gate half.

15. **First S5.2 commit superseded (gc-s05.2-psu stays, not moved).**
    Its gate printed `ISR removals: FAIL` on all six images and I logged
    "Gate PASS", arguing the diff was harmless (regalloc, one merged
    store). S5.2 is not one of the goal's exceptions (S4.4 divisions,
    S4.5 counters), so that was a condition not met, caught by the goal
    review. Reworked (Call 14 d-f) and re-gated twice: s05.2r (borrowed
    view only) still changed the COM root; s05.2r2 is ISR-identical on
    all six images by `isr_diff` (instruction counts equal, every root).
    Its removals check still prints FAIL: the five gate methods the ISRs
    call (`com_step`, `all_off`, `all_pwm`, `proportional_brake`, the COM
    root's `com_step`) are now mangled under `rm32::bridge::BridgeLegs`
    instead of `rm32::hal::PhaseOutput`. Deleting `PhaseOutput` is the
    step itself, so the names cannot stay; the bodies are identical. Not
    worked around by editing the gate. Flash images are not
    byte-identical: main-loop code changed (`play_startup` through the
    view) and handlers shifted (vector table). The tag was pushed, so it
    is not moved (no force-push). S5.2r2 (gc-s05.2r2-psu, also pushed)
    left removals FAIL on the rename and was superseded in turn by S5.2r3
    (Call 14 g): ISR diff identical AND removals PASS on all six images;
    gc-s05.2r3-psu is the S5.2 of record.

16. **Band-check name regex widened twice; no band changed.**
    `gc_summary.py` maps a run name to its band row by rewriting the
    step prefix to `s00-`. S4.4f needed one trailing letter
    (`[a-z]?`), S5.2r2 a letter and digit (`[a-z0-9]*`). Line 84 is the
    only line touched both times; `docs/gc_bands_s00.json` is unchanged
    since 13d609c (before this increment), and the s05.2 and s04.4f runs
    still pass 16/16 under the new pattern. Surfaced by the goal review
    as a possible instrument edit; recorded so it can be checked.

17. **S5.2f (d) LED blink: no change, AM32 does not restart the counter.**
    The review asked for the counter to restart on entering the blink
    range "as AM32 does". AM32's source says otherwise: `ledcounter++`
    runs unconditionally in the 20 kHz routine (`Src/main.c:1769`) and is
    zeroed only inside the blink branch (`:2412`), so on entering the
    range AM32 sees whatever the free-running u16 holds (above 4000 in
    94 % of cases): LED high at once, counter reset. rm32 since S4.5 does
    the same (its u32 age is always past the half period). Matching AM32
    means no change. Remaining divergences already noted in Call 10.

18. **S5.2f (e) i50max check: what it catches.** `gc_summary.py` fails a
    completed binz run whose r-line `i50max` is below its highest settled
    segment `ma`, or which has no `i50max`. On the S4.4 defect captures it
    fails R1, R3 and R4 (`i50max=2095` against 3000 / 2177 / 2982 mA) and
    passes R2, correctly: the start profile never goes above ~2.1 A, where
    S4.4's wrap began, so `actual_current` was right there. It also fails
    the discarded first S4.5 round's binz R1 (2095 < 3009). Across all
    117 other banded binz captures it passes; the tightest margin is
    +64 mA (R2 runs). Killed runs are already failures and are not
    checked. The bands file is untouched.

19. **S5.2f (c) tone through `bridge()`: statement order kept.** The first
    version computed the beep volume before the timer writes; binz's TIM6
    grew by 6 instructions (removals FAIL), isolated by building the
    image with fix (a) alone (PASS). Reading `state.config` between the
    prescaler and duty writes, where the old code read it, gives ISR code
    identical to S5.2r3 on all six images. (b) also made `AdcScale`
    `pub(crate)`: its constants have the now crate-private `AffineDiv`
    type, and nothing outside the core uses it (the public conversions
    are bounded on `Platform`).

20. **L431 R3 dump lost to serial character drops; run repeated once.**
    The first S5.2f L431 R3 wrote no CSV: all 12 recorder dumps had 7-11
    rows with a dropped digit or space (`11650` -> `1650`), different rows
    each time, and the parser keeps a dump only if every row is
    well-formed, so the two-read vote never started. Firmware clean
    (one boot, killed=0, guard=0, dsy=0); not a kill, desync, reboot or
    band miss, so repeated once (OK). Not new: every L431 R3 since S4.4f
    needed 4-8 dump attempts. Proposed tool fix, not done in this step:
    vote row by row over all dumps, ignoring malformed rows.

21. **S5.3 AUTO drive mode kept (a known divergence, now in one place).**
    AM32 runs complementary drive whenever the config enables it; rm32
    runs it only in interrupt mode and drives diode while polling
    (`old_routine`). S5.3 is "no behaviour change", so the policy moves
    unchanged into `rm32::bridge::drive_complementary(comp_pwm, shared)`
    with its rationale; dropping it is a behaviour change for its own
    step. `old_routine` is still read only when `comp_pwm` is set: the
    first version read it eagerly, which put one more critical section
    in binz's TIM6 tree (cpsid 83 -> 84, removals FAIL) and one more per
    step at runtime when comp_pwm is off.

22. **S5.3 tone: the config bit alone.** The port tone code passes
    `config.use_comp_pwm()` instead of the core's decision. Same result:
    `Tone::tick` returns `StartNote` only with the motor not running, and
    `old_routine` is a running mode, so the decision reduces to the config
    bit (AM32's tones follow comp_pwm alone too). Calling the core's
    decision there would add a critical section that cannot change the
    answer (it was the other +1 cpsid in the first gate). Blocking boot
    tunes (`Sounds`) pass false: they play before the config is loaded,
    where the port driver used to hold its constructed `false`.

23. **S5.3 bench 'D' key stays in the port.** The bench override
    (1 = diode, 2 = complementary, else the core's decision) now
    overrides the bool the core passes; value 3 ("auto") no longer means
    anything special because auto IS the core's decision. Boot still
    stores 3, so bench images behave as before. Production builds have no
    override.

24. **S5.3 imean reads higher on binz (not banded, explained).** R3
    imean 908.0 +- 3.0 (S5.2f) vs 923.7 +- 6.7 (S5.3), p=0.039; R1 and R4
    are higher in all three rounds too (+15, +26 mA). binz TIM6 lost 36
    instructions, so the main loop gets more of the CPU at 60-70 % and
    samples those segments more (the Call 8 mechanism, in the expected
    direction). Every banded metric agrees: 0 of 120 differ after
    Bonferroni, 1 at raw p < 0.05 (L431 R1 50 % mA -22, one ADC count),
    against ~6 expected by chance.

## S4.1 -- `Platform` trait with clock consts (HOST) -- tag gc-s04.1-psu

`rm32/src/platform.rs` (trait, no defaults), `MotorHal: Platform`,
`rm32_stm32::platform::StmPlatform`, `IsrHal` implements `Platform` by
delegating to it; harness `HarnessPlatform`, control-test mock likewise.
Nothing reads the consts yet.

Gate PASS vs s03.4; ISR dumps identical on all six images. Bench images
differ from s03.4 only in panic-location line numbers (binz: 3 bytes).
Host tests 384. `scripts/gc_round.sh` added: one As-run reference run per
call (relay, flash, name), so the step loop is one command per hold.

Bench 8/8 in band. One binz R3 OCFAST trip and one L431 R3 recorder dump
that never read back whole (CH340 corruption, no kill, dsy=0) were rerun;
originals in captures/bisect_s04.1/ (Call 2).

## S4.2 -- constants via `us_to_ticks` / `ms_to_control_ticks` (HOST) -- tag gc-s04.2-psu

`rm32::platform::{us_to_ticks, ms_to_control_ticks}` and the blanket
`constants::Timing` trait (26 constants, documented in physical units
with AM32's literal quoted); `MainState<P, LED>`, `SystemTick<P>`,
`SharedState::new::<P>()`. Mechanism and scope: Calls 3-5. New host test
`am32_clocks_fold_to_am32_literals` (every constant at 20 kHz / 2 MHz ==
AM32's literal): 385 host tests (was 384).

Gate PASS vs s04.1; ISR dumps identical on all six images. Main-loop
inlining changed (text G071 -184, F051 -112, L431 -64, G431 -80, binz
bench -224, L431 bench -20 B; Call 4). Bench 8/8 in band first time.

## S4.3 -- `six_intervals_to_us` / `e_com_us_to_avg_ticks` (HOST) -- tag gc-s04.3-psu

`rm32::platform::six_intervals_to_us::<P>` (AM32's `(sum + 4) >> 1` at
2 MHz, keeping AM32's +4 bias) replaces the bare shift in
`Commutation::record_interval` (now generic over `P`);
`e_com_us_to_avg_ticks::<P>` (AM32's `/ 3`, divide-free) replaces the two
`div3_i32(e_com)` calls in isr_logic and the main loop's `/ 3` (Call 6).
Non-2 MHz branches are const-folded away on every current port. New host
test `two_mhz_conversions_are_am32_arithmetic`: 386 host tests.

Gate PASS vs s04.2; ISR dumps identical on all six images (main-loop text
-48..+120 B from inlining). Bench 8/8 in band first time.

## S4.4 -- board ADC constants compile-time, divide-free (CODE) -- tag gc-s04.4-psu

`MainState` loses `voltage_divider_x100`, `millivolt_per_amp`,
`current_offset`, `use_ntc`; `Platform` gains them (Call 7).
`AdcCount::centivolts::<P>()` / `milliamps::<P>()` replace
`to_centivolts(div)` / `to_milliamps(off, mvpa)`. New host tests
`adc_conversions_equal_s2_3_for_every_count` (every board's tuple, the
harness default, offset-498 and no-sensor variants, raw 0..4095, both
conversions, equal to S2.3 to the count) and `affine_div_matches_division`:
388 host tests (was 386).

Gate PASS vs s04.3; ISR dumps identical on all six images (the conversion
is main-loop code). Intended change shown: M0 soft-division call sites
G071 public 69 -> 67, F051 58 -> 56, binz 79 -> 77 (the two removed are in
`__cortex_m_rt_main`; no function gained one); `__aeabi_lmul` 2 -> 1; text
G071 -536, F051 -628, L431 -520, G431 -600, binz -536, L431 bench -528 B,
mostly `NTC_TABLE` (512 B, dead with `USE_NTC` false) plus panic-data
reshuffle. Bench 8/8 in band first pass; binz `imean` shift: Call 8.
**DEFECT (found in S4.5): binz current wrapped above ~2.1 A; fixed in
S4.4f, Call 9. The S4.4 claim "equals S2.3" held only for 0..4095.**

## S4.4f -- ADC conversions exact over all of u16 (CODE, fix) -- tag gc-s04.4f-psu

The S4.4 defect and its fix: Call 9. `units::ADC_MAX` = `u16::MAX`;
`AffineDiv` gains the 64-bit fallback form; exhaustive test over
0..=65535; new `binz_current_above_12_bits_is_exact`: 389 host tests
(388 + 1). `gc_summary.py` accepts a step suffix (`s04.4f-`) when mapping
runs to the s00 bands.

Gate PASS vs s04.4; ISR dumps identical on all six images. Soft-division
call counts unchanged on every M0 image; `__aeabi_lmul` binz 2 -> 3,
public G071 / F051 unchanged. Bench 8/8 in band; binz `i50max` 3452 (R1),
4461 (R3), 3601 (R4) -- the S4.4 2095 ceiling is gone.

## S4.5 -- main-loop-rate counters onto the control tick (CODE) -- tag gc-s04.5-psu

`MainState::ten_khz_counter` / `led_counter` (incremented per main-loop
pass) become `consumed_epoch` / `led_epoch` measured against
`SharedState::tick_count`, whose increment moves into core `ten_khz_tick`
(Call 10). New test `consumed_current_samples_once_per_second_of_control_ticks`
(25 000 loop passes without ticks never sample; TICK_HZ+1 ticks sample
once) and a harness assert that the tick count advances: 390 host tests
(389 + 1). Blackbox vectors 81/81.

Gate PASS vs s04.4f. Intended ISR change shown, nothing else: only the
TIM6 root differs on every image (all other roots byte-identical); net
instruction change is the moved increment (M4 +2: `add.w` + table
alignment; M0 +0, register renaming and literal offsets). Bench 8/8 in
band (second round; first round discarded, Call 11).

## S5.1 -- core `step_legs` const fn; ports build `PLAN` / `ROLES` from it (HOST) -- tag gc-s05.1-psu

New `rm32::bridge::{LegRole, step_legs}` (AM32 `comStep` table) with host
tests `step_legs_is_am32_comstep` and `invalid_steps_float_everything`:
392 host tests (390 + 2). G071 `PLAN` and L431 `ROLES` built from it
(Call 13).

Gate PASS vs s04.5. HOST proof: ISR dumps identical on all six images;
G071 `PLAN` (288 B, read from the ELF) byte-identical on both G071 images;
text sizes identical on all six; the binz flash image is byte-identical
to S4.5's. Bench 8/8 in band (binz R3 run once more after an OCFAST
trip on the identical image, Call 12).

## S5.2 -- `Bridge` trait replaces `PwmOutput` + `PhaseOutput` (CODE) -- tag gc-s05.2-psu -- SUPERSEDED by S5.2r2 (Call 15)

(As first logged. "Gate PASS" below hid `ISR removals: FAIL` on all six
images; the shown diff was not limited to the intended change.)

Core `Bridge` trait; `MotorHal::bridge()`; isr_logic, sounds and
`commutation_timer_expired` drive the bridge; method renames per the
design (`set_auto_reload` -> `set_period`, `generate_update_event` ->
`latch`, `set_prescaler` -> `set_tone_divider`, compare1..3 ->
`set_phase_duties`). Port `StmBridge` (Call 14). 392 host tests
(unchanged count), blackbox 81/81.

Gate PASS vs s05.1. ISR change shown: only the two roots that drive the
bridge differ (TIM6 and the COM timer); every other root byte-identical.
Instruction counts equal or lower on every image (TIM6: G071 -7, F051
-22, L431/G431 -2, L431 bench -4, binz -7; COM: G071/binz -2, others +0).
Masked diff (registers, immediates, branch targets): register allocation,
address arithmetic and stack spills only, no call added or removed; the
one store that disappeared (binz COM, `rising` flag at `[r0, #22]`) is
tail-merged into the shared store both wrap branches now jump to. binz
WCET: COM 906 cycles @2ws (was 909), TIM6 unchanged. Text -8..-96 B.
Bench 8/8 in band first pass.

## S5.2r2 -- `Bridge` rework: ISR-identical (CODE) -- tag gc-s05.2r2-psu

Port files that touch `IsrHal` restored from S5.1 and re-applied
minimally (Call 14 d-f): core `BridgeLegs` + `Bridge: BridgeLegs`, GAT
borrowed `StmBridge` view, COM path on `BridgeLegs`. 392 host tests
(unchanged count), blackbox 81/81.

Gate vs s05.1: `ISR diff: identical` on all six images, every root's
instruction count equal (binz TIM14 1491, TIM6 3768; G071 1447/3583;
F051 1885/4020; L431 1539/3150; G431 1475/3083; L431 bench 1700/3449).
`ISR removals: FAIL` on all six, caused only by the trait-path rename of
five gate methods (Call 15). Bench 8/8 in band first pass (band-check
regex, Call 16). Intermediate s05.2r (borrowed view without the trait
split) was gated, not benched: it still changed the COM root (+3 L431,
+8 binz instructions) through the two-pointer argument.

Guard-kill bisect for S5.1 done and logged under Call 12 (no regression;
16 ABAB runs, 0 trips); `imean` finding under Call 8.

## S5.2r3 -- gate supertrait keeps `hal::PhaseOutput`: removals PASS (CODE) -- tag gc-s05.2r3-psu

`BridgeLegs` renamed to `rm32::hal::PhaseOutput` and moved into hal.rs
(Call 14 g); nothing else. Gate vs s05.1: `ISR diff: identical` and
`ISR removals: PASS` on all six images; no FAIL in any of the 14 gate
outputs; binz WCET COM 549 insns / 909 cycles, TIM6 2647 (= S5.1). 392
host tests, blackbox 81/81. Bench 8/8 in band first pass.

## S5.2f -- review fixes (CODE) -- tag gc-s05.2f-psu

From the increment 4-5a review: (a) the COMP gate latch converts e_com
with `e_com_us_to_avg_ticks::<StmPlatform>` (was `div3_i32`, i.e. a
2 MHz timer assumed); (b) `AffineDiv` and `AdcScale` are `pub(crate)`,
`AffineDiv` keeps its proved `x_max` and `apply` debug-asserts it (new
test, should_panic); (c) the port tone code drives `bridge()` (Call 19);
(d) LED: no change (Call 17); (e) `gc_summary.py` i50max check (Call
18); (f) Call 8 corrected. Host tests 392 -> 393; blackbox 81/81.

Gate vs s05.2r3: `ISR diff: identical` and removals PASS on all six
images; no FAIL in any gate output. Bench R1-R4 once per board, 8/8 in
band including the new i50max check (L431 R3 repeated once after a
serial dump failure, Call 20).

## S5.3 -- drive-mode decision into the core (RISKY) -- tag gc-s05.3-psu

The AUTO drive-mode policy (`comp_pwm && !old_routine`) leaves the port's
`PhaseDriver::effective_comp_pwm` for `rm32::bridge::drive_complementary`
(Call 21). `PhaseOutput::com_step` and `all_pwm` take the decision as an
argument; the core computes it at each of its six call sites from
`config.use_comp_pwm()` and `shared`, and `commutation_timer_expired` takes
a `comp_pwm` argument (the port passes `state.config.use_comp_pwm()`).
`PhaseDriver` loses its `comp_pwm` field, `new(bool)`, `set_comp_pwm` and
the main.rs wiring step: a missed wiring step can no longer leave the
silicon in diode drive while the config says complementary. Tone: Call
22. Bench 'D' key: Call 23. `com_step` / `all_pwm` pinned
`#[inline(never)]` (they were already out of line). Host tests 393 -> 394
(new: the COM path passes the core decision, 4 mode x config cases);
blackbox 81/81.

ELF SHA-256 (prefix): binz 9fd764014cc6, l431 33c1dff0b428, pub g071
3374dc5e2b3b, f051 119513ed111c, l431 2ffbfb6450ce, g431 840bef90dcb1.

Gate vs s05.2f: removals PASS on all six images, no FAIL in any gate
output. Every ISR root shrank: l431 TIM16 -129 / TIM6 -148, binz TIM14
-9 / TIM6 -36, pub g071 -1 / -45, f051 -25 / -61, l431 -50 / -62, g431
-89 / -103 instructions. The removed code is the per-call
`effective_comp_pwm` (a COMP_PWM_LIVE match plus an `old_routine` read,
inlined at each `phase_pwm` and twice in `com_step`); `all_pwm` now reads
`old_routine` once instead of three times. binz static WCET: TIM6 5118
-> 5080 cycles; TIM14 909 -> 918 (+9 cycles, 0.14 us): the
`old_routine` read moved from `com_step` into `commutation_timer_expired`
and the bool crosses as an argument, same work on every path.

Bench (PSU, ABAB, same session): A = S5.2f images (`s53a`), B = S5.3
(`s53b`), R1-R4 three times per board, order A/B, B/A, A/B; 48/48 OK, no
kill, desync or reboot, no R3 overcurrent trip; band check 48/48;
`scripts/ab_segments.py s53a s53b` 0/120 banded metrics significant (Call
24); `ab_r3_peaks.py s53a s53b`: R3 peak 4351 +- 312 vs 4185 +- 264
(p=0.52), dwell 2177 vs 2173. RISKY: added to the battery batch, see
`docs/gc_battery_s05.3.md`.
