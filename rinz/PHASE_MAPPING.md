# Phase mapping investigation — rinz B-G431B-ESC1

## Goal

Get genuine in-window BEMF zero crossings in all 6 commutation sectors, so that the
PLL has trustworthy per-sector timing information and the lock counter is an honest
signal.

---

## Fixed hardware facts

| Signal | Physical pin | ADC channel | DMA buf slot |
|--------|-------------|-------------|--------------|
| Drive phase A high/low | TIM1 CH1/CH1N PA8/PC13 | — | — |
| Drive phase B high/low | TIM1 CH2/CH2N PA9/PA12 | — | — |
| Drive phase C high/low | TIM1 CH3/CH3N PA10/PB15 | — | — |
| BEMF sense A | PA4 | ADC2 ch17 | buf[0] |
| BEMF sense C | PB11 | ADC2 ch14 | buf[1] |
| BEMF sense B | PC4 | ADC2 ch5 | buf[2] |

ADC scan order is [ch17, ch14, ch5] = [A, C, B], so buf[1]=C and buf[2]=B are
physically swapped relative to alphabetical order. `PHASE_TO_DMA = [0, 2, 1]`
corrects for this: logical phase idx → DMA buf slot.

`PHASE_TO_DMA` is the **single source of truth** for the logical→physical ADC mapping.
It is used in the ISR (v_hi, v_lo, v_fl reads) and in the `e` command (get_fl closure).
Changing it anywhere else (e.g. hardcoded match on fl) is a bug.

---

## What "in-window ZC" means

Sector window: ~926 µs at 180 Hz drive (1/(180×6)).
Blanking: first `ZC_BLANK=2` triggered samples ≈ 100 µs are skipped.
Detection: `ZC_CONFIRM=2` consecutive samples must both satisfy the crossing condition,
with `v_fl != 0` and `vn != 0`.

- **ZC<0**: condition already met at samples [BLANK] and [BLANK+1] — crossing happened
  before our observable window. Does NOT count as a lock hit.
- **ZC@i/n**: first confirmed crossing at sample i > BLANK+1. Counts as in-window.
- **ZC>n**: condition never satisfied in the sector. Miss.

The `i` command lock counter and the `e` command ZC annotation now use the same
definition. They should agree on any given revolution.

---

## RAMP_FALLING semantics

`RAMP_FALLING[s]` says whether the float phase is expected to be **falling** (true)
or **rising** (false) through virtual neutral in sector s.

For standard forward rotation (A→B→C):
```
s0 C-fall, s1 B-rise, s2 A-fall, s3 C-rise, s4 B-fall, s5 A-rise
→ [true, false, true, false, true, false]
```

For reverse rotation (A→C→B):
```
s0 C-rise, s1 B-fall, s2 A-rise, s3 C-fall, s4 B-rise, s5 A-fall
→ [false, true, false, true, false, true]
```

The motor appears to be running in reverse relative to our drive sequence (see
diagnosis below).

---

## Experiment log

### Baseline (lock_test1, lock_test2) — PHASE_TO_DMA=[0,2,1], RAMP_FALLING=[false,true,false,true,false,true]

Using original detection (single-sample, no v_fl!=0 check, early ZCs counted).
- s0, s1, s2: appeared LOCKED
- s3, s5: zero hits — C and A entered sector stuck far above neutral (ZC>n)
- s4: locked only at lower amplitude (amp=9%), B rose through neutral ZC@11/19

Key observation: s3/s5 float phases entered sectors already well above neutral with no
downward crossing. Root cause: the ZC for those phases was already happening before or
at the sector start.

### attempt1 — PHASE_TO_DMA=[0,1,2] (swap B↔C in ADC)

Result: **broke s4**. B now reads PB11 (physical C slot), rises from 0x21 to 0x31 but
never reaches N=0x35. With original [0,2,1], s4 was locking.

**Conclusion**: `PHASE_TO_DMA=[0,2,1]` is correct. The physical ADC wiring matches the
board's documented assignment. Swapping B↔C in the ADC mapping is wrong.

### RAMP_FALLING flip — [false,true,false,false,false,false]

Changed s3 and s5 from `true` to `false` (look for rising instead of falling).

With old (dishonest) single-sample detection: all 6 sectors showed LOCKED. But `e`
showed s3/s5 ZC<0 (trivial immediate detection — phase already above neutral).

Root cause of apparent success: s3/s5 float phases enter the sector already above
neutral, so RAMP_FALLING=false (rising) fires immediately on the first observable sample.
This is an early ZC, not a genuine mid-sector crossing.

### ZC detection honesty fix

Brought ISR detection into exact semantic parity with `e` command:
1. `v_fl != 0` guard (matches `e`'s `vb == 0` skip)
2. `ZC_CONFIRM=2` consecutive samples (was single-sample in ISR)
3. Early suppression: if both [BLANK] and [BLANK+1] are crossed → ZC<0, not counted
4. `ZC_IN_WINDOW_S` flag drives lock counter; `ZC_DETECTED_S` still suppresses re-fire
5. `ZC_PREV_CROSSED_S` holds previous-sample state for the 2-consecutive check

After fix:
- `i` and `e` agree on the same question: genuine in-window crossing this revolution?
- Early ZCs no longer inflate lock counts.

### attempt2 — same RAMP_FALLING=[false,true,false,false,false,false], honest detection

`logs/attemp2.txt`:
- s0: ZC@5-7/18-19, genuine ✓
- s1: intermittent (ZC<0 or ZC>18 depending on rev), lock stays low
- s2: ZC<0 always (A enters above neutral)
- s3: ZC<0 always (C enters above neutral)
- s4: ZC@10-12/18-19, genuine ✓
- s5: ZC<0 always (A enters far above neutral)

### attempt3 — PHASE_TO_DMA=[0,2,1], RAMP_FALLING=[false,true,false,false,false,false], honest detection

`logs/attempt3.txt`:
- s0: LOCKED, ZC@5-6/18 ✓
- s1: ~14% in-window hit rate. lock=0 (net negative per revolution)
- s2: 0 hits. zc_hits frozen at 57 from before honesty fix. ZC<0 always.
- s3: 0 hits. ZC<0 always.
- s4: LOCKED, ZC@10-12/18 ✓
- s5: 0 hits. ZC<0 always.

---

## Current diagnosis

**2 out of 6 sectors have genuine observable in-window ZCs: s0 and s4.**

s0 and s4 share: float phase enters the sector **below** neutral and rises through it.
Both have `RAMP_FALLING=false` (rising). Both are LOCKED.

The 4 failing sectors (s1, s2, s3, s5) share: float phase enters the sector already
on the crossed side of neutral, within the first ~100 µs (blanking window) or before.

**Why s0 and s4 work while the others don't:**

In s0 (A=hi, B=lo): C was driven HIGH in the previous sector (s5: C=hi, B=lo), yet
C enters s0 **below** neutral (~0x2e–0x31 = 46–49, N = 0x36 = 54). The motor's back-EMF
on C opposes the drive — BEMF is pulling C below neutral even while driven high. When C
floats in s0, it starts below N and rises through it at mid-sector. This is the signature
of the motor running in **reverse** relative to our drive sequence.

In the other sectors, the float phase enters already on the "crossed" side because the
motor's BEMF is aligned with (not opposing) the preceding drive, so the phase
transitions to the post-crossing state before we can observe it.

This is consistent with reverse rotation: in reverse, exactly every other sector pair
has the ZC in the observable window, while the interleaved sectors have their ZC at the
boundary. s0 and s4 are the two sectors where the reverse-rotation ZC naturally falls
mid-sector for this particular PHASE_TO_DMA assignment.

---

### attempt4 — reversed drive [0,0,2,2,1,1]/[2,1,1,0,0,2], RAMP_FALLING all-false

`logs/attempt4` (two `e` dumps at 130 Hz, amp=10%):

Sector float assignments with reversed drive: s0=B, s1=C, s2=A, s3=B, s4=C, s5=A.

Dump 1:
- s0 (B=float): ZC>n. B=0x23–0x2d, N=0x37. B rises but can't reach neutral.
- s1 (C=float): ZC<0. C=0x44–0x54, N=0x37. C stuck well above neutral.
- s2 (A=float): ZC<0. A=0x47–0x4c, N=0x37. A stuck above neutral.
- s3 (B=float): ZC@7/25 ✓. B rises from 0x34 through N=0x35.
- s4 (C=float): ZC<0. C=0x39–0x40, N=0x35. C above neutral.
- s5 (A=float): ZC<0. A=0x4c–0x52, N=0x37. A well above neutral.

Dump 2:
- s0 (B=float): ZC@8/26 ✓. B rises from 0x2a through N=0x37.
- s1 (C=float): ZC<0. C=0x50–0x54, N=0x37.
- s2 (A=float): ZC<0. A=0x48–0x4c, N=0x37.
- s3 (B=float): ZC<0. B=0x44–0x4b, N=0x35. B above neutral (opposite of dump1!).
- s4 (C=float): ZC<0. C=0x3d–0x47, N=0x35.
- s5 (A=float): ZC<0. A=0x4e, N=0x37.

**Result: WORSE than original.** s3 and s0 alternate between working and failing
revolution-to-revolution — neither locks. C-float and A-float always stuck above neutral.

**Root cause:** reversing the electrical drive cannot flip the motor's physical rotation
at 10% amplitude — insufficient torque. Motor keeps spinning in the same direction.
In original s0, C was special: coming off HIGH in s5, motor's counter-BEMF pulled C
below neutral during the float window. With reversed drive, C comes off LOW before s1 —
the counter-BEMF now pushes C high instead. The structural advantage of original s0 is lost.

**Reverted to original drive tables.**

---

## Current status (post attempt4)

Original drive [0,0,1,1,2,2]/[1,2,2,0,0,1] with RAMP_FALLING=[false,true,false,false,false,false]:
- s0 (C-float): LOCKED ✓
- s4 (B-float): LOCKED ✓
- s1,s2,s3,s5: ZC<0 always

The motor is spinning in reverse relative to the drive direction. Reversing the drive
does not change this at low amplitude.

---

## What has NOT been tried yet

- `ZC_BLANK` reduction below 2 (might expose transitions with fast blanking)
- Running at lower frequency where each sector window is wider and ZC timing is less critical
- Higher amplitude drive to see if motor direction reverses with stronger torque
- Phase-shifted commutation: shift the sector boundaries by 1–2 steps to see if the
  "ZC<0" sectors can be brought into window (their ZC IS happening, just before sector start)
