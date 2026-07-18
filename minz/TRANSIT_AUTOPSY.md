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

## Unbiased-review sharpening (2026-07-18, second-agent read of the chart)

Two corrections adopted:

1. **The initiating defect, stated exactly:** a float window opens
   already comparator-saturated, produces no edge BY CONSTRUCTION,
   and the powered bridge then waits indefinitely for an event that
   cannot occur. The orange-span + flat-comparator + accelerating-
   current trio in `captures/tautopsy5_autopsy.png` proves the
   mechanism visually. "Holding a stale sector" undersells it -
   the window is dead at birth, not abandoned.
2. **Everything after is downstream:** phase 2 is a failed catch-up
   process, not the defect; phase 3's smooth L/R decay shows the
   final cut is electrically sane; phase 4's gentle ~1 A hump shows
   the recovery slew is a separate, controlled re-ramp.

**Acceptance criterion for the bounded-wait step** (binary,
mechanism-anchored - NOT "fewer spikes"):
- no powered hold longer than the bound (live: `wmax=` on the
  i-echo, the max commutation gap under CL per echo period, must
  read ~<= 1.25x interval + poll grain);
- no comparator-flat interval allowed to pump past the early ~3 A
  region;
- therefore NO phase-2 catch-up fight should ever exist in a ring
  capture (`rlate=` counts holds outliving 2x the bound - must
  stay 0).

**AM32 confirmation experiment** (whitebox rig, next session): count
whether AM32 ever permits an already-saturated window to hold the
prior powered sector past its equivalent timeout - a wait-event
counter (INTERVAL_TIMER > 1.5x commutation_interval while running)
+ max-wait aggregate on the SPK line. Prediction: ~zero waits under
normal running; any nonzero reading would show what wait duration
its current levels tolerate.

## 2026-07-18 (late) — the bounded-event autopsy: VERDICT

Instrument changes: WAX trigger 150->110 raw (~2.9 A) + `rimax=`
(per-hold peak current self-reported AT each rescue fire - the
decisive instrument). En route, three findings that reshaped the
campaign:

1. **The J-armed IWDG-reboot class root-caused**: cortex-m-rt's
   paint-stack feature fills ascending from end-of-bss to
   _stack_start; with the stack moved to SRAM2 (0x10004000 <
   end-of-bss) the fill never terminates and walks off mapped RAM -
   imprecise bus fault at boot, dead chip, no prints. Fix: feature
   removed; stack stays in SRAM2 (own 16K, overflow = LOUD bus
   fault). Diagnosed via RCC_CSR (IWDGRSTF), CFSR/HFSR, and the
   stacked exception frame read straight off SRAM2.
2. **The microscope perturbs the patient**: J-armed (free-run
   oversample ON) runs collapse the bounding (rlate=80, wmax=980,
   deaths at 60) while J-free runs hold rlate=0 - the documented
   free-run/injected ch8 interference degrades the loop itself.
   Ring captures remain valid evidence but J-armed envelope/
   robustness numbers are NOT comparable to J-free runs.
3. **The operator's ear finding**: the open-loop engage setpoint
   (60 Hz) sits 7.5x below the CL equilibrium at the engage amp
   (~450 Hz) - the engage lottery IS the newborn loop surviving
   that pull-up on a single stale seed. FREQ_START 60->180 (2.5x
   gap) improved engage immediately (1 retry vs 4-fails); R6's 8/8
   is the zero-gap confirmation.

**THE VERDICT (fstart1: wmax=280 rlate=0 rimax=379):** the
bounded-wait step works exactly as designed - every dead-window
hold is time-bounded - and it is INSUFFICIENT: a single bounded
hold reached 9.7 A (rimax 379 raw) before the step fired. At the
measured ~2.5 A/PWM-cycle pump rate, current outruns ANY usable
time bound (1.25x interval = ~3 cycles = ~7 A). The accumulate-
across-events hypothesis is dead in its soft form; each event is
individually lethal-scale.

**Required fix shape (next rung): the IN-HOLD DUTY CUT.** Cut duty
to the floor when qzc==MAX and since_comm exceeds ~1.0x interval
(before the 1.25x step; a normal ZC has arrived well before 1.0x),
restore on the next real accept. This is the parked wait clamp
reborn with the autopsy-derived threshold: the old 2.5x-interval
key was too LATE (not too eager) and its 50 % cut too weak. The
rescue keeps the chain alive; the cut keeps the hold survivable.

## 2026-07-18 morning — the in-hold duty cut arc: PARKED, lineage CONVICTED

The spec was implemented and iterated three times, each fail
autopsied immediately:
- 1.0x threshold: fired EVERY window at ~1130 Hz (wcut=98, rsq=0,
  slew=1657) - the estimator's ~3 % climb lag means since_comm
  naturally exceeds 1.0x estimate; the loop strangled itself at
  floor duty (sag bb: perfect 146-150 us chains at the kill).
- 1.125x: still 56 false-fires per 1 real dead window - the
  legit-late and dead-window distributions OVERLAP in time during
  climbs; no advance threshold separates them.
- Instant-restore on SHOT_REFINED (discriminate after the fact;
  false fires cost ~us): machinery finally correct (wcut=42/42
  all-false, slew normal) - and the climb STILL died sub-60 with
  every instrument quiet.

**The same-hour triple that ended the arc:**
1. AM32 sweep: 100 % / 2325 Hz / 4.13 A / zero spikes - hardware,
   motor, prop, supply all exonerated on demand.
2. Pre-rescue control (7a4b77f): first-try engage, full ladder
   through 76.
3. The full rescue+cut lineage: died sub-60 the same hour.

**Verdict: the bounded-wait step + level-rescue + in-hold cut -
each individually bench-validated - SUM to a loop that fights
itself.** Parked behind RESCUE_MACHINERY=false; parity restored
(parked1: control band, first-try engage). Counters (wmax/rlate/
rimax/wcut) stay live as observers. Retrospective caution: the
dead-window stall that justified the arc (tautopsy5 ring) was
captured J-ARMED - the microscope perturbs the loop and may have
amplified the phenomenon. Re-open only on J-free evidence of the
dead-window class (the observers will show it: wmax/rlate move
while rsq/wcut stay 0).

## Review corrections (2026-07-18, second-agent + operator)

1. **Overclaim retracted**: the observer1 bb shows the loop ADVANCED
   sectors cleanly between the dead windows - it does not prove
   those advances were PHASE-correct. The dead windows may be the
   first OBSERVABLE symptom of a timing/estimator divergence that
   began earlier. Treat them as symptom onset, not fault onset.
2. **AM32 mechanism-absence assumption corrected**: established
   fact (operator's runs): AM32 exhibits no clips/holds on this
   rig. NOT established: HOW - whether it avoids dead windows
   entirely, advances on a timeout/prediction path, accepts a
   different class of ZC, or filters its interval estimate
   differently. The differential trace must answer the HOW, not
   assume it.
3. **The narrowed question**: dead windows are NOT inherent to this
   motor/load/board (AM32 proves it). What upstream timing/
   acceptance behavior prevents AM32 from ever opening one? All
   minz recovery work is downstream by definition; the comparison
   point that matters is BEFORE the first minz dead window.

### The differential climb trace (agreed design)
Same motor/supply/carrier-where-possible, identical 60->80 throttle
profile, common start marker. Per commutation/ZC window, BOTH
firmwares emit the canonical record:
  timestamp, sector, duty, window_open/close, raw_edge_time|none,
  accepted/rejected+reason, measured_interval,
  estimator_period_before, next_deadline, zc_position_in_window,
  prediction_error
Mechanically answerable questions, in order: does minz's estimated
period lag first? does its deadline exit the viable window? does
AM32 accept an edge minz rejects? or does minz schedule correctly
but apply the wrong drive state/duty? The FIRST divergence is the
bug class; the 11 A hold and sag are aftermath.

## 2026-07-18 — AM32-side trace emitter LANDED + the reference envelope

ZC_TRACE (AM32 uart_control, fully #ifdef-gated, unperturbed build
byte-identical): per-commutation canonical record from the COM_TIMER
ISR. Capture tooling: scripts/zctrace_capture.py. Perturbation-
checked at the trace regime (60%: behavior == baseline); >85%
throttle saturates the wire by design (not needed - the experiment
targets 60->80).

**First capture, 262,147 records through the 60->80 climb - the
reference envelope for the differential:**
- raw period vs estimate: mean +0.40%, sd 5.4%, p1 -9.8%, p99 +9.3%
- raw > estimate+12.5% (minz's wait-cut band): 1 event in 262k
- raw > estimate+25% (minz's step bound): ZERO events

AM32 never opens anything resembling our dead windows in this band.
Our loop logs rlate=3 (holds >2.5x interval) and 7-20 rescues per
fatal climb in the same band. The differential's remaining half:
emit the same record from minz over the same climb and find where
our period-vs-estimate tail is born.

## 2026-07-18 — THE DIFFERENTIAL OVERLAY (both halves captured)

minz MZT record landed (Z key, 5B AA, field-for-field comparable;
scripts/mzt_capture.py): 224,698 records over the identical 60->80
climb (which SURVIVED this run). Overlay vs AM32's 262k:

| period vs estimate | AM32 | minz |
|---|---|---|
| mean / sd | +0.40% / 5.4% | +0.82% / 5.0% |
| p99 / p99.9 | +9.3% / +10.5% | +10.8% / +16.0% |
| > +12.5% | 1 event (0.0004%) | 856 events (0.38%) |
| > +25% | 0 | 0 |

**Core tracking is near-identical; the whole difference is a thin
excursion tail 1000x more frequent in minz, capped at +21% on this
surviving run.** Tail anatomy: all 856 events carry REAL accepted
ZCs (qzc_off 61-85 us, late-in-window) with refined schedules -
zero misses, zero dead windows. The late-tolerance machinery
handles them correctly; the excursions themselves are genuine
rotor-period stretches that AM32's rotor+loop simply never
produces. Working hypothesis for the fatal climbs: the same tail
grows past +25% into dead windows.

Next analysis (data in hand): why does the same motor+profile
produce 15-21% period excursions under minz but not AM32 - duty-
pipeline micro-disturbance (6 kHz slew steps vs their 20 kHz),
commutation-jitter feedback, or estimator response shape (their
2-period half-blend vs our smoothing). Then: capture a FATAL
climb's MZT trace and watch the tail grow.

## 2026-07-18 — record v2 + the fatal-trace classification

MZT v2 (5B AB, 17 B): adds est_before (schedule_precheck's iv IS
pre-update - stored at the arm sites), giving the reviewer's full
quadruple: raw period, estimate-before, estimate-after, armed
deadline, at one semantic point. Both-sides-post-update overlay
semantics verified.

**The shared sawtooth (major reframe):** lag-1 autocorrelation of
period error: AM32 -0.76, minz -0.62/-0.45; alternating amplitude
AM32 4.6%, minz 3.3-5.4%. BOTH loops ride a ±10% every-other-
commutation oscillation - it is the physical system, not a minz
defect. The difference is BOUNDEDNESS: AM32's crests never exceed
+12.5%; minz's occasionally escape.

**Fatal-trace classification (fatal6 real death; fatal8/9 large
traces):** everything timing-side ORDINARY to the end - periods
84-104 us, sawtooth +/-13%, real mid-window ZCs, sane deadlines, 9
events >+25% in 330k records, zero >+50%. Four-way verdict: NOT
acceptance, NOT estimator, NOT scheduling - the class-4 boundary
(state/ownership or a non-control cause).

**And the non-control cause surfaced:** fatal8/9 'deaths' were
REBOOTS (boot banners mid-capture, lk=0, no kill prints, one
terminal record built from corrupt statics, sector sequence running
backward at the end). The silent-death class = the reboot/wedge
class, appearing under heavy-telemetry configs (J-armed earlier,
ZT now) and possibly underlying the whole silent family (levers1).
Permanent instrument added: reset-cause (RCC_CSR) printed + RMVF-
cleared at every boot - reboots now name themselves in any capture.
Open: the wedge mechanism under load (no panic, no hardfault print
-> LOCKUP-or-wedge -> IWDG).
