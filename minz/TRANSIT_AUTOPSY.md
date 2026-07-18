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

## Attack-list outcomes (2026-07-17 late session)

**#1 NOZ instrumentation — LANDED.** First-wrap comparator-level
sample (`WINDOW_OPEN_LEVEL`, TIM1_UP) + `window::classify_noz` +
`noz: pre=/nev=` i-line counters + core split
(`window_control_step_from` / `control_reset` + deferred-pair
equivalence test). Result data: **steady dwells 60-74 have ZERO
misses of either class** (~520k accepts clean — dwell parity with
AM32 already exists); the fatal transit shows both classes (~13
pre / ~15 never in one fatal climb). A pre-crossed window means the
crossing PREDATED the window: it happened while the mux still
pointed at the previous phase, so NO EDGE ever existed — neither
the ear nor the gate can rescue it; only schedule accuracy (or a
level-based acceptance) can.

**#2 blind-zone shrink (deferred close) — ATTEMPTED, REVERTED.**
LPTIM2 kept actuation+snapshot+reschedule and re-opened the ear at
~11 us (vs ~20), pending close/bb/serialize to a priority-2
TIM1_BRK_TIM15 handler. Bench: comp storms starved the handler
exactly when it mattered — dfo=2475 dropped closes during engages,
+201 during the fatal climb (reacq machinery crippled mid-crisis),
pre-misses UP (+49 vs +13), envelope unchanged (died 76 both).
A shippable version needs an inline-overrun fallback + seqlock;
core keeps the tested split for that retry. ALSO: the gate was
never the discard mechanism under --geom — the R2 stiff gate
already REPLACES the window gate (either/or), so rung #3's
gate-tolerance premise was largely already satisfied.

**#3 early-ZC handling (level-rescue) — ATTEMPTED, REVERTED to
count-only.** Publishing a synthesized qZC on a pre-crossed
first-wrap sample fired on 2.6 % of at-speed windows and STOLE
their real accepts (the COMP accept path keys on qzc==MAX):
estimator starved (rej census 5.5k -> 372), stiff-gate reference
stale, the 76 transit died HARDER (rsd 8/4, burst 7, 4.68 V bus).
The class is real; the mechanism must be a low-confidence candidate
that YIELDS to any real edge, not a preempting publish. `rsq=`
count-only diagnostic retained.

**Standing conclusion:** dwells are clean; the death is schedule
LAG under acceleration pushing crossings behind the mux switch.
The honest remaining levers are structural:true commutation-latency
reduction (a GP-timer one-shot to replace LPTIM2's chain, COMP ISR
diet) and/or estimator lead during commanded climbs — both
plan-scale work, not session-end patches.

## 2026-07-18 — THE TRANSIT-POINTED MICROSCOPE AUTOPSY (goal complete)

J-armed run tautopsy5 caught a fatal climb event with the full
instrument stack: the 4-channel CTX ring (85 ms of per-PWM-cycle
A/B/current/sector+comp), the spike-onset bb, and the sag-kill bb.

**The ring (frames 1210-1275, 41.6 us/frame):** 50 ms of rock-steady
1.7 A, then at frame 1214 sector 4 HOLDS for ~9 frames (375 us =
3.5 interval-times) - the rotor-clocked loop WAITING on a window
whose comparator sat saturated from open (no edge ever coming) -
while current stepped 69 -> 296 raw (~1.7 -> 7.4 A) at up to 2.5 A
PER PWM CYCLE with the drive parked on the sector. Recurring 4-frame
holds after recovery pumped more; bus sagged; lk=4. Both bb dumps
show PERFECT commutation (104 us, d=8-9) because they cover the
re-locked tail - the CTX ring is the only instrument that saw the
stall.

**Verdict: the transit surge is the INVERSION'S WAIT, not the
ramp, not the timing, not the recovery.** A window that opens dead
(pre-crossed or BEMF-invisible) never edges; the pure wait-forever
rotor-clock pumps BEMF-aided current into it. AM32 never enters
this state (no schedule lag); we still do on climbs.

**Fix (landed, rsq=9 firing): the BOUNDED-WAIT STEP.** No qZC past
1.25x interval = the window is dead in every class (an acceptable
edge would have been accepted long before) -> commutate NOW,
publish the synthesized qZC, count (rsq=) + bb EV_RSC, no estimator
feed, no starvation-watchdog feed. Three instructive misses en
route: polarity keying (convention mismatch with the observed
saturation level), raw==0 saturation keying (comp noise ticks raw
~1.2 edges/wrap on gated edges), and first-wrap-only evaluation
(the wait bound can never be true at 40 us). Also landed:
panic-to-UART in minz::panic (two silent IWDG reboots during the
hunt were unattributable; RTT-only panics are invisible on this
bench).

Envelope validation deferred to a fresh bench: tonight's band slid
to 66-74 across ALL builds including controls (9 h session).
