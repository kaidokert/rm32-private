#350-profile experiment — E435

This is explicit speed-envelope expansion under the operator goal, NOT a fix
for E434's sustained timing excursion. Earlier profiles/captures keep their
original results. Preceding7.0% has a3/3 cohort on14FE; FAA0 passed7.0 recovery
and7.1 hold, then failed7.1 sustained recovery after successful rearm. Raw300/
bus11032 stayed within guards. Neither hardware failure nor physicaloverspeed
nor CPU saturation was established by that refusal.

Opt-in bench-range350 inherits345 and selects running2858/238us (ceil1e6/350,
floor1e6/(12*350)), acquisition5716/476 half-us ticks, seed952ticks. Twelve
ordered intervals/seven repeated-sector checks remain. Uniform952 gaps fail
5712<5716;953 can qualify. Referencewait238ticks=119us at minimumseed. Age174
ticks(87us) leaves64ticks(32us) and passes;175 refuses, including wrap.
Observed85us would leave2us spare. This is NOT WCET or assured recovery.

Retain32us livearm floor,16us arm-cost bound, originaldeadline/one-recovery cap,
current/bus/nFAULT/feedback/tracking/IRQ guards,800mA PSU setting and hard30%
duty ceiling. A refused seed is a retained failed attempt, never refreshed or
retried around admission. This experiment initially tests only7.0 and7.1%.

Candidate SHA256:
`7F4B25F6022C9A15DF004F59FD8CAC6394C9A3E3B208A5925A1ED099D903F9D9`.
Release opt-s/thinLTO:text123836/data1104/bss29336. Automatic hashedmathaudit
matches; advisory findings are not division-free/WCET proof.364Python tests
PASS,350 replay193library+34otherPASS,previous345library192PASS. Boundarytests
cover half-us refusal/fullsequence/cycle checks; parser mutations require
matched profile/acquisition without manufacturing hardware evidence.

No hardware action inE435. ActualFAA0/offE434 remains installed. Next verifyoff,
flash7F4B, disabledguard/fivepreflights CPUmax2/7/10. Then one61startup/70BEMF
original30s injected-loss recovery. Require newRUNLIMIT2858/238 and recovery
cycle5716/476, originaldeadline,fresharm>=32/cost<=16, electrical/tracking
checks, validarchives, stack>=512 and finaloff. Only afterpass one71recovery30s
under the same gates (precedingFAA0's71hold already passed; no unchangedretry).
Stop on failure. No higher duty or further profile is justified by this plan.
Independent current calibration/rotor timing/archiveparity remain incomplete.

## E436 — installed; first7.1 recovery campaign passes

Installed7F4B after UARTverifiedoff. Bootguard01PASS3faults18refusals WITHOUT
clockrepair. This does not prove the intermittent boot defect fixed. Five
preflightsPASS,CPUmax2/7/10us. Both following campaigns used61startup,trace0,
original30s deadline and one injected tracking-loss recovery, new2858/238
and5716/476 profile confirmed. Both finaloff/UARTclosed.

70recovery01:PASS27.991137s resumed325.721563eHz,54704COM/54703accepted,
sigma23.711963us,IRQ52.920200%,raw299bus10972,seed1032age85us,
armspare12us/cost13,stack2676.
SHA `A023A2F02959D73323428D3E09FC80758F0539E600D82ED6DAF09EC9E53EC08E`.

71recovery01:PASS27.991537s resumed332.200739eHz,55793COM/55792accepted,
sigma22.968252us,IRQ52.961228%,raw312bus10901,seed1007/acq6224us/age85us,
remaining41us,armspare9us/cost13,stack2676,DMAmax24us/queue2.
SHA `7A97EB360DE023E01F301D930B597FB8FE973F075F56BE73CF15DB7C3755E093`.

Startup rawpeaks615/760 remain pulse counts, not calibrated amperes. No PSU
setting changed. IRQunion not totalCPU/idle or provenfullthrottle. Both pass
originaldeadline, acquisition, archive/timeline and finaloff; zeroexcluded
motor attempts in this two-test sequence. New retained-capture test passes.
This is ONE71recovery success, not a reliability cohort or jitterfix. The old
FAA0/34571failure remains failed. Next exactlytwo same71recovery campaigns,
fail-fast, before moreduty. Actual7F4B installed/stopped, gates/MOE/CCRs/en0.

## E437 — fixed repeats complete: 3/3 at7.1%

No firmware, limits or PSU setting changed. Process check found no bench
fixture/probe active (unrelated COM8 terminal and PDF service untouched).
Exactly the two declared repeats ran; both fixture verifications passed and
processes exited0 with final outputs-off. No excluded attempts.

02: resumed27.990485s,332.233159eHz,55796COM/55796accepted,
sigma22.893051us,IRQunion53.154962%,raw315,bus10889mV,stack2676.
Seed1024/acquisition6781us/age85us/remaining43us/spare11us/armcost13us.
COMPmax40, COM54, commit21, DMA24us/queue2; startupraw595.
SHA256 `E5456D33BCD5177621CCD3680588A4D5B972CE456470EA09D60EE6B6B1AFB77F`.

03: resumed27.991037s,332.197488eHz,55792COM/55791accepted,
sigma22.946254us,IRQunion52.933035%,raw308,bus10901mV,stack2676.
Seed1008/acquisition6228us/age85us/remaining41us/spare9us/armcost13us.
COMPmax40, COM45, commit21, DMA37us/queue2; startupraw711.
SHA256 `B6FD1F93BFF9F979AE9929FBC14E4EC0F9EC6E20288A4019E44F996A5C7CF0F9`.

The declared71 cohort is3/3 original30s startup/loss/recovery completions,
not proof of rare-failure absence, physical rotor timing, calibrated amperes,
or a jitter fix. The differing ISR wall maxima are not exclusive costs or
causal evidence. Earlier345 failure retains its original result. Frozen
hash-verified ELF: captures/reference/range350_7f4b/shell-pwm.elf.
Next assess a same-profile next-duty trial; do not automatically increase
speed guards. Current calibration and archive-grounded parity remain open.

## E438 plan — one same-profile7.2% hold

Preceding7.1%3/3 cohort has ~332.2eHz, rawpeak<=315 during recovery,
bus>=10889mV, ordered accepted events/COM agreement, sigma~23us and live
arm margin>=9us. A0.1 percentage-point step suggests ~338eHz from the recent
local duty/speed slope, NOT a prediction guaranteed by motor physics. This
fits the existing350 experimental profile nominally; excursions may refuse.
Run exactly one10s hold at72 with61 startup, same7F4B/trace0/features and
all electrical/tracking/timing guards unchanged. No recovery or speed-limit
change in this attempt. Stop on refusal, retain it, verifyoff. The present
current evidence is raw pulse counts, not calibrated amperes. A passing
hold would justify considering recovery, not declaring a qualified point.

E438 hold passed10.000035s,20174COM/20173accepted,336.229eHz whole-window
mean including acceleration, raw306bus11068, stack2676, finaloff. Startupraw
1023 is below1200 but closer than prior cohort; not calibrated current.
Proceed one original30s injected-loss recovery72 on unchanged profile.
Acquisition and livearm must independently pass; no retry if either refuses.

## E438 result — first7.2% recovery passes, same firmware/guards

Hold capture SHA256
`11A989B3582360FA9EBF228E2628564C9DBD96858841CBF37FC0F5524EC51675`.
Its sigma43.047us includes acceleration; IRQunion53.605%, not totalCPU.

Recovery01 passed original30s deadline,27.991286s resumed at336.450905eHz,
56506COM/56506accepted,sigma23.754329us,IRQunion53.195290%,raw310,
bus10805mV,stack2676. Seed996/acquisition6191us/edgeage85us/remaining39.5us,
armspare7.5us/cost13. COMPmax40,COM45,commit21,DMA24us/queue2.
Startupraw902, bus11557. Foreground completed deadline (POWERPATHreason0,
active0/disabled1); fixture independently verified originaldeadline and
outputs-off, so no timer-fault code is required for this normal completion.
SHA256 `A215BA54B0566DB0EE3F31C587A6B7A02EF91E1AB31583555DF1816A18ABF14C`.

Both processes exited0, portsclosed, finaloff. No excluded attempts. No
firmware/guard/PSU setting changed. One72recovery pass is not a cohort;
next exactlytwo same72 original30s recoveries, failfast beforehigherduty.
Current calibration, independent rotor evidence and archive parity remain
incomplete. Existing71 cohort remains the highest completed3/3 point.

## E439 — fixed7.2% repeats pass: cohort3/3

Same7F4B, same350 limits and PSU setting. Exactly02/03 as declared, both
processes exited0 with originaldeadline/recovery/archive/finaloff verified.
No excluded attempts, no port contention, no firmware write or reset.

02: resumed27.991504s,336.539365eHz,56522COM/56521accepted,
sigma23.770716us,IRQunion53.022181%,raw309,bus10925mV,stack2676.
Seed1007/acquisition6190us/age85us/remaining41us/spare9us/armcost13us.
COMP40/COM45/commit21/DMA25us maxima, queue2; startupraw465,bus11486.
SHA256 `6945B99BAA64DE32091A435A057C5ED315E8908BFB67336138994B44C4FA516B`.

03: resumed27.990773s,336.403093eHz,56497COM/56497accepted,
sigma23.769016us,IRQunion53.039415%,raw326,bus10889mV,stack2676.
Seed1000/acquisition6595us/age85us/remaining40us/spare8us/armcost13us.
COMP40/COM45/commit21/DMA24us maxima, queue2; startupraw622,bus11557.
SHA256 `6FB0C2F1C00FB8AF778D01CB71C9128A4C1C30FC3F1935AEC85573984750771A`.

Together with01,72 now has3/3 declared original30s startup/loss/recovery
campaigns at336.40..336.54eHz. Minimum measuredarmspare7.5us is not WCET.
Restore ELF already frozen; archiveREADME updated. No jitter-fix, calibrated
current or independentrotor claim. Next assess one73hold under unchanged350
profile; higherduty not yet authorized by an experiment plan or tested here.

## E440 plan — one7.3% hold, unchanged350 profile

Preceding72cohort3/3 demonstrates336.40..336.54eHz, raw<=326 during recovery,
bus>=10805mV, orderedaccepted/COM agreement, sigma~23.77us, armspare>=7.5us.
Recent local slope suggests ~341eHz for73, only an estimate. Existing2858us
cycle floor remains active; an excursion can refuse even below350 mean.
Run one10s hold,61startup/73BEMF, same7F4B/trace0/features, no firmware/PSU/
guard changes. Stop on refusal, retain capture and verifyoff; no retry.
Only if this passes consider one30s injected-loss recovery on the same limits.
Rawcurrent evidence is not calibrated amperes; no broad safety claim inferred.

## E440 result —7.3% stopped at the existing cycle floor

One attempt, fixtureexit1 as expected for a failed requested window. Stop at
197217us, CycleTiming12 step3:194339->197183=2844us<2858.387COM/386accepted,
raw268,bus11199mV,stack2676. Finaloff verified, portclosed; no retry/recovery.
SHA256 `7AEAB914EE90959EC0574054421A6ACBC5B558840B00338469D0C364C2A4C437`.

Wholewindow327.043eHz/sigma240.320us includes acceleration and is not a
steady327 failure. Retainedtail181700..196697us has26 overlapping cycles,
median2917us (~342.818eHz),min2862/max2999;322events omitted between windows.
Rejected2844us is ~2.5% shorter than localmedian. Reference preceding/refused
cycles5993/5681 half-us ticks, pairedmean2918.5us. Guardminusreference3.5us;
prior same-sector recorder is26us afterguard. These are accepted-event timing
observations, not physicalrotor or ISR-latency measurements. No cause proved.

Preceding72cohort remains3/3;73unqualified. Next review the local343 regime
and fresh-arm budget before any new profile, not an automatic guard increase.
No hardware/PSU/firmware changes. Capture tests preserve this refusal.
