# Graybeard memo — the reference is on the bench; put the microscope down (2026-09-14)

binz, three things happened today that change what the next hundred entries
should look like. In order of weight:

1. **Stock AM32 runs this rig to 741 eHz on known settings** (E530: 331 / 586 /
   741 eHz at 10 / 20 / 25%, forward, ENABLE-low after). The inherited-settings
   runs went to 831 with the current limiter *on*. Your ceiling is 342. The
   hardware, the 47 k star, the capless dividers, the PSU — none of it is the
   limit, and none of it goes back on the table. The graybeard's earlier
   "hardware door" was wrong in exactly the direction the operator said.
2. **E478 falsified the preemption mechanism** I proposed in the ENVELOPE
   addendum: you masked the reference persistence loop and the 7.2% fault
   happened anyway. That door is closed. Good — that's what the bench is for.
3. **E452–E510 (~60 entries) built a per-event recorder against a 2 µs / 4 µs
   cost gate and never got under it.** That's the pattern this memo is about.

## The one field, mechanically

With your UART bypass in `setInput`, `bi_direction=1` has one effect left on
the run path (`main.c:819-823`):

```c
if (eepromBuffer.bi_direction) polling_mode_changeover = POLLING_MODE_THRESHOLD / 2;  // 1000 ticks
else                            polling_mode_changeover = POLLING_MODE_THRESHOLD;      // 2000 ticks
```

That is the commutation interval (0.5 µs ticks) below which AM32 hands off from
**polling mode** (20 kHz tick BEMF check) to **interrupt mode** (comparator
EXTI + the 12-read persistence loop), `main.c:2034`; it falls back to polling
above `changeover + 500` (`:908`). Default: hand off at 2000 ticks ≈ 1 ms ≈
**167 eHz**. Halved: 1000 ticks ≈ **333 eHz**.

So on this rig, stock AM32 handed to the comparator-interrupt path at 167 eHz
**does not start** (E527: zc 2, ~21 eHz). Handed to it at 333 eHz it **runs to
741**. Same firmware, same comparator, same loop. And your ceiling — 342 eHz ≈
975 ticks — sits right on that boundary, while your own
`POLLING_MODE_CHANGEOVER` is the default 2000 and your qualified band lives in
interrupt mode from 167 up. Three coincident numbers are not a cause. They are
the most specific lead this campaign has produced, and it came from the
reference, not from a recorder.

Two one-variable tests, no instrument required:

- `bi_direction=0` with `polling_mode_changeover=1000` hard-set at line 822. If
  10% starts and runs like E530, the field is fully explained by the changeover.
  (`main.c:2422` is the only other `bi_direction` read on the run path — confirm
  it's inert under your input bypass.)
- Sweep changeover 2000 → 1500 → 1200 → 1000 on stock AM32. Where it starts
  succeeding is **the rig's low-speed comparator-interrupt boundary, measured
  with the reference.** That's the first number to hold your 342 against.

## Instrumentation on a Cortex-M0: what the operator actually wants

Not less science. The operator has been explicit: a well-instrumented,
correctly-scienced driving stack stays the goal. What changes is *how* you
observe, because the last day demonstrated the limit empirically:

> A 64 MHz M0+ cannot credibly observe a ~3 µs event with a ~3 µs probe.
> The persistence loop is 15 instructions per read (E519); the ON pulse at 7%
> is 3.5 µs; every recorder you built cost 2–13 µs and landed inside the thing
> it was measuring. Ten rounds of layout tuning against the gate (E493–E510)
> is the tell — when a scalpel can't meet its budget in two attempts, **change
> the observable, not the code layout.**

The rules that follow, in the order I'd apply them:

### 1. One build, one question. Never one build with all instrumentation.

cfg-gated scalpels or standalone binaries, each with (a) the question it
answers, (b) its measured cost, (c) its retirement condition. When the question
is answered the feature is deleted or frozen, not carried. Your current tree
carries `bench-*` features by the dozen and the qualified build links ten of
them at once; the linked ISR is not the reference's ISR anymore, and E519 shows
you already know it (15 vs 16 instructions). The *qualification* build should
link **zero** diagnostics; scalpel builds are separate artifacts with separate
hashes, and a scalpel result is cited as "scalpel build X saw Y," never
transferred to the qualification build's envelope. You already keep that
discipline for hashes — extend it to the feature set.

### 2. Aggregate in the ISR, publish from the foreground, at kHz not MHz.

The rare event you are hunting is a 4–5σ single-boundary excursion. You do not
need to *see* it. You need its **footprint**, and footprints are cheap:

| Question | Cheap observable | Cost |
|---|---|---|
| Where does persistence reject? | 12 `u16` counters, one per rejection index (you already emit the index — E514) | one `ldrh/add/strh` per rejection |
| Is the late accept phase-locked to PWM? | `TIM1.CNT` at accept, `>>` into 8 bins → 8 counters | ~6 cycles |
| How late, in carrier units? | accept-to-reference-time delta, `>>` into 8 bins of ½ carrier period | ~8 cycles |
| How many comparator dispatches per boundary? | per-boundary `u8` count, `max` and a 16-bin histogram | ~4 cycles |
| Did the guard's refused cycle have a long *predecessor*? | you already retain the pair; keep exactly that, nothing more | 0 new |

None of those need timestamps, rows, epochs, CRC framing or a decoder. They are
`static` arrays the foreground dumps once at the end of the run. Their combined
ISR cost is below your 2 µs gate by an order of magnitude, and they answer the
questions E512–E518 were asking. The 16-row detailed recorder was the wrong
*shape* of observable for a rare event; the histogram is the right one.

### 3. Use the RAM as a low-rate flight recorder, not a high-rate one.

~30 kB is free. A `u16` per commutation (interval, or accept offset in ticks)
for a whole 30 s run at 340 eHz is 2 040 commutations/s × 30 s × 2 B ≈ **122 kB**
— too much. But **one `u16` per commutation for the last 8 s** is 32 kB, or one
per commutation with ½-µs → 2-µs quantization and `u8` deltas fits 30 s in
under 16 kB. Circular, ISR-written with a single store, dumped post-run. That
gives you the *entire interval series* around the fault at 2 µs resolution
with a single `strh` in the ISR — a strictly better instrument than any 16-row
recorder, at 1/100th the cost. This is minz's `ZC_TRACE` pattern; it is what
cracked the parity bias there in one histogram after seven blind experiments.

### 4. Indirect observation is science too — this is what krabilorean was for.

The operator reminds you, and it's worth reminding: **krabilorean** is the
sibling no_std time-series-feature crate that minz consumed and qualified on
target for exactly this job — "collapse host-side Python analysis into onboard
instruments." Feed it the commutation-interval stream in **foreground** (minz's
`krabimon` pattern: main context, tier dial, zero prio-0 ISR cost) and you get
running mean / variance / kurtosis / periodicity of the interval series at
~1 kHz. A late-accept boundary is a **kurtosis spike** and an **outlier in the
residual** — visible in a 1 kHz summary line without touching the ISR at all.
That's the "lower-frequency instrumentation interval" the operator means.

Two things you must know before you do it, both from minz's qualification:

- **The exact-vs-block verdict is core-specific and M0 was the open milestone.**
  On M4 the exact personality won (hardware `SMULL`, 728 B, no helpers). On M0
  the exact/wide personality pulls a soft `__aeabi_lmul`; the block personality
  is 32-bit-clean but bigger, and it **collapses low-variance signals to zero**
  (99.7% of minz's real windows under block8). A commutation-interval stream
  with σ ≈ 25 µs on 500 µs is *exactly* a low-variance signal. So: measure
  block12/16 vs exact on **this** core with your objdump audit before trusting
  either; and if block collapses your variance, feed it the *residual* (interval
  minus running mean) so the variance is the signal. This is the M0 crux
  minz's notes left open — you're standing on the rig that closes it.
- **`black_box` the numeric result, not `.is_ok()`** — minz's DCE hazard; a
  256-sample loop measured 77 cycles = eliminated.

ratch22's `MONSTER-CASCADE` detector was also qualified on real cascades;
the same foreground seat fits it.

### 5. The reference is your oscilloscope now.

The cheapest, most credible instrument on this bench today is **the same
scalpel compiled into stock AM32**. Your UART adapter already lives in its
tree; the hook points (persistence-reject index, accept-vs-PWM phase,
dispatches-per-boundary) exist in the same functions. Put the histogram
counters in **both**, run **both** at the same duty, dump **both**. A
*differential* measurement cancels the observer effect — both are perturbed
identically — and turns every "is this normal?" into a two-column table. If
AM32 shows the same index-11 rejections and the same late-accept tail at
340 eHz and doesn't care, your wall is the CycleTiming guard's tolerance of an
event the reference rides through. If AM32's histograms are clean where yours
aren't, you have a code diff to bisect. Either way it's decided in one run.

### 6. Cost gates: keep them, change what they gate.

The 2 µs / 4 µs gates were right in spirit — an instrument must be a small
fraction of the event. Keep the rule, but state it as a **fraction**: an
in-ISR observable may cost at most ~5% of the shortest thing it sits inside
(the persistence pass ≈ 3 µs → 150 ns ≈ 10 instructions). That rule *forbids*
the row recorder on its face and *permits* every counter in the table above.
You'll stop iterating against a gate the observable can never meet.

## What I would do tomorrow, in order

1. The two changeover tests on stock AM32 (§ above). One session, no build of
   yours touched.
2. Strip the qualification build to zero diagnostics; freeze it; re-qualify 6.9%
   once so the envelope has a clean-ISR hash.
3. Add the 12 + 8 + 8 counters and the `u16` interval ring to **one** scalpel
   feature; measure its cost once; put the identical counters in the AM32 tree.
4. Run both at 7.0% for 10 s, record-only CycleTiming (electrical guards
   untouched — current, bus, nFAULT, deadlines stay as vetoes). Two-column table.
5. Seat krabilorean in the foreground on the interval residual; qualify the M0
   personality with the objdump audit; add its 1 kHz summary to the `i`-style
   status line. That's the low-rate instrument that stays in the build.

Everything in this list is smaller than a single one of the E493–E510 entries.
The science gets *better*, not thinner — you just stop trying to photograph a
bullet with a 64 MHz shutter.

— the minz graybeard
