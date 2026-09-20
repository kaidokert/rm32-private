# Recovery post-edge budget — E423

## E630 actual-duty retry snapshot staged

Prior turn made progress: current911A8% recovery pass plus precise high-speed
handoff constraint audit. This turn implements one necessary lifecycle fix:
resume_once no longer takes the caller's original duty. It snapshots stopped
POWER_DUTY after existing fault/output/archive admission and BEFORE prepare/
observe_begin can reset it. Both200/300us-reserve callers use the same path.
It refuses unsupported40..100 recovery range with result8 before prepare/wake,
instead of clamping or accidentally using a stale initial duty. High-duty PWM
preparation is NOT implemented by this change, and retry-session live updates
remain blocked in live_duty_current. No guard/deadline/seed changes.

scripts/test_recovery_duty_snapshot.py compiles actual resume wrapper with
hardware/guard seams:3Rust testsPASS, iterating all61 supportedduties, reset
preservation, original-end accounting, invalid0/39/101/200/300/301/MAX refusal,
abort and missing budget. Mock boundaries are explicit; not MCU timing or full
recovery proof.38 existing event100/report guard-policy testsPASS.

Full baseline-feature release-s build overflowed256B. No stale download.
Build retaining current-epoch but omitting optional current-baseline fits under
release-s/thinLTO/codegen1. No fast-coast selected (source opt-in remains).
Frozen captures/reference/recoveryduty_630/shell-pwm.elf SHA256
`71BCD7C67134129E8E44907F9ECFBD009C55A8BB1303B7763973E1FEB6F07052`;
emitted arithmetic audit retained adjacent. NOTflashed, no UART/SWD/motor calls.
Installed911A last verifiedoff E629; root71BC differs and carries no bench
qualification. Do not transfer either911A's20%cohort or its8%recovery pass.

Further source review: high-duty prepared CCR state is revoked by the existing
acquisition cleanup. A staged preparation token cannot silently survive that
cleanup. Next implement a coherent disabled-PWM/ownership boundary and shorten
the final-edge work; preserve current duty, one attempt and original deadlines.
HIGH_SPEED_RECOVERY.md updated with this distinction. Goal unfinished, no new
operator authorization blocker, no automatic threshold increase.

## E629 current8% recovery and high-speed protocol constraint

Prior turn was progress: measured passive near1kHz corroboration, retained
Current5 failure and restored911A. This entry used installed911A unchanged:
one drv_driven_handoff --ms30000 --drive-duty61 --bemf-duty80 --phase-shift60
--core-trace0 --pwm-roles --carrier-hz20000 --dma-peer --cycle450 --seed400
--seed-timing-reanchor --fast-cycle-report --event100 --dma-guard --dropout
--reentry run. Capture `captures/recovery_629_80_30s.txt`, SHA256
`0A3BE91FCB5C28A86FE76BC82D6BD6AC3B74435F7FDF6DC36F39C78911EFBBCA`.

First injected loss at2000051us, first Tracking8stop2000909us,4630COM/4629
accepted. Fresh acquisition5390us,12intervals/7cycles(5144..5310ticks),seed871.
CORESEED edge_age154ticks,remaining64ticks,arm6us: exactly the32us minimum
remaining, not extra margin. Resumed27.992068s,65193COM/65192accepted,
388.160746eHz,cycle sigma26.658us,raw338,bus10817mV,DMAmax40us/queue2.
Final elapsed34710978 vs original end34711219:241us before original deadline.
Final powered reason0/stop0 is reference-window completion and clean stop,
not an electrical fault; strict recovery verifier accepts real first-segment
Tracking8/fresh seed/timeline/CRC/finaloff. Single current-build pass only.

Source/math audit in HIGH_SPEED_RECOVERY.md identifies distinct blockers in
the protocol (not a request for operator approval): seed range, infeasible
current edge-to-arm budget at1k, intentionally disabled live updates in recovery
because original duty is reused, and fixed2s injection. Atci333 reference
wait83ticks=41.5us;20us confirmation+32us remaining already exceeds it. Actual
77us age cannot be fixed by seed threshold changes.3 tests compiling extracted
actual reference advance/wait routines PASS. No high-speed recovery attempted
through known-incompatible settings. No thresholds changed or timestamps faked.

Next stage cold setup before final qualification and design a bounded hot
handoff, or evaluate qualified unpowered reference tracking before granting
power; preserve current duty, original deadlines and all late refusals. This
is an implementation direction, not an achieved architecture. Current20%running
cohort remains separate;22-23%current mechanism still open. Actual911A off and
UART closed; root6A2E diagnostic differs. Full goal unfinished.

## E628 fast coast measured; independent near1kHz motion corroborated

E627 was progress (staged diagnostic and host timing contract). This entry
flashed exact6A2E after fresh prior911A off verification. Download/reset exit0.
Own guard3/18, fullfive3200CPU2/6/9, ADCphase3, IRQbudget5/0fail disabled
checks passed. Captures `fastcoast_628_{before_guard,guard,pulse,atomic,roles,
cpu,archive,adcphase}.txt`; IRQbudget MCP response synthetic50us boundary,
stopped_above1,disabled1. UART closed before every fixture/SWD session.

One --ms10000 --cycle450 --seed400 --fast-cycle-report --event100 --dma-guard
--fast-coast live_armed_baseline run without duty changes passed7% fixed:
20199COM/20198accepted,336.643eHz,rawpeak315,bus10901mV,finaloff verified.
Capture `captures/fastcoast_628_hold70_10s.txt` SHA256
`EC66F15D8048FFC34A558ED55D6BCB410F5A4DCBA002C5C8221CBAD8C67BD11C`.
All500 coast records measured128..129us spacing; first32 comparator scan
completion max82us (first row A60/B71/C82). Achieved cadence~7.8kHz, not an
assumed exact8kHz from the header. Early32 rows at7% contain too few periods
for coast_bounds's2period minimum; analysis refused, retained without rerun.

Then --ramp-duty220 --ms30000 with same profile flags ACK'd220 but stopped
Current5 at7026695us:22836COM/22835accepted,rawsummary707,bus10913mV.
This is a FAILED powered campaign, not a22% sustained pass despite successful
live ACK/fixtureexit0. Fastcoast500rows again128..129us,maxearlyscan82us;
CRC/timeline/finaloff validated. Capture
`captures/fastcoast_628_ramp220_30s.txt` SHA256
`8964B7E21E5A693740B980D065E90DBBC6557FBB27219C438D24AFDAE3596B00`.
Diagnostic build omits optional startup baseline; do not pool with911A cohort.

### Independent passive evidence, with explicit limits

After summarize/finaloff and fast-coast cadence validation, use
`drv_coast_periods.coast_bounds(text,30)`.30us is an explicit read/entry timing
allowance, not measured universal WCET or an analog-settling certificate.
Old analyze() intentionally still refuses noncompleted powered campaigns;
coast_bounds measures passive data separately and cannot turn Current5 intoPASS.

New pure period_spans uses the longest same-polarity edge pairs directly:
three full cycles divided by bounded elapsed time. No controller-rate input,
edge selection based on expected frequency, constant-speed fit, or extrapolation.
Temporal averages remain conditional on real comparator transitions and no
missed/additional edges. Results across the first~4.2ms of measured coast:

| Phase | First-polarity3cycle average bounds eHz | Other-polarity3cycle bounds eHz |
|---|---:|---:|
| A |939.850..1112.347|940.144..1112.347|
| B |954.502..1092.896|954.502..1092.896|
| C |954.198..1092.896|917.151..1044.205|

This independently corroborates rotor motion around1kHz and is inconsistent
with a simple500Hz rotor/1kHz controller interpretation under those premises.
It does NOT prove exact shutdown speed, whole-run tracking, calibrated qZC,
or exclude every possible waveform/alias mechanism. Single-period sensitivity
at10/20/30us allowances also retained in tool output;30us extrema786..1541Hz.
5coastperiod testsPASS (synthetic full-cycle spans and real Current5-not-pass),
2fastcoast testsPASS. Do not add more coast instrumentation merely for decimals.

### Current fault and final state

Post-stop SWD captured GUARD/STATE/QUEUE while off. Latest hardware ring
[1679,2079,3299,1211,1503] maps logicalA3299/B2079/C1679. A exceeds3248 by51;
do not conflate this single-phase excursion with E625's opposing two-phase
excursion. Bus/VREF raw1211/1503. Rejected scan excluded from summary peak707.
DMA count34959,queueempty/failedfalse; guard last_poll7026606,last_feedback
7026401(age205us),lastaccepted7026321/step5,guard/events faultNone. Global
Current5 came from immediate phase predicate, not freshness. Final accepted
interval204us, then no new accepted record before stop; no observed453us
accepted interval in THIS fault. Timing/current causality remains unresolved.
Raw `captures/fastcoast_628_current_swd.txt` SHA256
`89E199148DDCFF9E176E1CCB000090F8D7E19BC918540CEB74614F27405F15E5`.
GDB resumed then remote shutdown(exit1 after prints), OpenOCD exit0.

Restored exact frozen911A (download/reset exit0), fresh guard3/18/finaloffPASS
in `captures/fastcoast_628_restore_guard.txt`; UART closed. Root6A2E/source
fast-coast work remains, differs from installed911A.20%911A3/3 history retained,
22% repeatability and high-duty recovery incomplete. Next target recovery
timing and higher-speed current/acceptance behavior, not hardware blame or a
threshold increase. Goal remains unfinished; no new authority blocker.

## E627 staged fast passive coast diagnostic

Previous turn made progress:3/3 twenty-percent campaign cohort plus22% pass,
and identified the independent coast sampler's alias limitation. This entry
stages a post-stop-only diagnostic, not a new powered-control instrument.
No UART/SWD/flash/motor commands; actual911A remains last verified off E626.

Source: legacy comp_read uses asm::delay(3000), observed~142us per mux read.
flying_bench acquisition already uses TIM17-measured10us settle. Reuse that
budget (not an independently measured analog settling certificate) in opt-in
bench-fast-coast. Nominal coast cadence8000Hz,500records (~62.5ms). Retain
four ADC channels (VSENC,neutral,bus,VREF),all three comparator phase levels,
first32 per-comparator completion brackets, and every scan's measured CTIME.
No sensor caching or invented timestamps. Each fast scan checks ENABLElow
and powered_timer.outputs_disabled before acquiring. It never enables gates.
Legacy standalone comp_read calls keep the3000-delay behavior.

Shared noninline comp_read_settle(code,fast) reduces duplicated helper code.
Initial generic helper build overflow448B; shared helper overflow384B with
full911A features. Rather than change opt-level or remove safeguards, this
diagnostic omits optional bench-current-baseline (startup zero-stat capture),
retains bench-current-epoch and all running guards/ADC-DMA safety publication.
That changes startup observational work; its powered results require their own
checks and are NOT a transfer of911A qualification or an identical-image A/B.

Release-s/thinLTO/codegen1 build succeeds with E626 features minus
bench-current-baseline, plus bench-current-epoch,bench-fast-coast.
Frozen `captures/reference/fastcoast_627/shell-pwm.elf` SHA256
`6A2E46DFC7E6BE47A5D1F6025F1C346AB5AAF8EE080244BF3120D6E33AB968B5`.
Emitted math audit adjacent. Candidate NOTflashed; no stale-build download.

Host `drv_fast_coast.verify` requires explicit --fast-coast for8000Hz,500
measured timestamps,actual gaps125..150us,ordered32 comparator-bracket rows
with completion<125us. These are diagnostic admission checks, not motor-guard
changes or proof that hardware meets the cadence.2 new tests cover legacy
capture opt-in and synthetic boundaries/refusals;3 coast-period regressions
pass. Actual cadence/settling remain unmeasured. Firmware announces actual
selected sample_hz in existing COAST header, no added powered UART traffic.

Next: candidate's disabled preflights, a bounded baseline to measure fast scans,
then suitable high-speed passive coast corroboration. Keep no-missed-edges and
shutdown-vs-coast assumptions explicit; neither observer tests nor a nominal
8kHz header proves rotor lock. Original goal/recovery/23%current investigation
remain unfinished. Root6A2E differs from installed911A; all failed builds retained
as failures, no physical state change this entry.

## E62620% campaign repeatability and22% exploration

Prior turn was progress: real bench validation of IRQ safety publication.
Re-read goal and current state; frozen/root hash both911A. No firmware,
flash, guard, wiring or PSU-setting change this entry. Two additional20%
campaigns declared before running; neither excluded/retried. Same live fixture
command E625 with --ramp-duty200 --ms30000 and all explicit profile flags.

| Capture | Result | COM / accepted | Final tail eHz / cycle sigma us | Raw peak / bus min mV | DMA max us |
|---|---|---:|---:|---:|---:|
| E625 dmaguard_625_ramp200_30s.txt |PASS|150162 /150162|926.586 /30.303|760 /10913|52|
| dmaguard_626_ramp200_30s_02.txt |PASS|150043 /150042|923.755 /20.804|762 /10829|38|
| dmaguard_626_ramp200_30s_03.txt |PASS|150184 /150184|925.497 /28.366|815 /10877|47|

Thus3/3 thirty-second20%-capped campaigns, including startup/ramp. Not three
30s fixed20% holds or recovery qualification. Tail stats use26 overlapping
same-phase full-cycle windows only. All timeline/CRC/finaloff checks pass;
149253 contiguous summed frames each, with1/1/0 unaggregated safety-publication
tails explicitly reported. No calibrated current or full independent qZC claim.
IRQ union roughly72.77-72.90%, not total CPU utilization. FIFO peak3 each.

Then one --ramp-duty220 --ms30000 campaign completed30000032us,160883COM/
160883accepted, ACK220. Finaltail1002.236eHz,cycle sigma9.349us,min957/max1010
over26windows. Whole-campaign893.839eHz/sigma417.776us includes ramp: do not
use as stationary quality. Rawpeak838,busmin10889mV,DMAmax38us,queuepeak3,
IRQRATEpeak26/64. IRQunion22489519/30000009us=74.965%.149253summed frames,
1unaggregatedpublication. Stop reason2/deadline, off verified, UART closed.
One22% pass, not repeatability or recovery. No25/30 attempt this entry.

Source/evidence inspection of E62523% fault: accepted tail is available, but
drv_irq_trace.decode returns0rows on this trace0 image. The bench-irq-tail
feature alone does not make per-dispatch read instrumentation live. Do not
invent proof of preemption vs persistence rejection from absent rows.

Independent-speed evidence check: current bridge-off coast capture runs at
2000Hz. First32 scans record comparator completion offsets approximately190/
332/474us, after ADC reads and mux settling. At the22% final~1002eHz point,
that per-phase sample rate is at the alias boundary; the raw early VSENC and
flags alternate on successive samples. No defensible independent exact speed
or harmonic exclusion follows from that alone. Do not apply a no-missed-edges
coast estimator without establishing its premise, or extrapolate from later
coast decay to claim shutdown speed. Next select suitable fast passive sensing
using existing hardware (with bridge off) rather than add intrusive ISR traces.
High-duty recovery remains separately untested and seed/arm budgets need work.

PORTABLE_WINS updated with safety-vs-statistics split and stopwatch bracketing;
BEMF_PARITY_PLAN separates current20% campaign cohort from old32B1 recovery
and archived minz metrics. Full goal remains unchanged and incomplete.

New hashes (captures/):
- dmaguard_626_ramp200_30s_02.txt:
  `6714CB6D6F942CEA7C4F891770E988F8190225762462CC677E9D49CF25A20682`
- dmaguard_626_ramp200_30s_03.txt:
  `F79B7D1A37C630305F506A550A6C6D866D7759C3FD231F2CBF8C23A627F36B9E`
- dmaguard_626_ramp220_30s.txt:
  `2C4D8B57FFDB64E5B6C6C6555C4A91D3AA25B40893140B275661F18618FB58D5`

Actual/root911A, last outputs-off readback confirmed after final coast dump,
no live tool/process/UART handle left. Stop identical20% cohorts; pursue missing
independent corroboration/recovery and the retained higher-speed current event.

## E625 IRQ safety delivery bench test;20%30s campaign passes

E624 was progress: staged the architecture and tested its publication semantics.
This entry flashed exact frozen911A (dmaguard_624) after fresh prior-image off
verification. Probe-rs download exit0; OpenOCD reset exit0. Own guard3/18,
fullfive3200 CPU2/6/9, ADCphase3 checks, IRQbudget5 cases/0fail all passed.
Disabled captures `captures/dmaguard_625_{before_guard,guard,pulse,atomic,roles,
cpu,archive,adcphase}.txt`; IRQbudget via MCP returned boundary50/stopped_above1/
synthetic_elapsed1/disabled1/gate_authority0. MCP closed before fixtures.

Decoder now accepts explicit guard_irq0/1. Old mode still requires exact
sums=feedback count. IRQ mode separately reports unaggregated_publications,
bounded0..queue_peak+1 (FIFO plus one in-flight publisher/popped frame).
CRC, contiguous timestamp prefix and channel-window equality remain strict.
This bound is not proof of complete raw coverage: coverage_proven remainsFalse.
14 sum decoder tests pass including retained real old and new captures and
negative missing-tail/count/mode cases. Explicit fixture --dma-guard required.

Current image remains release-s/thinLTO, no further firmware changes E625.

| Capture | Outcome | COM / accepted | DMA max / FIFO peak | Raw peak / bus minimum |
|---|---|---:|---:|---:|
| dmaguard_625_hold70_10s.txt |7% fixed10s pass,337.188eHz|20231 /20230|38us /2|310 /10304mV|
| dmaguard_625_ramp250_30s.txt |Current5 at7.392150s,ACK23%,not25%|25017 /25016|38us /3|873 /11020mV|
| dmaguard_625_ramp200_30s.txt |20%-capped30s campaign complete|150162 /150162|52us /3|760 /10913mV|

All captures under captures/, startup61/initialBEMF70/carrier20k/seed400/
event100/report-only-fastcycle; hard30% live cap and original guards unchanged.
First fixed70 capture passes full handoff verifier. Ramp250 fixture exits1
because target250 never ACK'd; retains Current5 reason and final-off proof.
Do not call that a25% pass. Ramp200 fixture exits0; mixed-duty exploration,
not30seconds at20% or independent lock qualification. Raw peaks NOT amps.

20%-capped run final26 overlapping accepted-cycle windows: mean1079.231us,
926.586eHz,sigma30.303us,min1018/max1142. Whole-campaign834.269eHz and412us
sigma include acceleration and are not steady-state quality. IRQ union
21869644/30000008us=72.899%, not total CPU utilization. DMA52us remains below
201us scan period in this observed run; not a general WCET proof. No stale/
lease/overflow fault, queue peak3.149254 safety publications,149253 contiguous
summed frames; one tail publication explicitly unaggregated. All phase records,
wire CRC, timeline and final-off verified by exploratory decoder. No recovery
on this image, no30% attempt, no repeated cohort yet.

###23% fault postmortem, zero new runtime instrumentation

After UART closed/final off, SWD halted the already-off target and printed
GUARD/ADCSTATE/QUEUE, then resumed and shut down OpenOCD (exit0). GDB exit1
is intentional remote shutdown after valid prints, not missing evidence.
Raw output `captures/dmaguard_625_current_swd.txt`. DMA count36777, latest
hardware-order ring[434,2110,3612,1200,1508] maps logical A3612/B2110/C434.
Two phases exceed unchanged848..3248 limits, with opposing residuals+1564/
-1614counts. Bus/VREF raw1200/1508. Immediate current rejection precedes
feedback statistics, so printed peak873 does NOT include this fault scan.
This is a corroborated multi-channel excursion, not calibrated phase amps or
proof that switching artifacts are absent.

Guard last_poll7392107,last_feedback7391819:age288us,events.last7392015,
event faultNone/guard faultNone (global Current5 tripped before guard feedback).
Queue empty/failedfalse. Final accepted reported intervals mostly150..170us,
then213us and453us (last reference interval907half-us). Association with the
large current sample is observed; physical causality and exact aperture vs
accept boundary are not proved. No current threshold relaxation adopted.
Next investigate this late-accept/current event and establish20% repeatability,
not another hardware diagnosis or deadline waiver.

SHA256 captures, in table order:
`D79DD1BAFDC3B826278205013290F2CA24F20E3328ACDF3A6A852D27508C089E`,
`7E623F016F82F43293235A9D39BAED912EA7E9D22B24DB36D73F6E41C370E363`,
`E8D3FD0F9099B90D0FFD2F1AB59F1D8E174FD5187B5F738AE00AC7E956043456`.
SWD capture hash
`F0FF7BC20413BFAFF94DEBFE8BCC385626DBB154A23EF04FBBD0E02A423840F0`.
Actual/root911A, last final outputs off verified, UART closed. Full goal remains
unfinished; this is a measured architecture advance, not envelope completion.

## E624 staged IRQ safety delivery, not hardware-qualified

Previous turn made progress by preserving E623 evidence and current state.
This turn stages the architecture change directly addressing the demonstrated
foreground delivery dependency, rather than relaxing the1ms freshness limit.
No UART, SWD, flash or motor commands this entry; actual544B remains last
verified off from E623. Root now differs from installed.

New opt-in `bench-dma-guard` calls `stream_feedback(raw,acquired)` after the
existing coherent DMA lease/copy. It first retains immediate phase-current
checks, then runs the identical factory-calibrated bus conversion and complete
feedback validator, with original acquisition timestamp and existing expired-
gap refusal. Conversion remains preemptible by the independent guard timer;
publication remains serialized by feedback_inner's short critical section.
Only a healthy owner queues the scan. FIFO capacity/overflow stop unchanged.
Foreground drains all frames into raw sums, but does NOT re-publish an older
timestamp after DMA has validated a newer scan. No latest-only dropping.

Safety feedback count/peak/bus minimum now cover DMA publications; sums cover
foreground consumed records. An undrained tail can make those counts differ.
This is a declared semantics change that strict qualification verifiers must
account for, not a waiver of missing samples. Current fixture mode marker
`DMAFEEDBACK ... guard_irq=1` requires explicit `--dma-guard` selection.
No claim yet that current full fixed-campaign validators cover this mode.

`scripts/test_dma_guard_delivery.py`: two Python tests pass, one compiling
the actual publisher and conversion with only factory-read/guard integration
host seams;35 Rust tests pass including existing policy tests. New cases cover
publication acquisition-time identity across wrap, expired delivery/gap,
current/bus/VREF refusal, revoked owner. This does not simulate interrupts or
prove MCU timing. Event100/report suite separately38PASS.

Build command is E623's feature set plus `bench-dma-guard`, with
`--release --no-default-features`. First invocation omitted no-default and
failed conflicting portable-atomic features; corrected invocation overflowed
FLASH32bytes. A noinline experiment changed nothing and was reverted.
Shortening shell help text (no control logic) produced release-s/thinLTO/
codegen1 ELF SHA256
`911A3198CA48F7FD28B1FC108D19B90E525034DB327869DD03E7CB0BC11EC2EE`.
Frozen `captures/reference/dmaguard_624/shell-pwm.elf` and adjacent math audit.
It is NOT flashed. No stale failed-build download occurred.

Emitted DMA1_CHANNEL1 now has two `__aeabi_uidiv` calls at0x800204a and
0x8002146: the existing VREF division and bus /100 moved with conversion.
The change removes foreground safety dependence, not arithmetic cost. No
64-bit-free or WCET claim. Next review safety-vs-sums count interpretation,
run candidate's own disabled preflights and bounded low-duty DMA timing/
lease/guard checks before reattempting25%. Handler cost and sustained FIFO
drain headroom remain unproved; retain those risks explicitly.

## E623 live exploration through20%;25% foreground feedback deadline

Operator asked why work stopped: no new approval needed; continue within30%.
The previous12% Tracking8 stop was investigated using retained RAM with the
bridge already off, UART closed, and post-stop SWD. Old68F8 GUARD.events.fault
was TooFast (minimum238us), not a demonstrated loss of rotor tracking.
Capture `captures/tracking_623_live120_10s.txt`, SHA256
`57D9CDF7A72820E02A13F0DDC5C79CBB3119384A35D29308C11718587D1FA082`;
details `captures/tracking_623_guard_swd.txt`.

Explicit bench-event100 profile sets minimum event spacing100us. Order,
maximum event age1000us, slow-cycle6000us, electrical checks and seed400
remain unchanged. Fast-cycle2223us remains report-only. This is an explicit
exploration-envelope expansion, not a measured maximum-speed certificate.
38 event100/report guard tests pass; exact host profile opt-in required.

Intermediate frozen event100_623 ELF23C937B330BD256431DDE765492E87233413A5DA7514E24C9AFB844C031CA0B5
stopped TickGap3 while updating duty, measured122us, before ACK120.
Retain `captures/event100_623_live120_10s.txt`; not a12% pass. The stopwatch
included unmasked time before/after the transaction. Corrected it to bracket
the actual masked writer, preserving its100us limit and independent deadlines.
This identifies a measurement confound, not proof of which ISR caused122us.
Writer tests cover masked timing and101us refusal. Shortened only boot banner
to fit release-s after the change; no opt-z substitution.

Current frozen `captures/reference/liveclock_623/shell-pwm.elf` SHA256:
`544BBBF004F74A2180CDC22864E26E18DC4E6A10AB365A2573355A03F7CD6800`.
Release-s/thinLTO/codegen1, emitted math audit retained. Own guard/fullfive
3200 CPU2/6/9/ADCphase/IRQbudget preflights passed. No transfer of old recovery
qualification. All following runs use this image,20kHz carrier,startup61,
initial BEMF70,hard300 live duty cap. Raw current is NOT calibrated amps.

| Capture in captures/ | Outcome | COM / accepted | Final tail eHz | Raw peak | Bus minimum mV |
|---|---|---:|---:|---:|---:|
| liveclock_623_live120_10s.txt |10s campaign completed|30234 /30234|585|500|10877|
| liveclock_623_live150_10s.txt |10s campaign completed|35190 /35189|712|682|10889|
| liveclock_623_live200_10s.txt |10s campaign completed|44095 /44095|932|1139|10865|
| liveclock_623_ramp250_30s.txt |FeedbackStale4 at7.715279s|27190 /27190|not qualified|877|10674|

These are mixed-duty exploratory campaigns, not10s holds at their final duty.
Tail estimates use26 overlapping accepted-cycle windows; no independent rotor
measurement. IRQ union for first three ~55.73/58.16/63.11%, not total CPU use.
Capture hashes respectively:
`77F03EA6DB78CD9E1E08D4AA79AFC74885808E2E8F2CAB2DC675A25F30FBA5FA`,
`F8D64A4706F547C8D3910A636CBDAA0FC7BF0A6105D9A9C50E607EBC9DFA988A`,
`5A0DDD3F7AB345CEA2CA04522521170C7DBEA6BD65EEAB99404E02FC0526D8F3`,
`4247C5E569470DBD6943B6074E52D732A8983232180DC68208834F91D267158F`.

Ramp ACKs70,90,110,130,150,170,190,210,230,250;500ms minimum between
ACK-paced requests.25% was acknowledged but not sustained;30% not attempted.
Post-stop SWD `captures/stale_623_guard_swd.txt` SHA256
`EADA267285D5DDBBE41A8FFF99A6CF0340D4696CDAE139656EE16B63AF8F03CE`:
guard now7715253,last_feedback7714223,age1030us. Newest queued ADC acquired
7715027,age226us; FIFO head5/len3/peak4/failedfalse; DMA count38384/max20us.
Event last7715212,age41us,events.faultNone. Therefore fresh samples exist but
foreground guard delivery missed its deadline. Specific scheduling cause is
not yet proved. Do not call this ADC hardware failure or widen1ms to hide it.
Source audit: live reply transmits at most one nonblocking UART byte per main
pass; no flush/spin/formatting wait. Foreground converts and validates every
FIFO frame, then accumulates raw sums; reference observe_bands also runs each
loop. Next isolate delivery latency while retaining every electrical check.

Every attempt retains final-off verification and closed UART. Post-stop GDB
resumed the already-off target before OpenOCD shutdown; remote-close exit1
occurs after valid memory output, OpenOCD exits0. Installed/current544B remains
off, not a new sustained/recovery qualification. No hardware changes.

## E622 policy changed; sustained10%/481eHz achieved

Operator approved the fast-cycle report-only trial and then clarified that
ordinary measured tradeoffs toward30% do not need repeated approvals. The
old policy decision blocker is resolved. Keep genuine electrical/tracking
protections; do not reinstate tiny-step/approval bureaucracy.

RunGuard third const bool REPORT_FAST defaultsfalse. Only selected runtime
cycle450 image sets true: short cycles update segment-local saturating count
and minimum, then advance the original per-sector timestamp normally. Slow
cycles>6000 still latch CycleTiming. Event ordering/238..1000us, feedback,
current, bus, nFAULT, timing deadlines, seed400 and32us/16us arm checks remain.
FASTCYCLE r1 <count> <min_us> is post-stop report-only2223us-floor metadata;
host explicit --fast-cycle-report required, malformed/duplicate stats refused.
Report summaries identify the changed policy rather than implying enforcedfloor.

37reportpolicy +37defaultguard testsPASS including wrap/exactbounds,slowlimit,
fresh timestamp/reset,otherstops;2hostdecoder and4livewriterRustcasesPASS.
Initialrelease-s overflow32bytes, shortened reportmetadata only; finalrelease-s/
thinLTO/codegen1 fits and emittedauditcompleted. Frozenfastreport_622 SHA256
68F80A2FFC3BD28B5D7D03156D45A1EEFB3BEAA02675E10D55FE5F74738A5CCC.
Own guard3/18,fullfive3200CPU2/6/9,ADCphase3,syntheticIRQbudget5 PASS.
Download/OpenOCDresetexit0. No generalopt-z fallback.

All runs20kHz/startup61/seed400/cycle450report/reanchor/trace0. Results:

| Attempt | Outcome | Evidence |
|---|---|---|
| fixed90/10s | PASS438.238eHz |26293COM/26292acc,raw436,bus10937;140fast,min2201us|
| fixed120 requested | CLI refusal beforeUART | obsolete fixed-fixture10%cap;notmotorfailure |
| live70->120/10s | STOPTracking8 at3.409175s | ACK120,7004COM/7003acc,raw516,bus10984;267fast,min1702us |
| live70->100/10s | windowcomplete | ACK100,26400COM/26399acc,raw452,bus10937;19612fast,min1972us |
| fixed100/30s | PASS481.428349eHz |86655COM/86654acc,sigma34.840032us,raw512,bus10925;86518fast,min1983us |

Live results are mixed-duty exploration, not fixed-rate qualification.10%live
final26overlappingcyclewindows imply491.68eHz,not its whole-run440eHz mean.
12%last acceptedtail has roughly570eHz-like intervals; stop at3409152us is
219us afterlast recordedaccept3408933. Suggests early-event refusal, but no
rejected-event timestamp/subreason is recorded: do NOT claim provenTooFast or
physicaldesync. No injected dropout and no overcurrent/bus/driverfault reported.
Do not go straight from12%Tracking to moreduty without classifying it.

Fixed10%30sIRQunion=16386482/30000007=54.622%,not CPUutilization. Allfixed
captures strictCRC/ADC/phase/timeline/finaloffPASS. Live captures accepted
windows/transfer/stop/finaloff validated bylivefixture; not recovery passes.
Generalized existing livefixture --step-duty40..300; no newfirmware needed.
Fixedduty API and legacycarrierprepare still capped100; use livepathabove10%.

Captures/SHA256:
- fastreport_622_hold90_10s.txt:
  513B3E1237A3DB431FF0B3575DF7E00FD7064663D9A488B2F632E15A8EBB877E
- fastreport_622_live120_10s.txt:
  94A8EBD280934043B4AF3EDBDE521C387C63C90FBC811A25B4DA02F79F5A2221
- fastreport_622_live100_10s.txt:
  A891ECABAD79B6F3D54A8FA9746746D504E14035DCAB4202D7B10074F44AB154
- fastreport_622_hold100_30s.txt:
  CD2478824FFC6034BA99FF12DF18799E602C55627AEFCA6145BB2B569E84900F

Current68F8 installed/OFF,Uartclosed;source/rootmatch. Retainthis trialimage,
not oldpolicy. No calibratedamps ornewrecoverycohortclaim. Nextclassify12%
trackingstop,notanaloghardwareblame orfurtherpermissionrequests.

## E621 matched opt-z comparison: no gain, both fresh-arm refusals; retired

Reverted only E620 caller specialization to build original Option caller with
identical features/opt-z/thinLTO/codegen1 (NO prestart-dma). Frozen baseline
captures/reference/coastbaseline_621, SHA256
4303A151A0BA2DCFFEACCD7291766835EAB9DCD80B6586BC4521EA5A063FB6F8.
Candidate remains frozenE0D1. Both own guard3/18,fullfive3200,ADCphase3 and
nativeMCP IRQbudget5 checksPASS; CPU pair maxima baseline2/7/10us,candidate
2/7/9us. Every download/reset exit0. IRQbudget is synthetic elapsed,notWCET.

One commanded61startup/70BEMF/30s recovery per image, same20k/cycle450/
seed400/reanchor flags, no retries. Both initial drive reached injection;
both FAILED fresh recovery arm BEFORE second segment could drive:

| Image | edge age us | remaining us | SEEDLAT half-us stages |
|---|---:|---:|---|
| original caller opt-z4303 |97|28.5|96/120/140/176/182|
| specialized opt-zE0D1 |97|26|96/120/142/176/182|

Unchanged minimum32us correctly refused; remaining difference reflects fresh
seed interval, not a candidate speed gain. Both CRC/first archive/finaloff
verified by fixture failure handler. Full30s recovery FAILED,not a pass.
Baseline capture coastbaseline_621_recovery70_30s.txt SHA256
1CF636CD8711BDFFAE17C95CED2CEF405011F70D52D6DBBD8DF71F70BDDD0667.
Candidate coastreentry_621_recovery70_30s.txt SHA256
3F5794CF9DFB3C16FC05781BB96014A69E0FFDB5C2F4895063FA785B76CA51BC.

No measured benefit: retire caller specialization; source already restored
originalOption caller for baseline. Globalopt-z itself is unattractive here:
both97us versus retainedopt-s32B1's76us; not a controlled compiler-onlyWCET
claim, but sufficient to reject adopting these images. Do not repeat these
variants or loosen arm margin. Const specialization is not automatically a win.

Restored frozen32B1,download/resetexit0;coastreentry_621_restored32b1.txt
guard3/18/finaloffPASS,Uartclosed. Root4303opt-z baseline differs from board;
source originalcaller plus feature-gated E618metrology, defaultopt-s unchanged.

## E620 caller specialization candidate, not timed or installed

The ten-us guard bracket is not all reset work: live ready/ownership/output,
feedback and original budget admission plus timer activation remain necessary.
No moving these checks earlier on a stale observation. Instead applied the
E610 compile-time-mode technique one caller above: coast_run_inner now has
const REENTRY and Limits rather than runtime Option<Limits>. Four private
non-reentry callers pass false/unused zero Limits; only resume_once passes
true and the original SessionBudget::reentry_reserved result. All previous
resume.is_some branches now use REENTRY; start_reentry still consumes those
same limits. No edge-age, controller seed, deadline or guard changes.

Full function specialization duplicates substantial code. Release-s/thinLTO/
codegen1 failed FLASH by2528bytes. Per-command opt-z with same features as32B1
(NO prestart-dma) fits; audit SHA256
E0D10419D4749CE58EC1A5302EEA2631632E96CC99A47DF07DC58231C8A99DBC.
nm shows two coast_run_inner bodies:0x88c and0x878bytes, confirming distinct
specializations, NOT a speed gain. Frozen captures/reference/coastreentry_620
contains ELF and emitted audits.36 pure guard testsPASS, but do not execute
this PAC caller; caller/timing behavior still needs hardware qualification.

NO UART/flash/motor. Actual32B1 lastoffE619;root E0D1 staged. Before powered
use, own disabled preflights. A fair speed attribution needs an opt-z baseline
without this specialization, not just32B1 opt-s or E619's DMA-baseline opt-z
run. Consider outlining the shared post-arm loop if duplication is retained;
do not claim an opt-s flash overflow means the architecture is impossible.

## E616 successor candidate passed recovery but gave no timing gain; retired

Exact6A10 installed after32B1 guard/off. Own guard3/18,fullfive3200/CPU2,6,9,
ADCphase3,IRQbudget5PASS;download/resetexit0. One75/30s recovery under
unchanged cycle450/seed400/electrical/arm/deadline checks PASS361.735052eHz,
60753COM/60753accepted,resumed27.991532s,sigma22.298673us,raw335,bus10901.
Seed924,SEEDLAT88/106/118/138/142,edgeage76us,remaining39.5us,arm7us.
No measured improvement over32B1. CRC/ADC/timeline/finaloffPASS,Uartclosed.
Capture acquiresuccessor_616_recovery75_30s.txt SHA
F6C86642FCA2D42830BD3A7DB244AC2B66510865DF8B307E63700642FBBEE4EB.

Retired candidate without retry. Reverted only E614 successor source changes;
retained const-reentry improvement. Restored frozen32B1, download/resetexit0,
acquiresuccessor_616_restored32b1.txt guard3/18/outputsoffPASS,Uartclosed.
Root ELF remains retired6A10,notinstalled; future build must be checked anew.
No higher recovery qualification or guard change. Do not repeat successor
microvariants as a large latency opportunity; measured outcome is no gain.

## E614 bounded acquisition successor, staged; codegen limits explicit

Current acquisition already gives the final candidate an extra visit with
the mux held settled. No ADC scan occurs within that extra-visit branch;
skipping ADC there is not an available optimization.20us dwell unchanged.

Replaced three private-validsector `%6+1` calculations (final_candidate,
poll_filtered and edge order check) with inline const successor,1..6 mapping
exhaustively asserted at compile time; existing preparation hint shares it.
Private previous state is populated only by edge's valid phase/level mapping.
31 final-edge and30seed-profile testsPASS (host incremental AccessDenied note
nonfatal, build/tests exit0). No phase/dwell/deadline/seed arithmetic changes.

Release-s/thinLTO build and emitted audit, candidate
6A10FBC5873CAD10BC0143DEB79782FADF2A158E0908106017F043CF4209A3D0,
frozen captures/reference/acquiresuccessor_614/ ELF/audits,NOTflashed.
Correction to initial suspicion: old u8 remainder already emitted two MULS,
shift/subtract, not __aeabi division. Existing acquire_inner helper calls are
unchanged (ADC conversion/optional track path previously audited).
Edge symbol size0x1f4->0x1f0; local stack36->28bytes. acquire_inner size
0xa98->0xaa8. This is a mixed size result, not proof of meaningful time saved.
No hardware timing/qualification transferred; do not portray it as the large
software-division removal seen in earlier u32 arithmetic experiments.

Actual32B1 remains installed,lastoffE613,Uartuntouched. Root/source nowcandidate.
No guard changes. Any bench use requires own disabled checks; consider its
small expected benefit before spending another latency-only campaign on it.

## E613 8% recovery cohort:3/3, no excluded attempts

Two predeclared additional attempts on unchanged32B1, same30s original
deadline, startup61/BEMF80/phase60/20kHz/trace0/cycle450/seed400/DMApeer/
seedtiming with dropout+reentry. Both exit0, making3/3 with E612's first run.
All full fixture CRC/ADC/phase/timeline/finaloff checks PASS,Uartclosed.

| Attempt | eHz | COM / accepted | Cycle sigma us | Raw peak | Bus min mV | IRQ union % |
|---|---:|---|---:|---:|---:|---:|
|E612|387.511708|65084 /65083|21.414956|389|10793|53.761759|
|E613-02|387.489659|65080 /65079|20.518121|397|10960|52.713143|
|E613-03|387.192741|65030 /65029|21.265460|379|10805|53.192833|

All three freshedgeage76us,remaining33us,arm7us. Seeds871/872/873 ticks.
The cohort has only1us observed margin above the32us handoff floor; it does
not prove WCET, wider recovery envelope, or unchanged performance across
arbitrary conditions. No further identical80 repetitions planned.

New captures constreentry_613_recovery80_30s_02.txt SHA
FFDF8C356A064480E89945A3493A37D1FD26D3A2597E5D08A6AA7D3B6B5A2589;
constreentry_613_recovery80_30s_03.txt SHA
7ED7EA61DC5101B6E48102EC92874EF7EEBA260E3D45F988456C685B673239FA.
First attempt hash inE612. Current32B1/off; no code,flash,guard orPSU changes.
Higher recovery needs further latency reduction and eventual seed-range
qualification, independently of pending report-only speed-policy decision.

## E612 7.5% recovery confirms retained timing;8% attempt follows

Same32B1, one75/30s recovery startup61/phase60/trace0/20kHz,
cycle450/seed400/DMApeer/seedtiming. PASS361.814065eHz,60767COM/60766acc,
resumed27991662us,sigma22.277363us,raw349,bus10960,stack2664.
Final stop was foreground deadline COASTREF1,POWERPATH0, not a guard fault;
original deadline257us spare, CRC/ADC/timeline/finaloffPASS,Uartclosed.
SEEDLAT90/106/118/138/142 identical E611;seed926,age76us,
remaining40us/arm7us. Compare E60635.5us remaining/80us age atseed924;
observed improvement retained, not a WCET or3/3cohort claim.
Capture constreentry_612_recovery75_30s.txt SHA
AD33C31AF806D6099F7690D01FFD847DCA0BE6769C909F3F5DE12450AAEE376C.

This supplies new evidence for ONE8% recovery attempt under unchanged32us
remaining/16usarm/seed400/otherguards, not a retry of identical older code.
Capture constreentry_612_recovery80_30s.txt: now completed PASS,exit0.
Recovered387.511708eHz,65084COM/65083accepted,resumed27992132us,
sigma21.414956us,raw389,bus10793,stack2664,originaldeadline203us spare.
CRC/ADC/timeline/finaloff validated,Uartclosed. Seed871,age152ticks=76us,
remaining66ticks=33us,arm7us. This is only1us spare above32us minimum.
SHA3B32EBEB3FF1D2F37B23AF51161359D773639CD895D358AF4BC415596D7D6860.
This is the first8% recovery success here, not a cohort or proof of ample
margin. Next fixed repeatability attempts at80, retaining refusals; no85
recovery or seed/arm limit change. Current32B1 remains installed/off.

## E611 const-reentry hardware result: observed4us edge-age improvement

Installed exact frozen32B1 after current3A70 guard/off verification. Download
and OpenOCD resetexit0. Own guard3/18,fullfive3200/CPU2,6,9,ADCphase3 and
native IRQbudget5PASS,UARTclosed. No guard/deadline changes.

One70/30s injectedloss/recovery, same --cycle450 --seed400 --dma-peer
--carrier-hz20000 --seed-timing-reanchor, startup61/phase60/trace0:
PASS339.344474eHz, resumed27990733us,56991COM/56991accepted,
sigma22.710464us,raw342,bus10757,stack2664. CRC/ADC/timeline/finaloffPASS,
originaldeadline175us spare,Uartclosed. Capture
constreentry_611_recovery70_30s.txt SHA
82C293708ED7B3E94CD4F67388F93B9B56BFA37AD1B64EFFB3E2D6614165D6FA.

SEEDLAT90/106/118/138/142ticks;finalage152ticks=76us,seed1000,
remaining98ticks=49us,arm7us. Feedback->guard bracket10us versusE58813us
andE60614us. Uninstrumented prior age80us versus76us here. E60882us had
additional acquisition stamps and is NOT an equal instrumentation comparison.
Observed savings are encouraging but not a paired randomized effect estimate
or WCET. One recovery pass does not establish reliability at higher speed.

Current/root32B1 installed/off,normal (no acquire-timing) campaign. Next one
75recovery under same guards to check retained margin at preceding qualified
point before considering80recovery. E604 speed-policy answer stillpending;
this optimization does not change that policy or the seed400 boundary.

## E610 compile-time reentry specialization, staged only

Actual acquisition freezes completed Seed; existing final-edge hint is before
interval12. Do not implement later traffic as a timestamp refresh. No changes
to acquisition or its deadlines made here.

Instead staged a source-equivalent mode specialization in powered_timer:
start_inner<const PREPARED,const REENTRY> takes original Limits directly.
Three private callers select false/false,true/false,true/true. Initial callers'
zero dummy limits are unused; they retain reserved_elapsed and original
startup budgets. Recovery uses original elapsed/campaign/segment. Staged/
adopted/awake refusal expressions replace Option presence with const REENTRY;
feedback/output/owner checks and actual timer-install sequence unchanged.

36 actual guard-policy testsPASS;2source-contract/mode-algebra testsPASS.
These do NOT execute the PAC start wrapper or establish hardware timing.
Release-s/thinLTO/codegen1 buildexit0, math audit saved (advisory). Candidate
32B1188090EA7D70EE83B9CB4EBECBE2C4C63EB863B8B5D38FD8E2EF810475DE,
frozen captures/reference/constreentry_610/ ELF/audits. New emitted listing
has no standalone powered_timer::start_inner symbol; compiler inlining/layout
changed. No timing benefit claimed, and no prior qualification transferred.

NOTflashed; installed3A70 remains lastoffE608,UARTuntouched. Next candidate
disabled preflights and one70recovery timing comparison, not a higher-duty
attempt or a claim that this small specialization solves the whole latency gap.

## E609 guard-start source audit: GPIO grouping already present

No implementation change. powered_timer::outputs_disabled already samples
TIM1.MOE once and GPIOA/B once each with gate masks; the proposed six-pin
grouping optimization is already installed. start_reentry checks ready/reason8;
start_inner consumes staged/adopted one-shots, rechecks owners/outputs, validates
feedback and original limits, installs guard, then activates TIM6. Moving this
whole bracket earlier would require separate inert preparation and final live
admission/activation, not moving authoritative timestamps or eliding checks.

Examined emitted start_inner in root2F35 diagnostic at0800a460 (not installed
3A70). It retains conditional staged/adopted/refusal branches. Do not attribute
the entire14us measured bracket to duplicate GPIO reads or claim any removable
cost from source inspection. No new timing benefit found in this proposed
local optimization; no speculative firmware variant built or flashed.

Actual3A70 lastoffE608 remains unchanged. A larger recovery redesign would
need explicit lifecycle semantics and original-time admission tests. Pending
operator fast-cycle policy decision remains independent of that work.

## E608 existing acquisition timing instrument: publication is only3us

Added only existing bench-acquire-timing to current feature set, no source
changes. Release-s/thinLTO,math audit; diagnostic SHA
2F3548B188A7AE118CD60E3B2F1229DD46FC352390ADF959229FDDA8D573E219,
frozen captures/reference/acquiretiming_608/shell-pwm.elf. Preflash off/guard,
download/resetexit0, own guard3/18/fullfive3200CPU2/6/9/ADCphase3/IRQbudget5
PASS. Native MCP IRQbudget confirmed disabled/no output authority,Uartclosed.

One70/30s injected-loss/recovery using cycle450/seed400 and unchanged guards
PASS338.016241eHz,resumed27.990733s,56768COM/56767acc,sigma23.273912us,
raw364,bus10566,stack2632; freshseed997,age82us,remaining42.5us,arm7us.
CRC/ADC/timeline/finaloff verified, fixtureexit0. Capture
acquiretiming_608_recovery70_30s.txt SHA
06FEB928314C65B69E079635250CB10732859023C06DC6D84DA91252848BB70A.

Existing drv_timing_report.report(reentry=True) validates chronology:
edge->qualified31us; qualified->clear6; clear->publication3;
publication->return2; return->coast entry5. Then entry->reset9,
reset->feedback5,feedback->guard14,guard->reference2,reference->arm5.
Total82us. Brackets INCLUDE instrumentation and adjacent work; not exclusive
costs, removable-delay proof or WCET. Required20us qualification dwell is
unchanged; the31us bracket must not be read as31us disposable computation.

Publication alone cannot be assumed to yield the10us target at410eHz.
This rules out that proposed direction as sufficient by itself. Next target
larger setup scheduling with stale-state/ownership checks preserved, rather
than spend another variant removing a three-microsecond report copy.

Restored exact3A70 frozen cycle450_601: download/OpenOCDresetexit0 and
acquiretiming_608_restored3a70.txt guard3/18/finaloffPASS,Uartclosed.
Root build remains2F35 diagnostic, NOT installed. No controller, guard,
reference-core or persistent configuration change. E604 policy decision pending.

## E607 recovery planning: seed qualification and edge age are distinct

Read actual observation_reset/start_inner/acquisition-return path. Guard setup
still needs live owner/output checks, original feedback-age admission and timer
activation; these cannot simply be labeled cold data. Acquisition exit also
reduces/publishes reports and clears hardware before returning the fresh seed.
No claim of measured attribution beyond E606's stage brackets; no speculative
reordering implemented. Earlier nine-store preparation failure remains valid.

Added host-only drv_recovery_budget.py with exact integer wait arithmetic
(advance16,half-us ticks,32us remaining floor) and separate seed400 predicate.
3testsPASS,including conversion/rounding across1..2000eHz. Constant-speed
synthetic intervals only; not rotor measurement, qualification or WCET proof.
With observed80us age:387eHz needs4us reduction;410 needs10us;450 needs19.5us;
600 needs42.5us.410+ also exceeds current seed400 profile independently of age.
The arithmetic is a planning target, not authority to widen seed qualification.

Next implementation should target acquisition-exit/report/setup scheduling
with original timestamps and final checks retained, rather than expect another
one-store tweak to cover tens of microseconds. A higher-rate seed profile would
still need explicit policy tests and its own hardware timing evidence.
No firmware/build/flash/UART/motor; actual3A70 lastoffE606. E604 decision pending.

## E606 current-build recovery baseline, one pass

Independent work while E604 speed-policy decision remains pending. One
current3A70 run with --ms30000 --drive-duty61 --bemf-duty75 --phase-shift60
--core-trace0 --pwm-roles --carrier-hz20000 --dma-peer --cycle450 --seed400
--seed-timing-reanchor --dropout --reentry (CLI arguments separated normally).
No firmware, guard or flash changes. Existing disabled checks fromE601.

Startup, injected tracking loss, shutdown and one fresh measured reacquisition
PASS. Resumed27991436us within original deadline;361.159273eHz,
60656COM/60656accepted,cycle sigma22.413247us,rawpeak317,busmin10662mV,
IRQunion52.099212%,stack2656. Full fixture CRC/ADC/phase/timeline/finaloff
exit0,Uartclosed. Original end34710775us versus final34710611us.

Fresh seed924ticks, age160ticks=80us,remaining71ticks=35.5us,arm7us.
SEEDLAT90/108/118/146/150ticks; actual acquisition handler24us,overruns0,
discardedfault0. Margin above unchanged32us floor is only3.5us; the wider
running profile does not enlarge recovery's reference-predicted arm window.
Do not retry the known8% recovery-age refusal without changing that path.

captures/cycle450_606_recovery75_30s.txt SHA
2A8D854170773101BAB6EFBF9D40D237EB79A83AEA03A83ACB9E245A46594C5A.
One current-build recovery pass, not3/3 and not recovery at8/8.5%.

## E605 live host support for existing cycle450 profile

While E604 policy decision is pending, completed an independent tooling gap:
live_armed_baseline accepts --cycle450, verifies exact2223/238/6000/1000
profile, retains separate seed400 selection, and rejects conflicting running
flags before output-file creation or UART access. Startup pacing, live duty
commands, ACK checks, original10s deadline and firmware are unchanged.
The script no longer requires a post-run manual workaround on current3A70.

2profile tests,2live-line tests and1writer harness (4Rustcases) PASS.
This is host-only validation, not a new live-control hardware qualification.
No UART, motor, flash, firmware or guard change; last off evidence E603.
Automatic continuation did not answer the requested fast-cycle policy choice.

## E604 separate experimental speed veto from independent stops

Source audit of powered_guard and accepted_timing: independent accepted-event
monitor enforces order and238..1000us intervals; poll detects missing progress.
The single-cycle2223us minimum adds a distinct speed-envelope constraint;
it is NOT duplicate electrical protection or independent rotor-lock evidence.
The6000us slow-cycle limit is a separate side of the same CycleTiming branch.

Host-only counterexample added to cycle450_guard_test.rs: ordered278us events
(roughly600eHz) pass Monitor yet stop RunGuard on its seventh event.36 actual
policy testsPASS. Thus removing the fast-cycle veto expands admitted speed;
do not claim existing tracking checks preserve identical protection or catch
every false/harmonic lock. Event minimum alone implies1428us per six events
(approximately700eHz accepted-rate bound), NOT permission to run that fast.

Proposed operator decision: make ONLY the fast-cycle experimental floor
record/report rather than kill; retain slow-cycle, event-order/age/minimum,
electrical, ADC freshness, dispatch, finite deadline, duty30% and PSU800mA
protections. Keep seed400 and32us/16us handoff checks unchanged. This is a
deliberate policy change, not a semantics-preserving fix. Do not implement it
from advisory memo alone; request explicit direction because prior handoffs
specifically rejected record-only CycleTiming under the retained-guards goal.

No firmware edits/build/flash/UART/motor; installed3A70 last-offE603 unchanged.
Only host audit test/documentation changed. Full goal open; no blocked status.

## E603 9% reaches the next experimental running ceiling

Same3A70, one30s request startup61/BEMF90,20kHz,phase60,trace0,
cycle450/seed400/DMApeer/seedtiming. CLI exit1 CycleTiming12 at201596us,
493COM/492accepted,rawpeak417,busmin11223mV. Whole-attempt408.619689eHz and
sigma313.661us include acceleration; controller average757half-us ticks at
stop corresponds approximately440eHz, not the whole-attempt mean.

Step4 guard2214us<2223; reference2211us,prior2313us,two-cyclemean2262us.
Prior same-sector interval814ticks versus657ticks. Original fault context
and finaloff verified by drv_cycle_fault; original full fixture verified
timeline/outputsoff before reporting failure. No electrical/tracking stop
preceded this CycleTiming refusal; that is not independent proof of lock.

captures/cycle450_603_hold90_30s.txt SHA
880BE2C4E6222BE06EEB60C3136573F5A78E543C0DE03D556FF27020105E1945.
No retry, flash, threshold or source change during this run. Current3A70,
allfinal gates/ENABLE/MOE/CCRs0,nFAULT1,Uartclosed. This demonstrates another
experimental speed-envelope refusal, NOT a450eHz hardware limitation.
Do not keep ratcheting the floor after each threshold-selected refusal;
the experiment must distinguish an operating-envelope policy from electrical
and loss-of-tracking protection. Recovery latency remains independently open.

## E602 8% and8.5% thirty-second holds complete

Unchanged3A70 cycle450/seed400 build, startup61,phase60,20kHz,trace0,
DMApeer/seedtiming; no dropout/reentry and no changed guard afterE601.
Both full fixture runs exit0, CRC/ADC/timeline/finaloff verified,Uartclosed.

| Duty | eHz | COM / accepted | Bus minimum mV | Raw peak | IRQ union % |
|---|---:|---|---:|---:|---:|
|8%|387.398925|69731 /69730|10913|401|52.375221|
|8.5%|409.811081|73765 /73764|10841|404|52.789694|

Operator explicitly confirmed PSU130mA DURING8.5% run. This is independent
average supply-current observation, not phase-current calibration or peak.
8% observed30000031us;8.5%30000033us. Whole-run cycle sigmas32.482/32.976us
include startup acceleration.8% last26cycles mean2574.615us/sigma18.130us,
tiny retained tail, not whole-run stationary statistic.8% phasehist149254
triggers validated; no aperture/sector bias claim. Minimum recorded8% cycle
2491us is below old2500us floor; no old failed run has been reclassified.

Captures cycle450_602_hold80_30s.txt SHA
602668AFE942DD0BFAE6A7DE4A61803AF6B92219E31A6BCD82578398CE9BDCE8;
cycle450_602_hold85_30s.txt SHA
440B7D609C8E4FD78E23E2809AE034D25C464EFC7ED6077AE9475B5B6A3A89F0.
E60175 baseline SHA AD3D95CAA8456C0A7D0206879F198D5AD9AF51485FB111540E96A5FE1CB30DC2.
Each invocation drv_driven_handoff.py --out captures/<capture> --ms30000
(arguments separated in shell) --drive-duty61 --bemf-duty80/85 --phase-shift60
--core-trace0 --pwm-roles --carrier-hz20000 --dma-peer --cycle450 --seed400
--seed-timing-reanchor. No repeated trials or recovery qualification inferred.

Next bounded9% hold with unchangedprofile, not a ceiling increase. Recovery
seed/arm latency remains separate unfinished work;30% is not a target to force.

## E601 explicit cycle450 experiment, candidate baseline passes

Evidence for bounded expansion: current7.5% recovery3/3 at359-361eHz;
E598/E6008% both reach approximately384-386eHz with electrical feedback,
accepted/COM progress and the same single-cycle refusal. Measured E598
COMP/COM maxima49/45us, commit22us,DMA20us,IRQunion52.39%; these are measured
brackets, not WCET or proof of operation at450eHz. AM32 same-hardware results
already disprove treating400eHz as a demonstrated hardware limit.

New opt-in bench-cycle450 inheritscycle400 and changes running single-cycle
floor2500->2223us (ceil1e6/450). This intentionally expands the experimental
speed envelope, not a semantics-preserving optimization. It does NOT average
away individual cycle failures. Seed400 qualification,32us remaining margin,
16us arm bound,238..1000us events,6000us slow cycle,current/bus/nFAULT,
feedback ages, finite original deadlines and stop paths unchanged.
No claim that recovery at450 is possible. First operating tests remain75/80,
not an automatic higher-duty sweep or permission to remove the new floor.

SHA3A701EEF4AFAAAD2654436B438649AE39E55CF592CCF3CA2E13C6A4532EE395A,
release opt-s/thinLTO/codegen1; emitted math audit saved, advisory not helper-
free certification. Frozen captures/reference/cycle450_601/ ELF and audits.
35 actual Rust guard tests PASS, exact2222/2223/2224/latch and other stops;
23 cycle hosttests +1seed profile PASS. Initial shell-quoted rustc harness
command failed before compilation; fail-fast script with --cycle450 fixed it.

Current2329 beforeflash guard/offPASS, candidate download/resetexit0, own
guard3/18,fullfive3200/CPU2,6,9,ADCphase3,IRQbudget5PASS. Artifacts
captures/cycle450_601_*; no motor before these disabled gates completed.
One startup61/BEMF75/10s baseline completed10000033us at360.624379eHz,
21637COM/21636accepted,raw316,bus10960,stack3448,finaloff/timelinePASS.
CLI initially failed POST-RUN because drv_sustained_report omitted2223 from
its explicit whitelist. Corrected exact profile whitelist plus seed validation
branch, replayed same capture through full fixture, and added actual-capture
regression (2testsPASS); no repeated motor run. Tests require10000ms duration.
--cycle450 required; conflicting running-profile flags rejected before UART.
The existing live_armed_baseline script does not yet support this profile.

Current candidate installed, final outputs0/UARTclosed. No recovery cohort
transferred from2329. Next one80/30s hold; retain any stop without retry.

## E600 exact historical reference also refuses at8%; current restored

Found original13B5 in Cargo cache by hashing201 candidate image/artifact paths:
target/thumbv6m-none-eabi/release/examples/shell_pwm-f26e3707d24d0484.
No rebuild. Preserved byte-identical images in captures/reference/cycle400_13b5/
shell-pwm.elf and current_2329/shell-pwm.elf. SHA256 respectively:
13B5F0447080264D57DB66C4D742F76FAD62880FBC1AD8921FE7B3C344DE206B and
23298EC18646ACFA1638186DF218249EA858128ABA516751D56804D8328F3C0C.

Current guard/off beforeflash PASS. Historical download/OpenOCDreset exit0;
own guard3/18, fullfive filter/atomic256/roles3200/CPU2,6,9/archive and
ADCphase3PASS. Native MCP IRQBUDGETCHECK5/0 with50us boundaryPASS, disabled;
UART closed before fixture. Captures reference600_beforeflash/guard,
reference600_{pulse,atomic,roles,cpu,archive},reference600_adcphase retain checks.

One reference attempt (no retry):
```
python scripts/drv_driven_handoff.py --out captures/reference600_hold80_30s.txt --ms 30000 --drive-duty 61 --bemf-duty 80 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle400 --seed-timing-reanchor
```

Exit1 CycleTiming12 at3136678us,7254COM/7253accepted,385.507308eHz,
rawpeak382,busmin11032mV. Whole-run sigma81.169us includes acceleration.
Fault step2 guard2493us<2500; reference2489.5us, prior2652.5us,
two-cyclemean2571us. Last cycle boundary1017ticks followed697ticks
(508.5us then348.5us) shows timing redistribution, not independent rotor motion.
CRC/chronology/timeline/finaloff independently verified by existing fault
decoder and fixture. Hash27526040B40C813F6DA42EC135C479C5008CDE4EEA8AEFCEFE25F0B478DB2453.

This falsifies the necessity of E580+ changes for the8% CycleTiming refusal.
It does NOT explain the earlier lower accepted speed, identify a physical
cause, or turn historicalE577 into a failed run. A single old-image failure
does not prove identical distributions or rule out code shared by both builds.
Next investigate the common acceptance-timing/envelope relationship, not
bisect the later recovery optimizations as the required cause or blame hardware.

Restored frozen2329; download/reset both exit0. Fresh
reference600_restored2329.txt guard3faults/18refusals/outputsoffPASS,Uartclosed.
No firmware source, thresholds, PSU setting or reference-core changes.

## E599 cycle-policy review and historical operating-point comparison

Read actual RunGuard::accepted/poll and the existing host-only Window policy.
The running floor bounds every same-sector accepted cycle; independent event
order/238..1000us, slow-cycle6000us, electrical and watchdog checks coexist.
Two-cycle averaging is NOT equivalent protection: E551-553 already retained
acceleration-delay and alternating-cycle counterexamples. Do not deploy that
old candidate merely because E598's two-cycle mean passes.

Added E598 regression to test_drv_cycle_window_study.py: exact2489.5/2543.25us
reference values, original failure retained, no whole-run/physical-lock/safe-to-
deploy claim. All5 tests pass. A separate initial one-line comparison command
had a Python syntax error; corrected command exit0 supplies values below.

Historical E57713B5 and current E5982329 both record startup61/BEMF80,
20kHz, equal CCR roles, no per-COM UG, cycle2500/event238 guard. No recorded
configuration discrepancy found; metadata is not independent CCR measurement.
Each retained final32-event tail supplies26 overlapping same-sector cycles:

| Capture | Tail cycle mean us | Tail cycle sigma us | Whole-run IRQ union % |
|---|---:|---:|---:|
| E577 pass |2672.115385|19.531684|53.262411|
| E598 refusal |2584.423077|18.839756|52.389816|

These tails are tiny, exclude the refused event, and compare different builds
at different elapsed times (30s versus1.51s). They show different accepted
cycle rates, NOT a controlled speed regression or attribution to code/supply.
The failing tail is not broadly noisier and measured IRQ union is not higher;
neither statistic proves absence of a rare scheduling event or CPU slack.
The refused reference cycle is94.923us shorter than current preceding tail
mean. That is evidence to preserve, not authority to widen the speed floor.

No hardware commands, firmware edits, flash or guard changes.2329 last-off
evidence remains E598 (not a fresh read). Next isolate why the same requested
duty reaches a different accepted-cycle rate, using a hash-verified historical
image if available; do not reconstruct an allegedly identical reference from
current dirty source or resume identical8% attempts until one passes.

## E598 current-build 8% sustained attempt refused by cycle guard

One unchanged2329 attempt requested60s steady hold, startup6.1% then8% BEMF,
20kHz, cycle400/seed400; no injected dropout. Command:

```
python scripts/drv_driven_handoff.py --out captures/envelope_598_hold80_60s.txt --ms 60000 --drive-duty 61 --bemf-duty 80 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --cycle400 --seed400 --seed-timing-reanchor
```

Terminal exit1: CycleTiming12 at1511794us,3481COM/3480accepted,
mean383.901212eHz,rawpeak346,busmin11032mV. Whole-run cycle sigma114.878us
includes acceleration; do not call it stationary jitter. No current-in-amps claim.

Fault step4 guard2493us violates2500us floor. Reference counter cycle2489.5us,
previous2597us,two-cyclemean2543.25us; guard/reference difference3.5us.
Controller average860half-us ticks, filter12, running1/polling0 at snapshot.
Same-sector final interval changes947->817half-us ticks (-65us); retained
data does not identify cause or independently prove rotor overspeed/loss of lock.
Changing only the guard timestamp would not cure the reference-side refusal.

`python scripts/drv_cycle_fault.py captures/envelope_598_hold80_60s.txt`
revalidated CRC/chronology/output-off and fault context. Original fixture also
verified timeline/finaloff; all gates/ENABLE/MOE/CCRs0,nFAULT1,UARTclosed.
Capture SHA256 AD003743EF8A44FA861A48DC803CD7522EB706D78E9066D0BD850F19964582D8.
No retry, firmware/flash/guard change. Current8% sustained is not qualified;
historical13B5/E577 pass remains separate. Next investigate accepted-cycle
envelope versus timing residual, not an automatic floor increase or identical retry.

## E597 7.5% recovery cohort: 3/3, no exclusions

Two predeclared additional30s attempts on unchanged2329, startup6.1%/BEMF7.5%,
20kHz, seed400/running400 and alloriginalguards. Both pass, making3/3 withE596.
Each includes startup, injected tracking loss, one fresh measured reentry and
completion within the originaldeadline; CRC/timeline/finaloff verified/UARTclosed.

| Run | eHz | COM/accepted | Cycle sigma us | Bus minimum mV | Raw peak | Fresh arm margin us / arm cost us |
|---|---:|---|---:|---:|---:|---|
| E596 |359.550504|60387/60386|23.484152|10925|344|37 / 7|
| E597-02 |360.650678|60571/60571|23.363992|10160|376|35.5 / 6|
| E597-03 |360.710614|60581/60580|23.180475|10913|325|37.5 / 7|

Allfreshedgeages80us. Run2 lowerbus reading retained; above8400mVguard,
not independently calibrated power-path/current evidence. No samples/runs
excluded and no electrical threshold adjustment. Startup raw508/447 retained
separately for the two newattempts. Raw current does not establish PSUamps.

Capture recoverymap_597_recovery75_30s_02.txt SHA256
BEFBABB26067CD6A5955A588494AA601E8C45A94116764BFE6FF628673784E03.
Capture recoverymap_597_recovery75_30s_03.txt SHA256
82292D0C8E91B4B7BCC65A56253668B80F9A51233A739012AA9E957B8AA5C4EA.
No firmware/flash this entry. Stop identical75repeats.8%recovery remains
unqualified; this modest cohort is not the fullgoal or a statistical guarantee.

## E596 usable recovery range: 7.5% passes once

Rebuilt campaign excluding retired bench-final-edge-prepare, retaining live
control, seed400/running400 and all existing guards. Currentimage
23298ec18646acfa1638186df218249ea858128aba516751d56804d8328f3c0c.
release-s/thinLTO/advisorymathauditPASS, preflashoff, download/OpenOCDreset,
ownguard3/18/fullfive3200CPU2/6/9/ADCphase3/IRQbudget5 allpass.

One fixed75/30s recovery attempt passes359.550504eHz,60387COM/60386accepted,
cycle sigma23.484152us, recovered27.991637s. Raw344,bus10925mV,IRQ51.696831%.
Initialstartup raw553 retained. Freshseed937,age160ticks=80us,remaining74ticks
=37us,arm7us. Acquisition12intervals,gap166ticks,originaldeadline retained.
CRC/timeline/finaloff verified, UARTclosed. Not calibrated-current evidence.
Capture recoverymap_596_recovery75_30s.txt SHA256
B903C25FF1171897D07443A4149896F65CD206D10F864C6A6EB66CA3FDED7C10.

This is one successful midpoint in the7%pass/8%arm-refusal interval, not a
repeatability cohort and not proof at8%. Next predeclared two additional
tenths-duty75 (7.5%) recovery attempts, retaining every failure. No firmware
change or automatic threshold increase between those attempts. Actual2329off.

## E595 live preparation used, but no latency gain; retire this variant

CurrentE6EC fullfive3200 preflights pass (CPU2/6/9us), ADCphase3 andIRQbudget5
pass; captures/finalprep_595_* retains results. One70/30s recovery with explicit
--final-edge-prepare completes335.300589eHz,56312COM/56311accepted,
sigma23.832569us,raw315,bus10925mV,stack2664. Initialstartup raw543 retained.
FINALPREPused1, twelveinterval recoveryseed1002, maxsamplinggap174ticks=87us
below100us limit. Originaldeadline/CRC/finaloff verified, UARTclosed.

SEEDLAT88/104/118/148/152, actualedgeage162ticks=81us, remaining89ticks,
arm6us. Compared withE58880us, no measured gain; guardbracket again15us.
The moved cold stores plus new marker checks do not improve total handoff.
No8% retry or repeat cohort: retire cold-state-only feature from future envelope
campaign configuration, preserve source/test evidence. The idea of broader
preparation is not proved impossible, but this specific intervention did not
deliver it. Do not call it a successful speed optimization because used1.

Capture finalprep_595_recovery70_30s.txt SHA256
F5DF4E96AA2AD7F3A9FD49F38EB80548418CA5FBFEA23315E0D97CA3F5E54AC6.
ActualE6EC remains installed/off; baseline has NOT been reflashed. Future
baseline rebuild must exclude bench-final-edge-prepare and receive its own
preflights. No rollback or source deletion performed in this entry.

## E594 disabled lifecycle passes; campaign installed

InstalledF6BE diagnostic after preflashoff verification and successfuldownload/
OpenOCDreset. Native serial finalprepcheck31/31PASS withENABLElow; allgates,
MOE,CCRs,ENABLE0/nFAULT1 afterward. Transcript captures/finalprep_594_check.txt.
This tests injectedmetadata lifecycle, not successful awake preparation or
every possible hardware-owner refusal. Disassembly cold_coast_state08007b2c
shows only RAM literals0x2000xxxx plus stack, no calls/peripheralwrites.
No complete before/after peripheral snapshot claim is made.

Rebuilt livecontrol/final-edge-prepare campaign, excluding final-edge-check.
Release-s/thinLTO/advisorymathauditPASS, hash
e6ec96d42123abe5d5ca4f0af8a9240414a824d38dff838e378c11a0fa4340c6.
Installed withsuccessfuldownload/OpenOCDreset; own guard3/18 andfinaloffPASS
in captures/finalprep_594_campaign_guard.txt. UARTclosed. No motor thisentry.
Next candidatefullfive3200/ADCphase/IRQbudget, thenone70/30s recovery with
--final-edge-prepare plus existingflags. No diagnostic timing transferred.

## E593 disabled adapter harness built, not run

Extracted take_final_step into the actual live-consumption helper: atomically
clears marker, admits only matching1..6sector and nonpolling mode. No seed or
timer authority is returned. Added bench-final-edge-check/finalprepcheck.
It requires idle owners/bridge disabled, checks real prepare refusal with
ENABLElow, then injects metadata for each sector and tests actual cold reset,
single consumption, wrong-sector refusal, polling refusal and live_stop
cancellation (31expected checks total). Restores saved software cold state.
Metadata injection is explicit in output; no driver-readiness bypass exists.

Limitations: not yet executed, not a successful awake preparation test, not
proof of every live owner-refusal branch, no complete timer/mux snapshot test
yet. Those claims may not be inferred from the31counter. Inspection and
hardware checks remain before any powered use.

Diagnostic buildf6be73bb6494912967700d6bc6fc0de289829e81f3ccfb34c68ddd6e19c3e3b7
release-s/thinLTO/advisorymathauditPASS. Uses priorfeaturelist with
bench-final-edge-check, but omitsbench-live-control tofit the idle harness.
This is a diagnostic image, not the live-control campaign image. Host3marker
testsPASS. NoUART/flash/motor. Actual0835lastoffE588; root nowdiagnostic.
Next run disabled harness and inspect peripheral effects; then rebuild the
actual campaign image and qualify it separately, not transferdiagnostic timing.

## E592 strict capture provenance for final-edge preparation

Added drv_final_preparation.py and --final-edge-prepare in driven-handoff and
live exploration fixtures. Marker must be unique and exactly used0/1,
interval11,output_authority0,final_edge_checks1. A marker without explicit
selection, missing selected marker, duplicate, malformed or altered fields
fail verification. After ordinary successful campaign validation, recovery
requires used1; nonrecovery requires used0. Parsing used1 alone never certifies
fresh arming or powered success, so existing refusal diagnosis still applies.

Three protocoltest methods pass including malformed/duplicate/selection and
consumption mismatch cases; both existing retained-refusal tests pass too.
These are host protocol checks, not disabled adapter lifecycle qualification.
No firmwarebuild/flash/UART/motor. B78B remains staged; actual0835lastoffE588.
Next disabled adapter duplicate/mismatch/cancel/ownership and no timer/mux
effects tests, then candidatepreflights. No old capture reclassified as newbuild.

## E591 flash fit restored without changing live guards

Marked cold_coast_state noinline to share the reset between callers: overflow
480->448bytes, still failed. Separated the existing idle-only seedmaskcheck
harness into explicit bench-seedmask-check depending on bench-masked-seed-arm.
The live masked-arm feature, Comp masking/clear sequence and safety code remain
unchanged. Harness source remains available for diagnostic builds; seedmaskcheck
is not a supported command in the ordinary campaign build unless opted in.
No default firmware preflight should assume that harness is present now.

Added poststop FINALPREP used=0/1 interval=11 output_authority=0 final_edge_checks=1.
Usage resets at observation start/preparation and records consumption separately
from the revocable marker. Successful consumption is NOT proof of motor lock.
Host selection/strict decoding is still required before powered use.

Candidateb78b7adee1461053e1d4988c6260e59351c2a62c7acadee11ebdf6626f414df9
builds release-s/thinLTO, emitted advisorymathauditPASS. text129784,data1120,
bss29348; cold_coast_state08007c38,size0x50, seedmaskcheck absent in symbols.
No flash/UART/motor. Actual0835lastoffE588 unchanged. Remaining required work:
disabled lifecycle tests for duplicate/mismatch/cancel/ownership and no timer/mux
effects; strict host marker validation; candidate preflights before7% recovery.
Fit is not qualification and no latency claim is made from this build.

## E590 opt-in cold-state adapter staged; flash overflow

bench-final-edge-prepare depends on reentry-clear-once/reentry-staging.
In awake recovery only, once acquisition reports11interval preparation hint,
core checks inactive owners, ready driver, outputs disabled and no prior marker,
then initializes sector/rising, physical observation sector, coast status,
poll state/count and mode counters. No observation clock reset, COMP mux write,
guard start, timer start, interval mean or output authority moves before edge12.
Ordinary final acquisition and sampling-gap checks remain unchanged.

At final resume, a one-shot marker must match the measured final sector and
nonpolling mode before skipping those cold stores. Missing/mismatched marker
fails closed through observe_end/gates_off. live_stop clears the marker; common
gates_off invokes live_stop. Successful acquisition physical bridge_clear does
not revoke this preparation, but does still clear all physical drive authority.
Initial driven handoff remains on the ordinary cold-reset path.

31actual acquisition tests still pass, but they do NOT test this new adapter's
hardware lifecycle. Release-s/thinLTO link failed: data overflow472bytes,
total480bytes. No audit/flash of stale root ELF; actual0835lastoffE588 unchanged.
No UART/motor commands this entry. This is staged source, not usable firmware.

Remaining before any power: fit without removing safety or switching to an
unqualified optimization profile; poststop feature/consumption provenance;
disabled tests for duplicate/missing/mismatched preparation, stop revocation,
owner/output refusal and absence of timer/mux changes; emitted-code review.
Only then measure actual acquisition gaps and post-edge age in7% recovery.
If marker/check overhead cancels the saved stores, retain that result; do not
claim this limited cold-state move solves all setup or improves the envelope.

## E589 pre-final-edge scheduling contract (offline only)

Refined the architecture to prepare after11intervals, not refresh a completed
12interval seed. Acquisition::preparation_step() reports the expected final
sector only at11 with no fault/ready seed. It is a scheduling hint, never a
Seed, and leaves edge/poll/filters unchanged. One-shot completed seeds cannot
be made fresh by later calls. This method currently has NO live caller.

scripts/test_final_edge_preparation.py runs31tests against actual acquisition
policy/minzcore: all6sectors, wrappingclock, no seed before measured12th edge,
wrongorder/short/longinterval/cycle/deadline refusal, and samplinggap/dwell.
The new final-edge test explicitly preserves original edge timestamp. The
existing EdgeFilter still refuses >200tick sampling blackout; preparation
cannot silently pause sensing. Nonfatal existing minz incrementalAccessDenied
note remains; cargo/rustc/test all exit0. No embeddedbuild/flash/UART/motor.

Required live integration split:

- Before final edge: once-only cold reference/history preparation and statistics
  work with all drive authority disabled. Retain and continue all three filters
  and acquisition object; preparation must fit their100us sampling-gap bound.
- Do not call current coast_run_inner wholesale from the hint: it changes COMP
  mux, starts guard/timebase and later blocks in a powered loop. Those actions
  would violate acquisition ownership and are not licensed by the hint.
- After final edge: revalidate actual seed, current output/owner/nFAULT state,
  original remaining campaign budget and timestamped feedback. Install current
  guard timebase/first sector, seed-dependent reference intervals and actual
  sensing route, then retain32us fresh-arm floor/16us armcost.
- Any preparation failure or intervening rejected edge cancels the candidate
  and retains ordinary off/diagnostic paths. No prepared flag may survive stop.
- Keep cold setup separate from admission: starting the guard early would age
  its first-event watchdog and change deadline semantics, so is not this design.

This step supplies a tested acquisition scheduling seam, not a measured latency
improvement. Actual0835lastoffE588 remains installed. Next implement the
revocable cold-state preparation adapter before any powered experiment.

## E588 exact current predicate, measured modest gain

Before altering guard lifecycle, inspected emitted installation: its history
clear is24bytes, not a large newly discovered reset. Current validation still
looped over three samples with rail checks and signed absolute arithmetic.
Replaced it with three explicit const predicates v.wrapping_sub(848)<=2400.
This is exactly the old848..3248 inclusive range, including rejection of all
out-of-ADC-range u16 values.35guard/admission tests pass, including exhaustive
65536values at each of3phase positions. Current-before-bus precedence unchanged.
Shared helper affects admission, foreground feedback and stream current guard.

0835cea626bcdca3f6e65b505f4c09b0cb8b8bdaf83ae6161f7011fa859ce0f0
release-s/thinLTO/audit passes. validate_feedback0800a4d0,size0x60 versus old
0x7c; emitted code has no phase loop/abs/sample stack array or mathhelper.
Installed after off/guard verification and successful download/OpenOCD reset.
phasebounds_588_* captures retain disabledguard3/18,fullfive3200CPU2/6/9,
ADCphase3 andIRQbudget5 passes.

One7%30s recovery passes335.427269eHz,56334COM/56334accepted,
sigma23.389022us,raw321,bus10901mV; originaldeadline/CRC/finaloff verified.
Seed997,age160ticks=80us,remaining89ticks,arm6us. SEEDLAT88/106/118/144/148:
guardbracket13us vs E58715us. IRQunion51.587801% vs52.114634% at335.566eHz.
These are individual observed runs, not controlled paired WCET attribution.
Initialstartup rawpeak918 retained, not conflated with recovered321.
Capture phasebounds_588_recovery70_30s.txt SHA256
E524264491AD58CE4D319D38A15691B36CB774A8A3F03EBBF330374654E3459C.
No8% retry; total edge age still80us. Actual0835off/UARTclosed.

Next architecture candidate: qualify continuous rotation, prepare the handoff,
then consume a genuinely measured final fresh edge rather than spend setup
against the last already-qualified edge. Must preserve12interval evidence,
ordering/interval/cycle checks through the final edge, correct guard firstsector,
fresh feedback, originalbudget,32usfloor and16uscost. No timestamp refresh or
blind predicted edge. This candidate is not yet implemented or authorized to
skip any checks; its point is scheduling existing work before the critical edge.

## E587 const-prepared hardware result: no observed latency gain

InstalledCDE8F03D99250A89ED42D28463D67C98B69ED15965BB67C58D5D73EA39B65DB5
after preflash off/guard validation; successful download and OpenOCD reset.
captures/constprepared_587_* records disabledguard3/18, filter/atomic/roles/
CPU/archive preflights (3200ticks,CPU2/6/9us), ADCphase3 and IRQbudget5 pass.

One7%30s recovery passes335.565608eHz,56358COM/56358accepted,
sigma23.713603us,raw335,busmin10901mV. Freshseed1008,age162ticks=81us,
remaining90ticks,arm7us; originaldeadline/CRC/finaloff verified,UARTclosed.
SEEDLAT90/106/118/148/150: guard-start bracket still15us, total age still81us.
No measured handoff gain over E585. Keep this as code-size/stack reduction,
not a recovery-envelope win. No8% repetition without evidence of more margin.
Capture constprepared_587_recovery70_30s.txt SHA256
2CD4D9A96874C94BDFA9E25CAE213F9588D5595A42E42838A28A7C80EBBB66C3.

Host-only verifier now diagnoses retained armed0 measured-seed refusals after
checking CRC, initialfault8 archive, seedidentity and finaloff. E583/E585
report exact51/58ticks below64 instead of generic missing-arm telemetry.
Both remain failures; missing-finaloff negative tests pass (2test methods).
No live firmware diagnostic added; running fixture had loaded before host edit.
ActualCDE8 stays installed/off. Next substantive post-edge-path work; no
threshold change or repeated const-prepared tuning based on a smaller symbol.

## E586 const-prepared guard-start candidate

Previous turn progressed with measured common-clock improvement and retained
8% refusal. Examined installed3013's start_inner (0800a6c8,0x248bytes):
prepared remained a runtime argument despite all3callers supplying literals.
Changed only this boolean to const PREPARED, caller values false/true/true.
Live AWAKE/ENABLE/nFAULT/owner/output checks, one-shot adoption, current/bus/
feedback validation, deadlines and guard installation all remain. No check
was moved to a stale pre-acquisition observation.

Candidatecde8f03d99250a89ed42d28463d67c98b69ed15965bb67c58d5d73ea39b65db5
builds release-s/thinLTO, advisorymathauditPASS. Prepared start_inner now
0800a65c,size0x22c; stackallocation44bytes versus52. Emitted entry directly
checks AWAKE without the old prepared-argument branch. This establishes
specialization, NOT latency gain or an8% recovery pass.

Added scripts/test_guard_install_policy.py to build with bench-guard-install
and execute only after successful compilation:35currenttestsPASS, including
admission refusal precedence and installed-history replacement. Initial direct
PowerShell rustc invocation lost cfg quotes and failed; a subsequent old33test
binary ran inadvertently. That result is excluded; the fail-fast runner then
compiled successfully and produced the35test result above. Policytests do not
execute the changed PAC adapter; hardware preflights remain required.

No flash/UART/motor this entry. Actual3013lastoffE585. Next disabledpreflights
and7% recovery with the candidate, then decide from measured seed age; retain
32us armfloor/16uscost and originaldeadline. Do not count smaller code as speed.

## E585 common-clock candidate installed and measured

Installed3013338ca36c6e9ea50a1df7246582940755eab779d8bab63eb3e0513e0cec6f
after preflash off/guard checks. Download and OpenOCD reset both exit0.
captures/commonage_585_* retains disabled guard3/18, filter,atomic256,
roles6/3200ticks,CPU2/6/9us,archive3x3,ADCphase3 and IRQbudget5 passes.

One7%30s recovery passes:334.671565eHz,56207COM/56206accepted,
cycle sigma23.819311us,raw329,busmin10698mV. Original deadline and finaloff
verified; UART closed. Seed1019, actualage162ticks=81us, remaining93ticks,
arm7us. SEEDLAT88/106/118/148/152. PreviousE582age168ticks=84us;
small observed improvement, not paired WCET or repeatability proof.
Capture commonage_585_recovery70_30s.txt SHA256
B7E9380A9F65408ECE9A955E4412D5CF42FC785BC09D90039EAA72F63E01F897.

One8%30s recovery attempt then fails fresharm (not acquisition):12interval
seed886ticks,7cycles min5226ticks. Actualage164ticks=82us;
wait443-221=222ticks, remaining58ticks=29us below unchanged32usfloor.
CORESEEDarmed0/arm0, no second powered segment. SEEDLAT90/106/118/148/152.
First segment4563COM/4562events,raw397,bus10972. Independent CRC/firstarchive/
finaloff verified after CLI generic fresh-arm failure; UART closed.
Capture commonage_585_recovery80_30s.txt SHA256
7068E859635A36372B6A6075A7E8BA59F6C3A6D3897672196C4BE3340A0BF57F.

No repeat-until-pass, no further threshold change. Actual3013 remains installed
and off. The optimization helps slightly but does not establish8% recovery.
Remaining guard-start bracket is(148-118)/2=15us; next inspect its concrete
work rather than another timestamp microvariant or weaker fresh-arm floor.

## E584 staged common-clock feedback age

The previous turn was progress: E583 distinguished successful acquisition from
fresh-arm refusal. Current source confirms baseline bus conversion is already
cached; do not claim a new division removal there. Baseline's age calculation
did perform five volatile TIM17 reads. Candidate takes one common clock reading
after copying original samples/timestamps, then finds the maximum wrapping age.
It retains200us setup reserve and1000us stale limit. This is age at one instant,
not a claim that its clock reading is later than every read in the old program.
Neither helper nor caller refreshes acquisition timestamps. The existing short
acquisition window remains necessary for interpreting the16-bit clock.

Pure feedback_age.rs tests pass: all65536 clock values at ages0/1/799/800/801/
1000/65535 and each channel as oldest. Exact800us+200us allowed;801 refused.
Build3013338ca36c6e9ea50a1df7246582940755eab779d8bab63eb3e0513e0cec6f
release-s/thinLTO and advisory emitted math audit pass. coast_run_inner6342
contains one clock load before maxloop6358..636a; constants800 and200 resolve;
no divide/wide helper in this bracket. Array stack stores remain, so code
inspection alone does not establish a gain. NOTflashed; actual937F/offE583.
Next disabled preflights and preceding7% recovery timing on candidate; keep
original32us fresh-arm/16us armcost bounds and retain failure if no gain.

## E583 8% recovery: valid seed, insufficient fresh arm margin

Same937F image, no rebuild or guard changes. One attempt retained in
captures/seed400_583_recovery80_30s.txt, SHA256
AE6C70A21692B9128F2A2FF5E049B32C4FD3D1DC2517A8D5A0978916BAEC9747.
Recovery acquisition succeeds:12intervals, mean874ticks,7cycles,
cyclemin5222ticks, elapsed5586us. Fresh core check sees age168ticks (84us).
Reference wait=(874>>1)-((874*16)>>6)=219ticks; remaining51ticks=25.5us
is below retained64tick/32us floor. CORESEED armed0, arm_us0; remaining_arr
4294967295 is refusal sentinel, not measured timer slack. No second powered
segment. CLI fails with generic missing-fresh-arm error; independent CRC,
first-segment archive and finaloff validation pass. REENTRY result7 alone
does not prove successful recovery. No retries or threshold relaxation.
This isolates the next optimization to post-edge arm latency; it does not
invalidate previous8% sustained holds or establish a motor speed ceiling.

## E582 seed400 installed; preceding-point recovery passes

Installed937fc1df45b196a91cec8622ae2a891752a81602867b18039ca6e23ae23a7eea.
Disabled preflights retained in captures/seed400_582_preflight.txt: guard3/18,
IRQbudget5, filter, atomic256, roles6, CPU2/6/9us, archive3, ADCphase32 pass.
One7%30s recovery: accepted330.080eHz,55437COM/55436accepted, sigma24.337us,
rawpeak336,busmin10877mV. Fresh recovery seed1031ticks, edgeage168ticks,
remaining90ticks (45us), arm6us. Original deadline retained; outputs off verified.
Strict fixture replay with --cycle400 --seed400 --seed-timing-reanchor
--dma-peer --carrier-hz20000 --pwm-roles --dropout --reentry passes.
Capture captures/seed400_582_recovery70_30s.txt SHA256
7A990E984AFDD456C68D55AE037651D89407205881C6430155BC7343AEC00863.
One pass, not a repeatability cohort or calibrated-current evidence.

## E581 seed400 built, explicit provenance

Root937fc1df45b196a91cec8622ae2a891752a81602867b18039ca6e23ae23a7eea,
release-s/thinLTO build and emittedmathauditPASS. Notflashed: actual13B5lastoffE577.
Poststop SEEDPROFILE marker asserts compiled834/5000/476 constants, unchanged
64tickremaining/16usarm bounds and sharedacquisition scope. Strict host
--seed400 separatefrom --cycle400; malformed/duplicate/missing/implicitprofile
testsPASS. Recoverydecoder and drivenrestart policy consume explicit marker;
oldcycle400/no marker retains350seedlimits. No motor/UART/guardwaiver.
Next disabledcandidatepreflights, then precedingpoint recovery, before8%test.

## E580 staged seed400 policy, not installed

Separate bench-seed400 depends on runningcycle400, but is not inherited by it.
SEED_MIN834 (conservative ceil2000000/(6*400)), repeatedcyclefloor5000ticks;
individualfloor476ticks unchanged. Twelveinterval qualification/order/dwell/
deadline/oldseedage rules retained. Actual32us remaining and16us arm ceiling
remain independent runtime refusals. Scope includes both driven and passive
measured seed consumers; no startupwaveform/ramp/duty change.
python scripts/test_seed400_policy.py:30hosttestsPASS against actual minzcore
arithmetic, including exact64/63tick age boundary and wrapping timestamps,
all12interval qualification and shortcycle/edge refusals. Old350 test now uses
explicit Acquisition5716/476 instead of changed RuntimeAcquire alias; its
expectedrefusal retained. No embeddedbuild/flash/motor. Installed13B5 unchanged.
Next explicit profiletelemetry/host checks and release/disabled timing gates;
then precedingpoint recovery before an8% attempt. Newpolicyalone is neither
evidence of freshseedtiming nor qualification at400eHz.

## E579 current13B5 re-audit (supersedes no current-code assumption)

Read the complete E423-425 experiment before proposing another carrier move.
E425 already found no measured gain; do not repeat it as a newly discovered fix.
Current ELF13b5f0447080264d57db66c4d742f76fad62880fbc1ad8921fe7b3c344de206b
has acquire_inner at0800951c,size0xa98. Its three direct __aeabi_uidiv calls:
08009b3c and08009b4e are baseline ADC conversion in the ongoing scan;
08009e68 computes frequency for optional track/coast_flying_run. Awake recovery
clears TRACK before acquisition, so that optional branch is not its final-edge
handoff path. Do not claim removing these three saves three divisions from the
qualified-edge-to-arm bracket. The scan conversions can affect sampling timing,
which is a different question. The bounded twelve-interval mean remains exact.
coast_run_inner helper calls0800686e/08006890 feed feedback_inner in its running
foreground sampling path; symbol-level attribution alone does not put them in
pre-arm setup. No claim all transitive/indirect helpers are absent from setup.

Current postqualification source still aggregates diagnostics, restores mux,
clears bridge and publishes report, then restores selection flags and validates
the original session budget. Removing any of that needs a specific preservation
contract, not a blanket "move diagnostics" change. Historic E564 acceptedseed999
had edge_age166ticks=83us, remaining84ticks=42us, arm7us. This does not prove
current13B5 cost; it warns that the80us agebudget near372eHz is already tight.
E577 refused after5intervals, so has no current final-seed age measurement.

No firmware/flash/motor change E579. Next explicit acquisition-profile scope
and final-seed timing experiment must retain fresh32us/16us admission; no
repetition of retired carrier experiment or unsupported soft-divide savings.

Source and retained-capture audit, 2026-09-14. No firmware or guard change.
Installed 137F remains the last verified image; no new motor attempt.

## Measured brackets, not a WCET claim

`SEEDLAT` uses elapsed half-microsecond ticks from the original seed edge.
Subtract adjacent fields within this record; do not mix its origin with the
guard clock or accepted-event observation clock.

| Capture | Edge to entry | Entry to reset | Reset to feedback | Feedback to guard | Guard to reference |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline_start61_reentry68_30s_01 | 44 us | 9 us | 7 us | 15 us | 2 us |
| quietstamp_start61_reentry68_30s_01 | 44 us | 9 us | 7 us | 16 us | 2 us |
| quietstamp_start61_reentry69_30s_01 | 44 us | 9 us | 7 us | 15 us | 2 us |

These are grouped wall-time brackets, not costs attributed to one function.
In particular, edge-to-entry includes the mandatory 20 us confirming dwell,
acquisition validation, cleanup, publication, caller restoration and budget
admission. The 6.9% capture is a failed sustained run, not qualification.

## What remains on the fresh-edge path

- `flying_bench::acquire_inner`: final edge validates order, individual interval,
  repeated-sector cycle and twelve-interval mean. Then aggregates diagnostics,
  stops comparator delivery, restores CSR, physically clears the bridge and
  publishes the recovery report. The bounded mean is already division-free.
- `reacquire`: restores selection/flags, not the successful awake baseline.
- `resume_once`: samples the original campaign clock and obtains remaining
  budget. This must remain live; it is not a reusable pre-edge authorization.
- `coast_run_inner`: live owner/output checks, carrier preparation, observation
  reset, feedback age/validity, guard admission/start, seed-dependent reference
  state, comparator mux/priority configuration, then actual age-based arming.
- `start_inner`: TIM6 is started only after guard admission. The timer is reset
  to zero with a 100 us period. A full arbitrary steady-state interrupt stack
  must not simply be added to every earlier acquisition bracket. Conversely,
  the observed short tail is not proof that this deadline can never be crossed.

The current observation reset is small: clock initialization and optional
diagnostic counters. Bulk statistics were already staged before acquisition.
There is no newly discovered large reset or report memcpy to remove.

## Specific preparation candidate and its necessary safety contract

Carrier preparation currently falls in the 9 us entry-to-reset group, along
with other work. Its individual cost has NOT been measured. Merely moving
`prepare_carrier` before sensing does not work: successful acquisition calls
`bridge_clear`, whose `revoked_restore_af` restores ARR to startup PWM_ARR.

A meaningful next experiment is a recovery-only prepared-carrier path:

1. Prepare ARR while all bridge outputs and scheduler owners are disabled,
   before the final edge can arrive.
2. Preserve that period ONLY on successful awake recovery cleanup, while still
   clearing MOE, all three CCRs, GPIO gate latches, prepared-role authority and
   restoring AF modes. No outputs or commutation authority may survive cleanup.
3. At handoff, verify the expected period and existing live owner/output checks;
   do not silently accept a stale preparation flag. Every failure and generic
   shutdown keeps the current full reset, including startup-period restoration.
4. Require disabled cleanup/revocation tests, release disassembly/math audit,
   and existing-envelope recovery qualification before discussing expansion.

This is a candidate, not implemented authority or an estimated speed gain.
Keep the original seed timestamp, feedback-age allowance, campaign deadline,
32 us remaining-arm floor and 16 us arm-cost limit. Do not expand the profile
on the basis of these three observed traces alone.

## E424 — staged implementation, not flashed

Opt-in `bench-reentry-carrier` prepares the carrier after acquisition has
stopped owners and staged statistics, before its scan clock starts. Successful
awake recovery alone uses `bridge_clear_inner::<true>`; it still clears MOE,
all CCRs, GPIO latches and prepared-role authority and restores AF. Generic
`bridge_clear` always uses the false specialization, retaining period reset.
Initial driven handoff and every failed acquisition retain their prior paths.

Before starting the new guard, the recovery path checks live output/owner
state, zero role latch, carrier ARR, all three zero CCRs and inactive ADC DMA.
Failure performs ordinary full shutdown. Seed timestamps, budget admission,
feedback age and all arm/operating guards are unchanged.

Added an ENABLE-low `rolecheck` sub-check using actual register operations:
default reset rejects readiness; preparation admits it; applying a role revokes
readiness; successful preserving clear restores readiness with AF/latches
verified; generic clear restores startup ARR. It must report
`REENTRYCARRIERCHECK passed=5 total=5 enable=0`. This check is implemented but
NOT yet executed on hardware. A failed sub-check stops rolecheck before its
ordinary waveform checks. It never enables the driver.

Post-run `REENTRYCARRIER` records explicit provenance; the driven fixture has
`--reentry-carrier` to require an exact unique marker. Two host tests reject
missing, duplicated, altered and old-build claims. This marker is provenance,
not evidence that recovery succeeded; ordinary campaign verification remains.

Candidate SHA256:
`1717BA38D78466DF61A56D99F92A5428BA81239275A681EF5335A91F50751865`.
Release opt-s/thin-LTO: text124808, data1104, bss29336. Automatic hashed linker
audit matches ELF; root named audit also refreshed. The audit is advisory,
with100 unreviewed helper calls, NOT a declaration of division-free firmware.
353 Python tests pass. Rust replay passes189 library tests plus34 other tests;
existing incremental AccessDenied notes remain nonfatal. These host tests do
not execute PAC writes or establish cleanup correctness on hardware.

Next: disabled hardware cleanup/guard/filter preflights on this candidate,
then matched 6.8% recovery only if those pass. Compare SEEDLAT and arm margin;
no latency gain is claimed yet. Actual installed137F/offE421 is unchanged.

## E425 — hardware qualification passes, no observed latency gain

Installed1717 after preflash UART off readback. Initial guard01 failed on UART
silence. `earlycarrier_boot_snapshot01.jsonl` retained BEFORE repair: APBENR1
08000000, USART CR1=0d/BRR22b, ISR00600010; ENABLE ODR=0, BDTR0c1a, all CCRs0.
The documented APBENR1=08040000 repair restored UART. guard02 passed3 faults/
18 post-stop refusals. No motor command was issued before qualification.

`earlycarrier_preflight01` passed all five checks; roles capture contains the
exact `REENTRYCARRIERCHECK passed=5 total=5 enable=0` marker, checked explicitly.
CPU maxima2/7/10us unchanged. New physical cleanup/revocation test passed.

`earlycarrier_start61_reentry68_30s_01.txt` passed startup61/BEMF68, one injected
tracking-loss recovery, original30s deadline and finaloff. Recovered27.991123s
at315.703937eHz;53022COM/53021accepted, cycle sigma23.513089us,
IRQunion52.425834%, rawpeak274, busmin10889mV, stackuntouched2676.
DMAmax22us/queue2. Seed1059, acquisition6647us, armspare14.5us.
SHA256 `4355C3C86FD9B265631A2F87E7F9443673B650FB2E634B6A3AF35DBDE28EE7F5`.
PSU limit unchanged by software; raw current remains uncalibrated counts.

SEEDLAT88/106/120/152/156 is EXACTLY the preceding quietstamp68 record.
No measured post-edge improvement; IRQunion difference is not an attributed
gain from a preparation-only change. One pass is not a reliability cohort or
permission to widen the profile. Do not repeat6.9 solely on this basis.

Actual1717 now installed, stopped, final MOE/CCRs/gates/ENABLE zero, UART closed.
Next retire the no-gain opt-in from the campaign configuration (retain evidence)
and target the remaining acquisition/guard path, not more carrier relocation.
