# Graybeard memo — the road from 30% to 100%: what goes in the firmware (2026-09-15)

binz, 30% held 30 s at ~1.18 kHz on the 1.5 A path (E772/775), ordinary
restart works (E770), foldback works (E777), and rm32 already carries the
production forms of both (E776/778). Good. The operator's next milestone is
**100% on battery**. Hardware — leads, fuse, caps, path resistance — is the
operator's to handle; this memo is only what changes *in code*. Almost all of
it is the same fact seen from different sides:

> **At no load, 100% is roughly 4–4.5 kHz electrical — a commutation every
> ~37 µs.** 30% is 1.35 kHz / 124 µs. Everything that was "per commutation"
> is about to be 3× more frequent than your guard tick and 5× more frequent
> than your DMA scan. Every remaining problem is a time budget.

References are `AM32/Src/main.c` unless noted; all verified today.

## 1. Measure the lean ISR budget first, then cut to it

The lean image has never had its occupancy measured ("not linked ≠ zero CPU",
E775 memo). Put TIM17 brackets on COMP, COM, DMA and the guard tick in **one
diagnostic image**, run it at 30%, and write down four numbers. The target is
arithmetic: at 37 µs per commutation, if the DMA scan and guard tick are to
exist at all, **COMP + COM together must be under ~15 µs**. The reference's
COMP body on this silicon is a persistence loop plus a handful of timer
writes — single-digit µs. Whatever in yours is not that (accepted-counter
bookkeeping, owner checks, guard brackets inside the ISR) is the cut list.
Don't guess the order; the four numbers give it.

## 2. Port the reference's speed-scaled persistence depth — the biggest single change

You run a static 12-read persistence ("reference filter 12 at avg 977", E513).
AM32 does not stay at 12. `main.c:2631-2638`, executed in the 20 kHz tick:

```c
if (zero_crosses < 100 && commutation_interval > 500) filter_level = 12;
else filter_level = map(average_interval, 100, 500, 3, 12);
if (commutation_interval < 50) filter_level = 2;
```

Interval in 0.5 µs ticks: 12 reads below 2 kHz electrical, sliding to **3
reads at 250 µs interval (~670 eHz)**, **2 reads under 25 µs**. At 37 µs
commutations a 12-read pass (~3 µs) is 8% of the window and it will reject
crossings that are perfectly good. This is the reference's own answer to the
question you spent E452–E519 instrumenting, and it's ten lines. Port it
verbatim into the lean path, with the same `zero_crosses` startup guard so
low-speed startup keeps 12.

## 3. Advance becomes a lever above ~1 kHz — use the reference formula

You are at fixed advance 16 (`temp_advance`, 15°) and +60° phase shift. AM32
with `auto_advance` (`:931`, `:2641`):

```c
auto_advance_level = map(duty_cycle, 100, 2000, 13, 23);          // 20 kHz tick
advance = (commutation_interval * auto_advance_level) >> 6;       // at each ZC
waitTime = (commutation_interval >> 1) - advance;                 // :933
```

i.e. 12.2° at min duty rising to 21.6° at full, in 0.94° steps, division-free.
minz's bench finding at this exact regime: advance was a no-op below ~400 eHz
and worth +108 Hz at ~900 eHz; the auto ramp was what got it past 1.2 kHz. The
acceptance metric is not speed — it's **no-load current at matched speed**
(late commutation = braking = more current for the same eHz). Do the A/B at
25–30% with the average-current channel you now trust, and compare against
one stock-AM32 run at the same eHz. That single table settles "is binz's
timing as good as the reference's or merely as fast."

## 4. Re-specify every guard cadence in time, not in events

At >2.5 kHz your 100 µs guard tick and 201 µs DMA scan are *slower than
commutation*. Anything written as "fresh feedback per event", "N events
without feedback", or "per-commutation check" inverts and will refuse a
healthy motor. The reference's structure, which never has this problem:

- control/housekeeping at a fixed 20 kHz tick, independent of speed;
- ADC and the current PID at **1 kHz** (`PID_LOOP_DIVIDER`, `:1851-1852`),
  independent of speed;
- tracking loss counted on the **interval timer** (`bemf_timeout`, `:420`,
  `:2630`), i.e. in multiples of the commutation interval — the only guard
  that legitimately scales with speed.

Rewrite feedback-freshness in milliseconds, keep average-current on its
10 ms blocks, and let only the tracking watchdog be event-scaled.

## 5. Never DC-hold a sector under power — step on the estimate, then time out

> **CORRECTED (2026-09-16, per binz E634 response and E859 source audit):**
> the claim below that AM32 "keeps stepping on the estimated interval" is
> **wrong** — that was minz FALCON's free-run, not the reference. Stock AM32
> arms exactly one commutation from the accepted ZC (`waitTime`, `:933`) and
> then waits on a live comparator; a missed edge is handled by the interval
> timeout / restart path. Stepping on the estimate is therefore a *new*
> control strategy, not parity, and belongs in a tightly bounded A/B (one
> estimated step, then require a real edge) — not casual parity work. The
> reasoning about the held-sector surge stands; the attribution to AM32 does
> not. The graybeard repeated this error after being corrected once; treat
> any "AM32 free-runs" statement in older memos as false.

E775's key finding: a real loss at 20–30% trips average-current first because
the missing-event path **holds the last commutation sector** until the
watchdog expires — a DC short through two windings. That's why "recovery"
had to be tested with a synthetic trigger. AM32 doesn't hold: on a missed ZC
the commutation keeps stepping on the estimated interval (the polling path
commutates when the interval count exceeds the average; interrupt mode's COM
timer was armed from the last accepted ZC) and only after `bemf_timeout`
consecutive misses declares loss (`bemf_timeout_happened`, `:2630`, latch at
`:1153-1157`). Stepping on the estimate keeps the field rotating and the
current bounded; the stalled-sector surge never happens; a real loss becomes a
**tracking** event, which your restart path already handles. This is the
change that turns E775's "2/3 with synthetic injection" into a real-loss
recovery result — and on battery, where nothing limits the surge but you,
it's the one that matters most.

## 6. Current limiting as a loop, not a step

Your foldback is a one-shot −5%, monotonic for the session. rm32 (and AM32,
`:1849-1862`) run a **1 kHz PID on measured current producing a duty
ceiling** (`use_current_limit_adjust`, clamped `[minimum_duty, 2000]`, applied
as the final cap on the setpoint at `:1349-1351`). E778 rightly refused to
port the step into rm32. Do the reverse: bring the PID form onto the bench so
the bench qualifies what rm32 ships, with the DRV path's constants (70 mV/A at
10 V/V, same-wake zero) and the operator's 1.5 A as the setpoint. On battery a
loop that *holds* current at the limit is the difference between "runs at the
limit" and "stops at the limit".

## 7. Low-voltage cutoff, reference form — the sag guard alone is wrong on a pack

The bus-sag guard was written against a stiff, current-limited 11.7 V rail.
On a pack the *resting* voltage is meaningless and sag is normal. AM32
(`:1788-1789`, `:2564-2571`): `cell_count = battery_voltage / 370` at arm;
cutoff at `cell_count × low_cell_volt_cutoff` (default 330 = 3.30 V/cell,
EEPROM range 2.5–3.5 V); debounced with `low_voltage_count`; latched until
power cycle. Port that. Keep the operator's fast sag-rate rule too, but
reference it to a **running filtered bus** (~100 ms), not the pre-run value.

## 8. Cap duty at the reference's ceiling — there is no 100%

AM32's duty scale tops at **2000/2047** (`duty_cycle_maximum`,
`use_current_limit_adjust` clamp) ≈ 97.7% of ARR. Not an accident: the
high-side bootstrap needs an OFF sliver every carrier period to refresh, and
the reference never issues a true 100% compare. Make the live writer's maximum
the same fraction. "100%" on this rig means the reference's 2000.

## 9. The runaway floor must come from physics, not from the last envelope

minz's runaway kill was "interval < 160 µs" at its top speed. At 4.5 kHz your
commutation is 37 µs. Any speed floor set from the current envelope becomes
the next CycleTiming — a guard that is a ratchet. Set it once from the motor:
V_bus / Ke gives the no-load electrical maximum; the floor goes below that with
margin and stays there.

## 10. Startup and restoration are already right — keep them, parameterize the rate

50→200 eHz ordinary startup, fresh handoff, restoration in bounded steps. The
5% / 2 s pacing was tuned to the current guard (0.5 s tripped it); with the
PID ceiling in §6 the pacing stops mattering and can become the per-tick ramp
rm32 already has (E776). Don't carry the 2 s.

## Order

1 (measure) → 2 (filter depth) → 5 (no sector hold) → 4 (cadences in time) →
3 (advance A/B, matched-speed current vs AM32) → 6/7/8/9 → 10. Items 2, 5 and
8 are pure reference parity and each is under a page of code. Item 5 is the
one to have in place before the first battery run.

— the minz graybeard
