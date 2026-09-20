# Filtered interrupt source contract — E329

Purpose: reduce comparator-driven interrupt traffic without replacing the reused
AM32 scheduling/persistence controller or relaxing motor guards. Current
installed image remains E328 observer-only; this document does not authorize
unqualified direct gate writes or declare filtered lock achieved.

## Preserve the reference sequence

The actual `minz-core::am32_isr::comp_isr` checks pending, then uses current
TIM2 CNT strictly greater than average_interval/2. At a closed gate it clears
only a pre-crossing level; a post-crossing level keeps pending and can re-enter.
At an open gate it clears pending before running twelve consecutive level
checks. Only acceptance masks sensing, reads current CNT, resets it and arms
one COM at wait_time+1. Do not substitute the earlier captured CCR timestamp
for that current CNT in the first experimental implementation.

`tools/observer-replay/tests/filtered_source.rs` executes this actual controller
against the existing MockHal and a capture-source lifecycle model. Tests cover
strict equality, pending retention/clear, persistence reversal, single arm,
masked capture, stale callbacks and coalesced captures. Model tests do not
prove hardware flag-clearing order, filter delay, IRQ timing or motor lock.

## Hardware adapter obligations before a powered trial

- Use a separate experimental source mode; legacy EXTI remains available.
- Configure TIM2 CH2 filter and CKD before active sensing. Preserve interval
  PSC/ARR/CNT/SMCR and CEN; never issue a live UG to configure filtering.
- TIM2 CC2IE/CC2IF replaces the hardware source qualification, not only the
  ISR trampoline. Existing core dispatch currently checks EXTI IMR18 and must
  consult the selected source. Preserve software mask and IRQ storm checks.
- Do not read CCR2 or blanket-clear TIM2 SR at ISR entry: this would acknowledge
  an event the reference may deliberately leave pending. Explicit reference
  clear must acknowledge CC2IF/CC2OF while preserving unrelated timer flags.
- Set TIM2 vector priority consistently with the current comparator path;
  explicitly mask EXTI and stale NVIC state. Stop/dropout/panic revoke source
  before clearing pending. Phase changes disable capture before mux/filter
  settling. Late callbacks cannot re-enable a stopped or superseded phase.
- Observer sidecars currently clear/read CC2 flags on reset and mux. They
  cannot run independently against the same source of controller authority.
  Disable or refactor them under the experimental source mode.
- Keep raw comparator persistence in the initial experiment. Hardware-filtered
  pending does not certify that the raw level remains correct at dispatch.
- Missing accepted events do not generate free-running commutations. Keep
  independent accepted-age, feedback age, current/bus, deadline, actual-arm
  and rate guards; preserve original campaign end on any recovery.

## Required evidence

First disabled hardware event/mask/clear/retention tests using controlled pulses,
including stop then late IRQ, masked capture and phase replacement. Then normal
preflight and a bounded known-point hold, comparing accepted events/COMs,
interval variability, source visits, IRQ occupancy and worst observed timing.
Capture coverage is not the pass criterion. All failures and finaloff retained.
Do not expand duty or change speed guards concurrently with source selection.

E329: lifecycle and actual-reference host tests pass; hardware adapter is not
implemented or flashed. The source model is not yet part of the firmware.

## E330 — staged PAC backend, not connected to the controller

`bench-filter-source` compiles `filtered_irq_hw.rs` and the lifecycle model.
It deliberately has no runtime caller or TIM2 vector yet. The installed E328
image and release ELF remain unchanged. Do not enable this source until the
disabled diagnostic vector and shutdown path are installed and qualified.

The backend configures CH2/code12/CKD2 only with outputs and prior owners off,
CEN zero and DIER zero; refusal leaves another owner's timer untouched. It
preserves PSC/ARR/CNT/SMCR, never emits UG, masks EXTI18, and sets TIM2 priority
0x40. Phase tickets revoke on replacement/stop; repeated after_mux is refused.
Masking preserves peripheral pending; explicit clear writes rc_w0 ones to all
unrelated SR bits. No CCR read or counter reset exists in this backend.
Four saturating counters retain prepare/phase/arm/enable refusals.

The source/observer feature combination is a compile error, tested explicitly.
Full replay suite passes (163 unit tests plus 6/3/1/21 integration tests), as
does M0 library checking and release firmware checking with the source feature.
Three added unit tests cover status-bit preservation, edge bits and revoked
tickets. These are software checks, not peripheral-behavior measurements.

Next: add an outputs-disabled diagnostic TIM2 vector and fixture exercising
this backend's actual pending/mask/clear/stop/phase operations with forced
comparator transitions. Only then connect selected-source dispatch, Input
traits, setup/recovery and stop/panic hooks. Mux/filter settling must be explicit
in that integration; after_mux currently requires its caller to provide it.

## E331 — disabled peripheral/IRQ contract passes on hardware

`filtered_irq_check` now owns the experimental TIM2 vector, but only as a
disabled diagnostic. Outside an active diagnostic, its handler stops the
source; it never dispatches motor control. Panic also stops this source after
clearing gates/ENABLE. `filtersourcecheck` requires ENABLE low, outputs disabled
and no prior motor owner; refuses an active timer/DIER. Nine timer configuration
fields and comparator CSR are saved/restored, event flags cleared on exit.

Three trials in `captures/filtersource_02.txt` each report bits1023, visits2,
restored1, disabled1. Bits0..9 respectively prove prepare, timebase preservation,
masked capture/no IRQ, first delivery retaining pending, second delivery with
explicit acknowledgement, unrelated UIF preservation, old-phase rejection,
post-stop rejection, directly invoked late-handler fallback, refusal counters.
The late-handler check is a direct call, not a deliberately pended NVIC race.
Checks do not qualify mux settling, production controller timing or CPU savings.

Build SHA25654E95D70F514C13AD7BBD55102D8F99A2AF72981D5FA29E221443A6B665DA739;
capture SHA256499317423BD1AA9C4158FAD96E820278E7631AC9ECF2D5157FE3F3797A2CDDF5.
Release build and281 Python tests pass. First capture filtersource_01 stopped
at silent UART initial safety readback; safe probe registers were checked before
the known UART RCC-bit repair. Failure retained. No motor run; finaloff verified.
Next connect source selection to the controller without changing its sequence,
then preflight and known-point powered comparison before any envelope expansion.

## E332 — integrated source first powered comparison FAILS tracking

`bench-filter-control` selects TIM2 only after the powered ownership transfer;
driven acquisition remains on EXTI. Input traits, hardware-enabled dispatch and
TIM2 vector route to actual minz-core. Stale ADC_COMP vectors are masked rather
than dispatched as capture events. Stop revokes the backend and clears selection;
phase enable validates CURRENT ticket. Start paths that inadvertently clear
selection refuse rather than silently fall back. Initial/resume configuration
preserves CEN and later uses the original measured seed. Recovery unqualified.

Initial integration waits10us with capture disabled after mux change, then arms
the selected edge. This is a conservative settling experiment, NOT proven
reference-equivalent event availability. In particular a level transition during
that interval can be lost; do not assume filter history resets with CC2E.

Build955AB08B (full hash in AGENTS), release/s/thinLTO, replay194 tests pass.
Disabled source3trials and all5 standard preflight checks pass without UART
repair. Two finite10s commands at62/62 phase60 end early on tracking8:

- filtercontrol_hold62_01: observed1339us,2COM/1accepted,7COMP visits,
  COMPmax44us/COMmax230us/commit25us, raw162,bus11545mV, arm39us/cost12us.
- filtercontrol_trace62_01: observed1040us,1COM/0accepted,**zero COMP visits**,
  COMmax80us/commit25us, raw236,bus11605mV, arm37.5us/cost12us.

Both report prepared1, active0, zero backend refusal deltas, finaloff verified,
stackuntouched3644. These are FAILED motor trials, not CPU wins. The traced
attempt cannot be explained by persistence rejection because the handler never
ran. Current end snapshot records EXTI state, not TIM2 source state; extend it
before further interpretation. Need TIM2SR/DIER/CCER/CEN/filter/CNT at shutdown
before clearing to distinguish missing capture from disabled delivery. Test
arming/mux timing before changing filter duration or acceptance logic.

## E333 — stop snapshot proves no pending capture with delivery enabled

Backend retains first pre-mask/pre-clear stop SR/DIER/CCER/CR1/CCMR1/TISEL/CNT,
COMP CSR, software armed/enabled and NVIC enabled/pending. Never reads CCR.
Host `drv_filter_stop.py` validates format/ranges/finaloff and decodes bits;
this is not a lock classifier. Snapshot adds work at shutdown, not every IRQ.

An initial disabled test failed restoration; detailed followup reproduced
timer_diff0 with CSR0x40000281 ->0x281. Only live output status VALUEbit30 changed.
Corrected comparison excludes precisely bit30, retains all other bits and raw
values. Tests prove every other changed bit is rejected. Earlier failures
filterstop_source01 (C525178A build), filterrestore_source01 (C56F1F88 build)
remain retained, no powered test followed either failure.

Installed AB53C5E3 (fullhash AGENTS),285 Python/195 Rust tests/release build pass.
filterrestore_fixed_source01 three trials pass; filterstop_preflight01 five gates
pass. One10s-command trace62_01 ends tracking8 at1039us with1COM/0accepted/0IRQ,
raw146/bus11605/COM80us/commit24us, finaloff verified, stack3808.
Source snapshot SR27 (CC2IF/CC2OF both0), DIER4, CCER16(rising), CR1513(CEN1,
CKD2), CCMR149408(code12/directTI2), TISEL256(COMP2), CNT2278, rawlevel1,
armed/enabled/NVICenabled1, NVICpending0. No controller calls plus no capture
pending with delivery enabled supports absent capture, not persistence rejection
or a masked-delivery explanation. High level at stop does not establish its
history. Next compare removing the new10us capture-disabled settling interval,
keeping hardware filter unchanged; do not create artificial accepted events.

## E334 — earlier arming alone does not restore capture

Optional bench-filter-early-arm sets software wait to0; retains filter12/CKD2.
Build4AD6E3D7, disabled source3trials and preflight5checks pass. One bounded
62/62phase60trace1 attempt fails tracking8 after1040us,1COM/0accepted/0IRQ.
COM70us/commit44us,raw184/bus11617,arm101ticks/cost13us,stack3832,finaloff.
Stop SR27/DIER4/CCER48/CR1513/CCMR149408/TISEL256/CNT2276, armed/software/
NVIC enabled, no pending capture, falling selected/rawlow. Removing10us wait
is not sufficient to fix the missing-event failure. No CPU gain claimed.

Rechecking the duration budget reveals a distinct hypothesis: carrier2666ticks
and duty62/1000 gives CCR165ticks=2.578us; after timer deadtime26, ideal pinhigh
139ticks=2.172us (DRV/electrical delays not included). Filter12 requires448..512
kernel ticks=7..8us. It cannot preserve a pulse confined to that ON interval.
This does NOT prove useful comparator pulses are ON-only: measure or test the
hypothesis, not declare the signal bad. Next candidate filter5/CKD2 requires
56..64ticks=.875..1us, fits that nominal window, and leaves actual core raw
persistence unchanged. Qualify its short-pulse capture/IRQ behavior disabled,
then one bounded same-point powered trial. No guard/duty/carrier changes.
Added pure register-budget test; full196Rust tests pass. Candidate5 not built
or installed yet. Earlier code12 observer98% coverage was never lock evidence.

## E335 — shorter filter restores visits but not sustained tracking

bench-filter-one-us selects code5/CKD2 with earlyarm0. Disabled pulsecheck now
eight rows; code5 captures requested2us/measured3us pulse rejected by12, capture
latency3half-us ticks=1.5us (includes synchronization/instrumentation). This test
does not measure sub-microsecond rejection. Source3trials and all5 preflight
checks pass. Releasebuild and286Python tests pass; no UART repair.

Installed BEC72380 (fullhash AGENTS). Same62/62phase60,10s requested:

- hold62_01(trace0): tracking8 at10140us,15COM/14accepted,110IRQ,COMP64us,
  COM258us/commit44us,raw196/bus11569,stack3344. Partial-cycle mean4053.5us
  is not a sustained speed or lock qualification.
- trace62_01(trace1): tracking8 at2039us,3COM/2accepted,15IRQ,COMP74us,
  COM260us/commit45us,raw143/bus11581,stack3312. CRC trace has7closed-gate,
  2accepted and6open-gate persistence rejects. First5 rejects fail first read;
  final seq15 at1577us sees expected0 then reverses to1 by read3.

Both finaloff verified, selected source prepared/no refusal deltas, stopped
DIER4/CCER16/CR1513/CCMR120736/TISEL256, no CC2pending, rawhigh, NVICenabled.
Events now arrive, but no sustained lock. Do not infer IRQ savings from an early
tracking abort. Next use code0 on the same TIM2 adapter as an isolation control:
does removing filter delay restore legacy behavior, or is adapter latency itself
enough to perturb acceptance? Keep raw persistence, guards and duty unchanged.

## E336 — unfiltered TIM2 isolation control completes10s, no CPU benefit

bench-filter-bypass selects code0 on the same backend and earlyarm configuration;
combining bypass/one-us intentionally compile-errors. Installed C8E4236D (fullhash
AGENTS), release build passes, disabled source3trials and preflight5checks pass.
filterbypass_hold62_01 completes full10s62/62phase60trace0 at279.549eHz,
16774COM/16773accepted,raw253/bus10865,stack3264,arm48.5us/cost13us,finaloff.
IRQunion63.732%,17084COMPvisits/s,10.185visits/COM,COMPmax73us,COMmax258us,
commit45us. Source prepared1/code0/settle0/refusals0. Capture SHA256
C48DD685AC19AA36CA0575D1070D5D25E36B8EC14B6EF3D1656E15D69DF49C2B.

This n1 control demonstrates adapter functionality through sustained operation
at this operating point. Code5/12 failures are associated with filtering rather
than an adapter that can never sustain; do not infer a universal latency limit.
It is not a CPU win versus earlier~59–60% EXTI captures, nor recovery-qualified.
The scheme still combines a delayed event with subsequent raw-level persistence:
filter delay changes the sampled waveform phase, not just IRQ count. Before
further filter tuning, investigate that timing contract. Capture qualification
and delayed raw-level qualification are not interchangeable. Any redesign must
retain real-event causality, timestamp/age validity, cancellation, actual minz
commutation sequence and independent guards; don't fabricate accepted events.

## E337 — mirrored timestamp consumption isolated on hardware

For capture-to-software-read latency, reading authority CCR2 would clear CC2IF
and change the controller's retained-pending semantics. CH1 indirectTI2 shares
the same filtered input (E324) but has an independent flag/CCR. Disabled pulse
check now reads CCR1 first and observes CC flags, then reads CCR2. All8 cases
pass: captured flags6->4->0, rejected0->0->0. Thus mirror consumption preserves
authority pending in this diagnostic. No live controller instrumentation yet.

Releasebuild B3997A14 (fullhash AGENTS),287Python tests pass. filtermirror_ack01
silentUART safety-readback failure retained. ProbeRCC08000000/PD1zero/BDTRc1a/
CCRs0 verified before known UARTclockrepair. ack02 passes latency, flagisolation,
restoration and finaloff. Capture SHA256
4BAA129021373E73581E548B6DEA9EAD5A4D528DEB7904370A6DA9213F71B59C.

Next live diagnostic must configure/arm/disable CH1 with source phase lifecycle,
invalidate at every interval reset and stop, never use its timestamp to control
commutation. Record a bounded fixture-only prefix beside the first actual raw
comparator read, with CC1valid/overcapture and counter bracket. CCR1 is latest
capture, not necessarily the event that originally pended the interrupt; label
that ambiguity. Reading CC1 must never clearCC2. Measure instrumented latency
and its observer effect, not infer an exclusive ISR cost or a CPU gain.

## E338 — first-read mirror latency measured (instrumented)

bench-filter-latency configures CH1 indirectTI2 with the validated source phase
ticket, invalidates on phase/reset/stop, reads only CCR1 and clears only its
overcapture flag. A24-row prefix is recorded around the first actual raw read
when coretrace1; exactly one raw read is returned to the controller unchanged.
PRIMASK serializes mirror lifetime/read/record; instrumentation intentionally
adds pre-read/ISR work. Quiet fixture-only dump, no timing authority.

Build034C6E94,290Python tests/release pass, source3trials/preflight5checks pass.
filterlat_trace62_01 atcode5/earlyarm0/62duties/phase60 fails tracking8 at2040us,
3COM/2accepted/15IRQ,COMP79us/COM283us/commit25us,raw127/bus11581,stack2196,
finaloff verified. All15 mirror raw values match the corresponding CRC trace
first value after the existing polarity inversion. Latest valid capture ages
are2.5..40us; step6 specifically35/23/19.5/40/29/23.5us. Rawread brackets.5..1us.
Rows without a new mirror capture report no new age; overcapture marks latest
rather than original IRQ origin. These are NOT uninstrumented IRQ latencies.

At24kHz the41.7us period is comparable to these delays. Data supports examining
the timing contract between capture qualification and delayed raw persistence,
not attributing failures to wiring or a CPU floor. Next design/replay must keep
captured evidence/time/phase validity distinct from current raw level; preserve
actual minz scheduling and independent guard/age/cancellation semantics. Do not
substitute an expected level or timestamp as an accepted event without evidence.
CaptureSHA F39E9286D9D55535269F90AF362DC3643B8DE71500AA8DCD5308ACB5F30F65C5.

## E339 — replay rejects naive substitution; priority divergence is actionable

Three new actual-minz-core tests in capture_timing_contract.rs prove:

- With average1200, a capture at590 and dispatch at601, live CNT opens the strict
  >600 gate. Returning frozen590 never opens it despite repeated pending visits.
- Holding a captured expected level passes12 reads where actual11correct+1wrong
  rejects. A timestamped edge does not justify twelve fabricated live levels.
- Reporting captured900 instead of live940 changes this_zc; COM still arms301
  relative to dispatch. Event-time scheduling requires a coherent redesign, not
  just a substituted interval value.

Full199Rusttests pass; shared core not edited, no flash or motor action.

Source audit suggests a smaller scheduling experiment before such redesign:
../minz/examples/am32_clone.rs1080-1086 sets COMP and COM bothpriority0. binz
core_bench::observe_irq_start sets COMP0x40 and COM0x80. Actual reused COM ISR
blends intervals/wait, writes trace, enables comparator interrupts, then updates
zero_crosses and returns. In binz, pending higher-priority COMP can interrupt
that tail and extend COM walltime; reference peers cannot preempt one another.
This does not prove the observed258us maximum's cause, but is a specific testable
divergence. Do not change guard/DMApriority0. Next experiment: COM and sensing
both0x40, on the measured unfiltered adapter, with priority readback/disabled
behavior qualification then samepoint powered comparison. Preserve all core
calls, level reads, pending semantics, electrical/age/rate/deadline guards.
Compare COM wallmax, actual arm, IRQunion and completedtracking; not just duration.

## E340 — equal-priority trial cuts measured COM wall maximum

Optional bench-com-peer sets COM0x40 equal to sensing0x40. Guard and ADC DMA
remainpriority0. Disabled prioritycheck pends sensing from inside a bounded COM
probe: old priorities yield sequence123(nested), peer priorities132(tail-chain).
Three trials pass both cases and restore NVIC configuration. No timer events
or output authority in the diagnostic. prioritycheck01 initially silentUART;
exact safe probe readbacks precede known UARTclock repair. check02/source01 and
preflight01 pass. Releasebuild and292Python tests pass.

Installed EF4BFEC6 (fullhash AGENTS), peerprio_hold62_01 completes10s unfiltered
TIM2/62duties/phase60/trace0 at279.645eHz,16779COM/16779accepted. Actual priority
readback comp64/com64/guard0/dma0. COM wallmax75us versus E336258us, COMP75us,
commit45us, arm50.5us/cost12us, raw258/bus11092, stack3196, finaloff verified.
IRQunion63.562% versus63.732% baseline: not a demonstrated aggregate CPU gain.
Cycle sigma42.039us versus42.202us is likewise not a meaningful n1 improvement.
CaptureSHA FFF72315A1B83810B50CD191E99471FCA329A76433B60B5098C47A2E56382C14.

This measured reduction plus disabled nesting control supports the priority
divergence as a contributor to long COM wall brackets. It is not proof that
every historical outlier has the same cause or that recovery is qualified.
Next longer/injected-recovery qualification, then carry peer scheduling to the
lean EXTI baseline to avoid attributing extra TIM2-adapter overhead to silicon.
Keep source filtering failures separate from this scheduling improvement.
