# Hardware comparator filtering candidate — E310

## E328 live raw evidence

Installed58E0080BEEF6758398AF33961A82E915E933A1F79BD7AD50F787CC732AF23417.
277Python/releasepass, rawlive_preflight01 allsevenPASS,noUARTrepair.
rawlive_hold62_01 fullcampaign/detailedrawverifiersPASS10s279.641eHz,
16779COM/16778accepted,raw268bus10937,arm59.5/cost13,stack3184,
IRQ59.698%,COMP76COM266commit45. RawSHA256
758B0408FC79845D2B527E1875F5B4CC84F1D0BCD794FE884580AB06056FDB02.
Filter16520/16778,258missing,0filteredovercapture. All8prefixmisses have
rawready/capture/overcapture1; latestagemod201=29/6/4/1/0/1/1/2us.
Prefix3 expectedraw0 but read1 afteracceptance. Multiple raw edges and
postacceptance reversal show softwareacceptance isnot a clean-edge oracle.
Near-zero modulo ages support recentedges but don't exclude201us multiples.
Don't aim for100%match by blindlyshorteningfilter; qualification differs.
Next timing-aware filteredIRQ integration design must preserve referencegate,
persistence, one-shotCOM and independentageguards; measure actuallock and
visits, notcoverage. COM266 recurred;86uspreviousrun didnotproveitfixed.
Finaloffverified. No filtered commutation authority yet.

## E328 raw timestamps attached to observer, predeclared gates

ADC start calls raw_started after channel/DMA configuration beforeCEN; it only
attaches in active observer epoch. Stop revokes ready; mux arms rawonlyafter
valid epoch ticket, reset snapshots/stops capture in same critical section.
FILTERRAW bounded first8misses reports ready/capture/over/CCR/CNT, period201us
explicit modulo-only age (not full edge age). Observer adds per-reset overhead.
Require extended fail-fast preflight --raw (seven disabled checks) before10s
62/62phase60trace0no-recoveryhold; fullcampaign and detailedrawverifiers, all
existing electrical/current/age/arm/cycle guards unchanged. No authorityswap.

## E327 real ADC coexistence diagnostic, predeclared gates

rawadccheck runs three128scan trials at ENABLElow/alloutputsdisabled using
actual adc_stream five-channel ADC/DMA and phaseDMA coherent poll path.
One raw comparator pulse after eachscan; require128captures, noADC/DMA fault,
TIM3PSC63ARR200CR1CENCR2MMS2DIERUDE unchanged, expectedscan timestamps,
elapsed25528..29999us, nonzeroVREF rawcode, finaloff. NoADC IRQ/motor load.
Private idle-probe entry permits this ENABLElow path only withbench-filter-raw;
existing baseline mode remains ENABLEhigh, all requireoutputsdisabled/noowner.

E327 result: installedF3CEBD1BA7F6B62159B7F7EEEA8B27CD259435B25CC386576E4826CC5C88767A.
rawadc_01 three128scan/128capturetrials pass:25662/25663/25662us,
VREFminimum1503 all, noADC/DMA faults, configpreserved andfinaloff. NoUART
repair, no motor run. 276Python tests/release pass. Now have disabledADC+
phaseDMA coexistence evidence; liveISRload/epoch hookup remains unqualified.

## E326 disabled raw capture + update-DMA coexistence gates

bench-filter-raw adds raw_capture configuration and optional adcphasecheck
extension; no live controller hookup yet. TIM3CH2 capture modifies only CCER,
CCMR1input, TISEL and CC2 status, not counter/timebase/trigger/DMA registers.
Run three existing32-update phaseDMAchecks, inserting one disabled comparator
pulse after each consumed DMAword. Require32captures each, noovercapture,
PSC63/ARR200/CR1CEN/CR2zero/DIERUDE unchanged, originalDMA maxcounter<=2us,
outputs off. No ADCload in this test; not yet liveADCcoexistence qualification.

E326 hardware: installed51F1B6E54E5189FD76D85C6AED0568A750D8E809ACB919A8D61D8F54B56D8CC0.
rawcoexist_01 PASSthree32-update trials,32rawcaptures each, configpreserved1,
DMAmaxcounter0us each. NoUARTrepair, finaloffverified, no motor run.
275Python tests include retainedraw mutations and mandatorycoexistence rows.
This qualifies disabled updateDMA coexistence, not actualADCload or lifecycle
under motorinterrupts. raw_capture helper is diagnostic-only until session
cancellation and ADCinitialization ordering are connected and tested.

## E325 independent TIM3 disabled probe, predeclared gates

adc_stream::start owns TIM3PSC63/ARR200, CR2MMS2(updateTRGO), adc_phase_dma
owns DIERUDE and DMAch3 request37. CH2 is not used there; start clears CCER,
so eventual sidecar must attach after that initialization, not before it.
Disabled filtercheck temporarily configures TIM3CH2 TI2SEL1/filter0 at1MHz/
201us, CR2zero/DIERzero, no ADCtrigger or DMA. Requires stoppedTIM3/DIER0,
saves/restores10 configuration registers including CNT/CCR2 and checks equality.
All six pulses must capture onTIM3 (including2us whileTIM2filter12/15 rejects),
no overcapture, raw timestamp0..3us after prewrite modulo201. ExistingTIM2
timing/restoration gates unchanged. This is independence, not liveADCcoexistence.

E325 result: installed6B8696F29E7B2082A4E2BFA7C836A042CF8D780F398ACF3439E5EFCA75150B0B.
filtert3_01UARTsilence retained; exactsafeRCC/PD1/BDTR/CCRs beforeclockrepair.
filtert3_02 PASS: TIM3 capturesall6pulses, including shortpulses thatTIM2
code12/15 rejects. TIM3raw latency1us each; TIM2filteredlong delay14.5us/
7.5us in this diagnostic. Noovercapture, allrestoration/finaloff gates pass.
SHA256F31B5143F3313EB8CC8FDFE67777D46A6497774834E540C9D2D65E1129C577A8.
274Python tests/releasepass. No motor run. Independent raw route established,
not ADCcoexistence or unambiguous rawage: still modulo201us when ADC runs.

## E324 indirect-capture independence test (disabled only)

Local PAC CC1S2 maps IC1 onTI2; RM0444 describes IC1F as filter onTI1,
so independent filtering through indirect capture is NOT assumed. IndexedST
RM0444 source https://www.st.com/resource/en/reference_manual/rm0444-stm32g0x1-advanced-armbased-32bit-mcus-stmicroelectronics.pdf
describes input filtering and muxing. Test adds CH1CC1S2/IC1F0 alongside
existing directCH2 atfilter0/15/12, same rising pulse. Require code0 both
capture, no overcapture and existing pulse/timing/restore/off gates. For
nonzerofilter, CH1 short-pulse presence vs absence and CCR delta characterize
independence; either result retained, never relabel bothfiltered asraw/filtered.
No motor command or live observer route change. CCR1 saved/restored too.

E324 result: installedB58B250AE8FAE45CC3ED0BD7F39AB4AAD8EE4F20B4A7CCFFD674890DB88E7A61.
filterpair01 UARTsilent retained; exactsafe registers beforeknownclockrepair.
filterpair02 PASS protocol/restore/finaloff. CH1andCH2 captures identical:
filter0short/long bothcapture, filter15/12short bothreject; every long CCR1
equalsCCR2 (80,252,704,1108 valid values). IC1F0 didNOT bypass TI2filter.
SHA25622EEB46ECC72D8D817DCDFF22DA20C3702F6896C3D8B1D4EB3D1EF93BAEFA7F2.
273Python tests pass. No motor run. Proposed same-timer raw/filtered pairing
is invalid and must not be connected to live observations under that label.

Alternative source-grounded route: RM0444Rev6section22.4.29 TIM3_TISEL
TI2SEL1=COMP2, confirmed indexedST text at the official manual URL above.
TIM3 already runs ADC cadence; its independent CH2 filter could capture raw
COMP without resetting counter or changing trigger/DMA. Need verify actual
PSC/ARR and all channel users, preserve CEN/DIER/CR2/SMCR, test disabled first.
If period201us retained, capture age is modulo201us and cannot be interpreted
as unambiguous age across whole~600uscommutation; record this limitation.

## E323 bounded miss instrumentation, predeclared comparison

Adds per-sector sample/capture totals plus first8missing records containing
step/raw-edge polarity, comparator level AFTER acceptance, interval CNT,
time since capture-arm in same counter epoch, and TIM1 PWM count. All recording
under existing observer critical section; no new controller decision. Prefix
is bounded but costs extra time, so full preflight and actual-arm/cycle guards
remain mandatory. Host requires six ordered sector rows summing to aggregate,
exact min(misses,8) prefix length and field ranges. Not live signal history.
After disabled preflight passes: one10s62/62phase60trace0 no-recovery hold,
full campaign verifier plus code12 detailedobserver verifier, finaloff.

E323 result: installed0F4629972F4B489B59A448E9898B69AD7CA68218258FCB0AC6C6B72A2DFFBC72.
Preflight01 UARTsilence retained/exactsafeclockrepair;02allfivePASS.
filtermiss_hold62_01 PASS10s280.304eHz,16819COM/16818accepted,raw286,
bus10793,arm46/cost13,IRQ59.716%,COMP75COM86commit45,stack3628.
Detailedverifier: eachsector2803samples; captures2732/2780/2746/2792/2721/2768.
279missing across all6sectors (71/23/57/11/82/35),16539captures,0overcapture.
First8misses raw-after equals expected in everycase; arm age646..831half-us
ticks=323..415.5us, well above8us filter delay. Rules out latearming in these
eight, not transient/qualification delay or all phase problems. Prefix is not
representative whole-run timing. RawSHA256
DBE259EDCB190145B441FA4ED51018E60DFEA4F66EC37B444C9EDD38A8ED91A2.
Finaloff verified. Next audit paired CH1 indirectTI2 unfiltered vsCH2 filtered
captures for rawedgeage at acceptance, no IRQ and no control authority.

## E322 shorter observer filter comparison, predeclared gates

bench-filter-short selects code12/CKD2 (ideal7..8us) only for observer,
software controller and safety gates unchanged. Disabled pulsecheck now adds
code12: 2us request must measure<6us and reject,40us capture timestamp must
be14..22half-us ticks from prewrite; all existing code0/15 gates unchanged.
271Python tests pass. Full fail-fast preflight required before10s62/62phase60
trace0 no-recovery hold. Full campaign verifier plus FILTERCONFIGcode12 and
FILTEROBS required. Compare availability/overcapture, not claim qzc/lock.

E322 result: installed125B3C24C5FF6B25941F55F5E195F2920D2E5C4D46EEFCE6A261D0DBBEBC7D28.
Preflight01 UARTsilent retained, exactsafe registers then known clockrepair;
02 fivechecks pass. Code12 pulse measured2us rejected;40us request bracket41,
capture delay16half-us ticks=8us, allcounter/restore/finaloff checks pass.
filtershort_hold62_01 PASS10s280.324eHz16820COM/16819accepted,raw241,
bus10853,arm62.5/cost14,COMP74COM86commit45us,IRQ59.059%,stack3892.
FILTERCONFIG12 verified. Observer16819samples/16499captures/0overcapture,
lag1..275ticks. Availability98.10% vsE321code15 88.81%;320stillmissing.
RawSHA2563E908BFF16FCF1373B43B7BF74216DB2528D7D38AF9AAF2E0D1FA3DA2453C557.
Supports sensitivity to filter duration, but neither first-edge correctness nor
qzc nor CPUgain; software controller still authoritative. Next localize residual
missing captures and distinguish filter-late events from mux/phase artifacts.

## E321 accounting-clock candidate, predeclared powered confirmation

Installed81572D65E294CEA12B11CDCA47564357820F212424C9426650EB99F1831BD2CB.
irq_union enter_clock/leave_clock validate state before lazy TIM17 sample,
same nesting/overflow/elapsed guards. Full replay tests and release build pass.
Preflight01 stopped at UARTsilence; exactsafe readbacks then known clockrepair.
Preflight02 all five disabled checks pass; nestedmean9.46875/max10us vs
prior10.0/max10 and failing10.125/max11. No threshold change or old-fault cure
claim. Powered confirmation limited10s62/62phase60trace0 no recovery;
full campaign verifier and FILTEROBS required, retain failures/finaloff.

E321 powered confirmation PASS filterclock_hold62_01, SHA256
77DA8F464C70356FD10A8D0BBCC7A88A45D4D13E720CFE23A73DE29588315A36.
10s280.137eHz16809COM/16808accepted, sigma39.047us, raw283bus10865mV,
arm62us/cost14, COMP74/COM86/commit45us, stack4004. IRQ59.551% vs
priorobserver59.318%: no aggregateCPUgain established. Clock sampling now
occurs after state validation so software measurement boundary shifts slightly;
do not call union change exclusiveCPUcost. Observer16808samples,14927captures,
5overcapture,lag0..684ticks; stopped/finaloff verified. About88.8% availability,
notqzc. Investigate missing captures/timing before any authority replacement.
COMmax lower this run doesnot establish cause/removal of prior265us outlier.

## E320 observer-off disabled comparison

Saved5D7C observerELF at captures/reference/filterobs_5d7c/shell-pwm.elf,
verified exacthash before building same source without bench-filter-observe.
BaselineA1E963685400C670FCBCEF4BEEF2E9D0A2AB9257BAAE091056E20E406BED38A9
now installed. cpu03/04 both nestedmean10/max10, same as observer repeats.
cpu01/02 UARTsilent retained; checked RCC08000000,PD1zero,BDTRc1a,CCRsall0,
then restored only known USARTclock08040000. No motor command.
Scope1/2 drop symbols remain08003200/080032ac in both images. Instruction
comparison isolates data-pointer relocation rather than a new observer call in
the accounting bracket. This does not explain first11us outlier or COMmax265.
Next higher-resolution measurement/accounting optimization, not threshold fit.

## E319 unchanged-image disabled repeats

filterobs_cpu02..05 all pass mode0mean1.625/max2, mode1mean6.5/max7,
mode2mean10.0/max10. Original01 mode2sum2592/256=10.125,max11 remains failed.
This is not a fix or reason to discard01. Source check establishes measured
bracket contains cpu_meter software nested enter/drop only, no filter observer
call. TIM17 quantizes to1us; quantization alone is not proven as cause of the
changed mean. Need controlled baseline/high-resolution measurement, not a
looser10us gate. No powered run this entry. Fail-fast preflight tests added.

## E318 result and qualification failure

Installed5D7C2355. Disabled pulse/atomic/roles/archive pass; CPU mode2
max11us exceeds unchanged10us gate. Operator-facing sequencing error disclosed:
powered hold was launched before acting on the failed CPU output (PowerShell
batch returned last command success). Do not qualify this build from that hold.
New drv_filter_preflight.py uses subprocess check=True, stops at first failure,
never commands motor power. No further powered run until regression resolved.

Exploratory filterobs_hold62_01 raw SHA256
D24AAE4E404EBA98F58B6B5EE06DE3702CBC0D5417F1A0D0CD4D583D68CFB945.
Full powered verifier passes10s at280.027eHz,16802COM/16801accepted, raw287,
bus11056mV, arm48.5us/cost14us, IRQ59.318%, COMP75/COM265/commit46us.
Observer16801samples equals accepted count,15063captures,2overcaptures,
lag1..263half-us ticks; stopped and finaloff verified. Capture availability
is89.66%, notqzc; latest edge can include mux artifacts. This does not qualify
using filtered captures as authority. COMmax265 vs baseline99 also needs
regime-matched diagnosis; wall maxima alone don't identify exclusive cost.

## E318 first powered observer attempt gates

Installed5D7C23555C3249924DE9D03E1957A73546AEF5E6B8FAAAF2F49B3F53951F014B.
Disabled pulse/atomic/role/CPU/archive checks before powered attempt.
One10s maximum first-handoff hold: acquisition62/BEMF62, phase60, trace0,
24kHz, no dropout/recovery. Require full existing campaign verifier including
actual arm, electrical, tracking, cycle and finaloff gates. Additionally validate
FILTEROBS stopped/noauthority/count/extrema schema; inspect samples vs accepted
events. Missing captures are diagnostic, not permission to alter controller.
Retain failure without increasing duty or loosening guards.

## E317 first-driven-handoff hooks compiled, not flashed

Separate bench-filter-observe enables live hooks (bench-capture-filter alone
still only supplies disabled diagnostics). Preparation now allows awake ENABLE
only with outputs disabled and driven/core/powered owners inactive, timer
stopped and DIER0. Called after validated driven transfer adoption, before
new guard starts output ownership. No output permission is granted by observer.
Mux changes use tickets; comp_input::stop revokes; Interval::set_count snapshots
before reset only when n0, while nonzero initial seed discards any capture.
Restart changes CR1 via OR1 to preserve CKD. Summary is fixture-only.
Host drv_filter_observe validates counts, extrema and stopped/no-authority
provenance, not qzc or electrical safety (full campaign verifier still required).
267 Python tests and release build pass. No flash/hardware action. Next disabled
regression then bounded first-handoff hold at62/62, trace0; no recovery attempt
with this observation yet. Measure overhead against unchanged arm/cycle guards.

## E316 lifecycle tested before hardware hooks

Observer now uses filter_epoch pure state under existing critical sections.
before_mux issues one-use generation ticket; after_mux requires it. Stop,
new preparation, counter reset and superseding mux revoke stale tickets.
Unarmed initial handoff reset is not counted as a capture opportunity.
Tests cover normal/reset, stop+restart, stale/reused ticket, superseding mux
and generation wrap. This is software ordering evidence, not concurrent
hardware qualification. Ticket lifetime must never span2^32 invalidations.
Release build and replay tests/M0 library check pass. Not wired or flashed.

## E315 observe-only scaffold, not integrated

filter_observe.rs now contains optional, compiled-only operations. Preparation
requires ENABLElow, no active owner, stopped TIM2 and DIER0. It changes only
CH2 capture configuration, TI2SEL and CKD, not PSC/ARR/CNT/SMCR or EGR.
Mux hooks disable capture then select raw edge polarity. Snapshot before
Interval::set_count reads latestCCR/overcapture and lag in the old counter
epoch, then disables capture until next mux. Stop revokes observer under
critical section. No capture IRQ/DMA, no comparator writes or output authority.

Not yet wired: core_bench::Interval::set_count is the reset boundary;
comp_input::Input::change_input owns actual raw polarity/mux; stop must also
cancel observer. Need lifecycle tests and pre-ownership preparation hook.
prepare_driven currently initializes TIM2, and recovery release/start writes
CR1=1, which would discard CKD: explicit preservation is required before use.
Immediate capture after mux includes filter-settling artifacts. Counts are
latest-capture availability at software reset, not qualified crossings/qzc,
nor all edges. Overcapture is retained, never silently treated as first edge.
Build passed with unused-function warnings; no hardware action or gain claim.

## E314 timing extension, predeclared disabled test gates

FILTERTIME brackets CNT around each pulse without resetting it. Require
mod65536 delta within4 half-us ticks of twice the TIM17 elapsed microseconds,
post-pulse comparator low, and matching row index/elapsed. Captured timestamp
minus pre-write CNT must be0..6 half-us ticks unfiltered or28..38 filtered:
ideal14..16us plus software-write/synchronizer allowance, not a threshold fit.
Rejected pulse has no latency claim. This checks pulse-local running counter,
not every future configuration transition or live reference integration.

E314 hardware result: filtertime_01 PASS on installed9AA099C1661E7FCB3EAED14C0DAA92981FC46E6E516E77DF14237A2F1892F020.
Unfiltered timestamp delta0 ticks at0.5us resolution (not zero physical delay);
filtered40us request software bracket41us and capture delta30ticks=15us.
All pulse-local count/time differences meet predeclared4tick gate, post low,
capture pattern1,1,0,1, no overcapture, restore/finaloff pass. No motor run.

## E313 disabled pulse-test gates (declared before hardware run)

Candidate EF1E586997CA28C49C8E802EEAA7563E4FB03739EEF7F4A864242BA6FD8AB9A9.
Host suite: 263 tests pass. Require four ordered rows: filter0/2us capture,
filter0/40us capture, filter15/2us reject, filter15/40us capture. Require
pre/high levels0/1, no overcapture, ENABLElow, software width bracket >=request
and <10us short / <50us long, restored configuration and verified finaloff.
This is route/pulse discrimination evidence only, not live counter-continuity,
BEMF qualification, or CPU savings. No motor drive in this test.

Result: captures/filtercheck_01.txt PASS, SHA256
F8381CB30F817B25FA0476835B5872AACE895E0F6AFC62EAFF48D83724BC73F5.
Measured software brackets exactly2/40us. Captures1,1,0,1; levels2 throughout,
overcapture0, ENABLElow and configuration restoration pass. Rejected pulse
leaves stale CCR249 unchanged, correctly ignored because CC2IF0. Candidate
now installed, no motor run. Disabled atomic256, roles6 at6.2%, CPU2/7/10,
archive3x3 also pass, each finaloff verified. Next counter-continuity and
capture timing checks before observe-only motor integration. This does not
establish the filter's threshold or its behavior with analog BEMF/noise.

## Confirmed routing, not a qualified replacement

ST RM0444 Rev6 section22.4.28/page688 explicitly gives TIM2_TISEL TI2SEL=1
as COMP2 output,0 as externalTIM2_CH2. No external jumper is required.
[ST manual](https://www.st.com/content/ccc/resource/technical/document/reference_manual/group0/2f/21/cb/33/78/80/42/64/DM00371828/files/DM00371828.pdf/jcr%3Acontent/translations/en.DM00371828.pdf).
Source was retrieved through indexed ST text. FullPDF webopen exceeded10MB;
directcurl transferred0bytes for44seconds and was cancelled. No successful
local RM0444 cache was established; do not mistake any partial artifact for it.

Local stm32g0-staging0.17.0 stm32g071/tim2/ccmr1_input.rs defines IC2F via
ICFILTER: code15=fDTS/32,N8, code12=fDTS/16,N8. These are hardware
consecutive-sample filters, not CPU persistence loops. PAC encodings still
require disabled hardware validation; prior ADC route discrepancies exist.

## Resource constraints and next decisive test

TIM1 channels already generate motor PWM. TIM2 is already the reference
2MHz interval counter (PSC31 at64MHz), reset at accepted input. Its CH2
capture is a candidate side input, NOT permission to repurpose the counter.
Do not change PSC/ARR/CNT/SMCR or issue UG during active reference operation.
Audit CKD/filter sampling independently of PSC; do not assume2MHz counter
frequency is the filter clock. Confirm exact TIM2 CKD behavior before encoding
filter durations. No promised noise rejection or CPU savings yet.

First build an ENABLElow diagnostic with EXTI/NVIC motor path masked and all
output owners stopped. Save/restore TIM2, COMP2CSR, EXTI and NVIC state;
route COMP2 to TIM2CH2, capture controlled polarity transitions. Compare filter0
with a nonzero filter using measured short/long pulses. Count CC2IF and CC2OF,
verify expected edge/polarity and counter continuity, and restore with finaloff.
This tests route/filter, not BEMF performance. No DMA buffer needed initially.

Only after that evidence consider observe-only parallel capture while spinning.
Do not replace minz CompExti pending semantics with CC2IF naively: closed-gate
retained pending, polarity/mux transients, filter settling, delayed captures,
overcapture, counter reset and stop/recovery epochs all need explicit handling.
Keep actual arm, electrical, tracking and finite deadline safeguards untouched.
Filtered edge latency is a control change, not merely an optimization.

E310 no firmware/flash/UART/motor action. Installed8D1AAABF,lastoffE309,
traceOFF. This identifies a testable hardware route; whole goal remains open.

## E311 filter plan and sample-clock bounds

Local TIM2CR1 CKD enum confirms Div1/2/4 encodings0/1/2. New pure
capture_filter.rs covers all16 IC2F settings and three legal CKD settings,
without any PSC field or register writes. It produces TI2SEL=0x100,
CC2S=01 and IC2F bits in CCMR1. AtCKD2/filter15, samples are128kernelcycles
apart and8consecutive samples qualify. Ideal sampling-phase bracket is
896..1024cycles=14..16us at64MHz; synchronizer/edge/IRQ latency is excluded.
Do not call14us an observed hardware rejection threshold. Filter0 intentionally
has no deglitch-duration guarantee in the model. CKD changes would require
preserving CEN and all unrelated CR1 bits, not whole-register live writes.

184Rust tests plusM0no_std check pass. Module is not connected to firmware yet;
root/installed8D1AAABF unchanged, no serial/flash/motor action. Next disabled
probe compares nominal1us vs40us polarity pulses atfilter0/15, measures actual
pulse width, checks capture/overcapture and restores state. Software-generated
transitions must be verified at the comparator output, not assumed from CSRwrite.

E312 optional bench-capture-filter adds idle filtercheck, NOT flashed. It uses
TIM2CH2 risingcapture/CKD2 with code0/15 and requested2/40us pulses, measures
the software pulse bracket and reads comparator pre/high levels. DIER must0;
COMP/TIM2NVIC remain masked, no DMA or IRQ enabled. Restores timerconfiguration
and COMP2CSR; intentionally clears pendingflags and leaves interruptpaths masked
at safe idle, rather than resurrecting stale requests. Configrestore verifies
PSC/ARR/TISEL/CCMR1/DIER; countercontinuity and complete restore readback still
need stronger checks before calling the disabled test complete. No hardware
success claimed from compile. Add strict host sequence/width/capture/level/off
verifier before running; shortpulse must stay below calculated filter window.
