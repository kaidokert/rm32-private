# E442: remaining timeline initialization on the arm path

Source and hash-matched7F4B emitted-code audit. No runtime edit, flash or motor
attempt. Installed7F4B remains stopped followingE440.

`event_timeline::Timeline::new` is const fn but its window is runtime data:
it computes `(window_us+5)/6`. `coast_run_inner` invokes it inside the masked
flying-seed arm block AFTER `Timer.set_and_enable(arr)`. The measured16us
arm-cost check includes this work. Making the constructor const did not
eliminate its soft divide. Emitted7F4B has `__aeabi_uidiv` at08006160 with
divisor6 after64-byte timeline initialization. Another constructor instance
is08005eda; do not count both as necessarily executed by one recovery.

Authority: target/thumbv6m-none-eabi/release/examples/
shell_pwm-fa8b10da68fadbc9.math-audit.json reports SHA256
7f4b25f6022c9a15df004f59fd8cac6394c9a3e3b208a5925a1ed099d903f9d9;
the adjacent .math-audit.S contains these instructions. Source reference:
examples/support/core_bench.rs flying arm block and event_timeline.rs new().

This is NOT the missing85us edge-age fix: age is sampled before timeline
construction. Moving work before that sample could reduce arm_us while
INCREASING reported age and reducing remaining_arr. Reject such a change as
an edge-to-arm improvement unless both quantities and total completion time
are compared. Current measured armcost13us leaves3us against16; this is still
a useful separate target, not a quantified gain.

Candidate: prepare a zeroed timeline and exact bin width before the final
qualified edge, but the recovery duration is selected only after acquisition
from the original campaign budget. Therefore a stale duration/template cannot
be installed unchanged. Separate zero-state preparation from final exact
duration metadata, or use a proven bounded division-free constructor. Keep
last=u32::MAX, all six empty bins, overrun0, exact window/bin fields and
serialization against accepted-event recording. Do not transfer diagnostic
work into a long interrupt-masked foreground section during powered operation.

Other helper findings prevent misattribution: acquire_inner080095f0/08009602
are ADC bus conversion inside the scan, not automatically post-final-edge
cost;0800991c is the optional tracking branch. A caller-name helper inventory
alone cannot establish which successful recovery path paid for each call.

Next implement a bounded exact constructor optimization with exhaustive
quotient-boundary tests, then compare emitted code and both arm timing metrics
before any powered claim. Guard admission, seed timestamps and firmware speed
profile must remain unchanged. This does not replace the broader fresh-edge
handoff work needed for substantial speed expansion.

## E443 — candidate implemented, not flashed

Timeline::new uses a const exact32-bit base4096 division helper. Three radix
digits cover fullu32. Each intermediate<=24575; multiplying by10923 fitsu32.
10923/65536=1/6+1/196608, so maximum error<1/8 cannot carry a fractional
remainder<=5/6 into the next integer. Window bounds and all metadata unchanged.
Tests exhaust all24576 digit inputs, every12-bit boundary/neighbors/residues
through600000005, u32 endpoints, const evaluation and constructor refusals.
195library+34other Rust replay and367Python tests PASS. Nonfatal existing
incremental AccessDenied and firmware warnings retained.

Release opt-s/thinLTO candidate SHA256
`7C3724E846DBF562E734485D6FC158080D6EF67432A9391AE99BD6C6E10DFB48`.
text123892/data1104/bss29336 (+56text vs7F4B). Same campaign feature set.
Automatic hashed audit matches candidate. Timeline::new0800b0f8 now has only
32-bit multiply/shift arithmetic for division, no helper call; coast_run_inner
division-call count falls4->2 (remaining calls are not declared removed).
LLVM outlined the constructor; its52-byte local frame and call overhead mean
this is NOT measured latency improvement. Do not infer CPUgain from callcount.

No flash/motor operation. Actual7F4B/off remains installed. Next disabled
guard/fivepreflights, then matched72recovery only if those pass. Compare BOTH
edgeage/remainingarr and armcost against7F4B's85us/13us, plus stack/IRQ/capture
validity. Preserve all guards, profile350 and originaldeadline; no73retry on
the basis of a source-level arithmetic change alone.

## E444 — first matched hardware comparison passes

Installed7C37 after UARTverifiedoff. guard01 failed UARTreadback; retained
timelinediv_boot_snapshot01 BEFORE repair shows APBENR108000000,USARTCR10d/
BRR22b,ENABLE0/BDTRc1a/CCRs0. Documented08040000 clockrepair restoredUART;
guard02PASS3faults18refusals. FivepreflightsPASS,CPUmax2/7/10. Bootbugnotfixed.

timelinediv_start61_reentry72_30s_01 passed original30s startup/loss/recovery,
resumed27.990987s at336.395357eHz,56496COM/56496accepted,sigma23.885046us,
IRQunion53.531175%,raw303,bus10877mV,stack2644,DMA24us/queue2.
Seed1013/acq6672us,edgeage172ticks=86us,remaining81ticks=40.5us/spare8.5us,
armcost8us. SEEDLAT90/106/122/152/156. COMP40,COM55,commit21us maxima.
SHA256 `FFFF86B8B70E78326D5F672BE995BB7DA7CF6165E26CDF0FA24FF6082D9DA6C4`.
Finaloff/portclosed,process0. Startupraw501bus11545. No guard/PSU change.

Versus7F4B72cohort's85usage/13usarm: age+1us, armcost-5us, sum98->94us.
This is one observed comparison, not isolated-instruction/WCET or reliability
proof. Stackuntouched fell2676->2644; still above512 gate. IRQunion change is
not credited as a gain. The unchanged32us age-based arm floor still applies,
so reduced armcost does NOT grant a higher-speed admission automatically.

Actual7C37 installed/off. Next two same72recovery repeats failfast to check
timing and repeatability before further changes; no73retry yet.

## E445 — second attempt fails sustained timing; cohort stopped

Same7C37, no firmware/guard changes.02 failed after24.287464s recovered at
336.414492eHz,49024COM/49023accepted,sigma23.698675us,raw300,bus10948mV,
stack2644,DMA24us/queue2. Seed1000/acquisition6569us,age86us/remaining39us/
armcost8us: acquisition and arming succeeded. The8us measurement repeated,
but the requested30s campaign did NOT complete. Third attempt cancelled.

CycleTiming12 step3:24284572->24287426=2854us<2858. Reference previous/refused
cycles5993/5702halfusticks,pairedmean2923.75us,guardminusreference3us.
Prior recorder29us afterguard. No physicalrotor/ISRcause established.
SHA256 `E29800C8D7ED97A5DC5EA9AC1C0A75792E99FAC2A0D8DCD2A4F61E599AB602BB`.
Finaloff/portclosed verified, processexit1 retained. Candidate cohort1PASS/
1FAIL, not qualified3/3. No excluded attempts. Current failure was sustained,
not arm-cost refusal; this alone neither implicates nor exonerates the patch.

Next frozen7F4B matched72reference check after safing/flash/preflights, without
changing guards. Retain candidate ELF/evidence before any rebuild. Baseline
3/3 was finite evidence, not guaranteed absence of rare events. No threshold
change or hardware diagnosis follows from this comparison alone.

## E446 — frozen reference restoration passes

Verifiedoff then flashed hash-verified frozen7F4B ELF, not the current7C37
working-tree build. Bootguard01PASS3faults18refusals withoutclockrepair;
allfivepreflightsPASSCPU2/7/10. No change to guards/PSU/duty/fixture options.

timelineref_start61_reentry72_30s_01 passed original30s startup/loss/recovery,
27.990859s resumed336.307230eHz,56481COM=accepted,sigma23.907383us,
IRQunion53.172440%,raw307,bus10937mV,stack2676. Seed1012/acq6668us,
age85us/remaining41.5us/spare9.5us/arm13us. SEEDLAT88/106/120/152/156.
Finaloff/portclosed/process0. SHA256
`047A77083ABE9FBFE768B6427529359FD475AC2C62E89B10857F9911CD370A25`.

Reference timing returns to85/13 versus candidate86/8 in both candidate
attempts. This supports the measured build-dependent arm-cost difference,
not a sustained reliability improvement. One restoredreference pass cannot
convict the patch for an intermittent failure. Candidate remains1PASS/1FAIL;
oldreference3/3 and this one newpass stay separate cohorts, not rewritten odds.

Actual installed7F4B/off. Working-tree source/rootELF stillcandidate7C37;
do not confuse a rebuild with the installed reference. Next inspect emitted
steady-path differences/call layout before deciding whether to retain the
arithmetic patch or construct an isolated variant. Do not chase qualification
by repeating untilpass or widening the cycle floor. Current calibration and
archiveparity remain separate unfinished goal requirements.

## E447 — frozen-binary steady-handler comparison

Read-only nm -S -C and objdump -d -C comparisons of both frozenELFs, not
source assumptions. No hardware operation or runtime edit.

| Symbol | Both addresses | Both byte sizes | Observed differences |
| --- | --- | --- | --- |
|ADC_COMP|080016a4|086c|5 direct-call displacements only|
|DMA1_CHANNEL1|08001f10|02f8|2 trip-call displacements only|
|TIM16|08002208|0430|2 direct calls,1 relocated literal pointer|
|TIM6_DAC_LPTIM1|080027e0|0294|6 direct-call displacements only|

Targets retain their demangled names: driven_run::comp_irq/tick,
core_bench::live_stop/observation_elapsed, powered_timer::trip,
powered_guard::RunGuard::poll, pwm_sine, gates_off and __aeabi_uidiv.
This compares the listed handler bodies; it is NOT a recursive whole-callgraph
equivalence proof, and the surviving TIM16 helper is not newly introduced.

TIM16's literal at08002624 moves0801c66c->0801c6a4. Read-only .rodata dumps
show matching source-location metadata (length41,line330,column9), whose
filename pointer moves08019fe3->0801a01b. Both target strings are the same
E:/m/robot/esc/rm32/minz/core/src/am32.rs path (backslashes inbinary).
Do not misclassify this as a new motor-control constant or peripheral address.

powered_timer::commit stays312bytes, moving0800ac28->0800abb8; after address
normalization its only difference is the branch displacement to the same
phase_role_live::apply symbol. There is no newly inserted body instruction
in these inspected interrupt/commit routines. Relocated callees, flash-layout
effects, startup phase/state differences and rare-event statistics remain
outside this narrow result. No ISRlatency or physicalrotor proof is inferred.

Next isolate the constructor's call/stack/layout effects or collect a
predeclared matchedcomparison; do not label the failedcandidate a proven
steady-ISR regression. Keep the arithmetic improvement experimental until
its campaign qualification is resolved. Actual7F4B/off remains installed;
candidate7C37 source/rootELF and frozenarchive retained unchanged.

## E448 — inline constructor variant built, not installed

Only runtime-source change from7C37 is inline(always) on Timeline::new.
The exact radix arithmetic, metadata, window validation and all motor guards
are unchanged. This removes the outlined constructor call/frame as a variable;
it does not isolate all linker-layout effects or fix the sustained outlier.

Candidate SHA256
`ACEEC96C19C90E1979E1A641FAF27F3D0FE40787F70F473D3AE3E45F560DF111`.
Release opt-s/thinLTO:text124044/data1104/bss29336 (+152text vs7C37).
Automatic hashed audit matches. No Timeline::new symbol remains; the two
coast_run_inner timeline division calls remain eliminated (two other divide
calls survive). Four mainIRQ addresses/sizes unchanged.195library+34other
Rust replay tests PASS; existing nonfatal compiler/incremental warnings remain.

370Python tests alsoPASS. The subsequent objdump|Select-Object preview
returned pipelineexit1 after early pipeclosure; it was not a testfailure.
Emitted coast_run_inner local frame is204bytes plus20byte savedregisters;
removal of a separate constructor frame is not total-stack proof.

No motor/flash activity. Actual7F4B/off still installed; root/source nowACEE.
7C37 frozenarchive remains intact with1PASS/1FAIL. Next disabledguard/five
preflights onACEE, then one matched72recovery ifPASS. Compare85/13 reference,
86/8 outlined and newage/armcost/stack, keeping profile350, current/bus/age/
originaldeadline guards. Stop on anyfailure; no73retry as part of this probe.
Any eventual latency win is separate from sustained reliability qualification.

## E449 — inline first hardware campaign passes

InstalledACEE after UARTverifiedoff. guard01PASS3faults18refusals without
clockrepair; fivepreflightsPASSCPU2/7/10. Matched72original30s startup/loss/
recoveryPASS,27.990637s resumed336.409079eHz,56498COM/56497accepted,
sigma23.717699us,IRQunion53.015421%,raw311bus10853mV,stack2676,
DMA24us/queue2,COMP40/COM45/commit21us maxima. Startupraw596bus11522.
Seed1000/acq6637us,age85us/remaining40us/spare8us/armcost7us.
SEEDLAT88/106/120/150/154. Finaloff/portclosed/process0. CaptureSHA256
`8C1F073CD2ECF6CE44ABE29DB6B9EA019948254B347378B64FD4798EAB071B69`.

Reference85/13, outlined86/8, inline85/7: one observed inline comparison
reduces armcost6us and restores stack2676 vs outlined2644. No seed-age gain
overreference, no higher-profile authority, no sustainedjitterfix established.
The outlinedfailure remainsfailed. ActualACEE installed/off; next exactlytwo
same72recovery repeats failfast before moreduty/otherchanges.

## E450 — inline fixed cohort2PASS/1FAIL

02PASS original30s,27.991473s resumed336.361352eHz,56492COM/56491accepted,
sigma23.818030us,IRQ53.112652%,raw324bus10853,stack2676,DMA25queue2.
Seed1007/acq6190us/age85us/remaining41us/spare9us/arm7us.
SHA256 `B34245B3FCBACFDD3BD063F1B49061AD5079EE2EE807C0F9B87A67D045B97D2F`.

03FAILED24.757799s recovered336.505938eHz,49987COM/49986accepted,
sigma23.909865us,raw290bus10972,stack2676,DMA24queue2. Seed1007/acq6190us/
age85us/remaining41us/spare9us/arm7us: successful arm, sustained failure.
CycleTiming12step3:24754919->24757761=2842us<2858. Reference6133/5678ticks,
pairedmean2952.75us,guardminusreference3us; priorrecorder29usafterguard.
SHA256 `6325246499E0CC3C9AA78F2E7DC31B464CCE1B8AEB08B1C161B74522C65C272A`.

Bothfinaloff/closed, processes0/1, noexcludedattempts or unchangedretry.
Fixedinlinecohort2PASS/1FAIL; allthree85us/7us confirms measuredarmcost benefit
at this point, not reliability or WCET. Removing the outlinedcall/frame did
not eliminate the sustainedfailure class. Neither patchcausality nor hardware
cause is established. Do not repeat these constructor variants to chase3/3.

Next investigate recurring accepted-cycle redistribution atstep3 using the
existing tail and reference sequence, with an explicit discriminating test
before another poweredcampaign. Keep guards unchanged; separate diagnostic
armcost optimization from sustained timing. ActualACEE installed/off.
