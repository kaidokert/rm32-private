# Comparator read call-boundary experiment — E536, 2026-09-14

Latest E537: bounded comparison completed; call variant RETIRED, same fault.
Inline7E55 restored/off. Historical staged status below superseded by E537.

Status: two frozen release-s/thin-LTO images built and disassembled, NEITHER
flashed or hardware-qualified. Actual board remains frozenBC876, last E535 off.

## Question and scope

Does changing only the static comparator read's call boundary change the
late-accept / CycleTiming behavior? AM32's emitted O3 persistence loop calls
getCompOutputLevel each read; binz's optimized static loop inlines its read.
Read count alone therefore does not specify physical persistence duration.
This tests a cadence difference, NOT a proposed speedup or exact AM32 timing.

Added bench-comp-read-call (depends bench-static-comp), which changes only
StaticComp::output_level from inline(always) to inline(never). Function body
is unchanged read_comp_level(true,true,false): fresh volatile COMP2 sample,
same polarity, no cache, no delay loop or recorder. It applies to static-path
reads including the core's final read, not solely the inner persistence loop.
Post-run COMPREADCALL marker identifies the experiment. Host requires explicit
--comp-read-call; unexpected, missing or duplicated markers are rejected.
Two host tests pass. All electrical/tracking/timing guards remain unchanged.

Alternative changeover test rejected as a one-variable BINZ experiment:
core_bench::observe_begin initializes old_routine to !LIVE_IRQ, and the powered
loop stops with reason2 if old_routine becomes true while not in polling mode.
Lowering shared minz-core's threshold alone therefore changes wrapper refusal
behavior too; it does not reproduce AM32's startup mode transition. No shared
minz-core edits made. This is distinct from the feasible AM32-only config test.

## Reproduction / artifacts

Common command:

```
cargo build --release --example shell-pwm --no-default-features --features bench-adc-phase,bench-cpu-union,bench-current-baseline,bench-dma-201,bench-dma-fast-start,bench-dma-peer,bench-driven-dma,bench-driven-reanchor,bench-guard-install,bench-irq-tail,bench-masked-seed-arm,bench-pwm-20k,bench-quiet-irq-stamp,bench-range350,bench-reentry-clear-once,bench-reentry-staging,bench-seed-div12,bench-single-core-atomics,bench-static-comp
```

Candidate adds bench-comp-read-call to that list. Neither side includes
qualification-direct/reject/sparse/event/window, comp-paths/decisions/critical
or scheduling-tail. Existing CPU accounting, feedback and accepted-event
evidence remain on both: call these comparison builds, not zero-diagnostic
production builds. Both generated emitted math audits; advisory, not no-helper
certificates. Release opt-level=s,lto=thin,codegen-units=1.

- Inline baseline: captures/reference/compinline_536_6d82/shell-pwm.elf,
  SHA2566D82B4776F678B38A506319F803C796C7B6A407A1D904521FB574AFD000E1396.
  Recognized15-instruction successful loop iteration, no call, one COMP2 load
  at08001caa. Current root build is this image, NOT installedBC876.
- Call candidate: captures/reference/compcall_536_20f5/shell-pwm.elf,
  SHA25620F5569A960CFC6F81F6EEB8A1AA52C4A1F58A5627ECBB6ECB1A35CFAD5343C1.
  Persistence calls helper08003f20 at08001cb2. Helper performs exactly one
  volatile COMP2 load08003f2a; literal40010204 at08003f34. A second call at
  08001e02 is the other core read. Helper has prologue/epilogue; do not equate
  its static instruction count with cycles or AM32's physical read spacing.

## Bounded next test and retirement

Before powered use, BOTH images need their own disabled guard/fullfive3200/
ADCroute checks and actual core-service timing check; use existing machinery,
not a new per-read recorder. Existing CPU pair gate <=10us remains. If the
call variant fails timing, retain refusal and stop this experiment, not tune
its layout or raise the gate. The binary change can move other code too.

If those pass, one baseline6.9%10s hold then one candidate6.9%10s hold, same
6.1%driven/+60deg/20k settings, explicit --dma-peer and candidate marker flag.
Only if preceding checks/current/bus/tracking pass, one7.0%10s boundary attempt
per image with ALL existing guards. No retry-until-pass or speed-cap change.
Retain all outcomes and compare cost, acceptance timing and fault. Improvement
would motivate reference qualification-window investigation; it is not proof
of the precise mechanism or a reason to carry added call overhead permanently.
Retire/freeze after this bounded comparison; no new histogram/row collector.

## E537 — cadence changes, same CycleTiming fault; retire candidate

Corrected a prerequisite error: cpucheck measures meter pairs, not comparator
service. Added idle-only readcadencecheck on both builds:64 spans of12 fresh
StaticComp reads, numeric black_box, runtime-opaque loop bound, TIM17 endpoints.
No IRQ masking or live ISR hook. This is read-span timing INCLUDING harness,
not actual persistence-loop/ISR WCET. Explicit acceptance bound max10us for
this harness is separate from the pre-existing <=10us CPU-meter gate. No
claim that the earlier plan's full-core disabled timing was supplied by this.
Live service maxima remain reported and existing independent shutdowns remain.

Both rebuiltrelease-s/thinLTO and emitted math-audited/frozen:
- Inline7E55F5D0799CE677519928E4CC61E7F486B7A27E1FB7B8738EC09AC3D3E82CA6,
  captures/reference/compinline_537/shell-pwm.elf (current/root).
- CallDADD5C1D1E34979B5724EE1631305212C32EFE9A2E045B72974255D23EF43405,
  captures/reference/compcall_537/shell-pwm.elf (retired).

Each own guard3/18, fullfive3200preflight CPU2/6/9, ADCroute3checks PASS.
Inline read span3..4us, call8..9us. Three host tests (marker2+span1) PASS,
including malformed/excessive-cost/mode/finaloff refusal and actual wire.
No per-read trace, direct collector, or new live instrument on either side.

| Metric | Inline | Call |
|---|---:|---:|
|Requested hold|10s|10s|
|Observed duration us|10000040|8804455|
|Outcome|normal deadline|CycleTiming12|
|Accepted-rate eHz|333.638|335.692|
|COM / accepted|20018 / 20018|17733 / 17732|
|IRQ union %|51.5246|51.8802|
|Raw current deviation peak|298|308|
|Minimum bus mV|10913|11128|
|Delivered ADC scans / PWM ticks|49750 / 3200|43802 / 3200|
|ADC trigger-phase records|49751|43803|
|Untouched stack B|3460|3460|

Same6.1%driven,+60deg,6.9%BEMF/20kHz, --core-trace0 --dma-peer;
candidate additionally --comp-read-call. Inline full window verifier PASS;
candidate correctly FAILED. Marker/ADC/CPU/finaloff independently validate.
Candidate fault step2 previous8801576/decision8804422us:2846<2858us. Core
average989ticks,filter12,running1/polling0 before return after safing. Prior
reference cycle6173ticks,refused5685ticks; two-cycle mean2964.5us. Not physical
rotor period or proof of harmlessness. No guard or threshold changed.

Captures compinline_537_start61_hold69_10s_01.txt SHA256
1247FDACB49BFCBE478AC05A716A77CD748DF7E49242CEAA5D234394717B9F0C;
compcall_537_start61_hold69_10s_01.txt SHA256
F00E10EDD199D54DD77DF2531C2B85BF945F4128C3C63B9CA374D4953952E7B2.
No7.0% attempts after candidate failure. No retries. Restored inline7E55 after
verifiedoff/download/OpenOCD reset; freshguard3/18/finaloffPASS. UART closed.

Conclusion: this call-boundary intervention did not eliminate the failure.
One run each, slightly different speed, not statistical superiority proof or
exact AM32 cadence matching. Retire rather than tune call padding/loop layout.
This baseline has a10s hold result; not a new recovery reliability cohort.

## E538 — replay correction and expansion evidence

Replayed both E537 captures through drv_current_sums.decode and
drv_adc_phase.decode. The original table conflated trigger-phase records with
delivered current scans. Corrected above; raw captures are unchanged. E535
has49751 for both, so its published count was correct. These different counters
must not be interchanged or treated as a CRC failure: each decoder validates
its own count and framing, and phase recording precedes FIFO delivery.

The inline initial-segment drv_baseline_residual report gives powered centered
means[8.982090,9.765729,10.359558] counts, prestart means
[6.046875,8.968750,7.062500], residuals[2.935215,0.796979,3.297058],
sum7.029252 counts over49750 scans. Same initial wake is declared, but
stationarity/drift/calibration remain unverified; amps remains null. This is
not permission to substitute a nominal-gain current estimate for measured
operating current.

Source recheck: powered_timer selects RunGuard<2858,238>; accepted() compares
each sector's accepted-event timestamp with its preceding same-sector time.
The2858us floor is an instantaneous accepted-cycle envelope, not a CPU-load
measurement or independent rotor-speed measurement. The inline capture's
minimum2900us leaves42us to that floor. Do not infer a silicon ceiling from
this refusal, or silently remove it. Goal permits deliberate speed-profile
expansion after preceding sensing/timing/current/bus/tracking validation.

Missing independent current fact at the current operating point remains:
operator's~70mA observation belongs to E188/~238eHz, not E537/~334eHz.
Requested readiness to observe PSU voltage/current/CV-CC during a finite
existing-setting hold. No powered run while awaiting that observation, no
firmware/threshold change, and no restart of baseline-mode/settling experiments.

## E540 — first current-inline recovery qualification

Root ELF still7E55F5D0799CE677519928E4CC61E7F486B7A27E1FB7B8738EC09AC3D3E82CA6;
no flash/source change. Fresh disabled guard3faults/18refusals/finaloff PASS
(captures/inline_540_guard01.txt). One existing-setting recovery, not the
pending operator-observed PSU measurement:

```
python scripts/drv_driven_handoff.py --out captures/inline_540_start61_reentry69_30s_01.txt --ms 30000 --drive-duty 61 --bemf-duty 69 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --dropout --reentry
```

PASS initial driven qualification, injected Tracking shutdown, fresh-seed
recovery, original deadline and final outputs-off. Resumed27.991064s,
333.437206eHz,56000COM/55999accepted,full-cycle sigma22.303722us.
Rawpeak350,busminimum10925mV; software IRQunion50.659776%,not total CPU
utilization/WCET. Untouched stack2668B. Fresh seed1007half-us ticks,
acquisition6206us,remaining43us,arm7us; margin above32us is11us.
Final elapsed34711527 versus original end34711763us:236us spare.
ADC sums and trigger-phase decoders each validate139259 records,201us
cadence/3200PWMticks. Recovery revoked initial baseline (BASEEPOCH matches0),
so no baseline-subtracted current or calibrated-amps claim. PSU display
observation still unavailable here; old~70mA is not reused.

Capture SHA256330a144198aa37ec458db122e84afcafbc46051b28784ea6bd989101ca94e884.
UART closed by fixture, outputs-off verified. One attempt/one pass, not a
repeatability cohort or independent qZC proof. No duty/speed-limit expansion.

## E541 — fixed three-attempt recovery cohort complete

After E540's first pass, predeclared two additional identical attempts,
stopping on any failure. Both pass; no exclusions/retries. No build/flash or
guard change. Fresh inline_541_guard01.txt passes3faults/18refusals/off.
Same E540 command, output names below; actual duty6.9%,20kHz,+60deg,
6.1%driven startup, one Tracking injection and fresh-seed recovery per attempt.

| Resumed metric | E540 01 | E541 02 | E541 03 |
|---|---:|---:|---:|
| Duration s |27.991064|27.991137|27.991440|
| Accepted-rate eHz |333.437206|333.528263|333.578842|
| Full-cycle sigma us |22.303722|22.779997|23.052446|
| COM / accepted |56000/55999|56015/56014|56024/56024|
| IRQ union percent |50.659776|50.875345|51.118798|
| Raw current peak |350|347|343|
| Bus minimum mV |10925|10829|10937|
| Delivered ADC / phase records |139259/139259|139259/139259|139261/139261|
| Original deadline spare us |236|131|261|

All have fresh remaining43us/arm7us, stackuntouched2668B, verified recovery,
original timeline and final outputs-off. ADC/phase CRC/count decoders and IRQ
accounting independently pass. Current still uncalibrated; recovery revokes
the initial baseline. No contemporaneous independent PSU display reading.

New capture hashes:
- inline_541_start61_reentry69_30s_02.txt:
  6f1743e7f1e4907381833b8effd82f50e480f4c02521334b1677118bba34d71d
- inline_541_start61_reentry69_30s_03.txt:
  a45febde2b68851b76be64fc00f7790ed386f14d20076d5707d02c1b1e258f64

Current7E55 now has3/3 at this point, not inherited from9ACC. This small
cohort is not a reliability-rate guarantee or independent qZC measurement.
Stop identical repeats; higher-speed refusal and operating-current anchor
remain open. Finaloff verified/UARTclosed; no limit expansion this entry.

## E542 — acceptance timestamp audit: do not move the guard clock as a fix

Compared frozen minz_20260912.zip minz/core/src/am32_isr.rs:109–118 with
current AM32/Src/main.c interruptRoutine and the binz adapters. Both control
sequences mask sensing, shift last/this interval, read/reset the interval
counter and arm COM with wait+1 under exclusion. Frozen minz then emits
EV_ACC outside that exclusion. Binz Obs::record first calls
powered_timer::accepted, which samples the guard clock under exclusion;
its later recorder sample is a third timestamp. These are not interchangeable.

Replayed E537 call fault through drv_cycle_fault.context: guard2846us versus
reference sum5685ticks=2842.5us, same accepted-sector span, net closure+3.5us.
The reference cycle itself is below2858us. Moving the cycle check to that
reference timestamp therefore would NOT make this fault pass under the same
floor. Fifteen cycle-fault tests pass. Closure is not an ISR-latency bound:
read/reset overhead and endpoint-delay differences can cancel. It does rule
against claiming the entire short-cycle refusal was invented solely by the
post-arm guard timestamp in this capture. Physical rotor speed remains unknown.

One source-level adapter difference exists: binz com_timer::set_and_enable
stops TIM16, disables DIER, clears CNT/SR and NVIC pending, then restarts;
AM32's G071 macro writes CNT/ARR/SR and enables DIER on the running timer.
Both use wait+1. No measured timing difference or cause established by these
source operations alone; do not remove pending-clear/restart semantics without
checking stale-event and stopped-timer behavior. No adapter change proposed
as a demonstrated cure, and no new instrument, firmware or motor run here.

Next investigation must distinguish accepted-boundary jitter from actual
tracking loss rather than relocate timestamps to hide the existing refusal.
The repeated6.9% cohort stays qualified; current anchor and justified profile
expansion remain open. Memo's hardware/preemption claims are not revived.
