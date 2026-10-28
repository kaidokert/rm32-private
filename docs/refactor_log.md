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

25. **The pass bar's `dsy` column counts only fast desyncs.** The recorder
    packs `main_state.dsy_fast` into `dsy4`, which `gc_summary.py` reports
    as `dsy`. Slow-demote, current-demote and polling desyncs never reach
    it. The firmware's own `desync_events` (i-line `dsy=`) is the full
    count. On binz it accumulates from the last flash, so a run's count is
    the difference from the previous run. Every band check since S1 has
    read the fast class only. The S5.3 battery run is judged on the
    firmware counter instead. Fixed after the batch: `gc_summary.py`
    reports `dsy_fw` (last i-line `dsy=` minus first, from the run's .txt)
    and `--check` fails any increase, or a run without the count. All 324
    PSU gc captures so far have `dsy_fw` = 0, so the blind column hid
    nothing on the PSU.

26. **L431 orbit-trip wedge on a battery slam: present at
    gc-s00-baseline, so not S5.3's and not the batch's; tagged
    gc-s05.3-ok on that basis (operator to review).** On a 15 -> 100 %
    slam the L431 legitimately draws ~12.7 A while accelerating, above
    the wrong-phase-orbit line (8.5 A at 100 % duty scaled to ~11 A at
    10.6 V) for 30-40 ms. The trip needs more than 30 ms, and the
    transient exemption depends on where the 100 ms duty snapshot falls.
    So it trips on about a third of slams: replaying the recorded runs
    through the trip logic at all 100 snapshot phases gives 26-34
    tripping phases on both sides (`scripts/orbit_replay.py`), and 10 of
    30 slams this session tripped. At 100 % input the trip stops the
    motor. The restart then re-locks (4 of 10 trips, back to 3530 eHz)
    or wedges: duty kicked back to 170-400 by repeated slow-demote
    desyncs, ~850 of them, ~160 eHz until the end of the run, even after
    the drop to 15 %. Bisect on 48 kHz battery images (the L431 carrier
    since fc3818c; `scripts/gc_l431_k48_image.sh` adds it to older tags):
    the first single-trip-per-image pass pointed at S3.4, but S3.3 and
    S3.4 have identical ISR addresses and RAM layout, and S3.3 then
    wedged on its next trip. gc-s00-baseline wedged on its third trip
    (slam 6). Recovery is chance on every image. Both mechanisms are
    rm32 divergences from AM32 (the orbit trip; the restart at the 48 kHz
    carrier, which the L431 cannot start at low duty). They belong in
    the questionable pile (desync/orbit policy), not in this refactor.

27. **S5.3 battery batch: three runs repeated once.** binz A b1 (round 1)
    aborted as a reboot because the run began while the flash's boot was
    still in progress (`gc_battery.sh flash-binz` now waits 6 s). L431 B b2
    and A b2 (round 2) wrote no CSV: the recorder dump never arrived whole
    (the Call 20 transport problem). The firmware log for both was clean
    (no kill, no reboot, desyncs 0, counters like the other b2 runs).
    Their captures are kept under `captures/{binz,l431}/aborted/`.

28. **binz 5 -> 100 % slams desync and re-climb: pre-existing.** Round 2's
    b1 had 20 (A) and 37 (B) slow-demote desyncs at the slam, then held
    100 % settled. The 2026-10-07 qualification run `slam5-100` did the
    same (`dsy=12`, all slow-demote, 11 polling samples at the slam).
    `docs/gc_battery_s05.3.md` wrongly said that run had dsy=0: it read
    the fast-only column (Call 25).

29. **S6.1: one removals FAIL accepted (public G071 EXTI4_15 +1).**
    `handle_exti_frame` (DSHOT frame handling, not BEMF code) grew by one
    instruction because `BemfComparator` lost its cached step/rising (2
    bytes), shifting `IsrState` field offsets so one constant no longer
    fits a single `movs`. Proof: the same tree with 2 bytes of padding put
    back in `BemfComparator` gives EXTI4_15 2643 -> 2643 and removals PASS
    on that image. No padding committed. The other five images PASS; the
    binz bench image shrinks on every root (TIM14 -12, TIM6 -32, EXTI4_15
    -9).

30. **S6.1: removals check learned declared renames.** `change_input`
    became `route_bemf` (a core helper, out of line on purpose: a port
    `select_phase` with the phase computed at each call site cost binz
    TIM14 +2 to +20). `isr_diff.py removals --rename OLD=NEW`
    (`GC_RENAME` in `gc_gate.sh`) accepts the new name only when the old
    one left the same tree; every work check still applies.

31. **S6.1 bench ran on the battery, not the PSU.** I misread the supply
    (a stiff pack: 54 mOhm fit, like the PSU's) and flew the PSU images.
    The battery's higher bus put 6/8 runs 1-3 % over the PSU current
    bands. Same-supply controls with the previous step's images show the
    same offset: binz R1/R4 (s05.3c) 50 % 1573/1625 mA, out of band too;
    L431 R4 s05.3c PSU image hit its own 4.5 A OC guard at 80 %. So the
    comparison was redone on the battery with battery-guard images, L431
    R4 ABAB (`batt61a` = gc-s05.3-psu, `batt61b` = 42aa6c3): 80 % 4341 /
    4341 vs 4348 / 4328 mA, eHz within 0.5 %, no kill, dsy_fw = 0 on all.
    The PSU-band check for S6.1 is therefore not done; the S6.1 PSU runs
    can be repeated when the PSU is back (Call for the operator).

32. **S6.2: binz R3 slam peak -- open, not resolved.** The guard EWMA
    peak in the first 0.5 s after the 10 -> 60 % slam is bimodal per run
    (~4.5 A or ~5.07 A). Pooled over every interleaved run on 2026-10-26
    (battery): A (gc-s06.1-psu) 4608 +- 192 mA, high mode 1/12; F (S6.2f)
    4781 +- 220, 7/17; mean p = 0.024, mode rate Fisher p = 0.09. A first
    n = 7 interleaved set gave p = 0.010 (predeclared bar, fired); the
    last in-session triplets gave A 4726 vs F 4752 (p = 0.86). Ruled out
    by bench bisect: TIM6 cost (F + 51-cycle pad, t6mean 2457 = A's: still
    4792, 2/5 high) and the latch moment (X = F + the old end-of-tick
    latch: 4919, 2/5 high). Not explained: no remaining S6.2 hunk has a
    mechanism; code-placement sensitivity of the slam transient is the
    open hypothesis (alignment checked, no pattern). The high mode is
    above the PSU OCFAST 4.8 A, so on the PSU it would show as R3 trips
    (Call 2 measured ~1 in 11 at S4-S5). Resolving the rate needs ~25
    slams a side. S6.3's ABAB tracks the R3 high-mode rate. Operator call:
    run the rate test now, or carry it.

33. **S6.2 imean +95 mA is the Call 8 artifact, now tested directly.**
    F's TIM6 is ~51 cycles cheaper (t6mean 2455 -> 2404, R3); restoring
    the cycles as a pure busy-wait (F+pad) drops imean 834 -> 760 with no
    other change. `i50max` is main-loop sampled as well (`main.rs:1052`).
    The per-ms guard sample is the mean of the 20 kHz scans since the
    previous main-loop 1 kHz dispatch (`mcu_g071/adc.rs::raw_current`), so
    every main-loop-rate change moves these r-line numbers.

34. **S6.2 latch placement.** First version (3cc6ee2) latched at the top
    of the tick and fed the ramp limit from it: the ramp then read e_com
    before `bemf_polling` could update it in the same tick (an ordering
    change), and LLVM outlined `bemf_polling` on the L431 (+1 `bl`).
    S6.2f (38373b7) reads it at the ramp site as before and latches the
    gate from that value; the one-tick AllOff return skips a latch. Its
    48-run ABAB (`captures/*/abab1/`) is kept but superseded.

35. **S6.3: F051 and G431 gain the COMP gate.** They acked the EXTI line
    at entry and evaluated every edge; every COMP vector now runs the
    core gate (`handle_comp_irq`). G431 clears and checks both comparator
    lines (as its ack did) and re-raises the active one. Public F051
    ADC_COMP +71, G431 COMP1_2_3 +65 (the gate itself). Not benched (no
    board); qualified at their bring-up, as the plan records.

36. **S6.3: the cold-latch fallback is gone.** The L431 and G071 wrappers
    used a fresh e_com/3 when `gate_avg` read 0. AM32 reads
    `average_interval` with no fallback; the tick latches from boot, and
    while the latch is 0 so, nearly always, is e_com/3. On M4 the
    fallback cost 13 shift-adds in the root (`div3_i32`).

37. **S6.3: binz camp counter x2.0-2.2, imean -77; behaviour flat.** Every
    binz R3/R4 run doubled `camps` (e.g. 12594 -> 27238) with fewer
    `comp_entries`. The old G071 wrapper counted a camp only when a
    second, later comparator read was still post-ZC (two out-of-line
    `comp_at_pre_zc_level` calls); S6.3 counts each gate Camp decision,
    and a camp re-entry is about half as long, so a camp window holds
    about twice the re-entries. Hypothesis, not tested: count camp
    windows (first entry per window) on both images. imean -77 is the
    Call 33 main-loop sampling artifact. Segments, slam peak (-193,
    p=0.20) and dwell do not move.

38. **S7.1: `unpack`'s flag is `bidirectional`, not `edt`.** The design
    (5.4) writes `dshot::unpack(packet, edt)`; the flag the decoder
    actually takes selects bidirectional DShot's inverted CRC (EDT is a
    telemetry feature negotiated by command, not a framing mode). Named
    after what it does; `unpack` returns `DshotFrame` (its `BadCrc`
    variant), as `decode_frame` does, instead of a `Result`.

39. **S7.2 is removals, not byte-identical.** The plan classes S7.2
    "disassembly identical". `TransferState::process` is out of line, so
    `cpu_mhz` reached it as a runtime argument and the detection
    prescalers (`/6`, `-1`) were computed per frame; as `P::CAPTURE_HZ`
    they fold. binz `process` -8, L431 -16 instructions, L431
    `handle_exti_frame` -2 (one argument fewer); every other function
    identical, two declared renames (`receive_dshot_dma` ->
    `start_capture`, `send_dshot_dma` -> `start_gcr_output`).

40. **S7.3: public F051 EXTI4_15 +2 (register allocation).** After
    restructuring for the bench images (binz -25, L431 -40, public G071
    -17, L431/G431 -3), public F051 `handle_exti_frame` is 2 instructions
    longer: stores +9 / moves -9 and branch layout, no new atomic,
    barrier, PRIMASK or call. Frame ISR (priority 2) on an unbenched
    board; accepted like Call 29 rather than chasing layout on a parked
    branch. Reviewer may ask for another pass at merge time.

41. **S7.3 carries two unreachable paths, unchanged.**
    `CommandResult::ProgrammingCommit` only comes from
    `CommandProcessor::process_programming`, which nothing calls (firmware
    or harness), so the dispatch's commit arm never runs; and
    `isr_handlers::handle_crsf_byte` (now `CrsfParser::dispatch_byte`)
    has no caller in any build. Both are moved as they were; wiring
    either is a behaviour change for a later step.

42. **S7.3 EDT gate order.** The throttle arm now computes the gate,
    clears `edt_armed` on zero, then stores `newinput`; before, it stored
    and then cleared. Same result (the gate reads `edt_armed` before the
    clear); with the old order LLVM tail-duplicated the store on G071
    (one more PRIMASK site). Host test `dispatch_edt_arm_gate`.

43. **S7.3 / S7.4 harness coverage.** S7.3's harness now runs the core
    dispatch, so the drift is fixed by construction (config-diff writes,
    EDT init/deinit, calibration writes). No blackbox vector exercises
    those paths yet (the vector language cannot read the config-write
    queue); host tests cover them instead. S7.4's tone, bidir response
    and CRSF are not modelled by the harness at all (no tone output, no
    telemetry slot, no CRSF input), so the harness is unchanged there;
    the design's "firmware and harness both call it" holds for S7.3
    only.
44. **FC bench EDT restored at `dshot_edt = ON`; cause was FC config
    (2026-10-27, SUPERSEDES the first version of this Call).** The first
    version blamed BF #14490 (9458ac043): `dshot_decode_telemetry_value`
    decodes typed frames only once `telemetryTypes` has an extended bit.
    That reading missed the platform bootstrap: the STM32 DShot drivers
    set the STATE_EVENTS bit when the ESC answers cmd 13 with 0x0E00
    (`platform/STM32/dshot_bitbang.c:578`,
    `platform/common/stm32/pwm_output_dshot_shared.c:262`), so stock ON
    works whenever cmd 13 goes out. The real cause: `feature ESC_SENSOR`,
    left on since the 2026-08-02 KISS loop test. `core.c:587` sends the
    arm-time cmd 13 only if ESC_SENSOR is OFF, and with it on
    MSP_MOTOR_TELEMETRY's T/V/C come from the (now unwired) serial
    sensor. That also explains the 07-31 "ON works" record in
    bf_phase.md, which predates the feature. Bisect on the stock BF
    build (known-good, unmodified 57d872ac1 hex) + rm32 f0625ca, ON,
    MSP arm-fly at 25 %, read via `dshot_telemetry_info`: ESC_SENSOR on
    `R----` (x2), off `RTVCS` (x2: L431 36-37 C 12.00 V, binz 27-28 C
    12.25 V), A/B/A order. FC now: ESC_SENSOR off, `dshot_edt = ON`,
    `dshot_bidir = ON` (captures/gc/issue7/bf_diff_after_edt_bisect.txt).
    Known-good tag first, ESC side: L431 flashed with
    pre-main-merge-20260816 (EDT validated 07-31/08-01): ESC_SENSOR off
    `RTVCS` (36 C, 12.00 V), on `R----` -- same split, so the ESC
    commit is not the variable. L431 back on f0625ca: off `RTVCS`.
    No BF code change needed. KISS-serial ESC telemetry and EDT are
    mutually exclusive in BF by design: re-enable ESC_SENSOR only for a
    KISS session.
45. **Desync check skips the polling -> interrupt changeover (f0625ca,
    2026-10-27).** A divergence from AM32, forced by rm32 polling BEMF on
    the 20 kHz tick instead of AM32's main-loop rate: polled commutations
    run late, so the first interrupt-mode revolution is ~35 % faster and
    `|last - avg| > avg/2` tripped at zc 24 on ~50 % of L431 25 % starts
    (restart loop, ~500 starts per hold). Stock AM32 fd4d45f on the same
    L431/motor/FC: 8/8 clean. Refuted first: carrier (24 kHz chokes),
    direction, sine start, the refactor (gc-s00 chokes), duty (400 =
    AM32's low-rpm ceiling at every check). The check compares only once
    two revolutions ran fully in interrupt mode: one-pass guard 5/8,
    two-pass 8/8 (75.5-76.1k eRPM). Retire it if polling ever moves to
    main-loop rate.
46. **Issue #7 met on devel d30c3d3 (2026-10-27).** bf_ladder x3 +
    bf_slam x3 on each board, same commit, same motor via relay,
    alternating boards: 12/12 PASS (starts=1, dsy=0, 0 in-test resets).
    binz ~2.5 % faster at the top (226.4k vs 220.9k eRPM); unexplained,
    power-stage candidates only (dead time 405 vs 560 ns). Report:
    https://claude.ai/artifact/HhU21C4nWpHjVqyPZuSgyU. Open, outside the
    test windows: binz came up after 2/6 FC reboots with 0 % decode
    (~500 CRC fails, stale first interval ~62,350) until one
    signal-timeout reset -- the one-edge misalignment class. L431 SWD
    needs `--speed 100` since the 2026-10-27 power cycle.
47. **S8.2 ABAB: binz A3 ladder re-run after a mid-run input loss
    (2026-10-27).** On the A image (s08.1 battery, c6f858e -- 6/6 clean
    in S8.1's ABAB) the ESC armed and started on the first 10 % rung,
    then every DShot frame failed CRC (264) with the stale first interval
    ~62,340 followed by clean 37/69 edges -- the one-edge capture
    misalignment of Call 46's boot case, now seen mid-run -- until
    signal timeout (0.51 s) reset the chip, which then could not re-arm
    under nonzero throttle (`run: None`, 3 resets). Motor side clean
    (dsy=0, orbit=0). Not S8.2: A image, and no input-path code changed
    in S8. Re-ran A3 on the same image (A3r PASS) to complete the
    ABAB; the aborted run is kept (captures/gc/s08.2/bench/
    binz_A3_ladder.log). Open: the G071 capture never realigns once
    misaligned while frames keep arriving -- a fix belongs to the input
    path (re-arm on a CRC-fail run), its own step after S8.
    **Operator: accepted (2026-10-27).**
48. **S9.3: `sequre_g431` does not build (pre-existing, 2026-10-27).**
    build.rs panics `Unknown G431 COMP1 INM pin: PA5`, identically at
    gc-s08.2-ok (checked in a worktree). Not a missing table entry: on the
    G431 COMP1's INM inputs are PA4/PA0 only (PA5 routes to COMP2), so the
    yaml's `phase_a: { comp: 1, inm: PA5 }` describes impossible wiring,
    and AM32 has no G431 Sequre target to take the real pins from. Left
    as is (yaml says `tuning: am32`); fixing it needs the board's
    schematic. The other 12 non-rig boards build under `Am32Tuning`.
    **Resolved (operator: mirror AM32):** AM32 does have it --
    `SEQURE_G431` uses `HARDWARE_GROUP_G4_D` (my "no G431 Sequre target"
    above was wrong; I grepped the wrong name). G4_D puts phase A on COMP2
    (INM PA5, INP PA3) and B/C on COMP1 (PA4/PA1, PA0/PA1); the yaml had
    A and B's comparators swapped -- an original copy mistake. Yaml now
    mirrors G4_D; `sequre_g431 am32 34084 1756 1136`. Still unmirrored
    in that yaml vs AM32 (not build blockers): ADC channels (AM32 V=ch3,
    I=ch4; yaml 13/5, copied from protondrive), divider 480, 5 mV/A, 8 MHz
    HSE; and the G431 port hard-codes protondrive-style bridge pins (G4_D
    low sides are PB15/14/13). Not bench-relevant until G431 bring-up.
49. **S9.3: design §5.8's `type State` not added.** The rig counters
    (orbit trip count / duty snapshot / snapshot age, 6 bytes) stay in
    `MainState`. Moving them into a per-policy state type would change
    the rigs' `MainState` layout and code, and S9 is HOST only because
    the rig images stay byte-identical (they do: S9.1-S9.3 ELFs
    ae2941d6 / 6d7b3494). With `Am32Tuning` the counters are dead
    stores the compiler keeps: 6 bytes of RAM, no behaviour. Revisit if
    the rig policies go public (#6).
    **Operator: accepted (2026-10-27).**

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

## S5.3 battery batch -- tag gc-s05.3-ok (on 6fbd0a7)

3S battery, both boards on one pack, `docs/gc_battery_s05.3.md`: A =
gc-s05.2f-psu (`batt53a`), B = gc-s05.3-psu (`batt53b`), battery-guard
images, rounds A/B, B/A, A/B, binz b1-b3 then L431 b1-b3, one run per
command; 36 runs plus three repeats (Call 27).

- Every run completed: no guard kill, one boot, 100 % reached and held
  except one L431 b2 run on each side.
- 100 %: binz settled eHz within -0.5 / -0.2 / -0.2 % of A and mA within
  -1.0 / -1.2 / -1.1 % (b1 / b2 / b3, n = 3 a side); L431 b1 and b3
  within 0.3 %.
- `ab_segments.py batt53a batt53b`: 99 metrics, 0 significant after
  Bonferroni, 2 at raw p < 0.05 (~5 expected).
- The bar as written was not met, on either side: L431 B b2 (round 2)
  orbit-tripped at the slam and wedged (848 desyncs); A did the same in a
  follow-up (`orb53a-s5`, 860). Bisect: present at gc-s00-baseline (Call
  26). binz b1 desyncs on both sides, also pre-existing (Call 28). The
  bar's dsy column was blind to all of these (Call 25).

Verdict: no S5.3 difference on battery; the failures are pre-existing and
logged as Calls 25, 26 and 28 for the operator. Bisect images (L431, 48 kHz, battery):
battb00k48 878475ea34f8, bk-s03.1 f928153b3209, bk-s03.3 8119f935b7b1,
bk-s03.4 afa1d2c84e8c, bk-s04.1 679aa19a749e.

## S6.1 -- `BemfSensor` trait; `BemfPins` to the port (CODE) -- tag gc-s06.1-psu (42aa6c3)

`hal::Comparator` -> `hal::BemfSensor` (design 5.2): `select_phase(Phase,
rising)`, `enable_irq`, `mask_irq`, `level`, `filter_read`. The core owns
the step -> floating-phase map (`hal::floating_phase`, branch-free table;
AM32 `changeCompInput`: 1/4 C, 2/5 A, 3/6 B) and routes through one
out-of-line `route_bemf`. The port comparator no longer caches
step/rising. `BemfPins` moved to `rm32_stm32::board_ext`. `clear_pending`
/ `repend` deferred to S6.3, where `on_bemf_irq` first needs them. New
host tests: floating-phase table, commutation selects the new step's
phase (rm32 361 pass).

Gate (`GC_RENAME=change_input=route_bemf`) PASS vs s05.3c; committed tree
re-gated as s06.1c, ISR dumps identical to the gated tree. Removals PASS
on 5/6 images, public G071 EXTI4_15 +1 accepted (Call 29). binz WCET:
TIM14 918 -> 897 cyc, TIM6 5080 -> 4995, ADC_COMP 698 unchanged. Bench
ELFs: l431 a4aef5add519, binz 5040f9c691ad.

Bench (on the battery, Call 31): R1-R4 both boards, all complete, no kill,
dsy_fw = 0; band check 2/8 (current 1-3 % high, battery bus); controls
with s05.3c images show the same offset; L431 R4 battery-image ABAB 2 a
side equal within 0.5 %. Verdict: no S6.1 difference.

## S6.2 -- gate average latched by the core control tick (RISKY) -- tag gc-s06.2-psu (38373b7)

3cc6ee2 + 38373b7 (fix-up, Call 34). `ten_khz_tick` reads the average
interval at the ramp-limit site (where it always was) and latches it into
`SharedState::gate_avg` (`IsrTiming::gate_avg`, a `core` AtomicU32: a
`portable_atomic` load is PRIMASK-wrapped on M0 and put a critical section
on the priority-0 COMP path). L431 / G071 COMP ISRs read it;
`rm32_stm32::comp_gate` deleted. AM32 staleness kept (20 kHz latch, never
fresh e_com/3, clamped at 0 only). New host test: tick latches; the
AllOff tick keeps the previous value.

Gate PASS vs s06.1c, removals only on all six images, no FAIL (binz TIM6
-13, ADC_COMP -1; L431 TIM6 -17, COMP -2). binz WCET from s06.2f gate.

Bench (battery, battery-guard images; A = gc-s06.1-psu `batt62a`, B =
38373b7 `batt62f`): ABAB R1-R4 x 3 rounds per board, 48 runs, all OK, no
kill, dsy_fw = 0; `ab_segments.py batt62a batt62f` 0 significant after
Bonferroni (3 raw < 0.05, ~6 expected). Plus 4 extra R3 pairs, a pad
experiment (5 + 5) and a bisect triplet (5 x 3): Calls 32, 33.
Battery: covered by these runs (the pack was on for the whole step).
Verdict: step criteria met; R3 slam high-mode rate open (Call 32).

## S6.3 -- COMP acceptance gate into the core (RISKY) -- tag gc-s06.3-psu (2385fc0)

`rm32::control::isr_logic::on_bemf_irq` is AM32's `COMP_IRQHandler`
gate (pending -> open: clear, `Open`; closed at the pre-ZC level: clear,
re-read, `repend` if it flipped; closed post-ZC: camp). The port's
shared `isr_handlers::handle_comp_irq` runs `handle_comp` on `Open` and
keeps the bench counters; an `entered` hook runs the entry counter before
the gate, where the old wrappers counted it. `BemfSensor` gains
`pending` / `clear_pending` / `repend` (ExtiOps per MCU); every COMP
vector is one call. New host test covers every gate branch (363 pass).

Gate (`GC_RENAME=comp_at_pre_zc_level=handle_comp_irq`) PASS vs s06.2f;
committed tree re-gated as s06.3c, identical. Removals: binz ADC_COMP
-36, L431 COMP -14, L431 TIM6 -17; F051 / G431 add the gate (Call 35).
**WCET gate on the COMP root: binz ADC_COMP 694 -> 680 cyc @2ws (219 ->
185 insns, AM32 ratio 1.67 -> 1.41).** On the way: counting after the
gate made LLVM copy the counter into every path (+9, extra ldrex); the
cold fallback cost 13 instructions on M4 (Call 36).

Bench (battery, battery-guard images; A = gc-s06.2-psu `batt63a`, B =
2385fc0 `batt63b`): ABAB R1-R4 x 3 rounds per board, 48 runs, all OK, no
kill, dsy_fw = 0. One L431 B R3 recorder dump lost in transport (log
clean) and one A R2 that collided with a second run I sent in the same
message: both set aside under `captures/l431/aborted/` and repeated.
`ab_segments.py batt63a batt63b` 0 significant, 0 at raw p < 0.05. R3
slam peak 4785 vs 4592 (p=0.20), high mode 1/3 vs 0/3 (Call 32 data).
Camps x2 (Call 37). Battery: covered by these runs.
Verdict: no S6.3 behaviour change on L431 / G071.

## Increment 6 -- summary for review

S6.1 gc-s06.1-psu (42aa6c3), S6.2 gc-s06.2-psu (38373b7), S6.3
gc-s06.3-psu (2385fc0). Every root on both bench images is smaller than
at gc-s05.3-psu. All bench work ran on the battery (Call 31): the RISKY
steps' battery batch is therefore done, but **no step of increment 6 has
a PSU band check**; S6.1-S6.3 PSU R1-R4 can be flown in one session when
the PSU is back. Calls for review: 29-37 (32 is the open one: binz R3
slam high-mode rate).

## Bands rebuilt on the battery-parallel supply (2026-10-26)

The bench supply is now a 3S2P pack wired in parallel with the PSU (the
PSU gives ~2 A at full speed, the pack the rest and recharges between
runs; no sag). The operator keeps it. Currents read 1-3 % above the PSU
bands (Call 31), so the battery gets its own: `docs/gc_bands_batt.json`,
from the 16 `batt63b` runs (gc-s06.3-psu battery image, 4 interleaved
R1-R4 rounds per board, all dsy = 0, no kills), v4 recipe plus a binz mA
floor: `--ma-floor l431=30 --ma-floor binz=30 --min-settled 1.0`. The
binz floor is new: at 10 % its settled mA scatters sd ~10 mA (absolute,
72 R3 runs over four A/B-equal images), so +-5 % (~+-9 mA) of 4 tight
runs missed equal-code runs by a few mA; 30 = 3 sd, the L431's value.
`gc_summary.py` maps `battNN*-` tags to the `s00-` band keys
(`band_key`, also used when building).

Checks: self 32/32; every other battery run today passes (batt63a 24/24,
batt62f 38/38, batt62a 33/33, batt61a/b 4/4); falsifier: the PSU s00
runs fail 20/32 (59 segments out). `gc_bands_s00.json` is unchanged and
still passes its own runs. From here, step checks on this bench use
`--check docs/gc_bands_batt.json`; this closes the Call 31 question.

## Increment 6 review (2026-10-26) -- approved, tag gc-s06.3-ok

Peer review approved S6.1-S6.3 and the battery bands. Dispositions:
29, 30, 33, 34, 35, 36 approved as written; 31 closed by 823a030 (no
bare-PSU rerun, the battery-parallel supply is the bench); **32 carried**
(no rate test: pad and latch bisects ruled out timing and placement, S6.3
came back 4785 vs 4592, 0/3 high; reviewer's reading: the UART step
command is asynchronous to the 1 kHz guard window, ramp divider and
sector phase, so the inrush peak's alignment with the fast EWMA varies;
both modes have margin under the 12 A surge guard on this supply); 37
approved without the experiment (one level read per Camp, ADC_COMP 36
instructions shorter, so ~2x re-entries per camp window, gate opening
unchanged).
Follow-up (next touch of isr_logic.rs): the `bemf_zero_cross` doc
comment still describes per-port gates/acks; point it at `on_bemf_irq`.

## S7.1 -- DShot bit layer (HOST) -- tag gc-s07.1-psu (9b5ba9f)

`dshot::unpack(packet, bidirectional)` and `dshot::gcr_bits(value)`:
layer 1 for bit-slicing ports (Call 38). `encode_gcr_frame` calls
`gcr_bits` (inline(always)); no caller changes. Exhaustive host tests:
`unpack == decode_frame` on all 65536 packets in both CRC modes;
`gcr_bits` inverts `encode_gcr_frame` on all 4096 values. Also the
`bemf_zero_cross` doc comment now points at `on_bemf_irq` (S6 review
note). Gate PASS vs s06.3c; ISR disassembly identical on all six images
(committed tree re-gated as s07.1c, identical); host 365, blackbox 81 +
C known diffs. HOST class: no bench.

## S7.2 -- CAPTURE_HZ, GCR_SHIFT to Platform; DMA names out (HOST) -- tag gc-s07.2-psu (cdcf20e)

`Platform::CAPTURE_HZ` / `GCR_SHIFT` (StmPlatform: CPU clock,
`Chip::GCR_SHIFT`; harness 64 MHz / 6, the values its vectors were
recorded with). `TransferState::process<P>` drops `cpu_mhz`; the
detection prescalers are const fns of `P::CAPTURE_HZ` (same values on
every chip: 48/64/80/170 MHz -> 8/10/13/28 and MHz-1). `detect_input`
drops its unused clock argument. `GCR_SHIFT_F0/G0` and the F0-only
`encode_telemetry` wrapper leave the core. Renames:
`CaptureConfig.ndtr` -> `edges_per_burst`, core `dma_buffer` params ->
`edges`, `InputCapture::{receive_dshot_dma, send_dshot_dma, dma_buffer}`
-> `{start_capture, start_gcr_output, edges}`, `TelemetryUart::send_dma`
-> `send` (PAC register names stay). Gate PASS vs s07.1c with declared
renames, removals only (Call 39); committed tree s07.2c identical; host
365, blackbox 81 + C known diffs. HOST class: no bench.

## S7.3 -- transfer dispatch into the core (CODE) -- branch gc-s7-dshot (02ae2f5)

`rm32::transfer::dispatch(actions, shared, InputContext, on_command,
set_prescaler)`: the one `TransferAction` dispatch; firmware
`handle_exti_frame` and the harness both call it (the harness had
drifted, Call 43). The port's bench counters and the output-prescaler
register write are hooks run where the inline code ran them. Gate PASS vs
s07.2c: binz EXTI4_15 -25, L431 EXTI15_10 -40, public G071 -17, public
L431/G431 -3, public F051 +2 (Call 40); committed tree s07.3c identical.
Host 370 (five dispatch tests), blackbox 81 + C known diffs. Calls 41, 42.

## S7.4 -- tone, bidir response, CRSF into the core (CODE) -- branch gc-s7-dshot (027b2cc)

`rm32::tone::step` (+ `tone_duty`, AM32 `setVolume`), `rm32::edt::
encode_response`, `CrsfParser::dispatch_byte`. Kept to the old codegen
shape: bridge made only on a writing path, config and eRPM inputs read
inside the arms that use them (first cut: binz TIM6 +4, DMA +2, L431 DMA
+5, all from hoisted loads). Gate PASS vs s07.3c: binz TIM6 -6, L431 TIM6
-3, every other root identical; committed tree s07.4c identical. Host
373, blackbox 81 + C known diffs.

**S7.3-S7.4 are parked on `gc-s7-dshot`** (pushed to private, not merged):
they change the DShot path, which the bench builds do not exercise. Merge
after the FC session: DShot150/300/600, bidir, EDT and Configurator
passthrough on the L431 (`protocol_matrix.py`). Only that session remains
for increment 7.

Operator decisions (2026-10-26): Call 39 accepted (S7.2 judged
removals-only, not byte-identical: the compile-time fold is the step's
point). Call 40 accepted (public F051 +2 on the parked branch; revisit at
merge if the reviewer asks). Increment 7a complete; only the FC session
remains.

## Increment 7a review (2026-10-26) -- S7.1-S7.2 approved, tag gc-s07.2-ok

The review's S7.1/S7.2 audit matches the code and gate numbers: approved,
Calls 38 and 39 approved. Its S7.3/S7.4 sections and its rulings on
Calls 41-43 describe code that exists on neither branch
(`on_capture_complete`, `DshotTelemetrySource`, `service_dshot_telemetry`,
`process_dshot_command`, `dshot_edt_enabled`, "decode_frame delegates to
unpack" -- all absent, checked with git grep on devel and gc-s7-dshot), so
they do not count as a review of the real S7.3/S7.4 or of the real Calls
41 (unreachable paths carried), 42 (EDT gate order), 43 (harness
coverage). Call 40's verdict stands (operator-accepted) but its stated
cause is wrong: the +2 is block layout in `handle_exti_frame`.
**Open: re-review S7.3 (02ae2f5), S7.4 (027b2cc) and Calls 40-43 against
gc-s7-dshot before the merge.**

## S7.3/S7.4 re-review (2026-10-27) -- APPROVED

Re-review against `git diff gc-s07.2-psu..gc-s7-dshot` (02ae2f5, 027b2cc)
matches the code (all 8 cited tests exist on the branch). S7.3, S7.4 and
Calls 40-43 approved; Call 40 needs no layout pass at merge. Merge gate
left: the L431 FC session (`scripts/protocol_matrix.py`).

## S7 FC session & merge (2026-10-27) -- gate baseline s07.4m, tag gc-s07.4-ok

Merged gc-s7-dshot -> devel d30c3d3 (protocol matrix 5/5 legs on f0625ca),
issue #7 12/12 (Call 46, docs/issue7/), EDT restored at ON (Call 44).
Review of d30c3d3..ff9ebe0 (2026-10-27): approved, with fixes, all in this
entry's commit:

- **refactor_log restored.** 83d2c16's Call 44 rewrite sliced to the
  first "45." it found -- at the file's end -- and deleted S4.1 through
  the S7.3/S7.4 re-review (453 lines). Rebuilt from afc236a with Calls
  44-46 together in the numbered list.
- **f0625ca broke 3 blackbox vectors** (desync_detection,
  desync_recovery, desync_recovery_bidir): the changeover count started
  at 0, i.e. "changeover pending", so established interrupt-mode runs
  could not desync for 3 passes. Merged red: the commit hook runs no
  vectors, and my harness checks rebuilt `-p rm32`, not `-p rm32-harness`.
  Fix: the count starts settled (`CHANGEOVER_SETTLED = 3`), a stop or any
  polling tick resets it to 0 (review item: no stale count across a
  stop). A rotor that never polled is checked exactly as AM32. New vector
  `desync_changeover_guard`: one polling tick, then AM32's ZC-12 desync
  is held and fires at the ZC-18 pass; C-known-diff (AM32 fires at 12).
  Rust 82/82.
- **Bench fields gated:** `dsy_snap`, `dsy_hist`, `chk_hist` and their
  per-revolution stores are `#[cfg(feature = "bench-diag")]`;
  `chk_since_polling` stays (control state).
- **Gate s07.4m vs s07.4c: PASS.** Public ISR dumps identical on all four
  chips. Bench images differ by additions only: binz TIM14 +24
  instructions, cpsid 18 -> 20 (1109ca7's `EXC_COMMS` counter under
  debuguart; M0 atomics are critical sections), so the binz removals
  sub-check reports FAIL by design. s07.4m is the S8.1 baseline.
- **Bench re-qual (battery images of this tree):** L431 25 % starts 8/8
  clean; binz 1/1. Ladder PASS both boards (L431 1.844M, binz 1.896M
  comms). Slam: binz PASS; L431 3/4 PASS -- slam 1 had dsy=2 from one
  orbit trip (`orbit=1 fast=1`, `first=0/0/0/0`: the |delta avg| detector
  never fired). The orbit path is untouched here; 1/4 vs f0625ca 0/3 is
  not separable at this n. Open, watch in S8.

## S8.1 -- SharedComm split into groups, no default bodies (CODE) -- branch gc/s08

Design §5.5. `shared_comm.rs` now has six groups with no default
bodies: `MotorStatus` (mode, commutation timing, direction, changeover
and desync handoffs), `Throttle`, `Drive` (tim1_arr, filter level, min
BEMF counts, auto advance), `Protections` (duty ceilings), `Telemetry`
(requests, interval counter, EDT measurements), `Commands` (ISR actions,
settings flag, control-tick counters). `SharedComm` is their blanket
union. The four mode predicates (`armed`/`running`/`old_routine`/
`stepper_sine`) are pure derivations of `motor_mode` and live in the
blanket-implemented `MotorStatusExt`, so nothing can override them.

- **Defaults removed.** Every implementation now states every value.
  `SharedState` already implemented all 75 methods, so its bodies moved
  verbatim (scripted regroup refuses any dropped or invented method).
  `TestShared` had leaned on 20 defaults: the mode setters and `_isr0`
  variants are now written out on `Cell` (same logic the defaults had),
  and the seven silently-constant ones (`stall_protection_adjust`,
  `current_limit_adjust`, `battery_voltage`, `actual_current`,
  `degrees_celsius`, `save_settings_flag`, `send_esc_info_flag`) are real
  `Cell` fields. `TestShared` is `#[cfg(test)]` now (tests only).
- **tim1_arr from the bridge base period.** New `Platform::PWM_BASE_PERIOD`
  (no default, per the trait's rule); `SharedState::new::<P>()` and
  `TestShared` initialise `tim1_arr` from it, not 1999. STM32 port:
  `Chip::TIM1_AUTORELOAD` (L431 3332, G071 2665, F051 1999, G431 7082).
  Harness and test platforms: 1999 (the vectors' F051-class period).
  No behaviour change on the firmware: `main.rs` publishes
  `timer1_max_arr` before the ISRs start; the initial value only stops
  being a wrong one.
- **Gate s08.1 vs s07.4m: PASS** (Rust vectors 82/82; C harness 11 =
  the 11 known diffs). ISR dumps identical on all six images. The
  removals sub-check first reported FAIL on binz/pub G071/pub F051
  TIM14: three "new functions" that are the `_isr0` impls under the
  renamed trait (`MotorState` -> `MotorStatus` in the mangled name,
  +0 instructions). Re-run with the declared rename
  `GC_RENAME='shared_comm..MotorState$GT$=shared_comm..MotorStatus$GT$'`:
  PASS on all six. ELF SHA-256: l431 ed25ecf9..., binz ccc4b4ba...
  (differ from s07.4m by symbol names and the tim1_arr `.data` word).
- **Bench ABAB (battery∥PSU, FC/BF DSHOT300), one run per command,
  A = s07.4m battery images, B = c6f858e battery images:** L431 and binz
  each A1 B1 A2 B2 A3 B3, bf_ladder + bf_slam per flash: **24/24 PASS**,
  every run starts=1 dsy=0 resets=0, **orbit trips 0 in all 12 slams**.
  L431 ladder comms A 1.836/1.841/1.845M, B 1.838/1.846/1.844M; slam A
  349k/328k/349k, B 348k/349k/350k. binz ladder A 1.899/1.901/1.901M, B
  1.900/1.900/1.901M; slam A 361k/361k/362k, B 360k/361k/362k. Logs
  captures/gc/s08.1/bench/. (The s07.4m L431 slam-1 orbit trip did not
  recur in 6 more L431 slams on either image.) Tag gc-s08.1-ok.

## S8.2 -- `SingleWriter` token, `unsafe` `_isr0` (RISKY) -- branch gc/s08

Design §5.5, review #4.

- **Core.** `transition_isr0`, `set_old_routine_isr0` and
  `increment_zero_crosses_isr0` are `unsafe fn` with the precondition in
  a `# Safety` section (no context writing `motor_mode`/`zero_crosses`
  between the read and the write; every other writer uses an update a
  preempting store invalidates). New `ModeWriter` trait with two
  implementations: `AtomicWriter` (safe, the atomic RMW versions -- the
  default for any port) and `SingleWriter`, a zero-sized, non-`Clone`
  token whose only constructor is `unsafe fn new()`; its `ModeWriter`
  methods are the one place the core calls the `_isr0` stores.
  `commutation_timer_expired` takes `writer: &W` (`W: ModeWriter`) and
  makes its three mode/zc updates through it: monomorphised, no runtime
  choice.
- **STM32 port.** `init::single_writer()` carries the Cortex-M argument,
  the only place it holds: single core, one NVIC; the commutation timer
  is at the top preemption level of the writers (COMP shares it and
  cannot preempt); M4 exception entry/return clears the exclusive
  monitor so a preempted STREX retries; M0 atomics are critical
  sections; the DShot DMA ISR that L431 `adjust_irq_priorities` may lift
  above the commutation timer writes neither word (audited:
  `handle_dma_tc` only reads). Each chip's `init` mints the token after
  its NVIC priority block (G071/L431 explicit levels; F051/G431 leave
  every IRQ at 0, no nesting at all) and returns it in `InitResult`;
  main moves it into `IsrState.writer`; `handle_tim14` passes
  `&state.writer`. The harness mints one under a single-threaded SAFETY
  note (it keeps exercising the plain stores); core tests use
  `AtomicWriter`.
- **Gate s08.2 vs s08.1: PASS** (Rust 82/82; C harness = 11 known
  diffs). ISR dumps DIFFERENT on all six images, removals PASS on all
  six, +0 instructions on every root. The diff in every case is only the
  commutation ISR's stack-argument slots for `commutation_timer_expired`
  moving by 4 bytes (`str r3,[sp,#4]` -> `[sp,#8]` ... on binz TIM14,
  `strd r6,r4,[sp]` -> `[sp,#4]` ... on L431 TIM1_UP_TIM16): `&writer`
  occupies one more argument slot. No added call, barrier, exclusive,
  mask or constant. ELF SHA-256: l431 cb35de27..., binz 265bc258....
- **Bench ABAB (battery∥PSU, FC/BF DSHOT300), one run per command,
  A = s08.1 battery images (c6f858e), B = 4adc84e battery images:** L431
  and binz each A1 B1 A2 B2 A3 B3, bf_ladder + bf_slam per flash.
  **B side 12/12 PASS**, starts=1 dsy=0 resets=0, orbit trips 0 in all
  six B slams. A side 12/12 PASS after one re-run (Call 47: binz A3
  ladder lost the DShot input to a one-edge capture misalignment on
  the A image; A3r PASS); orbit trips 0 in all six A slams. L431 ladder
  comms A 1.846/1.850/1.848M, B 1.849/1.847/1.847M; slam A
  350k/349k/350k, B 350k/350k/348k. binz ladder A 1.900/1.902/1.903M,
  B 1.901/1.902/1.904M; slam A 362k/361k/362k, B 362k/362k/362k. This
  is S8.2's battery run. Logs captures/gc/s08.2/bench/. Tag gc-s08.2-ok.

## Increment 8 review (2026-10-27) -- APPROVED, tags gc-s08.1-ok, gc-s08.2-ok

S8.1, S8.2 and Call 47 approved. Answers: the group boundaries stand
(desync/changeover handoffs in `MotorStatus`, ISR actions and tick
counters in `Commands`); the `SingleWriter` argument is complete on all
four chips (equal priorities never preempt; M0 RMWs mask, M4 exception
entry clears the monitor; L431 DMA swap is read-only on both words); the
harness keeps `SingleWriter` so the vectors exercise the firmware's
plain stores while core tests cover `AtomicWriter`; `TestShared` is
test-only. Nit fixed in this commit: duplicate `#[cfg(test)]` on
`control::shared_impl`. Next: Increment 9 (S9.1-S9.3, `TuningPolicy`).

## S9.1 -- `TuningPolicy`, `QualifiedRigTuning` = today's constants (HOST) -- branch gc/s09

Design §5.8. New `rm32/src/tuning.rs`: `TuningPolicy` with associated
consts `ORBIT_TRIP: Option<OrbitTripConfig>`, `RECLIMB:
Option<ReclimbConfig>`, `DESYNC: DesyncPolicy` (`Am32` |
`HalfKickStayInterrupt { max_ci_us, rearm_holdoff_zc }`);
`QualifiedRigTuning` holds the eight constants that left `constants.rs`
(orbit slope 3, offset 2500 mA, 30 ms, cal 816 cV, transient 150;
reclimb 1500 ZC / cap 800; stay-interrupt 300 us, re-arm holdoff 100 ZC);
`Am32Tuning` is all None / `Am32`. `orbit_current::<T>` moved with them
(false without an orbit trip). `Platform` gains `type Tuning` (no
default); `Timing::DESYNC_STAY_INTERRUPT_CI` derives from the policy (0
under `Am32`). `main_state.rs` reads every rig constant through
`P::Tuning`: the orbit trip block sits under `if let Some(orbit)`, the
reclimb clamp under `if let Some(reclimb)`, the fast-rotor branch under
`match DESYNC`.

- **Rigs select it.** Board yamls gain a required `tuning: rig | am32`
  key (build.rs, no serde default; generates `board::Tuning`). All 15
  boards say `rig` in this step -- today's behaviour; S9.3 moves the
  non-rig boards to `am32`. Harness and test platforms:
  `QualifiedRigTuning`.
- **Deferred to S9.3:** the design's `type State` (rig counters into a
  per-policy struct). It only matters once a build selects `Am32Tuning`
  and the counters' RAM should vanish; S9.1 stays byte-identical
  without it.
- **Gate s09.1 vs s08.2: PASS** (Rust 82/82; C harness = 11 known
  diffs). ISR dumps identical on all six images; text/data/bss sizes
  identical. Flash images (objcopy -O binary) differ in exactly 3 bytes
  on each of the six: the line field of three panic `Location` records
  for `rm32_stm32/src/isr.rs` (161/164/170, were 160/163/169), shifted by
  the one `type Tuning` line added to `IsrHal`'s `Platform` impl. No
  code byte changed. First gate run failed clippy (`collapsible_if` on
  the reclimb `if let`); collapsed to a let-chain. ELF SHA-256: l431
  ae2941d6..., binz 6d7b3494.... Tag gc-s09.1-ok.

## S9.2 -- harness holds both tunings; vectors declare theirs (HOST) -- branch gc/s09

Design §5.8. `rm32-harness`: `HarnessPlatform<T>` / `MockMotorHal<T>` /
`Harness<T>` are generic over `T: TuningPolicy`; `main` holds
`AnyHarness { Rig(Box<Harness<QualifiedRigTuning>>), Am32(Box<Harness<Am32Tuning>>) }`
and dispatches each protocol line to the generic `command::<T>()`. New
protocol command `tuning rig|am32` replaces the harness with a fresh one
under that tuning (replies `ok`); the harness starts under `rig` for the
API sanity tests. The core stays fully compile-time.

- **Vectors declare their tuning.** Every vector carries exactly one
  `@tuning rig|am32` line; `harness.read_tuning` refuses a vector with
  none, two, or an unknown name. Classification by running all 76
  vectors under both: 74 pass under AM32 tuning -> `am32`; 2 pass only
  under the rig tuning -> `rig`: `desync_detection` (tick 13
  old_routine=1 under AM32) and `desync_changeover_guard` (tick 19
  running=0), both asserting the stay-interrupt desync response. All 76
  pass under `rig`.
- **C harness: am32 vectors only.** `rig` vectors skip on the C harness
  (no AM32 counterpart); their two `c_known_diffs.txt` entries are gone,
  9 remain. The runner refuses to send a `rig` vector to the C harness.
- **Falsifier.** New API test `test_tuning_selects_policy`: runs
  `desync_detection` under its declared `rig` (passes), then forced to
  `am32` and requires the assertion failure -- the switch is not a
  no-op. Rust suite 83 (82 + this test), all green.
- Harness unit tests (`mod tests`) pin `Harness<QualifiedRigTuning>`
  (first gate run: host tests did not compile without the annotation).
- **Gate s09.2 vs s09.1: PASS.** Rust 83/83; C 9 failures = the 9 known,
  3 skipped (2 rig vectors + the tuning API test). ISR dumps identical on
  all six; bench ELFs byte-identical to s09.1 (SHA-256 l431 ae2941d6...,
  binz 6d7b3494...). Firmware untouched. Tag gc-s09.2-ok.

## S9.3 -- `Am32Tuning` for the non-rig boards; size report (HOST) -- branch gc/s09

The 13 non-rig board yamls say `tuning: am32`; `neutron_l431` (the L431
bench image) and `binz_drv8304h_g071` keep `rig`. New
`scripts/board_sizes.sh` builds each named board's production image
(no bench features) and prints its sizes with the yaml's tuning. Before
(all `rig`, s09.2 tree) vs after:

  | board | text rig | text am32 | delta | data | bss |
  |---|---|---|---|---|---|
  | flycolor_g071 | 37580 | 37076 | -504 | 1760 | 1140 |
  | gen_64k_g071 | 39172 | 38660 | -512 | 1760 | 1140 |
  | iflight_12s_g071 | 37604 | 37068 | -536 | 1760 | 1140 |
  | iflight_f051 | 36536 | 36008 | -528 | 1752 | 1140 |
  | mamba_f55_g071 | 37580 | 37076 | -504 | 1760 | 1140 |
  | protondrive_g431 | 34708 | 34084 | -624 | 1756 | 1136 |
  | sequre_g431 | (did not build) | 34084 | -- | 1756 | 1136 |
  | pwm_enable_g071 | 38348 | 37804 | -544 | 1760 | 1140 |
  | siskin_f051 | 36512 | 35992 | -520 | 1752 | 1140 |
  | tbs_12s_g071 | 37604 | 37068 | -536 | 1760 | 1140 |
  | tbs_4in1_g071 | 37580 | 37076 | -504 | 1760 | 1140 |
  | vimdrones_l431 | 36036 | 35492 | -544 | 1752 | 1140 |
  | wraith32v2_f051 | 36512 | 35992 | -520 | 1752 | 1140 |

Text shrinks 504-624 bytes per board (the orbit trip, reclimb clamp and
fast-rotor desync branch compile out of the main loop); data and bss
unchanged (Call 49). `sequre_g431` does not build on either side (Call
48).

- **Gate s09.3 vs s09.2: PASS** (Rust 83/83; C 9 known, 3 skipped). ISR
  dumps identical on all six images, including the three public images
  that moved to `am32` (G071 gen_64k, F051 siskin, G431 protondrive):
  the rig code is main-loop only, and the ISR's `DutyKickHalf` arm stays
  (the action arrives as a runtime enum value; under `Am32Tuning`
  nothing requests it). Public L431 (neutron, `rig`) unchanged. Rig ELFs
  byte-identical to s09.2 (l431 ae2941d6..., binz 6d7b3494...). Tag
  gc-s09.3-ok.

## S10.1 -- RISC-V cross-build + STM32-name gate (HOST) -- branch gc/s10

Design §9 item 2.

- **`rm32` builds for `riscv32imc-unknown-none-elf`** with no source
  change. The core already used portable-atomic; riscv32imc has no CAS,
  so (like thumbv6m, where rm32_stm32 selects `critical-section`) the
  port picks the backend. The gate builds it with
  `--no-default-features --features portable-atomic/unsafe-assume-single-core`
  (the public surface; the default `bench-diag` feature also builds).
- **`scripts/stm32_name_gate.py`**: scans `rm32/src/**/*.rs`; an STM32
  name (numbered timer/COMP/EXTI/DMA/UART/ADC, GPIO bank or pin, NVIC,
  RCC, chip family, ...) may appear only in a comment whose comment block
  mentions AM32. Built-in self-test runs first: a fixture with three
  planted violations (code, string literal, uncited comment) and three
  allowed citations must flag exactly lines {1,2,3} or the gate exits 2.
  (The first fixture put the uncited line in the same comment block as a
  citing one; the self-test failed and I fixed the fixture -- block
  scoping is the intended rule.) Also falsified live: a planted
  `let _ = "TIM6";` in tone.rs failed the gate (rc=1).
- **Violations fixed: 37 -> 0.** 3 in code: the `units.rs` test board
  `L431` renamed `Neutron` (the board, not the chip). 34 in comments,
  reworded to roles (`TIM1` -> PWM timer, `TIM6` -> control tick,
  `TIM14/TIM16` -> commutation timer, `EXTI SWIER` -> software-trigger
  register, chip names -> clock rates or "AM32's F051/G071 values"), or
  pointed at the AM32 name they mirror (`buffersize`, `DEAD_TIME`,
  `WS2812_PIN`). 21 AM32-citing comment mentions remain, allowed.
- Both checks are now gate lines ("rm32 riscv32imc build", "rm32
  STM32-name gate").
- **Gate s10.1 vs s09.3: PASS.** ISR dumps identical on all six. Rig ELF
  hashes changed (l431 71476a4b..., binz 38543e84...) but the flashed
  bytes are identical (`objcopy -O binary` + `cmp`: 0 bytes differ on
  both): comment edits move debug-info line/column records only. Tag
  gc-s10.1-ok.

## S10.2 -- time-based vectors (HOST) -- branch gc/s10

Design §9 item 1, first half.

- **Syntax.** A `ticks` operand is either a bare integer (a tick count:
  event steps -- frames, pulses, one state-machine step) or a duration
  `<n>us|ms|s` (decimals allowed). `harness.ticks_for` converts a
  duration at the harness's tick rate with exact rational arithmetic and
  refuses a duration that is not a whole, positive tick count (e.g.
  `25us` at 20 kHz). Rust harness: new `rates` command
  (`tick_hz=... timer_hz=...` from its Platform); C harness: AM32's
  `LOOP_FREQUENCY_HZ` 20000 (Inc/targets.h).
- **Vectors converted: 22 lines in 16 files**, every `ticks` count that
  is a duration of something AM32 schedules in time: arming
  (19999 -> 999.95ms), the three signal timeouts (500.1ms, 600ms,
  2100ms), telemetry interval (29.95ms, 30ms), LVC/ADC accumulation, the
  1 kHz PID / speed / stall / temperature / EDT paths, slow ramp. Kept as
  tick counts: `bidir_autodetect` 110 (frames per tick), and
  `servo_calibration` 51/76 (pulses), and all 2-5-tick steps (boundary
  steps and single state-machine advances). At 20 kHz each converted
  line is the identical tick count, so results cannot move.
- **Tests:** `test_duration_operands` (exact conversions at 20 and 40 kHz,
  five refusals). Rust suite 84 (82 vectors + 2 API tests), all green; C
  9 known diffs, unchanged.
- **Gate s10.2 vs s10.1: PASS.** ISR dumps identical on all six; rig ELFs
  byte-identical to s10.1 (71476a4b..., 38543e84...). Tag gc-s10.2-ok.

## S10.3 -- rm32-sim port; all vectors run on it (HOST + one core fix) -- branch gc/s10

Design §9 item 1, second half.

- **`rm32-sim`** (new crate, no_std lib): `SimPlatform<T>` with a 40 kHz
  control tick, 1 MHz interval timer, 10-bit 3.0 V ADC and packet-level
  DShot; AM32-default board constants like the harness, so only those
  four axes differ. ADC model: `raw_for_centivolts` / `raw_for_milliamps`
  return the count nearest a value through the core's own conversion
  (unit tests: 12.00 V -> 372 counts reads 11.99 V; one count ~3.2 cV;
  full-scale current 14.99 A).
- **Harness generic over a `Port`** (`Platform` + CPU_MHZ, ADC injection,
  DSHOT_PACKETS). The STM32 model keeps its exact old values (literal
  1999 / 64 MHz / 4095-3630 now come from the port). New protocol
  command `port stm32|sim`; `rates` reports `port= tick_hz= timer_hz=`;
  the runner selects with `RM32_PORT=sim`; gate line "blackbox vectors
  (rm32-sim)".
- **Hidden assumptions found, and fixed:**
  1. *ADC fixed at 12 bit / 3.3 V* (`units.rs` consts). New `Platform`
     consts `ADC_FULL_SCALE`, `ADC_VREF_MV`; the Q16 step and AM32's
     `3300/41` derive from them (`VREF*100/FULL` truncated = 80, the same
     constant, so the rigs fold identically). Exhaustive every-count
     equivalence tests still pass for every board.
  2. *DShot only through timestamp capture.* `transfer::process` was the
     only entry; `dshot::unpack` existed but nothing dispatched it. New
     `TransferState::process_packet` (layer-1 packets, detection = two
     consecutive valid-CRC packets). First version extracted `process`'s
     shared steps into `#[inline(always)]` helpers: EXTI15_10 grew +27
     instructions (LLVM rescheduled), so `process` keeps its exact
     source and only `process_packet` uses the helpers. The duplicate
     logic is pinned by `packet_path_matches_capture_path`: 20k
     pseudo-random frames/states through both paths (identical actions
     and state), plus deterministic phases asserted to reach stick-cal
     entry and the bidir commit (the first coverage bar failed: random
     states never reached either -- fixed the scenario, not the bar).
  3. *Signal-timeout counter u16* (compile error on the sim: 2 s at
     40 kHz = 80000 ticks). AM32 has the same `uint16_t signaltimeout`
     vs `LOOP_FREQUENCY_HZ << 1` (Src/main.c:587, :2390), capping its
     loop at ~32 kHz. Widened to u32 (`SharedState`, `TestShared`,
     `MotorStatus::signal_timeout`, `Timing::SIGNAL_TIMEOUT_*`). Call 50.
- **Vector-side assumptions (timer counts):** 3 vectors failed on the sim
  because they wrote interval-timer quantities as raw 2 MHz counts:
  `motor_restart` (commutation_interval 10000, interval_timer_count
  5000), `sine_changeover` (9000), `variable_pwm_modes` (96/100/200).
  The runner now accepts durations on timer keys (`TIMER_KEYS`:
  commutation_interval, average_interval, last_average_interval,
  interval_timer_count), converted at the harness's `timer_hz`, refused
  on any other key or off-grid; those values are now 5ms/2.5ms, 4.5ms,
  48/50/100us. The other 121 raw timer values pass on both ports (their
  checks do not depend on the timer's scale) and stay raw.
- **Result: all vectors green on rm32-sim** (84 incl. the 2 API tests),
  STM32 model 84/84, C 9 known. Falsifier: corrupting every sim packet
  (`^ 1`) fails 8 of the 9 DShot vectors; restored, 84/84.
- **Not varied (design §9 lists four axes):** capture clock (64 MHz,
  servo edges), CPU_MHZ (64; AM32's CPU couplings: BEMF bad-count
  threshold, variable PWM mode 2, tones), PWM base period (1999), and
  the mock interval timer still advances one count per control tick on
  both ports (vectors drive it).
- **Gate s10.3 vs s10.2: PASS**, removals sub-check FAIL on l431,
  pub_l431, pub_g431 -- explained here, Call 50. ISR diffs, all from
  item 3: every other root is offset shifts only (SharedState fields
  after `signal_timeout` move +4 bytes: e.g. L431 COMP `ldr [r3,#60]`
  -> `#64`), same instruction count. Control tick (TIM6): the counter's
  `ldrexh/strexh` became `ldrex/strex` and the 0xFFFF saturation compare
  dropped (L431 3234 -> 3233, binz 3681 -> 3679, G431 likewise); the
  removals tool counts mnemonics, so the swap reads as "ldrex 5 -> 6".
  No new atomic operation. binz WCET TIM6 2562 -> 2561 insns, 4950 ->
  4942 cyc@2ws. Sizes: rig text l431 -64, binz +8 B; public -8..-24 B.
  Rig ELFs (re-gated on the committed tree after the fmt hook moved
  one isr.rs line; the first gate's images differed from it in 3 panic
  line bytes): l431 dd6b3ba8..., binz b60276b0... Tag gc-s10.3-ok.

50. **S10.3: widened signal-timeout counter changes rig firmware code,
    not benched (2026-10-27).** Increment 10 is host only and the goal
    asks byte-identical rig ELFs or an explanation; this one is
    explained above (offset shifts, halfword -> word exclusive on the
    counter, -1/-2 instructions in the control tick). The alternative
    -- keep u16 and forbid ports above 32 kHz -- would leave the sim
    unbuildable at design §9's 40 kHz. Proposed: fly bf_ladder +
    bf_slam on both rigs from gc-s10.3-ok at the next bench session.
