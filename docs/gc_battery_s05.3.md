# Battery run for S5.3 (operator's next step)

S5.3 (`gc-s05.3-psu`, 6fbd0a7) moved the drive-mode decision into the
core. It touches the commutation path, so the plan classes it RISKY: it
becomes `gc-s05.3-ok` only after this battery run passes. It is the first
and only step in the battery batch so far.

## What changed, what didn't

- Behaviour is meant to be identical: same AUTO policy (complementary in
  interrupt mode, diode while polling), now in
  `rm32::bridge::drive_complementary`.
- PSU proof already done: 48/48 runs OK, both boards, R1-R4 x3 interleaved
  against S5.2f; 0 of 120 banded metrics differ (refactor_log S5.3).
- What the PSU cannot show: 80-100 % on binz, 100 % slams, battery-level
  current. That is what this run covers.

## Images

Built with `scripts/gc_battery_images.sh <tag> <dir>` (BENCH_SUPPLY=battery:
binz guard BINZ_BATTERY 8 A / 15 A surge / 17.5 % sag; L431 battery
limits). The A to B change in these battery images is the same as on the PSU
(isr_diff removals PASS; binz TIM14 -10 / TIM6 -34, L431 TIM16 -129 /
TIM6 -148 instructions).

| Side | Tag | Dir | L431 SHA-256 | binz SHA-256 |
|---|---|---|---|---|
| A (control) | gc-s05.2f-psu | `captures/gc/batt53a` | `dbb2cdb6b1b0...` | `48342700bee8...` |
| B (S5.3) | gc-s05.3-psu | `captures/gc/batt53b` | `0d2a434d5b8f...` | `0b6ccbacdc7a...` |

`captures/` is gitignored: rebuild with the script if the files are gone
(builds are reproducible from the tag; the hashes above are from
2026-10-26).

## Setup

3S battery on the shared bus in place of the 5 A PSU; relay COM19 as
usual (the scripts route the motor). Same motor and prop as the PSU runs.

## Profiles (`scripts/gc_battery.sh <dir> <board> <bN>`)

| Run | binz | L431 |
|---|---|---|
| b1 | slam 10 -> 5 -> **100** -> 10 % (the 2026-10-07 `slam5-100` qualification) | ladder 15 -> 100 %, 3 s rungs, 10 s at 100 % |
| b2 | slam 10 -> **100** -> 10 % (`slam15a-100`) | slam 15 -> **100** -> 15 % |
| b3 | ladder to **100 %, held 15 s**, back to 10 % | ladder to **100 %, held 15 s** |

## Order (one run per command, strictly sequential)

Three rounds, alternating which side goes first (A/B, B/A, A/B), each
round = binz b1-b3 then L431 b1-b3 for that side:

```
bash scripts/gc_battery.sh batt53a flash-binz
bash scripts/gc_battery.sh batt53a binz b1   # then b2, b3
bash scripts/gc_battery.sh batt53a l431 b1   # then b2, b3
bash scripts/gc_battery.sh batt53b flash-binz
bash scripts/gc_battery.sh batt53b binz b1   # ... and so on
```

36 runs, about 45 minutes. A Claude session can drive it the same way as
the PSU rounds once the battery is connected.

## Pass bar (decided before the run)

1. **Every B run**: completes OK, no guard kill, `dsy=0`, one boot (no
   reboot). Any B failure means bisecting S5.3 on the battery.
2. **Every A run** must pass the same bar. An A failure is not an S5.3
   result: it means an earlier PSU-only step broke something at battery
   power. Stop, then bisect the batch from `gc-s00-baseline`.
3. **Settled metrics**: `python scripts/ab_segments.py batt53a batt53b`
   (from `rm32_stm32/`) shows 0 significant after Bonferroni. One or two
   raw-p hits are chance (expect ~5 % of the metrics).
4. **100 %**: binz reaches and holds 100 % in b1-b3, as it did in the
   2026-10-07 qualification (`dsy=0`, no kill); settled 100 % eHz and mA
   within 5 % of the A mean.

Pass gives `gc-s05.3-ok` on 6fbd0a7 and the step log gets a battery entry.

## Not covered

- F051 / G431: no bench; build- and ISR-gated only.
- Production (DSHOT input, no benchuart) builds: the 'D' bench override
  is absent there; the core's decision is used directly.
