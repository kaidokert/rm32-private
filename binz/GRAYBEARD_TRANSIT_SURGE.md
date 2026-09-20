# Graybeard memo — the 42–44% trips are one event, and it's lock (2026-09-16)

binz, E797–E813 was strong work: the smooth ramp dissolved the "35% wall",
40% held 60 s clean at ~1727 eHz (E809), the severity-banded governor holds
45% under 3.5 A by giving duty away (E813). Then the campaign started sorting
nFAULTs — VDS OCP vs GDF vs UVLO — and the graybeard, yesterday, helped it do
that. That's the wrong layer, and the operator has now said so in the general
form. Writing it here so it outlives both of us:

> **Every wall or glitch in minz and rm32 bring-up was marginal or lost lock —
> transient or fatal.** Classify the events, but the cause is always the loop.
> The supply sagging, the DRV tripping, the governor folding, the PSU going CC
> are *messengers* of a commutation-timing event. Name them; don't investigate
> them.

## What that makes of E803, E810, E811, E813

One event, four messengers:

| entry | messenger | what it saw |
|---|---|---|
| E803 | bus fold, ~6.3 V instantaneous | the surge, through the supply |
| E810 / E811 | nFAULT at 44% / 42% | the surge, through a DRV comparator |
| E813 | five foldbacks 45 → 22% | the surge, through the 4.5 ms governor |

The slower-ramp-trips-lower finding (E811) already rules out acceleration.
What's left is the thing minz found every time with `onset_probe.py`: **the
commutation interval diverges first — a late step, then an early one — and
the current ramps after.** At ~1700 eHz a commutation is ~98 µs; one late
accept puts a full-duty pulse into the wrong pair and the messengers fire in
whatever order their deglitch times dictate. Which one fires is a property of
the messengers, not of the fault.

## The instrument

Not the fault register. **The interval trace around the trip.** A `u16` per
commutation (interval in 0.5 µs ticks) into a ring, frozen on any stop,
dumped post-run — the ZC_TRACE pattern. Then one question, answered by eye or
by a 20-line script: in the ~10 ms before the stop, did the interval series
show a late/early pair (transient marginal lock) or a run of growing intervals
(cascade, fatal)? That distinction, not the DRV's opinion, decides the fix.

Keep the 5 ms nFAULT re-read from yesterday's note as **confirmation only**:
a self-releasing trip says "surge-class", which is what the interval trace will
already have shown; a latched one (GDF) is the single case where the messenger
carries its own information.

## The levers — all in `GRAYBEARD_ROAD_TO_100.md`, now with the reason to pull them

1. **Persistence depth.** Static 12 reads at 98 µs commutations is a
   late-accept generator by construction; AM32 is at 3 here
   (`main.c:2631-2638`). This is the first thing to A/B, because it is the
   most direct way to *create* a late accept and the reference doesn't do it.
2. **Advance.** 18 fixed. Late commutation at 1700 eHz is braking, and braking
   is current for the same speed — the exact budget the surge spends. AM32's
   duty-mapped 12–22° (`:931`, `:2641`).
3. **Never hold a sector on a missed ZC** — step on the estimate. Turns a fatal
   cascade into a transient the governor rides through. *(Corrected: this is
   NOT AM32 behaviour — see the banner on ROAD_TO_100 §5. It's a new strategy
   and gets a tightly bounded one-step A/B, after the CPU measurement, not
   before.)*
4. **Matched-speed current vs stock AM32.** The parity metric: if the reference
   pulls less current at 1700 eHz on this rig, the difference *is* the marginal
   lock, quantified.

Order: instrument (interval ring) → 1 → 4 → 2 → 3. Each is under a page.

## What not to do

- Don't spend a morning on fault taxonomy, VDS strap levels or IDRIVE tables.
  Those are hardware-pass items and the operator is handling them.
- Don't attribute 42–44% to the supply, the leads, the DRV, or the motor until
  the interval trace has been read *and* stock AM32 has run at the same speed.
  Every prior wall on this bench was code; the base rate is not in the
  messenger's favour.
- Don't ratchet the current ceiling to reach 45–50. A governor that holds the
  motor at 22% under a 3.5 A limit is telling you the loop is wasting current
  at that speed; fix the waste, and the duty comes back on its own.

— the minz graybeard
