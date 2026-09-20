# Graybeard memo — the corners at 1 kHz: current, recovery, CPU (2026-09-15)

binz, E622→E632 is the best stretch of the campaign: 334 → 1002 eHz in a day
once the single-cycle floor became report-only, the FeedbackStale wall removed
by publishing feedback from the DMA handler (E624 — the right fix, not a
timestamp refresh), and 20% qualified 3/3 at ~925 eHz. Three corners are now
in view. Two of them are arithmetic, and arithmetic is cheaper than cohorts.

## 1. `Current5` is PWM ripple peak, and it scales with ON-time by physics

The per-sample phase check is `848..=3248` raw (`powered_guard.rs:65`), i.e.
±1200 counts about 2048. With the DRV8304 CSA at 10 V/V × 7 mΩ = **70 mV/A**
and 0.806 mV/count, that is a **±13.8 A instantaneous** limit on a low-side
shunt sampled ~8 kHz, asynchronous to the 20 kHz carrier. Random sampling
against the carrier means the 30 s maximum *is* the ripple peak at the end of
the ON pulse. Read your own peaks that way:

| duty | ON pulse (20 kHz) | raw peak − 2048 | ≈ A peak |
|---|---|---|---|
| 7% | 3.5 µs | ~350 | 4.0 |
| 20% | 10 µs | ~760 | 8.8 |
| 22% | 11 µs | ~838 | 9.6 |
| fault (E625/E628) | 11.5–12.5 µs | 1250–1560 | 14–18 |

I_pk ≈ V_bus · t_on / L → with 11.7 V that's **L ≈ 10 µH**, a normal small
motor, and the line is straight. It predicts the 3248 ceiling at **~24%**,
which is exactly where E625 (after ACK 230, before 250) and E628 (22%, marginal
at 7 s) died. This is not a fault and not a code path: **every duty above ~23%
will trip this check on this motor at this bus voltage, forever**, no matter
what the controller does. Stock AM32 ran 30% on this board with no per-sample
guard at all and the PSU reading 130 mA-class.

What the graybeard would do — the *decision* is the operator's, the *facts*
are these:

- **Calibrate the channel before touching the number.** The DC offset is
  characterized (E617–619); the gain is not. One point is enough: a static
  aligned two-phase hold at a known duty with the operator reading the PSU —
  phase current ≈ I_psu / duty (the OFF interval recirculates through the low
  FETs, so the PSU only supplies the ON fraction) versus the mean of the sink
  phase's samples over the same seconds. Counts per ampere falls out.
- **Split the two meanings the one number is carrying.** A *per-sample* peak
  clamp is a FET/driver SOA guard: set it from the CSD88584 pulsed rating and
  the CSA's own linear range (VREF/2 ÷ 70 mV/A ≈ 23 A — the sensor rails
  there, so anything above ~20 A is unreadable anyway). The *operator's*
  protection spec — >10% PSU drop = kill, and a current ceiling — is an
  **average** quantity: mean over a commutation or the 1 ms feedback window,
  which you already sum. AM32's limiter is an average for the same reason.
  The DRV8304's VDS overcurrent (nFAULT → PA5, strapped default) remains the
  hardware backstop under both.
- **Do not raise 3248 without the calibration.** A ratcheted raw number is the
  E547/E574/E601 pattern again, only with amps this time.

## 2. Recovery at speed: don't arm from the fresh edge — schedule from the estimate

E629 states the wall exactly: 20 µs confirmation + 32 µs arm > 41.5 µs
remaining window at 1 kHz. That arithmetic is only binding because recovery
tries to **arm the commutation inside the remaining window of the edge it just
detected**. The reference does not do that. After a qualified ZC, AM32 (and
minz-core's clone) writes the COM timer for **ZC + interval/2 − advance from
the interval estimate**, and if the next ZC is missed the COM timer free-runs
at 1.0×T. At 1 kHz the seed interval already tells you the next commutation is
~83 µs after the edge — the deadline is T/2, not "what's left of this window",
and the 32 µs floor stops mattering. The 12-interval seed you already collect
is the estimate; use it for the first scheduled commutation and let the
following ZC refine it. minz's own note from the FALCON days: *"free-run-at-
1.0×T + ZC-refine scheduling (AM32 semantics) — a 1.5×T 'fallback' compounded
lag instead."* The E630–632 staging (CCRs loaded before sensing, prepared-off
state revalidated at admission) is the right groundwork for it; the missing
piece is the scheduling rule, not more latency shaving — you've retired four
micro-variants (E616, E621, E595) with no gain, which is what that means.

## 3. The CPU wall is arithmetic, and it lands before 30%

Numbers you've already logged: IRQ union **72.9% at 925 eHz, 75.0% at 1002**.
Per event: COMP ~40 µs, COM ~45 µs, DMA handler **38–52 µs max per scan at a
128 µs scan period** (E625–628), guard tick 100 µs period. At 1002 eHz the
commutation is 166 µs. Budget at your 30% target (≈1.3 kHz, ~128 µs
commutations, if the duty→speed line holds):

| ISR | cost | per 128 µs | share |
|---|---|---|---|
| COMP + COM | 85 µs / commutation | 85 | 66% |
| DMA (scan @ 8 kHz) | ~40 µs mean-ish / 128 µs | 40 | 31% |
| guard tick | ~10–20 µs / 100 µs | 13–26 | 10–20% |

That's >100% before the foreground gets a cycle. **30% does not fit in the
ISR bodies you have.** It's not a mystery to instrument; it's a budget to cut.
Levers in order of yield:

1. **DMA handler.** It now carries the guard validation and two `__aeabi_uidiv`
   (E624 audit: `0x800204a`, `0x8002146`, "cost NOT yet measured"). Measure
   it; remove the divides (precompute, bounded mul-shift, or move the
   bus/VREF conversion to the foreground where it was); and ask whether 8 kHz
   scanning is needed when the feedback freshness limit is 1 ms — a 4 kHz scan
   halves the biggest single line in the table with no guard change.
2. **COMP body: 40 µs vs a reference-class ~5–10 µs.** The persistence pass is
   ~3 µs. The other ~35 µs is adapter — recorder, tail, stats, timeline,
   guard bracket. This is the parity gap that matters now; the qualification
   build's COMP should approach the reference's, with aggregation (counters,
   min/max) in place of per-event structures.
3. **COM body: 45 µs vs ~10–15 µs** for the same reason.

Do the DMA handler first: it's one function, it has known soft divides in it,
and it's ~30% of the core at speed.

## Smaller notes

- The bridge-off coast corroboration (E626–628) is fine as a sanity check of
  rotor speed; its Nyquist limit at 8 kHz is real, and AM32's estimator and
  yours agree within a few percent on the same rotor, which is the
  corroboration that counts. Don't build a faster coast recorder.
- E632's "prepared-off state revalidated before guard admission" is exactly
  the discipline the reference lacks and you're right to keep.

— the minz graybeard
