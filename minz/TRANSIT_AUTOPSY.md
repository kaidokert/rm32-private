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

## 2026-07-18 — overflow-checked matrix campaign + crest-escape verdict

Build: release with overflow-checks=true (every wrap panics with
file:line over UART). Matrix, identical 60->80 profile:

| config | terminal |
|---|---|
| baseline | COMPLETED |
| J only | REBOOT (iwdg=1, no panic, no fault) |
| ZT only | REBOOT (iwdg=1, no panic, no fault) |
| J+ZT | COMPLETED |

**Arithmetic overflow EXCLUDED for the reboot class** (checked
build, zero panics before both reboots). The wedge is
telemetry-load-correlated but stochastic (both-config completed);
common factor = heavy TX volume -> prime suspect is the main-loop
TX/ring service path wedging -> IWDG. Reset-cause line (now at
every boot) named iwdg=1 in-capture both times.

**Crest-escape analysis (certified non-reboot runs):**
- escapes are sudden (+20-25% from an ordinary sawtooth, no
  build-up), carry REAL late ZCs (qzc_off 150 vs 98 us), zero
  unrefined, zero duty involvement (1/1173 nonzero duty deltas),
  and CLUSTER (26% within 6 records vs mostly->60 random).
- **THE PHASE-B SIGNATURE (J-free run): escapes concentrate on
  sectors 1 and 4 - the two phase-B float windows - at 0.84%/1.26%
  vs <=0.10% for all other sectors.** Sector 4 is the worst in the
  J-armed run too (7.6%). Phase B = PA5 + its own divider network:
  a per-phase physical/config asymmetry (divider tolerance shifting
  B's effective crossing, mux settle on PA5, or B-window confirm
  interplay) - the concrete next target for the crest-bound gap
  vs AM32.

## 2026-07-18 — pin/PWM config diff vs AM32: EXONERATED

Operator question: is the phase-B signature a pin-config
difference? Answer via live register diff (scripts/pin_reg_dump.py,
both firmwares flashed + dumped same hour):

- ALL six motor pins (PA8/PA7, PA9/PB0, PA10/PB1) and ALL four
  comparator inputs (PA4/PA5/PB7/PB4): MODER, OSPEEDR, PUPDR,
  OTYPER, AF byte-identical between minz and AM32.
- TIM1 CR1/CCMR1/CCMR2/CCER/PSC/ARR/BDTR: identical (dead-time,
  PWM mode, carrier - all matched).
- The only diffs are deliberate non-motor items: PA2 (USART2-RX
  swap vs DSHOT input), PB3 (scope trigger vs WS2812), TIM1.CR2
  (our injected-ADC TRGO2), and COMP2 INMESEL = mux position at
  dump time (state, not config).

The phase-B escape signature is therefore NOT static configuration.
Remaining candidates: runtime/software sequencing per phase (mux
switch timing, per-port role-flip ordering, window handling) or
the sense network's physical per-phase tolerances interacting with
OUR acceptance specifically (AM32 shares the network and holds the
bound - so if it is the network, the difference is in how the two
loops respond to the same shifted crossing).

## 2026-07-18 — HOW the pins toggle: the drive-mode difference

Operator question answered (phaseouts.c + targets.h + eeprom
default verified): with comp_pwm=1 (shipped default), AM32's PWM
role = high pin ALTERNATE (TIM1) + **low pin GPIO OUTPUT driven
STATICALLY HIGH** - the FD6288's internal interlock + dead-time
does the complementary switching. Their TIM1 CCxN outputs never
reach the pads while driving (low pins not in AF). We drive true
timer-complementary (CHx + CHxN AF, DTG=45 = 562ns). Invisible in
idle register dumps - a RUNTIME MODER difference only.

Implications: (1) our LIN pins switch at 24 kHz on every driven
phase, theirs are DC - every LIN edge couples into the shared BEMF
divider network, per-phase-asymmetric by layout (a candidate for
the phase-B escape concentration); (2) dead-time source differs
(562 ns DTG vs FD6288-internal); (3) their commutation order is
FLOAT -> LOW -> PWM under __disable_irq.

Priority question: materially equivalent (their COM_TIMER ISR at
NVIC 0 == their COMP, global mask during writes; our LPTIM2 at 1
== our COMP, interrupt::free around the role flip).

TESTABLE: one change to set_phase_roles (Pwm role: low pin
OUTPUT-high instead of AF) = byte-for-byte AM32 actuation
semantics. A/B ladder + MZT sawtooth/escape overlay answers
whether the low-side switching is the crest-bound gap.


## Drive-mode experiment — CLOSED, NEGATIVE (2026-07-18)

The hypothesized difference DOES NOT EXIST. The "AM32 low leg is
static-high enable" reading of phaseouts.c (commit 5b0ae36) was wrong:
that code lives only under `PWM_ENABLE_BRIDGE`, which VIMDRONES_L431
does not define (grep: only 3 unrelated targets at targets.h:3470/
3508/3546; `USE_INVERTED_LOW` also not ours). The shipped comp_pwm=1
path in `phaseBPWM()` sets the LOW pin **ALTERNATE** — timer
complementary CHxN + dead time, byte-identical semantics to our AF
Pwm role. AM32 on this board has the same 24 kHz low-side switching
edges we do; the phase-B escape asymmetry is NOT a drive-mode delta.

Bench confirmation that enable-mode is physically non-viable here:
implemented behind `DRIVE_ENABLE_MODE` (tim1_motor_pwm.rs, kept
=false as documentation), LIN static-high + HIN PWM hits the FD6288
interlock (both-high -> both FETs OFF): isns=0.000 A, comp storm,
0/16 engages. The const must stay false.

Estimator/scheduling investigation of the crest-escape tail remains
the open path (reviewer notified the premise was a misreading).

## Bench state changes discovered en route (2026-07-18)

1. **minz image now extends past 0x0800F800** (overflow-checks bloat)
   and CLOBBERS the AM32 EEPROM page on every minz flash. AM32 swap
   recipe is now: bootloader bin @0x08000000 + app bin @0x08001000 +
   eeprom page (0x01 + 0xFF*2047) @0x0800F800, then
   `bootloader_spray` (the bootloader only second-chance-jumps after
   GARBAGE bytes on PA2; a quiet idle-high UART never triggers it).
   Verified: AM32 NOTRACE flies first attempt (erpm 67100, 0.59 A).

2. **RETRACTED (2026-07-18, operator called it): the "mechanical
   load changed" claim was WRONG** — the ~1000 Hz baseline I quoted
   was confabulated. Verified: era-AM32 at 60% = 1634 Hz (zct_first
   ci_us median 102), today-minz amp 60 = 1650 Hz, era-minz amp 55 =
   1610 Hz — ONE curve, load unchanged. The 50 Hz jump-catch 0/20
   regression is therefore an OPEN code/build question (not load,
   not bench). Consequence: the jumped 50 Hz open-loop catch went 0/32
   (perfect rotating field, flat-BEMF float windows = rotor
   motionless — stallwax2 dump); AM32 ramps from ~0 Hz and is immune.
   Diagnosis path: engage crawl-locks -> AM32 falsifier flies ->
   waxwing shows textbook field + motionless rotor.
   **Fix: RAMP START** in both capture scripts (mzt_capture.py,
   cl_lock_map.py): arm at 50, dip to 10 Hz (stepper regime, always
   catches), ramp +10 Hz/150 ms to 180, engage. First-try engage,
   476 Hz lock at 66 mA on the manual probe; first-try engage in the
   validation run.
   ⚠ ALL cross-session comparisons to matrix-era data (mzt_first,
   mxC/mxD, AM32 zct_first) are now CROSS-LOAD — re-baseline both
   sides before quantitative overlays.

3. **Validation run `mzt_rampstart` (zt config)**: engaged first try,
   amp 60 = 1650 Hz, then DIED in the band climb as TERMINAL: REBOOT
   (IWDG, banner in capture) — the known telemetry-load reboot class,
   now holding a 303k-record MZT fatal trace
   (captures/mzt_rampstart_*). Next session: TX-wedge hunt has a
   fresh specimen.


## B-escape mechanism analysis (2026-07-18, mzt_rampstart 303k records)

Fresh same-session facts, all offline from the trace:
- Phase-B concentration REPRODUCES: >12.5% crests per-1k: s1=53.0,
  s4=63.6 vs 1.3-12.5 all other sectors (7216 total vs AM32-era 1 in
  262k; >25%: ZERO — bounded here).
- refined=100% EVERYWHERE, including inside escapes: these are NOT
  missed detections. The comparator ZC is found every window; the
  measured period itself runs long.
- Refund/sum-conservation test (period[i]+period[i+1] vs local 2-rev
  baseline; a late TIMESTAMP conserves the sum exactly, real decel
  does not): quartiles 0.30/0.56/0.77 — ~25% pure timestamp
  artifact, ~32% real-decel-like, 43% intermediate. Median escape
  excess 14 us at ~100 us windows.

Coherent mechanism hypothesis (testable): B's noisier analog path
occasionally makes the persistence check fail through the true
crossing region; the accept slips ~10-20 us to the next clean PWM
dwell (14 us ~ 1/3 carrier period). Because the accept ARMS the
commutation (scheduling inversion), the late timestamp fires a late
commutation = REAL torque mistiming = the intermediate/real fraction.
AM32 is immune because its filter_level scales DOWN with speed
(main.c:2112, shallow persistence accepts early) while ours DEEPENS
5->12 reads when the blank shrinks — our own noise armor manufactures
the latency. Predictions: (1) per-window persistence-retry counters
(instrument decisions!) spike in s1/s4 at escapes; (2) capping
persistence depth at speed (AM32-style) collapses the s1/s4 crest
rate. Next bench experiment: persistence-depth A/B at the high rungs.


## The IWDG reboot class — beacon-attributed (2026-07-18)

Per-ISR `.uninit` flight recorder (survives the reset) nailed the
mechanism across 6 instrumented reboots:

- main's IWDG refresh stops while TIM1_UP (prio 2) and COMP (prio 1)
  keep beating to the reset; TIM7 (prio 3) freezes WITH main.
- => **ISR CPU saturation pockets**: total load at priority <=2
  crosses 100% (engage-phase comparator storms ~70k/s at stall;
  serialize+confirm at climb tops), everything at prio >=3 plus
  thread mode starves >1 s, IWDG fires. cpu=99% was on the wire in
  the final pre-death i-echo.
- The eerie 53.6x s death times were NOT the CYCCNT wrap: with
  first-try engages the climb profile puts amp 78-80 at ~53 s. A
  pre-band death at 64.9 s (engage-phase storm) broke the pattern.
  The wrap-extension store-order race found en route (HIGH bumped
  before LAST -> reader double-counts the wrap, +53.7 s forward time
  glitch) was real and is FIXED (LAST-first), just not this killer.
- Also hardened en route: drains bounded 64/microloop (a starved
  main's `while let Some = dequeue()` never exhausts against a live
  producer); run_until target clamped to now+2 microloops (no clock
  glitch can spin past the refresh); write_blocking bounded (50 ms ->
  drop + TX_DROPPED).
- REMAINING WORK (next campaign item): load shedding — AM32-style
  COMP masking outside windows at stall/open-loop, serialize budget
  at the top rungs. The reboot class persists until ISR load pockets
  stay under 100%.

## Zombie + liveness follow-ups (operator goal, all landed)

1. **comms-delta liveness** (mzt_capture.py): mid-run check = record
   FLOW (records stop when commutation stops); final check = comms
   counter delta across two reads. The ACTIVE flag is frozen statics
   and lies on a zombie (the 30 s / 1.5 A mzt_beacon incident).
2. **i-print exonerated**: 6 rapid i-presses at idle -> t1u maxgap
   42 us (one PWM period). The print masks nothing; the earlier
   2.7 ms attribution was wrong (probe/boot artifacts).
3. **Comm-silence zombie backstop** (`guards::comm_silence_backstop`,
   100 ms): kills on raw commutation silence under CL, independent of
   every derived reference (qzc chain / reseed / desync fold — all of
   which the zombie confused). Runs in BOTH TIM7 and TIM1_UP — the
   TIM7 copy alone starves in exactly the saturation pockets where
   zombies form. Counter zbk= in the i-echo; validated no-false-fire
   at amp 40 CL.


## COMP-storm mask-and-kill (2026-07-18, operator-directed)

The load-shedding first responder for the saturation-reboot class.
Detection = per-window edge count (`WINDOW_RAW`) checked at COMP ISR
entry - the storming ISR is the one context guaranteed cycles, and it
masks its own source. Two regimes (guards::comp_storm_action, host-
tested): open loop = MASK-ONLY at 64 edges/window (the ramp start
deliberately stalls at 50 Hz; a kill would break every arm; TIM7
re-enables once per sector, capping worst-case storm load at ~19k
IRQs/s while the drive continues); CL = MASK+KILL at 500 (a window
holding 500 edges without closing is a dead loop; kill lands in ~7 ms
vs the IWDG's 500+ ms, preserving main + diagnostics). `storm=M/K`
in the i-echo.

Live-fire: 50 Hz stall storm comp 70k -> 9.8k/s, motor driven, zero
kills; ramp engage OK; CL amp30 931 Hz clean, kills=0.


## Main-starvation guard + cpu=% removal (2026-07-18, operator-directed)

The saturation pockets starve main, not the ISRs - so the detector
watches the SYMPTOM: BEACON_MAIN heartbeat staleness, checked from
TIM1_UP (guards::main_starved_action, host-tested). SHED at 250 ms
(stop ZT/MAGPIE producers, give main a recovery window), KILL at
500 ms (clean stop, 500 ms before the IWDG would reboot and destroy
the evidence). `mst=sheds/kills` in the i-echo.

**Acid test: the ZT-climb reproducer that rebooted 5x now terminates
as TERMINAL: KILL** - mst=1/1, kill print on the wire, chip alive,
all counters readable. The IWDG reboot class is converted to clean
kills with full post-mortem data; the IWDG remains as the true-wedge
backstop.

cpu=% RIPPED OUT (idle-loop busy machinery + ~1 s boot calibration):
the baseline was boot-sensitive, the number invited misreads, and it
was read via main - the context that starves. `dur cyc:` remains the
trustworthy accounting; mst= is the actionable monitor. Microloop
slack is now a plain bounded spin.

Load-shedding levers (gate ADC-confirm under SWIFT, serialize in
main, half-rate vbat/sag, retire TIM1_CC) remain the path to actually
CLEARING the pockets so runs complete instead of kill.


## Load-shedding arc — final state (2026-07-18, operator goal)

Landed and validated, in order of discovery:
- **Diag-ring gates** (`DIAG_RINGS`, X key, default OFF): waxwing CTX
  + PWM sample byte (6 stores/cycle in TIM1_UP) and EDGE_BUF per-edge
  writes (~5 atomics x 30k/s in COMP) now cost zero unless an autopsy
  session enables them.
- **Engage restored 7/8** (was 0/8): my own storm mask was fighting
  the engage detector - open-loop "storms" ARE the no-accept-yet
  windows the detector must sift; budget 64 masked mid-search.
  STORM_MASK_EDGES 64->256 (the IWDG-starvation concern it guarded is
  covered by the proxy refresh). Cold-boot first-arms still warm up
  (~2 throwaway arms after a flash).
- **CL rate-storm detector** (comp_storm_rate_step): the per-window
  budget is rate-blind at speed (125 us windows cap at ~8 edges in a
  45%-CPU storm); sustained >=72k/s for 4 ms now kills.
- **mst guard redesigned**: sub-1 s main stalls are survivable (bb
  showed the loop riding through); TIM1_UP proxy-feeds the IWDG while
  main-stall < 30 s (true wedge = ISRs dead = 1 s reset unchanged);
  the mst KILL is disarmed (counter + shed only) - a stalled main is
  a telemetry outage, not a motor hazard.
- **PendSV stall profiler** (naked handler): stacked-PC + LR + stack
  slice + 8-sample PC ring of thread mode, on demand from any ISR.
  GDB is unusable on this bench (error 138 + mcp timeouts) - this is
  now the debugger of last resort, and it works.
- **TX black-hole wedge found + self-healed**: probe autopsy during a
  live "stall" showed main HEALTHY (beat advancing) while all UART
  output silently died - the tx_writer's DMA state machine can
  desync (L4 DMA ignores CMAR/CNDTR writes while EN=1; a lost TC
  completion wedges service forever). service() now resyncs from
  hardware state (CNDTR==0 => complete; force-EN=0 before arming),
  counted as txrs=. The "main stalls" the mst guard measured were
  write_blocking crawling against the wedged DMA at 50 ms/byte
  timeouts during multi-line dumps.

**Where the runs die now**: with every instrument artifact silenced
(txrs=0, mst=0/0, no reboots since the proxy refresh landed), the ZT
reproducer terminates in REAL control kills - CL desync / sag at
~1100-1400 Hz mid-climb - each with full MZT trace to the death, bb
dump, and classified terminal. That mid-rung death class (present all
day, previously masked by reboots and guard kills) is the estimator-
tail campaign's subject, not an instrumentation failure.

Session tally lesson (operator): engage wasted a large fraction of
bench time before the tally was demanded - keep per-session engage
tallies; a collapsing engage rate is a CODE signal (it was the storm
mask), never bench luck.


## THE TRACKING ANSWER (2026-07-18, operator-directed AM32 head-to-head)

Same bench, same hour, same 60->80 climb:
- **AM32 (zct_today, 459k records): COMPLETED.** |ci-avg|/avg median
  1.0% at 100-150us (92k records), 2.6% at 150-350us. **No sawtooth**
  (lag-1 structure absent).
- **minz (mzt_txfix/mode0): dies at 950-1400 Hz.** |per-est|/est
  median 8.4%, p95 15%. Lag-1 autocorr of the period residual:
  **-0.77 at +-17% amplitude - a violent period-2 alternating
  oscillation.** ZC position within the window is TIGHT (0.556
  +-0.043 of period) - detection is fine; the PERIOD oscillates.

**The matrix-era "shared physical sawtooth" conclusion is FALSIFIED:
the sawtooth is OURS.** Our accept->schedule feedback self-excites an
alternating early/late commutation cycle; amplitude grows through the
mid-rungs until a window misses its ZC -> desync kill. AM32 schedules
from the RAW last interval (wait = ci/2 - advance) and stays clean;
our smoothed-estimate scheduling overcorrects at period-2.

Also: mode-3 EXTI default exonerated for the mid-rung deaths (mode-0
revert died identically at 947 Hz) - reverted to 0 anyway pending the
oscillation fix. Next campaign: damp the period-2 mode (schedule from
raw-last-interval AM32-style, or add period-2 damping to the
estimator), then re-run the ladder.


## THE PARITY BIAS (2026-07-18) — the tracking gap's mechanism

The "period-2 oscillation" is a STATIC POLARITY BIAS, not an
instability: even-sector (rising-BEMF) periods run +54 us (+19%)
longer than odd-sector, consistent across runs (mode0 +54, txfix +55,
rampstart +24 at higher speed). **54 us ~= one 24 kHz carrier
period**: one polarity's ZC is accepted ~one PWM cycle late — the
crossing's ringing defeats the persistence check in that polarity and
the accept slips to the next clean dwell. The geometry-mode
estimator's pair-averaging (AM32's rule, already implemented) hides
the bias from the ESTIMATE while commutations still fire off the
biased ZC train, so periods alternate mechanically. As windows shrink
toward 120 us the fixed bias becomes ~45% of the window -> gate
misses -> the mid-rung desync deaths. AM32, same comparator and
rotor, shows NO bias (1.0% tracking) — their acceptance does not
slip carrier cycles; why is the next investigation (their
filter_level sampling cadence vs our spaced-read persistence is the
prime suspect).

Fix directions (next session): (a) understand AM32's immunity and
port it (persistence sampling cadence/phase relative to the carrier);
(b) polarity-aware accept-time compensation (+~1 carrier period on
the biased polarity); (c) polarity-aware persistence depth. Then the
ladder should climb through the former death band.


## Parity-bias kill list (2026-07-18 late) — what it is NOT

Systematic single-variable experiments, one ladder each, bias
measured in the 150-350us band every run:
- **Persistence spacing** (delay(8) spaced -> AM32-verbatim
  back-to-back reads): bias +51us — NOT detection-window width.
- **Scheduling elapsed-feedback** (per-window measured elapsed ->
  AM32-verbatim fixed overhead, no feedback): bias +43us — NOT a
  scheduling limit cycle.
- **Parity compensation** (backdate even accepts): bias +54 -> +8us,
  tracking err 8.4 -> 3.4% (mechanism arithmetic confirmed: comp
  moves bias by 2C) — but deaths persist as SAG kills: the bb shows
  even-window accepts still clamping at the d=2 floor = the even ZC
  OBSERVATION stays ~50us late; compensating the timestamp cannot
  un-brake a physically late commutation.
- Earlier: mode-3 vs mode-0 EXTI (identical deaths), blank 0 vs 8
  (identical), CPU/TX/main-stall class (all fixed, deaths unchanged).
- **AM32 same rotor/comparator/divider: bias +1.4/-0.1us (ZERO).**

Remaining candidate (right magnitude ~1 carrier, right parity):
**actuation asymmetry between hi-role-flip and lo-role-flip
commutations** (HIGH:[0,0,1,1,2,2] flips the PWM leg on alternate
steps; an incoming AF leg connecting mid-carrier waits for the next
wrap = up to 41.7us of lost drive on alternating commutations).
Next-session tools: WAXWING phase-voltage scope at both commutation
classes (X key enables rings), and the AM32 structural diff of their
leg-connect timing. The 60->80 ZT reproducer CANNOT complete until
this is closed - everything instrumentation-side is done and clean.


Final cut (per-sector, 150-350us band, two independent runs): the
bias is a PURE PARITY SPLIT — all three even sectors +20..+30us, all
three odd -19..-28us, identical across phase A/B/C float windows.
Phase-independent => not per-pin analog, not the CHAMELEON mux, not a
per-leg timing quirk. A polarity-pure, ~+-0.6-carrier, phase-blind
offset that AM32 does not have on the same silicon. The hi-flip/
lo-flip actuation classes are parity-aligned and remain the prime
candidate; the WAXWING scope shot of both commutation classes is the
decisive next experiment (X key first).


Seventh experiment (ADC comparator-node loading): dropping ch9/ch10
(PA4/PA5) from the injected burst moved the bias +51 -> +36us —
partial at best, within run-to-run spread; REVERTED (it also blinds
WAX/confirm infrastructure). The parity bias survives every code-side
lever available tonight. HANDOFF: the phenomenon is fully bounded
(polarity-pure, phase-blind, ~+-25us, carrier-scale, AM32-zero on
identical silicon) and instrumented; the decisive next experiments
need the scope (WAXWING at both commutation classes, X key) and/or an
AM32-side ZC_TRACE with their comStep timing stamped. The flashed
build is the committed HEAD state: b2b persistence, AM32-verbatim
scheduling, parity comp disabled, all guards + flight recorders live.
