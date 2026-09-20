# Mean-current qualification on the DRV8304H rig

## E619: actual same-DMA run still negative; not a calibration fix

Frozen213D in captures/reference/prestartdma_618 installed after32B1 off/guard
verification. Download/OpenOCD reset exit0. Own baseline fourcasesPASS:
disabled-driver0samples; hostabort1scan/113us; wake interruption1scan/115us;
complete128scans/25646us followed by rewake invalidation. Finaloff verified.
prestartdma_619_basecheck.txt SHA256
B0ED7CC4384DB551AC9AA002B55D9AE059D495785999505CA973C305092ED1C4.
Guard3/18,fullfive3200 including CPU pair maxima2/7/9us,ADCphase3 PASS.
NativeMCP IRQBUDGETCHECK fivecases/zero failures/50us boundary PASS; synthetic
elapsed test,notISR WCET. Serial closed before fixtures and SWD.

Added explicit --prestart-dma fixture check (presence AND absence enforced).
One startup61/hold70,20kHz,10s at unchanged cycle450/seed400 guards PASS:
338.959258eHz,20337COM/20337accepted,cycle sigma42.980453us,rawpeak298,
busmin10734mV,49751ADCscans. CRC/timeline/ADC/finaloff verified. Initialarm12us.
This opt-z one-run result is NOT32B1's recovery qualification or its jitter.
Capture prestartdma_619_hold70_10s.txt SHA256
38E32C2749ADA38B41A86488C0598B901BDC59F2C53CFC4E82A7377FC1D7E089.

Same-DMA prestart centered means11.273438/12.343750/12.171875;
powered10.164560/11.144841/12.300758. Residualsum -2.178904 counts.
No simultaneous PSU reading at this7% run; do NOT attach E602's130mA here.
This is direct evidence that matching sequence/cadence alone is insufficient,
not evidence proving drift or switching bias specifically. Do not fit an offset,
flip signs, declare gain calibration, or repeat identical runs to seek agreement.
Further current work requires a new discriminating observation (e.g. same-wake
drift bounds plus acquisition-aperture/sector evidence), not another zero-mode
or settling sweep. Regression retains negative result and amps=None.

Restored frozen32B1 download/resetexit0, prestartdma_619_restored32b1.txt
guard3/18/finaloffPASS,Uartclosed. Actual32B1/OFF; root213D diagnostic differs.
Keep feature-gated code/data for metrology; no new speed/recovery qualification.

## E618: same-DMA prestart candidate implemented, not installed

Feature bench-prestart-dma selects a128-scan pre-drive acquisition through
adc_stream's actual start_inner/poll/stop path. No fake powered owner, no
gate enable, no interrupt unmask; foreground consumes coherent DMA leases.
Existing wake token, abort, outputs-off and50ms bound remain. Producer/owner
checks precede IRQ masking. Failed acquisition disables ENABLE before cleanup;
the caller also safes any final refusal. Success preserves the wake and returns
raw statistics only. No offsets applied; recovery still revokes initial zero.
The phase-capture DMA is prepared/stopped through the existing stream lifecycle
and is reset again when powered streaming starts; this needs physical checking.

BASEACQ explicitly identifies ascending0/1/4/6/13, cadence and first trigger.
Host decoder rejects duplicate/malformed metadata and residual report rejects
baseline/powered cadence mismatch. Five residual +five baseline testsPASS.
Synthetic metadata tests are parser evidence, NOT proof old captures used DMA.
Old captures remain labeled software baseline and uncalibrated.

Release-s/thinLTO/codegen1 compilation reached linker but overflowed416bytes.
No old output flashed. Per-command CARGO_PROFILE_RELEASE_OPT_LEVEL=z build
passes, thinLTO/codegen1 retained; emitted objdump/math audit completed:
root SHA256 213D27559DF04C863158D6835E5E89AE45CD7CB8533BD53F0CC595932F7FF94C.
size reports text117488,data1108,bss29260 (not a stack/WCET qualification).
Cargo.toml default remains s; opt-z was local to the build process.

NO flash/UART/motor. Actual32B1 remains lastoff E616. Root is now213D, not
retired6A10 and not the installed board. Next own disabled baseline abort/
epoch/cleanup tests, guard and timing preflights before a powered candidate
run. Need fixture-required BASEACQ provenance before powered collection; no
transfer of32B1's recovery or timing qualification to opt-z. Matching scan
configuration alone will not prove drift, sampling coverage or calibrated amps.

## E617: independent 8.5% supply anchor exposes ADC-estimate disagreement

Operator explicitly confirms 130mA DURING E602's 8.5%/30s run, not idle.
This supersedes historical "anchor missing" statements for this operating
point only. Capture cycle450_602_hold85_30s.txt SHA256
440B7D609C8E4FD78E23E2809AE034D25C464EFC7ED6077AE9475B5B6A3A89F0
was rechecked with drv_baseline_residual.report (full motor verification,
CRC baseline/sums). Build3A70,409.811eHz,149254 delivered scans.

Powered centered means A/B/C:4.918073/4.330892/9.281922 counts.
Prestart centered means:5.148438/7.484375/11.328125 counts.
Subtracting gives -0.230365/-3.153483/-2.046203, sum -5.430051 counts.
The adjacent E6028% capture also gives negative sum -4.120195 counts.
These are NOT negative measured supply currents, and do not corroborate
130mA quantitatively. Regression test retains amps=None and uncalibrated.
Raw peak404 is not a supply-current estimate either.

The documented nominal gain10 and7mOhm shunts imply70mV/A, but multiplying
an invalid offset-corrected estimate by that scale cannot validate it.
Source recheck confirms the known E286 confound: prestart uses individual
software-triggered4/1/0/6/13 reads; powered DMA scans ascending0/1/4/6/13
every201us. Same wake is necessary, not sufficient. E287/E288 already tested
mode/settling and found no stable correction; do not restart that same grind.
No evidence here isolates mode, drift, switching bias or actual current as
the cause, and no sensor rewiring or sign reversal is justified.

Next meaningful current implementation is a SAME-DMA pre-drive baseline
with bounded ownership and retained raw evidence, followed by a matched
powered comparison; not another software-zero repeat or an empirical offset
chosen to force130mA. Even that needs drift/coverage validation before amps.
This is supplementary metrology, not a prerequisite to retain the established
130mA operator observation or a reason to weaken electrical guards.
No hardware commands, firmware build or guard changes. Actual32B1 remains
last verified off E616; root6A10 is retired and must not be flashed.

## E288: bounded settling comparison, not a calibration fix

On4DA1EE7E, measured1ms and20ms idle settling probes pass3/3 each, ordered
1/20/20/1/1/20ms. Current-channel summed DMA-minus-SW-midpoint differences:
1ms `[4.109375,3.44140625,2.875]`;20ms `[-1.08984375,0.6953125,-1.05859375]`.
SWafter-minus-before sums are `[3.3125,0.8046875,-0.625]` and
`[-0.2578125,2.703125,1.6328125]` respectively. Longer settling changes the
comparison but does not consistently reduce observed within-wake variation.
Small cohorts, mode order and different wake calibrations limit interpretation.
Do not apply a correction or change motor startup delay on this evidence.

The first diagnostic revision incorrectly demanded a readiness token before
settling and refused at61us. Corrected probe records nFAULT low during wake,
then requires it high after the fixed delay before any acquisition. All six
trials observed that low-then-ready behavior; no ready timeout was extended.
Motor safeguards and existing startup code are unchanged.

Bound this investigation here until new evidence justifies another test.
The independent current anchor remains missing; no old PSU number is reused.
Continue waveform/reference work on the intermittent5.5% timing limitation.

## E287: gates-off SW/DMA/SW comparison on one wake

New optional `basemode` probe reuses the stream's hardware setup, coherent
DMA poll and restoration without creating a powered motor owner. DMA IRQ is
masked; foreground polls. It compares128 five-channel software scans,128
hardware-triggered DMA scans, then128 software scans on the same CSA wake.
No PWM output is enabled; no offsets are applied. This isolates some ADC-mode
differences, not all running ISR/temperature/settling conditions. Software-loop
checks/gaps also differ from the original prestart function.

On D40A2D88, basemode_compare_01..03 pass. Summed current-channel DMA-minus-
software-midpoint differences: +2.0625, −2.09375, +1.1796875counts. Software
after-minus-before sums: +3.40625, +1.921875, +1.09375counts. Acquisition mode
alone does not yield a stable correction from this sample. Midpoint subtraction
is descriptive, not a proven drift correction. VREF DMA-minus-midpoint is
+0.65625/+0.4296875/+0.5859375counts; no resulting amp correction is inferred.

Next distinguish wake settling/repeatability from mode with bounded idle
measurements, retaining before/after controls. Independent PSU reference is
still missing. This diagnostic image has not requalified powered operation;
the measured motor baseline remains807372B6. Never treat these raw differences
as justification to alter guards or baseline-subtract recovery data.

## E286: matched initial-run capture, independent reading still absent

`cachedoff_current54_30s_01.txt` on807372B6, trace0, 5.4%, completes30s
near290.863eHz with149253 coherent ADC scans. Prestart and initial stage
epochs match; no recovery invalidation was crossed. All motor/safety/capture
checks pass. A nonblocking request for PSU display current/voltage was made
before this run; no reply had arrived when this entry was written. Do not
substitute the historical70mA reading or repeat runs solely awaiting a reply.

Baseline centered means `[8.6796875,10.6953125,9.6875]` counts versus powered
`[9.9168526,9.1264229,9.9309160]` yield sum residual−0.0883085counts. Six
nearby retained initial runs have residuals−6.986..+4.381counts. Different
builds/durations/tracing and baseline epochs prevent treating this spread as
an isolated offset-drift measurement. The near-zero or negative sums are not
zero/negative measured bus current and do not establish sensor polarity.

One source-confirmed confound is acquisition mode: prestart_baseline.acquire
reads single software-triggered channels4/1/0/6/13; adc_stream configures one
ascending0/1/4/6/13 scan triggered every201us. Same sampling-time setting and
wake epoch do not make these identical acquisition conditions. Next use a
bounded gates-off same-mode comparison, preserving timer/DMA ownership,
timeouts and original raw guards. Do not apply compensating offsets or amps
until baseline validity, drift and independent scale evidence justify them.

## E261 correction: counter-bin flatness is not the time-average target

E260 proved nonuniform trigger-counter counts, NOT sampling bias. The
sixstep_write implementation resets TIM1 through EGR at every commutation.
Truncated PWM cycles give unequal actual dwell time to counter bins. Uniform
time sampling can therefore correctly produce a nonuniform counter histogram.
Do not flatten bins or apply inverse-count weights without independently known
time occupancy and adequate conditional coverage; this could bias the estimate.

scripts/pwm_time_occupancy.py computes exact dwell for explicitly supplied
reset intervals under a continuously running, zero-at-reset timer model.
Tests demonstrate the distinction with synthetic150-tick resets/100-tick PWM:
uniform time observations occupy the lower half twice as often, correctly.
This does NOT reconstruct E260's missing exact reset history or paused timing.
Decoder now explicitly reports both sampling_bias_proven=False and
time_occupancy_known=False. No inferred unbiasedness either.

E260 adc_phase45_01 raw A+B+C centered mean is80194/4973=16.12588 counts;
its initial baseline centered sum is2346/128=18.328125 counts. Naive subtraction
is about-2.202 counts. Both are same-initial-wake records, but stationarity,
offset drift, acquisition differences and sampling representativeness remain
unresolved. This is a diagnostic residual, NOT negative measured bus current
or proof of any one cause. No amp conversion or guard calibration is justified.
Next prioritize baseline/offset validity and independent supply comparison;
do not turn histogram flatness into an invented completion requirement.

## E258 timed stream now survives driven entry and recovery

Combined driven/DMA build uses explicit201us steady cadence and101us first
trigger. The original101us combined profile overloaded delivery (E253);
201us first-trigger wait then exhausted recovery initial-feedback budget
(E255/E257). Preloading the stopped trigger counter removes100us initial
wait while preserving acquisition timestamps and the1ms age guard.

Two same-boot4.5% campaigns (10s and30s, loss injected at2s) complete near
234eHz with39739/139244 resumed scans,queue peaks2/3 of8,DMAmax11us.
First delivery sees previousfeedback943/936usold, newframe159/158usold,
both accepted. Old initial baseline explicitly revoked on recovery wake;
resumed sums are NOT calibrated using that baseline.

This is coherent timed delivery and recovery evidence, not unbiased current
or independent BEMF parity. Next: PWM/sector coverage under commutation
resets, same-epoch initial-run zero/drift evidence and independent current
comparison. Do not treat relative-prime nominal periods as measured coverage.

## E252 sequential timer reuse implemented, not hardware-qualified

Optional `bench-driven-dma` connects the existing powered ADC stream to the
under-drive handoff build. The old blanket compile refusal remains for other
driven-power/DMA combinations. The forced owner ends TIM3/TIM6 and PWM-capture
DMA before releasing its handoff token. Only the later powered foreground loop
starts ADC streaming. Recovery already stops/restores the stream before fresh
software-triggered acquisition.

ADC start additionally refuses active forced/probe owners, TIM3 CEN or DIER,
TIM1 CC4 DMA requests, or enabled DMA channel2. It masks/unpends the old TIM3
vector before configuring TRGO. Existing channel1/ADC-idle admission, 101us
triggering, owned FIFO, overflow/raw-current shutdown, original timestamps,
and post-stop restoration remain. No simultaneous timer ownership is intended.

This is compiled integration, not live qualification or proof of unbiased
coverage. New combined-build startup latency, FIFO consumption, IRQ/stack
headroom and shutdown/recovery restoration need hardware tests. Baseline uses
the same ADC sampling-time setting but a different software-triggered sequence;
offset/drift and powered PWM/sector coverage must still be established before
calibrated current claims. Installed firmware remains E251 foreground ADC.

## E251 actual baseline and recovery identity

E250 added a bounded initial scan with the existing ADC reader; E251 records
its invalidation on a real powered recovery. Capture
`captures/prebase_epoch_reentry53_01.txt` has 128 samples/channel in 13690 us,
`entry_same_epoch=1`, then `BASEEPOCH recovery_checked=1 recovery_matches=0
initial_records_only=1`. The recovery check occurs after wake and before fresh
edge acquisition; it does not overwrite the initial statistics or entry result.
The complete 10-second campaign, injected loss and resumed operation pass.

This closes initial-versus-recovery identity ambiguity, not current calibration.
Do not pair initial BZ85 offsets with resumed-segment AL85 measurements. Neither
physical standstill nor offset drift has been measured here, and the existing
early launch histograms do not prove unbiased powered sampling. Next work is
timed powered sampling with explicit timer ownership and coverage, followed by
appropriate same-epoch baseline/drift and independent current comparisons.

## E248 live epoch adapter qualified at one operating point

Optional `bench-current-epoch` connects singleton calibration_live to PD1
set_pin. Physical write comes first inside an IRQ critical section, followed
by epoch update; begin/matches use the same serialization. There is a small
interrupt-mask entry cost before the physical write,not a zero-cost guarantee.
No other-pin/default-feature path changes. Audit found no raw GPIOD support
writes outside this adapter; shell panic uses set_pin(false) then never resumes.

Idle epochcheck never commands gates,only wakes CSA. Three trials each pass
five checks (no prewake token,valid wake,repeatedhigh retention,oldtoken refused
afterlow/rewake,freshidentity). Repeatedhigh256writes total184us,max1us; initial
<=5us diagnostic gate passes. No old tracker is restored after diagnostics.
Archive/CPUoverhead checks pass. One5.3%10soriginal-budget powered recovery
passes284.951eHz,arm43.5us/cost14us,deadline132usspare,stack4752,finaloff.
This is adapter timing/identity evidence,not actual baseline calibration.

Next add bounded stationaryprestart acquisition with existing powered ADC
settings and rawstats,token captured before and checked afterscan. Store raw
epoch provenance through firsthandoff and invalidate on recovery; don't apply
offsets to fast guards. Stationarity,analogsettling,VDDA/drift and unbiased
powered sampling remain separate prerequisites for a calibrated current claim.

## E247 baseline-identity prerequisite (pure, not integrated)

`examples/support/calibration_epoch.rs` supplies a single-board/boot identity
tracker and non-Copy token. Repeated commanded ENABLE high preserves identity;
any commanded low, explicit invalidation, or failed physical ENABLE/nFAULT
readback while checking a token revokes it. A recovery wake cannot restore
the old token. Counter exhaustion permanently refuses instead of wrapping.
Six host tests cover these transitions,including shutdown during a baseline
scan; no_std M0 check passes. This provides no gate authority or offset math.

Live integration is still required. Use ONE tracker per boot,never construct
a replacement while retaining tokens. Serialize the physical PD1write and
state update with token operations across foreground/IRQs; perform electrical
shutdown first and avoid delaying it for diagnostic work. Audit raw GPIO and
panic paths as well as set_pin. Account for repeated high writes in COM:
provenance bookkeeping must not consume the recently recovered timing margin.
Do not regard a successful begin() as evidence of rotor standstill,CSA settling,
same-reader configuration,baseline completeness,or calibrated current.

## E246: driven-path coverage exposed; initial wake constraint has changed

The reader already collected192early launches per current channel, but
driven_run::dump omitted coverage_dump. It now emits existing ADCLAUNCH/AL85
only in fixture output after shutdown. No extra runtime sampling is added.
current_coverage53_01 completes1s at5.3%,283.48eHz with full capture verification.

| Logical channel | Rejected /192 | Eight launch bins, early to late |
| --- | ---: | --- |
| A | 9 | 8,10,16,33,24,20,23,49 |
| B | 9 | 16,11,29,34,26,23,25,19 |
| C | 5 | 8,21,26,26,29,24,28,25 |

These are descriptive early-window launch counts,not independent random trials,
aperture counts or joint sector coverage. No statistical-IID test or current
reweighting is justified. A visits every bin but ranges8..49counts; merely
taking more biased samples does not establish a mean. This192cap is already
full after1s; repeating long holds with the same collector adds no information.

E185's calibration-epoch impossibility applied to the old coast-then-wake
handoff. Current successful driven_run keeps ENABLE during gates_off,
prepare_driven/stage_driven,reason22release,adopt_driven and awake start_inner.
A pre-start zero can therefore potentially share the FIRST powered segment's
epoch. This is a source-grounded opportunity,not implemented calibration:
still require stationary pre-drive acquisition,explicit wake/epoch identity,
same ADC settings,drift/VDDA handling and valid powered sample coverage.
Faults still disable immediately. Recovery prepare+wake starts a new epoch;
never apply the initial zero to that resumed segment or measure its zero from
moving coast without independently excluding diode currents. No safeguards
should be delayed or removed to preserve a calibration epoch.

## E243 operating context (supersedes envelope numbers below)

Repeated under-drive startup/injected-loss recovery now passes5.3% at285..286
eHz (three10soriginal-budget trials).5.4% stopped on a3329us accepted cycle
against3333us floor. Do not widen that guard simply because rawcurrent405 and
bus11056mV looked healthy. Mean-current calibration and sampling qualification
below remain missing; higher speed needs those alongside timing/sensing review.
No new independent PSU mean reading has arrived. No standalone offset is
valid for another ENABLE epoch. Installed460C0BD8 includes foreground ADC,
not the TIM3/DMA current-statistics experiment. Avoid claiming calibrated
current from either rawpeak or accepted-event sampling occupancy alone.

## E238 status and current authority

The active objective permits duty-led staged exploration up to30% as a hard
maximum, not a target. Historical statements below about no duty expansion
describe their earlier evidence state; E235–238 now retain guarded sustained
operation to5.2%/~278eHz and repeated recovery at5.0%/~266eHz. They do NOT
turn raw ADC peaks into mean supply-current measurements.

Current installed build is release/s/thinLTO with under-drive entry, IRQ-union
instrumentation and staged recovery. The30s capture current_anchor50_01 passed
at266.319eHz, rawpeak403 and busfloor10901mV. An operator PSU reading was
requested asynchronously for that run but has not been received as of E238.
The filename is an intended measurement opportunity, NOT proof of an anchor.
Do not substitute the historical~70mA reading at238eHz or the800mA limit.

Re-reading E185 confirmed that repeating a standalone zero check would not
calibrate a later ENABLE epoch. No new zero was taken or offset applied. A
same-wake, pre-drive zero plus unbiased powered sampling and an independent
supply anchor remain the route to mean-current qualification. Missing operator
data is not a reason to fabricate current or block unrelated firmware work.

## Established topology and units

Visually checked `block_04_Half-bridges_and_Sense.png`: R33/R29/R15 are
7mOhm shunts between each half-bridge PGND and common GND. They are NOT inline
phase shunts. SPA/B/C sense the bridge side; SNA/B/C sense ground.
The cached `drv8304.pdf`,pp28-31,describes bidirectional low-side current sense:
CORRECTION E755: cached SLVSE39B p28 equation3 and Figure30 explicitly give
I=(VREF/2-SOx)/(gain*R): SOx = VREF/2 - gain*(SPx-SNx).
The former PLUS sign here was wrong and propagated into the average guard.
Positive bridge return lowers SOx. Auto calibration follows ENABLE reset.
The existing bench conversion uses gain10,thus70mV/A. Preserve measured
logicalA/B/C channels4/1/0; do not adopt the schematic's connector labels blindly.

For stationary operating statistics, the signed sum of the three low-side
shunt means estimates net bridge return current. Positive/negative recirculating
currents cancel in the sum if sampling is unbiased. Sequential ADC samples
cannot be treated as an instantaneous Kirchhoff sum. Board electronics and bus
capacitor transients prevent identifying this directly with instantaneous PSU
current. Use a steady segment and label the estimate as bridge return current.

## Measured reason a fixed2048 subtraction is inadequate

Entry157: four independent gate-disabled32sample `sensezero` bursts, each after
driver wake. Mean raw(ch0,ch1,ch4):

| Burst | ch0 | ch1 | ch4 | Sum minus3*2048 |
|---|---:|---:|---:|---:|
|1|2052|2049|2051|8|
|2|2054|2049|2053|12|
|3|2054|2050|2044|4|
|4|2054|2053|2050|13|

At nominal3.3V,one ADC count is about11.5mA through70mV/A gain. Fixed2048
would report roughly46..150mA with gates off. That exceeds/is comparable to
the historical minz44.85mA reference. These short noisy bursts are evidence of
uncertainty,not final offset calibrations. The historical command's ADC timing
must not be assumed identical to the guarded powered reader.

## Measurement implementation gates

1. Measure zero with the SAME ADC channel/sample configuration as powered
   feedback,after wake settles and before any drive. Preserve raw sums,count,
   per-channel spread and VREFINT. A new ENABLE wake recalibrates the CSA,so a
   prior-wake offset is not automatically valid for recovery. Do not calibrate
   from a moving coast without ruling out diode/recirculation current.
2. Retain the existing fast electrical guards unchanged. Measurement statistics
   are supplementary,never used to relax the phase peak or bus protections.
3. Sample independently across the100us PWM period and commutation sectors.
   Prove phase occupancy; an asynchronous-looking foreground loop or rotating
   channel order is not proof of uniform sampling. Preserve acquisition timing
   or phase-bin counts,not merely a single average. Do not use mid-ON-only data
   or multiply one sampled peak by duty and call it measured supply current.
4. Accumulate signed raw sums/counts (adequate width for600s),not absolute values.
   Apply per-channel zero and measured VDDA on the host. Publish uncertainty
   from zero drift/sampling coverage; invalid or biased samples remain visible.
5. Validate zero,known stationary operating point,and repeatability against an
   independent PSU/DC-link mean reading before claiming current parity. Supply
   limit800mA is a ceiling setting,not that independent current reading.

Keep the measurement path out of the fresh-seed arm budget. Current default
stack span4136bytes leaves only40bytes above the existing4096span check; budget
RAM before adding per-phase histograms or first-segment copies. Measured untouched
stack1548 is useful evidence,not permission to ignore nesting/temporary copies.
No duty expansion is justified by the current evidence.

## Offset probe implementation (Entry158, not yet hardware qualified)

`cargo build --profile bench-probes --example shell-pwm --features bench-current-probes`
builds optional idle `zerocheck`. Default release omits it to stay within flash.
Use `cap1` then `zerocheck` for five CRC-protected Z85 records; the capture flag
is consumed. Each record contains channel,n,sum_lo,sum_hi,squares_0..3,min,max
(all little-endian u16 plus existingCRC32). Channels4/1/0/6/13 match powered
feedback.512samples per channel,200ms acquisition deadline,bounded per-conversion
reader and driver/outputs-off checks. Driver wake precedes measurement; printing
occurs only after disabling. No offset is applied to firmware or guards.

The optional size-optimized image is for DISABLED metrology only in this campaign.
It does not inherit release ISR/arm timing qualification. Restore the normal
release image before motor campaigns. Same reader source/ADC settings do not
make the compiler timing identical; no powered current parity follows from it.

Entry160 extends zerocheck to TWO512sample windows within one wake,still under
the200ms acquisition bound. First recordsZ85,secondZB85; header windows=2 and
same_wake=1. Three paired hardware captures show nominal signed-sum drift
-1.15/-14.61/-12.00mA over~125ms with gates off,versus108..198mA false absolute
readings from a fixed2048 baseline in E159. This supports same-wake subtraction
as a candidate,not proof of powered accuracy,longer-term drift or IID errors.
Normalize by measured VDDA when comparing windows; raw-count conversion alone
uses nominal voltage. Capture firmware/calibration mode remains metrology-only.

Entry161 adds host-only `drv_zero_check.py --input <capture> --vcal 1662`.
The explicit factory calibration argument (E157 board value) converts each
window using VDDA=3000*vcal/mean(VREFINT), then differences CSA output voltages.
For zero_samewake_01/02/03 the summed output drift equivalents at nominal
70mV/A are +1.893/-13.552/+10.204mA, not the nominal-count values above.
The third pair changes sign because estimated VDDA changes by1.036mV.
These are output-voltage drift equivalents, including CSA bias drift, NOT
measured current. Ratios of window means omit within-window covariance;
neither voltage normalization nor near-zero drift certifies powered accuracy.

Source audit: powered_timer::sample_feedback reads4/1/0/6/13 sequentially;
read_channel records TIM17 launch age only for ADC faults. It does not capture
TIM1 PWM phase at the sampling aperture. Existing successful captures therefore
cannot prove PWM occupancy. Future coverage instrumentation must bracket actual
conversion launch (not scan entry or delayed completion), account for sample
aperture and ISR preemption, and distinguish phase/channel/sector coverage.
Budget its memory and timing before granting it powered measurement authority.

Entry163 now supplies actual early launch occupancy on two10s powered runs near
205eHz. E162's30byte collector fits stack span4108; measured untouched1860.
All eight bins are visited, but earliest-bin counts are6..16 in run01 and9..13
in run02 versus a uniform24 of192attempts before rejection. Rejections5..10
are explicit. Do not equate broad coverage with unbiased means. Measurements
are first192launches/channel only,not the physical ADC aperture or joint sector
distribution. Next design must address repeated early-bin deficit and characterize
aperture timing; increasing averages alone will not remove systematic bias.

## Live timing audit (Entry164)

Read-only probe on the E162 image after E163: ADC_CFGR1=00000000,
CFGR2=80000000,SMPR=00000007,RCC_CFGR=00000012. Source and device PAC agree:
12-bit software-triggered single conversion,no oversampling,CKMODE=PCLK/4,
all channels selecting SMP1=160.5cycles. With the configured64MHz PCLK this
is16MHz ADC: tracking10.03125us, SAR0.78125us,total10.8125us,excluding launch
and software channel-selection overhead. Existing 4%/10kHz PWM has a nominal
4us on slice before dead-time/commutation effects. Launch bins are12.5us wide.

RM0444 section15.3.9 explains charging the sample/hold capacitor during sampling;
section15.4.3 distinguishes EOSMP from EOC. This is tracking/settling followed
by hold, NOT an ideal rectangular average over10us. Do not convolve the PWM
waveform with a10us boxcar or shift old coarse bins and claim calibrated
current. The trigger-to-tracking offset and analog source settling remain
additional timing/accuracy terms. Shortening SMP blindly would change settling
and invalidate same-reader zero evidence (including VREFINT acquisition).

Authoritative manual lookup (local named RM0444 not found during this audit):
[ST RM0444](https://www.st.com/resource/en/reference_manual/rm0444-stm32g0x1-advanced-armbased-32bit-mcus-stmicroelectronics.pdf).
Indexed section15.3.9/15.4.3 retrieved; complete PDF transfer timed out. Bitfield
values independently checked in installed stm32g0-staging0.17.0 stm32g071 ADC
smpr.rs/cfgr2.rs and against live readback, not borrowed from another MCU.

Next measurement design must retain the guard reader while establishing
sample/hold endpoint timing and source settling for an explicitly timed
measurement path. Hardware-trigger/DMA is a candidate to remove foreground
latency; it must have explicit ADC ownership, bounded transfers and guard
freshness validation before replacing the reader. The observed launch deficit
alone is not authority to reweight currents or expand duty.

## Independent supply anchor before further metrology expansion

The precision firmware-current path is still unfinished, but that does not
prevent directly measuring this rig's supply consumption. Coordinate one
operator-observed known-point hold before adding further ADC mechanisms:

1. With all gates/MOE/ENABLE verified off, record the PSU's actual voltage and
   current readouts (including units/display resolution), not limit settings.
2. Once the operator is watching, run the already proven4.5% powered profile
   with a finite60s hold, no deliberate dropout, existing800mA limit and all
   unchanged guards. Retain firmware capture and actual accepted-event speed.
3. Record voltage/current and CV/CC indication during a settled portion, with
   approximate run time and observed variation. More than one display update
   helps identify an unstable reading; do not present a single rounded display
   as precision metrology. Verify finaloff and record idle again if available.
4. Report total PSU current and supply power separately from idle-subtracted
   incremental consumption. ENABLE-off idle is not the same electronics state
   as powered operation; subtraction is not a pure motor-loss measurement.

The historical FALCON run used a different supply/board/controller; compare
conditions and power, not raw mA alone. This anchor is neither a calibration
of the low-side shunts nor a matched frozen-AM32 parity result. It can still
give a real operating-current fact without waiting for perfect ADC timing.

## Hardware-trigger building block (Entries168–169)

Separate TIM3_TRGO rawEXTSEL3 at101us -> ADC13 -> DMAMUX request5 -> DMA1CH1
finite32halfword transfer now passes3/3 disabled trials in3246–3247us. This
avoids repurposing TIM6's guard or TIM2's reference interval timer. The installed
PAC EXTSEL names disagree with RM0444 Table73; use verified raw route constants.
The optional diagnostic cannot coexist with a foreground ADC transaction;
normal motor firmware has been restored and its stopped-state checks passed.

Before integration: verify five-channel ordering,producer/consumer ownership,
bounded stale/error handling and restore semantics. A cyclic single buffer can
be overwritten while foreground reads it; the old harvest pattern must not be
copied as proof of coherent scans. Keep current guards and same-wake baseline
requirements. A101us trigger period alone does not prove unbiased occupancy
when commutation resets the PWM timer.

E172 hardware validates the two-half ownership checks in a disabled foreground
consumer: two32scan runs,max4us copy; injected350us consumer delay refuses
both-pending completion flags and publishes zero scans. Normal firmware restored.
This is not yet IRQ coexistence or a fresh guard-feedback publisher. A restarted
production acquisition needs a real epoch and all stale/error exits must retain
shutdown semantics. No mean-current inference follows from coherence alone.

E180–181 supersede the integration-pending status: opt-in IRQ producer plus
eight-scan owned FIFO passes three10s holds and three30s dropout/reentry
campaigns near205eHz. Raw phase limits checked in IRQ; every queued scan gets
original-timestamp guard checks in foreground. DMAmax10us,queuepeak4/8.
Normal firmware remains foreground ADC. This establishes usable coherent
delivery and restart,not calibrated mean current: same-wake baseline,whole-run
signed current aggregation,coverage evidence and independent supply anchor
are still required. No precision-current claim from scan counts or peaks.

E183 supplies segment-local64bit signed raw A/B/C sums and raw bus/VREF sums,
with count/acquisition stamps in five fixture-only CRC S85 rows. Corrected
dma_sums40_02 validates99008delivered scans over10s; the initial mismatched
capture remains rejected. drv_current_sums.py deliberately emits no mA result.
These sufficient statistics do NOT retain within-window current/VREF
covariance or PWM/sector coverage. Do not turn ratios of these sums into a
precision-current claim. Same-wake zero must avoid moving-coast diode current
and ENABLE-triggered recalibration; both remain design constraints.

## Entry185: calibration epoch audit and observed raw ambiguity

Source audit makes the epoch problem concrete:
- shell-pwm.rs powered handoff at4.7s explicitly disables PD1, calls
  core_bench::prepare_early, then powered_timer::wake before flying acquisition.
- core_bench::resume_once likewise calls prepare and wake before re-acquisition.
- powered_timer::prepare disables PD1; wake raises it and waits1100us.
- adc_stream::ensure_started/start require powered_timer::owns. This stream
  currently has no gate-disabled zero acquisition mode. zero_check uses the
  old software-triggered reader, not the timed five-channel DMA scan.

Therefore a stationary pre-start zero does not share the BEMF segment's wake.
Adding it alone would not satisfy calibration. Sampling a new zero during
flying acquisition would instead require independent evidence that moving
motor diode currents are absent. Do not remove ENABLE safing or delay a fault
shutdown merely to preserve a calibration epoch.

Revalidated raw captures with current CRC/count/window/finaloff checks:

| Capture | Delivered scans | Mean centered A/B/C raw counts | Sum of means |
|---|---:|---|---:|
| dma_sums40_02 |99008|3.466417 /3.010726 /8.587680|15.064823|
| dma_fifo_reuse_01 |99008|1.409896 /0.208660 /6.698519|8.317075|
| dma_sums45_reentry30_01 |277111|1.666484 /2.337110 /6.989618|10.993212|

The two~205eHz captures differ by6.747748 summed raw counts despite similar
speed/duty. This is observed raw variation, NOT a measured current difference
or proof that CSA offset alone caused it. Different wake epochs, voltage,
sampling bias and actual current remain confounded. More samples cannot
resolve that identifiability problem by themselves.

Next measurement remains the independent supply-anchor hold described above.
Readiness/actual idle display readings requested again; no operator-observed
run started without a reply. DMA delivery qualification (E184 fault/reuse)
does not change the calibration gate. No firmware or hardware changed in E185.

## E188 independent supply observation obtained

Operator confirms idle11.7V/0.7mA. E187 original6.5% startup trips phase-current
guard at3.264s; operator saw~211mA during that brief attempt,not a steady hold.
E188 reduces only startup target duty to6.2% (catch remains6.5%) and completes
60s at237.6425eHz/4.5% BEMF duty. During that run operator reports~70mA PSU
current. Raw psu_anchor45_62start_01 validates completed window and finaloff.
This is an independent sustained-operating supply-display fact,not a precise
time average,matched historical benchmark or calibration of the ADC shunts.
Running voltage,CV/CC,display resolution and variation remain unreported.
The earlier statements that no actual operating supply reading exists are now
superseded. Same-wake ADC calibration and coverage limitations are unchanged.
