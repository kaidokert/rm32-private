# Candidate improvements for rm32 integration

## E803-E804: foldback authority must scale with excursion severity

A fixed tiny foldback can be correct but dynamically useless. At38% duty,
E803's first complete current block reached1.853x the nominal3.5A raw allowance.
The experimental1% governor published38->37 correctly, but the independently
averaged bus then stayed below8.4V for a complete~11.3ms block and safed. A
small actuator chosen to avoid losing envelope margin did not protect the power
path from a severe transient.

E804 retains the first-over residual and selects a bounded1..5% foreground
reduction using integer cross-product bands. DMA remains measurement-only;
foreground owns the coherent duty write; the learned ceiling stays monotonic;
and the second unacknowledged over-window remains terminal. The exact bands and
3.5A setting are bench policy, not values to copy. The portable rule is:
current-limit action needs magnitude (or a real PI/PID controller), not merely
a boolean warning plus a universally small decrement. Never weaken bus,
nFAULT,tracking or hard-current stops to make foldback appear successful.

## E791-E796: PWM-asynchronous current averages need designed phase coverage

Calling samples "asynchronous" is insufficient. At20kHz,201us advances by1us
per sample and a50-scan block covers the carrier well. At24.006kHz,209us advances
only46/2666 timer ticks; one block traverses a mostly contiguous portion of the
PWM waveform and can bias the protection result. Choose cadence from the actual
integer timer geometry. For this rig,226us yields50 unique phases with a maximum
uncovered arc of60ticks (<1us), while reducing DMA rate. Make this property a
compile-time/host test. Do not copy226us as a universal constant.

Individual phase ADC rails and individual low-bus samples occurred during clean
operation. Treating either as DC current or sustained sag manufactured false
walls. Retain phase-rail and low-bus counters, but apply phase-distributed block
averages; keep VREF validity and hardware nFAULT immediate. A full-window bus
average owns sustained sag. Conversely, an aggregate average must not overrule
nFAULT: E796 asserted the driver fault with a simultaneous2.16x raw current
residual while the bus stayed healthy.

The protection layers answer different timescales: driver VDS/VSENSE/GDF/UVLO
act in hardware; block current gives foldback and terminal policy; bus averaging
detects sustained supply collapse; tracking guards detect control loss. Record
which layer won. Never raise a software threshold merely because a hardware
fault occurred first.

## E777: current protection needs warning, policy, and hard-stop layers

A complete-block average-current comparator should not write PWM from the DMA
ISR. E777 publishes a latched first-over warning, applies the 5% reduction in
foreground through the existing coherent live-duty transaction, and retains
the second-consecutive-over hard stop. The learned ceiling is monotonic for the
powered session and also caps restart restoration, so no independent controller
can silently undo derating. This ownership split is portable to rm32; the bench
1.5 A nominal threshold and 2 s restoration pacing are not.

The E778 source audit found that main rm32 already implements the production
version of this split: its continuous current PID runs in main, publishes
`current_limit_adjust` atomically, and TIM6 applies it as the final duty ceiling
after requested duty, ramping, duty maximum, and stall boost. A regression now
proves that neither a high command nor positive stall boost can override the
published current ceiling. Therefore the correct port was evidence and a test,
not a second governor; the bench's fixed 5% step and no-release session policy
would degrade the existing closed-loop controller.

## E776: rm32 already has the bounded restoration mechanism

The E775 policy was traced through main `rm32` before adding another controller.
Confirmed tracking loss already requests `AllOff`, returns to `Armed`, preserves
the receiver request as data only, and enters ordinary startup on the following
10 kHz tick. `DutyState::start_motor` resets applied duty to `min_startup`; the
existing per-tick ramp then bounds every move toward the current request. A new
receiver request naturally supersedes the old one because every tick recomputes
the setpoint before applying that same ramp. The low-zero-cross reclimb ceiling
adds another limit while lock is unconfirmed.

A new sequence-level regression test proves: no bridge authority on the AllOff
tick, retained command unchanged, normal startup on the next tick, first applied
duty bounded to 122 from the default120 floor, and a newer low command replacing
the retained high command through the ramp. The complete host suite passes363
core +7 harness tests. The release G071 build remains opt-s/thin-LTO/codegen1;
the emitted motor-root arithmetic audit still passes with the frozen ELF hash
`64B67879...D447E47`. The audit workflow now explicitly re-audits cached final
ELFs and embeds their SHA in the sidecar; this closed a stale-sidecar provenance
hole. No bench fault injector or2s bench pacing constant was ported. Main-rm32
powered qualification is still outstanding.

## E775: separate tracking detection, safing, restart, and throttle restoration

A practical recovery path is four policies, not one ISR trick:

1. the tracking detector requests the existing all-off transition;
2. only tracking faults are restartable (current, bus and nFAULT remain
   terminal for that attempt);
3. ordinary low-duty startup establishes a fresh BEMF handoff;
4. foreground restores the last *acknowledged* request through the normal
   guarded live-duty writer, in bounded steps. A newer host request supersedes
   the retained target.

On G071, 5% every0.5s restored30% but later hit average current. Keeping the
same steps and current limit while pacing them every2s produced a25% recovery
3/3 cohort and30% recovery2/3. The failed30% run is retained as a current
margin result. This is portable policy evidence, not a magic2s constant.
Production rm32 should make ramp rate a control parameter and use calibrated
current feedback/governor behavior rather than copying the bench number.

Fault injection must preserve the question being tested. Suppressing BEMF at
high duty holds one sector until the tracking watchdog expires; here the
average-current guard correctly won first. That test proves protection
ordering, not restart. An immediate reason8 injection separately tested the
post-decision recovery policy. Keep both tests; never turn an electrical fault
into a restartable tracking label merely to exercise recovery.

The last requested duty must be captured before controller reset clears live
state, but it grants no output authority. Startup remains low duty, all writes
flow through the coherent guarded PWM transaction, and failure to prepare or
apply a step safes the bridge. E775 implements and tests these invariants.

## 2026-09-15 — main rm32 M0 arithmetic gate landed

The G071 lesson is now code in `../rm32` and `../rm32_stm32`, not just campaign
advice. Main rm32's emitted M0 firmware had seven soft divisions reachable from
the motor IRQ roots, including the normal 20 kHz PWM compare. Exact bounded
32-bit helpers removed all seven without changing the mapped control results.
Exhaustive domain tests pass, as do 362 core and 7 harness tests.

`rm32_stm32` now builds release with opt-s, thin LTO, one codegen unit, and a
link wrapper that rejects forbidden division/remainder, wide-integer, and
software-float helpers reachable through emitted direct calls from the motor
vectors. Current G071 result: TIM6/TIM14/ADC_COMP/DMA all zero. See
`../rm32_stm32/M0_ARITHMETIC_AUDIT.md` for the exact build, SHA, bounds, and
audit limitations.

The same integration also changes confirmed BEMF-timeout handling in main
`rm32`: it now requests `AllOff`, returns Running/OldRoutine to Armed, resets
the startup interval, and lets persistent throttle enter the ordinary startup
path on the following tick. It no longer keeps blind-commutating via
`CommutateKick` after confirmed tracking loss. Unit tests prove both halves of
the handoff (timeout decision and AllOff-then-normal-start sequencing); this is
not yet a powered qualification of the main rm32 image.

## E657: release ADC ownership on every foreground return

DMA cleanup only at the normal bottom of a controller function misses early
arm/refusal returns. Physical gates-off is necessary but does not restore ADC
trigger/channel/DMA configuration for the next foreground owner. Wrap the
controller body so every result safes outputs before bounded foreground ADC
restoration; retain normal cleanup before passive capture. Never put ADCSTOP
wait loops into an ISR. E657 verified an actual arm refusal, then correct bus
ADC reads and a successful10s restart without resetting. This covers that
sequence, not every failure class. Source/test/artifacts: HIGH_SPEED_RECOVERY.

## E648-649: recovery setup must preserve sensing continuity and edge age

Next-edge recovery needs two distinct budgets: continuous comparator sampling
through setup, and the remaining delay from the real edge to commutation.
Moving a phase to the end of a scan merely moved a100us sampling-gap refusal
upstream (E648). Keeping the original scan and adding a real successor read
bridged that transition without resetting its EdgeFilter. A retained context
borrow avoids copying acquisition history across a time-sensitive boundary;
ISR cancellation must only revoke authority, never mutate borrowed storage.

Then prepare interval history/advance/wait from the provisional seed BEFORE
the final setup checkpoints, rather than after they can have captured a real
edge. E648 reached that edge but refused31us remaining against32us required.
E649's first powered recovery completed after moving seed-only initialization
earlier, with43.5us remaining and7us arm work. This is an ordering lesson, not
a portable latency guarantee or permission to refresh edge timestamps. Keep
actual edge age, guard authority and the original overall deadline through
every checkpoint. Higher-speed recovery still needs its own evidence.

## E625-626: separate timely safety publication from foreground statistics

On G071911A, coherent ADC DMA scans now pass immediate phase-current and
complete bus/VREF/acquisition-age validation in the DMA handler. Foreground
still drains every FIFO entry for statistics; it must not replay an older
timestamp into the already-updated guard. The independent1ms freshness limit
and FIFO-overflow stop remain. No latest-only dropping or cached refresh.

Evidence: E623 had fresh226us-old samples queued while foreground-delivered
guard feedback was1030us old. E625-626911A completed3/3 thirty-second20%-capped
live-ramp campaigns, ending924-927eHz, without stale feedback. DMA handler
maximum52us on the first campaign, not a portable WCET guarantee. Moving work
into DMA changes scheduling and adds two soft divides there; do not transplant
without auditing interrupt priorities, preemption, handler cost and lease margin.

Safety publication counts and foreground sums have distinct endpoints. Declare
unaggregated tails explicitly (1/1/0 frames in this cohort), keep CRC/time-prefix
checks, and do not claim complete coverage or calibrated current from raw sums.
Immediate current refusal currently bypasses peak-summary recording: E625's
printed873count peak excluded the actual rejected A3612/C434 scan. Retain fault
samples separately when integrating so outcome summaries cannot hide them.

Another portable measurement lesson: E623 moved a masked live-duty stopwatch
inside PRIMASK entry/exit. An outside bracket includes unrelated interrupts;
it is not the critical-section cost. Keep a separate real-time deadline guard.

These are candidates, not rm32 changes. High-duty recovery and parity remain
unfinished; the23% current/timing excursion is retained, not explained away.

## E611 candidate: specialize known lifecycle modes at compile time

`powered_timer::start_inner` now takes const PREPARED/REENTRY, replacing the
runtime Option<Limits> distinction for its three private callers. Initial
calls still calculate original startup budgets; recovery passes original
elapsed/campaign/segment. Live ownership/output/feedback checks stay at use.
Do not turn a compile-time mode into permission to cache mutable safety state.

On G071 release-s/thinLTO32B1, one7% recovery measured guard-start10us
versus13-14us in earlier captures; edgeage76us versus80us uninstrumented.
This is an observed, build/regime-scoped gain, not paired WCET or portable
cycle-count promise.36 guard policy tests and2 source/mode-algebra tests pass;
hardware disabled gates and one recovery pass in E611. Higher-duty recovery
and repeatability still need evidence. General lesson: const mode selection
can unlock useful inlining even after arithmetic helpers have been removed.

## E597 inventory update: preserve semantics, measure the whole path

- E588: replace the three-channel absolute-current predicate with the exact
  inclusive raw range848..3248 (only for the same2048offset/1200limit).
  Exhaustive65536values per channel pass; emitted M0 code removes iterator,
  absolute arithmetic and temporary sample array. Observed guard-start13us
  versus15us, IRQunion51.588% versus52.115% near335eHz; not paired WCET.
  Do not copy these raw thresholds into a differently calibrated board.
- E584–585: one common feedback-age clock reading after the raw/timestamp
  snapshot replaces five reads. Original timestamps,200ussetupreserve and
  1000uslimit remain; exhaustive wrap/boundary tests. Observed edgeage81us
  versus84us. This is not permission to relabel acquisition times.
- E586–587: const PREPARED specializes guard startup, reducing symbol/stack
  size, but gives no measured handoff gain. Keep size benefit separate.
- E589–595: moving only cold sector/status stores before edge12 did NOT
  improve age (81us versus80us). Feature retired from campaigns despite
  successful lifecycle/consumption tests. Do not port it as a speed win.

These are binz results, not proof of performance parity with archived minz.

## E539 consolidation: M0 arithmetic and experiment boundaries

Two later candidates belong in the port inventory, not just the lab chronology:

- **Compute invariant carrier compare only when preparing the segment.**
  M0_ARITHMETIC_SWEEP.md E388–389 retains exact duty validation and refusal
  precedence, computes the original compare on the initial CCR load, and
  verifies emitted steady-path branching around the helper. E389 has one
  original30s recovery near315.8eHz; commit maximum41us versus45us in the
  comparison records. Not an isolated cycle-cost measurement or broad cohort.
  Port the prepare/invalidate ownership contract, not a stale global cache.
- **Prove bounded arithmetic and inspect its actual placement.**
  SEED_DIVISION_EXPERIMENT.md E374–378 documents exact-domain reciprocal
  multiplication for the measured12-interval mean, const-derived constants
  and proof assertions, plus an exact exceptional fallback. E375's source
  conditional still emitted unconditional division; E376 moved the fallback
  behind the actual branch. E377 measured a5us shorter acquisition bracket
  in one comparison; E378 lean recovery passed. Port only with the same operand
  bound, invalid-input behavior, exhaustive tests and target disassembly audit.

These are documented candidates, not completed rm32 integration. Existing
reference-build instruction addresses and timing numbers are historical and
must not be asserted for a newly linked port.

Do NOT port the E537 noinline comparator-read variant as a fix: its6.9% hold
failed CycleTiming after8.804s and it was retired. Preserve fresh volatile
reads and qualify the read-count filter's time aperture after optimization.
Do not transfer diagnostic-build qualification to another binary hash, or
substitute a guard threshold change for evidence of correct tracking.

## E357-361: specialize invariant comparator modes, never its signal

Caching runtime modes per invocation still left source/trace branches inside
the release persistence loop. A statically specialized real/inverted/traceoff
adapter removes those branches while retaining every volatile comparator read
and the unchanged shared AM32 routine. All control/safety methods delegate to
the original live implementation; diagnostics use the original adapter.

G071 release/s/thinLTO at6.4%/~293eHz: matched hold IRQ58.774->54.782%,
COMPmax71->65us,COM69/commit45 unchanged.3/3original30s recovery campaigns
pass withIRQ54.818..55.199%.6.5/6.6% hold+recovery also pass. Candidate text
cost+492bytes versus archived baseline; data/bss unchanged (not isolated to
this edit because archive source predates other changes).

Do not claim unchanged analog filtering: shorter sample spacing changes the
persistence aperture.6.7% still trips the exactcyclefloor at3124<3125us; this
is a workload improvement, not a speed-limit fix. Port requires board-specific
mode invariants, cancellation/ownership behavior and measured qualification.
Not yet integrated into rm32. See STATIC_COMP_EXPERIMENT.md and cohortCSV.

## E340-341: preserve comparator/commutation peer priority

The frozen minz example uses equal comparator and COM priority. Binz had
allowed COMP to preempt the tail of COM immediately after COM re-enabled
sensing. Equalizing these at64 while retaining independent guard/DMA at0
reduced the measured COM wall maximum from258us to75us on the unfiltered
TIM2 adapter. Disabled pending tests demonstrate old nested versus new
tail-chained ordering. One10s hold and3/3 original30s injected-loss/recovery
campaigns passed at6.2%/~280eHz; see peerprio_reentry62_30s_cohort.csv.

This is a worst-case scheduling improvement, not an aggregate CPU reduction:
the recovered segments still measured63.30..63.45% IRQ union. Recovery arm
margin was only2..3.5us above the unchanged32us floor. Carry the priority
relationship, not the reference's lower safety-guard priority, into a port;
requalify on the lean EXTI path before making broader timing claims.

## E300–303: select an appropriate atomic backend at the application boundary

On this privileged single-core G071, portable-atomic/critical-section inserted
PRIMASK transactions even around ordinary loads/stores. The library-supported
unsafe-assume-single-core backend removes those for native-width loads/stores,
retaining RMW exclusion and all explicit multi-field critical sections.
Audit transitive Cargo features: the HAL also forced the expensive backend.
Do not enable this assumption in a general shared library or on multicore/
unprivileged targets. See ATOMIC_BACKEND_EXPERIMENT.md for exact local HAL patch.

At6.2%/~279eHz with identical path instrumentation, measured IRQ union fell
70.126->62.471%. Then3/3 thirty-second injected-loss/recovery runs passed near
280eHz, IRQ62.44..62.79%, cycle sigma35.09..35.19us, unchanged guards.
This is a qualified platform-specific candidate for rm32, not a completed port,
full-duty feasibility proof, or a claim that every atomic can become volatile.

## E295–296: inspect generated comparator-read code, then qualify timing

OnG071 release/s/thinLTO, the cached comparator adapter still had a per-read
function call. Explicit adapter inlining removes it while retaining a fresh
volatileCSRload eachiteration. No sharedminz-core/filtercount/polaritychange.
Matched~279eHz10s cycle sigma58.945->43.983us; COMPmax87->81us and
IRQunion68.539->67.260%. Then3/3 thirty-second injected-loss/recovery runs
passed at6.2%/24kHz/~279eHz, originaldeadline andallguardsretained.

Portablelesson: read-count persistence depends on generated-code cadence;
inlineannotationalone isnotproof. Verifyassembly/freshloads and requalify
noiseacceptance, timing, startup/recovery andflash/stackcost onthetarget.
Thiscost380textbytes betweenE294/E295. It is not a demonstrated cure for
the earliernear300eHz failure, nor justification to reducefiltercounts.

## E285: comparator adapter and fault observability candidates

- Cache only source/polarity/tracing mode per controller invocation when those
  flags are immutable during the call. Never cache comparator level, pending
  state or safety state. E282 measured shortest traced qualification bracket
 37->29.5us; E285 passed10s/30s recovery near291eHz. Short-cycle failures still
  occur at5.5%; this is not a complete reliability fix. Read-count filters have
  platform- and instrumentation-dependent time apertures.
- Retain a bounded IRQ tail with full-width timestamps and CRC. E280 captures
  switching-related persistence rejections near faults that a startup prefix
  missed. Tail collection adds runtime cost; qualify it separately.
- Capture rejected-event controller state only after physical safing, before
  the callback returns. A12-read qualification pass can still be refused by
  the independent safety guard; an absent accepted-record increment alone
  does not identify a persistence rejection.
- Keep observation, interval-counter and guard clocks separate. Join only
  matching-clock records; summed reference intervals are not independent rotor
  periods. Preserve pre-injection failures as such, not recovery failures.

No sibling rm32 changes made. These mechanisms need destination-specific
timing/ownership validation; neither local register addresses nor current
experimental speed limits are portable constants.

## E263: DMA startup, run identity, and measurement semantics

Port candidates only; no sibling rm32 changes. Reproduce destination register,
timing and safing evidence rather than copying G071 addresses or thresholds.

| Mechanism | Evidence | Contract to preserve |
| --- | --- | --- |
| Separate first-trigger delay from steady ADC cadence | E253101us overload; E257 expired initial feedback; E258101us-first/201us-steady10s and30s recovery | Exact first and subsequent acquisition timestamps; no timestamp refresh or weakened age guard |
| Clear sums on staged run-statistics reset | E255 same-boot failure retained4974 old rows; corrected hold and subsequent restart | Every entry path resets segment identity before acquisition, including paths bypassing ordinary prepare |
| Sequential timer ownership | E258 forced entry, release, ADC stream, stop and recovery restart | Refuse active old owners/requests; mask stale vectors; stop producer before ADC restoration |
| Driver-wake baseline identity | E250 initial match; E251/E258 recovery invalidation | Physical shutdown first; serialized GPIO/metadata; every low revokes tokens; identity is not calibration |
| First-delivery diagnostic | E257 initialage674us expired; E258 previousage943/936us accepted | Distinguish frame production from guard delivery; retain previous/acquisition/decision timestamps |
| Trigger-phase DMA instrument | E260 route3x32 and powered1s; E26230s/149253scans | No extra interrupt, but ADC-handler max17us versus11 without it; qualify load/latency and retain pre-FIFO count semantics |

E261 measurement correction: nonuniform counter bins do not prove sampling
bias when commutation resets PWM. Flat-bin weighting can bias a time average.
The explicit-interval `pwm_time_occupancy.py` model is not missing-event
reconstruction. `drv_baseline_residual.py` refuses recovery baseline reuse and
reports raw counts, never amps. Preserve aperture/sector/drift limitations.

Most-instrumented build: E26230s initial hold234.44eHz,stack4420 untouched.
Preceding no-phase-histogram build: E25810s/30s recovery~234eHz,stack4376.
Older foreground5.3% recovery~285eHz is not current-profile qualification.
Attach feature set, ELF hash and captures; all use release/s/thinLTO.

E263 source archive2799b284...128files again matches selected live dependency;
historical FALCON archive94df4240... hashes and metrics reproduced. Neither
proves FALCON used the frozen AM32 bytes. No reference hardware rerun required.

Status: consolidation of implemented binz mechanisms, not an rm32 port or a
claim of minz parity. Preserve the full shared minz-core control sequence.
Bench evidence is in controlboards/BOOSTXL-DRV8304H/LAB_REPORT.md; raw captures
and offline validators remain the authority for individual passes/failures.

## E238 update: measured G071 entry, recovery and timing improvements

These are port candidates, not changes applied to the sibling rm32 tree.
Reference source revalidated with freeze_minz_reference --check-source: all128
selected files match the original archive SHA2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44.
No reference-board rerun was needed or performed.

| Mechanism | Evidence | Invariants to retain on a destination |
| --- | --- | --- |
| driven_run opaque Transfer and completed-ADC-scan yield | E225–229; under-drive entry and original-budget recovery | Keep original edge/feedback timestamps, revoke ownership generation on stop, discard partial ADC scans, never grant gates merely because data is fresh |
| driven_seed one fresh window after a forward missing epoch | E237 reentry_stage50_01 actually reanchors at epoch3 then hands off and recovers | At most one restart, discard all old interval/cycle history, require twelve new intervals, preserve original acquisition deadline, never refresh a completed seed; retain raw gap evidence |
| powered_timer early recovery statistics preparation | E237 saves15us; three5% recovery passes near266eHz | Place reset AFTER acquisition's final shared shutdown but BEFORE new measurements; preserve Tracking reason/first archive; consume preparation once and revoke on every stop, including inactive stop; fresh feedback/guard/seed still required |
| irq_union software-boundary accounting | E234–238 powered records, roughly52–54% IRQ union in sampled runs | Serialize timer reads/state; nested time counted once; separate foreground from idle; preserve protocol identity; measure probe cost; no claim of corrected total CPU utilization or arbitrary-blackout detection |
| Separate initial-arm archive and recovery-arm diagnostics | E229/E237 strict original deadlines; E238 late5.2% recovery refusal | Never use a resumed CORESEED to certify the initial transfer; retain first fault/events and one absolute campaign deadline; report failed attempts separately |

The E238 setting of5.2% duty sustains278eHz for10s
but refuses recovery at28.5us remaining versus32us required. Do not port a
smaller arm floor to erase this measured limit. At5.0%, three10s campaigns
recover with34.5..36.5us and a separate30s hold passes. This distinguishes
sustained capability from a repeatable recovery envelope.

Latest measured untouched stack is4856bytes on the instrumented/reanchor/
staged-recovery build, not the historical1520byte figure below. It is still
observed high-water evidence, not worst-case proof. Build release/opt-level=s,
thinLTO, onecodegenunit; changing optimization requires timing requalification.
Keep G071 register mapping, timer allocation, interrupt priorities and DMA
ownership board-specific. Minz control decisions remain in minz-core.

## Portable policy and evidence components

| Component | Reusable mechanism | Constraint that must accompany a port |
|---|---|---|
| examples/support/powered_guard.rs | Latched electrical/tracking faults, finite campaign and segment budgets, bounded one-attempt recovery | Current/bus/age thresholds are rig-specific; no fault automatically grants retry |
| examples/support/flying_acquire.rs | Ordered fresh physical intervals, dwell-qualified onset, rolling full-cycle checks, seed-expiry refusal | Timer units, speed envelope, mux settling, dwell and arm margin require target validation |
| examples/support/sampled_clock.rs | Extend a sampled 16-bit counter using elapsed differences | Must sample within every hardware wrap; serialize hardware read with state mutation; not a blackout watchdog |
| examples/support/event_stats.rs and event_timeline.rs | Bounded whole-stream moments and time-resolved accepted-event diagnostics | Accepted events are not an independent rotor/zero-cross denominator |
| examples/support/segment_archive.rs | Owned first-segment evidence, refusal to overwrite, separate reset epochs | Stop all writers before copying; stack cost and separate clock origins remain explicit |
| scripts/drv_sustained_report.py | Preserve failed attempts; verify CRC, post-run safing, deadlines, order, recovery and stack gates | A completed powered window is not a lock or current-parity certificate |

## Hardware/integration fixes to reproduce as invariants, not copy blindly

- Restore the complete intended PWM configuration on every sine restart.
  E124 found retained six-step CCMR/CCER state; this was deterministic state
  contamination, not engagement variance. Keep shell-sine as the reference.
- Clear pending comparator events in the correct peripheral/EXTI/NVIC sequence.
  E094 fixed an interrupt storm. Audit actual target interrupt-latching semantics.
- Validate authority and apply the gate vector in one short serialized operation.
  A late COM must not resume after shutdown and re-enable the bridge. E152's
  disabled guardcheck exercises refusal without granting motor authority.
- Freeze observation and append accepted records in serialized transactions.
  E150 retained a record35us after observation end; E151 fixed that race.
  recordcheck must demonstrate stopped callbacks cannot alter the frozen log.
- Keep seed setup out of the narrow final arm budget. E149 saved a mux revisit;
  E155 moved timeline initialization after actual timer programming. Both retain
  qualification dwell and seed expiry, rather than accepting stale rotor phase.
- Separate long-lived current/voltage safety feedback from precision metrology.
  Current ADC launches, current peaks and an800mA PSU setting do not establish
  mean supply current. CSA zeros can shift after every ENABLE wake.

## M0-specific constraints to retain

Current E162 release: stack address span4108bytes; powered recovery capture
launch45_reentry30_01 reports1520bytes untouched. Neither number is a proof of
worst-case nesting. E141's large inlined archive temporary caused real corruption;
retain non-inlined stopped-state copying and remeasure after layout changes.
Do not reduce the stack gate just to fit additional telemetry.

Guard timer has higher priority than COMP/COM. Keep bounded ISR work, no UART
formatting there, and actual elapsed-time gap checks. TIM17 has no DWT substitute
and its sampled extension cannot see an arbitrary missed65.536ms wrap.
DRV ENABLE/low-gate safing is not interchangeable with the old EVL stage module.

## Port acceptance sequence

### E186 addendum: coherent ADC delivery and evidence accounting

This is a port contract for the experimental E180–184 path, not permission to
replace rm32's ADC driver or a claim of calibrated current. Normal binz firmware
still uses the original foreground ADC reader. No sibling files were edited.

| Reusable source | Preserve on the destination | Required evidence |
|---|---|---|
| dma_snapshot.rs | Validate DMA half ownership before AND after copying; refuse ambiguous completion flags/remaining count | Host lease tests, then a real delayed-consumer refusal on the destination DMA |
| scan_queue.rs | Copy into owned frames; retain acquisition stamps; full queue latches failure, never overwrite; clear only for explicit new acquisition | FIFO wrap/order tests and real full-queue shutdown followed by a separate same-boot restart |
| powered_guard.rs::feedback_aged | Guard acquisition age, not delivery time; late fresh data cannot repair a missing-feedback gap | Cached/reordered/late delivery, electrical limits and wrapping-clock tests |
| current_sums.rs plus powered_timer.rs::feedback_inner | Accumulate only the exact delivered-feedback decision, including a concurrent stop; wide signed sums and explicit segment reset | CRC/count/timestamp validation; preserve E183's rejected one-extra-sample capture as a negative case |
| drv_fifo_fault_check.py | Validate fault/reuse captures including finaloff; same-boot identity still needs bench provenance | E184 retained pair, not merely a passing policy test |

Board-specific implementation that must NOT be copied as generic policy:
TIM3 TRGO every101us, ADC scan0/1/4/6/13 reordered into logical4/1/0/6/13,
DMAMUX request5 and DMA1 channel1. DMA IRQ copies one coherent scan and checks
raw phase limits before enqueueing; foreground performs voltage conversion and
aggregation. DMA IRQ and independent TIM6 guard share priority0, above COMP
and COM. This priority assignment is part of the measured timing result.
Mask/unpend the producer IRQ before setup/stop and before foreground ownership
of its state. Stop bridge authority immediately on a stream fault; defer bounded
ADC-stop waiting and register restoration to foreground. Do not wait for ADC
shutdown inside the high-priority ISR.

Measured scope: E181–182 each retain3/3 thirty-second dropout/reentry campaigns
near205 and238eHz with DMA ISR maximum10us and queue peak4/8. E183–184 add sums:
experimental stack span4124bytes,1600 untouched on the30s recovery capture.
Optional IRQ prefix was reduced to24 records (normal32) to preserve stack span.
E184 fault injection fills8/8,stops at~2.021s,then a new same-boot run completes
10s near205.13eHz. Fault-path IRQ maximum22us includes shutdown. These are
observed maxima,not a worst-case nesting proof. At~238eHz FIFO cycle sigma~27us
versus~21us for the earlier foreground cohort; do not claim zero timing cost.

The `bench-dma-stall` feature deliberately withholds consumption once per boot
after20000 scans. Keep it explicitly test-only; it is not normal recovery logic.
S85 current sums remain uncalibrated and final-segment-only. Acquisition
coherence does not prove PWM/sector coverage,valid same-wake zero or unbiased
mean supply current. E185 confirms handoff/reentry both cycle ENABLE.

Revalidation on E186: frozen128-file reference still matches the live selected
source;111Rust tests (89unit,1recorded,21sequence) and108Python tests pass.
The tests cover policies and replay,not a matched reference motor benchmark.

### Destination sequence

1. Run the same pure policy/reference-sequence tests on the target host build.
2. Implement target HAL/register semantics and verify outputs-off behavior and
   stopped-callback refusal with no drive authority.
3. Verify observe-only comparator/timer timing, polarity and measured seed age.
4. Repeat bounded known-point powered holds and injected tracking-loss recovery;
   retain every attempted entry, independent coast evidence and final safing.
5. Establish independent quality/current metrics before claiming matched parity.

No sibling rm32 or minz files were changed for this consolidation. The frozen
128-file minz archive and live source were reverified during this work:
2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44.
Historical FALCON capture provenance does not establish a measured baseline of
that frozen AM32 revision; the disconnected minz bench remains a parity gap.

### E208 prototype: PWM-triggered comparator capture without a sampling ISR

G071 TIM1_CH4 compare requests can drive DMA1 CH2 reads from a selected APB
register. First qualify the read timing using TIM1 CNT as source,with real
foreground ADC contention:64samples atCCR4=192 and64at320 all arrived4..6
timer counts later (64MHz),well inside declared32count/0.5us bound.
Then retain full COMP2 CSR words atCCR4=192. A20ms200eHz driven interval
captured216words without a per-sample ISR. Raw mux/polarity/configuration bits
permit host checks; DMA indices before/after physical commutation identify
ambiguous boundary samples instead of silently assigning the wrong phase.

This is an instrument prototype,not a production BEMF qualification win or
a measured CPU-utilization percentage. Motor-ISR-contention aperture has not
been directly bracketed: CNT timing was measured disabled with ADC traffic.
Under-drive samples still do not form a complete qualified sequence. Port
only the method after target DMA-access/request mapping,finite-buffer bounds,
ownership revocation,read timing and comparator semantics are revalidated.
No rm32/minz files changed. No reference-hardware rerun is required by the goal.

### E669: audit aggregate copies as well as soft arithmetic on M0

In the frozen B1E54 recovery build, a nested Result/Option combinator sequence
in NextEdge::edge emitted two eleven-byte unaligned __aeabi_memcpy calls after
edge confirmation. Explicitly destructuring Seed fields in the match and
reconstructing the result removed both calls in frozen followscalar_669 (8DD2)
and reduced that function's stack allocation44->28bytes. Acquisition checks,
history updates and result semantics remain tested. Inspect generated code
after each compiler/profile change: source-level Copy does not imply a cheap
aligned transfer, and helper audits should include memory routines, not only
division/multiplication. This is a code-generation win, NOT a measured complete
handoff WCET or higher-speed recovery qualification. E669's8% recovery refused
its unchanged arm margin. No sibling rm32/minz code changed.

### E755–762: signed current, normal restart, and lean qualification

- Follow the current-amplifier equation, not an assumed sign. DRV8304 positive
  bridge return is `(VREF/2 - VSO)/(gain*R)`. Keep regeneration signed; never
  turn ripple into fictitious load with an absolute value.
- Define average-current persistence in physical time. Two complete10.05ms
  blocks must exceed the physical target; one good block resets persistence.
  ADC rails and independent electrical faults remain immediate.
- On tracking loss, safe outputs first and restart through normal startup. Use
  one attempt, acquire a fresh same-wake current zero, and subtract disabled
  coast plus complete startup from the original deadline. Do not make flying
  seed recovery an envelope gate or silently issue a fresh run budget.
- Qualification and instrumentation are distinct images. A lean functional
  verifier can require deadline completion, control progress, fresh event/ADC
  ages, no veto and final safing without pretending omitted qZC/sigma exist.
  Detailed timing/event distributions remain diagnostic-build evidence.

These are portable control/policy wins. GPIO, ADC ordering, comparator mux,
timer/DMA routing, physical current constants and exact startup waveform remain
board/HAL-specific and must be requalified when folded into rm32.
# E763 — calibration honesty and transitive M0 arithmetic checks

- One simultaneous loaded PSU/ADC point is enough to establish an engineering
  scale and its uncertainty. Keep the distinction between supply DC and
  sampled bridge return; do not turn one point into a calibration curve.
- An exact-caller objdump check is not an ISR audit. Check forbidden soft
  arithmetic transitively from each vector through emitted direct calls, and
  state that indirect calls and inlined wide arithmetic remain outside it.
- A clean commutation vector does not exonerate periodic ADC and startup
  vectors. On M0, audit all live interrupt roots for each production mode.

# E764–E765 — raw-domain protection and foreground preparation

- Protection decisions do not need engineering-unit division in an ISR.
  Compare bounded ADC ratios directly, prove the conservative relationship
  against the old rule, and leave unit conversion to foreground diagnostics.
- A calibrated pulse clamp must use the channel's same-wake zero. Absolute
  raw distance from a guessed midscale is wiring/offset dependent and becomes
  another arbitrary envelope wall.
- Prepare duty-dependent scale in foreground. For bounded constant divisors,
  use an exhaustively proven shift/add identity; here `/255` is exact and the
  factored PWM differs from the old formula by at most one conservative tick.
- Enforce arithmetic cleanliness transitively from every critical vector at
  link time. Generating an advisory objdump report is not enough.
- Remove all shadow copies of a retired guard. E765's one startup failure was
  a second `+/-1200` check outside the main guard module; the system behavior
  only changed after both copies stopped owning shutdown.

# E797 — separate acceleration demand from steady duty

- E791–E796 made35% look like a current/nFAULT wall while the host raised duty
  in5% steps every0.5s. With identical electrical/tracking protections, E797's
  1% steps every0.25s let35% hold30s near1.56keHz; the maximum50-scan current
  residual was only67.5% of the same2.5A nominal allowance.
- A smooth40% ramp crossed that allowance by just2.2% and folded safely to35.
  Always report ramp slope/step with duty, and distinguish acceleration-current
  failures from steady-state limits. First bound command slew and retain the
  coarse-ramp failure; do not raise a threshold to hide a command transient.
- Current foldback should be a monotonic ceiling, not a single cliff. E799 uses
  1% downward steps and rearms only after the lower PWM value is coherently
  published. If the producer completes a second over-limit block first, the
  hard stop wins and stays latched. This preserves fail-closed response while
  allowing repeated foreground regulation without an ISR-side controller.

# E860-E861 - record decision inputs, not lookalike quantities

- A field with matching units is not necessarily the controller quantity.
  EV_ACC `this_zc` is the newly measured interval; `average_interval` is built
  from six sector-indexed, smoothed values. Replaying a decision with the first
  in place of the second produced precise-looking but invalid trip points.
- Version diagnostic wire formats when semantics change. Preserve historical
  decoding, name measured and averaged values distinctly, and refuse analyses
  whose required field is absent.
- For a safety decision, retain the input and the threshold actually in force.
  E861 mirrors the guard deadline and records gap, measured interval, average
  and pre-event deadline. This makes post-run replay exact without live UART or
  reconstructing foreground scheduling from a lossy trace.

# E872-E873 - firmware safing is not hardware clearance

- A clean post-stop register/readback report proves the commanded shutdown at
  that instant. It does not prove that FETs, connectors, windings, insulation or
  a supply path survived the event.
- When a physical burn report follows a firmware protection trip, preserve the
  last telemetry but promote the incident above software experimentation. Do
  not back-power the target through debug or UART connections.
- Record configured-limit metadata separately from measurements. E872 printed
  a stale4.0A compile-time label after the operator had set4.5A; neither value
  is a measured current waveform.
- A safe resume gate needs physical attribution, power-off checks and an
  explicit re-energization plan. A successful `off`/`p`/`i` transcript alone
  is never that gate.
