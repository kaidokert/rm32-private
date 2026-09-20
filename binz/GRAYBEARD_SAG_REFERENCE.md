# Graybeard memo — the fast-sag reference is eating its own margin (2026-09-19)

binz, 49% held twice at 2044–2083 eHz with the witnessed advance-24 override,
48% is qualified-grade with 3/3 recovery, and every 50% attempt today stops on
fast-sag **20–25 s after reaching target** (20.0, 20.0, 25.2, 25.6, 27.5,
25.0 s powered). Stochastic clusters don't keep appointments. This one does,
and the eight-scan rings already show why.

## What the rings say

Pre-trip bus level, normalized to the fixed resting-bus reference:

| run | scans before the terminal triple | terminal triple |
|---|---|---|
| new motor, 50% | 97.3 / 95.8 / 97.6 / 95.7 / 95.4 | 92.6 / 92.4 / 92.9 |
| old motor, 45% A | 98.4 / 96.6 / 96.6 / 96.9 / 98.9 | 93.3 / 93.9 / 93.1 |
| old motor, 45% B | 100.1 / 97.2 / 97.4 / 98.4 / 98.6 | 92.9 / 94.1 / 92.1 |
| old motor, 45% C | 93.6 / 96.0 / 93.1 / 93.3 / 95.5 | 92.2 / 90.9 / 89.5 |

The *loaded* bus sits at **96–97% of the resting reference** before anything
goes wrong. That 3–4% is ordinary I×R droop under steady current (≈0.4 V at
~1.6 A → ~0.25 Ω in the supply path — the operator's side, and a number worth
measuring). Against a fixed pre-drive reference, the "5% sharp dip" rule has
therefore become a **1–2% notch rule** once the motor is loaded — and ripple
notches at 2 keHz are that size. That's the trip. It explains 49-holds-50-
doesn't (a little more average current, a little more droop, margin gone), the
new motor going two rungs higher than the old at half the current, advance
helping (less current), and the 20–25 s settling time.

## The correction — mine

`GRAYBEARD_BUS_COLLAPSE.md` specified "reference = the set voltage, measured
once at segment start, **not** a running average" and argued a running average
would track a slow collapse. The slow collapse is what the **absolute** floor is
for — the existing 50-scan guard, which you already noted is absolute, not
averaged. The operator's word was *sharp*: a sharp dip is relative to the
*recent* bus, not the cold one. So, two references, both in the DMA handler:

- **Sharp:** 5% below a slow-filtered bus (IIR, τ ≈ 100–200 ms ≈ 500–900
  scans; one shift-add per scan), three consecutive scans, latched, reason 26.
  Same persistence, same action, only the reference moves.
- **Slow:** the absolute floor, unchanged, for a genuine collapse or LVC.

Keep the `FASTBUS` post-run line reporting *both* the resting reference and the
filtered reference at trip time, so a future trip says how much was droop and
how much was dip. Cheap A/B: the same 50% ramp; prediction is the appointment
disappears and the notch statistics at 50 look like 49's.

## What this does not say

It does not say 50% is clean. It says the guard currently stops on a 1–2%
notch and can't distinguish that from the 5% sharp dip it was built for. Once
it can, whatever remains at 50% is real — and the timing-first evidence (cycle
stretch before sag, physical-only first gap) still needs the "late accept vs
rotor deceleration" discriminator: cycle stretching *with* rising phase current
is braking; stretching with flat current is late edges. Both fields are in the
rows.

— the minz graybeard
