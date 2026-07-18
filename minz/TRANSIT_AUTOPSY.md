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

## THE DECISION — REVERSED 2026-07-17 evening (operator was right)

The first cut of this document decided 'harden the recovery re-ramp,
not commutation latency,' reasoning that the miss was survivable and
the recovery was the killer. The operator called it exactly
backwards: AM32 likely NEVER runs its recovery code — and that is
now PROVEN with counters (AM32 uart_control commit 579a893, SPK line
dsy=/bt= fields, keepalive-correct ladder on a fresh boot):

- Full 60->100 % ladder (1631->2308 Hz, 4.0 A) with every commanded
  transit including 100->70->100 swings: ZERO recovery-counter
  movement while running. AM32's desync/backstop paths are dead code
  in normal operation. It never needs recovery because it NEVER
  MISSES.
- The only in-run bumps correlated 1:1 with keepalive byte-drop
  input glitches (a lost '6' turns '60' into a 0 % command for
  0.8 s) — which AM32 ALSO rides through inaudibly, but that is an
  input event, not a ZC miss.

**Therefore the target is the MISS, i.e. the blind window.** Our
NOZ d=0 signature means the comparator saw nothing: the ZC of an
accelerating rotor drifts early, into the ~20 us commutation-ISR
blind zone (LPTIM2 ISR ~1570 cyc + mux settle at window open) and
ahead of the gate. At 100 us intervals that blind zone is ~20 % of
every window, positioned exactly where a climbing rotor's ZC
arrives. AM32's equivalent blind time is a fraction of ours (TIM16
one-pulse, no close_float_window/serialize work in the commutation
ISR, 0.5 us grain).

Attack list (latency/blind-window work, in order):
1. Measure the miss position directly: instrument the NOZ path with
   where-the-level-sat (pre-crossed vs never-crossed) + time from
   window open to first edge on the misses that DO see edges.
2. Shrink the commutation-ISR blind zone: move close_float_window
   out of the LPTIM2 ISR (lock-free MPSC or deferred serialize -
   the earlier deferral failed on struct size, not on principle),
   pre-arm the comparator before actuation, mux earlier.
3. Gate position under acceleration: allow early ZCs (AM32's rule
   is elapsed > avg/2 with NO late bound - our 30 % window-start
   gate discards an early ZC edge and then the level never
   transitions again = NOZ d=0).
4. The recovery-re-ramp softening (the v1 decision) stays on the
   shelf as a second-line mitigation only - fixing it first would
   mask the miss signal the counters give us.

The recovery-side observation from the first cut (reseed exit at 6
accepts + 16 %/ms re-ramp = multi-amp surge) remains true and
documented above - it is the amplifier, not the cause.
