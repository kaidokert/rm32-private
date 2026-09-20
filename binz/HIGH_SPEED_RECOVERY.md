# High-speed recovery: E629 constraint audit

## E630 implemented prerequisite: preserve actual requested duty

resume_once now snapshots the stopped segment's POWER_DUTY before observe_begin
resets it, rather than accepting the original caller duty. Both retry callers
use this path. Unsupported targets outside the currently implemented40..100
preparation range refuse with REENTRY result8 before preparation/wake; no clamp
or fallback to an older duty. The30% live cap is not changed. Live updates in
retry campaigns remain disabled pending the complete high-duty path.

Actual wrapper host-seam tests pass for all61 supported duties surviving a
controller reset, unchanged original deadline accounting, invalid targets,
abort and absent budget. These do not prove hardware execution or timing.
38 existing guard policy tests pass. Candidate71BCD7...07052 frozen under
captures/reference/recoveryduty_630,NOTflashed. Release-s/thinLTO full optional
baseline build overflowed256B; omitting optional startup zero-baseline capture
(retaining current-epoch and all running safety feedback) fits. No fast-coast
feature selected in this candidate. Installed911A remains unchanged/off.

Next prerequisites are still substantive: high-duty CCR/metadata preparation
must be coherent while outputs are off, and must survive or be safely rebuilt
after acquisition cleanup. Current flying acquisition cleanup calls bridge_clear
and revokes preparation; do not treat preloaded state as valid across it. This
needs a deliberate preparation/ownership boundary, not simply increasingMAX.
The short final-edge transaction and later injection schedule remain unimplemented.

This is an implementation direction, not permission to bypass checks or a
claim that recovery is complete. The running20% cohort and passive near1kHz
corroboration remain valid. They do not qualify high-speed re-entry.

## Measured current-build baseline

911A recovered once at8% in E629, with12 fresh intervals/7 checked cycles.
Seed871half-us ticks, edge age154ticks (77us), wait218ticks, remaining64ticks
(32us), actual arm6us. Resumed27.992068s at388.161eHz with65193COM/65192
accepted events. Original deadline preserved; finaloff verified. This is a
single current-build recovery pass, not the older32B1 cohort transferred.

## Four independent constraints, not one speed constant

1. `flying_acquire::RuntimeAcquire` uses5000tick minimum full cycle and476tick
   minimum individual interval; `Seed::handoff_with_min` requires seed>=834.
   These intentionally refuse the~333-360tick intervals at1kHz/~927Hz.
2. Reference TEMP_ADVANCE16 gives `wait=(ci>>1)-(ci*16>>6)`. Atci333 the wait
   is83half-us ticks=41.5us; atci360 it is45us. The existing20us confirmation
   dwell plus32us remaining-arm floor exceeds both windows BEFORE any other
   setup. The measured77us full edge age exceeds them by itself.
3. `live_duty_current()` refuses changes while REENTRY_SESSION is true. Simply
   deleting that check would be wrong: callers pass the original `duty` into
   resume_once, which would silently restart at a stale pre-update duty.
4. Injection is fixed at2s, before the established live-ramp fixture reaches
   high duty. Fixed-duty preparation/fixture still caps at100tenths, while
   live coherent updates permit300. A high-speed recovery experiment needs
   an explicit timed/armed injection after reaching its requested setting.

`scripts/test_recovery_budget.py` extracts and compiles the actual linked
reference advance/wait functions;3 Rust cases pass for the measured boundary
and333/360tick examples. No hardware changed. This arithmetic is a constraint
on THIS handoff protocol, not a silicon or motor speed limit.

## Implement toward a short final-edge transaction

The next design should stage timer/vector/controller/statistics/guard setup
before the qualifying edge, while bridge outputs remain disabled. Retain fresh
ordered sensing, but publish/revalidate the final seed and arm with a small,
measured transaction. Existing optional final-edge preparation moved only a few
cold stores and did not produce the needed gain; do not repeat that experiment.

An alternative worth evaluating is an unpowered reference-tracking phase that
earns ordered fresh-event confidence, then grants output authority at a future
qualified COM boundary. That must prove phase alignment and estimator state;
it is not implemented here. In particular, do NOT skip an edge, invent a fresh
timestamp, or reset interval age against an unobserved crossing to gain time.

Any revised arm margin must come from measured complete transaction/dispatch
cost and explicit late-refusal behavior. Reducing32us alone cannot remove77us
of existing setup. Likewise, expanding seed limits alone cannot make the
reference commutation deadline reachable.

Before powered high-speed recovery:

- Preserve current requested duty across the stop/recovery boundary explicitly;
  validate hard30% again and rebuild prepared CCR/metadata coherently while off.
- Add an injection schedule that occurs after reaching the test point; log the
  actual injection, keep one attempt and original campaign deadline, and make
  host validators use that declared schedule rather than silently assuming2s.
- Test stale seed, wrong order, feedback age, host abort, insufficient remaining
  time, cancellation, and teardown with outputs disabled. No preparation token
  may carry gate authority after cancellation or across a new run.
- Measure the hot transaction on target and retain failures. Start powered
  verification at an already-demonstrated setting, then raise the recovery
  operating point. Do not transfer running-only qualification to recovery.

The22-23% current excursions are a separate open mechanism. No current/bus/
tracking threshold change follows from this recovery audit.

## E631 — disabled PWM preparation and current-duty regression verified

Candidate `captures/reference/recoverypwm_631/shell-pwm.elf` is now installed:
SHA256 `0D64952053325867D49C483063FF36F02CCC595A7B9B48A8D2521FA42BB736F5`.
Release opt-s/thin-LTO/codegen1, no defaults, E630 feature set plus
`bench-recovery-duty-check`; no optional current-baseline or fast-coast.
Frozen `.math-audit.S/.json` retained; helper audit is not a WCET certificate.

The new cfg-gated diagnostic accepts a validated `live_duty::Prepared`, checks
idle ownership, ENABLE low, cleared outputs/metadata, stopped ADC DMA and exact
TIM1 geometry, then blanks, loads equal compares, and publishes matching tags.
It cannot select a sector, set a sink, change the comparator mux, or enable MOE.
Repeated preparation is refused; global shutdown clears compares and both tags.
Ordinary recovery does NOT call it yet. In particular, this is not an awake
preparation lifecycle and not permission to retain tags across acquisition cleanup.

`python -m unittest scripts.test_recovery_pwm_prepare` compiles the actual modules:
23 Rust tests pass, including all261 supported duties at all3 timer geometries.
The first harness compilation omitted its existing sixstep test dependency;
adding the real module fixed that harness failure. The10 related tests from
recovery_duty_snapshot/recovery_budget/live_duty_writer also pass.

Downloaded this exact frozen ELF, reset via OpenOCD, then used MCP COM41:
`recoverpwmcheck` reported `passed=4 expected=4 max_us=5 disabled=1 gate_authority=0`
for duty40/100/220/300. The timed bracket is this preparation call only, not
recovery handoff or ISR WCET. Transcript: `captures/recoverypwm_631_disabled_mcp.txt`.
The final burst off/p/i only returned off; subsequent standalone guard fixture
verified all six pins, ENABLE, MOE and CCRs off and nFAULT high. Guard3/18,
full five disabled preflights at3200ticks (CPU observer2/6/9us), ADCroute3 pass.

One regression campaign (NOT a cohort):
```
python scripts/drv_driven_handoff.py --out captures/recoverypwm_631_80_30s.txt --ms 30000 --drive-duty 61 --bemf-duty 80 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle450 --seed400 --seed-timing-reanchor --fast-cycle-report --event100 --dma-guard --dropout --reentry
```
Capture SHA256 `B6189B11035115041A22982BAA40B0D7AC0CD34E089EA0C7144E9203522F3CFF`.
Tracking injection2000029us; fresh acquisition5445us,12intervals/7cycles,
cycle5084..5288ticks, seed872. Edge age154ticks=77us, remaining64ticks=32us,
arm6us. Resumed27992131us vs27992095us requested segment; original campaign
end34711615us preserved, final34711401us (214us spare). Completed window,
65116COM/65115accepted,387.704118eHz, cycle sigma26.503622us. ADC139264scans,
raw peak358 (uncalibrated), bus minimum10925mV, DMAmax41us/queue2, acquisition
IRQmax25us/overruns0, stackuntouched2804. Strict recovery/CRC/timeline/finaloff
validation passes. UART closed. Independent whole-run lock is not newly proved.

This verifies E630's actual-duty snapshot on the normal recovery path and
the separate disabled preparation primitive, not high-duty recovery. No timing
gain:77us remains the fresh-edge-to-arm age. Next integration must establish
an explicit prepared-but-off admission state, stage guard/controller/CCRs before
the final edge, and keep cancellation/cleanup revocation. CORRECTED E632:
`powered_timer::start_inner` and `coast_run_inner` require MOE and physical gate
pins low via `outputs_disabled()`, NOT zero CCRs. The zero-CCR checks are in
`prepare_carrier()` and the old period-only recovery readiness helper.
`acquire_inner` success also calls bridge_clear. Do not bypass those checks or
pretend this diagnostic token survives them. Preserve the original campaign
deadline, current duty, measured edge age and late-refusal behavior.

## E632 — prepared PWM survives the actual recovery acquisition

The source audit corrected the E631 zero-CCR claim above. Physical output
disable is already independent of CCR preload. No change to that predicate
or any electrical guard was needed.

New opt-in `bench-reentry-pwm-stage` takes the retained actual duty into
`reacquire_prepared`. After acquisition's initial full shutdown and statistics
staging, it prepares the carrier and equal compares while awake but with MOE
and all gate pins low. Geometry, owner, nFAULT/wake state and DMA exclusion
are checked. The compare is calculated once before sensing, then retained
separately from the duty tags. Successful acquisition revalidates the actual
compare/mode/enable registers, physical gate pins, GPIO latches and stopped DMA;
only this success skips destructive bridge_clear. Core entry revalidates the
same prepared-off state, skips period reinitialization and uses the existing
live-prepared role application for the first COM. Guard admission and fresh
seed arm are otherwise unchanged. All cancellation/shutdown paths revoke the
compare token as well as the two existing tags. Failed validation safes fully.
Period-only `bench-reentry-carrier` is compile-time incompatible with this mode.

This does NOT expand recovery beyond10% or seed400, does not enable live changes
in recovery campaigns, and does not move guard/controller setup yet. No changes
to reference wait math, acquisition persistence, arm floor or original deadline.
The helper supports30% preload, but that is not a powered recovery qualification.

Frozen and installed `captures/reference/recoverystage_632/shell-pwm.elf`:
SHA256 `E402FE6B706173808DEEBD75D2ECAAB32DDD779F158CD4FA1DD9C47A4B37334E`.
E631 feature set plus `bench-reentry-pwm-stage`, release-s/thinLTO/codegen1,
hashed `.math-audit.S/.json` frozen. Emitted `recovery_prepared` at0800cc58
has no calls/divisions; compare/constants are cached/literals. This is not WCET.

Actual resume-wrapper tests run with and without the new cfg:3+3 pass, including
all61 valid duties, pre-observe snapshot forwarding, unsupported duty refusal,
host abort and original budget. Existing23 module tests pass. Fixture requires
explicit `--reentry-pwm-stage` and exact unique RECOVERPWMSTAGE marker; missing,
duplicate, altered and undeclared markers are rejected. Marker is provenance,
not a substitute for recovery result validation.

Downloaded frozen ELF, OpenOCD reset. On-board `recoverpwmcheck`:4/4 at
40/100/220/300, max9us for the preparation call, disabled=1. Added checks at
each duty prove wrong-duty refusal, alteredCCR2 refusal, and global shutdown
revocation of RECOVERY_COMPARE. All tests keep driver ENABLE low. Transcript
`captures/recoverystage_632_disabled_mcp.txt`; subsequent guard fixture3/18,
fullfive3200 CPU2/6/9, ADCroute3 pass, finaloff readbacks retained.

One powered campaign:
```
python scripts/drv_driven_handoff.py --out captures/recoverystage_632_70_30s.txt --ms 30000 --drive-duty 61 --bemf-duty 70 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle450 --seed400 --seed-timing-reanchor --fast-cycle-report --event100 --dma-guard --dropout --reentry --reentry-pwm-stage
```
SHA256 `8C38222ADC3AB80AD27D1F6E8AA5B34F9C64D385F0E1074DA183B80C79889593`.
Tracking injection2000011us; fresh acquisition6562us,12intervals/7cycles,
5948..6104ticks, seed995. Remaining89half-us ticks=44.5us, edge age160ticks=80us,
arm7us. Resumed27990932us;56807COM/56806accepted,338.246086eHz, cycle sigma
28.650645us. Raw peak306 (not calibrated amps), busmin10984mV, DMAmax41us,
queue2, acquisitionIRQmax24us/overruns0, stackuntouched2800. Final34710872us
vsoriginal34711028us leaves156us. Strict recovery/CRC/timeline/finaloff passed;
UART closed. One pass, not cohort or independent whole-run lock.

The preload lifecycle now works on the real recovery path, but the timing
problem remains:80us is not an improvement over the prior77us (different
operating point, not paired timing attribution). Do not tune this helper to
recover a few microseconds. The next architectural work is staging complete
guard/controller state before the final fresh edge and keeping only bounded
fresh feedback/seed publication and arm there. The unchanged20us dwell plus
32us remaining-arm floor still cannot fit a41.5us wait near1kHz; moving cold
work is necessary but not alone sufficient. A changed final qualification/arm
protocol must have measured timing and truthful edge-age semantics.

## E633 — guard construction staged; no observed arm-age gain

Opt-in `bench-reentry-guard-stage` (depends on E632 PWM stage and guard-install)
constructs the cold RunGuard storage during `stage_reentry_statistics`, before
recovery sensing. It contains an InvalidSeed fault and is deliberately not
healthy. The existing one-shot staging flag is consumed at start and revoked
even by inactive stop. At final admission the SAME electrical/feedback-age/
sector/deadline checks run; `install_staged` requires the dormant sentinel and
updates time, sector, event monitor and limit fields without clearing the cold
cycle array/counters. Missing, live or consumed slots are refused. Admission
timestamps are not refreshed. No changes to admission semantics, gate authority,
startup's ordinary installer, arm margin, seed profile, or campaign limits.

`python scripts/test_guard_install_policy.py --fast-cycle-report --event100 --reentry-guard-stage`
passes40 Rust tests. New tests compare staged vs ordinary construction including
poisoned prior state, all six sectors, wrapping clocks, baseline ages and future
events/feedback/polls. They verify missing/live/consumed refusal and dormant
unhealthy status. Hardware ownership and staging-token cancellation remain
adapter obligations, not claims from the pure-policy test. Four Python mode/
provenance tests pass. Fixture requires explicit `--reentry-guard-stage` with
`--reentry-pwm-stage` and exact unique RECOVERGUARDSTAGE marker.

Frozen/installed `captures/reference/recoveryguard_633/shell-pwm.elf` SHA256:
`2DB80FEFE2D0CD63308379FC083885373236FF38F5150818407F3154FB5E68B2`.
E632 features plus guard-stage, release-s/thinLTO/codegen1. Math-audit artifacts
retained. Actual emitted `install_staged` at0800a7f0 has no calls or division;
its field stores replace cold history clearing. This does not prove a timing
gain. No global ISR or math-profile change was made.

Downloaded exact frozen ELF; OpenOCD reset. Guard3/18, fullfive3200 with CPU
observer2/6/9us, ADCroute3 pass. Explicit MCP `reentrystatscheck` passes3/3:
wrong-fault refusal, dormant construction plus stale-admission refusal/fresh
synthetic installation while owner inactive and bridge disabled, and stop
revocation. Exact UART transcript `captures/recoveryguard_633_disabled_mcp.txt`.
No motor command during these checks.

One powered recovery campaign:
```
python scripts/drv_driven_handoff.py --out captures/recoveryguard_633_70_30s.txt --ms 30000 --drive-duty 61 --bemf-duty 70 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle450 --seed400 --seed-timing-reanchor --fast-cycle-report --event100 --dma-guard --dropout --reentry --reentry-pwm-stage --reentry-guard-stage
```
Capture SHA256 `9B58D33A7B427F7A8D3402A09FFB449A922E38FE443F13C3CEA0DD5247F5EAB5`.
Injection2000010us, recovery6563us/12intervals/7cycles5960..6108ticks, seed1010.
Edge age162ticks=81us; remaining91ticks=45.5us, arm7us. Resumed27990833us,
56768COM/56767accepted,338.015075eHz, cycle sigma28.752172us. ADC139258 scans,
raw peak320 (not calibrated amps), bus minimum10686mV, DMAmax41us/queue2,
acquisitionIRQmax24us/overruns0. Original end34711318us, final34711155us:
163us spare. Strict recovery/CRC/timeline/finaloff pass, UART closed. One run,
not a repeatability cohort or independently established whole-run lock.

No observed handoff speedup:81us vs E63280us. Do not present cold-store movement
as sufficient for high-speed recovery or keep tuning those stores. Next design
should complete setup against the qualified provisional seed and then arm on a
subsequent genuinely observed edge. That must preserve ordered phase history,
fresh feedback and the original deadline, and explicitly qualify the final
edge/timer transaction. The existing20us confirmation plus32us remaining floor
still exceeds41.5us at~1kHz before overhead: this protocol constraint must be
addressed with measured timing, not fabricated timestamps or a seed-limit bump.

## E634 — next-edge protocol and incoming high-speed memo review

Source-only `bench-reentry-next-edge` adds an owning NextEdge state to the actual
Acquisition module. Creation consumes a completed, still-valid acquisition;
an incomplete/faulted/expired source cannot create it. It retains original
start time, last sector/onset and all full-cycle history. Its ONE follow-up
edge uses the existing sequence/individual/full-cycle validation and checks
completion against the original acquisition deadline. Confirmation remains
40..240half-us ticks, and poll_filtered retains only the existing real-candidate
grace. Completion or failure latches; no repeated call can refresh a result.

The returned seed has the NEW physically observed onset and sector. Its interval
estimate intentionally remains the original measured twelve-interval mean;
the prior edge and follow-up interval are separate fields. It does not claim
a newly averaged13-interval seed. The caller must carry continuously serviced
EdgeFilters through setup; this pure interface does not prove actual sampling,
mux settling, ISR cost or physical edge observation by itself.

`python scripts/test_seed_timing_policy.py --next-edge`:43 Rust tests pass
against the actual host minz-core. New cases cover all six sector transitions,
clock wrap, provisional mean vs new onset, retained full-cycle checking,
wrong order, too-fast/too-slow intervals, short cycles, invalid confirmation,
one-shot consumption, original-deadline expiry, and real-candidate-only grace.
The host dependency build emitted its existing nonfatal incremental-cache
AccessDenied note; build/test exit codes were0. No firmware build, flash,
UART or motor commands this entry. Installed/root ELF remains2DB8, last
verified off in E633. Hardware integration is still pending, not silently
enabled by this feature or by tests.

The operator supplied GRAYBEARD_HIGH_SPEED.md mid-work. Read it fully and
retained corrections in GRAYBEARD_HIGH_SPEED_RESPONSE.md. Its DMA arithmetic
uses the passive128us cadence instead of powered201us and treats maxima as
means. Its COM free-run premise contradicts the linked reference; its83us
wait is83half-us ticks with the configured advance. Its current ripple/amp
claims remain hypotheses and do not justify changing the raw clamp. No memo
claim changes a threshold, hardware configuration, or qualification cohort.

## E635 — guard/owner transition for a subsequent edge

The existing guard is initialized for the provisional seed's successor. A
subsequent real seed advances the bootstrap sector by one, so leaving that
guard unchanged would falsely reject the first accepted event after bootstrap.
Resetting the whole guard would instead erase elapsed waiting and feedback age.

New cfg-only `RunGuard::follow_seed` polls the existing clock and permits one
ordered successor before accepted progress. It changes the expected first
sector, not start/campaign elapsed/segment limit/feedback timestamp or the
accepted-event monitor's original start. Driver/host fault inputs are explicit.
A second transition, wrong order, expired tracking/feedback/deadline or prior
accepted progress refuses. The staging constructor resets its one-shot field;
ordinary admission initializes it too. No hardware authority is granted here.

`powered_timer::follow_seed` wraps that policy under interrupt exclusion and
requires active healthy ownership, physical outputs disabled, COMMITS==0 and
a present guard. It samples actual nFAULT and accepts the caller's host-abort
state. Every refusal invokes full trip; success does not write any timer or
gate register. This adapter is not yet called by the live recovery path.

Tests:
```
python scripts/test_guard_install_policy.py --fast-cycle-report --event100 --reentry-guard-stage --next-edge
python -m unittest scripts.test_next_edge_guard_adapter
```
42 policy tests pass, including all sector successors and wrapping clocks,
unchanged deadline/feedback fields, missing-event expiry and repeated follow.
The adapter harness compiles the actual wrapper body and real policy against
explicit owner/pin/clock/trip seams:35 tests pass, including ten wrapper cases
(valid, no owner, pre-existing reason, outputs on, prior COM, missing guard,
driver fault, host abort, wrong order, second follow). No mocked test is claimed
as a PAC timing or physical shutdown measurement.

Release-s/thinLTO/codegen1 with E633 features plus bench-reentry-next-edge builds.
Frozen `captures/reference/nextguard_635/shell-pwm.elf` SHA256:
`2ADD042BB8103EF2B3AD76EAF45AA4820250F268AAB3D7CD7D2303423CE38B0E`.
Math-audit files frozen; no live caller means unused next-edge functions may
be eliminated, so this does not establish their linked hot-path cost. No
flash/UART/motor commands. Installed2DB8 remains last verified off in E633.

Pending integration must carry the qualified NextEdge and continuously serviced
EdgeFilters through setup; preserving their timestamps is mandatory. Once the
guard runs during the wait, its original first-event age and feedback age keep
running. Fresh current feedback should have exactly one ADC owner: do not
start DMA while continuing acquisition's manual ADC scans. After the new
physical edge, call the guarded sector transition, publish the corresponding
core sector/polarity and arm against that actual onset. No zero-cross event
may be fabricated to satisfy the guard, and no captured previous edge may be
relabelled with setup completion time. Sampling gaps and insufficient arm
margin must remain explicit refusals. No higher-speed profile is enabled yet.

## E636 — live next-edge integration staged, not flashed

`bench-reentry-next-edge-live` connects the E634/E635 policies to recovery.
Successful original acquisition moves its qualified history and three existing
EdgeFilters into a one-shot context. Global gates_off cancels this context;
take/store/cancel use interrupt exclusion because the guard can stop from ISR.
Cold controller/guard setup runs first. The foreground follow loop services
DMA feedback only (no simultaneous manual ADC conversions), retains original
filter timestamps, and confirms one subsequent actual crossing. It then moves
the guard's expected sector and the controller sector/polarity once. Existing
arm logic uses the new physical onset, unchanged provisional twelve-interval
mean, actual remaining margin and original campaign/tracking deadlines.

Post-run FOLLOWEDGE reports result, prior/new sector, prior/onset/confirmation
ticks, follow interval and sampling-gap failure detail, all half-us ticks.
Result codes: 2 context, 3 host abort, 4 owner/feedback/output state, 5 sampling
gap, 6 qualified-edge policy, 7 polling policy; 1 means an edge was followed,
NOT that arming or the powered recovery succeeded. The strict host checker
requires `--reentry-next-edge` plus both staging flags, checks singleton records,
sector identity against RECOVERYACQ, wrapping timestamp arithmetic and original
confirmation limits. Existing full-run/ADC/CRC/deadline/off checks remain needed.

Build initially overflowed FLASH by 2272 bytes with the E633 feature set plus
the live feature. Removing optional bench-current-epoch allows release-s/thinLTO
to link. This removes calibration-epoch bookkeeping, not electrical guards or
raw feedback, but is a new feature image: no prior qualification transfers.
Frozen ELF and emitted math audit: captures/reference/nextlive_636.
ELF SHA256: 47B22F2C2EC540B81520AC748B9536810D59BCA03D95AB50503105031B6F6AFD.

Build command (no flash):
```
cargo build --release --no-default-features --example shell-pwm --features bench-adc-phase,bench-cpu-union,bench-dma-201,bench-dma-fast-start,bench-dma-peer,bench-driven-dma,bench-driven-reanchor,bench-guard-install,bench-irq-tail,bench-masked-seed-arm,bench-pwm-20k,bench-quiet-irq-stamp,bench-seed400,bench-reentry-clear-once,bench-reentry-staging,bench-seed-div12,bench-single-core-atomics,bench-static-comp,bench-seed-timing-reanchor,bench-live-control,bench-cycle450,bench-fast-cycle-report,bench-event100,bench-dma-guard,bench-reentry-next-edge-live
```

43 acquisition, 42 guard-policy and 35 actual-adapter/mock-hardware Rust tests
pass. Three Python provenance tests pass, including success/wrap, wrong sector,
wrong interval, confirmation boundaries, duplicate/missing/undeclared markers,
non-success and out-of-u32 evidence. Build and audit exit0. These do not execute
the new PAC-driven follow loop or establish its timing. No UART, flash or motor
commands this entry; actual board remains2DB8, last verified off E633.

Next: review/test the live loop and cancellation/arm admission before its own
disabled preflights. In particular setup plus DMA startup may exceed the retained
100us per-phase sampling gap. Measure and retain that refusal; do not reset the
filters or relabel setup completion as an edge. The context copy under interrupt
exclusion also needs timing evidence. Even a successful follow cannot make a
20us confirmation plus32us remaining-arm floor fit a41.5us reference window at
1kHz; no higher-speed recovery feasibility or envelope gain is claimed here.

## E637 — hardware exposes setup sampling gap, not a motor-speed wall

Closed a stop/arm race: after follow_seed succeeds, the original guard can trip
before the arm critical section. The live-next candidate now checks owner,
ready/nFAULT/ENABLE, reason0 and physical outputs-off inside that section before
configuring vectors or arming COM. Guard-follow refusal also records coast stop8
and completion consistently. These do not change protection thresholds.

New `python -m unittest scripts.test_follow_live` compiles the actual follow loop
and cancellation function with explicit simulated clock/COMP/owner seams and
the real acquisition/EdgeFilters. 96 cases pass: six sectors, clock wrap, normal
following, cancelled context, sampling gap, host abort, feedback-induced shutdown,
lost owner, wrong edge and missing edge. It proves tested logic, not hardware
timing, interrupt interleavings or analog settling. Real next_candidate hint is
exercised, unlike the earlier pure-policy suite alone.

Same E636 build command, release-s/thinLTO, emitted audit and frozen files at
`captures/reference/nextlive_637`. SHA256:
`CD48B6E581BADC44766F81339CD133F77FB962E128B86FD59D78E330BF8C754E`.
Optional current-epoch remains omitted; guards/raw feedback retained. Flash and
OpenOCD reset both exit0, UART closed during SWD. Disabled checks on this image:
guard3 faults/18 poststop refusals; all five filter/atomic/role/CPU/archive checks
at3200ticks; CPU pair maxima2/6/9us; ADC route3; MCP recoverpwmcheck4/4,max10us,
reentrystatscheck3/3 and irqbudgetcheck5/5. Final off/p/i before motor verified.

One finite attempt, no identical retry:
```
python scripts/drv_driven_handoff.py --out captures/nextlive_637_70_30s.txt --ms 30000 --drive-duty 61 --bemf-duty 70 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle450 --seed400 --seed-timing-reanchor --fast-cycle-report --event100 --dma-guard --dropout --reentry --reentry-pwm-stage --reentry-guard-stage --reentry-next-edge
```
FAILED as intended by the host verifier: FOLLOWEDGE result5,max_gap260half-us =
130us > unchanged100us sampling limit. Prior acquisition succeeded at step2,
edge12330half-us, mean1007ticks, twelve intervals, acquisition maxgap168ticks.
No new follow edge, timer arm or resumed powered commutation was established.
POWERPATH reason0 is therefore not a success: the explicit follow-loop refusal
is the cause, REENTRY result7/COASTREF stop8. RECOVERYACQ result1 is prior seed
evidence only; CORESEED remains assumed1 because recovery never reached arm.

Initial segment:4028COM,4027accepted,9955ADC scans, peak_abs_raw323,busmin10662mV,
injected tracking fault8 at2000908us. Original deadline34711116us; early final
elapsed6719726us. This is not a completed30s campaign or reliability cohort.
Capture SHA256:
`CD8F3A72F01CFF267DB1059AE24F332FD7090CEA322BDDCAC3D9CFB2CC10D326`.
Separately replayed parse_dump CRC, decode_first_segment and verify_off PASS.
Finaloff has all six gates/en/MOE/CCRs zero,nFAULT1; fixture process exit1 after
closing UART, no process remains. Stack diagnostic span6312,untouched2572.

This falsifies continuity through the current setup layout. Merely preserving
filter timestamps was not sufficient: they must be serviced during setup.
The loop's first phase isA and the prior seedstep2 is alsoA; the reported gap is
between actual samples, not an inferred rotor period or a CPU blackout bound.
Next restructure setup into bounded work interleaved with the retained sampling
stream, or move setup before acquisition while preserving fresh guard admission.
Do not reset filters, refresh onset timestamps or widen100us to make this pass.
CD48 remains installed, safely off, but its recovery path is NOT qualified.

## E638 — moving DMA startup is insufficient

One ordering intervention in follow_prepared: sample the retained comparator
filters before the first `service_feedback`, then service feedback after each
complete sweep. If an edge is accepted during a sweep, feedback startup/drain
still completes before return; the actual onset remains unchanged, so that
work consumes real arm margin. Owner/readiness/output checks remain before
sampling and before return; the original electrical guard runs throughout.
No new manual ADC owner, timestamp refresh or limit changes.

96 actual-loop/simulated-IO cases and three host provenance tests pass.
Same release-s/thinLTO build command as E636/E637; emitted audit frozen in
`captures/reference/nextorder_638`, SHA256:
`F6545DEE436A96A6A5DD09B84AEF466CCCAB172BEACB9FCEA45FF5718624323F`.
Verified off/p/i before flash, UART closed during SWD, download/reset exit0.
Own disabled guard3/18, five preflights3200ticks (CPU pairs2/6/9us) and ADCroute3
pass. No transfer of E637's extra staging-check measurements to this image.

One attempt with E637's exact campaign command, changing only output filename
to `captures/nextorder_638_70_30s.txt`: FAILED follow result5, max_gap228ticks =
114us against100us. Prior acquisition succeeded:step2,edge13058half-us,mean996,
twelve intervals,elapsed6560us,maxgap168ticks. No new qualified follow edge or
recovery arm; CORESEED assumed1,POWERPATH reason0 are not recovery success.
The first phase remainsA, matching priorseedstep2. This establishes that DMA
startup before the first sample is not necessary for the excessive gap.
The16us difference from E637 is a single cross-build observation, not isolated
DMA runtime or a measured worst-case improvement; no identical retry.

Initial segment4032COM/4031accepted,9954ADC scans,rawpeak322,busmin10853mV;
tracking injection fault8 stopped at2000708us. Original end34711382us, early
final6720136us. CRC, first-segment archive and finaloff replay PASS. Stack
span6312,untouched2572 diagnostic only. Capture SHA256:
`498C55EAE8F1B745AF47B0D4AA700E7F244BD6C6134FA5D10D06B03165B912B0`.
Fixture exited1 after saving and closing UART. All gates/en/MOE/CCRs0,nFAULT1.

The remaining gap precedes the first sample even with DMA deferred. Next change
should service the retained comparator stream during controller setup, with
explicit handling if a new qualified edge arrives before setup completes.
Do not discard such an edge, reset filter history, refresh its onset, or invent
a new guard deadline. This is setup-architecture work, not a rotor-speed limit.
Current F654 stays installed/off with recovery unqualified; no envelope gain.

## E639 — earliest checkpoint localizes discontinuity to acquisition finalization

Refactored one sweep into follow_sweep, used by foreground setup_follow at
three checkpoints: immediately after acquisition returns, after cold core
state/observation setup, and after fresh powered-guard installation. It samples
the phase following the final acquisition phase first (oldest sampled phase),
not unconditionally phaseA. Each sweep retains the exact existing filters and
order/cycle/deadline policy. A completed next edge is cached once with its actual
onset; later setup cannot overwrite it with a later timestamp. Final handoff
still services DMA and applies the original actual-age arm check.

Ownership fix needed for checkpoint borrowing: ISR gates_off now clears only
FOLLOW_VALID (AtomicBool), never the foreground-owned Option/context. This
permits foreground filter mutation without an ISR simultaneously clearing the
same storage. Setup checks validity/readiness/output state before and after its
sweep; final take atomically consumes validity. Cancelled storage may remain
allocated, but no caller can use it as a valid seed. It is replaced by the next
foreground acquisition, not by ISR cleanup. No new gate authority is granted.

Actual-loop simulation now132cases PASS, adding pre-guard setup sampling,
shutdown during simulated comparator read, retained early edge, and300us extra
setup after that edge (same seed returned, actual arm rejects its age). These
are simulated IO/clock tests, not MCU WCET or all interrupt-interleaving proof.
35 adapter Rust tests and three Python provenance tests pass. Same E636 build
command, release-s/thinLTO/math-audited; frozen nextsetup_639 SHA256:
`AD3B04510697E5E3C81FADB2B28BE23B3200A51CE14E290B4A46BA6AF3A1E4EB`.
Flash/OpenOCD reset exit0, UART excluded during SWD. Own disabled guard3/18,
five3200tick preflights (CPU2/6/9us) and ADCroute3 PASS.

One E637 campaign command with output `captures/nextsetup_639_70_30s.txt`:
FAILED follow5 gap300half-us=150us. Crucially REENTRY result5, resume_elapsed0,
remaining0 and REENTRYSTATS used0 identify the FIRST checkpoint immediately
after acquisition, before budget/controller setup or guard installation.
RECOVERYACQ itself succeeded:step2,edge13142,mean1003,twelve intervals,
elapsed6602us,maxgap168ticks. No new qualified edge or recovery arm occurred.
This is not evidence that later checkpoints fail; they were never reached.

Initial4029COM/4028events,9956ADC scans,raw281,bus11020mV,injectedtracking8 at
2001108us. Original end34712500us,early final6721645us. CRC/first archive/finaloff
replay PASS; gates/en/MOE/CCRs0,nFAULT1, UART closed when fixture exited1.
Stack span6280,untouched2540 diagnostic only. Capture SHA256:
`1D40C38B907D376C945771E50E4599553B12C6B2047E254DEE61F08075194A1C`.
No retry or threshold change; no recovery qualification or envelope gain.

Next action changes: sampling is already discontinuous during acquisition
finalization/context construction/reporting/return, before the controller work
previously suspected. The first sampled phase now differs from E638, so150vs114
is not a paired per-phase regression or a CPU blackout measurement. Preserve
sampling through finalization itself, preferably by allocating/constructing the
retained context before sensing and updating it in place, or by moving cold
report work out of that gap. More later checkpoints cannot repair it. Do not
reset filters or widen100us; retain this first-checkpoint refusal as evidence.

## E640 — in-place ownership removes transfer, gap remains before first checkpoint

NextEdge now supports construction before sensing: unqualified(acquire), a
prequalification-only acquiring_mut borrow, and qualify(now) promotion in place.
Step0 is not a seed. Premature follow/poll/qualification calls latch refusal;
repeated qualification latches Consumed. Promotion preserves original history,
sum, sector/cycle data and deadline while clearing the prior cached-ready value.
The existing by-value constructor delegates to the same policy for compatibility.
Live recovery allocates its final foreground-owned FOLLOW context before the
first comparator sample and samples directly into it. Finalization changes
qualification/phase/report/validity, not the full acquisition/filter storage.
ISR cancellation remains authority-only, so it cannot invalidate Rust borrows
by mutating the underlying Option while foreground acquisition uses it.

45 policy tests PASS, including stable acquisition storage address across all
six sectors/wrap, unchanged history/deadline, and premature/repeated-call latch.
132 actual-loop/simulated-IO cases plus3hostprovenance tests PASS. Same E636
release-s/thinLTO feature command. Emitted finalization no longer copies the
history/filter block; small report clear remains. The audit still lists four
acquire_inner soft divisions, as in E639; no arithmetic-clean claim is made.
Frozen `captures/reference/nextinplace_640`, SHA256:
`CC9BE199B080B5080E2DE690B8AE62AFCE8B15C2933655C6D2509E89320F08CC`.

Off/p/i before flash, UART closed during SWD, download/OpenOCD reset exit0.
Own disabledguard3/18, five3200tick checks (CPU2/6/9us), ADCroute3 PASS.
One unchanged E637 campaign command, output `captures/nextinplace_640_70_30s.txt`:
FAILED firstcheckpoint (REENTRY5,resume_elapsed0,remaining0,stagingused0).
FOLLOWEDGE result5,maxgap260ticks=130us; RECOVERYACQ result1,step3,edge13098,
mean1001,twelve intervals,elapsed6580us,maxgap170ticks. No follow edge/controller
setup/recovery arm was established. This is not a completed30s recovery.

Initial4021COM/4020events,9955ADC scans,raw333,bus11008mV; injected tracking8
at2001008us. Original end34711648us,early final6720665us. CRC/first-segment
archive/finaloff replay PASS; all gates/en/MOE/CCRs0,nFAULT1. Fixture exited1
and closed UART; no retry. Stack span6280,untouched2540 diagnostic only.
Capture SHA256:
`3570C46E9F732EA4178F9E0A6BA53AAD9E8502C48EECB04ED73BD6BFE2726EFA`.

The prior sector differs from E639 (3 versus2), so the20us smaller gap is not
an isolated copy-cost measurement. Removal was insufficient. Final reporting,
prepared-state checks, comparator cleanup and return still precede the first
checkpoint. Move continued sampling before that housekeeping, retaining the
original acquisition snapshot separately if needed. Do not insert refreshed
filter levels or synthetic edges to bridge the unsampled interval. Current
CC9B remains installed/off; recovery unqualified, no envelope gain.

## E641 — pre-cleanup continuation still misses continuity

Moved in-place qualification and the first continuation sweep before
comp_input::stop, prepared-PWM checks and report publication. The original
twelve-interval snapshot (including cycle/filter diagnostics) is collected
first and retained separately; a later following edge cannot rewrite it.
On early follow failure, full shutdown occurs and that acquisition snapshot
is still published, with actual disabled state. This is why RECOVERYACQ can
correctly say result1/disabled1 while FOLLOWEDGE reports failure.

Added foreground-only FOLLOWSTAGE:1acquisition finalization,2returned to caller,
3cold core initialized,4guard installed,5final follow wait. It neither samples
an ISR nor changes timing limits. Host accepts historical captures without the
marker but validates singleton0..5 when present, requires5 for success, and
includes stage in refusal messages. The first build overflowed FLASH32bytes;
shortening this new label (retaining the numeric stage) made it fit. No failed
build was flashed.132actual-loop simulated cases and3provenance tests PASS.

Same E636 release-s/thinLTO command, audit frozen at
`captures/reference/nextearly_641`, SHA256:
`BA3B6DD90E4B1DF723D025D961494A526697C5CB77A13297E24A980C2BFBC1F0`.
Off/p/i verified before flash, UART excluded during download/OpenOCD reset,
both exit0. Own disabledguard3/18, five3200tick preflights(CPU2/6/9us), ADC3 PASS.

One E637 command with output `captures/nextearly_641_70_30s.txt`: FAILED follow5,
gap248half-us=124us, explicit stage1. Prior acquisitionstep5,edge13190,mean999,
twelve intervals,elapsed6626us,maxgap168ticks. REENTRY5,resumetime0; no actual
follow edge, recovery arm or powered restart. Initial4034COM/4033events,
9954ADC scans,raw340,bus10972mV,injectedtracking8 at2000808us. Original end
34711383us,early final6720242us. CRC/firstarchive/finaloff replay PASS;
all gates/en/MOE/CCRs0,nFAULT1,UARTclosed after fixture exit1. Stackspan6280,
untouched2540 diagnostic only. Capture SHA256:
`868790334442C5F5974583BC7B18FBA4473EA13726B524490053A5E25190CE26`.

Stage1 executes before cleanup and report publication, falsifying their being
necessary causes. Original acquisition policy/qualification plus snapshot,
promotion and starting a new sweep still leave an older phase unsampled too
long. Priorsector5 differs from E640sector3, so124vs130 is not a paired runtime
measurement. Next maintain phase sampling inside the original scan through
qualification, instead of leaving it and restarting via another checkpoint.
Do not reset history or extend100us. One attempt retained, no retry; current
BA3B installed/off, recovery unqualified and no envelope gain.

## E642 — original-scan continuation staged, not flashed

At the original scan's successful seed branch, freeze the twelve-interval
snapshot and immediately call continue_filters, before leaving the scan for
promotion, cleanup or report publication. It visits the next three phases with
the same10us mux settling and the SAME existing EdgeFilters. It stops on the
first real qualified edge, retaining phase/polarity/onset/confirmation as one
pending record. After in-place promotion that record must pass NextEdge's
ordinary order, interval, cycle and confirmation checks before becoming the
cached follow seed. No observed edge is discarded or relabelled as setup time.

FOLLOWSTAGE6 identifies this early continuation (1..5 retain E641 meanings).
On refusal, shutdown and the original acquisition snapshot are retained. Common
snapshot generation is now one outlined function used for success/failure;
following samples cannot rewrite the original snapshot. This still costs time
before the continuation; no claim yet that hardware sampling gaps are fixed.

Actual-loop simulated132 cases remain PASS. Additional actual-helper tests on
both TIM17 origins retain a prior candidate's onset through a confirming sample,
feed that edge into the real NextEdge after delayed promotion, check unchanged
onset/mean, and refuse stale sample gaps and disabled driver state. Three host
provenance tests PASS. These do not replace PAC timing or a powered test.

Initial full-feature release link overflowed480B; no flash followed it.
Added opt-in `bench-recovery-runtime-only`, which removes only the optional
disabled recoverpwmcheck harness and its disabled-only wrapper. Shared
prepare_recovery_inner, powered staging/revalidation and all electrical guards
remain. The shell explicitly reports the test command is not linked. Historical
feature sets retain the harness; prior recoverpwmcheck timing is not transferred.
The exact candidate build uses E641's command PLUS bench-recovery-runtime-only:
```
cargo build --release --no-default-features --example shell-pwm --features bench-adc-phase,bench-cpu-union,bench-dma-201,bench-dma-fast-start,bench-dma-peer,bench-driven-dma,bench-driven-reanchor,bench-guard-install,bench-irq-tail,bench-masked-seed-arm,bench-pwm-20k,bench-quiet-irq-stamp,bench-seed400,bench-reentry-clear-once,bench-reentry-staging,bench-seed-div12,bench-single-core-atomics,bench-static-comp,bench-seed-timing-reanchor,bench-live-control,bench-cycle450,bench-fast-cycle-report,bench-event100,bench-dma-guard,bench-reentry-next-edge-live,bench-recovery-runtime-only
```
Release-s/thinLTO builds and emitted audit frozen in
`captures/reference/nextscan_642`, SHA256:
`586A2B6A9B6946AFA53331CFCEB4178DF801AE08E46702CC94A0674AA260B5E4`.
No UART, flash or motor commands this entry. Actual board remains BA3B, last
verified off E641. Candidate needs its own disabled preflights and a fixed7%
recovery attempt; no timing, physical shutdown or recovery qualification claim
is transferred from that installed image. Goal/envelope unchanged.

## E643 — continuation reaches final wait; real feedback starts too late

Installed exact frozen nextscan_642 (586A) after off/p/i, UART closed during
download/reset, both exit0. Own disabled guard3/18, five3200tick preflights
(CPU2/6/9us) and ADCroute3 PASS. One unchanged E637 campaign command with
output `captures/nextscan_643_70_30s.txt`.

FAILED, but a different measured failure: FOLLOWSTAGE5, FOLLOWEDGE result4,
max_gap0; POWERPATH reason4 (FeedbackStale), stop209us, guard ISRmax17us,
no commits. Original acquisition succeeded:step4,edge13256half-us,mean999,
twelve intervals,elapsed6660us,maxgap170ticks. Checkpoints1..4 were reached
without sampling-gap refusal; this does not prove the whole recovery stream.
No actual followed edge or recovery arm was established.

FEEDBACKFIRST initial_age_us893,seen0; DMAmax0,queue0,POWERFEEDBACK scans0.
The original baseline leaves only107us until the unchanged1ms freshness limit.
Controller code currently performs checkpoint4 and another final-wait sweep
before first service_feedback/ensure_started. The guard stopped at209us before
any first frame was recorded. This is delayed real ADC service, not permission
to refresh the old timestamp or waive FeedbackStale.

Initial4026COM/4025events,9955ADC scans,raw287,bus11044mV,injectedtracking8 at
2000908us. Original end34710783us,early final6720245us. CRC/firstarchive/finaloff
replay PASS; all gates/en/MOE/CCRs0,nFAULT1. Fixture exit1 after closing UART.
Stackspan6280,untouched2540 diagnostic only. Capture SHA256:
`DC15622C070B639C77341909B1889E82DBE35B70F2DA06580964DD9FCAF968BD`.
No identical retry or recovery qualification claim.

Staged next fix: call existing service_feedback immediately after successful
start_reentry, before checkpoint4. Acquisition's manual ADC owner has finished;
this starts the sole DMA owner, whose IRQ publishes actual coherent frames with
their original acquisition times. Existing service failure path shuts down.
No current, bus, age, tracking, timing or campaign limit changes.

New guard regression uses893us initial age: a first real frame at108us is
refused even if itself20us old; one delivered at80us with actual acquisition50us
is accepted and retains50, not80, as feedback time.43guard-policy tests PASS.
E642's exact runtime-only build command succeeds release-s/thinLTO; audit and
ELF frozen at `captures/reference/nextdma_643`, SHA256:
`38812A23A3103BC0CB0AE08B3BC9C0D308421A09F6FAFC09C6B0413BCD1F4407`.
This candidate is NOT flashed or timed on-board. Actual586A remains installed,
verified off in the failed capture. Next perform candidate-specific disabled
preflights and one fixed7% recovery attempt; no earlier qualification transfers.

## E644 — upstream continuity still intermittent; DMA change not exercised

Installed exact nextdma_643 (38812A) after off/p/i, serialized UART/SWD,
download/reset exit0. Own disabledguard3/18, five3200tick preflights(CPU2/6/9us)
and ADCroute3 PASS. One E637 campaign command, output
`captures/nextdma_644_70_30s.txt`: FAILED follow5/stage6, gap222half-us=111us.
Prior acquisitionstep5,edge13180,mean999,twelve intervals,elapsed6621us,
maxgap168ticks. REENTRY5,resumetime0; no new guard installation or DMA startup.
This attempt did NOT exercise the earlier-DMA change, so cannot convict or
qualify it. E643's single successful traversal of these checkpoints did not
establish repeatable sampling continuity.

Initial4030COM/4029events,9955ADC scans,raw287,bus11020mV,injectedtracking8 at
2000908us. Original end34711196us,early final6720133us. CRC/firstarchive/finaloff
replay PASS; all gates/en/MOE/CCRs0,nFAULT1. Fixture exit1/UARTclosed. Stack
span6280,untouched2540 diagnostic only. Capture SHA256:
`6B5572EFF4BBD4DD8654569A1BBC855E4533D4C11B2F121AE423FEA709FF78DD`.
No identical retry, timing-limit change or recovery pass.

Source review identifies an obsolete scheduling choice for this new path:
acquire.final_candidate parks on the final phase for20us to minimize the age
of a seed used by immediate handoff. The next-edge recovery now uses this seed
only provisionally for setup, then waits for a subsequent crossing. Parking
still ages the other two filters without providing that old immediate-arm
benefit. The new candidate skips only this acquisition shortcut for awake
next-edge recovery with nonzero prepared duty. The ordinary round-robin scan
still confirms through exactly the same EdgeFilter40tick dwell/200tick gap;
order, twelve intervals, cycle checks and original deadline remain. The final
following-edge shortcut and initial driven handoff are unchanged.

45policytests PASS including sequential-mux qualification and dwell/gap cases;
these are not on-board timing evidence. Same E642 runtime-only release-s/thinLTO
build succeeds, audit frozen at `captures/reference/nextround_644`, SHA256:
`74128B9917525D4D980008136A70AB2D00074580033DD7199A3596A6B85AA6D8`.
Candidate NOT flashed. Actual38812A remains installed/off. Next own disabled
preflights and one fixed7% recovery; verify RECOVERYCYCLECONFIRM final_visits0
for this acquisition mode and retain any refusal. No envelope gain claimed.

## E645 — real DMA delivery works; final all-phase wait hits sampling limit

Installed frozen nextround_644(74128B), serialized off/UART/SWD, download/reset
exit0. Own disabledguard3/18, five3200tick preflights(CPU2/6/9us), ADCroute3 PASS.
One E637 campaign command with output `captures/nextround_645_70_30s.txt`:
FAILED follow5/stage5,gap240half-us=120us. The provisional shortcut is confirmed
inactive: RECOVERYCYCLECONFIRM final_visits0,dwell20. Prior acquisitionstep1,
edge13084,mean998,twelve intervals,elapsed6630us,maxgap166ticks.

Unlike E643, early DMA produced a real accepted frame: initial age850us,
decision96us,actual acquisition15us,framefault0. POWERFEEDBACK scans1,raw23,
bus11701mV; DMAmax41us,queue1. POWERPATH reason0 here does NOT imply recovery
success: follow's explicit sampling-gap refusal shut down before an edge/arm.
No FeedbackStale in this attempt. DMA runtime coincides with this phase of
execution but has not been individually proven the preemptor of the gap.

Initial4031COM/4030events,9955ADC scans,raw352,bus10877mV,injectedtracking8 at
2000909us. Original end34711822us,early final6721184us. CRC/firstarchive/finaloff
replay PASS; all gates/en/MOE/CCRs0,nFAULT1,fixture exit1/UARTclosed. Stackspan
6280,untouched2540 diagnostic only. Capture SHA256:
`8E0ADF11D6375743F9385BD85A42F54B70D469AFAFE6A67495F0102FB63061E7`.
No retry, threshold change or completed recovery claim.

Staged opt-in bench-follow-expected-phase narrows only the final wait, not
acquisition or setup, to the known successor phase (c.phase). The prior twelve
intervals establish the sequence; this next input must still pass the same
phase/polarity/order/individual/full-cycle/dwell/gap/deadline checks. It uses
that phase's ORIGINAL continuously serviced EdgeFilter, not a newly primed
level. Setup still samples all three. Nonselected phase edges are no longer
observed during the final wait: this is explicitly a different observation
mode, not an assertion of full all-phase coverage. Missing expected edges
still time out. Electrical feedback and guard timing remain unchanged.

Actual-loop tests compile/run both modes:132cases each, plus continuation-helper
tests, PASS. A wrong-phase-only simulated transition now leads to missing-edge
timeout in expected-only mode instead of the all-phase mode's order refusal;
the test and scope record that difference. Three strict provenance tests PASS.
FOLLOWMODE expected_phase_only1 requires explicit --follow-expected-phase;
the fixture flag also requires --reentry-next-edge. No silent pooling with
historical all-phase captures.

E642's runtime-only release-s/thinLTO build command PLUS
bench-follow-expected-phase succeeds; audit/ELF frozen at
`captures/reference/nextphase_645`, SHA256:
`8138ECC57EDE6C1950800135AFCC94764E7E2B7D5F70634538364E1F4B787D8C`.
Not flashed. Actual74128B remains installed/off. Next own disabled preflights
and one fixed7% campaign with --follow-expected-phase added; no timing or
recovery qualification transferred to the new observation mode.

## E646 — expected-phase wait still refuses; distinguish first versus later read

Installed frozen nextphase_645 (8138ECC), download and OpenOCD reset exit0.
Own disabled guard3/18, five3200 preflights (CPU pair2/6/9us), ADCroute3 PASS.
One E645 campaign command, output `captures/nextphase_646_70_30s.txt`, with
`--follow-expected-phase`: FAILED follow5/stage5, gap244 half-us =122us.
FOLLOWMODE confirms expected_phase_only1. Prior step1/edge13326, provisional
mean1005ticks, twelve intervals, acquisition6752us/maxgap164ticks. No next edge
qualified and no recovery arm. POWERPATH reason0 is not a recovery pass.

Real DMA first-frame acquisition14us, decision95us, initial baseline age859us,
fault0; scans1/raw18/bus11605mV, DMAmax41us/queue1. FeedbackStale did not recur.
Neither DMA preemption nor first-versus-later final-wait sample is established
by this capture. Expected-phase-only selection alone did not remove the gap.

Initial segment4029COM/4028events,9954ADCscans,raw315,bus10984mV, injected
tracking fault8 at2000809us. Original end34710718us, final6720113us. CRC,
first-segment archive and finaloff replay PASS; gates/en/MOE/CCRs0,nFAULT1,
fixture exit1/UARTclosed. Stackspan6280/untouched2540 diagnostic only.
Capture SHA256:
`8C63868233B03B562BB0B360640EC4181C8BDCD453ACFD0509CE6143C145209D`.
No identical retry or guard change.

Staged a foreground-only sample-attempt counter, reset at each follow stage,
incremented after the actual timestamp and before each EdgeFilter sample.
Postrun FOLLOWREADS includes the refused attempt. This separates a setup-to-
first-read gap from a later wait gap without a per-event recorder. It adds
observer work; it is not timing-neutral or a new qualified firmware image.
Historical captures may omit the counter; duplicate/out-of-u32 values and
sampling-gap with zero attempts are rejected by the decoder.

Actual-loop simulation132cases in each sensing mode plus helper tests and
four host tests PASS. Same E645 features, release-s/thinLTO build exit0 and
emitted math audit saved in `captures/reference/nextreads_646`, ELF SHA256:
`0C5CC8419406ECD225443BE44537868932AEDF3EBC901D79CA4B58B730001B25`.
NOT flashed; actual8138ECC remains installed/off. Next own disabled checks,
then one fixed7% recovery with the same E645 flags, inspect stage/read count.
No sustained or recovery qualification transfers to this counter build.

## E647 — first-read gap localized; retained-context borrow tested

Flashed nextreads_646 (0C5CC8), own guard3/18, five3200 preflights CPU pair2/6/9,
ADCroute3 PASS. One unchanged 7%/30s recovery command (E646 flags), capture
`captures/nextreads_647_70_30s.txt`: follow5/stage5/read1, gap244ticks=122us.
Thus refusal is the FIRST final-wait sample, not a subsequent repeated read.
Priorstep1/edge12324; realDMAacq14/decision95us, initialage857us, fault0.
Initial4033COM/4032events,9954scans,raw347,bus11104,tracking injection8.
Capture SHA256 A981DD4E16637339EBE28382291C37B146188418DAE15F2259A3FDF1026E2FC8.
CRC/firstarchive/finaloff PASS, UARTclosed. No arm/recoverypass.

The final wait consumed the large retained Option by value before first read.
Changed only storage handling to as_mut after the same one-shot validity swap.
Foreground retains exclusive access; ISR cancellation only updates validity,
never the context. Owners/off/feedback/seed identity/deadline/filter checks
remain. Actual-loop132cases each mode plus helper/4host tests PASS.
Release-s initially exceeded FLASH64B. A noinline sweep trial had the same
overflow and was removed; shortened only the foreground help banner to fit.
Release-s/thinLTO build/audit PASS; frozen nextborrow_647 ELF SHA256:
B26D0FBA0BB6CD7E72986D32B49326DD3EC325556FCE153D18D88E1B5E09B952.
Failed builds were not flashed. Download/OpenOCDreset exit0.

Candidate own guard3/18,five3200 CPU2/6/9,ADCroute3 PASS. One same-command
recovery, capture `captures/nextborrow_647_70_30s.txt`: follow5/stage5/read1,
gap220ticks=110us. Priorstep3/edge13192; realDMAacq15/decision95us,
initialage842us,fault0; scans1/raw18/bus11677. No followededge/arm.
Different priorsector means the12us difference is not isolated copy-cost proof.
Initial4028COM/4027events,9954scans,raw307,bus11092,tracking injection8.
Capture SHA256 FCBF950715CD05A8FD8269ACCD45220E6EC63C7F29121BB670FF1BE71C7E4700.
Original end34712967us, final6722271us; CRC/firstarchive/finaloff PASS,
UARTclosed. Actual B26D remains installed/off. No guard changes.

Next source-directed question: setup_follow(4) samples successor FIRST in an
all-three sweep, followed by two mux/settle visits and controller preparation
before the final wait returns to successor. Sampling successor LAST at this
last checkpoint could remove avoidable ageing without losing all-three setup
coverage or refreshing timestamps. Not implemented/tested yet; the first-read
counter remains the check. Do not infer DMA alone causes the remaining gap.

## E648 — bridge the last setup scan; real next edge reaches arm-margin check

First tested successor-LAST-only stage4 scan, preserving identity on every
return. Host simulation132cases each mode/4tests PASS, including all3 reads,
last-selected mux, identity after refusal, no ADC ownership. Release-s/thinLTO
build/audit frozen nextlast_648 SHA256:
23B2EEFE55578E244A5AF95C99C797598BB5E1E554BE38D49B3F0FD851315405.
Own guard3/18, five3200 CPU pair2/6/9us, ADCroute3 PASS; one same E646 command
with output `captures/nextlast_648_70_30s.txt`. FAILED stage4/read3 gap258ticks
=129us, priorstep1. Reordering moved the gap upstream; it did not solve it.
RealDMA acquisition15/decision96us, initialage864us, fault0. Initial4030COM/
4029events,9954scans,raw303,bus10996, injectedtracking8. CRC/firstarchive/off
PASS; UARTclosed. Capture SHA256:
E1365344E2D6DD198DB424A080889432917F9B561CD2723026D8EDB4FEBD968C.
This variant is retired, not silently counted as a pass.

Replaced reordering with original all-three sweep followed by one additional
expected-phase sweep at stage4. Same real EdgeFilter timestamps/dwell/gap,
same order/cycle/deadline checks. If already qualified, cached onset is kept;
the extra call never fabricates/relabels an edge. Shared refusal processing
keeps the first error and prevents another sweep after failure. Original first
bridge build overflowed FLASH32B; sharing result/refusal code fits, no threshold
or protection removal. Host simulation132cases each mode/4tests PASS, now checks
four reads and final successor mux in no-edge setup; no hardware WCET claim.
Release-s/thinLTO/audit frozen nextbridge_648 SHA256:
45B486599F34D8CA4B6806712276180391D6633FBEB7D7905579FDF94F49E7C9.
Download/OpenOCDreset exit0 after successful build only. Own guard3/18,
five3200 CPU pair2/6/9us, ADCroute3 PASS.

One same 7%/30s recovery command, `captures/nextbridge_648_70_30s.txt`, reached
FOLLOWEDGE result1: priorstep3 edge13232 -> step4 onset14230, confirmed14270,
interval998half-us. The required20us confirmation completed in stage4; final
stage5 reads0 because it correctly consumed the cached real seed. No sampling
gap refusal. Provisional12interval mean999ticks. RealDMAacq15/decision95us,
initialage849us,fault0,scans1/raw17/bus11641mV.

The run then FAILED the unchanged fresh-arm margin: edgeage188ticks=94us,
remaining62ticks=31us <64ticks=32us. It reached the arm decision, not an arm
or completed recovery. Do not waive1us or retry until pass. Initial4030COM/
4029events,9954scans,raw302,bus11140, injectedtracking8 at2000808us. Original
end34711775us,final6721177us. CRC/firstarchive/finaloff PASS, all gates/en/MOE/
CCRs0,nFAULT1,Uartclosed. Capture SHA256:
7CDF6A5EB4353828C99C7BACF5C9660AE30C77D6751B7523630F16915AFE1694.

Current45B486 remains installed/off. Next inspect the actual work between
stage4 confirmation and final arm: controller-state preparation, follow-seed
guard transition and arm checks. The bridge removes this observed sampling
failure but does not prove high-speed timing feasibility or reliable recovery.

## E649 — seed-only initialization moved earlier; 7% recovery 3/3

Moved the unchanged provisional seed initialization (S interval/history/last-ZC,
advance/wait, observed seed and D.zero_crosses12) from after stage4 to before
stage3. Exact wait remains a local value used by the later original arm check;
no timer or sensing IRQ is enabled early. This also changes initial-handoff
setup ordering; the three campaigns below exercise initial entry as well.
Earlier setup can lengthen an upstream sampling gap, so hardware qualification
was required rather than treating source reordering as timing proof.

132actual-loop simulations each mode/4hosttests PASS (these cover follow
semantics, not the core initialization relocation or hardware WCET). Release-s/
thinLTO build and math audit PASS. Frozen nextseed_649 ELF SHA256:
3E19D0544FEA332D5E6A050A6E52C48B5CC233788D85A87092AA17ACD9219A01.
Download/OpenOCDreset exit0. Own guard3/18,five3200 CPU pair2/6/9us,ADCroute3 PASS.

One fixed7%/30s recovery passed; then TWO repeats declared before either ran,
both passed. No further attempts or exclusions. Same E646 command/flags with
outputs listed below. Tracking loss is injected at~2s, reacquisition uses12
fresh intervals and a real following edge, then resumes~27.990s within each
original campaign deadline. All CRC/firstarchive/ADCphase/timeline/finaloff
checks PASS; every fixture exit0/UARTclosed.

| Capture suffix (`nextseed_649_70_30s`) | eHz | COM / accepted | Cycle sigma us | Remaining us / arm us | Raw peak / bus min mV |
|---|---:|---:|---:|---:|---:|
| `.txt` |337.666|56708 / 56708|29.440|43.5 / 7|317 / 10889|
| `_02.txt` |338.364|56826 / 56826|28.570|34.5 / 7|331 / 10925|
| `_03.txt` |337.518|56683 / 56683|29.360|34 / 7|309 / 10925|

The32us remaining floor and16us arm budget are unchanged. Original first
segments ended on injected tracking8, with4038/4040/4036COM and one fewer
accepted event. Final gates/en/MOE/CCRs0,nFAULT1. IRQ-union brackets occupy
about59.2..59.4% of resumed time, not a CPU-utilization/WCET certificate;
foreground work and observer cost remain distinct. Raw peaks are not amps.
Independent rotor-lock qualification is not established by these counts alone.

Capture SHA256 in row order:
- A3A0340E4A6F4BC003EA69462C7A194F93E0C3C2E63B36C6E10B178B96176A1C
- D2CBC6DC2BDD1EEB52EE16AFE0AC7B90FC03D56BE7088C7F2E808293B3542AE2
- 7FA287CA5791B5513ED40C12FDABFC94B2EBF488109E9B0C3B1B4C5B51177230

`captures/nextseed_649_cohort.csv` includes all3 matching attempts. Current3E19
installed/off. This is current-build low-speed recovery evidence, not transfer
of the older911A20%/near1kHz results. Next one8%recovery on this next-edge path;
no more identical7% trials or high-duty recovery claim. Full goal remains open.

## E650 — 8% reaches a real edge but misses arm time; stage4 capture localized

One unchanged3E19 command at bemf-duty80 (all other E649 flags unchanged),
`captures/nextseed_650_80_30s.txt`, FAILED fresh-arm: provisional874ticks,
age252ticks126us, wait219ticks109.5us, remaining-16.5us. Realstep2->3 onset11630,
confirmed11670,interval802ticks; finalwaitreads0 means previously cached, but
old protocol does not identify its capture checkpoint. Initial4630COM/4629
events,9955scans,raw389,bus11044, injectedtracking8. CRC/firstarchive/finaloff
PASS/UARTclosed. Capture SHA256:
F992EEF8AAE134C516D95C77AC294EF5B7F8C7E9D9CF37C15C833D0A8CD1F9E5.

Added one foreground retained capture-stage value, assigned only alongside
successful edge publication (stage6 for original-scan continuation); reset per
recovery, printed after shutdown as FOLLOWREADS count=N captured=N. It grants
no authority and changes no sensing/arm policy. Decoder accepts historical
absent field but rejects malformed/out-of-range stage and success captured0.
132actual-loop simulations each mode/4hosttests PASS; extra provenance cases
include1..6 and rejection0/7/u32overflow. Not a timing-neutral observer claim.
Release-s/thinLTO build/audit frozen nextorigin_650 SHA256:
7123F4547EB27C4EF6C75E2D855075F37C38469196C6B8EFB71D2A0EF982811B.
Download/OpenOCDreset exit0, own guard3/18,five3200 CPU pair2/6/9,ADCroute3 PASS.

One8% diagnostic `captures/nextorigin_650_80_30s.txt` FAILED arm:
FOLLOWREADS count0 captured4, step5->6 onset12400 confirmed12440,interval802.
Provisional866ticks, age254ticks127us, remaining-37ticks=-18.5us. The real edge
was qualified in stage4, with20us dwell; another107us elapsed before arm check.
This is not a missing-edge or sampling-gap failure, nor a1us safety-margin
dispute. No recovery arm occurred. RealDMAacq15/accepted95us,initialage832us,
framefault0. Initial4627COM/4626events,9955scans,raw383,bus10948,injected8.
CRC/firstarchive/finaloff PASS,Uartclosed. Capture SHA256:
9D382E2F3FD8FD17F197E73C2AD2A7477B839485E920EDB70FFD5AC4D7A513AF.

Current7123 installed/off;3E19 7%3/3 stays scoped to3E19. Source review finds
follow_prepared drains service_feedback after obtaining a ready seed. Under
bench-dma-guard this drain is foreground statistics: actual raw-current/full
bus/VREF/age checks already run in DMA stream_feedback. Next evaluate deferring
that statistics drain to the resumed loop ONLY for ready-seed DMAguard path,
without dropping a frame, disabling overflow, refreshing timestamps, skipping
guard/abort/owner/output checks or changing deadlines. Its cost is not yet
measured/convicted; capture-stage observation is the new evidence, not proof
of an entire107us cause. No optimization staged yet, no identical retry.

## E651 — defer ready-edge statistics drain; arm failure remains

Only when bench-dma-guard AND seed.is_some(), follow_prepared now leaves FIFO
statistics for the resumed foreground loop. DMA stream_feedback continues raw
current/full bus/VREF/age validation and publication. Waiting iterations and
non-DMAguard builds keep the drain. Owner/output checks remain after the
conditional drain, and caller follow_seed polls original guards before arm.
No queue entry is popped/dropped by the shortcut; FIFO overflow remains live.

Host actual-loop132cases now run in THREE modes (all-phase,expected-phase,
expected-phase+DMAguard), including cached-seed zero-drain assertions and
normal waiting/refusal tests; all plus3provenance tests PASS. Initial expanded
harness compile failed once; subsequent complete direct invocation exit0.
Build initially overflowed FLASH32B with the E650 capture-stage marker; retired
that answered diagnostic (historical parser retained), then release-s/thinLTO
build/audit PASS. No failed image flashed. Frozen nextdefer_651 SHA256:
0C5A88E49D21EE58CCC107C3CDB8D39F5D675FD95EF4D26D6D454DEAA5357EA7.
Download/OpenOCDreset exit0. Own guard3/18,five3200 CPU pair2/6/9,ADCroute3 PASS.

One same8%/30s campaign `captures/nextdefer_651_80_30s.txt` FAILED fresh arm:
seed876ticks,age234ticks117us,wait219ticks109.5us,remaining-7.5us. Realstep1->2
onset11724/confirmed11764,interval800ticks,finalwaitreads0. No recoveryarm.
Different sectors/build/probe removal prevent interpreting the10us change
fromE650 as isolated drain cost. This did NOT remove the dominant delay.
Initial4624COM/4623events,9955scans,raw368,bus11068,injectedtracking8. CRC,
firstarchive/finaloff PASS, gates/en/MOE/CCRs0,nFAULT1,Uartclosed. Capture SHA:
EB9D3D2D57B349FB5CB65CBBA3BFCFEB31BDBFADEABAF94FD81534A927941FA0.

Timing-label correction: FOLLOWEDGE confirmed is sampled at the FINAL COMP
read, BEFORE EdgeFilter.sample and NextEdge.edge software qualification. Thus
E650's107us and this97us are final-read-to-arm, not purely post-qualification
setup. Do not attribute that entire bracket to ADC or controller setup.
Emitted call audit: NextEdge.edge at0800c7a4 calls Acquisition.edge at0800c9d4
then two __aeabi_memcpy helpers. Acquisition.edge has only the conditional
mean_twelve_fallback call in its direct call listing (not evidence it executed).
No direct hot divide call found there; no cycle/WCET claim from this listing.
Next separate final-read->software-result->arm costs before another claimed
latency fix. Current0C5A installed/off, no recovery qualification transferred.

## E652 — measured qualification bracket varies16us versus60us

Added one t17 read immediately after successful NextEdge.edge; retained
policy_ticks measures FINAL COMP READ through EdgeFilter/NextEdge success,
including intervening interrupts and timer observation. Postrun only. Zero
is not proof of zero cost: the original-scan buffered-edge path is not timed
by this probe. Historical parser accepts absent field, rejects u32overflow.
132actual-loop cases in3modes/4hosttests PASS; not hardware cost qualification.
Release-s initially overflowed32B; removed only duplicate RTT boot announcement
(UART boot banner unchanged), build/audit then PASS. Frozen nextcost_652 ELF:
3F3E34FB7442955B6FA2B5949DE131D5D850D966A738A6FA331D13C51A2AFA1A.
No failed image flashed. Download/OpenOCDreset exit0. Own guard3/18,
five3200 CPU pair2/6/9us,ADCroute3 PASS.

One8%/30s diagnostic PASS, then TWO fixed repeats declared before running them:

| Capture suffix (`nextcost_652_80_30s`) | Result | Final read->policy result | Later result->arm | Arm margin |
|---|---|---:|---:|---:|
| `.txt` |recovery window complete|16us|37us|35us,arm7us|
| `_02.txt` |arm refused|60us|38us|-9.5us|
| `_03.txt` |startup stopped, no follow evidence|unmeasured|unmeasured|not reached|

First: realstep3->4 onset11716/confirmed11756,mean862ticks,age146ticks. Resumed
~27.992s at387.210eHz,65032COM/65032accepted,sigma26.450us,raw352,bus10901mV.
Second: step5->6 onset12412/confirmed12452,mean867ticks,age236ticks; no arm.
DMAmax42us in that failure is compatible with the44us added bracket time,
but does not prove which interrupt (if any) preempted it. Need direct overlap
evidence or bounded controlled intervention; do not call60us pure CPU cost.
The nearly unchanged37/38us later path makes further blind optimization there
the wrong next experiment. Probe perturbs timing; no baseline qualification
transfer. Previous48..107us descriptions must retain their exact endpoints.

Third: CAPreason7,n0,energized7993us/control_ticks1/max_control_interval6992us,
no driven-transfer/follow records. Fixture reported missing expected-phase
provenance, but the motor had already stopped during startup. Separate failure
class, not another recovery arm refusal. Cause remains open; no retry.
All3 CRC/finaloff verified, gates/en/MOE/CCRs0,nFAULT1,Uartclosed.

Capture SHA256 row order:
- 3F50B51F50C6CB48781CCEB8B0706BABE359AC46EF5AF7B4B28347DD53FF6A7B
- 7BF4C4474A0FC425F997F400CA8B2E1663BF9626580DB220F50970F980DAF7B3
- 5770128E565683B5316F9E454263B7227B792C74B9E0E1EFF4BE68B9465D8452

Current3F3E installed/off. Cohort1/3 complete campaigns, NOT repeatable8%
recovery. Next identify DMA/guard progress across the measured qualification
bracket or justify a bounded mask A/B against unchanged guard deadlines.
Keep the startup failure separately accounted. Full goal remains unfinished.

## E653 — existing DMA scan count advances across slow qualification

Added aligned volatile read access to existing adc_stream STATE.count. Two
foreground reads bracket the timed software-qualification interval, with the
first just before its start timestamp and the second just after its end.
No new ISR bookkeeping or priority change. Counter span is slightly wider
than timer span; it cannot locate the exact interrupt entry. A zero delta
would not exclude other IRQs or DMA entries that delivered no scan. Original
scan-buffered edge path remains untimed by this observer.

132actual-loop simulations3modes/4hosttests PASS; host DMA seam is zero and
does NOT emulate preemption. Parser allows historical absent dma field but
requires policy_ticks if dma exists, rejects u32overflow. Release-s/thinLTO
build/audit PASS, frozen nextdmaobs_653 ELF SHA256:
D562078278F8E07E39BB8A9F28F16B1E5E0BD1A4FBEF5AE5C83DB8B570404BA5.
Download/OpenOCDreset exit0; own guard3/18,five3200 CPU pair2/6/9,ADCroute3 PASS.

One8%/30s diagnostic `captures/nextdmaobs_653_80_30s.txt` FAILED arm:
policy_ticks120=60us,dma1,handlermax42us. Realstep6->1 onset11694,confirmed11734,
interval802ticks,seed877ticks,age238ticks119us,remaining-19ticks=-9.5us.
DMA scan processing overlapped the wide qualification bracket. Combined with
E65216us pass versus60us failure, DMA preemption is the leading explanation
of the44us addition; this is not a controlled proof assigning every cycle.
No recoveryarm/pass. Initial4623COM/4622events,9955scans,raw392,bus10996,
injectedtracking8. CRC/firstarchive/finaloff PASS,Uartclosed. Capture SHA256:
124466A8B5BA813BD69E6FC66B24776D24F109B7B543763946D4DA6EF4269404.

CurrentD562 installed/off. Next scheduling review must cover qualification
THROUGH arm and first COM, not just mask NextEdge.edge: a pending DMA IRQ
would run immediately on unmask, moving the same delay before arm. Current
DMApriority64 also outranks COM128, so moving pending work after arm can delay
the first commutation. TIM6guard0, sample freshness and DMA lease deadlines
must remain satisfied. No IRQ-policy change staged; do not claim a short mask
solves this end-to-end constraint. Keep E652 startup failure separate.

## E654 — scheduling review rejects a local mask as a complete fix

Read current adc_stream start/poll, RunGuard.poll/feedback_aged and live priority
setup. DMA lease requires a coherent unread half with the expected parity;
both flags/overwrite refuse, and trigger-derived acquisition ages are retained.
Guard rejects publication if previous feedback is already older than1ms;
a newly completed scan cannot repair that gap. TIM6 guard has highest priority,
DMA64 outranks COM128. Therefore lease, first feedback and first COM deadlines
must be checked independently, not represented by one occupancy percentage.

Added `scripts/test_recovery_schedule_budget.py`: four conditional arithmetic
tests PASS. This is NOT a production admission rule or measured WCET proof.
The illustrative case baselineage832,pending53,defer60,DMA42 leaves13us of
first-publication slack. Adding45us COM preemption makes slack-32us while
the conservative201us next-scan budget still passes. Pending53 is an example,
not an independently measured arrival; observed maxima are not WCET bounds.
Masking just the16us policy leaves the same42us pending ISR before arm, so it
cannot remove the total delay. These counterexamples invalidate a simplistic
mask/priority flip argument; they do not prove hardware infeasibility.

No firmware change, flash, UART or motor command this entry. Actual D562
remains last verified off E653; no fresh off readback claimed. Next architecture
question is staging first DMA safety publication earlier while maintaining
continuous comparator acquisition and a single ADC owner, or reducing its
measured safety-publication latency. Do not delay first publication through
first COM without a valid remaining-feedback budget. Full goal active.

## E655 — exact bounded bus scaling removes one normal-path soft divide

Current DMA convert performs runtime VREF division followed by bus scaling
floor(n*1194/100), where n=floor(raw_bus*vdda/4096). Added bus_scale.rs:
for n<=4095, use12*n-ceil(6*n/100). The ceil is an exact bounded reciprocal
on6*n+99, with SHIFT21/RECIP20972. Const assertions bound the product and
reciprocal error. Other n uses the original wrapping multiply1194/divide100
behind an opaque cold fallback. ADC/VREF validity, lower bus threshold,
conversion rounding order and feedback timestamps are unchanged.

Direct rustc host tests exhaust0..65535 (normal domain plus fallback) and
wide/overflow boundaries:2PASS. Release-s/thinLTO build/audit PASS. Emitted
convert0800c03c retains one VREF __aeabi_uidiv; the normal bus scaling uses
32bit multiplies/shifts/subtract, no divide or wide helper. Compiler folded
6*20972 and99*20972 to125832 and2076228. Fallback at0800c3dc retains uidiv.
This proves code generation and arithmetic equivalence, NOT a timing win.

Frozen busscale_655 ELF SHA256:
305B7CF05A45F6A6580179DC934420C648910CB7AB9510203A4B3F993DB69086.
NOT flashed; actualD562 remains last verified off E653. NoUART/motor this
entry. Next candidate own disabled checks and one8%diagnostic to measure DMA
and qualification/arm timing. Do not infer this modest arithmetic change
eliminates the observed42us preemption or establishes higher-speed recovery.

## E656 — bus scaling measured; early-return ADC cleanup defect found

Installed305B with download/OpenOCDreset exit0, own guard3/18,five3200 CPU
pair2/6/9us,ADCroute3 PASS. One8%30s recovery PASS388.224eHz,65201COM=accepted,
sigma25.289us,raw406,bus10937,margin35us/arm7; policy16us,dma0,DMAmax35us,
firstpublication91us. TWO repeats declared before running:02 armFAILED
remaining-4us (age114us,seed878),policy45us/dma1,DMAmax27,firstpublication91us.
This is lower observed handler time, not a controlled WCET or sufficient fix.
03 failed during startup: CAPreason5,n1,energized1000us,low-bus message. That
message incorrectly says6V although code checks8400mV; not a measured PSU sag.
All captures CRC/finaloff PASS,Uartclosed. Cohort1/3,not repeatable8% recovery.

Capture SHA256 (`busscale_656_80_30s`, then `_02`, `_03`):
- DB09F264F7B077A1953050BC47F7EBE1DF4532C0405783F573C9EF8C40BB3628
- 681A4EC0DFDD9D3C1684D3657B8A051A0819B971C168607D75BEF0306C2B3159
- B287591C3BEA86F4D64FDE132E68C32529A2BEF0DC0C3B0B70E8E2D6A630FFAB

Source review found a concrete lifecycle defect: after guard/DMA startup,
early follow/arm refusals return from coast_run_inner before the sole normal-
path adc_stream::stop. gates_off stops gate/guard/wave owners, but does not
restore DMA ADC configuration. A subsequent foreground scan can inherit that
configuration. This is a source-proven missing cleanup; its responsibility
for each historical startup failure is not yet hardware-proven.

Staged a wrapper around renamed coast_run_body: every returned result first
safes gates/en, then calls foreground adc_stream::stop, preserving the result.
Normal-path cleanup remains before its existing passive capture. Never run
ADCSTOP loops in ISR shutdown. Host actual-wrapper test verifies true/false
results and gates->enable-low->ADC-stop ordering; inner body is stubbed, so
this is not a peripheral restoration test. Initial extractor test failed due
to private function naming; corrected extraction and complete test PASS.
Release-s/thinLTO build/audit PASS, frozen cleanup_656 ELF:
443836A7E9F86E61673AC12EEDE51A4E52F41911AED9DDC9A0ED108E73605058.
NOT flashed. Actual305B remains installed/off. Next own disabled checks and
refusal->foreground ADC/next-startup validation. No qualification transfer.

## E657 — cleanup verified through actual refusal and no-reset restart

Installed frozen443836 cleanup_656, download/OpenOCDreset exit0. Own disabled
guard3/18,five3200 CPU pair2/6/9,ADCroute3 PASS. One8%/30s recovery command
`captures/cleanup_657_80_30s.txt` armREFUSED:seed875ticks,age228ticks114us,
remaining-9ticks=-4.5us. Realstep1->2 onset11696/confirmed11736,interval794,
policy_ticks34=17us,dma1. No recovered segment. CRC/firstarchive/finaloff PASS.

After that refusal, WITHOUTreset/reflash, native MCP COM41 commands a,p,i:
VDDA3317mV,VBUS11760mV; all six gates/en/MOE/CCRs0,nFAULT1. Shell idle phase
current conversions are uncalibrated and not current evidence. MCP connection
be498ba9-f04d-4c78-a081-fc6b94d7eaa2 closed before the next fixture.

Then one7%10s hold (same carrier/guard/seed/DMA flags, no dropout/reentry),
`captures/cleanup_657_after_refusal_70_10s.txt` PASS:338.000eHz,20280COM/
20279accepted,10000013us,raw321,bus10972mV,arm7us. CRC/ADC/phase/timeline/off
checks PASS, UARTclosed. Actual443836 installed/off. This is hardware evidence
of successful ADC/restart ownership restoration after an arm refusal, not
proof that every historical startup fault had that cause or an8% recovery pass.

Capture SHA256:
- 6FB0515A194E46B8AF33D17ED3F8994818CBE3600FF10BA5C7810021991DC7F0
- 1400CE755A5E2C17637792FD7BE2284BF19B3FAD0754B2B5993D44A7ECEA9B31

Observer limitation is now demonstrated: dma1 with policy17us means scan
progress need not be inside the narrower timed policy bracket. Counter reads
are outside timer reads, and COMP level is read before its timestamp. An IRQ
between adjacent observations can shift attribution or the assigned sample
time. E653 overlap remains evidence over its stated wider bracket, not exact
IRQ placement. Next require coherent short COMP/timestamp/counter snapshots
before stronger causal attribution; don't mask the entire policy on an
overstated timing claim. Recovery arm deadline remains unresolved.

## E658 — IRQ-coherent observations confirm scan progress inside qualification

Changed only follow_sweep observations: short interrupt::free groups contain
COMP level/TIM17/existing DMA count at each sample, and TIM17/count at the
successful policy endpoint. No IRQ can split those observations. Hardware
reads are still sequential, not simultaneous latches; timer resolution is
1 us. Dwell20us, gap refusal, edge onset, qualification, seed/arm floors and
electrical guards are unchanged. Qualification itself remains interruptible.
Cleanup from E657 is retained.

Host actual-loop132 cases in each of three modes PASS; counter stub now
asserts MASK at every endpoint. Parser and actual cleanup-wrapper tests PASS.
This verifies structure/semantics, not hardware WCET. Release-s/thinLTO/
codegen1 build and emitted arithmetic audit succeeded; frozen snapshot_658:
EA33255560112765FC0EFAF2214B10CC493BD3B61BFAC9CE0CC1EA2881A0EFDB.
In follow_sweep the three masks at 08009326,080093fa,080094ea contain 5,7,5
load/store instructions respectively before PRIMASK restore. No call, branch,
loop or arithmetic helper inside them. This is an emitted-code bound on
instruction structure, not measured peripheral-access latency.

Download/OpenOCD reset exit0; own disabled guard3/18, five3200 preflights with
CPU pair2/6/9us, ADCroute3 PASS. One powered command:

```text
python scripts/drv_driven_handoff.py --out captures/snapshot_658_80_30s.txt --ms 30000 --drive-duty 61 --bemf-duty 80 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle450 --seed400 --seed-timing-reanchor --fast-cycle-report --event100 --dma-guard --dropout --reentry --reentry-pwm-stage --reentry-guard-stage --reentry-next-edge --follow-expected-phase
```

FAILED recovery arm: seed878 half-us ticks, age230=115us, remaining-10=-5us,
no arm performed. FOLLOW step2->3, onset12362/confirmed12402, interval794,
policy90ticks=45us, dma1; DMA reported max26us. Final stage5 count0 means
the completed follow seed was cached during setup, not freshly sensed there.
The atomic endpoints now put that scan progress INSIDE the timed bracket,
unlike E657's ambiguous dma1/policy17 pair. This supports a scheduling delay
inside qualification; it does not attribute all45us to DMA, give exact ISR
entry/exit, or establish a worst-case bound. Snapshot correction alone did
not fix the recovery deadline, and this is not an8% qualification pass.

Capture SHA25602336F705ABC964F38F5AFAAEA25DB4B64A61ED7F8CCFE622D292D1E852E5944.
Fixture verified CRC/first archive/finaloff before reporting refusal; separate
CRC/off replay PASS. Final p/i all six gates/en/MOE/CCRs0,nFAULT1. UARTclosed,
actual/root EA332 installed/off. No identical retry or threshold change.
Next work must remove real deadline-path cost or resolve the scheduling
conflict with an explicit freshness/lease budget, not add more ambiguous
observers or mask through the first commutation without that budget.

## E659 — staged running-timer publication policy, no hardware authority yet

E658 was progress: its coherent observations place scan progress inside the
timed qualification path. But eliminating that DMA delay alone does not solve
high-speed recovery: actual reference arithmetic still gives41.5us at333
half-us interval ticks, less than20us dwell+32us old full-arm allowance.
The goal therefore needs a different final transaction, not endless8% repeats.

Added standalone no_std `examples/support/prepared_handoff.rs` prototype.
Its intended adapter starts an IRQ/gate-inhibited running timer BEFORE the
final edge. After the existing acquisition validates a real edge, publish
the exact onset+reference wait into its absolute ARR (target-1, since overflow
is ARR+1). This does NOT predict a crossing, shift the onset to confirmation,
or introduce free-running commutations through missing edges. The policy
permits one commit only after publication and within an explicit ISR lateness
budget. No edge, cancellation, stale clock/deadline, late publication/service
or duplicate transaction refuses/latches. Existing electrical/ownership and
qualified-input checks remain indispensable adapter responsibilities; the
prototype is not itself an acquisition or electrical safety implementation.

Four Rust tests pass, including2280 origin/edge-offset/wait combinations,
u32 wrap, exact ARR+1 timing, publication boundary, original deadline,
cancellation, missing edge, late service and duplicate refusal. The example
atwait83 uses hypothetical20us confirmation+6us policy work+8us publication:
it fits arithmetically; adding26us DMA delay correctly refuses. Neither6us
nor8us is a measured timing result or a new admitted live guard setting.

`python -m unittest scripts.test_prepared_handoff scripts.test_recovery_budget scripts.test_recovery_schedule_budget`
PASS: prototype4Rust tests, actual reference3Rust tests, conditional budget4
Python tests. Automated thumbv6m-none-eabi opt-s assembly check verifies
publish/service remain emitted and contain no BL/BLX/division/aeabi helpers.
Initial standalone compile needed no_std; added it. Initial assembly test
incorrectly expected an outlined trivial cancel symbol and a .cpu directive;
corrected to require actual publish/service code and16-bit encoding. No
claim of whole-firmware LTO codegen or hardware WCET from this standalone test.
Prototype source SHA256:
F25679F78D99B1EA01FD2DD21638D9771268CFBE82A4190EC6D2C7B6CA111903.

NOT linked into shell-pwm, flashed, or powered. Actual/rootEA332 remains as
last verified offE658; no fresh UART/off readback claimed. Full goal active.
Next implement/measure a DISABLED-only timer adapter: establish a coherent
TIM17/TIM16 origin, keep source/NVIC masked while preparing, publish ARR with
current-count/pending-state checks, and stop/clear on every refusal. Before
powered integration, validate final publication WCET, COM lateness, ADC
freshness/lease, and original campaign deadline. The prototype local span
must be bounded by BOTH remaining campaign time and counter-wrap horizon;
it cannot replace a long campaign deadline with a newly extended deadline.
Do not lower the old32us allowance in the existing full-arm sequence: this
is a proposed split transaction, not a waiver for the current implementation.

## E660 — disabled prepared-timer hardware prototype, mixed first evidence

Added opt-in `bench-prepared-timer` and shell `preparedcheck`, entirely separate
from powered handoff. The primitive requires running/nonpreloaded TIM16,
DIER0, NVIC masked/nonpending and clear UIF; checks >=16 half-us ticks to an
absolute ARR+1 deadline, writes ARR, rechecks count/UIF, enables DIER while
keeping NVIC masked. Refusal calls Timer::stop. The16-tick allocation is a
disabled experiment only; no live32us floor changed. Harness checks all owners
idle/bridge disabled, explicitly safes, and never unmasks the COM vector.

Five modes,32 each: nominal100us absolute deadline after~50us elapsed;
too-close deadline; staleNVIC; staleUIF; stopped counter. Every exit checks
stopped CEN/DIER and masked/cleared NVIC. Waits bounded at1000us. The TIM17
bracket around counter start measures only origin uncertainty; a coherent
cross-timer origin mapping for real edge timestamps is still unimplemented.

Built release-s/thinLTO/codegen1 with no defaults, features
`bench-single-core-atomics,bench-prepared-timer`. This is a small diagnostic
build, NOT the full powered configuration. Initial compile found an unguarded
driven_run reference; cfg-gated that ownership check and rebuilt successfully.
Both final builds math-audited and frozen:

- preparedtimer_660 ELF4B6B1826F4BDB67E2BCB178EDB25C56754C399761BB60FB2D0CE089BD0E6DE11.
  First hardware attempt FAILED:0accepted/160refused/32failed; maxpublication2us,
  originspan1us. Refusal cause not recorded in this boolean version.
- preparedtimer_660b ELF0FA65AED232E4A89C1A211792D8A199B65B50128EE331FF85AB1A76E23C0AA1A.
  Replaced compound boolean refusals with per-condition reason codes, retaining
  the same predicates. One disabled attempt PASS:32accepted/128refused/0failed,
  maxpublication3us,originspan1us,absolute-deadline error1us. The publication
  maximum includes refusal/stop paths. No actual COM ISR was serviced.

The first failure is UNEXPLAINED, not retired as fixed by adding a reason code.
Neither build's result transfers to the other or proves loaded WCET. Need
emitted/read-order or initialization bisection before claiming reliable timer
preparation. Keep that work bounded; no automatic repeat-until-pass.

Captures:
- preparedtimer_660_disabled.txt SHA2562CCA223DE0C629AAF58359CCD41B7A53C2D97DEFBA4C62EB65BEF533DDE5BC7B
- preparedtimer_660b_disabled.txt SHA25696D134A6A315C5180A9348AB91C260151E14966D9241E2A2720BF8B15C5ABBDD

`scripts/drv_prepared_timer_check.py` verifies counts, disabled status,
finaloff and experimental8us/2us/3us limits. Regression test retains the first
failure, accepts the second capture and rejects mutated timing/count/mask
fields. `python -m unittest scripts.test_prepared_timer_check scripts.test_prepared_handoff`
PASS (including4pure-policy Rust cases and standaloneM0 check).

Restored exact frozenEA332 snapshot_658 after testing (all download/reset
exit0), fresh guard3/18/finaloff PASS in preparedtimer_660_restored_guard.txt.
UARTclosed; no motor command this entry. ROOT ELF remains diagnostic0FA65,
NOT installed. Next resolve initial nominal refusal and cross-timer origin,
then loaded publication/COM/feedback budgets before any powered integration.

## E661 — failed boolean prototype reproduces; no register-level cause yet

Read objective/current state; E660 was progress (first real disabled timer
results), not qualification of the powered path. Inspected emitted boolean
4B6B and reason-coded0FA65 checks. Both include NVIC enabled/pending, CR1,
DIER, UIF, remaining-count and post-write checks; no source-grounded reason
to call a changed numerical allowance the fix. The boolean version inlines
into the large shell main and uses spilled predicate operands; this alone
does NOT prove compiler error or stack corruption.

One declared reproduction on exact frozen4B6B after download/OpenOCDreset:
`python scripts/drv_prepared_timer_check.py --out captures/preparedtimer_661_old_01.txt`
again FAILED0accepted/160refused/32failed, max2us, origin1us, outputs-off
verified by fixture. Capture SHA256:
CFC6ED297EB435E294818C106A58F60B09B2535BAA4562439200A166F7170EA1.
This reproduces the bad candidate, not a whole-rig inability to publish ARR.

Attempted non-overlapping debug/UART refusal inspection: OpenOCD installed
hardware breakpoint0800c958, resumed and exited; MCP opened COM41, sent only
preparedcheck and closed; subsequent OpenOCD poll/reg failed to read PC.
Therefore no captured register values or completed test result from that
attempt, and no claim about the refusing predicate. Do not cite the planned
breakpoint as if it observed execution. MCP connection
c7a823f5-a01d-486c-acac-691e6737a972 closed before SWD reconnection.

Restored frozenEA332 (download/reset exit0), fresh guard3/18/finaloff PASS in
preparedtimer_661_restored_guard.txt. UARTclosed, no motor command, no live
guard change. Root remains diagnostic0FA65 rather than installedEA332.

Retain/retire the opaque failed boolean prototype; further identical runs
cannot identify its branch. Keep explicit reason codes in the proposed
adapter. Next meaningful work is coherent cross-timer origin and deadline
publication in that diagnosable implementation, followed by measured loaded
COM/feedback constraints. The earlier failure remains unresolved, and the
one0FA65 pass remains scoped to its own disabled build, not powered reliability.

## E662 — explicit TIM17-to-TIM16 deadline mapping tested on hardware

Implemented map_deadline in the staged policy. An IRQ-masked TIM17-before /
TIM16 count / TIM17-after snapshot maps the original reference deadline onto
the running counter. Reject odd half-tick representations, >1us read bracket,
zero publication budget, late/ambiguous deadline, and first-counter-wrap
overflow. Uses the late endpoint rather than allowing an early commutation;
returned uncertainty is bracket+2 half-us ticks (TIM17 quantization), <=2us.
This is NOT zero-error synchronization. The integration must account for that
uncertainty against the original campaign deadline and COM lateness allowance.

Pure tests:3200 mapping combinations acrossu32 wrap/read phases, late/broad
bracket/overflow refusals, plus existing transaction tests:5Rust cases PASS.
Automated standalone thumbv6m opt-s check now requires emitted map_deadline
as well as publish/service, and rejects calls/division/aeabi helpers:PASS.

Extended disabled preparedcheck with32 mapped-deadline trials (192 total),
keeping COM NVIC masked and original four invalid-state modes. A mapped
deadline is100us after the original TIM17 start reading, not100us after
qualification/mapping. New PREPAREDMAP row measures the complete masked
snapshot+mapping bracket separately from publication. Host parser requires
that row for192-case results and retains historical160-case compatibility;
missing/wrong/count/overbudget mutations and prior failure capture tests PASS.

Build command: cargo build --release --no-default-features --example shell-pwm
--features bench-single-core-atomics,bench-prepared-timer. Release-s/thinLTO,
math-audited; frozenpreparedmap_662 ELF:
5F8C41A9359ECF0BFC65CF2D654EBB840C41E1C625D0333758628071CFF79444.
Downloaded/reset successfully. One disabledhardwarecheck PASS64accepted,
128refused,0failed; maxmapping2us, maxpublication3us, originspan1us,
maximum observed deadline error1us. Both maxima include observer overhead;
their sum is not a loaded whole-handler WCET. No COM ISR, guard tick, or ADC
DMA interference was exercised, and no live32us floor was changed.

Capture preparedmap_662_disabled.txt SHA256:
078A0AFF16348F96B89D0274209468D2939B8EA6C6863FD91E5F2A642D744226.
Restored exactEA332 snapshot_658 (download/reset exit0); freshguard3/18 and
finaloff PASS, UARTclosed. Root remains5F8C diagnostic, NOT installed.
No motor commands or transferred recovery qualification. Next is loaded
transaction/one-shot COM ownership and feedback-budget validation, not more
identical unloaded trials. The old4B6B failure remains retained/unexplained.

## E663 — prepared deadline under real guard/DMA interrupts

Added opt-in bench-prepared-load and shell preparedload. Real DRV wake with
six gates/MOE held off; actual five-channel initial feedback with conservative
age from before the scan; normal prepared guard admission, TIM6guard and
ADCstream/DMA feedback validation. No synthetic electrical sample, gate commit,
or powered BEMF owner. Diagnostic COM ISR only, priority128 against guard0 /
DMA64. Each600us trial drains the normal feedback FIFO while sweeping its
single COM deadline100..317us in7us steps relative to DMA startup. Every
iteration stops COM/guard, disablesEN and restores ADC; all early refusals
reach the same cleanup. ISR count must advance exactlyonce, not free-run.

Build: release/no-defaults, bench-single-core-atomics,bench-prepared-load;
feature includes real DMAguard/201us/fast-start/peer plus driven-DMA ownership.
Initial compile correctly refused missing sequential-TIM3 dependency; added
bench-driven-dma, successful release-s/thinLTO build and math audit. Frozen
preparedload_663 ELF:
246E963A63416FCC22A50961B3A8A11A021D223A6622E55EC50A2E599D2757E9.

Own disabled192cases PASS64accepted/128refused, publicationmax3us,origin1us,
deadlineerror1us. Then one32-offset load sweep PASS32trials/32events, no
refusal or guard fault, minimum3 validated feedback deliveries per trial,
all outputs off. Maximum diagnostic COM entry lateness24us; maximum DMA ISR
time24us. This is compatible with DMA blocking lower-priority COM, not direct
per-event attribution. Fast ARR publication does not remove that delay.

This is NOT timing qualification of the powered handoff: COM body is the small
diagnostic handler, initial baseline is freshly scanned (not the aged~830us
recovery baseline), observation windows600us, and no comparator qualification
runs concurrently. Priority/guard/ADC ownership are real, but loaded production
WCET and original high-speed recovery deadline remain open. Parser explicitly
returns timing_qualified=False; it validates count/fault/feedback/off semantics
without declaring24us an acceptable commutation error.

Captures SHA256:
- preparedload_663_loaded.txt:5D924514942BD696E4E0E4536FA7A7B163357246BCDCC87790A65D6CC4EB1BC7
- preparedload_663_disabled.txt:078A0AFF16348F96B89D0274209468D2939B8EA6C6863FD91E5F2A642D744226

Regression tests retain loaded timing-not-qualified status and reject event,
refusal, missing-feedback and failure mutations. Existing mapped/historical
captures and5Rust policy tests/M0codegen PASS. Restored frozenEA332 after tests,
freshguard3/18/finaloff PASS in preparedload_663_restored_guard.txt; UARTclosed.
Root246E diagnostic differs from installedEA332. No motor command this entry.
Next a bounded diagnostic COM-priority A/B with actual feedback preserved,
then account for the full first-COM body; no more ARR-only micro-variants.

## E664 — same-build COM priority A/B resolves diagnostic service delay

Added preparedload0 alongside preparedload. Both invoke the SAME helper with
priority0 or128; guard stays0, DMA64, waveform/gates stayoff. No production
core priority changes. Results carry a mandatory priority field for new A/B
fixtures; old archival rows still decode without pretending a priority was
measured. Host tests reject missing/wrong identity for requested comparisons.

Release-s/thinLTO/no-default build, bench-single-core-atomics,bench-prepared-load;
math-audited/frozenpreparedpriority_664 ELF:
04BC49A31E6A688215CC7EBCA4D09AC71565928E895A5DB0D2C72B7965CB16C6.
Own192case unloaded test PASS. Two declared load sweeps in order128,0, no
reset/reflash between them; each32offsets, real ADC/guard,600us windows:

| COM priority | max diagnostic COM lateness | max DMA ISR | events | min feedback/trial | guard faults |
|---|---:|---:|---:|---:|---:|
| 128 | 23us | 23us | 32/32 | 3 | 0 |
| 0 | 4us | 25us | 32/32 | 3 | 0 |

This intervention supports lower-priority COM blocking as the cause of most
of the diagnostic delay. It does NOT revive E478's falsified comparator
mid-persistence explanation. Priority0 COM peers with the unchanged guard;
DMA can be preempted. Its higher observed max is consistent with that cost,
not an independent WCET measurement. The diagnostic COM ISR is small; the
real commutation body and~830us-aged recovery baseline remain untested here.
No live priority switch or timing guard relaxation is inferred from the pass.

Captures SHA256:
- preparedpriority_664_p128.txt DCF64FD737CCD712096A6CDFB003FB870F03AE676CAFCEBF4DAFE04DAB5EB2C1
- preparedpriority_664_p0.txt 02314ED3FB22811904849A4016303D616E0DF3F2228287B1EEB669D614542D81

Parser identity/mutations and all prior prepared tests PASS (5Python tests,
including5Rust policy tests and standaloneM0codegen). Restored frozenEA332,
freshguard3/18/finaloff PASS in preparedpriority_664_restored_guard.txt.
UARTclosed; root04BC diagnostic differs from installedEA332. No motor run.
Next connect the prepared transaction to first-COM ownership with the REAL
body and feedback budget; this comparison is enough to stop repeating the
small-handler probe. Qualification still requires full-path powered evidence.

## E665 — real recovery adapter staged; flash budget still prevents candidate

Added opt-in bench-prepared-handoff. After real guard/DMA startup and before
follow checkpoint4, prepare TIM16 running with interrupt/gate authority off.
Preparation checks owner/readiness/disabled state atomically with initialization.
Timer::stop revokes the publication latch. The existing final handoff critical
section consumes that latch once, revalidates ownership, maps the qualified
onset+wait deadline, publishes absolute ARR and sets COMpriority0. It unmasks
COM only after controller metadata and the original full-arm timing check.
Initial startup stays on its original relative-arm path. No early gate write.

IMPORTANT: the current32us remaining floor and16us full-arm budget are retained
at this staging step. This does not yet solve1kHz admission arithmetic, nor
claim the diagnostic3us publication bound covers this full wrapper. FLY_SEEDED
now becomes true only after successful timer publication AND timing check;
failed publication must not claim an armed seed. Core metadata, ownership,
first-COM delay and aged feedback need integrated validation before powered use.

Actual publish_recovery_deadline extracted into a mocked-hardware Rust harness:
32 combinations of prepared/owner/readiness/outputs-disabled and publication
success/failure, plus duplicate and late-deadline refusal PASS. Wrapper must
consume latch once, stop on refusal, avoid publication after revoked ownership,
and selectpriority0 only on success. This is NOT hardware/IRQ interleaving
proof; peripheral seams and enclosing critical section are mocked. Existing
5pure policy tests and standaloneM0 codegen tests PASS. Final atomic prepare
check placement was reviewed but has not been rebuilt/tested in an image.

Build attempts, allrelease-s/thinLTO, all refused before any flash:
- Full E658 feature set plus bench-prepared-handoff:384B FLASH overflow.
- Omit optional bench-irq-tail only:192B overflow. Electrical ADCphase, guard,
  accepted events, CPUunion remain selected; no qualification transferred.
- Force helper outlining:288B overflow, worse; reverted that experiment.

No candidate ELF exists. Root shell-pwm hash is still the PREVIOUS successful
diagnostic04BC49A31E6A688215CC7EBCA4D09AC71565928E895A5DB0D2C72B7965CB16C6,
not these source edits. ActualboardEA332 remains last verifiedoffE664; no UART,
SWD or motor operation this turn. Keep full goal active. Next deliberate
code-size cleanup, explicit candidate wire marker/fixture checks, then its own
disabled and full-path timing/feedback qualification. Do not flash stale root
or silently substitute opt-z/remove electrical guards just to fit.

## E666 — candidate fits and runs; recovery refuses sensing gap before publication

Code-size cleanup: shell a still reads/reports VDDA, bus, three CSA voltages,
VSENC and neutral, but no longer prints uncalibrated nominal mA using an
assumed VDDA/2 offset and70mV/A gain. Removed those misleading conversions.
Prepared-handoff builds omit the LED blink demo; other builds retain it.
Optional IRQ tail recorder omitted from this candidate, keeping CPUunion,
electrical/ADCphase guards and event archives. No opt-z or guard changes.
Initial cleanup stilloverflowed64B; LED omission plus explicit configuration
marker now fits: text129816+data1228=131044 bytes (28bytes below128KiB).

Successful release-s/thinLTO/codegen1 build uses the E665 no-irq-tail feature
set plus bench-prepared-handoff. Frozenpreparedhandoff_666 ELF:
AC54842890D3D3D573947E0905D71A860F1CB4092D3C7D4F7B7C021D8DEEB22A.
Math audit completed. Marker PREPAREDHANDOFF floor=64 arm=16 priority=0 is
CONFIGURATION identity only, not proof an arm occurred. Host fixture requires
--prepared-handoff for that marker, rejects mismatch/unknown configuration,
and requires next-edge mode when recovery is requested. Marker/wrapper/policy/
cleanup tests PASS. Old32us admission and16us arm limits remain unchanged.

Downloaded/reset exit0. Own guard3/18, five3200 disabled preflights with CPU
pair2/6/9us and ADCroute3 PASS. One powered command:

```text
python scripts/drv_driven_handoff.py --out captures/preparedhandoff_666_70_30s.txt --ms 30000 --drive-duty 61 --bemf-duty 70 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle450 --seed400 --seed-timing-reanchor --fast-cycle-report --event100 --dma-guard --dropout --reentry --reentry-pwm-stage --reentry-guard-stage --reentry-next-edge --follow-expected-phase --prepared-handoff
```

FAILED recovery before timer publication: FOLLOW result5,stage4,reads3,
maximum gap244half-us ticks=122us against unchanged100us sampling limit.
Priorstep3/onset12964; no next qualified seed. DMAmax26us. Initial segment
reached its2,000,019us injected loss; no recovered segment or successful
prepared arm is claimed. Marker does not override that failure. No identical
retry or gap/arm guard relaxation. Capture SHA256:
DB6079DDEA1AD005A3F7F751D98824594E6074BB4BBEA1C78897D07A32517F73.
Separate raw CRC/finaloff replay PASS; UARTclosed. Actual/rootAC548 installed
and off, not recovery-qualified. No restoration to another image this entry.

Next review placement of prepare_recovery_counter (currently between real
DMA service/start and follow4), preserving continuous comparator filters.
The capture localizes failure to that setup/sensing interval; it does not
prove which individual instruction/IRQ caused the gap or a TIM16 defect.
Move invariant preparation earlier or split it from counter activation rather
than widening the sensor gap. Full COM/aged-feedback qualification stays open.

## E667 — remove unused final phase visits; first prepared recovery completes

Variant A removes the duplicate Timer::init during recovery-counter activation.
observe_begin already initializes TIM16 before acquisition. Activation now checks
CR1/DIER/UIF/PSC31/ARR65535 plus owner/readiness/off under the critical section,
then writes only CNT0/CEN1 and the prepared latch. Actual extracted activation
harness:96 owner/config cases PASS, with zero writes on refusal. Existing
32-case publication wrapper tests PASS. This does not claim hardware WCET.

Frozen preparedactivate_667 ELF:
B21D1344F6878F34293F0C88889A05B2A37F4395600D6EC5060148C172B2EA54.
Own disabled guard3/18, five3200 CPU2/6/9 and ADCroute3 PASS. One7%30s recovery
FAILED follow5/stage4/reads3/gap244half-us ticks=122us, before publication.
Capture preparedactivate_667_70_30s.txt SHA256:
05ADE4B35D7C9257B12EB16B4DE449FC67504BA112D96F7416768ADF4EDB2F0E.
CRC/off PASS. Removing duplicate initialization alone was insufficient.

Variant B changes only final setup checkpoint4 to expected-phase-only when
BOTH bench-prepared-handoff and bench-follow-expected-phase are selected.
The following final wait already senses only that known next phase. Stages
1/2/3 retain all-phase sweeps. No filter reset, timestamp refresh, gap/dwell
limit or original deadline changes. This avoids rejecting stale unused phases
in the final sweep, not proof the previous relevant-phase sampling was faster.
Actual extracted follow-loop tests132 cases in each of four feature modes PASS;
prepared final setup reads one phase instead of four, with refusal tests retained.

Release-s/thinLTO/codegen1 build and emitted math audit PASS. Frozen ELF
captures/reference/preparedfinal_667b/shell-pwm.elf SHA256:
B1E54D5FB0FF5FE0B44C4C3241CECEC5BD5BCB0A046417352814FADF09897D74.
Download/reset exit0. Own guard3/18, five3200 CPU2/6/9, ADCroute3 PASS.
One powered test, same E666 command flags with output
captures/preparedfinal_667b_70_30s.txt. Capture SHA256:
7EE2D33FB973B4DC3873A52686B6A24A6DD81C90AB1198FE7E728589687E0948.

Recovery completed: measured seed992half-us ticks; next step4->5 onset14068,
confirmed14108 (20us dwell), follow result1. Remaining ARR102half-us ticks,
arm12us within original16us limit, actual armed1. Recovered segment27.990734s,
56862 COM/56861 accepted,338.576eHz, cycle sigma26.551us, rawpeak363,
busminimum10984mV,139257 ADCphase records, stack2584. Timeline/original end,
tracking-loss stop/recovery, CRC and final outputs-off validated. No calibrated
amps or independent rotor measurement is claimed. DMAmax77us is an observed
preemptible duration, not exclusive work or mean occupancy.

First fixture exit1 was POST-RUN: old priority verifier required COM64, while
prepared publication intentionally selected COM0. Added explicit prepared-
recovery expectation with exact configuration and armed1 checks; other modes
retain old priorities. Mutation tests reject missing config/arm and wrong
COMP/COM/guard/DMA values. Replayed the SAME capture with --infile and all
original flags: PASS. No motor rerun to repair a host validation error.

Actual/root B1E54 installed/OFF, UARTclosed. This is one recovery pass, not a
cohort, high-speed recovery or transfer of old20% qualification. Original32us
admission remains. Next bounded repeatability and full COM/aged-feedback timing
review before extending recovery speed. Read incoming high-speed memo again;
E634 GRAYBEARD_HIGH_SPEED_RESPONSE corrections remain authoritative: no
free-running COM assumption, no calibrated-ripple claim, no maxima-as-mean CPU
budget or passive128us cadence substituted for powered201us cadence.

## E668 — prepared recovery 3/3 at7%; first8% recovery passes

Previous turn classified progress (first integrated prepared recovery). Read
the objective again; full up-to30% envelope/recovery/parity goal remains active.
Unchanged B1E54 image, release-s/thinLTO, no firmware/guard/priority edits or
flash. Fresh disabled guard3/18 PASS (preparedfinal_668_guard.txt).
Declared two additional7%30s recovery attempts, both executed and retained:

| Capture | eHz | COM / accepted | sigma us | raw peak | bus min mV | arm us |
|---|---:|---:|---:|---:|---:|---:|
| preparedfinal_668_70_30s_02.txt |337.789562|56730 /56729|28.435597|307|10925|12|
| preparedfinal_668_70_30s_03.txt |337.875118|56744 /56743|27.977317|342|10948|12|

Both fixture exit0. Together with E667 these are3/3 campaigns on THIS build,
each with injected tracking loss and approximately28s recovered operation.
IRQ union57.629..58.069% over the three recovered segments; valid accounting,
not total CPU utilization or exclusive per-vector work. Arm12us all three,
within16us. Additional-run seed1005/992, remaining127/124half-us ticks. First
feedback baseline age830/812us, decision91us; no freshness fault. COMmax38us,
commit14us, COMP40us; DMAmax77us may include preemption. No causal exclusive
DMA cost is inferred from that duration. Initial startup raw462/660 differs
from the recovered-segment peak and is retained in the captures.

Then one8%30s recovery, same flags except --bemf-duty80, PASS:
preparedfinal_668_80_30s_01.txt. Recovered27.991496s at388.112237eHz,
65183COM/65182accepted, sigma25.968948us, rawpeak360, busminimum10614mV.
Seed865ticks, new observed edge12460 confirmed12500, age at arm124ticks,
remaining92ticks, arm12us. Full COMP40/COM38/commit14us maxima, guard21us,
DMA77us, first feedback830us initial age+91us decision with fault0.
IRQ union59.020834%, valid. Original deadline/timeline, electrical/ADC checks,
tracking-loss shutdown/recovery and finaloff all passed the campaign verifier.
Independent raw CRC/finaloff replay passed all three new files. UARTclosed.
No calibrated current, independent rotor-speed measurement or8% cohort claim.

Commands: E667 flags unchanged, with outputs above, --ms30000 --drive-duty61
--bemf-duty70 for the two cohort runs, then80. No retries or exclusions.
Capture SHA256:
- preparedfinal_668_70_30s_02.txt:3BF9795B6EDC1C94AD3149BCC673B571CDD742E0FF7954BFA820169BE7D3E208
- preparedfinal_668_70_30s_03.txt:0729CFA8163A1CF1486A34DDF86EEAEC814847078703521C0344F3DB8EEA2AA1
- preparedfinal_668_80_30s_01.txt:76A70B2447B5D6BAE9A2DD4339BA62E59252B8ED0E59E90000BB9FEAD7AC5045

Next is not more7% cohorts. Current recovery admission remains seed400 and
32us remaining allowance. At the observed124half-us age, planning arithmetic
needs3ticks less latency at450eHz and21 at500; current seed profile rejects
those speeds independently. At1kHz, wait is~84half-us ticks (~42us), already
less than the observed62us edge-to-arm age even BEFORE the remaining allowance.
This is not a hardware speed ceiling (sustained1kHz already observed on another
build). Prepared publication now works, but final sensing-to-publication setup
must be shortened/restructured and its admission budget justified to deliver
high-speed recovery. Merely raising seed400 cannot achieve that end state.

## E669 — remove unaligned result copies;8% attempt still refuses arm margin

Previous turn progress: integrated recovery3/3 at7% and first8% pass. Objective
read again, goal remains full envelope including high-speed recovery, not just
these low-duty cohorts. Inspected the post-confirmation path and emitted code.
NextEdge::edge called Acquisition::edge then map_err(...)?/ok_or(...)?; LLVM
generated TWO eleven-byte unaligned __aeabi_memcpy calls (B1E54 addresses
0800c874 and0800c884). This is concrete extra work after edge confirmation.

Replaced only that nested conversion with an explicit scalar destructuring
match and reconstructed Seed. Every Acquisition::edge call, history update,
original deadline, confirmation window and terminal refusal remains unchanged.
Did NOT implement the initially considered read-only final validator or remove
history accounting.45 actual policy tests PASS, including all-sector history,
refusal latching and wrap checks. Actual follow-loop132 cases in each of four
feature modes PASS; prepared identity tests PASS.

Release-s/thinLTO/codegen1 build with exact E668 features succeeds. Math audit
SHA2568DD2B04882A5FCA8AB7CBA38B19A305A60C19043E5F2C13F03CF78D4FD1C76DB,
frozen captures/reference/followscalar_669/shell-pwm.elf plus disassembly/JSON.
NextEdge::edge now has only the Acquisition::edge call; both memcpy calls are
absent and payload uses aligned ldmia/stmia. Stack allocation44->28 bytes.
This is emitted-code evidence, not WCET or full-path improvement by itself.

Download/reset exit0; own disabled guard3/18, five3200 CPU2/6/9 and ADCroute3
PASS. One8%30s recovery command with E668 flags and output
captures/followscalar_669_80_30s.txt FAILED before timer publication:
measured seed862, actual edge onset11546 confirmed11612 (33us), arm observation
age162half-us ticks (81us), remaining54ticks=27us<unchanged32us floor.
Armed0, arm_us0, no recovered COM. Follow result1/step2->3, policy46ticks=23us,
DMA counterdelta0 in its coherent bracket. Prior E668 event policy54ticks=27us;
these different events are not a controlled WCET comparison. Confirmation grew
from20 to33us, and total timing did not meet admission. Do not claim the copy
change caused the failure or solved the recovery bottleneck.

First feedback: initial age812us, decision91us, fault0; DMAmax26us. Raw CRC and
final outputs-off independently PASS. Capture SHA256:
65689CF8914B7929E4E05E8576AA60D7CD9B62983E47496EE2F5FBADB522B426.
No retries/threshold edits. Actual/root8DD2 installed/OFF,Uartclosed, not
recovery-qualified; known-qualified B1E54 remains frozen, not installed.
Next reduce/restructure confirmation-to-arm work with the original onset and
shutdown checks preserved. No further speed-profile expansion follows from
this refusal alone; current32us arm allowance still cannot fit1kHz recovery.

## E670 — staged validation during confirmation, full-state differential tests

Previous turn progress: emitted copies removed and failed8% attempt localized
remaining timing. Objective read. This turn makes a source-only policy change;
no UART, flash or motor commands, and no fresh outputs-off claim. Actual/root
8DD2 remains last verified offE669. The staged API has NO live caller yet.

Added PendingFollow transaction to flying_acquire.rs. prepare_edge takes an
exclusive mutable borrow of NextEdge, snapshots Acquisition, and executes the
same Acquisition::edge policy on the tentative onset. The intended caller can
do this while the existing20us physical confirmation dwell is in progress.
No Seed/result is exposed during this transaction and there is no timer/gate
authority. If the level cancels or a different onset replaces it, dropping
the token restores EVERY acquisition field; NextEdge remains reusable.

finish requires matching phase, level and onset plus actual confirmation time.
It preserves original ordering of terminal/unqualified, original40,000tick
deadline and40..240tick confirmation checks. Only then does it retain the
already-computed acquisition update and latch Consumed or the original fault.
Early refusal restores acquisition but retains the correct terminal refusal.
Exclusive borrowing prevents another foreground method from observing tentative
state. This pure API does NOT independently validate an analog level; caller
must still get its matching edge from the continuously serviced EdgeFilter.

Differential test compares old edge() against prepare_edge().finish() for all
results and the entire Acquisition state, prior seed, terminal latch and a
second invocation: two clock origins (including wrap),six initial sectors,
four input phases,two levels,eight onset gaps,five confirmation ages =3840
cases per profile. Profiles8000/666,5716/476,5000/476 =>11,520cases PASS.
Tests also cover rollback after valid/invalid tentative edges, retry after
cancellation, unqualified cancellation, and mismatched phase/level/onset.
Command python scripts/test_seed_timing_policy.py --next-edge --seed400:
47 tests PASS. Added optional --seed400 to harness so actual current recovery
profile is covered rather than only older slower acquisition configurations.

Important remaining costs: full snapshot and token movement may compile into
memory-copy helpers; rollback also costs time after a cancelled candidate and
must not break the next sampling-gap deadline. No speedup, memory fit, WCET,
ADC freshness, hardware sampling or motor qualification is claimed yet. Next
opt-in actual follow-loop integration, then emitted thumbv6m inspection and
bounded timing/sampling tests before any powered use. No guard, seed ceiling,
dwell or reference scheduling changes this turn. Preserve old direct-edge path
for already-confirmed edges and explicit differential comparison.

## E671 — real loop integration passes semantics; candidate exceeds flash

Previous turn progress: speculative policy with full-state differential tests.
Objective read. Added opt-in bench-follow-prevalidate depending on prepared
handoff, expected-phase following and DMAguard. Added prepare_candidate which
uses the existing final_candidate predicate and actual retained filter onset.
The real follow_sweep creates its exclusive PendingFollow before the existing
40half-us-tick wait, samples the comparator/filter afterward, and finishes only
on a matching confirmed edge. A cancelled candidate or filter error drops the
token and rolls back. An edge already confirmed on the first visit continues
through the original direct-edge path. No dwell or sampling-gap change.

Actual extracted follow-loop harness now runs132 cases in five feature modes,
including prevalidation. All PASS. A host-only counter proves the new branch
executes in the prevalidation mode and never executes in the other modes.
This is functional simulation, not a measurement of policy/snapshot/rollback
time, analog settling or interrupt latency. No ISR can access the exclusively
borrowed acquisition; shutdown still revokes separate authority as before.

Full release-s/thinLTO/codegen1 build, exact E669 features plus
bench-follow-prevalidate, failed linking: .data overflow780bytes and final
FLASH overflow800bytes. No candidate ELF, no flash or hardware commands.
Verified root shell-pwm still SHA2568DD2B04882A5FCA8AB7CBA38B19A305A60C19043E5F2C13F03CF78D4FD1C76DB:
this is the PREVIOUS successful build, not the staged integration. Actual
board remains same8DD2, last verifiedoffE669, not freshly queried this turn.

Did not switch to opt-z or remove guards to fit. Inspected shell code for
possible separation of optional diagnostics; no such removal made. The new
transaction carries a full rollback snapshot and added control flow, whose
layout/code-size cost needs examination before motor use. Explicit wire
provenance/fixture opt-in for prevalidation is also not implemented yet and is
required before powered testing. Next reduce or separate that footprint,
inspect emitted code, then test timing and cancellation sampling continuity.
No performance or recovery-envelope gain is claimed from these tests.

## E672 — opt-in candidate fits; explicit identity; not flashed

Previous turn progress: actual-loop integration and verified flash overrun.
Objective read. Tried flat Acquisition snapshot plus rollback bool instead of
Option snapshot:47 policy tests PASS, but FLASH overflow grew800->832bytes.
Reverted that layout; no benefit claimed. No thresholds or optimization profile
changed. Omitting legacy manual sine command in prevalidation builds reduced
overflow to288bytes. Omitting manual single-channel rN and comparator c commands
in the SAME opt-in build then fits. Normal builds retain these commands.
Recovery fixture uses run, not fixed-sine mode; a voltage report, captured ADC
feedback, run/duty controls, electrical/tracking guards and off/p/i remain.
These are explicit experiment-specific CLI omissions, not a qualification claim.

Added FOLLOWPREVALIDATE v1 configuration marker and --follow-prevalidate host
selection, requiring --prepared-handoff. Host rejects missing/unselected/unknown
marker. Three identity tests PASS. All132 actual-loop cases in five modes PASS.
Candidate release-s/thinLTO/codegen1 build succeeds, emitted math audit completes.
Frozen captures/reference/prevalidate_672/shell-pwm.elf and .math-audit.S/.json:
SHA25615C99B5469D9178018AB6C589B9FB03ACC0491631893D1CA63678A6F8FE55A0D.

Emitted follow_sweep at080093fc allocates268stack bytes versus92 in8DD2 (+176).
Pending transaction methods inline; no separate finish/prepare symbols. There
are two memcpy calls in follow_sweep, as in baseline (addresses/caller layout
changed). Mere helper count does NOT establish equivalent cost or path, nor
prove snapshot work fits inside20us. Cancellation rollback and first-confirmed
direct path require timing evaluation. No flash/UART/motor operation this turn.
Actual board8DD2 remains last verifiedoffE669; root15C99 candidate now differs.
Next candidate-own disabled preflights and bounded timing/sampling evidence,
then one guarded recovery if those checks permit. No old cohort transfers to
this image, and there is no fresh off readback claimed from this offline turn.

## E673 — prevalidation policy6us; stale setup-completed edge still refuses8%

Previous turn progress: fitting identified candidate. Objective read; installed
exact frozen15C99 via checked download/reset exit0. Own disabled guard3/18,
five3200 CPU2/6/9, ADCroute3 PASS (captures/prevalidate_673_* preflights).
No new standalone prevalidation WCET/cancellation timing harness was built.
Instead the declared low-duty experiment uses unchanged100us sampling-gap,
original confirmation/deadline,32us admission and16us arm checks, plus original
electrical/tracking guards. The speculative policy has no output authority;
late sensing is refused before handoff. This is bounded integrated evidence,
not a substitute WCET certificate or proof all cancellation branches were hit.

One7%30s recovery with E672 --follow-prevalidate and all E669 campaign flags:
PASS337.517676eHz, recovered27.990505s,56683COM/56683accepted, cycle sigma
28.189146us, rawpeak320/busminimum10984mV, IRQunion57.951376%, stack2592.
Next edge onset14154/confirmed14194 =20us dwell; follow policy12ticks=6us,
DMAcounterdelta0. Age at arm104ticks=52us, remaining147ticks, arm12us.
First feedback840us age+91us decision, fault0; DMAmax77us preemptible duration.
Fixture exit0, originaldeadline/timeline/ADC and independent CRC/off PASS.

Then one8%30s attempt, same image/flags except duty80, FAILED before arm:
seed878; onset12458/confirmed12498 (20us), policy12ticks=6us/dma0; age206ticks
=103us at admission, remaining14ticks=7us<32us, armed0/arm_us0. Stage5 READS0
means setup already captured this qualified edge; the final follow loop took
no new sample. This is NOT a slow confirming dwell or a repeated23us policy
cost. The remaining setup/return/final-owner sequence aged the retained edge.
No claim which exact IRQ/instruction consumed that interval without evidence.
Firstfeedback856+91us, fault0; DMAmax26us. CRC/firstarchive/finaloff verified.
No identical retry, threshold change or recovered COM on this8% attempt.

Capture SHA256:
- prevalidate_673_70_30s.txt:5549E4EACECAE85BA690C894596A05C240DC34E2177D347597113A05F3A76DDC
- prevalidate_673_80_30s.txt:EE34B68B40DC513AD3169C0D5EE40DD659EEF15B02FD56687C1509EF1CF4613A

Actual/root15C99 installed/OFF,Uartclosed. One7% recovery pass is not a cohort.
The6us post-confirmation bracket is a real observation, but not total work or
WCET: validation moved earlier into the dwell. Next simplify the setup-completed
edge traversal so confirmation occurs at the final handoff point rather than
returning through another generic setup/follow entry. Preserve real onset and
guard checks; do not relabel confirmation time, skip the physical edge, or
raise the speed cap on this evidence. PORTABLE_WINS now records E669's separate
aggregate-copy/codegen lesson without claiming end-to-end timing qualification.

## E674 — direct final wait exposes first-sample gap; expected-last staged

Previous turn progress:6us final policy and retained setup-edge delay. Objective
read. New opt-in bench-follow-direct merges final checkpoint4 sensing into
follow_prepared, after setup, rather than confirming there and returning through
another entry. Original filters, onset, owner/ready/off/abort checks, sampling
gap, confirmation and arm limits remain. Earlier setup checkpoints remain.
FOLLOWDIRECT v1 plus --follow-direct explicit fixture selection; requires
--follow-prevalidate. Four identity tests PASS.132 actual-loop cases in six
feature modes PASS, including direct entry and stale/expired retained edges.

First candidate release-s/thinLTO/codegen1 audited/frozen followdirect_674:
5CD4D79ADFAF5C0358D4096C2E0095D27B3849B996AAD9FC8278FCC217E0117D.
Downloaded/reset exit0, own guard3/18/five3200CPU2/6/9/ADCroute3 PASS.
One8%30s recovery command (E673 flags plus --follow-direct) FAILED follow result5:
stage5 reads1, gap210half-us ticks=105us>unchanged100us. No confirmed next edge
or arm; firstfeedback seen0, DMAmax0, so no completed DMA handler evidenced at
this refusal. This is the first-sample continuity limit, not E673's old confirmed
edge aging. All CRC/firstarchive/finaloff verified, no retry/guard change.
Capture followdirect_674_80_30s.txt SHA256:
874EECDD60A681CB0E697903B722716478A73957E189070B00F653201BA8F8AD.

Staged follow-up changes ONLY direct-mode all-phase checkpoint3 order: sample
the expected next phase LAST instead of first, retaining all three reads and
each original filter's gap checks. This places its most recent observation
closer to final setup completion; it does not reset history or guarantee that
an earlier phase won't now expose a gap. Uses bounded conditional wrap, no new
modulo division. Harness asserts three reads and final mux is the next phase;
132cases x6modes PASS. No timing gain claimed from simulation.

Second release-s/thinLTO build/audit succeeds, frozen followlast_674b:
4007127556E29FCCD2CF0D2662E377A8D198080C089466E0051CB262C07F48D2.
ROOT now this staged image; NOT flashed. ACTUAL5CD4 remains installed/off after
failed first attempt, UARTclosed. Next second candidate's own disabled checks
and one bounded8% recovery to test this concrete sampling-order hypothesis.
Keep both failures and upstream gap evidence; no old qualification transferred.

## E675 — reorder moves gap upstream; stage expected-only setup consistently

Previous turn progress: first-sample gap found and bounded order intervention
staged. Objective read. Installed exact400712 followlast_674b; download/reset
exit0, own guard3/18, five3200 CPU2/6/9, ADCroute3 PASS. One8%30s recovery
with E674 flags FAILED result5 at stage3,read3,gap232half-us ticks=116us.
No final edge/arm or recovery guard/ADC installation. This falsifies the
reorder as a sufficient fix: it moved the gap upstream. No identical retries
or threshold change. CRC and finaloff independently PASS. Capture:
followlast_675_80_30s.txt SHA256
5549C23F026DF29756E22A5D63C9922E183F63569A8D42D4A09800921657EB5E.

Staged bench-follow-setup-phase (depends on direct/prevalidation) consistently
uses the expected-phase-only follow_sweep for ALL post-qualification setup
checkpoints. Before entering these checkpoints, the original full12-interval
acquisition has already qualified phase sequence and period. The final wait
already follows only that next phase; now the intervening setup does too.
It retains that phase's continuous filter, original100us gap,20us confirmation,
onset, sequence/cycle/deadline checks and full electrical guards. It deliberately
does NOT observe unrelated-phase edges/gaps during these setup checkpoints;
do not claim unchanged all-phase observability. Initial acquisition is unchanged.
The stage3 rotation is inert in this mode because each sweep has one phase.

Explicit FOLLOWSETUP expected marker and --follow-setup-phase fixture opt-in
(requires --follow-direct).132 actual-loop cases in seven modes PASS; harness
asserts one stage3 read on expected phase in this mode. Five identity tests
PASS. Release-s/thinLTO/codegen1 build and emitted math audit succeed, frozen
captures/reference/setup_phase_675/shell-pwm.elf and audit files, SHA256:
0FD7ACB0784E76EC01A3469D5BF2FF4AE1887399BBAA9DD2C3C10DA5267F7316.
NOT flashed. Actual400712 stays installed/OFF,Uartclosed after failed run;
root now0FD7 candidate. Next own disabled preflights and one8% recovery test.
Earlier-completed edges may still age before admission; this change does not
grant stale-edge authority, prove timing fit or qualify a higher speed.

## E676 — expected-phase setup passes two8% recoveries; cohort startup failure

Previous turn progress: rejected reordering and staged consistent expected-phase
setup. Objective read. Installed exact0FD7 setup_phase_675, download/reset exit0;
own guard3/18,five3200 CPU2/6/9,ADCroute3 PASS. One8%30s recovery passed; then
declared TWO further attempts on unchanged build/settings. Both were executed;
full cohort is2/3, not3/3. No exclusions or replacement retries.

| capture suffix | outcome | recovered eHz | COM / accepted | sigma us | raw peak | bus min mV |
|---|---|---:|---:|---:|---:|---:|
| setup_phase_676_80_30s.txt |PASS|388.776979|65294 /65293|26.544284|345|10889|
| setup_phase_676_80_30s_02.txt |PASS|388.897695|65315 /65315|26.446263|360|10841|
| setup_phase_676_80_30s_03.txt |startup current stop|not reached|not reached|n/a|C3271 raw absolute|not a recovered segment|

Passing recovered durations27.991459/27.991869s; original deadlines, timeline,
ADC/phase data, tracking-loss shutdown/reentry and finaloff validated. Seed865/
862ticks; edge age118/124ticks=59/62us; remaining98/92ticks=49/46us; arm12/13us.
Post-confirmation policy12ticks=6us both. Confirmation27/20us. Firstfeedback
initial age720/738us plus91us decision, nofault. IRQ union59.194026/59.255234%,
valid accounting; stack2592. These two attempts do not reproduce the E674/E675
setup gap faults; this is measured scope, not a guarantee across every timing.

Third attempt stopped at1,625,167us during initial6.2% open-loop run, BEFORE
driven qualification and BEMF/recovery. CAP reason4 and explicit current ADC
rail/peak stop. Retained CURRENTTIMING: phaseC raw3271, PWM bracket6192->2243,
elapsed38us, CCR393, ARR6399. The bracket crosses PWM wrap; that is not proof
of harmless switching artifact or calibrated current. Do not relax the current
threshold from this single event. It is also not an8% recovery failure: that
segment was never entered. Finaloff and CRC passed independently for all files.

Initial fixture error on third attempt incorrectly led with missing prepared
build marker, which is emitted only later. Added reject_pre_handoff_fault:
requires explicit startup-current message, no driven report, CAPreason4 with
valid CRC and finaloff before reporting the actual early stop. Regression test
uses this saved capture, rejects missing finaloff, leaves the passing capture
alone. No rerun needed and no firmware change from this host classification fix.

Capture SHA256:
- setup_phase_676_80_30s.txt:B7ACD49C22E2646F33E359504E975B165924141CB07EF4D0EF578CD5FFDA75C0
- setup_phase_676_80_30s_02.txt:8400A7882C652210FEA2D4F08C5989A1CC45EB0B312E03E603810994CB290602
- setup_phase_676_80_30s_03.txt:3ADCEC13BFE7E4678A2E32285FD9A249FF2CD4C1C3C7337965D174DB8C4C103C

Actual/root0FD7 installed/OFF,Uartclosed. Full goal active. Separate startup
current-refusal reliability from high-speed recovery scheduling; do not pool
this cohort with earlier hashes or repeat until3/3. Seed400,32us remaining and
16us arm budgets remain unchanged. Neither faster recovery nor higher-duty
qualification is established by these runs.

## E677 — host-only monotonic startup: qualification IRQ-rate refusal

Read objective. E676's third failure occurred during the fixture's downward
100->50 eHz ramp, at commanded84.53eHz. C3271 is23 counts beyond the original
peak cutoff, not an ADC rail. Its wrap-straddling bracket does not establish
a false sample. Tested one host-only alternative, no firmware or guard change:
--startup-direct selects run200, no intervening ehz commands, same6.2% startup,
4.7s forced-drive duration,6.1% qualification and requested8%/30s recovery.
All other E676 feature flags retained. Fresh startup_direct_677_guard.txt
disabled guard3/18 passed before the run.

Capture startup_direct_677_80_30s.txt FAILED before initial handoff:
DRIVEOBS reason21,start220us,stop1037us,commands1,scans0;
DRIVENIRQ calls65,accepts0,rate_peak65 against64,max_us10,overruns0.
driven_irq_live::interrupt stops on rate.hit refusal; driven_run maps the
false return to reason21. No BEMF or recovery was reached. No current trip
occurred in this attempt; that does not prove the startup ramp caused E676.
Retained ramp105 samples100.30->199.10eHz monotonically increasing, verified
by verify_direct_startup. CRC and finaloff independently PASS: all six gates,
ENABLE,MOE,CCRs zero; nFAULT1. Fixture process is no longer running.

The full fixture replay fails its downstream FOLLOWMODE provenance check,
which this early failure never emits. Do not describe that as the physical
fault or a firmware mismatch. No replacement retry. Three host tests for the
new startup plan/verification and prior early-current classifier PASS.
Actual/root remains0FD7ACB0784E76EC01A3469D5BF2FF4AE1887399BBAA9DD2C3C10DA5267F7316.
Capture SHA256 D64A40D6468C83CDD254795ADDEC5B348F7BE92C5BEAAC40C9E14676E463F0D2.
Next inspect initial qualification edge/pending handling and waveform alignment;
do not raise the dispatch-rate limit from this single run. E676 stays2/3.

## E678 — staged terminal-only qualification rate snapshot

Previous turn was progress: E677 retained failure changes next investigation.
Objective read. Source review: driven_irq_live calls the actual minz COMP ISR;
its closed-gate/post-ZC branch deliberately leaves pending set. Rate is a
fixed1ms bucket, not a lifetime quota. Mux change clears both EXTI flags and
enable drops stale NVIC pending. None proves what happened in E677; especially
do not equate65 dispatches with65 physical edges or revive E478 preemption.

Added opt-in bench-driven-rate-snapshot. On rate.hit failure ONLY, record
TIM2 count, fixed1666 average, EXTI18 rising/falling flags, actual adapter HAL
level, rising, command epoch, elapsed TIM17 bracket. Original stop follows;
no guard threshold, acceptance, or pending-clear policy changed. Dump RG85
after run using existing CRC wire encoding. Samples are sequential under the
existing interrupt mask, not simultaneous; terminal state cannot reconstruct
the preceding64 calls. Bracket ends before final record stores/shutdown and
is NOT a complete added-shutdown-latency measurement.

Release opt-s/thinLTO build PASS after fixing u32->u16 snapshot average cast
(observer always stores1666). Emitted math audit PASS; inspected terminal
branch uses scalar loads/stores/shifts, no arithmetic/memory helper calls.
No assertion of unchanged successful-path codegen or timing: recompilation
can change layout/register allocation. Candidate frozen at
captures/reference/rate_snapshot_678/shell-pwm.elf, SHA256
525327b453eb4ac3de7363832e73adddadb738c10d07cc4b5a25584e6246e7cb.
NOT flashed. Actual0FD7 remains last verifiedOFF E677, no UART/motor this entry.

drv_rate_snapshot.py validates CRC, singleton/header, fixed average and
boolean domains; computes strict count>833 gate and explicitly marks
historical_cause_proven=False. Boundary832/833/834, malformed/duplicate/missing
provenance and nofault snapshot tests PASS; together with prior startup tests,
five host tests PASS. Next disabled terminal-branch timing (including stores
before shutdown), candidate's own preflights, then bounded diagnostic startup.
Retire feature after answering the pending/gate question; not qualification.

## E679 — rate snapshot on hardware; stepped startup control passes recovery

Objective read; previous turn staged useful diagnostic. Added disabled timing
via existing irqbudgetcheck. Combined old/new harness overflowed288 bytes;
opt-in snapshot build now substitutes rate-stop harness for old five synthetic
duration cases. Normal build retains old harness. Sixteen forced-latched
rate failures call actual interrupt()->snapshot->stop with NVIC masked and
bridge disabled. MCP returned RATESTOP max_us=5 cases=16 failed=0.
This includes snapshot stores and comparator/TIM2 cleanup, excludes exception
entry/exit and driven_run::end physical shutdown; not full WCET. Snapshot's
own bracket is narrower still. Initial multi-command UART write lost commands;
separate p/i later verified all gates/EN/MOE/CCRs0,nFAULT1; MCP closed.

Release-s/thinLTO/audit PASS, frozen rate_snapshot_679 SHA256
566ACF5BC3CB4A9A21B985238D4EB3638FA558139CCAA681831B1CE7A27FE604.
Flash/reset exit0. Own guard3/18, fullfive3200 CPU2/6/9, ADCroute3 PASS.
No electrical/tracking threshold changes. Two different startup trajectories,
one attempt each, same image/settings: initial6.2%, driven6.1%, BEMF8%,30s
with dropout/reentry and E677 flags. First uses --startup-direct; second omits.

Direct: rate_679_direct_80_30s.txt FAILED initial qualification reason21,
65calls/0accepts/peak65 against64,max10us; start218,stop1079us,epoch1.
RG85 actual TIM2count1711,gate833 OPEN,EXTI pending,HALlevel!=rising(preZC),
bracket3us. This excludes a closed gate at the terminal sample; does not
explain the preceding64 calls or prove fresh physical edges. No BEMF entered.
CRC/finaloff verified. SHA256
1578088E948A9A989A86A6B6FF636A3BF9722559C3F65C635409CFAC59B88D2E.

Stepped: rate_679_stepped_80_30s.txt PASS startup+injected-loss recovery.
Resumed27.991685s,387.860046eHz,65141COM/65140accepted,sigma25.201701us,
rawpeak417,busmin10960mV,IRQunion58.530011%,stack2580.
Recoveryseed875ticks,age104ticks=52us,arm12us,policy6us; original limits
unchanged. Initial qualification220calls/14accepts,peak39,max23us,overruns0.
No rate snapshot (correct: no rate refusal). Full fixture timeline/feedback,
tracking-loss stop/reentry and finaloff PASS. SHA256
2DB6A0E339B97FAC97BE5F0CDF3FCA13767E9C98800F0CC5DBA5547C7578CF86.

This is one diagnostic-image pass, NOT repeatability/cohort qualification.
Direct trajectory did not improve entry; retain established stepped default.
No inference that all startup-current faults are fixed. Added saved-capture
early IRQ-rate classifier before downstream provenance checks, requiring CRC
and finaloff; six host tests PASS. Actual/current566A remains installed/OFF,
UART closed. Do not transfer old0FD7 cohort onto this image. Full goal active;
faster recovery remains open, not solved by a startup comparison.

## E680 — current-image9% sustained hold; recovery timing budget separated

Objective read, previous turn progress (startup A/B and recovery pass).
No UART fixture/SWD process live at start. Same566A candidate, freshguard3/18
PASS, no code/flash/guard changes. One stepped-startup9%30s HOLD:

python scripts/drv_driven_handoff.py --out captures/envelope_680_90_hold30s.txt
--ms 30000 --drive-duty 61 --bemf-duty 90 --phase-shift 60 --core-trace 0
--pwm-roles --carrier-hz 20000 --dma-peer --cycle450 --seed400
--seed-timing-reanchor --fast-cycle-report --event100 --dma-guard
--prepared-handoff --follow-prevalidate --follow-direct --follow-setup-phase

PASS30.000036s,444.386159eHz,79988COM/79987accepted,
cyclemean2250.295108us,sigma37.662887us,IRQunion59.996920%,stack2580.
Powered rawpeak436,busmin10937mV; initial qualification raw914,bus11462mV.
Uncalibrated raw counts, no PSU amperage inference. ADCphase149254 records,
DMAhandlermax35us. Fast-cycle report11594,min2145us; original report-only
policy retained, not misclassified as a trip. Initial13accepted/216IRQcalls,
peak44,max23us,seed1541,arm7us. No rate snapshot, no rate refusal.
CRC/fixture feedback/timeline and finaloff verified, port closed.
SHA25656F6B6983AE23F4E4D57193FA2C8971B0CED8A888AF7CDA9F2E07DBF3B211D66.

This is a single current-image sustained hold, not a recovery or repeatability
cohort. No injected dropout. Existing400eHz seed domain remains unchanged;
do not mistake that admission profile for the running controller's limit.
At advance16, reference wait=(interval>>1)-((interval*16)>>6),half-us ticks.
For nominal450eHz interval740ticks, wait92.5us; retaining64ticks/32us for
arm requires edgeage<=60.5us. At500eHz it is51.5us; at1000eHz9.5us, less
than the existing20us confirmation alone. Recent successes52..62us show why
faster recovery needs a scheduling decision, not just widening seed bounds.
Those ages are observations, not WCET or proof that450 always fails/passes.
Next review a bounded450 recovery profile and actual late-edge handling;
do not infer missing-edge free-running COM or reset the physical onset clock.
Actual566A remains installed/OFF; full goal active.

## E681 — operator review: examine bench confirmation and arm allowance

Objective read; previous turn progress (444eHz hold). Fresh disabled guard3/18
PASS. Planned9% recovery was deferred before motor command when operator
review identified the real architecture issue:20us two-sample confirmation
plus32us admission reserve cannot fit41.5us reference deadline at1kHz.
These are bench choices, not immutable electrical protections. Existing
measured full arm7..13us and enforced16us limit justify examining the reserve;
they do not by themselves certify a smaller reserve across all paths.

Implemented pure Seed::handoff_with_budget<MIN,BUDGET>. Existing
handoff_with_min forwards with64ticks, so all live call sites retain current
behavior. A candidate adapter may specify a compile-time allowance. Reject
zero/>u16 budgets and unsupported seed profiles; preserve physical onset,
checked subtraction and absolute deadline. New tests sweep802 onset/age
cases across wrap, compare old64-tick behavior, test40-tick exact threshold,
and invalid budgets/profile. Full standalone policy harness48testsPASS.
No release rebuild/audit yet; source differs from installed566A.

Next architecture work: implement consecutive hardware comparator reads in
final recovery confirmation, retaining mux settle, expected phase/order,
original acquisition deadline and physical onset. Reference persistence is
12 reads in the existing static-COMP path (~3us prior disabled measurement),
not20us two-sample waiting. Do not call a shortened two-sample dwell equivalent
to those reads. Measure complete confirmation+arm path, then choose explicit
reserve from that evidence and qualify higher seed domain. The present
20us EdgeFilter and32us live allowance are still deployed pending that work.
No higher-speed recovery claimed. Actual566A lastOFFE680 plus freshguardE681,
UARTclosed, no flash/motor this entry. Full goal remains active.

## E682 — consecutive-read confirmation primitive and candidate tests

Objective read; previous turn progress (explicit arm budget policy). Added
PersistentLevel with private level field; persistent_level performs exactly
12 matching comparator reads and returns at the first mismatch. Hardware
caller owns actual mux and volatile read cadence. Same count as current
reference filter, without claiming identical instruction timing or analog
qualification. EdgeFilter::confirm_persistent consumes this witness, confirms
only an existing candidate, retains original sampled onset, and enforces
original200-half-us sample-gap limit. Initial static level cannot become an
edge; mismatched candidate cancels. This is separate from sampled dwell mode.

Tests cover every mismatch position0..11 and verify exact read counts/no
reads after rejection; both level polarities pass exactly12. Candidate tests
cover absent static candidate, true reversal, original onset, wrap and gap
expiry. Full standalone policy suite50testsPASS. Existing sampled filter,
live recovery loop, PendingFollow40tick condition and deployed566A unchanged.
No UART/flash/motor. No emitted M0 timing claim: unused primitives may vanish
from live ELF. Next connect witness to transaction retention and follow_sweep,
test cancellation/ownership paths, inspect emitted volatile loop and measure
full confirmation+arm. Neither simply bypassing the transaction dwell nor
shortening two-sample wait qualifies the new path. Full goal active.

## E683 — opt-in consecutive persistence connected to actual recovery loop

Objective read. Added bench-follow-persistence dependent on expected-phase
setup. follow_sweep prepares original transaction, performs12 consecutive
volatile COMP2 reads, confirms candidate using the private read witness and
calls PendingFollow::finish_persistent. Shared finish_checked retains exact
phase/level/onset, original40_000tick acquisition expiry and240tick freshness
ceiling. Sampled finish retains40tick minimum. Successful read witness allows
zero minimum elapsed dwell; hardware elapsed time is still recorded honestly.
Rejected pass drops transaction, restoring full acquisition state.

Review found old sample() could autoaccept a mature candidate without the
read pass. Added sample_candidate() using same gap/cancellation logic but no
automatic dwell confirmation; new mode calls it exclusively. Every accepted
candidate therefore requires read evidence. Normal sampled path unchanged.
The witness certifies matching read levels; hardware caller still owns mux
selection, phase association and cadence, not encoded in the witness itself.

Actual-loop harness132cases in each of8 modes PASS, including new mode.
Old20us minimum assertion now conditional on sampled mode; remaining freshness
ceiling retained. Existing50policytests passed before final candidate-only
change. Intermediate release-s/thinLTO build PASS, but latest source edit
requires fresh release build and emitted audit: current rootELF is stale.
No flash/UART/motor. Actual566A remains last verifiedOFF. Next explicit build
marker/fixture opt-in, fresh release/volatile-loop audit and disabled hardware
timing. Arm64tick reserve and seed400 profile still unchanged. No hardware
recovery qualification or timing improvement claimed. Full goal active.

## E684 — explicit persistence identity and fresh emitted M0 loop

Objective read; previous turn progress (connected opt-in recovery path).
Added FOLLOWPERSIST reads=12 sampled_dwell=0 marker and --follow-persistence
fixture selection requiring setup-phase. Exact marker checked before recovery
evidence; new mode permits0..240tick confirmation age, old mode40..240.
The marker is configuration evidence, not proof of the actual read pass.

Fresh release-s/thinLTO build PASS, math audit generated. Frozen
captures/reference/persistence_684/shell-pwm.elf SHA256
2B5DD7839A9DDCF7A90769464047334D8E2DFC4ED1190284B6A00E3AFEDD3A33.
NOT flashed. Inspected actual follow_sweep emitted0x080095ae..0x080095c8:
counter13 decremented before each read,12volatile COMP2loads total, immediate
exit on mismatch. Successful iteration13instructions incl stackloads and
boolean normalization; no calls/division/wide arithmetic in this loop.
This is code evidence, not elapsed-time/WCET measurement. Whole follow_sweep
still contains acquisition snapshot/rollback work outside the read loop.

50 policy tests PASS after candidate-only fix; actualfollow132casesx8PASS.
Normal sampled path preserved. No UART/flash/motor, actual566A lastOFF.
Next disabled timing and candidate's own preflights, then powered8% comparison
with original32us reserve/400 seed domain. Initial full acquisition still
uses20us sampled dwell; this change targets the final next-edge path only.
Faster recovery domain and measured reserve selection remain unfinished.

## E685 — candidate hardware recovery passes; earlier sampled path still active

Objective read; previous turn progress (fresh audit/identity). Exact2B5D
persistence_684 flashed, download/reset exit0. Own guard3/18,fullfive3200,
CPU2/6/9 andADCroute3 PASS. No standalone read-loop elapsed certificate;
existing original32us admission and16us full-arm bound retained for comparison.
One8%30s stepped-startup campaign, E684 configuration plus --follow-persistence,
dropout/reentry and all E679 recovery flags.

Capture persist_685_80_recovery30s.txt PASS recovered27.992072s,
388.537701eHz,65255COM/65255accepted,sigma24.202266us,
poweredraw401,busmin10984mV,IRQunion58.259394%,stack2592.
Startupraw852,bus11307,IRQmax25us/peak35; no calibratedamps claim.
Seed860ticks,age122ticks=61us,arm12us,policy10ticks=5us.
FOLLOWEDGE onset11636,confirmed11676=20us. An earlier continue_filters path
can complete a sampled edge before final follow_sweep, so this run DOES NOT
prove the new12read loop supplied this accepted edge or saved confirmation
time. No direct read-loop branch counter was installed. Firmware marker
certifies configuration only. DMA reportedmax77us may include preemption;
do not equate that with isolated handler body cost.

Fixture original deadline,coherent feedback,tracking-loss shutdown/reentry,
CRC and finaloff PASS. Port closed. SHA256
5D3A5B34C5A51C3024F211807E89C9A2FCE6E2EECD8C1D3650C42919D77E0CD9.
One candidate pass, not3/3. Actual/root2B5D remains installed/OFF.
Next extend persistent witness through earlier continuation and promotion;
every path completing the follow edge must carry its actual confirmation
policy. Do not report this pass as measured persistence speedup or extend
the seed domain from it. Full goal active.

## E686 — continuation carries persistent evidence into promotion

Objective read; previous turn progress (hardware pass revealed earlier path).
continue_filters new mode uses sample_candidate then12volatile COMP2reads,
confirm_persistent preserving onset. Return tuple now includes private witness;
promotion feeds it through prepare_edge.finish_persistent. Missing witness
explicitly refuses in new mode; legacy sampled edge path remains in old mode.
Original sampling-gap,owner/output-off checks and deadline retained.

Actual-helper follow harness132casesx8PASS incl continued earlier candidate,
wrap, gap/owner refusal and witness-fed policy retention. It does not extract
the full acquire_inner promotion body; that body receives compile checking,
not a claim of simulated whole-hardware ownership coverage.
First release link overflowed64bytes. Removed legacy sampled-promotion fallback
from new-mode code using cfg, retaining explicit missing-witness refusal.
Release-s/thinLTO build then PASS; arithmetic audit generated. Frozen
captures/reference/persist_cont_686/shell-pwm.elf SHA256
36FA2EFF417C59210EE30C877E6644BAFC061C0C4FA242F0247F7F95B8225DEA.
NOT flashed. Actual2B5D lastOFFE685, no UART/motor this entry.
Next own preflights and8% powered comparison for actual confirmation age;
no measured speedup/new-domain qualification yet. Full goal active.

## E687 — continuation candidate hardware pass, complete latency still64us

Objective read; previous turn progress (continuation witness staged).
Exact36FA persist_cont_686 download/reset exit0, own guard3/18/fullfive3200,
CPU2/6/9/ADCroute3 PASS. One stepped8%30s recovery with E685 flags.
Capture pcont_687_80_recovery30s.txt PASS27.991904sresumed,
388.217325eHz,65201COM/65201accepted,sigma25.720220us,
IRQunion58.460330%,poweredraw351,bus10960mV,stack2592.
FOLLOWonset11698confirmed11752=27us; seed867,armage128ticks=64us,
arm12us,policy24ticks=12us,DMAcounterdelta0. Completepath did not demonstrate
the intended latency reduction. Original limits and physical onset retained.
Initialraw528,bus11486,IRQmax25; no calibratedamps. DMAmax77 includes possible
preemption. CRC,feedback,timeline,tracking-loss/reentry and finaloffPASS.
SHA25634681E0D3A070985B92B387FBF86624DB9A5225C50D45D7DA997815AAD51180E.
Actual/root36FA installed/OFF,Uartclosed. One pass, not repeatability cohort.

Next investigate/remove speculative prepare_candidate acquisition clone in
persistence mode. It was designed to spend the old20us dwell on work; the
new short read-pass path still pays that work before confirmation. Once a
private read witness succeeds, direct checked acquisition commit can avoid
the speculative snapshot/rollback, preserving all sequence/cycle/deadline
checks. This is a candidate optimization, not yet implemented or timed here.
No9% recovery qualification, numerical guard changes or hardware blame.

## E688 — persistent mode validates once after successful reads

Objective read; previous turn progress (complete latency observation).
Added NextEdge::edge_persistent requiring matching private read witness,
then shared edge_checked<0> validates original deadline,confirmation ceiling,
sequence/cycle/history and latches consumed/refusal. Sampled edge retains
edge_checked<40>. Rejected read pass never calls acquisition.edge, so the
speculative snapshot and rollback are unnecessary in persistent mode.
follow_sweep no longer calls prepare_candidate before persistence in that
mode; valid witness feeds direct checks. Continuation promotion uses same
direct path. Old sampled prevalidation preserved.

Actual-loop132casesx8PASS, verifies zero speculative prevalidation in new
mode.51policytestsPASS, including differential valid confirmations at
0/6/40/240ticks: direct result and full retained acquisition state equal
transactional path. This valid-state test does not claim identical internal
failure mutation: direct acquisition failures latch without rolling back,
which grants no authority. Existing deadline/order failure tests retained.
Release-s/thinLTO build PASS; audit generated, frozen
captures/reference/persist_direct_688/shell-pwm.elf SHA256
C7E45AB369060DC7A298F2259CDB1660C99F702C1FA3E20F451433064C9868D0.
NOT flashed. Actual36FA lastOFFE687 unchanged, no UART/motor here.
Next own preflights and powered8%comparison to measure complete edge-to-arm
age.32us reserve/seed400 unchanged; faster qualification still unfinished.

## E689 — direct persistent path hardware pass, measured46us edge-to-arm

Objective read; previous turn progress (copy removal/tests). ExactC7E4
persist_direct_688 flash/reset exit0. Own guard3/18/fullfive3200/CPU2/6/9/
ADCroute3 PASS. One stepped8%30s recovery with E687 flags. Capture
pdirect_689_80_recovery30s.txt PASS27.991857s recovered,388.166289eHz,
65193COM/65192accepted,sigma25.942295us,IRQunion58.397971%,
poweredraw360,bus10972mV,stack2592. Initialraw640,bus11354mV.
FOLLOWonset11498confirmed11516=9us. Seed862ticks,age92ticks=46us,
arm12us,policy22ticks=11us,DMAcounterdelta0. Original32us reserve and16us
full arm enforcement unchanged. Comparison E687age64us vs current46us is
an18us observed improvement; different runs/edges, not paired causal proof
or WCET bound. Currentconfirmation9us is below old20us dwell and measured
on actual hardware. It includes surrounding observation work, not isolated
12read loop body. DMAmax77 may include preemption.
CRC,feedback,timeline,tracking-loss/reentry and finaloff PASS;UARTclosed.
SHA2565EE949214196541F4B096440DE9B6FEE1A80F9F48C5B157D66DE94E6BAFA2EDE.
Actual/rootC7E4 installed/OFF. One pass not3/3. Next bounded450eHz seed
domain review/qualification: nominal450 allows60.5us age under unchanged
32us reserve, versus current46us observation. Do not generalize that one
observation to all phases/timing or grant stale-edge authority. Full goal active.

## E690 — bounded450eHz recovery seed domain staged

Objective read, previous turn progress (46us measured age). Existing same-rig
9%hold444eHz plus current8%recovery46us age support next staged recovery
domain. New bench-seed450 depends on persistent mode and runningcycle450;
minimummean741half-us ticks and cycle4445ticks (ceil450eHz), individual476
unchanged. Twelveintervals/fullcyclehistory,20ms original acquisition expiry,
32us admission reserve and16us full-arm bound retained. This changes the
seed speed envelope intentionally; no electrical/tracking threshold changes.
Active profile explicit SEEDPROFILE741/4445/476 and --seed450, mutually
exclusive with --seed400 and requires --follow-persistence. Old400 marker
omitted in450 builds, oldprofile decoder retained for oldcaptures.

52policytestsPASS, new profile test verifies741accepted after12 intervals,
740mean refusal at handoff,exact remaining64ticks permitsage121 and refuses
122ticks. Allprior policytests retained. Release-s/thinLTO build PASS;
audit generated. Frozen captures/reference/seed450_690/shell-pwm.elf SHA256
6A202774EDCB0ED6E624C7D948C04A74EC75E4CF6841CE6D48ED8034C89DD232.
NOT flashed. ActualC7E4 lastOFFE689, no UART/motor this entry. Next own
preflights and9%recovery attempt, preserving actualonset age and original
deadline. No450eHz recovery reliability or timing guarantee claimed.

## E691 — first9% recovery refuses seed cycle envelope before arm

Objective read; previous turn progress (450 profile staged). Exact6A20
seed450_690 flash/reset exit0. Own guard3/18/fullfive3200/CPU2/6/9/
ADCroute3 PASS. One stepped9%30s dropout/reentry with E689flags, replacing
--seed400 with --seed450. Retained failure, no replacement retry.
RECOVERYACQ14(CycleTooFast),7intervals/3checkedcycles,min4436max4586ticks;
4436<4445 by9half-us ticks=4.5us. Elapsed3293us,maxgap158ticks.
Final follow/arm not reached: downstream result0/stage0/read0 is expected.
InitialBEMF2001135us until injected loss,5298COM/5297accepted,
raw451,bus11020mV. No calibratedamps/currentfault/hardwareceiling claim.
CAP/ADC CRC,firstarchive and finaloff independently PASS;gates/EN/MOE/CCRs0,
nFAULT1,Uartclosed. SHA256
0AED4C0DFE8A2AD74F7AD1904E80443D1FEBB0C60D31C50A89DD43E8C1A3E45B.
Actual/root6A20 installed/OFF. Next review acquisitioncycle envelope versus
retained variation before choosing wider domain. A nominal444eHz operating
point can refuse a450domain when one measured cycle is shorter thanfloor.
Not evidence arm-floor optimization failed. No guards changed this entry;
9%recoveryunqualified,fullgoalactive.

## E692 — explicit500 seed envelope staged after retained cycle review

Objective read; previous turn progress (9% acquisition refusal). Decoded
E691 RF85 CRC row(4,5710,6556,0,9,0,6556,9,0,6440,8,0,6476).
Observedcycle4436ticks is450.856628eHz, correctly outside450domain; not
proof of false sample. Threecheckedcycle range4436..4586 and E680steady
444eHz/sigma37.663us show why a450 instantaneous-cycle ceiling is too close
to the9% operating point. Staged separate500domain, not edited450profile:
bench-seed500 and explicit --seed500 select667mean/4000cycle/476individual.
Original64tickreserve/16usarm/12intervalhistory/deadline and electrical guards
retained. Requires persistentmode,excludes other seed fixture selections.

52policytestsPASS incl twelveintervalready and exact handoffage103allow/
104refuse for667ticks,mean666refusal. Prior tests retained. Release-s/thinLTO
build PASS; audit generated. Frozen captures/reference/seed500_692/shell-pwm.elf
SHA2562F1B55C0924C11A247E886856626F6218CA13D3E0268EA55756ABE4A4AF39016.
NOT flashed; actual6A20 lastOFFE691,noUART/motor here. Next candidatepreflights
and9% recovery. Nominal500latestarmage51.5us vs prior46us observation is
small margin, not WCET or reliable500recovery proof. Numerical seed envelope
expansion is deliberate; no current/bus/tracking kill waived. Full goal active.

## E693 —9% recovery passes above400eHz with500 seed envelope

Objective read; previous turn progress (domain staged). Exact2F1Bseed500_692
flash/reset exit0. Own guard3/18/fullfive3200 CPU2/6/9/ADCroute3 PASS.
One stepped9%30s campaign, E691flags replacing --seed450 with --seed500.
Firmware completed recovery. Initialhostvalidation failed because shared
sustained_report still called oldseed400-only decoder; update generic strict
profile decoder and both report consumers, then replay SAMEsavedcapturePASS.
No rerun to fix parser. Saved400/450/500 selection tests and duplicateprofile
rejectionPASS. Explicit--seed selection remains mandatory.

seed500_693_90_recovery30s.txt PASS27.992316s recovered,444.632645eHz,
74677COM/74677accepted,sigma25.074940us,IRQunion59.559696%,
poweredraw425,bus10877mV,stack2592. Recovery12intervals/7cyclechecks,
mean761ticks,mincycle4436max4596,elapsed4838us; original physicalonset
and deadlines retained. FOLLOWonset10318confirmed10336=9us,
armage92ticks=46us,arm12us. Original32usreserve and16usarm bound unchanged.
Raw counts uncalibrated; no PSUamp claim. Fastcycles12364,min2142us,
originalreportonlypolicy retained. CRC,coherentfeedback,timeline,
tracking-loss shutdown/reentry and finaloff independentlyPASS,UARTclosed.
SHA2564DF32E10A7C64BE3CC490B51C2C3F1E9F2165087DD8A1E2DCFD48EEE5CBB501D.
Actual/root2F1B installed/OFF. First currentbuild9%recoverypass,NOT3/3.
Next predeclared repeatability cohort before fasterrecovery. Fullgoalactive.

## E694 — current-build9% recovery cohort3/3

Objective read. Predeclared two additional attempts on unchanged installed
2F1B seed500_692, identical E693 fixture flags and30s campaign duration.
Guard check and full strict fixture validation PASS. No replacement retries,
firmware edits, flashes, or protection changes.

repeat_694_90_recovery30s_02.txt PASS27.992557s recovered,444.299167eHz,
74622COM/74621accepted,sigma24.896370us,IRQunion59.561145%,raw432,
bus10423mV. Recovery seed761,12intervals,4838us; confirmation9us,
edge-to-arm46us,arm12us. SHA256
87E04A389371344B59DE7FBD3E121D68699103F9F580AE1F6F034F077DC6319F.

repeat_694_90_recovery30s_03.txt PASS27.992658s recovered,444.447618eHz,
74647COM/74646accepted,sigma25.338706us,raw416,bus10901mV.
Recovery seed749,12intervals,4836us. Stack untouched2592 both attempts.
Third IRQunion59.522736%,confirmation9us,edge-to-arm46us,arm12us.
Third SHA256BE8E29B006F3EDE255231D3B0C5B4ECE21A12D12918E29044BA4E42218B2DFB7.
CRC, coherent feedback, full timeline, tracking-loss shutdown/reentry,
and finaloff verified; all gates/EN/MOE/CCRs0,nFAULT1,UARTclosed.

WithE693 this is current-image9% recovery3/3,444.299..444.633eHz.
Raw current remains uncalibrated. No higher-speed recovery qualification
transferred. Original32usreserve and16usarm bound remain; higher-speed
recovery needs a measured complete confirmation/arm budget rather than
further rearrangement. Actual/root2F1B installed/OFF. Fullgoalactive.

## E695 — separate measured arm limit from chosen admission reserve

Objective read. Previous turn progress: current-build9%recovery3/3.
Inspected actual core_bench final handoff, com_timer prepared publication,
and flying_bench continuous follow. No UART, motor commands, or flash.

FLY_AGE is sampled at arm_start, after follow_seed, controller step stores,
owner checks, observe_irq_start_inner mux/priority writes and pending clear.
FLY_ARM_US starts only there. Thus46us age is NOT the whole handoff latency:
successful12us arm completes about58us after sampled onset. The admission
reserve32us provides20us beyond the observed12us, while a separate16us
arm-limit kill is enforced before final TIM16 unmask. Publication also checks
absolute deadline, timer state, pending flags and minimum remaining timer ticks.
Those checks must survive any admission-budget reduction.

Host test seed500_twenty_us_reserve_keeps_absolute_deadline exercises the
existing const-budget helper with40half-us ticks=16us enforcedarm+4usslack.
For167tick wait, age127 allows exactly40remaining; age128 refuses. Every
age0..200 tested at ordinary and u32-wrapping origins; absolute deadline
unchanged, too-slow mean666 still refused. 53 policytests PASS via
python scripts/test_seed_timing_policy.py --next-edge --seed500.
This is policy arithmetic, NOT full-arm WCET or hardware deadline evidence.
No live constructor, firmware marker or fixture selection changed.

At observed46us admission age,20us reserve would require a nominal66us
edge-to-COM window instead of78us, moving the arithmetic ceiling from about
534 to631eHz (advance16 quarter-interval approximation). It still cannot fit
the41.5us window at1kHz: the arm starts after that deadline already.
Measured9us confirmation plus FOLLOW_POLICY_TICKS22=11us accounts for about
20us before remaining return/setup work; counters are brackets, not instruction
attribution or blackout bounds. Next shorten actual post-confirmation policy
and pre-arm setup or fuse accepted-edge publication, while validating the
complete edge-to-unmask bracket. Do not mistake reserve relaxation alone for
reference-class recovery. Actual2F1B lastverifiedOFFE694; source adds test only,
root ELF unchanged. Fullgoalactive.

## E696 — return to live operating-envelope exploration

Objective read. User review identifies flying-recovery qualification queue
as distinct from running envelope. No goal scope removed; recovery remains
unfinished, but it no longer gates higher-duty exploration.
No firmware, guards, flash, PSU or wiring changed. Installed2F1B.
Added explicit --seed500 and conflict rejection to live_armed_baseline.py;
three host running-profile tests PASS. No other seed profile silently accepted.

python scripts/live_armed_baseline.py --out captures/explore_696_ramp200_30s.txt
--ramp-duty200 --ms30000 --cycle450 --seed500 --fast-cycle-report --event100
--dma-guard (arguments actually separated by spaces in invocation).
Finite30s running window, existing ACK-paced2percentage-point steps every0.5s
after initial7% entry, no dropout or flying restart. ACK200 confirmed.
Completed30000034us,153358COM/153357accepted,order_bad0,
rawpeak754,bus10853mV,IRQunion71.271675%,stack2592untouched.
Final32event tail gives26 overlapping same-phasecyclewindows:
940.631670eHz,sigma18.587272us. Whole-ramp852.029eHz/sigma416.520us
is nonstationary and not final-setting quality. No calibrated current claim.

The running Python process had loaded the first host edit, which passed
required=True together with seed500=True; explicit verifier rejected after
firmware completion. Fixed call uses required=seed400,seed500=seed500.
Saved capture replay: strict seed500 identity, CRC, acquisition/transfer,
sustained timeline, ACK200 and all finaloff gates/EN/MOE/CCRs0,nFAULT1 PASS.
No motor rerun for parser error. SHA256
7254F4D03D953BA1B34A618C992CAD4C4DBAD83503E3DCB9963E2422A4157FF1.
Actual2F1B/OFF,UARTclosed. One currentbuild20% explorationPASS, not3/3 or
higher-duty recovery qualification. Next meaningfulhigher-duty exploration;
retain electrical/tracking kills and separate any diagnostic refusals.

## E697 —25% target ramp stops after23% acknowledgment

New active objective explicitly separates exploration from qualification,
normal-startup recovery from optional flyingreacquisition, and diagnostic
quality thresholds from demonstrated safety limits. Prior turn was progress:
real20% explorationPASS. No firmware/flash/guard change in this attempt.
Ran live_armed_baseline.py with --ramp-duty250 --ms30000 and E696profileflags.
ACKs90,110,130,150,170,190,210,230; no250ACK. Retained failedattempt,
no replacementretry. Stopped7019612us with poweredreason8 Tracking,
22856COM/22855accepted,order_bad0,raw732,bus10972mV.
This is a23%-reached ramp refusal, not a25% speed ceiling or current trip.
CRC/coherentfeedback/timeline/finaloffPASS,allgates/EN/MOE/CCRs0,nFAULT1.
SHA2562A6F7A9B75969ED721189C3FE2F439EEF0D4226723EEB40950E61BFACEB44FB0.

Actual Tracking8 conflates firststep/order/stale/TooFast in RunGuard::accepted.
Final retainedaccepted step5 at7019507us, guardstop7019586us,79us later;
COASTSTATEstep6,pending1,intervalcounter38,COMdier1. Tail remains ordered;
latest six-event gaps113,144,162,191us beforestop. A new event79us later
would violate the100us minimum even without losing progress/order. Strong
minimum-spacing candidate, not exact subcause proof: accepted recorder and
guard timestamp differ, refused event is not in retained acceptedtail.
No missing-edge conclusion follows from generic Tracking8.

## E698 — exploratory event floor becomes report, candidate staged

Read newgoal attachment2006cabb: five-percentage-point exploration steps,
average-current protection, bus/nFAULT/tracking/watchdog kills; other
diagnostic floors/counts/margins report absent concrete safety justification.
Flying recovery optional; normal startup recovery and lean qualification
images still required. No scope reduction or goal completion claimed.

Monitor::event_policy<const REPORT_FAST> preserves stale-before-progress
validation and strict sector order. In report mode only short-event spacing
becomes a count/minimum; valid next events retain actual timestamps. Default
Monitor::event and strict RunGuard still kill TooFast. Existing exploratory
REPORT_FAST selects both event and cycle reports. FASTEVENT r1 postrun
includes count/min, report_only1, stale_max_us1000 and order_kill1.
41guardpolicytestsPASS including zero/short/exactintervals, defaultstrict
refusal, stale latch, duplicate refusal, u32wrap and unchangedelectrical stops.
Old testexpectation MIN-1=>Tracking failed as expected under changedpolicy;
replaced with explicit report/defaultstrict contrast, not removedcoverage.
ThreehostprofiletestsPASS. Live ramp now starts10% then5% increments to
requestedcap, ACK-paced with originalfinitewindow. No retries run here.

Release-s/thinLTO candidatebuild/auditPASS. Frozen
captures/reference/eventreport_698/shell-pwm.elf SHA256
F4870BA620360B7B9156BAC061B131F21E77BD2A1CFA610FDB2EFA22870A9CEC.
NOTflashed. Actual2F1B lastOFFE697; noUART/motor commands thisentry.
Newgoal average-current channel calibration/control remains missing;
raw phase clamp unchanged, not a device-rated calibrated protection.
Current candidate remains diagnostic, not zero-in-ISR qualification image.
Next own preflights, evaluate remaining protection architecture and exploration.

## E699 — bounded signed-average current policy, not yet integrated

Updatedgoal attachment2006cabb read. Previous turn progress: report-mode
event spacing candidate staged. Current measurement topology source inspected:
three7mOhm low-side bridge shunts, not inline phase sensors; signed means
estimate return current only with unbiased sampling. Fixed2048zero has
documented46..150mA nominal offset errors and is not adequate calibration.

Added examples/support/average_current.rs, standalone pure policy. Caller
provides measured same-ENABLE zero_block over32 scans and calibrated
positive limit_block. No amp conversion or nominal gain assumed. Accumulates
signed residual of three sequential-channel sums as a block average, no
absolute-value bias, no runtime division or64bit operations. Raw sum bounded
by393120, fitsi32. Invalid ADCrails latch failure. Current excursions latch
when block residual exceeds threshold; equality allowed. At201us scans,
6.432ms window, worst boundary-straddling detection up12.864ms; not an
instantaneous device-SOA clamp or proof of unbiased PWM sampling.

rustc --edition=2021 --test examples/support/average_current.rs -o
target/average_current_test.exe then execute:4testsPASS, exactboundary,
latch, invalidconfiguration/rails, signedrecirculation and1000blockreset.
No live integration, MCUtiming measurement or calibratedcurrent claim.
NoUART/SWD/motor commands. F487 candidate unchanged/notflashed; actual
2F1B lastverifiedOFFE697. Next integrate samewakezero/threshold authority
and ADCcoverage, without turning missing calibration into unrelated blocker.

## E700 — staged event-report image installed and disabled-qualified

Updatedgoal attachment2006cabb read. Inspected actual startup and current
calibration adapters. prestart_baseline::acquire already runs128scans after
normalstartupENABLE wake and before pwm_sine firstphase drive; supports
sameDMAconfiguration under bench-prestart-dma. Token tracksENABLEidentity;
newwake/faultinvalidates, baseline itself grants no motor authority or
stationarity proof. driven_run baseline_at is after openloopdrive and cannot
be mistaken for zero. powered_timer::zero_check creates its own wake then
disablesENABLE, so it cannot calibrate the subsequent runningepoch.

Flashed exactfrozen eventreport_698 F487 ELF, probe-rsexit0; OpenOCDreset
exit0. Own disabledguard3timerfaults/18poststoprefusalsPASS. Fullfive3200
filter preflightPASS, privileged256checks, carrierinvariance, CPUobserver
pairmax2/6/9us and archive routing3x3PASS. ADCphase3checksPASS.
All runs outputsdisabled, finaloff verified, no motor commands or UART/SWD
overlap. UARTclosed. Artifacts captures/eventreport_700_guard.txt,
eventreport_700_*preflight files and eventreport_700_adcphase.txt.
These are disabledfunctionalchecks, not actualCOMP/COMWCET or powered
eventreportqualification. Average-current limit stillnotintegrated/calibrated,
and this diagnosticimage doesnot satisfy zero-in-ISR qualification requirement.
ActualF487 installed/OFF; next integrate samewake calibratedaverage protection
and normalstartup recovery, not resumeflyingreacquisitionqueue.

## E701 — phase-complete average window and fresh-zero accessor

Updatedgoal2006cabb read. Corrected average_current SCANS32 to50:
201us scans modulo50us carrier advance1us;50scans visit eachcarrierphase
once at20kHz. Thirty-two samples cover only part and cannot establish the
unbiased PWM-average premise. Newblock10.050ms, boundarystraddling bound
two blocks20.100ms. This cadence arithmetic is not measured launch-jitter
or commutation-sector coverage; valid coherent scan delivery still required.

zero_from_128 scales three128scan sums by50 then>>7. Maximumproduct
78,624,000 fitsu32, returnedzero614250 max fitsi32. Roundzero DOWN;
positive residual error lessone block-count, never relaxes currentlimit.
SixhosttestsPASS including exactboundaries, invalidrails, signedrecirculation,
1000windowresets, bounded/conservativescaling and all50carrierphases.

prestart_baseline::average_zero now exposes offsetevidence only when
STATUS2, liveENABLEtokenmatches, allthreechannels128samples, nonrailed.
No baseline from partialcapture or earlierwake. It doesnot grant motor
authority, prove stationarity, or provide currentgain. Getternotyetconsumed.
Releasecargo check withbench-prestart-dma,bench-live-control and
bench-single-core-atomicsPASS. Initialminimalcheckwithoutsinglecorebackend
failedCASrequirements, correctedfeatureselection; no stalebuild executed.
No newlinkedELF/flash/UART/motor. ActualF487 lastverifiedOFFE700.
Next connect freshzero and calibratedthreshold to coherentADCguard;
average-current shutdown and calibratedgain stillunfinished.

## E702 — opt-in coherent-DMA average shutdown connected

Updatedgoal2006cabb read. Added bench-average-current feature requiring
prestartDMA,201usADC,20kPWM and coherentDMAguard. average_current_live
configures a positive raw50scan threshold onlywhileENABLElow/gatesoff and
no active owners. avgrawN shell provision reports unitsraw_sum and
calibration_required1, not amperes or verifiedgain. No defaultthreshold.

Normalrun now installs from completefreshprestart_baseline::average_zero
before first pwm_sine phase drive. Missingconfiguration or zero refuses
startup with gates/ENABLEoff. Coherent stream_feedback invokes50scan average
and trips existingFault::Current on limitcrossing, missinginstalledstate,
or invalidrails. Existingphasepeak/current/bus/tracking/watchdog stops remain
for now. ENABLElow revokes the active accumulator within the same serialized
write; thresholdconfiguration retained, zero must be freshly installed.
This connects BEMFaveragecurrent shutdown, not startupaveragecoverage.
No nominalgain or currentdevice rating was fabricated to provisionthreshold.

Actualadapter hostharness scripts/average_current_live_test.rs compiles real
average_current_live with mocked GPIO/baseline/IRQserialization.7testsPASS:
missingconfig/zero, active/ENABLEhighconfiguration refusal, exactblocklimit,
exceed/latch, revocation, freshchangedzero and rails. Doesnotproveactual
GPIOIRQraces, gaincalibration or hardware safinglatency. Initialminimalcompile
had unresolveddriven_run infeaturecombination, addedmatchingcfg, releasecheck
PASS. Release-s/thinLTO linkedbuildPASS usingE696controlfeatures plusaverage,
without optional flyingreentryfeatures. Audit and frozenELF:
captures/reference/averageguard_702/shell-pwm.elf SHA256
FFFC59AE058DBA8E6E084D37F1544371F97DCD6BE6C165D1AEBD44C7DFFA7733.
NOTflashed. ActualF487 lastverifiedOFFE700, noUART/motor thisentry.

Still missing calibratedthreshold, validatedstartupaverage, device-ratedpulse
clamp, normalstartup lossrecovery and zero-in-ISR qualification image. The
oldrawpeak kill is not mislabeled averageprotection or silentlywaived.
Next no-config startuprefusal and disabled averageguard checks, then calibrated
provision/measurement and renewed5%exploration; flyingrecoverynotgate.

## E703 — real missing-threshold startup refusal verified

Updatedgoal2006cabb read. ExactfrozenFFFC averageguard_702 flash/resetexit0,
no otherUART/SWDprocess present. Added drv_average_refusal.py: off/p/i,
run200 with unconfiguredthreshold, alwaysfinallyoff/p/i. Realfirmware emits
one missing-average-configuration refusal beforephase drive; noRUNalignmarker,
allgates/EN/MOE/CCRs0,nFAULT1 finaloffPASS. Freshwake baseline acquisition
occurs, but no PWMphase drive or calibrated-current experiment. Savedcapture
verifiernegativechecksPASS for absent/duplicate refusal, successfulRUNmarker,
nonzeroENABLE and missingfinaloff. Capture averageguard_703_refusal.txt.
SHA256A3849239D3F79035A7C599FBD10D8068ABE76067ED1A558B97853C677036B4A5.

Ownguard3timerfaults/18poststoprefusalsPASS. Timerfilter/rawpulse/privileged256,
carrierinvariance and CPUobserverpairs2/6/9usPASS. Fullfilter_preflightexit1
at archivecheck: omittedoptionalarchivecommand replies ?cmd threetimes.
Retained averageguard_703_archive.txt; no fullfivePASSclaim or motorfault.
ADCphase3checksPASS, allfinaloff verified, UARTclosed.

ArchivedindependentPSUanchorE546 includes complete samewake softwarebaseline
and149253poweredDMA samples. Existing strict drv_baseline_residual report
gives residualA1.309667/B0.394448/C1.058259,total2.762374rawcounts for
operatorabout70mA. Nominalgain conversion is not verified by this result;
softwarebaselinevsDMA timing, zero drift, reference changes and PSUelectronics
need accounting. Do not manufacture measuredgain or blamehardware from one
smallresidual. No thresholdprovisioned, no ordinarymotorspin thisentry.
ActualFFFC/OFF; calibratedgain/average startupcoverage/normalstartup recovery/
leanqualification stillpending. Fullgoalactive.

## E704 — separate lean physical comparator/commutation core

Updatedgoal2006cabb read. Added opt-inbench-lean-core, requiringstaticCOMP.
RealphysicalBEMF comparator path preserves dropout/owner/source checks and
sharedreferencecomp_isr, without diagnosticdispatchcount/ratecap/timebrackets/
traces. Synthetic/coastbench cases are not silently substituted: leanpath
requiresREAL_IRQ/LIVE_IRQ/COAST_REFERENCE. Sourcequalify still checkssoftware
mask, hardwareenable and actualpendingflag before invokingreference.

LeanObs EV_ACC keeps powered_timer::accepted electrical/trackingguard and
operationalACCEPTS progress forforegroundwatchdog. It removesACCEPT_STATS,
ACCEPT_TIMELINE,prefix/tailrings, perrecordtiming and unrelatedeventcounts.
COMwrapper omitsCOM_MAX measurement, retainsactualTIM16flags/referenceISR/
ownerstate checks. ConflictingCPU/coreprobe/filterexperiment features are
compile-time rejected to keepimagesseparate. This is a leanCORE candidate,
not all-vector diagnostic-free qualification: DMA/guardmetrics and other
adaptercosts stillremain. IRQstormcountcap no longer inleanCOMP; existing
priority0guard, feedbackfreshness and finitecampaign shutdown remain, but
storm/overrun behavior needs actualhardwarequalification.

Release-s/thinLTO linkedbuildPASS withoutCPUunion/flyingreentryfeatures;
audit and frozen captures/reference/leancore_704/shell-pwm.elf SHA256
D413F29734EED0CA1ED6191308658742D8C1045BC8B2F1257DC176FAEE0097A6.
arm-none-eabi-size text116928/data1132/bss27724 vspreviousaverageguard
125372/1140/29304:8444textbytesless. This is NOT speed or occupancy evidence.
Afterfreeze, sourceMonitor::event_policy sector%6 replacedwithbounded6->1
branch;41guardtestsPASS includingstrict/report/defaultorder/stalecases.
That lastsourceedit notyetinD413ELF; sourceaheadcandidate noted.
NoUART/SWD/motor. ActualFFFC lastverifiedOFFE703. NextcompleteDMA/guard
diagnostic separation and explicitleanreport identity, then hardwarechecks;
calibratedaveragecurrent/normalstartuprecovery and30%exploration remainopen.

## E705 — lean DMA/guard steady-state diagnostic separation

Updatedgoal2006cabb read. bench-lean-irq extendslean-core: omits DMAmax clock
bracket, feedbackcount/peak/bus-min and firstdeliverysnapshot, guardMAX_US,
COMcommit maximum/vetocounter, queuepeak and fast-event/cycle report counters.
Preserves coherentfeedback/agechecks, averagecurrentaccumulator, finiteguard
deadlines, sectororder, queuecapacity/overflow latch and commit100usoverrun
shutdown. Needed commitstartclock remains, not mislabeledinstrumentation.
Foregroundrawsums and operationalprogress/commit counts still present.
StartupISR work and remainingcontroladapters need separateaudit; not a claim
of completezero-diagnostic qualification architecture. LEANCORE r1 summary
identifiesrecorders/maxima0 and lean_irq1 so emptydiagnostics cannot certify
timing or lock. Oldstrict captureverifiers are not automatically transferable.

Release-s/thinLTO build/auditPASS, frozen captures/reference/leanirq_705/
shell-pwm.elf SHA256
7D6B3B07AA06171BEC381456D8EFAE44F828B178F1FA3E1B8E2E8DE1494C613A.
Text114948/data1132/bss27572. No CPUmeasurement orpoweredqualification.
41normalguard and7averageadaptertestsPASS. Targetedactuallean-policytest
compiledwithbench-lean-irq:1PASS/38filtered, confirms report-onlyshortevents
withzerodiagcounts, stale/orderkills and queueFIFO/capacityrefusal.
InitialPowerShellcfgquotingfailedcompilation and executablemissing; then
checkedPythonargv compile+executePASS, no staleharnessrun.
NoUART/SWD/motor; actualFFFC lastverifiedOFFE703. CandidateNOTflashed.
NextremainingstartupISR/audit and hardwarecomparison; currentcalibration,
normalstartup recovery and duty-envelopeexploration remainunfinished.

## E706 — lean hardware refusal and guard shutdown verified

Updatedgoal2006cabb read. Auditedcounterdependencies: foregroundcontrol
usesoperationalACCEPTS, not omittedACCEPT_STATS/timeline; COMcommitcount
remains for firstcommit/ownershipchecks. Leanaccepted recorder now also
rechecksACTIVE/OBSSTATUS before operationalprogress publication.
Found disabledguarddiagnosticPOSTSTOP depended onremovedVETOcounter.
Changed it to count each actual refusedcommit locally, after checkingthat
nooutput revived. NoISRdiagnosticrestored. LeanGUARDPROFILE explicitly
marks timing_measured0; printedzero maxima are not speed/WCET evidence.

Release-s/thinLTO build/audit/freezePASS, captures/reference/leanirq_706/
shell-pwm.elf SHA256
D5FDFC6F3EC0152165786361E39A6FE2CCA55835A89E7F5F253C8EBE7F34BACC.
Exactfrozenflash andOpenOCDresetexit0. Realunconfiguredaverage run200 refusal
beforephase drivePASS, finaloffPASS (leanirq_706_refusal.txt).
Ownguarddiagnostic3timerfaults/18poststopcommitrefusalsPASS,finaloffPASS
(leanirq_706_guard.txt). UARTclosed, allgates/EN/MOE/CCRs0,nFAULT1.
No ordinarymotor spin, calibratedthreshold provision orCPUmeasurement.
StartupISRcleanup/gaincalibration/normal-startuprecovery/30%exploration
remainunfinished; these disabledchecks do not qualify poweredleancontrol.
ActualD5FD installed/OFF. Fullgoalactive.

## E707 — remove hidden shared COM trace from lean build

Updated practical goal read fully. The D5FD TIM16 disassembly contains
__aeabi_uidiv at 0x8001a9e from shared ZC_TRACE batch gating, not motor control.
Lean binz's recorder removal had not removed this reference-owned trace.
Added am32_isr::tim1_up_tim16_isr_policy<const TRACE>; the original public
API forwards with TRACE=true, while bench-lean-core explicitly selects false.
Only the trace write is conditional; interrupt acknowledgement, COM disabling,
commutation, interval blend, advance/wait, comparator enabling and zero-cross
saturation retain their original order. Shared minz dirty edits preserved.

All 22 reference ISR host tests PASS. New paired test covers old-routine
false/true and zero-cross 5/10000, matching HAL actions and resulting controller
state while confirming traced/untraced record counts 1/0.
Release-s/thinLTO build exit0; emitted math audit has no TIM16 helper calls.
Frozen captures/reference/notrace_707/shell-pwm.elf SHA256
71192D3AB849B33CBB21FC532B125F150D6C4AFA50FEC3E6B51E783F29F4C95E.
Candidate NOT flashed; actual D5FD remains last verified OFF E706.
No UART/SWD/motor action, no measured occupancy improvement, no powered pass.
Polling trace and startup diagnostics need separate treatment; calibrated
average-current threshold and normal-startup recovery remain unfinished.

## E708 — no-trace image installed, shutdown checked on hardware

Updated practical goal read fully; previous turn made source/codegen progress.
Frozen notrace_707 ELF hash verified as
71192D3AB849B33CBB21FC532B125F150D6C4AFA50FEC3E6B51E783F29F4C95E.
Flash exit0, then OpenOCD reset exit0. Existing Python process 57848 was
identified as pdf-mcp rather than a serial fixture; no concurrent UART session.
drv_average_refusal.py exit0: missing threshold refused, outputs off.
Capture captures/notrace_708_refusal.txt. drv_guard_check.py exit0: three
timer faults, eighteen refused post-stop commits, outputs off.
Capture captures/notrace_708_guard.txt. UART closed, actual no-trace image
installed/OFF. These are disabled shutdown checks, not sustained motor testing
or CPU occupancy evidence. Calibration remains unproven; no raw threshold
was invented or motor run attempted. Next average-current calibration path
and lean-aware powered observation, with normal-startup recovery still open.

## E709 — enforce the emitted arithmetic regression

Practical goal read fully; E708 was hardware shutdown progress. Current
measurement plan rechecked: retained E619 same-DMA residual is -2.178904
counts with no simultaneous independent reference. Same acquisition mode
does not establish a valid gain calibration, and old PSU anchors cannot fit
this residual into a physical current scale. Startup uses the bounded
software-driven feedback path; average protection currently covers BEMF DMA
only. No nominal gain was relabeled as measured calibration.

drv_math_audit.py now supports repeatable --forbid-caller NAME with exact
emitted symbol matching and exit1 on any arithmetic helper category there.
This is a direct-call regression gate, NOT transitive ISR or inline wide-math
verification. Two host regression tests PASS, including saved D5FD versus
7119 TIM16 code and exact-name/category behavior. Actual CLI audit on frozen
7119 returns0; the same command on D5FD returns1 as expected. Reports retain
all other callers for review. No firmware changes, UART/SWD or motor run.
Actual7119 remains last verified OFF E708. Calibrated average protection,
startup coverage, lean-aware powered evidence and normal recovery unfinished.

## E710 — remove startup timer row recorder in lean image

Goal read fully. Startup ownership audit finds wave startup and driven
acquisition use bounded foreground software ADC reads, not the steady BEMF
DMA scan. Simply feeding those irregular reads into the 50-scan accumulator
would not establish the same time average or baseline acquisition conditions.
No current calibration or startup average-protection pass is claimed.

Independent aligned cleanup: bench-lean-irq driven_run::tick omits READS row
recording, diagnostic reads/tick_max and the diagnostic buffer-cap stop17.
Electrical/readback, guard freshness/deadline, DMA health and observer.sample
arguments/order remain. The default diagnostic path retains its original
row writes/cap behavior. Observer internal counters still remain, and other
startup diagnostics need review; this is not a zero-all-ISR qualification image.

Release-s/thinLTO build exit0, emitted audit/TIM16 direct-helper gate PASS.
Frozen captures/reference/leanstartup_710/shell-pwm.elf SHA256
CF38B901A4E89411055E7838C97F59EB03E2D4430E4EC3227B58605C1D66F4BD.
Attempted cargo test --lib --target x86_64-pc-windows-msvc failed before
execution: embedded panic_impl duplicates std test panic_impl. Incremental
directory permission notes also emitted. No test binary executed; not a
semantics PASS, no claim of hardware validation or measured CPU benefit.
Candidate NOTflashed, actual7119 last verified OFF E708; no UART/SWD/motor.

## E711 — isolate pure host tests and omit duplicate startup microscope

Goal read fully. Attempted test-only panic cfg removes E710 duplicate panic
error, but cargo host lib then fails linking MCU interrupt/Reset vectors.
Reverted own lib changes rather than mocking board symbols; production panic
handler unchanged. Added scripts/startup_policy_test.rs and checked runner
scripts/test_startup_policy.py, compiling real detector/driven_observer and
average_current against the actual host minz-core library. Fifteen tests PASS
(six average policy, four detector, five observer), no filtered tests. This
does not execute full driven_run ISR or prove hardware timing/equivalence.

Authority audit: driven_run::tick polling Observer sample produces diagnostic
candidates only. Actual handoff construction is comp_irq -> driven_irq_live
seed -> guarded release token, with independent live IRQ command validation.
Lean TIM6 now omits polling COMP reads/bracket/detector work as well as the
E710 row recorder; all guard and DMA health checks remain. Default microscope
unchanged. Observer command validation remains; not all startup diagnostics
are eliminated. Legacy observer summary must not be treated as lean detection
quality. No average-current or gain calibration claim follows from this.

Release-s/thinLTO build exit0, emitted audit/TIM16 helper exclusion PASS.
Frozen captures/reference/leanstartup_711/shell-pwm.elf SHA256
3CD3E0CEB3778168EE490CD7B820DBC7FDC96B17667FAEA619CB2FA8573D4CC4.
NOTflashed; actual7119 last verified OFF E708, no UART/SWD/motor. Current
calibration, startup average coverage, normal recovery and envelope unfinished.

## E712 — explicit nominal average setting, without fake calibration

Goal read fully. It requires average-current protection during exploration
and calibration for a device-rated pulse clamp, not precision metrology before
every run. Added an explicit idle avgnominal command alongside avgraw, no
automatic boot threshold. Provisions 1991 raw-count sum per 50 scans, from
nominal500mA*7mOhm*gain10*4096*50/(1000*3600mV), rounded down. Reports all
assumptions and calibrated=0 uncertainty_bounded=0. VDDA3600 is conservative
relative to nominal3300 for the raw allowance, NOT a bound on total measurement
error. CSA gain/offset/sampling and analog accuracy remain unverified; this
setting cannot certify a physical500mA ceiling. Existing electrical guards,
missing-config startup refusal and fresh-zero installation remain unchanged.
Startup averaging is still missing, so this alone is not the goal's complete
protection architecture and no powered run was made.

Initial u32 constant numerator overflow failed compilation; checked runner did
not execute stale harness. Replaced with u64 compile-time intermediate; Rust
const evaluation resolves the entire expression. Sixteen real pure-policy
tests PASS, release-s/thinLTO build exit0. Math audit has no nominal helper
callers and TIM16 helper exclusion PASS; no runtime wide arithmetic added by
this constant. Frozen captures/reference/nominal_712/shell-pwm.elf SHA256
877510AF41541A2FA8E42ACFC18FCCD32B795F721D1D4D7D26D02348F4ECE8A3.
NOTflashed, actual7119 last verified OFF E708. No UART/SWD/motor or new envelope
evidence. Next startup averaging/ownership continuity, then lean live control.

## E713 — current sample accumulator spans normal startup and handoff

Goal read fully. bench-average-current now feeds the existing three current
channels from shell startup capture_sample into the shared average monitor.
Any missing/revoked/tripped monitor stops through the existing current shutdown
branch and retains the offending row. Driven acquisition feeds its complete
foreground Feedback.phase after live ownership recheck; average failure calls
end with Current before feedback publication/continued acquisition. No new
ADC conversion or timed ISR sampling loop added. Steady BEMF DMA hook remains.
No reinstall or partial-block discard at producer transitions. ENABLElow still
revokes the monitor; subsequent wake requires a fresh pre-drive baseline.
This gives sample-count continuity, NOT a uniform-time average: startup
foreground cadence differs from201us DMA, mixed blocks retain unequal sampling
conditions and baseline/coverage uncertainty. It cannot certify the nominal
500mA physical limit or a calibrated safety specification. Pulse guard remains
the old unchanged backstop pending rated/clamped channel calibration.

Sixteen purepolicy tests and eight actual adapter/policy tests PASS. Adapter
tests cover missing config, revoked state, equality/exceed/latched trip and
freshzero reinstall; they do not execute shell/driven hardware IRQ races.
Release-s/thinLTO build exit0, TIM16 emitted helper exclusion PASS, frozen
captures/reference/startavg_713/shell-pwm.elf SHA256
39B9F7CED7FB5B190EB6F93CF38053EE74887DD3901D3776C5E8503B26F2ECAB.
NOTflashed, actual7119 last verified OFF E708. No UART/SWD/motor. Next prepare
lean-aware exploration capture/configuration, then bounded actual startup/hold;
not another flying recovery qualification gate. Goal remains unfinished.

## E714 — real lean startup/hold, then retained initial-current refusal

Goal read fully, previous turn made average integration progress. Updated
live_armed_baseline.py with explicit --nominal-average, requiring exact idle
avgnominal ACK retained in .average.txt; no automatic default. --lean-explore
verifies unique FINALOFF and outputs off, reports controller lines/targetACK,
never calls empty timelines lock or qualification. Missing lean summary is
retained as a possible startup failure, not a success. One host report test
PASS with three negative finaloff variants. No speed/quality inference added.

Verified frozen startavg_713 hash39B9; flash exit0 and OpenOCD reset exit0.
Only existing Python owner identified pdf-mcp. Own disabled guard3timerfaults/
18poststoprefusalsPASS, capture startavg_714_guard.txt. Then actual normal
startup into 7%/10s BEMF, no injected dropout. Command:
python scripts/live_armed_baseline.py --out captures/startavg_714_hold70.txt
--ms 10000 --nominal-average --lean-explore --cycle450 --fast-cycle-report
--event100 --dma-guard --seed500
Scheduled POWERPATH reason2 at10000004us,20269COM/POWERCOMMITS, average987ticks,
COASTREF stop7/desync0, complete finaloffPASS, UARTclosed. Lean IRQ counters,
peak/busmin/maxima are omitted; zeros/stale initialized values are NOT measured
CPU/current/bus-floor evidence. No accepted-event ratio/sigma qualification.
Capture SHA256 5D52EC58312C6C3B9E7ED2657019959BE24EA4B3CFC2D0951FCD90608F6A3BA4.

Next one same-command --ms30000 --ramp-duty200, output startavg_714_ramp200.txt:
FAILED before BEMF/live ramp during initial driven acquisition, Current reason5
at9410us,70foregroundscans,12IRQaccepts, no transferred seed/COM/targetACK.
Capture SHA256 2DBC53ACAEAD3413117B067FF3B2E1A1B7090028000F3012D951E97AF3B2D0CD.
This is NOT a20% ceiling; startup still61tenths. Average failure and old pulse
failure share Current5, so exactsubcause not proven by stopcode. Retainfailed
run, no retry or thresholdchange. FinaloffPASS, UARTclosed, actual39B9/OFF.
Next distinguish average versus pulse refusal and inspect retained foreground
sample block; no flyingrecovery gate or precision-current claim. Goal active.

## E715 — preserve average-failure samples and distinguish monitor refusal

Goal read fully. CRC-decoded E714 failed initial acquisition's70 DA85 rows,
maximum abs(raw-2048)=485, below1200 legacy pulse threshold. Found new average
check returned BEFORE row retention, so the failing sample was not saved.
Cannot use preceding valid rows alone to prove its precise raw cause.
Moved average check after the foreground ADC row save; now end reason25
distinguishes average-monitor refusal from old Current5. No threshold change.

Release-s/thinLTO build/audit/TIM16 gate/freeze PASS. avgcause_715 ELF hash
F96D86F2FEA7ECD703DB35EB077E294D58403FC22D49F706D6FB751AA5AA19C1.
Checkedflash/OpenOCDresetexit0, own guard3/18PASS, avgcause_715_guard.txt.
One live --ms30000 --ramp-duty200 --nominal-average --lean-explore --cycle450
--fast-cycle-report --event100 --dma-guard --seed500 attempt retained in
captures/avgcause_715_ramp200.txt, SHA256
177FB4381124B2255B0BF1246F3957A65A6B07234EE838FD5556FA87F4A55AC2.
Startuprelease22/freshtransfer1; BEMF184COM then POWERPATH Current5 at98383us,
before query/live duty changes. Target20% NEVER acknowledged; not20% ceiling.
FinaloffPASS, UARTclosed. DMA average path still combinedreason5 in that image.

Then refactored trip to trip_reason(code), preserving firstreason/STOP_US and
exact stop/gatesoff/ENABLElow cleanup. DMA average monitor failure uses25,
legacy pulse still5. No successful-path extra ISR diagnostic/counter added.
Code25 can mean threshold, missing state or ADC rail; not a calibrated current
measurement. Second release-s/thinLTO build/audit/TIM16 exclusion PASS, frozen
avgcause_715b ELF hash
DA8EB1E7F1099213091363EDC419C730D3B3D25624495E64110123ACC285FC3B.
715b NOTflashed; actual F96D installed/OFF. No repeat after second source edit.
Next install distinct DMA refusal image and inspect retained outcome, not
raising raw threshold or calling pre-ramp refusal a running speed envelope.

## E716 — startup average refusal proven; separate ADC trigger backend staged

Goal read fully. Frozen715b DA8B hash verified, flash/resetexit0, ownerprocess
pdf-mcp only. Own guard3/18PASS, captures/avgcause_716_guard.txt. One normal
--ms30000 --ramp-duty200 nominal-average lean exploratory attempt stops sine
startup at2659091us/tick2650 before underdrive or BEMF/live query. CAPreason4,
final offending row decoded/CRCchecked: ia1651 ib2093 ic2855 vbus1212 vref1506
flags7. No rails, peak807<1200. From installed branch predicates, the average
monitor refusal necessarily triggered this stop. No physical500mA excess is
proven, no signal-noise/wiring verdict. Startup sequential foreground sampling
still lacks proven representative time-average. No threshold changed.
Capture avgcause_716_ramp200.txt SHA256
FD1EFF62F83EA6FF538594EF2D234444D1997FF01EAE57CB7920026C5D2078B0,
finaloffPASS/UARTclosed, actualDA8B/OFF. Target20 was never acknowledged.

ST primary source stm32g0xx_ll_adc.h confirms TIM15_TRGO EXTSEL_2 (raw4),
versus TIM3 raw3:
https://raw.githubusercontent.com/STMicroelectronics/stm32g0xx-hal-driver/master/Inc/stm32g0xx_ll_adc.h
Added opt-in bench-adc-tim15 to existing ADC stream start/stop: native PAC
TIM15 RegisterBlock (no cast), APBENR2 TIM15EN, const EXTSEL4. Default TIM3
backend unchanged. Same finite admission/coherentDMA/queue/restore logic;
existing driven/TIM3 restrictions deliberately remain, so this is NOT yet
continuous startup sampling or simultaneous-owner permission. Timer-specific
bench-filter-raw/bench-adc-phase probes compile-refused until ported. Dump
marks timer15/extsel4/backend_only1/continuous_startup0.
Release-s/thinLTO build/audit/TIM16 gatePASS, frozen tim15_716 ELF SHA256
DC8767D4AA98691F828C5D2D3E3DD3BFE41C0B82099CF9E7BB4B80135C2EE302.
NOTflashed, no hardwareTIM15/restore/timing qualification. Next disabledroute
check then continuous stream ownership, rather than tuning a biased threshold.

## E717 — TIM15 trigger validated on the powered, gates-off board

Goal read fully; E716 hardware/source work was progress. Verified frozen
TIM15 DC876 hash, checkedflash/OpenOCDresetexit0, no competingUART owner.
Ran drv_baseline_check.py, saved captures/tim15_717_baseline.txt SHA256
54183F76361597D2E0115F0F93F32713A183D9610B0BE6F955D06D2E6E1E6AB6.
FourcasesPASS: disableddriver0samples/0us; hostabort0samples/71us;
ENABLEinterrupt0samples/73us; complete128samples on all5channels/25633us,
same-wake valid then oldbaseline refused afterENABLElow/rewake.
No gate commanded. NativeTIM15 backend/routedADC produces coherent CRC raw
data at expected acquisition duration. This validates the disabled trigger
building block, not continuous startup ADC or independent hardware register
restore readback. Existing stream boundedstop/restore completes successfully.

Added --tim15 to baseline fixture and extracted pure verify_cases. Requires
exact timer15/extsel4/backend_only1 metadata for EACH case; mismatch rejected.
Savedcapture replay and one host test with four backend negative variantsPASS,
no unnecessary repeat of hardware acquisition. Own guard3timerfaults/
18poststoprefusalsPASS, saved tim15_717_guard.txt. FinaloffPASS, UARTclosed.
ActualDC876 installed/OFF. No motorrun/mean-current calibration claim.

Continuous-startup integration constraints verified from source: interrupt(),
take() and start admission currently poweredowner-only. Startup capture_sample
does foreground ADC4/1/0/2/3/6/13; driven_run baseline and feedback also use
foreground reads. These MUST be replaced in a timed-startup feature, not left
running alongside DMA. Lean startup VSENC/neutral are diagnostic, so coherent
5-channel feedback can replace them without expanding scan width; explicitly
mark omitted feedback rather than fabricating sensing data. Keep one stream
and original acquisition timestamps across sine -> driven -> BEMF, retain
freshness/overflow/rail/bus/nFAULT shutdown and cleanup before coast ADC reads.
Current source still has strict admission and no new simultaneous ownership.
Next implement exclusive timed-startup consumption/ownership; goal active.

## E718 — timed startup feedback cache policy staged and tested

Goal read fully; E717 was verified hardware progress. Added pure
examples/support/startup_feedback.rs, no peripheral authority and no firmware
binding yet. Latest stores only a coherent5-channel raw Frame plus original
32bit acquisition timestamp. Publish rejects rails, stale delivery, duplicate/
reordered acquisition or producer gap>1ms, latching invalid producer state.
Foreground read checks original acquisition age without refreshing it; wrap
uses bounded wrapping deltas in ONE shared serialized clockdomain. Deliberate
cache overwrite is not FIFOoverflow: the average guard must consume every
DMA producer frame independently, never repeatedly average this cached frame.
Caller still must shut down on errors; this policy alone does not stop gates.

Standalone startup runner now20puretestsPASS, including4new cachetests:
repeatedreadsage/staleboundary, invalidproducerlatching, wrappedstamp/overwrite,
allfivechannelrails. Not live ISR/ownership/race/timing qualification. No new
firmware build/UART/SWD/motor; actualDC876 last verified OFF E717. Remaining
integration is substantial: dedicated startup DMA producer/admission and one
clockdomain across handoff; replace shell and driven foregroundADCreaders,
preserve fresh/aged initialfeedback and stop/restore before coast. Not a claim
that continuous startup sampling or representative average is implemented.

## E719 — wire optional latest cache into the real DMA producer

Goal read fully; E718 completed pure cache policy work. Added bench-adc-latest
implying TIM15, feature-gated static Latest and serialized adc_stream::latest
accessor. It requires powered ownership and stream active; returns original raw
frame/acquisitionstamp only if stillfresh. No fabricated delivery timestamp.
The real DMA ISR publishes only AFTER existing per-frame electrical/average
guard succeeds, BEFORE FIFO push; publish error masks DMA and calls original
stream_fault cleanup (detail10, poweredADCtimeout reason11). Existing FIFO
overflow handling remains. Average guard remains fed exactly once in the
producer, not by repeated latest cache reads. Cache resets at masked start
and stop, including already-inactive stop, so oldsession data cannot survive.
No startup admission change: poweredowner-only paths deliberately remain until
foreground ADC readers are ported. This is NOT continuous-startup integration.

Release-s/thinLTO build exit0, emitted math audit/TIM16 helper exclusion PASS.
Twenty real purepolicy tests PASS (not full adapter/peripheral race tests).
Frozen captures/reference/latest_719/shell-pwm.elf SHA256
7EE81A00CCD90DECB51ADB73A000943A7A4A68F0B43A29B2A1D6A06D556ED991.
NOTflashed; actualDC876 remains last verified OFF E717, no UART/SWD/motor.
No measured ISRcost or envelope gain; latest foreground accessor has no caller
yet. Next dedicated startup owner/clockdomain and ADC-reader replacement;
do not activate DMA alongside legacy startup conversions. Goal active.

## E720 — immediate ADC revocation, deferred bounded cleanup

Goal read fully. Before startup DMA ownership integration, added opt-in
adc_stream::quiesce: serialized DMA mask, stop new selected-timer triggers if
stream active, mark QUIESCED, invalidate Latest. No ADCSTOP wait or restore
inside a fault ISR. Called AFTER physical bridge_clear from gates_off, and
after physical ENABLElow/provenance/current revocation in calibration_live.
latest() rejects quiesced stream; ensure_started rejects it and performs
foreground bounded stop rather than silently treating paused hardware as live.
stop() finishes/restores acquisition as before and clears quiesced state after
active cleanup. Default non-latest feature path unchanged. Startup ownership
and foreground ADC readers remain unported; no continuous startup claim.

Release-s/thinLTO build exit0, audit/TIM16 helper gatePASS, frozen quiesce_720
ELF SHA256
2B137A23DDEEB8FBF65F847E28C6077BBD5F725CD403ADB5D4DB548AB98542C3.
Checked flash then OpenOCDresetexit0. Own --tim15 hardwarebaseline4casesPASS:
disabled0; hostabort0scans/73us; wakeinterruption0scans/75us; complete128x5/
25631us then oldepochrefused. Saved captures/quiesce_720_baseline.txt.
Own guard3timerfaults/18poststoprefusalsPASS quiesce_720_guard.txt.
AlloutputsOFF/UARTclosed, actual2B13 installed. These are disabled foreground
ADC abort/cleanup and guardchecks, NOT real running fault ISR latency or
producer/cache race qualification. No motorrun or new current/envelope result.
Next dedicated startup producer/admission and exclusive reader replacement.

## E721 — acquisition clock independent of BEMF segment reset

Goal read fully. Source audit found powered CLOCK resets at segment startup;
using it for a future uninterrupted startup stream would change old timestamp
meaning at handoff. Latest backend now has independent ACQ_CLOCK reset only
when the ADC stream starts. Original acquisition stamps remain in Latest.
Guard/FIFO publication maps them into the current powered segment domain,
preserving age rather than retiming the sample as fresh. Clock::map_stamp
samples both clocks from ONE hardware TIM17 snapshot under the same IRQ mask;
no read skew or unsynchronized reset/subtraction. Default non-latest backend
keeps its previous segment-relative timestamps. No startup ownership change.

Twentyfive purepolicy tests PASS, including actual map_stamp across a segment
clock reset and source softwaretimestamp wrap, preserving75us age. Existing
10s/multiplehardwarewrap tests remain. Clocks must still be fed at least once
per65536us; not a CPUblackout watchdog. Hardware/adapter race and ISRcost
verification not inferred from pure tests.
Release-s/thinLTO build exit0, math audit/TIM16 gatePASS, frozen acqclock_721
ELF SHA256
06B0CBD8AB15C6F740F872747C46D1FF2632FDBF67F54D6F8C5B53FDE4CB2DDC.
NOTflashed; actual2B13 last verified OFF E720, no UART/SWD/motor. Remaining
work still dedicated startup producer/admission and exclusive ADC readers;
no continuousstartup/protection completion or new throttle envelope claim.

Next separate minimum-spacing quality from freshness/sector-order kills;
revisedgoal permits diagnostic thresholds report-only absent demonstrated
safety need. Keep electrical limits,1ms missing-progress shutdown, order
checks and finite execution. Actual2F1B/OFF,UARTclosed. No25/30passclaim.
