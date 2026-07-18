# TRANSIT-DEATH AUTOPSY — the 70–78 climb kill class (2026-07-17)

First full-depth capture of the class that owns the remaining envelope
gap vs AM32. One J-armed ladder (`captures/autopsy1_*`, build 346f0eb
+ sag-kill bb dump) caught BOTH phases in one run: the >4 A WAX
trigger froze the current ring + bb at a surge onset (amp ~70-74),
and the fatal sag kill at the 74→76 transit dumped its own bb
(instrumentation added this session: `BB_FROZEN` at the ISR sag kill,
dump in the main-loop sag report).

## The two black boxes

**Onset bb (at the >4 A trigger)** — ends in the known high-speed
miss signature, 92-96 µs intervals then:

```
ACC s3 d=5 / REF s3 d=103      <- interval jumps 91->103
BLD s4, NOZ s4 d=0             <- blind commutation, ZERO raw edges
ACC s5 d=2 / DRK s0, NOZ s0 d=0
ACC s1 d=11 / BLD s2, NOZ s2 d=0
```

`NOZ d=0` = the comparator saw NOTHING in the window — the
window-position/comparator-blind miss class from the monster work,
occurring during a climb (where the speed-gated monster fixes have
less margin). The current crossed 4 A right here.

**Fatal bb (frozen AT the sag kill, bus 5635 mV)** — 64 events of
PERFECT commutation: strict ACC/REF alternation, sectors in order,
intervals 98-106 µs, delays 7-10 µs, zero NOZ/BLD/RAQ/RSD. The loop
was fully re-locked and healthy while the bus spent its final 1.3 ms
below −10 %.

**Counters across the fatal climb:** rsd 2→5 with strikes 0→3
(three reseeds inside one 100 ms amnesty window = a reseed storm),
burst=2. Nothing else moved (floor/ceil/harm ~0).

## The mechanism — the RECOVERY is the killer, not the miss

1. During a 70-78 climb a ZC-miss run occurs (onset bb; the known
   comparator-blind class — survivable in itself).
2. The guards respond correctly: burst clamp and/or R3 reseed
   (duty → 6 % floor).
3. The rotor decelerates through the dip (BEMF drops).
4. Reseed exits after 6 accepts (~600 µs). The duty re-ramp crosses
   the `max/13` (7.7 %) startup band in ~1 ms and then runs at the
   at-speed slew class — `SLEW_HIGH` 16 %/ms (`timing::duty_slew`
   rate selector keys on interval <500 µs, which still reads
   ~100 µs) — reaching 70+ % duty in ~5 ms against a slowed rotor.
5. (V_applied − BEMF)/R_winding = a winding-limited multi-amp smooth
   ramp (the exact shape the spikehunt captures showed). The supply
   chops in response to the draw — amp draw is the cause.
6. The surge sags the bus and distorts BEMF → often seeds the NEXT
   miss → reseed storm (strikes 3 observed). One recovery's draw
   finally holds the bus below −10 % past the 1.3 ms debounce →
   SAG KILL — with the bb showing a healthy loop, because
   re-acquisition had already succeeded. The energy transient kills,
   not the timing.

Why AM32 doesn't have this rung: its desync recovery re-ramps from
the slow state through `max_duty_cycle_change` at the STARTUP class
(2/2000 per 50 µs ≈ 0.8 %/ms — ~20× slower than our recovery), so
its recovery never outruns the rotor. Its lower miss rate is
secondary — its recoveries are simply not violent.

## THE DECISION (per the goal): harden the climb — specifically the
## recovery re-ramp. Do NOT chase commutation latency first.

- Cutting commutation latency (LPTIM2 floor/TIM16 one-pulse) attacks
  step 1, which the loop already survives; the kill lives in step 4.
  (And the TIM15 route is separately convicted — APB2 contention.)
- The targeted fix: after a reseed exit or burst-clamp release, hold
  the duty slew at the STARTUP class (2 %/ms) until the duty has
  caught its commanded target once (SlewOut.clamped == false), then
  return to the normal regime map. Turns the 5 ms 6→76 % jump into
  ~35 ms — the rotor can follow, the draw stays bounded, and the
  storm's positive feedback (surge → next miss) is broken.
- Instrument (decisions, not outcomes): recovery-window entry counter
  + max-duty-gap-at-entry telemetry, i-line row, same commit.
- Validate: paired ladders 66→80; expect the rsd strike storms and
  the 70-78 transit deaths to become logged single reseeds; then
  re-attempt 80+ and the R4 revisit (a calm recovery is exactly the
  loop-stiffness R4's climbs were missing).
