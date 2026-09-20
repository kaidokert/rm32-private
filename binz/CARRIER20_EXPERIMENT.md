# Carrier-frequency discriminator — E480

Status: pure timer geometry only; no live selection, flash or motor run.
Installed F22D remains the failed critical/sparse diagnostic, outputs OFF.

E479 finds roughly100us long/short interval redistribution at the preceding
same-sector boundary with no >40us dispatch there. Existing trace1 changes
the comparator adapter; completed-decision recorder cost about70% IRQ union
in E414 and its32calls cover only a few accepted events, not reliably the
preceding six. Do not revive that recorder unchanged or call short trace
coverage proof of the missing earlier sequence.

Next causal intervention: BEMF carrier24.006kHz versus20kHz, keeping reference
sequence, safety limits, startup and acquisition unchanged. This changes PWM
phase-dependent qualification opportunities without new per-call recording.
It also changes deadtime fraction, ADC trigger-phase coverage and workload;
a pass would NOT uniquely prove a PWM-qualification cause.

Added pure Carrier::Khz20:3200ticks/ARR3199 at64MHz, exact20kHz.32-bin mapping
is phase/100 implemented as const-derived10486 multiplier then shift20.
Exhaustive phase tests against division, all duty1..100 checks, timer-continuity
tests and208library+34other replay tests PASS. No hot runtime divide was added
to phase_bin; emitted target code still needs inspection once integrated.

At nominal6.8% compare217ticks minus26deadtime gives ideal191/3200=5.96875%
versus24k compare181 minus26 gives155/2666=5.81395%. These are MCU timer
geometry, not measured MOSFET conduction or calibrated electrical power.
Thus start the candidate at the preceding6.8% operating point, not7.2%.
Keep initial open-loop6.1%/200eHz handoff/phase60 and normal guards. One finite
10s hold first, then original30s injected-loss recovery only if preceding
current/bus/tracking/deadline/stack evidence passes; no speed/guard widening.

Before power: integrate explicit feature selection, retain existing10k startup
and24k behavior; derive ADC allowed period/bin mapping from selected geometry;
extend strict host marker/period checks and configurable disabled rolecheck.
Use same lean profile on both sides (remove comp-critical and sparse to avoid
their observer workload), freeze exact binaries, release-s/thinLTO audit, verify
disabled guard/role/ADC-phase/CPU/archive checks. Current pure enum addition
does not satisfy any of those hardware prerequisites. Quantify outputs and
failures, not only whether the motor ran. Older24k captures are context, not a
matched-build A/B baseline; obtain one at6.8 on the resulting lean profile.

## E481 — integration and both lean release builds, not yet flashed

bench-pwm-20k explicitly overrides CARRIER toKhz20 and depends on existing
bench-pwm-24k preparation/restoration machinery (historical feature name).
Without override existing24k selection remains; without either startup/default
10k remains. ADC phase DMA accepts startup6400 or the selected CARRIER ticks,
not a blanket acceptance of another powered period.3200 maps toKhz20 bins.
Host carrier/role/ADC decoders now validate20000/3200 and reject24k mismatches.
Full disabled preflight supports explicit --period-ticks3200 (default2666).
396Python tests PASS including20k period/marker mismatch and startup rejection.

Lean candidate (no sparse/critical/decision features):
9ACCA8CEBF9A975D50E28F75D4312CEC99B1D37A09A17235E36E34BF34097BCA
frozen captures/reference/carrier20_9acc/shell-pwm.elf, hashverified.
Matching24k profile:
6D026D5659AEC6746C14454509C0FCD880CCAF0BC09478AB14A231CB0E6B1B47
frozen captures/reference/carrier24_6d02/shell-pwm.elf, hashverified; rootELF.
Bothreleaseopt-s/thinLTO buildsPASS.20k autoaudit hashmatches. Emitted
adc_phase_dma::consume08009fb4..a070 uses constliteral10486(0x28f6), MULS
thenLSR20 for3200; startup uses5243. No division helper on that bin path;
remaining bounds panic retained. This is codegen evidence, not measured cost.

No flash/hardware action in E481; actualF22D remainsinstalled/OFF. Next archive
actualdiagnostic if needed, install frozen6D02 afteroff, disabledguard/fullfive
preflight with2666 plus ADCphasecheck, then baseline68/10s. Candidate9ACC needs
its own3200 hardwareprerequisites before candidate68/10s. Never borrow a24k
hardware pass as20k qualification. Run plan andconfounders above unchanged.

## E482 — both matched 6.8% ten-second holds pass

Installed frozen6D02 then9ACC, each after outputs-off and serializedOpenOCD
reset. Each exactimage passes guard3faults/18refusals, fullfivepreflight with
its period(2666/3200), CPU2/6/9us and three ADCphase DMAroute checks. Captures
carrier24_482_* andcarrier20_482_* retain allchecks. No UARTrepair required.
ADCroutecheck is a disabled selfcounter test, not powered aperture proof.

Both start61/phase60/200eHzhandoff/trace0/68BEMFduty holds complete10seconds;
all current/bus/tracking/deadline guards unchanged. Oneattempt percarrier,
not repeatability qualification. PSUlastknown11.7V/800mA unchangedassumption,
no newexternalcurrent measurement. Bothfinaloffverified/serialclosed.

| Metric |24.006kHz6D02|20kHz9ACC|
|---|---:|---:|
| Duration s |10.000033|10.000010|
| Aggregate accepted eHz |315.428360|330.282205|
| Tail median accepted cycle us |3174|3026|
| Aggregate cycle sigma us |39.175452|38.948143|
| COM/accepted |18926/18925|19817/19816|
| IRQ union percent |52.723863|50.426010|
| COMP calls/s |20514.832|17915.182|
| COMP/COM/commit max us |40/45/21|40/45/21|
| Peak phase deviation raw |294|314|
| Minimum measured bus mV |10984|11068|
| Untouched stack bytes |3460|3460|

Aggregate sigma includes startup acceleration; not a matched-speed steady
jitter verdict. IRQunion isn't total CPUutilization.20k speeds up at equal
nominalduty and reduces COMPcall workload, but deadtime and waveform changes
remain confounded. No rare-fault improvement established by these two holds.

ADC49751samples each; metadata2666/3200 andCRC/bins verified.24k bins1528..1569;
20k995..1991.20k's201us trigger advances1us per50us carrier, yielding50 nominal
phase locations over32bins, so equal histogram bins aren't expected. Trigger
histogram isn't ADCaperture/sector/current-unbiasedness proof; retainthis
sampling-grid confounder rather than claiming phaseuniformity/calibration.

Capture hashes:
carrier24_482_start61_hold68_10s_01.txt
0A89BA5E8BDC08A0BAC1C527FF3A30BDF3BB1B06A357D1D94761FB11D9E3AB1D
carrier20_482_start61_hold68_10s_01.txt
ADEF9AD98A8BB7461892CB6DF39C6E3D9F6755C7490B120BC6C0B19AF1DA239D.

Actual9ACC installed/OFF; rootELF6D02 unchanged. Next same68original30s
injected-loss/reentry comparison with per-build off/preflight requirements
on swaps; no72jump yet. Derive steady resumed metrics to avoid startup sigma
confounding. Existing350 admission and current/voltage limits remain intact.

## E483 — both original-deadline recovery comparisons pass

Installed9ACC20k fromE482 ran one68/30s dropout/reentry. Then restored frozen
6D0224k after verifiedoff, OpenOCDreset and freshguard/fullfive2666/ADCroute
checks (CPU2/6/9), and ran the matched command. Bothoriginal30s campaigns
complete with verifiedTracking-loss shutdown, freshseed reentry, independent
archives and finaloff. Oneattempt each, no retries/guard changes. UARTclosed.

| Resumed metric |20k9ACC|24k6D02|
|---|---:|---:|
| Duration s |27.990839|27.990765|
| Accepted eHz |330.757175|315.751387|
| Cycle sigma us |21.996249|23.632021|
| COM/accepted |55549/55549|53029/53028|
| IRQ union percent |50.655775|52.713293|
| COMP calls/s |18045.726|20386.974|
| COMP/COM/commit max us |40/45/21|40/49/21|
| Phase deviation raw peak |352|265|
| Bus minimum mV |10925|10925|
| Seed ticks / acquisition us |1024/6786|1073/6714|
| Arm spare above32us floor |11us|17us|
| Untouched stack bytes |2668|2668|

ADC phase CRC/periods3200/2666 independently verified. No external PSUcurrent
reading acquired; rawpeaks are not calibrated amps. Comparison is equal-duty,
not equal-speed; cannot isolate filtering/CPU/deadtime causal contributions.
Reduced IRQ union repeats the10s observation, but one recovery each cannot
prove a rare-outlier fix or broadly reliable entry. Hardware/current/portability
and archiveparity goals remain incomplete.

Raw captures andSHA256:
carrier20_483_start61_reentry68_30s_01.txt
229EE175D6C3AADB1056F6BD54A8806ECCF9AC767D06CA8440C98288DDF439AB
carrier24_483_start61_reentry68_30s_01.txt
6825FBA665EBA361896C266A630FBFF9C0E23F57A3AEAF4FF30E5440DF65491C.

Actual/root6D02 installed/OFF. Next restore20k9ACC with exactdisabledchecks,
then exactlytwo further68/30s recovery attempts, failfast and alloutcomes
retained, to make a fixedthree-attempt20k cohort before any69duty step.
Do not claim stockAM32/hardwarebaseline or calibrated current parity.

## E484 — fixed20k68 recovery cohort3/3; first69hold passes

Restored frozen9ACC afteroff/OpenOCDreset. Freshguard3/18,fullfive3200preflight
CPU2/6/9,andthreeADCroutechecksPASS. Exactlytwo further68/30s recovery attempts
bothPASS, completing planned3/3 withE483. No excluded failures/retries. All
originaldeadlines, archivedfirstsegments, freshseed reentry andfinaloff validate.

| Resumed metric |Attempt02|Attempt03|
|---|---:|---:|
| Duration s |27.991039|27.990647|
| Accepted eHz |330.174111|328.953847|
| Cycle sigma us |22.409870|22.867293|
| IRQ union percent |50.395337|50.234770|
| COM/accepted |55452/55451|55246/55245|
| Raw phase peak / bus minimum mV |301/11032|310/11056|
| Recovery arm spare us |10.5|9.5|

BothCOMP/COM/commitmax40/45/21us,stackuntouched2668. ADC3200metadata/CRC
validated. Cohort includingE483 covers328.954..330.757eHz,not arbitraryspeed
or rare-fault absence. No freshexternalPSUcurrent reading/calibratedamps.

Aftercohortpassed, one69duty10s hold on SAMEimage alsoPASS10.000032s:
335.061084eHz aggregate,20104COM/20103accepted,raw283,bus10984mV,
IRQ50.656440%,COMP40/COM59/commit21us. Sigma40.189914us includes startup;
not a sustainedjitter increase claim. Stack2668 is cumulative paint since
priorrecovery, not thishold's isolated footprint. ADC3200verified,finaloff.
Current/tracking/cycle2858/event238/age/deadlineguards unchanged.

Captures andhashes:
carrier20_484_start61_reentry68_30s_02.txt
BD45F8C6E68518FABAD76929399FF9D23AF740DDBB4BB705128F4C8B6D0913EA
carrier20_484_start61_reentry68_30s_03.txt
4B2C0F7DBB76EF2A2E01830B750D482951F4344B37827D7FD3607B8F7C1734AB
carrier20_484_start61_hold69_10s_01.txt
464D4BA0E6B54FF9DDF0B552CD50866E03E91BF62725E1D5190C6C37DED3330A.

Actual9ACC/OFF,UARTclosed;root6D02still24k. Next one69/30s recovery with
unchangedlimits, not70yet. A failure is retained and investigated, not retried
untilpass. This advances20k envelope, not completionofcalibration/paritygoal.

## E485 — 6.9% original-deadline recovery3/3

Sameinstalled9ACC, no rebuild/reset/guard changes. First69/30s recoveryPASS,
then predeclaredtwoadditional attempts bothPASS. Fixed3/3 startup-to-recovery
completion; no excluded attempts. Each injects oneTrackingloss, uses a fresh
seed, preserves original30sdeadline/firstarchive, then verifiesfinaloff.

| Resumed metric |01|02|03|
|---|---:|---:|---:|
| Duration s |27.990842|27.991239|27.991073|
| Accepted eHz |334.187241|333.652412|333.866517|
| Cycle sigma us |22.299669|22.641151|22.708069|
| COM/accepted |56125/56125|56036/56036|56072/56071|
| IRQ union percent |50.497890|50.879442|50.554131|
| Phase raw peak |312|311|314|
| Bus minimum mV |10925|10889|10937|
| Seed ticks / acquisition us |1014/6683|1010/6270|1010/6274|
| Arm spare above32us |10|9.5|9.5|

AllCOMP/COM/commitmax40/45/21us,stackuntouched2668,ADC3200metadata/CRCvalid.
Independent outputs-off and recoverytimelines verified; UARTclosed. Requested
nonblocking PSUdisplay current/voltage during02; no reply received byentrytime.
Do not substitute old70mA or turnrawcounts intoamps. No calibrationclaim.

Captures carrier20_485_start61_reentry69_30s_01/02/03.txt hashesrespectively:
132467B3341829510830C4CC47CAC949C68DEF5A6CD37810352A915CD391A327
4E1ED908FA5CDCFACB2B6AEE560779AD4D002EA59A7457709812FCA604A320FC
4774DF8A940C351E92FD29DD3548EE4C21786CCD56D89860D2FFA20E61C54AD6.

Actual9ACC/OFF;root6D02unchanged. Nextone70/10shold withsame350profile,
then recoveryonlyifprecedingevidencepasses. No thresholdraise orclaim20k
removedrareoutliers. Preserve3/3at68 and69 as regime-specific evidence.

## E486 — first7.0% hold fails near the old24k speed boundary

One70/10s attempted onunchanged9ACC20k after69cohort3/3. FAILED onCycleTiming12
at203236us; no recovery/retry/higherduty.400COM/399accepted,rawpeak297,
busmin11354mV,stackuntouched2668. Guardstep2 previous200354->203198 gives
2844us below2858floor. Refprevious3003/current2841.5us; localtailmedian2926us
(341.7635eHz). Pair+77/-84.5us relative median,meanresidual-3.75us;
guard/referenceclosure2.5us. COMPmax65/COM44/guard20/commit21/DMA22us,
queue2; maximaareunlocated andCOMP includes faultshutdown. ADC3200valid,
finaloutputs-offverified/UARTclosed. No newexternal currentanchor received.

Raw captures/carrier20_486_start61_hold70_10s_01.txt
SHA25688AE2A008B271DACD9467E942B1B79AED2C0D1E03CA0C86485A39BB7F224F3FC.

Offlinecomparison to frozenolder24k7F4B E44073duty failure:
range350_start61_hold73_10s_01.txt, hash
7AEAB914EE90959EC0574054421A6ACBC5B558840B00338469D0C364C2A4C437.
It failed197217us,guarddelta2844 also,step3; tailmedian2917us/342.818eHz;
ref2996.5/2840.5, pair+79.5/-76.5us,closure3.5us. Both are acceleration
windows, not steady-speed physicalrotor measurements, and builds differ.

This is evidence against treating20k as a demonstrated extension of the speed
envelope: it moves nominalduty-to-speed and saves some IRQwork, but the same
guarded short-cycle class recurs at a similar~342eHz local operating point.
Not a proven fundamental/hardwarelimit or a precise carrierquantization result.
Keep20k68/69cohorts as measured successes; do not turn them into a faultfix.

Actual9ACC/OFF;root6D02unchanged. Stopcarrier/duty ratcheting here. Next review
the accepted-cycle timing and fresh-seed arm budget at this localpoint against
archivedreference behavior; any expansion of350admission needs actual timing
headroom evidence, not simply changing2858 until the observedfaultpasses.
Current calibration/archiveparity remain open; no goalcompletion claimed.
