# Timing evidence: release/s, build-scoped measurements

## E362: recovery seed age is already instrumented

Installed58B570 recovered6.6% capture staticcomp_reentry66_30s_01 fully verifies.
SEEDLAT ages98/118/132/172/176half-us, actualarm age192half-us. Stage brackets:

| Bracket | Time |
| --- | ---: |
| Earliest qualified edge to coast_run_inner entry | 49us |
| Entry through carrier/reset setup | 10us |
| Reset through feedback validation | 7us |
| Feedback through guard startup | 20us |
| Guard through reference-state setup | 2us |
| Reference setup through actual arm-age read | 8us |

Sum96us. These are measured wall brackets including timestamp/observer costs,
not exclusive cost or all removable delay. The49us includes mandatory20us
edge confirmation plus acquisition completion/caller work; don't remove dwell.
6.4/6.5 recovery captures also report98tickentry/192tickarm, with reset116or118.
Host timing report now validates optional singleton/mode/monotonic/bounded
SEEDLAT before exposing stages; legacy missingdata staysunknown.304testsPASS.

Source target: powered_timer::start_inner builds RuntimeGuard from live limits
and feedback, installs it and starts TIM6. Statistics are ALREADY staged with
bench-reentry-staging; repeating that change is not a new optimization. Any
further prestaging must retain fresh sample/age, step, original deadline and
one-shot ownership checks. Reference setup2us is not the largest bracket.
No hardware/firmware change E362; next inspect guard construction before
proposing a staged immutable template. Actual32usarmfloor remains unchanged.

## E346-348: peer-priority EXTI operating boundary

Actual installed archivedBA1F982B,24,006Hz carrier,6.2% startup/6.4% BEMF:
one10s hold and3/3 original30s injected-loss/recovery campaigns pass near292eHz.
Recovery cohort IRQ58.368..58.738%,COMmax69us,COMP71us,commit45us,
cycle sigma28.875..29.166us. Recovery arm47..49.5us leaves15..17.5us above
unchanged32us minimum; all original deadlines and finaloff verify. See
captures/peerexti_reentry64_30s_cohort.csv. Startup raw1147/1200 on one attempt
is a separate, narrow electrical margin; recovered raw275..280 is not its proxy.

At6.5%, a10s-command stops after3.34s near299eHz on an actual3211us cycle
against3226us minimum. COMmax still69us, not old nested258us. Physical speed
and the cause of crossing variation remain unproven; do not call this CPU
saturation. No automatic range change. Range expansion must coordinate
runtime/acquisition profiles AND retain the independent actual-age arm refusal.

Budget forecast only: measured recovery seed ages190..192half-us ticks. With
reference wait=ci/2-floor(ci*16/64), ci1041 gives260ticks; subtract192 leaves
68ticks=34us (only2us abovefloor). ci1010 gives253, leaves61=30.5us and MUST
refuse. These are algebraic scenarios, not qualified320/330eHz operation or
worst-case age bounds. They show why a cycle-floor edit alone is insufficient.

## E304: optional path counters removed

Installed1E13B559, E301features minusbench-comp-paths, release/s/thinLTO.
One10s6.2% hold passed280.562eHz, IRQunion58.472% (instrumented62.471%),
cycle sigma39.503us, COMP72/COM98/commit46us, actualarm68us/cost12,
raw272,bus10996mV,stack4124,DMA18queue2,finaloff. NoCOMPPATH output.
This is n1hold, not new recovery cohort or higher-duty qualification.

## E302–303: single-core atomic backend, instrumented 24 kHz

Installed E0FE2277 (full hash in AGENTS), release/s/thinLTO. Fixed3/3
thirty-second original-budget dropout/reentry runs pass at6.2%/~280eHz.
Recovered IRQ union62.44..62.79%, COMP75/COM105/commit46us maxima,
cycle sigma35.09..35.19us, actualarm52..53.5us/cost11..12us,
stack untouched3552, deadline176..242us spare. Cohort raw hashes retained in
captures/singlecore_reentry62_cohort.csv. Still includes bench-comp-paths.
Earlier instrumented critical-section-backend hold E299 was70.126%IRQ; new
same-setting hold62.471%. This is workload evidence, not total CPU utilization.
No higher duty or physical speed ceiling demonstrated by this comparison.

## E296: inlined comparator, 24 kHz BEMF carrier

Current installed945D6788 (fullidentity incohortCSV), release/s/thinLTO.
At6.2%/about279eHz,3/3 thirty-second original-budget injected-loss/recovery
attempts pass. Recovered cycle sigma40.25..40.83us; COMPmax81us,COMmax135us,
atomiccommit bracket48us. Actualrecoveryarm41..42us (9..10us abovefloor),
armcost14us (2usbelowceiling), originaldeadline131..208usspare.
IRQunion67.16..67.37% is software-wall occupancy, NOT totalCPUutilization.
Stack untouched3992bytes. Rawcurrentpeaks265..284counts remainuncalibrated.
See captures/inline24_reentry62_cohort.csv andLAB_REPORT E296.

Same-point10s comparison at~279eHz: inlining cycle sigma58.945->43.983us,
COMP87->81us,COM199->134us,IRQunion68.539->67.260%. Counts/polarity/core
unchanged; shorterread-loop walltime changes physicalfilteraperture.
Notproofof a higher speed ceiling or the near300eHz fault being solved.
The6.2% driven-acquisition command cap currentlyalso limitsBEMFduty; it is
not evidence of an electrical or motor ceiling.

## E285: current cached-comparator build, tracing off

Installed ELF `807372B6F101925F2AA197690B1FA71975315F90B7716B1DDDB1B11640ED10BA`,
release/s/thinLTO, E280 features plus `bench-cached-comp`. No electrical,
tracking, deadline or actual-arm guards were relaxed for these results.

| Setting / capture | Result | Recovered eHz | Cycle sigma us | Recovery arm / cost us | Deadline spare us |
| --- | --- | ---: | ---: | --- | ---: |
| 5.4%, cachedoff_reentry54_01 | 10 s original budget completed | 290.549 | 30.099 | 40 / 13 | 227 |
| 5.4%, cachedoff_reentry54_30s_01 | 30 s original budget completed | 291.010 | 29.634 | 40 / 13 | 216 |

Both have COMP/COM/commit maxima86/81/45us, DMA18us/queue2, untouched stack4128.
Recovery seed1167ticks, actual age212ticks leaves80ticks=40us (8us above floor).
These are observed wall brackets, not exclusive CPU cost or worst-case proof.

At5.5%, three10s holds near297eHz passed, but the subsequent recovery campaign
stopped on CycleTiming at1.746s before injection. Thus neither robust5.5%
operation nor recovery is qualified. Keep that failure alongside the hold CSV.
Current software cycle floor3226us is an experimental envelope, not a measured
physical/MCU ceiling. Its exact guard timestamps differ from recorder minima.
IRQ union near56% is not CPU utilization; foreground is not idle. Mean current
is still uncalibrated, and no new PSU reading was supplied for these runs.

Caching source/polarity/trace mode reduced the shortest traced gate-to-accept
bracket37->29.5us, but changes the twelve-read time aperture too. Retain this
as an experimental candidate, not a proven cure for the short-cycle fault.

## E270: current fast-start build reaches the cycle guard at5.4%

E269 completes3/3 thirty-second5.3% startup/recovery campaigns near284.2..284.5
eHz on6775326F (release/s/thinLTO,phaseDMA,firsttrigger1us/steady201us).
Recovery arm43us/cost13 leaves11us above32us floor. DMAmax18us,queue2..3/8,
stack4232; original deadlines preserved. These supersede E263's statement
that current phaseDMA recovery/staged-speed evidence is absent.

E270 fast_start_hold54_01 stops at689514us on CycleTiming12. Guard sample
step3:686170->689501=3331us, below3333us floor; recorder minimum3336us is
different. Rawpeak494,busmin11092mV,queue2,DMA18us,stack4232,finaloff.
Failure is a software cycle floor, not proof of physical maximum or desync.

There is also a distinct prospective recovery-arm boundary: observed final
seed age212half-us plus minimum64half-us requires the AM32 wait expression
ci/2-floor(ci/4) >=276half-us (using integer divisions). For ideal regular
intervals this is approximately302eHz, before allowing any age variation.
This is a derived scheduling constraint, NOT a measured motor-speed limit.
Merely widening the cycle guard cannot remove this independent arm floor.
Before a coordinated next range profile, inspect acquisition/cleanup latency
and retain the actual-age/arm-cost checks; never refresh the seed timestamp.
Current raw guards are measured, but mean current remains uncalibrated.

## E263 current instrumented-build evidence

Replayed complete captures through current handoff/recovery validators:

| Capture/build | Mean eHz | ADC scans/s | COMP/COM/commit max bracket us | Arm remainder/cost us | Untouched stack |
| --- | ---: | ---: | --- | --- | ---: |
| E262 adc_phase_hold45_30s_01,phaseDMA142AEAC3 |234.442|4975.093|115/79/45|67.5/13|4420 |
| E258 early_trigger_reentry45_30s_01,no-phase314A45 |234.370|4975.106|100/73/39|76.5/14|4376 |

First is30s initial hold, second~28s recovered segment in30s original budget
(164us spare). Different capture paths/builds; not a controlled overhead A/B.
DMA handler maxima17us with phase histogram versus11us without. These are
observed wall brackets including preemption, not exclusive instruction cost
or worst-case proof. No max-times-rate CPU-utilization calculation is valid.
Foreground partition is not idle, and exception/probe cost is not removed.

Old foreground285eHz results do not qualify this added-instrumentation build.
Current phaseDMA recovery and staged-speed timing remain to be demonstrated;
no guard relaxation is justified by these maxima alone.

## E245: distinguish the guard decision from recorder timing

`cyclefault_range54_01.txt` captures actual guard timestamps: step1,
158616->161947us =3331us, below3333us floor. Stop at161956us follows9us later.
Recorder minimum is3348us,not3331: the guard samples first and the rejected
event never enters the recorder. E243's3329us recorder minimum must likewise
not be called the exact rejected interval. CYCLEFAULT now records that exact
value on failures,with host wrapping/profile validation; legacy absence is
unknown. This is a software guard boundary,not proof of physical rotor speed.

Same build passes a5.3%10soriginal-budget injected-loss recovery at284.916eHz,
arm41.5us/cost13us,deadline93usspare,CPUunionfault0 and finaloff. No guard
widening or calibrated mean-current claim. Installed6406EA73; E245lab details.

## E242: recovery margin measured at 278 eHz

Shared acquisition plus direct diagnostic routing and masked two-port gate
checks passes three5.2%10soriginal-budget campaigns with2s tracking-loss
injection. `archive_mask_reentry52_01..03.txt`: recovered278.26..278.74eHz,
fresh-seed arm margins45.5..47us against32us floor,armcost13..14us against16us.
Acquisition max gaps83..85us against100us; prior shared/unmasked build failed
at101us. _01 uses the same1212tick interval as E238 but actualage210 instead
of246half-us:18us gained,measured. Do not attribute the entire saving to just
one of the report-routing/shared-loop/masked-check changes.

Full reentry verifier and timing reporter pass all3;originaldeadline spare
95/114/174us,stackuntouched4768. No current/CPUutilization claim. E241's
460C0BD8 ELF is unchanged,now bench-tested after USART3 BRR/CR1 restoration;
underlying console initialization fault remains unresolved. See E242 lab log.

## E239 offline recovery-path change (not bench-qualified)

E238's refused 5.2% recovery has seed1212, actual age246 half-us ticks:
303 - 246 = 57 ticks remaining, versus the unchanged64-tick arm floor.
At that identical seed, at least3.5us must be recovered merely to reach the
floor; this is arithmetic, not permission to reuse a stale measured seed.

Disassembly of installed ELF63801C85 confirms `reacquire` copies a64-byte
report and26-byte failure snapshot into the recovery archive, then restores
both originals AFTER acquisition returns (0x0800a39e..0x0800a3fa).
New const-generic diagnostic routing writes the existing recovery destinations
directly, leaving the initial report/failure/edge untouched. Original baseline
timestamps and all acquisition/arm guards remain unchanged. E238's57us entry
age also includes20us confirmation dwell, qualification, cleanup, caller
budget work and the entry marker; do not attribute all57us to these copies.

Both release/s/thinLTO configurations compile;189Python and154Rust tests pass.
Those tests do not directly run this firmware diagnostic routing. Before a
powered trial, test archive isolation and refusal paths with outputs disabled.
Then compare actual SEEDLAT/CORESEED, archive preservation, stack and complete
recovery verification. No measured latency improvement or new operating-point
qualification yet. New reacquire inlines acquisition into a492-byte frame;
the old148-byte wrapper had a separate nested acquisition frame, so their
individual frame sizes are not a whole-stack comparison.

Root experimental ELF DA6B5F4B45EA0A4CEF98BC8F9BD86438D9C97FB74423B7538193732C4E3EEF79:
text102776,data964,bss28740. Flash grows1708bytes; static RAM unchanged.
No flash/UART/motor action in E239; installed image remains E23763801C85.

Reproduce from unchanged raw captures (no hardware access):

```text
python scripts/drv_timing_report.py --reentry captures/driven_reserve45_01.txt captures/driven_reserve45_02.txt captures/driven_reserve45_03.txt
```

The reporter first runs the complete driven-handoff/recovery verifier, including
CRC records, original seed versus recovery seed, electrical limits, original
deadline and final outputs-off. It reports each input SHA256. Known E228 late
recovery remains rejected; timing summaries must be singleton, complete and live.

Firmware: shell-pwm, bench-driven-handoff, release, opt-level="s", thin LTO,
one codegen unit. ELF SHA256:
`6A2B4D5C7326643F258B7F5308C1132CC80CC2B0EA5E74DABCCDFDF0BE426100`.
These are E229 measurements, not new motor runs or opt-level="z" qualification.

| Recovered segment | 01 | 02 | 03 |
| --- | ---: | ---: | ---: |
| Mean electrical Hz | 235.025 | 235.610 | 235.582 |
| COMP calls/s | 12834.7 | 12828.7 | 12845.3 |
| Completed ADC scans/s | 4208.3 | 4204.9 | 4200.9 |
| COMP measured wall bracket max, us | 82 | 82 | 82 |
| COM measured wall bracket max, us | 55 | 55 | 55 |
| Guard measured wall bracket max, us | 20 | 4 | 4 |
| Commit measured wall bracket max, us | 21 | 21 | 21 |
| Recovery arm spare above 32 us floor | 3 | 2 | 3 |
| Recovery arm cost spare below 16 us maximum | 2 | 3 | 2 |
| Finish before original deadline, us | 145 | 198 | 232 |

Initial under-drive arm spare is 24/25/25.5 us: distinctly more than the
passive recovery path. Faster recovery is therefore a separate qualification
problem, not something initial-handoff success establishes.

## What this does not measure

CPU utilization is unknown. Source review of core_bench::comp_interrupt shows
the timer bracket starts AFTER dispatch/rate-limit checks. Early returns are
not bracketed. core_bench::interrupt similarly starts timing after COM status
checks. Both omit root vector dispatch and exception entry/return. A preempting
guard can also contribute to a bracket's wall time. Commit timing is nested
inside control work; adding those maxima double-counts work.

Multiplying 12.8k calls/s by the 82 us maximum would exceed one second per
second. That is neither measured saturation nor a valid headroom estimate:
it substitutes a maximum for every call and still omits other work. Do not
derive a CPU percentage or authorize faster operation from this product.

The next runtime instrument needs whole-vector entry/exit accounting for all
active motor IRQs, nesting-aware exclusive totals, overflow/clock-gap checks,
and measured instrumentation overhead. TIM17 is the available timebase; a
16-bit difference cannot certify long blackouts. Aggregate counters should be
dumped only after stopping under fixture capture, not streamed during drive.
Foreground ADC work is real CPU work too: IRQ occupancy alone is not total
utilization. Qualification must compare instrumented and unchanged builds at
the proven point before using instrumentation to expand the envelope.

Current remains raw peak evidence, not calibrated mean phase or supply current.
The 800 mA supply setting does not replace that measurement. None of this
report widens guards, changes minz control, or declares independent qZC parity.

## E231: optional runtime instrumentation implemented, not flashed

`bench-cpu-timing` wraps software bodies of TIM6 guard, COMP, COM, TIM3 forced
drive, TIM7 polling and optional ADC DMA IRQs. RAII exits cover early returns;
serialized TIM17 reads partition nested time without counting it twice.
Context0 is foreground/unattributed, then1..6 follow the order above.

The meter resets with powered statistics, starts at the first powered guard
tick (excluding acquisition and the initial guard period), and freezes when
powered ownership stops. Recovery resets it: CPU85 describes the last segment,
not the original segment or full campaign. Stop may occur inside a nested IRQ;
the open-frame depth is recorded and subsequent exits cannot change totals.
Visible sampling gaps>1ms, bad nesting and duration/counter overflow invalidate
the measurement; the meter never changes motor guard decisions. A16-bit timer
still cannot detect a blackout that aliases across complete65.536ms wraps.

Fixture-only CPUMETER/CRC CPU85 records are decoded by drv_cpu_meter.py, now
called by the sustained reporter whenever present. Seven contexts must sum to
elapsed time; invalid measurements are flagged, malformed records rejected.
Foreground is not idle, probe cost is not subtracted, hardware exception cost
is not separately measured, and no total CPU-utilization claim is made.

Idle-only `cpucheck` measures256 software enter/exit pairs in inactive, active,
and nested modes with gates/ENABLE already disabled. It never commands gates.
These are calibration helpers, not executed measurements yet. Before any
motor experiment, measure their cost on the board, inspect the added guard
stop latency, and qualify a finite known-duty run against E229. Optional
wrappers also cost time while acquisition is active even though accounting
has not started: check handoff performance rather than assuming zero impact.

Build: `cargo build --release --example shell-pwm --features bench-driven-handoff,bench-cpu-timing`.
Normal builds exclude the meter. No minz control, electrical thresholds, duty
ceilings, original deadlines or recovery-arm floor were changed.

## E232: idle probe overhead fails the initial qualification gate

Flashed and measured with outputs/ENABLE disabled; no motor commands. The
predeclared gate was fault-free measurements and maximum<=10us in all three
modes. `drv_cpu_check.py` retains all UART bytes and final outputs-off evidence.

| Capture / build | Inactive mean/max us | Active mean/max us | Nested mean/max us |
| --- | ---: | ---: | ---: |
| cpu_overhead_01 / 5CE5A70A... | 2.3125 / 3 | 8.3125 / 9 | 16.75 / 17 |
| cpu_overhead_03 / C43B672B... | 2.15625 / 3 | 8.21875 / 9 | 16.5625 / 17 |

Each mean is sum/256. Nested mode contains two enter/exit pairs. Both fail the
stated10us gate; do not relabel them as passes by changing the denominator.
The second build uses explicit hot-method inlining and a nesting bitmask in
place of scanning the active stack. The improvement is negligible for deciding
whether to deploy this probe. Disassembly still shows substantial bookkeeping
and stack spills in Scope Drop; a lighter accounting design is the next step.

All accounting faults were0, final gates/MOE/CCRs/ENABLE0 and nFAULT1 verified.
`cpu_overhead_02.txt` is a UART-silent preflight failure, not an overhead sample.
Before the known UART clock workaround, readbacks were RCC08000000, PD1ODR0,
BDTR00000c1a and CCRs0. UART then recovered; no motor drive was attempted.

Installed/root experimental SHA256:
`C43B672BC2181BC72304B822B836D41DF3EDC85A7B2F594DB667703CC2026C11`.
This image is NOT motor-qualified. The software-body occupancy measurement
remains unperformed. Do not extrapolate total CPU utilization from idle probe
costs or deploy this probe in faster runs. All original motor guards remain.

## E233: specialized vector bookkeeping improves cost, still fails gate

Const-generic Scope<ID> removes runtime identity and dynamic exit indexing;
repr(C) places frequently accessed scalar state in Thumb immediate-offset
range. Capture cpu_overhead_05: inactive520/256us,max3;active1520/256,max6;
nested3024/256,max12. Improvement from17us to12us is real but not a pass.

Packing six nested IDs into18bits and charging a validated leaving context
directly retains identical partition semantics. cpu_overhead_07:
inactive512/256,max2;active1528/256,max6;nested3064/256,max12. No further
material saving. All faults0 and complete captures verify finaloutputs-off.
_04/_06 are UART-silent preflight failures, recovered only after exact safe
register checks and the established clock workaround. No motor commands.

Current installed/root SHA256:
`7B09BD5259A2C07AAA09A8B103D9336F9B6CA5F6E47A040295441FCA76223585`.
Text99580,data956,bss28784; release/s/thinLTO. Not motor-qualified.

Further instruction-level tuning is not the next useful experiment. A separate
IRQ-union meter could charge time only at outermost boundaries, preserving
nested exclusion without seven-way attribution. That can answer aggregate
IRQ occupancy with less probe cost, though foreground still is not idle and
total utilization remains a separate question. Such an alternative needs its
own explicit protocol and correctness/overhead qualification; old CPU85
records must retain their original meaning. No operating guard changed.

## E234: aggregate runtime measurement landed

`bench-cpu-union` implies the optional timing feature but selects a distinct
two-context accounting implementation. Outermost entry/exit charge foreground
or IRQ-union time; nested identities are still validated, but their timestamp
reads and time updates are unnecessary. IRQ-union time includes nested work
once. The host distinguishes CPUUNION/CU85 from legacy CPUMETER/CPU85 and
rejects mixed protocols. Foreground remains work/unattributed time, NOT idle.

First aggregate idle capture cpu_union_overhead_02 measured maxima2/7/11us,
still a failure. Omitting nested timestamp reads yielded _03 maxima2/7/10us,
means1.5625/6.375/9.875us: passes the unchanged initial overhead gate. The
CPUCHECKTYPE union=1 marker establishes which implementation was measured.
_01 is retained UART-silent preflight, not an overhead result. Final build's
disabled transfer8/8, post-stop18/18 and recorder12/12 checks passed.

Same known4.5% duty/+60degree under-drive entry, first1s then10s finite holds:

| Capture | Mean eHz | COM / accepted | IRQ-union us / elapsed us | IRQ fraction | Raw current peak | Bus floor mV |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cpu_union45_01 | 233.545 | 1401 / 1400 | 522718 / 1000008 | 52.271% | 347 | 11151 |
| cpu_union45_02 | 234.449 | 14067 / 14066 | 5278743 / 10000010 | 52.787% | 359 | 10937 |

Both complete the requested powered window and full handoff verifier, with
valid CRC/partition sums, fault0, maxgap95/97us, nesting2 and finaloff/nFAULT1.
Calls22375/224901; untouchedstack5160. Initialarm remaining49/36.5us, cost12/13.
Ten-second cycle sigma41.10us, COMP bracket85us, COM58us, guard22us, commit24us,
36303ADC scans. These are instrumented-build observations, not proof of zero
instrumentation disturbance versus E229. No recovery injection in these runs.

Installed/root release/s/thinLTO SHA256:
`CAD4FB0C7AE48939A377AA94F8EC44E5AB0B10387C83061B01B874A840880AC1`.
Text99728,data956,bss28740. User PSU setting remains800mA; no new PSU-current
reading. Raw peaks do not become calibrated mean current through this probe.

The IRQ fraction is a measured software-boundary partition INCLUDING probe
effects. Do not call the other47.2% spare CPU: foreground ADC and control work
occupy it. Idle overhead calibration is recorded separately, not deducted
from CPUUNION; probe_cost_measured=0 in that stream means no calibrated
correction is supplied. Exception overhead is not separately accounted.
This advances timing evidence but does not qualify faster drive, independent
qZC, calibrated current, or instrumented recovery. Next work should progress
those campaign requirements rather than endlessly refining this instrument.
