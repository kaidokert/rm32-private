# E547 — distinguish running speed envelope from tracking protection

## E578 recovery review, no admission change yet

E577 refusal is now diagnosed explicitly by the host verifier before staging
checks: result14 CycleTooFast,5intervals/3112us, no second powered segment.
CRC/first-segment archive/finaloff required; test verifies failed capture stays
failed and missing finaloff cannot pass. No retroactive successful recovery.

Source audit: flying_acquire RuntimeAcquire and SEED_MIN_TICKS are shared by
driven_seed and flying_bench, so a simple global change would broaden both
driven and passive seed admission. Do not describe it as recovery-only.
core_bench handoff remeasures edge age, checks >=64half-us remaining, and
refuses armcost>16us. Neither check is removed by a speed-profile proposal.
Actual reference TEMP_ADVANCE=16, advance=(ci*16)>>6,wait=(ci>>1)-advance:
ci952 =>wait238ticks=119us, age<=87us to retain32us;
ci895 (~372eHz) =>wait224ticks=112us, age<=80us;
ci834 (~400eHz) =>wait209ticks=104.5us, age<=72.5us.
Thus lowering seed-speed floor alone cannot guarantee a valid handoff; fresh
seed age must fit the smaller budget. E577 failed after5intervals, before
qualification, so it contains NO measurement of final12interval seed latency.
Next isolate admission-profile scope and measure/optimize recovery's final-edge
path while retaining32us floor and16us arm ceiling. No motor/flash/guardchange;
13B5 lastverifiedoffE577. Avoid repeated knownbound refusals or latency claims
from a seed that never qualified.

## E577 sustained8% PASS; recovery retains old speed refusal

Same13B5. Fixed80/30s --cycle400 --seed-timing-reanchor --dma-peer --carrier-hz20000,
startup61/+60deg. Hold completes30000033us,67831COM/67830accepted,
376.844974eHz,cycle sigma33.537us acrossrun,raw392,bus10805,IRQunion53.2624%.
Last32events span13771us,26same-sector cycles mean2672.115us/sigma19.532us.
That tiny tail is not whole-runsteadyquality. HoldfullverifierPASS/finaloff.
Capture cycle400_577_hold80_30s.txt SHA
2E23166017C14AA868B1A3A4F907A389D54368B9EA8674981B8861E3C35F3D01.

One subsequent --dropout --reentry80/30s attempt: first segment4491COM/
4490accepted,raw431,bus10889,trackinglossinjected2000138us,stop2001109us.
Reacquisition result14 CycleTooFast: cycle5370half-us ticks<5716, at5intervals
and3112us; no recoveredpoweredsegment. REENTRYresult5,REENTRYSTATSused0.
CLI fails on stagingprovenance, a consequence of not reaching guardinstall;
not a failure to spin initially. parse_dumpCRC,decode_first_segment and finaloff
independentlyPASS. No recovery qualification or relaxed admission inferred.
Capture cycle400_577_recovery80_30s.txt SHA
4DEB6E2EEAD7682C4E0F34335BF304CFBC2B4E2A861888D2595458B34CEC2BCD.
Both finalgates/en/MOE/CCRs0,nFAULT1,Uartclosed. No retry/flash/guardchange.
Next review recovery admission with measured arm margin; don't rerun unchanged
known-bound refusal or conflate sustained operation with recovery support.

## E576 first candidate motor evidence: baseline and live8% PASS

Same13B5 cycle400. One --cycle400 baseline70/10s PASS329.970eHz,
19798COM/19797accepted,raw304,bus10901,observed10000034us. Then one
--cycle400 --step80 (query70 then80): full10000033us deadlinePASS,
21603COM/21602accepted,final controlleravg885ticks (~376.65eHz estimate),
rawpeak353,busmin10889,49750ADCscans,commitmax26us,IRQunion52.0713%.
Full-run360.065eHz/sigma185.186us mixes both duties and acceleration, NOT
steady8% quality. ArmACK and D0050reply retained. No additional guard or
firmware changes, no retries; each finaloff/nFAULT1 verified andUARTclosed.
captures/cycle400_576_baseline70.txt SHA
486F1C24194BB37DB289FBF1E6E7FC3B443D9EF20D8F9334D5692FFAF80123AA;
captures/cycle400_576_step80.txt SHA
280FA7CAA859D9E450A8B6CA82CE166C7E9FA6EF5F36D8976466A02DD8768BF4.
Acquisition/transfer/CRC/ADC/timeline validated with explicitprofile.
Next sustained8% quality measurement, not another automatic speedbound change.
Old350-band recovery admission remains; this is not recovery qualification.

## E575 cycle400 installed, no motor yet

Current/root13b5f0447080264d57db66c4d742f76fad62880fbc1ad8921fe7b3c344de206b
release-s/thinLTO build+emittedauditPASS. Explicit --cycle400 recognition added
to driven_handoff and live_armed_baseline; exact2500/238/max6000/1000 header
required, missingflag/mismatch rejected. Synthetic hostprofiletestPASS and
33actualguard hosttests previouslyPASS; no oldcaptures reclassified.
Old029Foffverified beforedownload, newdownload/OpenOCDreset exit0. Own disabled
guard3/18,IRQbudget5,filter,atomic256,role6at3200/62,CPU2/6/9us,archive3 and
ADCphasePASS. captures/cycle400_575_preflight.txt retains finalalloff/nFAULT1;
MCPclosed. No motor oncandidate yet. Next one70/10s baseline with --cycle400,
then8%step onlyif baselinevalid; no automatic threshold escalation.

## E574 staged follow-on: cycle400, not yet built or installed

E570-573 actual029F supports guarded live writes. E57370->80 reaches the
2778us cycle floor with2755us guard/2752.5us reference cycle, currentraw311,
bus10960mV, ADC/timelinevalid, 6583COM/6582accepted. Its measured IRQ union is
1728663/3335096us=51.8325%; max observed COM/live-write bracket22us, DMA22us,
clock-observation gap93us. These are measured brackets, NOT WCET or proof of
physical speed/lock at an untested operating point.

Stage bench-cycle400 (inheritscycle360) changing ONLY running cycle floor to
2500us. At400eHz ideal sector416.7us versus238us unchanged minimum event gap;
this arithmetic is context, not a scheduling guarantee. Current/bus/nFAULT,
deadline, freshness, maxevent1000us, slowcycle6000us and tracking rules retained.
Startup/recovery seed range and32us arm floor unchanged; higher running speed
does not imply recovery admission. No reference-core or estimator-window change.
This is a deliberate experimental envelope expansion, not retroactive E573PASS.
Host exact2499/2500/2501us boundary/refusal-latch and other-guard tests added
in scripts/cycle400_guard_test.rs. Need explicit host profile recognition,
release build/audit and own disabled preflights before any candidate motor run.
Then one existing-setting baseline, followed by bounded8% live request only
if baselinevalid. Retain failures; no automatic profile ratchet or30% sweep.
Actual installed029F remains cycle360/offE573; source now differs.

Basis: E540–541 current inline7E55 completes3/3 thirty-second6.9% recovery
campaigns. E546 independent steady supply reading11.7V/~70mA,noCC/CVblips
at335.122eHz; raw phase322,busmin10996mV,IRQ50.716%,finaloff verified.
No observed steady supply wall. Phase pulse guards still required; ~32mA
nominal baseline-subtracted ADC estimate does not agree quantitatively with
PSU70mA and is not treated as calibrated current or another motor-work blocker.

The goal permits staged speed-envelope expansion after preceding evidence.
bench-cycle360 inherits range350 but changes ONLY running same-sector cycle
floor2858->2778us (ceil1,000,000/360). Event floor238us,maxevent1000us,
slowcycle6000us,current/bus/nFAULT/deadline/feedback guards unchanged.
Acquisition stays seedmin952/cycle5716/individual476ticks; actual remaining
arm>=64half-us and armcost<=16us unchanged. Therefore recovery can refuse
above its old admission band even when running is allowed. Not a360eHz
startup/recovery qualification or a physical-overspeed sensor.

No COM-keep-running or read-call experiment enabled. No sharedcore edits,
new live instrumentation or loss-of-tracking waiver. This explicitly changes
the envelope guard; it does NOT retroactively pass earlier failures or prove
they harmless. Broader controller correctness remains to be measured.

33 hostRust guard tests pass (one initial test wrongly assumed the initial
accepted edge had an inter-event predecessor; corrected test, policy unchanged).
New host profile test exercises exact profile selection and unchanged recovery
admission using a synthetic header substitution; never a real reclassification.
Release-s/thinLTO/codegen1, emitted arithmetic audit required for binary.

Bounded test: own disabled guard/fullfive3200/ADCroute, then one7.0%10s hold,
explicit --cycle360, all other settings baseline6.1%startup/+60deg/20k/trace0/
dma-peer. Success requires completed verified window/off, valid chronology,
current/bus and independent tracking guards. Failure ends this attempt; no
automatic profile ratchet. A pass motivates recovery testing at the same
point, not a sweep to30%. Preserve startup/refusal denominators.

## Hardware result E547

Current/root/frozen09C32D2F8508C3C36EFBC87629CB474C38B68A20F718101F74713D05C6971ABE
at captures/reference/cycle360_547/shell-pwm.elf, mathaudit alongside it.
Qualified7E55 safed beforedownload; OpenOCDreset. Candidateown guard3/18,
fullfive3200preflightCPU2/6/9,ADCroute3PASS. No new ISR instrumentation.

Command:
```
python scripts/drv_driven_handoff.py --out captures/cycle360_547_start61_hold70_10s_01.txt --ms 10000 --drive-duty 61 --bemf-duty 70 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle360
```
OneattemptPASS10.000013s,340.584838eHz,20435COM/20434accepted,cycle sigma
41.961726us includes acceleration. Recordedcycle min2844us belowold2858;
recorderclock differsfromguardclock so not exactcounterfactualfaulttime.
Rawphasepeak371,busmin10948mV,IRQunion51.041439%,COMP40/COM45/commit21/
guard16us,stack3460. Startupraw829/bus11522. ADC/phase49751 eachvalidate;
allwindow/timeline/finaloff checksPASS,Uartclosed. Capture SHA256
a72c34c3567d6d6843a8dbb6a5a11a79265790478b1210768a14c07ccabf645f.

This is an actual guarded7.0% completion, not a policy-only replay or proof
all prior excursions harmless. Electrical/tracking/seed/arm protections held
unchanged; full-cycle speed envelope was explicitly expanded. No independent
qZC/rotor-speed proof or calibratedcurrentclaim. Nextsamepointrecovery, not
anotherprofileincrease. Current09C3 installed/OFF; candidateCOMfeatures off.

## E548 — 7.0% original-deadline recovery passes

Same09C3, no rebuild/reset/guard changes. Freshguard3faults/18refusals PASS.
One run with E547command plus --ms30000 --dropout --reentry; output
captures/cycle360_548_start61_reentry70_30s_01.txt. Full fixture PASS initial
entry, injectedTracking shutdown, fresh-seed recovery, originaltimeline/off.
Resumed27.991285s at340.693223eHz,57219COM/57218accepted,fullcycle sigma
23.010795us,min2842/max3035us. Recordedcycles again crossold2858floor while
the segment completes, not an independentphysicalrotorspeed measurement.
Rawpeak321,busmin10913mV,IRQunion51.000767%,COMP40/COM45/commit21/guard4us,
stack2668. ADC/phase139260 eachvalidated. Freshseed996halfus ticks,
acquisition6121us,edgeage166ticks,remaining83ticks=41.5us,arm7us. Unchanged
32usfloor has9.5us spare. Originaldeadline200usspare. No acquisition limit
change, oldbaseline revokedacrossrecovery; no calibratedampsclaim.
Capture SHA25601710c8f92e4728814adbedb19470c457b8824f829e30cd7cd6d9902d0a5334f.

One recoveryattempt/onepass at7.0%; notyet repeatedcohort. Current/root09C3
remainsinstalled/OFF,Uartclosed. Next boundrepeatabilityatthispoint before
furtherduty exploration withinthesameprofile; no automaticnewceiling.

## E549 — 7.0% recovery3/3, then7.1% hold passes

Predeclared two additional70/30s recoveries afterE548, stoponfailure; both
PASS. Then one71/10s holdPASS. Same09C3/profile/settings, no code/flash/limit
change. Freshguard3/18PASS beforecampaign, eachrunverifiedoff/UARTclosed.

| Resumed metric | E548 01 | E549 02 | E549 03 |
|---|---:|---:|---:|
| Accepted-rate eHz |340.693223|340.531736|340.714094|
| Cycle sigma us |23.010795|23.209093|23.312844|
| COM / accepted |57219/57218|57192/57191|57222/57221|
| IRQ union percent |51.000767|50.969728|50.954763|
| Raw peak |321|354|323|
| Bus minimum mV |10913|11032|10996|
| ADC / phase |139260/139260|139261/139261|139259/139259|
| Fresh remaining us / arm us |41.5/7|40/7|40.5/7|
| Original deadline spare us |200|213|214|

Alloriginal30s campaigns includeoneTrackinginjection/freshseedrecovery and
~27.991s resumedoperation. Stack2668 throughout. Threeattempts/threepasses,
no exclusions. Oldseed350 and32usarmfloor unchanged; calibratedamps and
independentqZC notclaimed. Stop equivalent70repeats.

71/10s:PASS10.000015s,345.338272eHz,20720COM/20719accepted,
cycle sigma42.659436us includingacceleration,mincycle2807us,raw292,
bus11008mV,IRQ51.067989%,ADC/phase49751,stack2668 cumulativepaint.
Oneholdonly, not71recoveryqualification. Nextsamepointrecovery; keep360
profilefixed and retain any acquisition/timing/electrical refusal.

New captures/hashes:
- cycle360_549_start61_reentry70_30s_02.txt:
  7a931bf5dff7732eeb97b36029696ff9f8d698861ad071966d2cbf69e755b0cc
- cycle360_549_start61_reentry70_30s_03.txt:
  1c54b778c7e20e7c3dc83ac4f6c24ea4bd5bedd2c89d9009f344dd2ec10ae0e5
- cycle360_549_start61_hold71_10s_01.txt:
  de7a817782d7186871e224574d63e7a74c1c7e1568b0ba4416fc68d9c2e1c23b

## E550 — 7.1% recovery passes;7.2% hits cycle envelope

Same09C3, freshguard3/18PASS. One71/30s recoveryPASS: resumed27.991139s,
344.928551eHz,57930COM/57929accepted,cycle sigma22.213760us,min2804/max2997,
raw369,bus10948mV,IRQ50.919717%,ADC/phase139259,stack2668. Seed975ticks,
edgeage166/remaining78ticks=39us,arm7us,acquisition6355us. Originaldeadline
208usspare,fullrecovery/timeline/finaloffPASS. Onlyone71recovery, notcohort.
Capture cycle360_550_start61_reentry71_30s_01.txt SHA256
fe7060eb4404efe74a52d7dd316505415d89c2985953009a9acd49aaea353d80.

After validation, oneexploratory72/10s hold FAILED CycleTiming12 at8.627467s.
All other settings/guards fixed.18077COM/18076accepted,wholewindow349.226475
eHz includesacceleration; raw304,bus11044mV,IRQ51.366054%,ADC/phase42922,
stack2668. Guardstep2 delta2761<2778us,previous8624673/decision8627434.
Reference previous5853ticks/refused5517ticks=2926.5/2758.5us,twocyclemean
2842.5us,closure+2.5us. Controlleravg945,lastavg950,interval953,thiszc889,
lastzc986,wait238,filter12,running1/polling0 aftersafing. Late/short pattern
recurs at the expanded envelope; not evidence of physical overspeed or an
independent lock certificate. Average945 also warns recovery could hit the
unchanged952seedfloor; thisrun didnotattempt recoveryat72.
Capture cycle360_550_start61_hold72_10s_01.txt SHA256
7cf51af0b0b36fefdfceb29fb9900f7fb3962810782505ecc9e639080f021a24.

No retry,furtherduty,profilechange,codechange orflash. Failedcapture retained;
finaloffverified,Uartclosed,current09C3/OFF. Nextreview speed-estimator versus
singleaccepted-cycle envelope and retained recoverybudget; no blind floor
ratchet. Supply is not implicated by thisrecord. Goalnotcomplete.

## E551 — offline two-cycle estimator study, NOT deployed

scripts/drv_cycle_window_study.py plusfourtests: validatesoriginalfaultcontext/
CRC/off and comparesretained referenceintervals, never reclassifiesfailures.

| Failure | Floor us | Single referencecycle us | Two-cycle mean us |
|---|---:|---:|---:|
| E537 call6.9% |2858|2842.5|2964.5|
| E545 COMkeep7.0% |2858|2857.5|2920.5|
| E550 inline7.2% |2778|2758.5|2842.5|

Allsinglevaluesfail/allmeanspass at thisoneboundary. Not whole-run replay:
mosteventsomitted,referenceclocknotguardclock,no independentphysicalrotor
measurement. Selectedfailuresdo notestablishfalse-positive rate.
safe_to_deploy=False in allreports.

Synthetic policy retains238..1000us gaps/order,6000us singleslowcyclelimit;
onecyclefloor until12intervals then2cyclemean.475us spacing/100us single
eventdelay:singlefails/twopasses.150usdelay:bothfail. Averagingattenuates,
doesnotuniversallycancelshiftedboundary.475->400usspacing:singletripsfirst
newinterval/twosecond,400usaddedlatencyinthisexample,NOTworstcasebound.
Steady250us harmonictrain,237usgap,1001usgap,wrongsector allrefuse. Silence
needsindependenttimerpoll;eventlistscannotproveit. Warmup/wrap/overflow,
abruptaccelerationandfalseorderededges needruntime-policyproof beforedeploy.
No independentlockcertificate orhardwaretimingqualification fromthesetests.

No firmware/guard/flash/UART/motorchange. Current09C3lastoffE550. Nextdesign
must accountfordelayedspeeddetection and retainelectrical/progress/order/
feedback/fresharm protections; no blind2778lowering.

## E552 — host policy and acceleration-delay bound, NOT deployed

`scripts/cycle_window_policy.rs` implements the candidate as a host-only Rust
module, not included by firmware. It retains per-sector single-cycle warmup,
the6000us slow-cycle limit, wrapping timestamps and a latched refusal containing
the actual span and threshold. Caller must first validate sector order/gaps,
and independently poll missing events. Both summed operands are bounded to6000
before addition (sum<=12000). No division or wide arithmetic in this policy.

`rustc --edition=2021 --test scripts/cycle_window_policy_test.rs -o
target/cycle_window_policy_test.exe`, then executable `--nocapture`:8testsPASS,
including4 existing event-monitor tests. Covers u32 wrap at two offsets,
463us exact boundary versus462us warmup refusal, displaced boundary100us
tolerated versus150us refused, latching, silence, order and fast events.

Exhaustive constant-spacing acceleration sweep: old463..1000us, new238..462us,
121050 pairs. Maximum extra detection delay **2772us**, old474 -> new462,
single-cycle refusal event36 versus two-cycle event42. This is materially
larger than E551's400us example; not a physical acceleration model or approval.

Conditional general bound: if every subsequent sliding single cycle stays
below2778us after first single-cycle refusal at event k, by k+6 both adjacent
cycles are below2778 and their sum fails5556. Time to k+6 is itself below2778us.
Assumes valid ordered events; independent <=1000us event-age enforcement still
must catch silence. Alternating above/below cycles need not satisfy this premise:
the policy limits a two-cycle average, NOT the old instantaneous speed bound.

This is a protection-semantic trade, not a deploy-safe replacement. Before
integration assess transient allowance against commutation/arm timing, retain
independent guards, then measure emitted code and guard cost on its own build.
Fault telemetry needs actual window length, not a fictitious single-cycle delta.
No firmware/flash/UART/motor changes.09C3lastoffE550; original failure retained.

## E553 — timing review and 7.1% recovery cohort:2/3 campaigns

Source review: `flying_acquire::Seed::handoff_with_min` enforces remaining64
half-us ticks only at handoff; `core_bench` checks that installation costs<=16us.
The ordinary reference interrupt routine arms `wait_time+1`, and `com_timer`
does not enforce that handoff-specific32us floor on each subsequent acceptance.
`powered_timer::commit` checks electrical/freshness/deadlines and bounds its
own duration, not accepted-cycle speed independently of RunGuard. Therefore
the handoff margin cannot certify the averaging candidate's transient allowance.

Added executable counterexample (host harness now9testsPASS): after slow warmup,
six238us gaps alternating with six688us gaps pass order, event gaps and the
5556us two-cycle sum, while admitting1428us individual cycles. This is not a
physical acceleration claim; it disproves preservation of the instantaneous
envelope. Average-only replacement remains NOT deployed. Any future combined
policy needs a separately justified instantaneous bound, not an invented number.

Actual board work used unchanged09C3 (root/frozen SHA matched), no flash.
Fresh `cycle360_553_guard.txt`:3faults/18post-stop refusalsPASS/off verified.
Predeclared two additional7.1%30s recovery attempts, same command as E550,
startup61, phase60,20k carrier,trace0,dma-peer,cycle360,dropout+reentry.

1. `cycle360_553_start61_reentry71_30s_02.txt`: PASS. Resumed27.991152s,
   325.442840eHz,54658COM/54657accepted,cycle sigma29.161976us,raw397,
   busmin10984mV,IRQunion50.425929%,ADC/phase139259 each,stack2668.
   Recovery seed1047,age166,remaining96ticks=48us,arm7us,acquisition6528us;
   original deadline220us spare. Timeline/recovery/finaloff validated.
   SHA256 bb26060d46f52640b52d21a877efa4271e763acedfd7b48ec1b6d565cb1853ff.
2. `cycle360_553_start61_reentry71_30s_03.txt`: FAILED initial seed acquisition,
   no BEMF powered handoff/recovery. DRIVEOBS reason2 at20231us,seed fault3,
   ready0/intervals0/cycles0. Full epoch1 acceptance1453us -> epoch2 acceptance
   2524us:1071us gap=2142ticks exceeds2000ticks, matching retained TIM2 interval
   2142. This is the explicit `driven_seed::Qualification` fault3, NOT a
   running cycle360 guard refusal. Epoch0 correctly excluded. Startup raw1060,
   busmin11271mV; acquisition archive/CRC/finaloff independently validated.
   SHA256 75da21a966c2adc18257f7027631da0fa485a107d495ad5a43ff0b05c8fa1fe8.

Fixture exits first at missing RUNLIMIT/profile verification because no powered
segment exists; that message must not be mistaken for a flashed-profile change.
No retries or guard changes. With E550, commanded7.1% cohort is2/3 campaigns,
both reached recovery succeeded, one failed before handoff. Successful speeds
344.929 and325.443eHz differ; do not claim repeatable345eHz qualification or
attribute the difference to hardware without evidence. No new PSU observation
or calibrated-current claim. Current09C3,all outputs off verified,Uart closed.
Next investigate retained initial-acquisition timing failure; do not ratchet
duty/floor or spend another campaign on the average-only candidate.

## E554 — startup refusal localized; bounded fresh-window candidate staged

E553 failed command/accept chronology: epoch1 command948us/accept1453us,
epoch2 command1757us/accept2524us. Command spacing809us plus acceptance-offset
change505->767us yields1071us. The reference interval2142ticks corroborates
the gap; not evidence of a1071us scheduler stall. Later accepted epochs remain
recorded because acquisition continues under forced-drive guards until20ms.
No physical-ZC/rotor cause established from these timestamps.

`driven_seed.rs` now has a cfg-gated, explicitly selected
`with_timing_reanchor` candidate. New `bench-seed-timing-reanchor` feature is
declared but NO live caller selects the constructor. Existing constructors
retain old behavior. One shared reanchor budget covers missing-epoch and
long-interval restarts. Only a valid ordered long interval, corroborated by
the TIM2/bracket relationship, may start a fresh window. Zero-time, invalid
order, short intervals and bad brackets remain refusals. Original start time
and40000half-us deadline are retained; discarded fault3 is separately retained.
The new anchor contributes zero intervals; twelve NEW intervals and seven
cycle checks are still required. No powered guard, cycle floor or arm change.

`python scripts/test_seed_timing_policy.py`:39 Rust tests PASS against the real
host minz-core dependency. Includes actual first15 E553 DI85 rows: oldpolicy
fault3/no seed; candidate discards only epoch1->2 interval and qualifies at
epoch14,edge23570ticks=11785us,mean1543ticks,12intervals/7cycles. This is a
counterfactual seed only, NOT a successful handoff or motor recovery. Tests
retain second-restart refusal, shared missing-gap budget, original deadline,
corrupt reference interval/duplicate/fast-event refusal, and existing tests.
One initial test incorrectly overlapped timing brackets; corrected synthetic
fast-gap case to100ticks so it reaches the intended acquisition-fast check.

No target firmware build/flash/UART/motor this entry. Installed09C3lastoffE553;
root ELF unchanged, source now has inactive candidate. Before deployment:
select constructor only under feature, report discarded cause+fresh anchor,
teach strict host validation the new window without reclassifying old failures,
release/LTO audit and disabled timing checks, then bounded entry cohort.

## E555 — live feature wiring and strict host provenance, built NOT flashed

`bench-seed-timing-reanchor` now depends on driven-reanchor and selects the
new constructor only for that feature in driven_irq_live. Post-run SEEDTIMING
reports discarded_fault0/3, shared one-restart budget,12freshintervals and
original deadline. Existing DRIVENSEEDRESTART still identifies count/anchor.
Host `--seed-timing-reanchor` is required for candidate captures; absent or
unexpected marker rejects. seed_window validates first eligible ordered long
gap, corroborating TIM2 bracket, valid preceding prefix and unchanged deadline.
Malformed/duplicate/wrong-cause/wrong-anchor records refuse. Old failed E553
capture remains failed even with synthetic candidate provenance appended.

Tests:13 seed Python tests,57 driven Python tests,39 Rust policy tests PASS.
Release opt-s/thinLTO/codegen1 builds. Candidate root/frozen SHA256
0fe48ab540f544aee2d269c62a50be0d1231c2a31c479377a8c7efceaa51cae0,
in captures/reference/seedtiming_555/shell-pwm.elf with math-audit.S/json.
Features are E547's complete list plus bench-seed-timing-reanchor. No guard,
duty, carrier, acquisition threshold or handoff-margin changes.

Emitted new long-gap branch in driven_run::comp_irq includes bounded acquire
reset via memclr4/memclr, then a branch to fresh-anchor processing (no recursive
call on this emitted path). This has not been timed on hardware. Audit is not
a helper-free certificate: driven_irq_live::command has a uidivmod call, and
all retained audit findings remain available for review. Need disabled timing
of the newly exercised reset branch as well as existing guard/carrier/ADC checks
before powered use. Do not borrow baseline timing qualification for this ELF.

No flash/UART/motor. Installed09C3 remains lastoffE553; ROOT ELF NOW0fe48a
candidate, NOT installed. Next safe/flash/disabled checks, not motor yet.

## E556 — disabled reset check fails timing; NO motor run

Added feature-only `seedcheck` command and drv_seed_timing_check.py. With bridge
disabled and no powered owner, it runs64 fresh long-gap resets and validates
discardedfault3/anchor2/zero joined intervals, then second-long-gap refusal.
TIM17 brackets black-boxed policy invocation, including argument/result handling;
this is NOT full comparator ISR WCET. Predeclared isolated budget10us leaves
room within the existing50us acquisition observer limit; does not prove that
combined bound or waive it. Full preflights remain required.

Release-s/thinLTO build/audit SHA256
e752a7c56ee425a2f6b22efb5fed7fd878ec73873e0447e76d8124ab2d4ffe02,
frozen seedtiming_556 with emitted audit. Safed old09C3 via guard3faults/18
poststop refusals, downloaded new ELF, OpenOCD reset. Capture
seedtiming_556_disabled.txt:64trials,failed0,reset_max_us18,disabled1.
**FAIL10us timing gate**, all semantic cases passed. Final outputs-off independently
verified from cleanup; UART closed. No motor or full preflight on failed build.

Emitted evidence explains why the E555 code view is insufficient: adding the
second caller caused Qualification::accept to become out-of-line, shared by
the live COMP path and harness. It now contains __aeabi_uidivmod at0800908a
(sector successor/remainder class), plus reset memory work. Do not attribute
all18us to memclr or transfer E555's inline behavior to E556. Next eliminate
bounded sector remainder exactly and inspect/reset timing; no gate increase.
Current board/root E752,disabled but NOT powered-qualified. E555 never flashed.

## E557 — exact bounded sector arithmetic saves3us, timing still fails

Replaced driven_seed successor/reanchor remainder expressions with private
const next_step/after_gap helpers. Caller invariants step1..6 and gap2..6 are
checked before use; compile-time exhaustive assertions cover every input.
39Rustpolicy testsPASS. Release-s/thinLTO build and emitted audit: no arithmetic
helper calls attributed to Qualification::accept (prior uidivmod removed).
This is not a whole-firmware helper-free claim.

Safed E752 via freshguard3faults/18poststop refusals, download/OpenOCDreset.
Current/root SHA256 bf5f9527143c84c8e6a56e6cdb2a28662d4d3a4219ed245e4a6d4fc9efa83587,
frozen seedtiming_557 ELF/audits. seedtiming_557_disabled.txt:64semanticsPASS,
reset_max_us15 versus18 E556, still FAIL10us gate. Finaloff independently
verified, UARTclosed. No motor/fullpreflight or thresholdchange.

Remaining emitted reset branches still call memclr4/memclr. Next examine
in-place invalidation with original start retained: stale per-sector timestamps
must never be read until their seen bits have been repopulated. Prove equivalence
before skipping clears; do not infer all remaining15us is memory-reset cost.
Currentbuild is disabled-only, not powered-qualified.

## E558 — reset invalidation and nonrecursive anchor reduce cost to11us

Feature-only Acquisition::restart_window preserves original start and clears
all validity/counter/summary fields, leaving six sector timestamps inert behind
seen=0. Used only by timing-reanchor branch; missing-epoch path unchanged.
New equivalence test poisons old seed/fault/history and compares a fresh object
across216 combinations (2 clock origins,3 deadline offsets,6 initial sectors,
3 spacings),15 events each. Includes u32 wrap, expiry, fast/slow rejection,
cycle summaries and freshness.40RusttestsPASS.

Variant A SHA7509097b16324e2daaa269f80d9fe0d0102361beef451a29d5ad0b8e3c83d068:
release-s/thinLTO/audited,frozen seedtiming_558a. SafedBF5guard3/18,download/
OpenOCDreset; seedtiming_558_disabled.txt64semanticsPASS,cost13us FAIL10.
Variant B removes recursive re-entry after already-validated reset, branching
directly to common fresh-anchor processing. Same40testsPASS. SHA
19b8e8301ea486ac4969d92eb6dd92817625b78fb5ee0e0822757e86561990e8,
frozen seedtiming_558b. SafedAguard3/18,download/OpenOCDreset;
seedtiming_558b_disabled.txt64semanticsPASS,cost11us STILL FAIL10.

Both raw failures retained and finaloff independently verified. No motor or
fullpreflight, threshold unchanged. These are complete harness brackets, not
isolated memory-reset costs or live ISR WCET. Current/root19B8/OFF/UARTclosed,
NOTpowered-qualified. Inspect remaining duplicate admission work/codegen next;
do not round11 down, subtract observer cost, or increase the gate.

## E559 — duplicate-check removal no gain; inlining regresses

Removed the second successor check: preceding block has either validated the
same predecessor/successor or cleared previous.40policytestsPASS. Release-s/
thinLTO/audited variant4a261a2b0f6baab6dd3404de5833bc3dfce66f02494a1eb759fc015c4eaee50c
downloaded after freshguard3/18,OpenOCDreset.64semanticsPASS,11us stillFAIL10.
Raw seedtiming_559_disabled.txt retained; this intermediate ELF not frozen.

Then explicit feature-only inline(always) on accept, removing aggregate call
boundary in live/harness. Release-s/thinLTO/audited variant
dc0db63c9b63770cac3811d5caaeeed49387a48faed6d2052de78aa085ba410e,
frozen seedtiming_559b. Freshguard3/18,download/OpenOCDreset.64semanticsPASS,
12us WORSE than11 and FAIL10. Actual/rootDC0D/OFF,sourceinlinecandidate,
NOTpoweredqualified. Bothrawfailures/finaloffvalidated,Uartclosed,no motor.
Inlining is not an improvement; do not claim eliminated call implies gain.

The isolated10us gate was a campaign engineering allocation, not a silicon
limit or user-specified requirement. It remains failed. Before further tiny
code changes, inspect which instructions are inside the timed bracket and
derive the complete acquisition-ISR deadline budget. Do not relabel12us a
pass or silently widen the gate. A different full-path qualification would
need its own explicit measurement and rationale; current result proves none.

## E560 — real handler limit promoted from post-run check to immediate refusal

Source budget review: driven_irq_live previously only retained max_us; host
drv_driven_run rejects >50us after the run. driven_run::comp_irq masks interrupts
around the observer and handoff; driven_run::sector independently stops when
command lateness>50us. Thus the isolated10us seedcheck is an engineering
allocation, not a directly enforced hardware deadline. It remains failed;
neither prior12us nor this unmeasured build is relabeled a pass.

Promoted the existing50us observer-body limit to live enforcement: reuse its
existing final timestamp, retain maximum, increment overruns, stop comparator/
interval timer and returnfalse if elapsed>50. Caller end(reason21) safes bridge
before testing owns/consuming a seed. Adds no new timestamp read. Postrun
DRIVENIRQBUDGET reports limit50,overruns,immediate_stop,before_handoff,
excludes_release. Explicit timing-reanchor fixture requires exact singleton
metadata and zero overruns. Other refusal modes still share reason21 but the
new counter distinguishes duration failures. No electrical/order/freshness/
cycle/arm/deadline guard is relaxed.

Scope: this brackets observer body through qualification, not outer exception
entry/exit or later transfer/release. It detects overruns after return, not
unbounded stalls or an unobserved16-bit timer wrap. Do not call it WCET proof.
13seed+57drivenPythontestsPASS including missing/failed metadata rejection.
Release-s/thinLTO/audited root/frozen seedtiming_560 SHA
4bc0c363d8242fbb8d1046b43027f56974581a0ab154abff7f1a0fa81c3dfcab.
NOTflashed; actualDC0DlastoffE559. NoUART/motor this entry. Before any powered
assessment: disabled direct test of50/51us stop boundary and handler cleanup,
existing guard/carrier/ADC preflights, then separately stated guarded whole-path
measurement. Historical10us isolated refusals stay failed, not a silent waiver.

## E561 — disabled handler-stop boundary and full preflights pass

Refactored existing duration decision into inline finish_duration, shared by
live handler and disabled irqbudgetcheck. Synthetic elapsed0/49/50/51/65535
tests exact decision, TIM2.CEN and logical comparator-mask cleanup, counter
increment; bridge remains disabled throughout. This does NOT measure full IRQ
execution, exception latency or powered safing. Caller end(reason21) ownership
ordering remains source-verified. Host --budget-check is separate from old
seedcheck; regression test explicitly keeps old12us isolated test failing.

First build overflowedFLASH352bytes. Tool orchestration erroneously continued
and downloaded OLDroot4BC0,not the failed build; no motorcommand. Explicit
OpenOCDreset/guard3faults18refusals verifiedoff. Fixed procedure to inspect
exit codes before subsequent audit/download. Removed rejected inline(always)
experiment; size-opt-s/thinLTO build now fits. Current/root/frozen seedtiming_561:
c7d0b31adacd6edfed04bafc5b35aefb1b9540d407a55d2fb14bb9397451827e.
Audited/downloaded/OpenOCDreset; all subsequent tests on this image.

seedtiming_561_budget.txt5casesPASS, guard3/18PASS, fullfive3200preflightPASS
(pulse,256atomic,roles,CPU2/6/9us,archive), adcroute3PASS. One new host test
actualcapture+malformedcasesPASS. ELF/audits frozen, finaloffverified/UARTclosed.
No motor on candidate yet; isolated10us allocation NOT declared passed.

Next campaign question is guarded whole-handler behavior, not satisfying the
isolated microbenchmark: one previously qualified6.9% bounded10s hold with
--cycle360 --seed-timing-reanchor --dma-peer --carrier-hz20000. Require actual
DRIVENIRQmax<=50/overruns0,unchanged current/bus/freshness/order/arm/deadlines,
strict seed replay and finaloff. A run with discardedfault0 does NOT exercise
or qualify timing-restart reliability. Retain any failure; no repeat-until-pass.
This separately scoped measurement does not erase historical10us failures or
claim wholepathWCET. Duty expansion/recovery cohort remain unfinished.

## E562 — candidate powered hold and fixed3/3 recovery cohort pass

Same installed/rootC7D0; freshguard3faults/18refusalsPASS. No flash or code
change. One6.9%10s hold then predeclared three7.1%30s recovery campaigns,
startup61/phase60/trace0/PWMroles20k/dma-peer/cycle360/seed-timing-reanchor.
Recovery commands add dropout+reentry. All attempts retained; no retries.

Hold: seedtiming_562_start61_hold69_10s_01.txt SHA
f40775127d679537722fef854875b40fbc2ae72bc11a23b2229353c43e14f927.
10.000033s,319.549271eHz,19173COM/19172accepted,raw345,bus10984mV,
IRQunion50.927809%,ADC/phase49751 each,stack3436. Acquisition handler23us,
overruns0; no restart. Timeline/CRC/finaloffPASS.

Fixed7.1% cohort, filenames seedtiming_562_start61_reentry71_30s_0N.txt:

| N | Result | resumed eHz | COM/accepted | cycle sigma us | raw peak | bus min mV | IRQ union % | ADC/phase |
|---|---|---:|---|---:|---:|---:|---:|---|
|1|PASS|329.416176|55325/55324|27.541283|327|10841|51.333684|139260/139260|
|2|PASS|329.860541|55399/55399|27.228591|343|10984|50.922450|139258/139259|
|3|PASS|331.621359|55694/55693|24.784979|321|10853|51.406449|139257/139257|

Resumed durations27.991242/27.991139/27.990737s; recovery seed1019/1021/1025,
age166ticks all,remaining89/89/90ticks,arm7us all. Acquisition6264/6358/6742us,
original deadline182/183/207us spare. Stack2644. All acquisition-handler maxima
23us,overruns0. All original deadlines/recovery/timeline/finaloff validated.
Third run used missing-epoch reanchor at epoch3 (count1,discardedfault0); the
new long-interval restart was NOT exercised in any run. 3/3 is campaign result
on this build/operating region, not proof of that branch or independent lock.

SHA256 by N:
1:20f21047dd3a6b635f94f609e54690d6f06a31310360c983381de0bb378079e6
2:1c33cafab9822921100fc7111754606ff64cb5edc643b3ee6045ebbf96304c7f
3:dc9dce695a5a8bdab68c29fccce60e3f891688bf75c7c3b19796c11302444db4

No new PSU observation or calibrated-current claim. Original isolated10us
failures remain failed; these measure actual handler on successful inputs,
not its unexercised timing-reset path or WCET. CurrentC7D0/OFF,Uartclosed.
Next bounded7.2%10s hold on unchanged envelope; do not repeat this cohort just
to encounter the rare branch. A targeted disabled/live-injection design is a
separate possible test of long-gap restart, not grounds to fabricate one.

## E563 — 7.2% hold/recovery and7.3% hold pass, unchanged guards

SameC7D0, freshguard3/18PASS. No code/flash/profilechange. Commands retain
startup61,phase60,trace0,PWMroles20k,dma-peer,cycle360,seed-timing-reanchor.
All three runs handlermax23us/overruns0/discardedfault0, no reanchor, strict
acquisition/timeline/CRC/finaloffPASS; raw ADC sums/phase counts validated.

| Capture suffix | Result | eHz | COM/accepted | raw peak | bus min mV | ADC/phase |
|---|---|---:|---|---:|---:|---|
|start61_hold72_10s_01|PASS|337.700240|20262/20261|310|10984|49751/49751|
|start61_reentry72_30s_01|PASS|336.191010|56462/56461|336|10937|139258/139259|
|start61_hold73_10s_01|PASS|338.904204|20334/20333|318|10937|49751/49751|

Filenames prefixed seedtiming_563_.72hold10.000034s,IRQ51.911518%;72recovery
27.991037s resumed,sigma25.878398us,IRQ51.544443%,seed1000/age166/remaining84
ticks=42us,arm7us,acquisition6592us,originaldeadline146usspare.73hold
10.000035s,cycle sigma48.113922us includesacceleration (not steady-only).
Stack2644 across runs. One72recovery, not3/3;73recovery notyettested.
New long-interval restart remains unexercised. No calibrated-current or new
PSU observation claim. CurrentC7D0/OFF/UARTclosed.

SHA256 in table order:
f30d2878c1fa2e355361ba12cc390768b06af881839425180a42c56cf793e156
1f26e4def09ba822a4721c06355ac41a9b5bd78491ccbe3431e9f7d9df800689
f5245a4cd6a61b5dddbc60fb243718e96772557d1652eb694448866b62c32c95

73hold IRQunion52.074434%. Next73recovery before another duty step; existing
2778us cycle floor,952tickseedfloor and32us remaining-arm bound stay unchanged.

## E564 — 7.3% recovery and7.4% hold pass; next startup refuses

SameC7D0,guard3/18PASS,allcommands unchanged except BEMFduty. No flash/code/
guardchange.73/30srecoveryPASS340.003998eHz,27.990648s resumed,57102COM/
57101accepted,sigma26.647603us,raw332,bus10948mV,IRQ51.458831%,ADC/phase
139257each. Seed999/age166/remaining84ticks=42us/arm7us,acquisition6591us,
originaldeadline256usspare. Capture seedtiming_564_start61_reentry73_30s_01.txt
SHA5bf09b16698722fc24910e04eb2a3a283a2e83e3aba5eecd5ed71615d235a44b.

74/10sholdPASS346.241798eHz,10.000035s,20773COM/20773accepted,raw323,
bus10960mV,IRQ52.066093%,ADC/phase49751each. Capture
seedtiming_564_start61_hold74_10s_01.txt SHA
cc0d00c9716aaa6bd34b77b51cca43a095044dd6243911a8b26f0802e923deaa.
Bothactualacqhandler23us/overruns0,strictCRC/timeline/finaloffPASS,stack2644.

Then74/30srecoveryattempt FAILED INITIAL ACQUISITION, before poweredhandoff:
DRIVEOBSreason2 at20265us,seedfault3,ready0,11intervals/6cycles. Missingepoch2
consumed reanchor at epoch3; later long interval cannot earn a second restart.
SEEDTIMINGdiscardedfault0,DRIVENIRQmax23us/overruns0,22acceptedepochs retained.
Startupraw1121,bus11486,ADC141scans; acquisition/CRC/finaloff independently
validated. Fixturefirsterror missingRUNLIMIT reflects no poweredsegment,not
profilemismatch. Capture seedtiming_564_start61_reentry74_30s_01.txt SHA
004158f33ddb8f892015a81014d5b707839f249322f6fae1c97ce8c9c5730c2d.

No retry or75%tenths step.74holdsuccess is not recoveryqualification and this
failure does not implicate74steady duty (startupduty remained61). CurrentC7D0/
OFF,Uartclosed. Next inspect discarded-window/remainingdeadline evidence to
choose an entry improvement, not another duty step or wider timing threshold.
