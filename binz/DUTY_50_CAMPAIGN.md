# binz duty-to-50 campaign

## 2026-09-19 replacement-motor 5–50% curve and 75% forecast

The opt-in diagnostic image SHA `10CD4D47...` (reverse48k, high-duty COM-top
control as qualified image, plus `bench-rate-curve`) was built release-hybrid
opt-s/thinLTO/codegen1, passed the four motor ISR-root soft-arithmetic audit,
explicit G071 flash/verify/reset and disabled guard3/18+role6/6+duty6/6
preflights. It leaves nominal4A foldback, nFAULT, tracking, watchdog and the
three-scan5% fast-sag **hard stop** active. One protected run at each10–50%
5-point rung used a1%-per0.5s ramp and measured~15–19s only after the final
target ACK. Every run ended on normal deadline with zero foldback, fast-sag,
tracking, nFAULT or final-output fault. The 10–50% captures are
`captures/reverse48k_rate_curve_{10,15,20,25,30,35,40,45,50}_20260919.txt`.

| Duty | final eHz | signed-current proxy mA | approx board bus V | accepted gaps >125% of prior average | sub95% bus streak max |
|---:|---:|---:|---:|---:|---:|
| 10% | 401 | 45 | 11.66 | 0.00% | 1 |
| 15% | 704 | 58 | 11.66 | 0.35% | 1 |
| 20% | 941 | 167 | 11.65 | 1.46% | 1 |
| 25% | 1186 | 326 | 11.63 | 2.14% | 1 |
| 30% | 1371 | 519 | 11.61 | 3.42% | 1 |
| 35% | 1564 | 740 | 11.60 | 4.20% | 1 |
| 40% | 1736 | 945 | 11.57 | 5.31% | 1 |
| 45% | 1893 | 1237 | 11.54 | 6.74% | 1 |
| 50% | 2096 | 1603 | 11.52 | 6.82% | 1 |

The current column is the average **signed three-shunt residual** over
completed100-scan blocks, scaled by each run's nominal4A raw allowance;
it is neither a calibrated DC-link/PSU current nor a PWM peak. The earlier
operator PSU read of about1.6A at49% is useful rough corroboration, not a
calibration of this image. Bus values are computed from coherent `bus_sum /
vref_sum`, the cached G071 VREF factory code1662 and the DRV board11.94
divider ratio; call them approximate board-sense volts, not a PSU-terminal
measurement. Sub95% isolated scans rise from0.04% to0.154% of scans, but no
run had a streak longer than1, so the specified three-scan guard did not
trip on these single-sample notches. The late-gap rate rises with speed but
mostly comprises isolated events; it is not a standalone lock certificate.
The table is one diagnostic run per rung, while10/25/50 hold/recovery
qualification is separately on the frozen low-observer image below.

One protected5% endpoint check on the restored qualified image ACKed `du50`
after running at10%, then stopped Tracking8 at2.114475s powered. It had no
fast-sag/foldback/nFAULT event, and final outputs were off. This verifies
that5% is **not a sustained BEMF point** on this replacement motor at this
voltage; it is not a zero-current or zero-speed measurement.

CPU was measured on a different opt-in `bench-cpu-target-epoch` image SHA
`C1746D3D...` which samples every17th TIM6/COMP/COM/DMA invocation and
resets the sample window at the final5%-rung ACK. Its protected40% run
completed normally; over18.368s at target the inclusive, probe-affected
root estimates were guard4.0%, COMP35.6%, COM30.3%, DMA12.2%, sum82.1%.
The attempt toward50 fast-sag-stopped at last ACKed47% (50% **never ACKed**)
after1.412s in the45%-rung sample window; corresponding inclusive roots
were4.0/34.9/34.0/12.3%, sum85.2%. These sums double-count preemption,
include probe cost, and are NOT lean IRQ union or measured50% utilization.
The qualified lean image held50%3/3, so the diagnostic stop is observer
effect, not a new lean ceiling. Captures:
`captures/reverse48k_cpu_epoch_{40,50}_20260919.txt`. The qualified SHA
`0C734C67...` was re-flashed/verified/reset after this diagnostic; disabled
checks passed, outputs off and COM41 closed.

**Projection toward75%, explicitly uncertain:** the10–50% curve reaches
~2.10keHz at50. Linear extrapolation of the40–50% speed increment would put
75 near3.0keHz, or a56us commutation period; this is only a planning number,
not a predicted achieved speed. At~47% the sparse diagnostic COMP+COM mean
services total about47us inclusive per commutation, before DMA/guard time;
therefore the present software schedule is the leading *risk* for75 even
though lean WCET remains unmeasured. Signed current rises increasingly fast
near50: a straight final-segment extrapolation gives~3.4A at75, while a
curved continuation can exceed the configured4A foldback. Neither is a
credible current limit forecast because aerodynamic load, timing quality and
transient peaks may change. The steady board bus fell only~0.14V from10 to50;
the prior dangerous sag was a fast control-associated dip, not this steady
droop. The next campaign would need code-level ISR/COM latency reduction and
new guarded measurements above50, not a PSU-cap increase or guard relaxation.

## 2026-09-19 same-image 10/25/50 dwell and ordinary-restart milestone

Frozen reverse48k/effective-advance26 high-duty COM-priority A/B image SHA
`0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0`
at `captures/reference/reverse_48k_com_top_high_20260919/shell-pwm.elf`
keeps COMP at priority0x40, guard and DMA at0, and changes COM from0x40
to0 only for ACKed duty>=48%; below48% it remains the existing peer. The
comparator dispatch snapshots the pre-arm sector so a preempting COM cannot
mislabel the accepted edge after advancing `current_step`. All electrical
stops and nominal4A signed-average foldback remain active. This changes
control scheduling, not a protection threshold. Release-hybrid opt-s,
thinLTO/codegen1, four motor ISR-root soft-arithmetic audit, explicit G071
flash/verify/reset and disabled guard3/18, reverse role6/6, duty6/6, p/i
off/nFAULT1 passed. It is a lean-core/lean-IRQ control candidate with only
foreground target-ACK reporting; no COM-lag, interval, ADC-ring or CPU probe.

After a clean10% gate and a clean48%/35s powered gate, one exploratory50%
run completed31.334156s from target ACK with no fault. A **predeclared three-run
cohort on the unchanged image** then completed true>=30s target dwells:

| Duty | ACK-to-stop seconds, A/B/C | Result |
|---|---|---|
| 10% | 31.691726 / 31.689313 / 31.684040 | 3/3 |
| 25% | 32.024435 / 32.011462 / 32.004254 | 3/3 |
| 50% | 31.280030 / 31.275623 / 31.238969 | 3/3 |

All nine ended at the normal requested deadline, with zero fast-sag trips,
current foldbacks, phase-rail counts, tracking faults, or nFAULT stops;
outputs were disabled and nFAULT high. The10%A lower bound uses the last real
accepted BEMF event within1ms of the requested foreground stop; all other
dwells use the powered timer stop stamp. At50%, final speed estimates were
2123/2096/2123 eHz; this is a point estimate, not a speed-vs-time variance
measurement. The fixture still prints `exploration_only=True` by design for
this command path, but its explicit `--qualify-hold` parser verified each
MCU target ACK, >=30s dwell, uninterrupted protected run and final-off state.

Three separately planned80s normal-start tests each injected Tracking8 at
~5.000s, settled disabled for1s, performed fresh ordinary startup/handoff,
restored the retained50% request in eight foreground5%-per-2s steps, and
completed the remaining ~69.236s powered segment at normal deadline with
zero fast-sag/foldback or second tracking stop. One exact-image ordinary
restart regression at10% restored immediately (zero steps) and one at25%
restored in three steps; both completed their remaining windows. The postrun
`NORMALRESTART3 resume_applied=500` proves restoration, but automatic resume
has no separate MCU `LIVEACK` target-dwell stamp, so do not claim an exact
50%-only dwell within those recovery windows. Strict final serial `off/p/i`
again showed all gates/ENABLE/MOE/CCRs0 and nFAULT1; COM41 closed. Current
installed board image is the SHA above, OFF.

This is now a **qualified 50% bench operating point under the defined 30s
hold+ordinary-restart tests**, a material improvement over predecessor SHA
`79BDA4C57...`'s2/3 true50% dwell cohort. It does not prove the timing
intervention uniquely caused the improvement: the predecessor fault was
intermittent, the sample is small, and no matched high-duty CPU/current/bus
curve has yet been collected. Do not erase the prior failures or describe
this as a75% prediction. Remaining goal work: low-observer current/bus/timing
and CPU characterization across the5-50% envelope, then an explicitly
uncertain75% bottleneck projection. Captures are
`captures/reverse48k_com_top_{10cohort,25cohort,50cohort,50restart}_*.txt`.

## 2026-09-19 guarded COM-lag diagnostic: programmed advance is mostly consumed in COMP

The operator rejected a proposed fast-sag-report-only experiment after the
earlier motor burn. Correct: **fast-sag stayed a hard stop** in every run here.
The raw three-scan fault replay passes the existing `u32` comparison without
overflow; a 60 s sampled-clock wrap test passes. These tests do not rule out
all controller arithmetic or scheduling faults, but the particular fast-sag
comparison and long-run clock overflow theories lack evidence.

Two opt-in, postrun-only COM-lag diagnostic images sampled one in sixteen
commutations above48% duty. The first SHA `AB89155F...` logged actual TIM2
half-us ticks at COM entry versus the previously armed wait. A protected10%
gate completed; its50% attempt fast-sag-stopped after2.814449s true target
dwell. COMLAG:3017 samples, actual20..84 half-us ticks, programmed13..21,
3017/3017 more than1us beyond programmed wait. That difference **also includes
timer-arm and IRQ-entry latency**, so it alone does not prove comparator-ISR
overrun. Capture `captures/reverse48k_comlag_50_20260919.txt`.

The paired-exit image SHA `1F0F2369...` additionally sampled TIM2 at the end
of the same accepted comparator ISR, keyed by the commutation index. Its10%
gate completed; the guarded50% attempt fast-sag-stopped after1.579986s true
target dwell. All2031 COM samples paired with a comparator exit. On1869/2031
(92.0%), **the comparator ISR was still active when its programmed wait
expired** (`exit_count >= wait`); exit maximum26 half-us=13us, actual COM-entry
range21..85 half-us, wait13..20. This establishes a scheduling floor on this
diagnostic image at50%, not that the floor alone caused the bus sag or that
every lean-image commutation has the same latency. The lean50% image also
intermittently trips fast-sag, so the probe is not necessary for the fault.
Capture `captures/reverse48k_comlag_exit_50_20260919.txt`.

Both diagnostic builds passed release-hybrid (opt-s/thinLTO/codegen1), the four
motor ISR-root soft-arithmetic audit, explicit G071 flash/verify, and disabled
guard3/18+role6/6+duty6/6 preflights. Both postrun outputs were off. Frozen
lean SHA `79BDA4C57...` was then re-flashed/verified, reset, and passed the
same disabled checks; COM41 closed. Neither diagnostic hold qualifies50%.
Next control work should shorten or reschedule the accepted comparator path
while preserving event ordering. Simply raising COM priority can preempt the
recording of the very accepted edge that armed it and risks a sector/guard race;
do not deploy it without an ordering design and low-duty gate.

## 2026-09-19 target-dwell accounting correction and current test

The host's `--ms` is the **total powered window**, including startup and live
throttle ramp. Prior statements calling a30s powered window a30s hold at48%
or50% overclaim: a1%-per0.5s ramp from10% to50% takes roughly20s after
startup. None of those pre-stamp captures prove a30s target-duty dwell. The
earlier level26 image completed three30s powered windows at50%, then its60s
window fastbus-stopped at41.153s powered (roughly18s after the50% command,
not a precise ACK-stamped value). Its3/3 ordinary-restart trials completed
the remaining powered window after restoring50% in steps, not a30s restored
target dwell. The physical4.5A PSU limit and all electrical stops were intact.

An opt-in `bench-target-ack-stamp` now records MCU powered time when a guarded
10/25/50% live command is accepted; postrun `LIVEACK` gives ACK-to-stop age.
`scripts/live_armed_baseline.py --qualify-hold` requires an explicit
`--target-hold-ms` and verifies the stamp, rather than the powered window.
Four host stamp/parser tests pass. The current frozen image is reverse48k,
effective high-duty advance26, SHA
`79BDA4C57DCD8D097B25E084F96C1812582D38232768F25D5307332E36EC0CA9`;
the binz-only post-COM wait override is witnessed at high duty. Explicit
G071 flash/verify and disabled guard/role/duty/output checks passed. Current
foldback nominal4A, fast three-scan 5% bus stop, nFAULT, tracking and watchdog
remain. This is an exploration image with postrun instrumentation, **not yet
a zero-diagnostic lean qualification image**.

Three separate 10% runs each completed a true >=30s target dwell, measured
from MCU ACK to normal stop: 30.701017, 30.689944 and 30.723852s. Each had
zero fastbus/current foldback and final outputs off. The 25% cohort passed3/3
true30s target dwells: 31.187478, 31.198311 and31.175032s, each zero
fastbus/foldback and final outputs off. The50% true-dwell test is in progress;
50% remains **unqualified** pending the repeated cohort and recovery on this
image. The first 50% run on this image completed the 55s powered window with
the first50% ACK at23.632885s; its last real accepted event was at54.999917s,
giving a conservative target-dwell lower bound31.367032s. No fastbus/current
foldback/phase rail/tracking/nFAULT; outputs off. This normal foreground
deadline stop leaves `POWERPATH stop_us=0`, so the initial verifier rejected
the run as an accounting error. It was fixed host-side to accept only a normal
foreground stop with a clean TRACKSTOP event within1ms of the requested
deadline; six unit tests pass and replay of the retained raw capture passes.
The second 50% run ended by TIM6's normal deadline, directly stamped31.359143s
at target with the same zero-fault result. The third, unchanged-firmware run
fastbus-stopped at44.770010s powered, **21.158641s after the50% ACK**;
`phase_rail_codes=2`, foldback0, tracking event_fault0, nFAULT high and final
outputs off. This is a2/3 cohort, **not** 50% qualification. Do not repeat
until a passing lottery draw; next capture must address onset cause at high
duty. The operator's
approximately1.6A PSU-display observation was during an earlier49% effective24
run, not a calibrated current value for this image.

### Fault-tail differential at effective advance26

The diagnostic SHA `A074D0E0B9CB0C049CCCB62A14F541B06EF2B03C3B526EC951BB9BC4F35193D4`
added IT86 last128 accepted intervals and BS85 last16 coherent DMA frames
to the protected advance26 ACK image. The four motor ISR-root soft-arithmetic
checks, explicit G071 flash verify and disabled guard/role/duty/output checks
passed. This is an observer-bearing image, not a qualification transfer.
In a predeclared three-attempt50% set, A/C finished the55s powered deadline;
B fastbus-stopped at29.611156s powered,5.945094s after the50% ACK. All three
ended outputs off with nFAULT high and no current foldback.

The clean A/C last21 complete electrical cycles had no period above520us
(A max513, C max500). B's final two complete cycles were549 and562us;
its first136us accepted gap ended at29.609834s, after a bus scan still at
98.38% of reference at29.609712s. The next scan at29.609938s was96.74%;
first sub95% scan came at29.610616s, and the three-scan guard shut down
within ~0.54ms thereafter. Coherent phase-current samples became extreme
in the suffix (last raw IA14/IB4089), but asynchronous PWM-phase samples
are neither a DC-link average nor calibrated thermal current. The sequence
repeats the earlier timing-first/bus-later finding at50% with level26.
The accepted-event clock alone cannot distinguish a genuinely slowing
rotor from delayed comparator qualification; it also cannot prove the
source of the current transient. Do not interpret 1.6A PSU average at49%
as evidence those phase-current peaks are harmless. Exact A/B/C captures
are `captures/reverse_direction_2026-09-19_advance26diag_it86bs85_50_*.txt`.
The low-observer SHA79BDA4... was restored/verified/reset; disabled preflights
and final outputs-off/nFAULT-high readbacks passed, COM41 closed. No more
motor runs followed the diagnostic set.

### 2026-09-19 arithmetic/observer audit of the fast-bus trip

The operator suspected a large-number or observation-instrument error rather
than PSU capacity. This is plausible enough to test, but the *fast-bus decision*
itself is not an overflow: `fast_bus_sag::Guard::observe` compares
`bus * reference_vref * 100` with `reference_bus * vref * 95`, using `u32`.
With validated 12-bit inputs, the largest product is1,676,902,500 (<2^31).
The streak is only0..3 and latches at3; no elapsed-time or event counter enters
the comparison. A new source-level test replays fault B's raw pretrip and trip
frames and passes all four guard tests. For B's reference1212/1506, the last
three normalized bus samples were **91.81%, 94.35%, 90.41%**; the comparison
operands are respectively167,467,200<173,285,700;
171,985,200<173,170,560; and166,111,800<174,552,240. VREF stayed
1504..1516. Those are real ADC-code decreases, not a math wrap in the guard.
The same kind of fastbus fault occurred on the low-observer image as on the
IT86/BS85 image, so these added recorders are not *necessary* for the failure;
their presence did move its timing and cannot be declared neutral.

`GRAYBEARD_SAG_REFERENCE.md` proposes replacing the fixed set-voltage
reference with a filtered loaded reference. That is **not adopted** here: fault
B's pre-onset bus was about98..99% of the fixed reference, not an established
96..97% steady level, and its fault occurred only5.945s after50% ACK, not at
a fixed20..30s appointment. A hypothetical98.5% moving reference would put
the95%-of-recent line near93.6% of set; the middle94.35% sample would break
this very trip's three-scan streak despite the neighboring91.81/90.41% scans.
The change therefore alters an operator-critical protection and could mask a
sub-millisecond power event; it is not a demonstrated correction. The
upstream cause of the late accepted edge/rotor slowdown and brief bus/current
transient remains open. PSU current-limit headroom at ~1.6A average does not
measure sub-millisecond phase or bus transients; it also does not prove a
thermal mechanism. No slow-drift conclusion is justified by these captures.
The sampled 16-bit TIM17 extension was separately regression-tested through
60s at100us read cadence, crossing both2^24 and2^25 microsecond boundaries
and about915 hardware wraps; all six clock tests pass. The current diagnostic
fault at29.611s powered, the lean fault at44.770s powered, and clean55s
runs do not share a fixed counter boundary. This does not clear *all* control
math or ISR scheduling, but it narrows the large-number hypothesis away from
the fast-bus comparator and primary segment clock.
The simple "a counter got too large" variant is also contradicted by the
cohorts: the low-observer failure had435,347 COMs while two unchanged-image
clean runs passed564,026/564,351; the diagnostic failure had244,271 COMs
while its clean controls passed563,311/564,788. Thus neither observer's
event count nor the powered microsecond count has a deterministic failure
threshold below the values the clean runs reached. A state- or
event-dependent control arithmetic error remains possible and is the next
software investigation, not a reason to relax the bus guard.

## 2026-09-19 sparse IRQ workload, not a lean utilization claim

The every17th-entry CPU probe made two45% attempts fastbus-stop at~7.08s
powered, shortly after the ACK; the lean controller had held45% for60s.
Both sparse runs nevertheless agreed on sampled means (guard3.96us,
COMP16.6-16.7us, COM27.8us, DMA27.6us) and an inclusive summed workload
upper bound75.76/75.83% across the35->45 climb. This double-counts nested
interrupt time and is observer-sensitive; it does not prove lean CPU
saturation. The exact lean staircase image was reflashed and disabled checks
passed afterward. A high-duty comparator-priority A/B is the next test of
whether a late physical accept is a scheduling delay.

## 2026-09-19 CPU observer check

An aggregate-CPU diagnostic that starts only after live duty>=35% failed
fastbus at40% before reaching45, whereas lean control held45%60s. It measured
75.88% IRQ union over2.016s traversing35-40 (guard1.87%, COMP30.81%, COM29.33%,
DMA13.86% of elapsed, inclusive-root attribution). Its own per-IRQ timestamp
and critical-section cost is not calibrated, so **75.88% is observer-affected**
and cannot be projected as lean occupancy or declared the limit. The image and
capture are retained. The next measurement is the existing every17th-entry
sparse bracket, which samples ISR cost with lower perturbation.

## 2026-09-19 accepted-origin vs bus timing

An IT87+BS85 origin diagnostic reached47% but fastbus-stopped before48.
At the47% epoch it counted physical-only2829, software-only83, both69;
late-by->25% counts were171/27/0. Software-only accepts increased from1
in the earlier64 retained events to11 in the final64 (8 of the final20).
However the first obvious cycle extension (537us) ended at22.455046s with
a132us **physical-only** accepted gap; a bus scan at22.455154s was still
~98% of reference, and the first sub95% scan came at22.456284s. Revisit
activity spiked later. It is therefore not supported as the first cause in
this capture. Extra origin instrumentation is observer-affecting; this is a
causal timing clue, not lean qualification or proof of analog-edge quality.
Capture `captures/reverse_direction_2026-09-19_origintail_48_b.txt`.

## 2026-09-19 replacement-motor 45%-clean / 48%-marginal boundary

On the protected reverse48k staircase image with revisit active below50%,
45% held60s at ~1926eHz, zero current foldback and fast bus stops. A slower
1%-step climb then stopped fastbus at49% before the50 command was ACKed;
another reached48% but stopped fastbus there. The off-at50 feature never
switched modes and has **no A/B verdict**. All runs ended with outputs off.

A separate fault-only diagnostic (SHA E235D083..., IT86 accepted intervals +
eight coherent bus/current scans) made three predeclared48% attempts. Two
fastbus-stopped and one had Tracking8 followed by a safe ordinary restart at
10% for the short remaining window; all first segments reached48, none held
30s, none folded current or tripped nFAULT. In fastbus A, a cycle grew from
~500 to615us and phase current hit a rail before the first sub95% bus scan.
In fastbus B, three ~521-534us cycles preceded the first sub95% scan versus
~474-504us earlier. This repeats the **timing-first, bus-collapse-later**
sequence without claiming whether accepted-event timing itself is the root
or a measurement of rotor deceleration. The third run's Tracking8 without
fastbus shows the same duty band can fail through a different guard. IT86
CRC/ordinals decode; exact speed-watch transition replay of B is unresolved.
These diagnostic results cannot qualify the lean envelope. Next discriminate
revisit-origin and per-sector late events at the boundary, then test a causal
control change under the same electrical stops. Do not repeat blind50 climbs.

## 2026-09-19 measured seed-arm margin (low-duty startup)

An explicit12-start/10s cohort on the protected staircase image with only
post-run seed-stage reporting produced10 full powered completions and2 late
seed-arm refusals. A second8-start/15s prefix probe produced6 completions
and2 refusals. The failed selected edge was already90-91us old at handoff
entry; bridge clear/filter/cold-state/reset, feedback and guard setup then
spent~37-41us before timer arm. Only13-16us remained against the unchanged
20us reserve, so the refusal was correct. The ~5-6us redundant bridge clear
was tested feature-gated:11/12 starts versus10/12 control, with one identical
late refusal. That is inconclusive and insufficient as a fix; retire it for
the envelope campaign. No motor or bus fault occurred in these refusals.

The host strict-hold verifier now accepts either normal timed stop: TIM6
reason2 or foreground `coast_stop=1` with POWERPATH reason0 and a recent real
accepted event at the deadline. Earlier10s captures that the old verifier
rejected were replayed; no failed seed was reclassified. Captures
`captures/reverse_direction_2026-09-19_seedstage_*`, `*_seedprefix_*`,
`*_clearonce_*`; frozen diagnostic images under matching reference folders.

## 2026-09-19 replacement-motor staircase and low-duty qualification

Current installed protected reverse48k lean SHA
`F6F88F85019CA22F7EC37D7A2F7A1B1294984E887D2B35DC3DCD806B64C9DA26`
uses the existing autonomous startup staircase for both first and ordinary
restart. Disabled guard3/18, roledu1006/6, duty6/6, explicit G071 flash verify,
four motor ISR-root soft-math audits and final-off readbacks passed. The 4A
signed-average adaptive current policy, >5%/three-scan fast bus stop, nFAULT,
tracking and watchdog remain active. No sibling rm32 changes.

On this exact image, **10% hold 3/3** completed 30s uninterrupted, and **10%
ordinary restart 3/3** completed after injected Tracking8 with 1s disabled
settle. **25% ordinary restart 3/3** likewise restored 10->15->20->25% in
three 2s steps and completed. The **25% hold cohort is 2/3**, not qualified:
two uninterrupted 30s holds passed, one refused before the powered hold. Its
`DRIVENENTRY refusal=4`, `coast_stop=8`, `fly_age=260`, `fly_arr=MAX` point to
a late seed-arm handoff: about 14us remained versus the existing20us reserve.
Successful first-arm ages were196..236 half-us ticks and actual arm writes
6..7us. All attempts ended off; no fastbus/current/nFAULT event occurred in
the failed one. Next investigate stage timing rather than silently shrinking
the reserve. Captures `captures/reverse_direction_2026-09-19_staircase_newmotor_*`.

Matched revisit-at-48 on/off controls were inconclusive: each suffered one
Tracking8 near48-49%, then refused second startup; neither reached50. An
intermediate image without `bench-fast-cycle-report` wrongly reactivated the
single-cycle kill and its 11% stop is excluded. The report-only feature was
restored for the matched images. The high-duty 50% fast-sag wall remains
unqualified, and this low-duty startup work does not erase it.

## 2026-09-19 smooth-entry control at 50%

One more protected run used 1%-per-0.5s steps rather than a final5% jump.
It ACKed50 at powered23.641419s and fastbus-stopped reason26 at23.808726s:
**167307 µs after50 ACK**, again with no current foldback, tracking, or
nFAULT stop. The three stamped post-50 ages are131851/155129/167307µs,
despite materially different climb times. The large final step is therefore
not necessary for the event, and a seconds-long wall-clock buildup is less
plausible. Neither conclusion identifies whether controller timing, phase
current, or bus physics began first. The existing 32k rate-census study
already showed coarse low-scan/late-gap rates did not predict a terminal
streak; a repeat of that counter alone is not the next causal test.
Capture: `captures/reverse_direction_2026-09-19_restart_ackstamp_50pct_smooth1.txt`.
50% remains unqualified.

## 2026-09-19 50% ACK-to-stop measurement on replacement motor

The staged protected SHA72FF21BB image was flashed to the explicit G071,
passed disabled guard/role/duty checks, and ran a protected10%/15s hold
cleanly. A Tracking8 injection safed after5s, but its ordinary second
startup refused BEMF acquisition, so the upward-live-command restart fix is
host-tested only, not yet powered-verified. All attempts ended with outputs
off/nFAULT high and COM41 closed.

Two matched ACK-paced10→50% ramps each reached/ACKed50 and stopped on the
independent fast5%/three-scan bus guard. The new MCU timestamp locates the
terminal stop **131851 and155129 µs after the50% ACK**, respectively. Both
had zero current foldback, tracking, and nFAULT stops. One counted3 exact
phase ADC rails, the other0, so rails are not required for sag. These are
~0.15s post-step events under this faster ramp; they do not prove a fixed
latency or identify source. Earlier19.956/20.019s total powered times
cannot be converted to50%-dwell without an ACK timestamp.
50% remains unqualified. Captures:
`captures/reverse_direction_2026-09-19_restart_ackstamp_50pct_ramp_a.txt`
and `_b.txt`; exact image/readme under
`captures/reference/reverse_48k_restart_ackstamp_20260919`.

## 2026-09-19 restart command ownership and 50% timing

An aggregate-CPU diagnostic was run after the new-motor fast-sag captures. It
stopped Tracking8 near the40% step. During its automatic ordinary restart, a
live45% request was ACKed and immediately published at low speed, bypassing
the intended5%-per-2s resume schedule; a fastbus stop followed. The run is
not a valid sustained CPU measurement. Exact known protected reverse48k
cache-owner SHA42C9D49E was restored/verified; outputs off and COM41 closed.

The source now queues upward post-restart live requests as resume targets,
preserves the current rung/cadence, and reports the actually applied duty in
the ACK. A stale policy/current mismatch fails closed. Lower duty requests
can still apply immediately. Pure normal-restart policy tests pass7/7.
The staged diagnostic SHA72FF21BB adds a foreground-only timestamp to the
successfully queued50% ACK and prints its age at stop, with no ISR work.
It has not been flashed or powered-tested; see the frozen README under
`captures/reference/reverse_48k_restart_ackstamp_20260919`.

The two earlier new-motor fastbus stops were19.956141s and20.019369s in
powered time. Because those images did not timestamp the50% ACK, their63ms
separation does **not** establish a fixed delay after reaching50. A new-motor
running-level-revisit on/off A/B can compare low-bus and late-gap rates at a
matched duration; the old-motor revisit-off35 test already failed tracking,
so the new-motor result must not be inferred from it.

## 2026-09-19 replacement motor: 20% clean, 50% guarded

After the operator replaced the motor, protected reverse48k smoke runs held
10% for10s and reached20% in a16s run with zero electrical faults or current
foldback. The operator heard a quieter, smoother spin. This changes the
motor-under-test; all older motor envelope verdicts are historical only.

On diagnostic SHA69BB6A10, one live ramp ACKed25/30/35/40/45/50% and stopped
at50 on a single exact phase ADC rail (`Latest::publish` invalid), not on
fastbus, current, tracking or nFAULT. Source audit showed that cache veto
contradicted the selected current-owner policy, which already counts/averages
phase rails. A source-only A/B made phase 0/4095 cache-valid while preserving
bus/VREF rail, impossible DMA word, stale/reordered stops and every electrical
guard. Pure tests5/5, four ISR math audits and disabled hardware checks passed.
The A/B SHA42C9D49E counted a phase rail without ADCFAULT, then stopped on
the independent three-scan fastbus reason26 after50 ACK. Thus the cache
ownership defect was real, but it was not the only50% stop.

A separate causal-ring image SHA79670724 reproduced fastbus reason26 at50,
with 8 bus/VREF-normalized scans over1.8ms of97.29/95.77/97.57/95.71/
95.36/92.62/92.36/92.90%. Terminal phase codes included IA19, IA4038,
then IC0; accepted-event gaps included116,110,120us but commutations
continued. One long gap preceded the first sub95% scan, yet the bus was
already down to95.36% in the prior scan. These samples establish a brief
real bus/current/timing disturbance but not its causal direction; do not
label PSU or motor as the limit. All runs safed with gates/ENABLE/MOE/CCRs0,
nFAULT high, COM41 closed. The diagnostic image remains installed OFF.
50% was commanded, not qualified. Retained captures and exact SHA details:
`captures/reverse_direction_2026-09-19_new_motor_48k_explore50.txt`,
`captures/reverse_direction_2026-09-19_new_motor_48k_sag_ring50.txt`,
and `captures/reference/reverse_48k_cache_owner_20260919/README.md`.

## 2026-09-19 48 kHz protected exploration

Carrier-only A/B built from the frozen32k fast-sag PWM diagnostic:
48.012kHz ARR1332, active4A signed-current limiting, fast5%/
three-scan bus stop and all nFAULT/tracking/watchdog stops unchanged.
One correctly armed run held40%18s and45%~18s, then stopped only on
host `off` (reason9), zero foldback/electrical fault. A second run
held40%9s/45%5s and fast-sag stopped reason26 at last ACKed46.5%,
before47/50. The terminal bus samples were93.24/94.21/92.58% of
start and sampled at distinct OFF/near-compare/mid-ON carrier phases.
Thus the real bus-drop mechanism persists at48k. The single45% pass
is promising but not a rate comparison, lean qualification or50%
milestone. Captures:
`captures/reverse_direction_2026-09-19_48k_hold45.txt`,
`captures/reverse_direction_2026-09-19_48k_sag_pwm465.txt`.
Known reverse32k lean restored/verified, outputs off/nFAULT high,
UART closed. Next: investigate current-demand onset and protected
control timing rather than infer a hardware ceiling from n=1 A/B.

## 2026-09-19 fast-sag PWM-phase discrimination

One terminal-only TIM1 snapshot was added to the existing eight-scan
fast-sag diagnostic; four interrupt-root arithmetic audits and
disabled hardware guard/role/duty checks passed. The corrected run
ACKed40%, held18s clean, then stopped at the last ACKed44.5% on the
unchanged three-scan fast5% bus guard. The final three normalized bus
codes were92.58/93.03/92.97%; TIM1 phase reconstruction locates
their bus apertures at CNT1570/34/498 versus CCR890 (OFF, early ON,
mid ON). Thus the three-scan event is a real multi-phase-duration bus
drop, not one asynchronous PWM-edge sampling notch. The guard acted
~512us after the first estimated low bus aperture. The accepted-event
tail shows no immediate missed edge or held sector, but does not
identify the transient's source. No 45/50% lean qualification. Exact
known lean image restored/verified, outputs off/nFAULT high/UART
closed. Full capture:
`captures/reverse_direction_2026-09-19_32k_sag_pwm445.txt`.

## 2026-09-19 first-peak timing result

Correct live-transfer arming restored the diagnostic path. A12s low
control accepted live duty and ended by deadline. One reverse32k
protected climb ACKed40%, dwelled9s, then stopped on first phase peak
at last ACKed42.5%: IC=24 raw, normalized bus93.62% on that one scan,
no fast three-scan bus trip/foldback/tracking/nFAULT. TIM1/ADC timing
reconstruction places IC's sequential sample aperture at the PWM
compare edge (nominal CNT853 vs CCR850, with timestamp/launch
uncertainty). The bus aperture three conversions later lands near
CNT929, just after the following PWM compare; its one-scan93.62%
reading may be a switching notch. This frame is consistent with
edge contamination, not proof of
23A continuous current or a floating-phase wiring error. At42.5%,
48k PWM ON-time8.85us is shorter than the configured10.03us ADC sample
window; carrier increase alone does not solve current observability.
Single sample and diagnostic early-stop do not explain or qualify the
real three-scan45% sag; 45/50 lean holds remain undone. Exact lean
image restored/verified/off, UART closed. Capture:
`captures/reverse_direction_2026-09-19_32k_peak_timing_425.txt`.

## 2026-09-19 peak-timing probe staged, no envelope result

First-terminal-peak timing snapshot compiled and passed four ISR-root
arithmetic audits plus disabled guard/role/duty checks. The first
setup run refused before drive for missing `avgnominal`; the second
used the non-live `engage1` path by mistake, then UART went silent.
After an unacknowledged `off`, SWD reset restored shell response and
all outputs read disabled with nFAULT high. Exact reverse32k lean
image restored/verified; no further motor test and no `PEAKTIME` row.
The 43% diagnostic first-peak observation and 40% clean holds remain
the latest usable data; 45/50% are still unqualified.

## 2026-09-19 first-peak stop and 48 kHz carrier A/B

The replacement motor remains clean at40% on protected diagnostic
60s holds, both32k and48k. An opt-in first>=1900-raw phase-current
stop caught one near-rail sample at last ACKed43.0% in each image:
32k IC4087/bus~95.8%, 48k IB4019/bus~98.2%, estimated speeds
1792/1801eHz. Neither event was an exact ADC rail or a three-scan
bus collapse. This intentionally early diagnostic stop is not a
qualified duty ceiling; one run per carrier cannot establish event
rates or a carrier remedy. No new 45/50% lean qualification.

48k (ARR1332,48012Hz) timer/bin/ADC-coverage tests7/7 and four M0
ISR-root arithmetic audits PASS. Its first live images had a valid
1333-tick admission omission in `live_duty::Prepared::new`, now fixed
and host-tested4/4 plus disabled `livedutycheck`24/24. A bulk UART
command separately caused active RX error0x300; ~148ms paced six-byte
commands ACKed, while preserving the250ms partial-command timeout.
The final paced48k image held40%/60s with zero >=1500 phase frames,
then stopped on the first peak at43%; all outputs safed. Exact32k lean
restored/verified, p/i off/nFAULT high, UART closed. Source-grounded
analysis and captures are in `FAST_SAG_COHORT_ANALYSIS.md` and
`captures/reverse_direction_2026-09-19_48k_peakstop43.txt`.

## 2026-09-19 phase-current rate census: near-rail events before 45%, then ADC stop

Frozen diagnostic SHA E62E4D4E2... passed explicit G071 flash/disabled
checks, 10%/15s and 40%/60s protected holds. At ACKed 40% the
50.45s census counted zero >=1500-count phase-current displacement
in 223,220 coherent DMA scans. Bus<95% singles were common (1,418),
but max streak was two and no protection fired. The gradual comparison
ACKed only through 43.5%, then stopped `POWERPATH11` / `DMAFAULT10`
at powered35.259715s; do not call it a 45% bus-sag attempt. Over the
28.00s census since 40% ACK, 123,903 scans included five >=1500,
four >=1900 physical-phase displacements (IA4/IB2/IC0, overlap
possible), one exact ADC rail, with four severe samples coincident
with bus<97%. Fastbus remained quiet. Source maps DMA10 to failed
`Latest::publish`; exact raw rails are rejected, but this image did
not retain the error subtype, so invalid rail is plausible rather
than uniquely proven. Output-off/nFAULT-high readback passed; exact
lean reverse32k SHA06B1C1D6... restored and verified, p/i off, COM41
closed. No 50% qualification. See `FAST_SAG_COHORT_ANALYSIS.md` and
`captures/reverse_direction_2026-09-19_32k_phasecensus.txt`.

## 2026-09-19 DMA-below-COMP with period-bounded lease did not clear lower gate

A protected reverse32k candidate let the lower-priority DMA copy lease
use200us within its226us scan period, retaining flags/NDTR/epoch checks.
It passed host lease tests, M0 ISR-root audit, disabled preflights and
one ordinary10%/15s BEMF hold. The second ordinary startup stopped
before the first15% live ACK. The live host read truncated and discarded
the terminal reason, so this failure is unclassified. It cannot support
an upper-duty timing claim. The candidate was retired without an
upper-rung run and the exact fixed20 lean image restored/verified with
disabled off/safety checks. Capture:
`captures/reverse_direction_2026-09-19_32k_dma_lease200_lower_gate.txt`.
For any next MCP live campaign, retain and emit the full serial read
before testing an ACK; the discarded terminal bytes cannot be recovered.

## 2026-09-19 slower reverse32k ramp still stopped at43%

On the exact protected fixed20 lean image SHA06B1C1D6..., the motor
started normally, ramped to40%, held quiet20s, then ACKed40.5 through
42.5 in0.5% steps with5s quiet at each. It held another25s at42.5.
After ACKed43%, the fast5%/three-scan normalized bus guard stopped
reason26 at powered118.546712s. Current foldback, phase rail, absolute
bus-low, tracking and nFAULT counters were zero; final outputs OFF and
nFAULT high. This refutes the simple idea that the prior quick-ramp
42.5 failure was cured by longer settling. It does not prove that the
43% step itself caused the sag; the lean image has no aligned fault tail.
The earlier single clean42.5 hold and this quiet dwell do not qualify
42.5 in the face of a same-image42.5 fast-sag failure. No43.5-50 command
was issued. Capture:
`captures/reverse_direction_2026-09-19_32k_fixed20_slow43_fastbus.txt`.
Retire slow stepping as a standalone cure; next attempt must change or
measure a specific event/control mechanism with the electrical stops intact.

## 2026-09-19 COMP/DMA peer priority A/B also fails startup

One-variable reverse32k image SHA8F3BD75A... put COMP alongside
TIM6 guard and ADC DMA at priority0, leaving COM64 and all protection
thresholds intact. Disabled preflights passed. Its first ordinary10%
startup stopped at powered2.224ms/4 COMs on the existing TickGap guard
(reason3). Poststop hardware priority readback confirmed COMP0/COM64/
guard0/DMA0. This is a lower-gate failure, not a high-duty improvement.
The candidate was retired after one attempt; the exact reverse32k lean
image was restored, flash-verified, reset and passed disabled safety/
off checks. Capture:
`captures/reverse_direction_2026-09-19_32k_comp_dma_peer_tickgap.txt`.
Together with the earlier DMA-below ADC lease failure, this shows the
present ISR/ADC producer timing cannot simply be reprioritized without
repair. It does NOT establish the45% transient's cause. 50% reverse
remains unreached.

## 2026-09-19 40 kHz carrier A/B: startup works, 45% still sags

Exact reverse40k lean SHA22AEFD4C... passed disabled preflights and a
separate15s protected10% BEMF hold with normal handoff and no electrical
fault. On a second smooth ramp, 45% was ACKed; fast bus sag stopped at
powered39.998459s, roughly3s after the45% command by host wall clock.
No current foldback, phase rail, nFAULT or tracking stop; bus minimum
11.510V and final outputs off/nFAULT high. This falsifies a simple
40k-carrier cure for the45% transient. It does not prove32k is optimal
or establish a hardware limit. Captures:
`captures/reverse_direction_2026-09-19_40k_10_gate.txt` and
`captures/reverse_direction_2026-09-19_40k_45_fastbus.txt`.
The original32k lean SHA06B1C1D6... was restored, flash-verified,
reset, and passed disabled guard/role/duty/off checks. No more carrier
climb is justified by the20/24/32/40k trend alone. The next lever
needs to address comparator-event timing/acceptance or the actual
control/load transient, with a separately verified electrical stop.
50% reverse remains unreached.

## 2026-09-19 lean 32 kHz 40% hold and staged 40 kHz A/B

The restored exact reverse32k lean SHA06B1C1D6... completed a70.000006s
powered window after a live ramp to40%, stopping only on its programmed
deadline. Zero fast sag, current foldback, rail, bus-low, tracking or
nFAULT; bus minimum11.689V, ~1692eHz final estimate,698728 COM;
final gates/ENABLE/MOE/CCRs off and nFAULT high. The40% command was ACKed
early in the run, but the shell did not board-timestamp it, so this is
not an exact70s at40% alone. Capture:
`captures/reverse_direction_2026-09-19_32k_lean40_70s.txt`.

A one-variable reverse40k BEMF carrier image is staged, not yet flashed.
Startup remains10kHz; all electrical stops and226us ADC cadence remain.
The 10% BEMF pulse after nominal deadtime is only2.09us, so a bounded
startup/handoff gate precedes any high-duty inference. Frozen SHA and
tests are in `captures/reference/reverse_40k_20260919/`. This tests the
observed20→24→32k carrier trend; a clean40k result would still need
repeated holds and normal restart before qualification. 50% reverse
remains unreached.

## 2026-09-19 scheduling A/B fails startup; lean32k restored

The one-variable `bench-dma-below-comp` candidate retained all electrical
stops and passed disabled checks, but its first ordinary10% startup ended
at powered4.453ms on ADC reason11 / DMA lease-finish code7: 237us between
DMA services against226us scan cadence, only7 commutations. It cannot
compare high-duty behavior and was not repeated. This does not prove that
normal-priority DMA causes the44.5% tracking/load disturbance; it proves
the proposed priority arrangement is not viable with this ADC producer
as built. Exact prior reverse32k lean SHA06B1C1D6... was reinstalled,
flash-verified, reset, and passed disabled guard/role/duty and p/i off/
nFAULT1 checks. Capture:
`captures/reverse_direction_2026-09-19_32k_dma_below_startup_fail.txt`.
The high-duty root cause remains open; 50% reverse still unreached.

## 2026-09-19 reverse 32 kHz aligned event/bus stop at 44.5%

After the operator resumed, diagnostic SHA AAE4E8FE... added a bounded
IT87 accepted-event suffix and BS85 last16 coherent ADC frames to the
existing32k fault-frame image. Four ISR-root arithmetic and disabled
preflights passed. A normal-start ramp ACKed44.5% but fast bus sag reason26
stopped at powered36.774451s before45% was accepted. No foldback, rail,
nFAULT or tracking stop; final outputs off/nFAULT high. The trip came522us
after the first *sampled* sub95% bus value and70us after the third.

CRC-valid tails show the last21 complete cycles lengthen545→612us,
with no event-watch violation. Software-only accepted comparator events
become more frequent; one occurs36.773641s, then a phase-current raw
IA34 at36.773703s while bus is97.46% of baseline. Another software-only
accept occurs36.773916s; first sampled bus below95% is36.773929s
(93.50%), with phase current raw3092/959/3360. The following two bus
scans are93.38/94.66%. This ordering supports a timing/load disturbance
before the recorded bus collapse but does not prove revisit caused it:
226us ADC spacing can miss an earlier dip. The instrument itself adds ISR
cost, so this is not a lean envelope result. Capture:
`captures/reverse_direction_2026-09-19_32k_it87_bs85_445_fastbus.txt`.
Next A/B: keep the32k lean control and protections, move only ADC DMA IRQ
below COMP/COM while TIM6 guard stays highest, then compare a bounded
reversed35→45 ramp. Current DMA priority0 can delay COMP/COM service;
the captured chronology makes this a testable scheduling hypothesis, not
an established root cause. 50% reverse remains unreached.

## 2026-09-19 operator pause after diagnostic replay

The operator paused bench work for several hours. No further powered work
is authorized until they resume. The32k fault-frame diagnostic image
(SHA9C236575...) was flashed with the same control and electrical stops
as the lean32k A/B, plus a fault-only raw ADC snapshot. Disabled guard,
role, duty and off preflights passed. Its one normal-start ramp to45%
stopped at powered35.625135s on the existing fast >5%/three-scan bus
guard (reason26); bus minimum11.498V, current foldback0, phase rails0,
tracking0, nFAULT high. Because bus sag stopped first, no ADC fault frame
was emitted and the prior45% rail remains unexplained. Final outputs
off, nFAULT high, COM41 closed. The image is diagnostic, not a qualified
hold. Capture:
`captures/reverse_direction_2026-09-19_32k_faultframe_45_fastbus.txt`.
Desired reversed airflow was visually confirmed. 50% reverse remains
unreached. Keep the pause and protections; do not convert either45% stop
into a pass.

## 2026-09-19 reverse 32 kHz carrier A/B: desired airflow, 45% ADC stop

The operator watched and confirmed the reversed airflow is the direction
wanted. Exact lean SHA06B1C1D6... at32kHz (startup still10kHz) passed
disabled preflights and normal BEMF entry. A protected60s powered run
reached37.5% and ended at its timer without electrical faults; this was
not a full-duration37.5%-only hold. A120s attempt then crossed40% and
42.5%, reached45%, and stopped at powered87.389s on reason11 / ADCFAULT
stage30. `CURRENTQUALITY phase_rail_codes=1` points to one invalid raw
phase-current frame, but this lean build did not retain the five raw ADC
counts. `Latest::publish` treats any0/4095 raw value as Invalid and calls
`stream_fault(10)`. There was no fast-sag, current-foldback, nFAULT, or
tracking stop. Bus minimum11.605V. Final gates/ENABLE/TIM1 outputs off,
nFAULT high. The result establishes that32kHz crossed the earlier40% wall
without a reported bus-collapse stop; it does not qualify45% or prove the
root cause of the isolated ADC rail. 50% remains unreached in reverse.
Capture: `captures/reverse_direction_2026-09-19_32k_45_adcfault.txt`.
Next: retain the same electrical protections and record the exact raw fault
frame in a bounded diagnostic image before interpreting the45% event.

## 2026-09-18 aligned reverse IT87/BS85 result and restoration

The diagnostic SHA BBCC4F56... preserved every electrical stop and packed
dispatch-origin bits into the existing interval-tail deadline field. One
ordinary-start, guarded ramp ACKed40%; at powered33.272317s it stopped on
ADC/DMA publication reason11 (phase raw IB4095/IC11), not fast-sag reason26.
Source audit resolves the label: `Latest::publish` rejects exact rail4095
as Invalid after the current/bus scan and calls `stream_fault(10)`; this is
a fail-closed raw-frame-validity stop, not evidence of broken DMA transfer.
`CURRENTQUALITY phase_rail_codes=1` independently corroborates the rail.
The fast-sag three-scan sequence did not complete because a95.20% sample
interrupted the sub95% scans. The bus tail otherwise trends from ~98-99% to
95.65% at33.271099s and then as low as92.78%. No current foldback, nFAULT
or tracking stop; gates/ENABLE/PWM off after stop.

CRC-valid IT87 aligns128 accepted events with16 bus frames. A cluster of
software-only accepted comparator dispatches begins33.270690s, ~409us
before the first *sampled* bus decline; their gaps and smoothed interval
lengthen. This is evidence of a tracking/event disturbance antecedent to
the sampled load surge, not proof that the comparator path caused it: a
shorter earlier voltage dip could fall between226us scans. Final40 epoch
origin counts physical7231/software291/both192/neither0; both remains
ambiguous. Capture:
`captures/reverse_direction_2026-09-18_it87_40_adcfault.txt`.
The exact reverse lean SHA2B4EA006... was reflashed/verified and passed
disabled guard3/18, role6/6, duty6/6 and final p/i outputs0/nFAULT1;
COM41 closed. The aligned diagnostic is not lean qualification, 40/50
remain unqualified, and no old-direction run occurred.

## 2026-09-18 reverse dispatch-origin test, 35% clean / 40% fast sag

The first origin diagnostic sampled EXTI too late: minz-core had already
cleared it before EV_ACC. Its 35% `neither` majority is invalid. The corrected
dispatch-entry snapshot (frozen SHA698758C5...) produced zero `neither` in
two bounded powered attempts. At the final 35% duty epoch, 156677 accepts
were physical-only, 1705 software-only, 2316 both; host stopped the clean
run at32.371s powered, with no fast sag, current foldback, rail or bus-low
count. Software-only is a lower bound on real level rescue. Both is ambiguous
because a rejected synthetic dispatch can leave its flag set until the next
real edge.

The same image ramped smoothly to ACKed40%; fast bus sag stopped it at
39.487s powered, reason26. The final40 epoch counts were 57342 physical-only,
1985 software-only, 1445 both, zero neither; late intervals (>prior smoothed
average+25%) were 4733/185/243 respectively. These aggregate counts do not
establish which source or timing event preceded the three low bus scans.
There was no current foldback or nFAULT; final outputs were off. Final speed
estimate was1536eHz at the fault versus1579eHz at clean35 stop; the two are
not matched stable epochs. Retained captures:
`captures/reverse_direction_2026-09-18_dispatch35_valid.txt` and
`captures/reverse_direction_2026-09-18_dispatch40_fastbus.txt`.
Do not qualify40 or climb above it from this result. The next causal
instrument must align event origin and interval chronology to the bus-trip
window with bounded ISR cost, or test a source-grounded control change.

## 2026-09-18 correction: revisit accepts are not event-origin proof

Source audit found that `LEVEL_REVISIT_INFLIGHT` is set at a foreground pend
and cleared only at the next accepted event. If the pended ISR rejects during
persistence, a later *physical* EXTI crossing may accept while the flag is
still set and increment the revisit-accept counter. Therefore the many
attempts=accepts reports prove eventual acceptance per attempted sector, not
that each synthetic dispatch itself rescued the crossing. The 35% cutoff A/B
still shows the mechanism matters to stability, but its direct rescue rate is
unmeasured. The next diagnostic must classify the dispatch that actually
accepted (synthetic-only / physical-only / coalesced) with a tightly costed
instrument; do not infer cause of the40% bus collapse from the old counter.

## 2026-09-18 lean normal-start recovery at 10% and 25%

On the exact corrected reverse lean SHA2B4EA006..., one deliberate Tracking8
injection per point was followed by a disabled1s settle, fresh ordinary
startup/BEMF handoff, restoration of the previous live request, and a full
remaining powered deadline. The strict compact verifier and final p/i
off/nFAULT1 pass both captures:

| Point | Injected first stop | Restored duty | Second powered window / plan | Result |
|---|---:|---:|---:|---|
| 10% | 5.000010 s | 10%, 0 steps | 14.236014 / 14.236473 s | normal deadline; 459 us rounding |
| 25% | 10.000053 s | 25%, 3 guarded steps | 29.236004 / 29.236325 s | normal deadline; 321 us rounding |

Both have unique LEANCORE marker, transfer1, FASTBUS0, foldback0, rail0,
bus-low0, no nFAULT or second tracking stop. Captures:
`captures/reverse_direction_2026-09-18_lean10_restart.txt` and
`captures/reverse_direction_2026-09-18_lean25_restart.txt`. Paired with the
two separate ordinary-start holds at each point, this is the current lean
10/25 qualification evidence. It does not settle physical shaft-direction
observation or the40% fault; 50% remains unqualified.

## 2026-09-18 corrected lean reverse 10/25 hold cohort

Exact corrected SHA2B4EA006... passed explicit G071 flash verify and disabled
guard/role/duty/off checks. Four **separate** ordinary-start attempts completed
their full powered deadlines, with transfer1, the unique `LEANCORE` witness,
FASTBUS0, current foldback0, rail0, bus-low0, tracking event_fault0,
no nFAULT stop, and p/i outputs off/nFAULT1 after each:

| Request | Powered window | COM | Final speed estimate | Capture |
|---|---:|---:|---:|---|
| 10% #1 | 20.000004 s | 63260 | 529 eHz | `captures/reverse_direction_2026-09-18_lean10_hold1.txt` |
| 10% #2 | 30.000015 s | 95016 | 529 eHz | `captures/reverse_direction_2026-09-18_lean10_hold2.txt` |
| 25% #1 | 35.000004 s | 244376 | 1212 eHz | `captures/reverse_direction_2026-09-18_lean25_hold1.txt` |
| 25% #2 | 30.000010 s | 207780 | 1230 eHz | `captures/reverse_direction_2026-09-18_lean25_hold2.txt` |

The 25% live ACK occurred early in each powered window, but compact firmware
does not timestamp the duty epoch, so these are total powered durations, not
exact25%-only dwell times. Strict captured off/marker/stop checks4/4 PASS.
Normal-start recovery on this exact lean SHA is still to be run. The 40%
control transient and50% qualification remain open.

## 2026-09-18 provisional lean 10% and corrected marker

The first lean reverse normal-restart image SHA9EF8DF7A... completed an
ordinary-start 10%/30.000005s powered hold, reason2, 94983 COM, ~532eHz
estimate, no fast sag/current foldback/rail/bus-low/tracking/nFAULT stop,
final p/i outputs off/nFAULT1. Capture:
`captures/reverse_direction_2026-09-18_lean10_provisional.txt`. Compact mode
hid the `LEANCORE` build witness, so this is provisional electrical evidence,
not a formal exact-image qualification. A post-stop-only marker fixes that;
corrected SHA2B4EA006... is frozen but not flashed. Repeat formal 10/25
holds and recovery on the corrected image.

## 2026-09-18 staged reverse lean qualification image

Normal-start recovery now fits in a lean reverse image SHA9EF8DF7A... at
`captures/reference/reverse_lean_restart_20260918/`. It omits the fault-frame
and seed-stage postrun diagnostics but retains active current limiting, fast
relative and absolute bus stops, nFAULT/tracking/watchdog, and the comparator
level-revisit path that the off35 A/B showed was needed. Four ISR-root math
audit and policy/compact-report host tests pass. It is **not flashed or
motor-qualified** yet. Next use is repeated separate 10/25% ordinary-start
holds and normal-start recovery, with the lean marker and final off-state
verified for each. Prior two mixed-point lean holds and one hybrid recovery
per point are supporting evidence, not this exact image's qualification.

## 2026-09-18 safe handoff after revisit A/B

The negative off35 candidate was removed. Exact archived reverse control
SHA62956855... was re-flashed/verified on G071, reset, and passed disabled
guard3/18, reverse role6/6, duty6/6, p/i gates/ENABLE/MOE/CCRs0/nFAULT1.
No powered run on this restored SHA after reflash. The 35% Tracking8 A/B
failure remains retained; 40% and50% remain unqualified.

## 2026-09-18 reverse revisit cutoff A/B result

The reverse-only A/B disabling foreground level revisit at ACKed35% failed
its lower gate. Tracking8 stopped at10.295059s powered; the built-in normal
startup recovered and restored35%, then a second Tracking8 stopped at
10.112055s powered. Fast bus sag, current foldback, phase rail, absolute
bus-low and nFAULT stops did not fire; final p/i confirmed off/nFAULT1.
The `REVISITPOLICY cutoff_tenths=350` witness and1424/1424 total earlier
revisit attempts/accepts were retained. This is evidence that physical COMP
edges alone remain unreliable at35% in this configuration. Retire this
candidate; do not climb it to37.5/40 or label the40% sag as caused by
revisits. Capture `captures/reverse_direction_2026-09-18_off35_tracking.txt`.

## 2026-09-18 staged high-speed revisit A/B

The reverse40 event remains unexplained. One bounded control candidate now
keeps the real-level revisit rescue below35% and disables only its foreground
pend at35% and above, leaving physical COMP accepts and all electrical stops.
This tests whether the high-speed hybrid rescue is a contributor without
breaking the lower-speed entry path. Pure cutoff test and four ISR-root M0
math audit pass. SHA9D46A9B8... is frozen at
`captures/reference/reverse_revisit_off35_20260918/`, **not flashed**.
Test35 then37.5 before considering40; any fault is retained, not repeated to
pass. Current installed SHA6295 reverse control remains off.

## 2026-09-18 control image restored

After the observer-affected CPU35 fast-sag stop, the CPU build was retired.
The BF06 reverse control feature closure was rebuilt as SHA 62956855... and
frozen at `captures/reference/reverse_control_20260918/`. It passed four
motor ISR-root arithmetic audit, explicit-G071 flash verification/reset,
disabled guard3/18, reverse role6/6, duty6/6, and p/i outputs off/nFAULT1.
The cfg-gated CPU-only source edit changed ELF SHA but not image size; do not
claim byte identity or a new powered qualification for this restored hash.
Board is OFF on reverse-only guarded firmware, not the intrusive CPU image.

## 2026-09-18 reverse 35% CPU observer fault

The corrected CPU image ACKed35% after a live10->35 ramp but the fast bus
guard stopped reason26 at10.973529s powered, about1.3s after the35 command.
No current foldback, rail, bus-low, nFAULT or tracking stop. CRC-valid CPUUNION
was66.21% over the *whole ramp* (COMP30.04%, COM19.88%, DMA13.59%, guard
2.70%; accounting fault0). It is not a35%-steady rate. The non-CPU reverse
hybrid held35 and38.8 earlier, and an older CPU diagnostic also sagged at35;
therefore this is evidence of a material observer effect, not a lean35 ceiling.
Fast shutdown and postrun p/i off/nFAULT1 verified. Retained capture:
`captures/reverse_direction_2026-09-18_cpu35_fastbus.txt`. No more blind35/40
repeats on this image; use a lighter instrument or return to the lean build.

## 2026-09-18 reverse 30% aggregate CPU result

Corrected diagnostic SHA 3EAE27DF... passed explicit-G071 flash verify and
disabled cpucheck/guard/role/duty checks. A live reversed 10->30% ramp ended
by deliberate host stop at22.974335s powered; no fast sag, current foldback,
phase rail, bus-low, nFAULT or tracking fault. The CRC-valid CPU meter covered
22.974235s: IRQ union15.978068s (69.55% observer-affected), partitioned as
guard0.566685s (2.47%), COMP6.921644s (30.13%), COM5.347643s (23.28%),
DMA3.142096s (13.68%); CPU fault0, six roots sum exactly to union. The
remaining6.996167s is foreground/unattributed, not necessarily idle. Disabled
software-pair costs2/7/10us are not a live observer correction. This result
cannot be transferred to lean utilization or used to qualify30%.
Capture `captures/reverse_direction_2026-09-18_cpu30_fixed.txt`;
`drv_cpu_meter` decoder and seven tests passed. A matched higher-duty CPU
point would show trend, but the 40% sag remains a separate control fault.

## 2026-09-18 reverse CPU instrument correction

The first aggregate-CPU image was flashed after disabled cpucheck/guard/role/
duty checks passed and reverse-ramped to30%. It ran to ~22.97s powered with
zero fast-sag/current-foldback/nFAULT/tracking/rail stop; reason9 was the
deliberate host stop. No CPUUNION appeared: compact mode suppressed the entire
capture branch, including the intended small aggregate rows. This run is
electrical exploratory evidence, **not a CPU measurement**. The compact
terminal path now prints CPUUNION/CPUROOT after shutdown, and a corrected
image SHA 3EAE27DF... is frozen but not flashed. Full hashes/features in
`captures/reference/reverse_cpu_20260918/`. Qualification still requires
separate lean-image runs.

## 2026-09-18 offline reverse40 timing/CPU follow-up

The CRC-valid aligned reverse40 IT86/BS85 tail records one165us accepted
gap at23.662141s, with the next 226us bus sample still98.54% of its
same-wake reference. The bus then declined to three consecutive94.61/94.57/
93.04% scans at23.663826/.664052/.664278s, triggering the existing stop.
This makes timing/load disturbance a useful hypothesis, not a proven cause:
the scan cadence could miss an earlier narrow dip. A separately built
CPU-accounting reverse image is frozen at
`captures/reference/reverse_cpu_20260918/` (SHA DF453D20..., four ISR-root
math audit PASS). It is *not flashed* and any resulting run would be
observer-affected. Current installed board stays BF06/OFF. Next diagnostic
must first pass disabled preflights and use a known-clean reversed duty point;
the prior 40% failures remain retained, and 40/50 are unqualified.

## 2026-09-18 reverse 40% hybrid attempts

The first guarded hybrid40% run stopped on ADC/DMA publication reason11 at
28.828783s, with one phase-rail code and no fast-bus trip. A follow-up image
added a post-safing coherent-frame snapshot for that error; its guarded
10->15->20->25->30->35->37.5->40% run ACKed40%, then the fast 5%/three-scan
bus guard stopped reason26 at22.078298s (baseline raw1210/vref1506, no
current foldback, nFAULT, tracking or rail stop). `ADCFRAME` was absent as
expected on the bus stop. Both runs ended with gates/ENABLE/MOE/CCRs0,
nFAULT1; the later board readback independently confirmed this. Captures:
`captures/reverse_direction_2026-09-18_hybrid_40_adc_fault.txt` and
`captures/reverse_direction_2026-09-18_hybrid_40_fastbus_repeat.txt`.
Installed diagnostic SHA BF06A19BFD483CA92B8042073EA4D74EA4E72AAC36DDB333C83ED9C05A1B352A.
The earlier 38.8% hold stands; 40% remains unqualified. Do not infer a
hardware limit or retry blindly: characterize the control/current/bus event.
All powered images remain reverse-only, and physical shaft direction still
needs an independent visual witness. No change to sibling rm32.

## 2026-09-18 reverse 38.8% upper map

Same guarded hybrid image held35% ~15s,37.5% ~15s and38.8% ~15s in one
ordinary-start live ramp, stopped deliberately at67.831s energized.
No fast sag, current foldback, bus-low, rail, tracking or nFAULT fault;
~1718eHz and582548 COM at the end. Postrun outputs off/nFAULT1. Capture:
`captures/reverse_direction_2026-09-18_hybrid_388_hold.txt`. This is one
diagnostic hold; three earlier40% sharp-sag stops on a different24k image
remain contradictory evidence, not a reason to label40% qualified.

## 2026-09-18 reverse 30% recovery extension

The same guarded hybrid image ACKed30% before an injected Tracking8 at
10.000129s, safed/settled, made a fresh ordinary startup and BEMF handoff,
restored30 in four foreground5-point steps, and ended normally at29.236008s
second powered time versus29.236576s planned (568us rounding). No fast sag,
foldback, bus-low, rail, second tracking or nFAULT fault; ~1400eHz final.
`captures/reverse_direction_2026-09-18_hybrid_restart30_deadline_pass.txt`
passes the compact deadline verifier. One recovered run, not a cohort.

## 2026-09-18 reverse normal-start recovery

Installed/OFF image SHA
820F901CD2538F5BD040E0989A8C050ADDFED0450D8EFD420B5EA49D4732939E
uses binz opt-s/dependency opt-z, thinLTO/codegen1, and omits only post-stop
bulk capture output to fit automatic restart in128k flash. Electrical stops
are unchanged. Disabled guard/role/duty preflights and four ISR-root math audit
passed. The first10% injected Tracking8 produced a disabled1s settle, fresh
ordinary startup/handoff and second powered deadline14,236,005us vs planned
14,236,408us. The25% case ramped to25 before the injected stop, restarted,
restored25 in three bounded foreground steps, and ended at29,236,004us vs
planned29,236,358us. No fast sag, foldback or nFAULT in either recovered run;
final outputs off/nFAULT1. Captures and verifier:
`captures/reverse_direction_2026-09-18_hybrid_restart10_deadline_pass.txt`,
`...restart25_deadline_pass.txt`, `scripts/live_armed_baseline.py` compact
parser. One recovered run per point; repeatability not yet proven. Historical
opt-z measured-seed refusal, legacy replay-duty cap and missing restart ADC
stream failures are retained in adjacent captures. The next envelope question
remains the genuine40% sharp-sag onset on the replacement motor, not whether
the old-direction50% result transfers (it does not).

## 2026-09-18 lean reverse 30% hold

Current installed/OFF image is the same lean 24-kHz SHA
3BE434DD8B79EBDB04CA78F8CBAAC5FDCF31DD7370BB2863924046C68CB8EA9B.
A third independent ordinary start live-ramped10->30% and held30% ~30 s,
deliberate host stop at46.878 s energized, ~1394 eHz, 325126 COM,
10079/10079 real level revisits. No fast sag, foldback, bus-low, phase rail,
tracking or nFAULT fault; p/i outputs off, nFAULT1 after. Capture:
`captures/reverse_direction_2026-09-18_lean_30_hold.txt`. This is one lean30
hold, not repeated30 qualification. The remaining higher-duty problem is the
40% sharp-sag stop, not demonstrated loss of lock at30%.

## 2026-09-18 lean reverse hold 2

The same installed reverse 24-kHz lean image made a second independent
ordinary startup/handoff, held10% ~12 s and25% ~30 s, then stopped on host
command at55.595 s energized. No fast sag, foldback, bus-low, phase-rail,
tracking or nFAULT fault; ~1212 eHz at25%, gates/ENABLE/MOE/CCRs off and
nFAULT1 afterward. Capture:
`captures/reverse_direction_2026-09-18_lean_10_25_hold2.txt`. This and hold1
establish two clean lean low/mid-range holds, not restart/recovery or40/50%
qualification. Physical visible shaft-direction confirmation still pending.

## 2026-09-18 lean reverse hold 1

The installed reverse 24-kHz lean SHA is
3BE434DD8B79EBDB04CA78F8CBAAC5FDCF31DD7370BB2863924046C68CB8EA9B.
After guard/role/duty disabled preflights, ordinary startup and BEMF handoff
succeeded; 10% held ~11 s, then25% held ~25 s, stopped deliberately at
49.617 s total. No fast sag, current foldback, bus-low, phase rail or tracking
fault; ~1225 eHz at25%, gates/ENABLE/MOE/CCRs off and nFAULT1 afterward.
This is one lean hold, not repeated qualification or recovery. Capture:
`captures/reverse_direction_2026-09-18_lean_10_25_hold1.txt`. An earlier lean
candidate accidentally re-enabled the old single-cycle floor by omitting
`bench-fast-cycle-report` and stopped at2222us versus2223us; it is retired,
capture retained. The corrected lean image keeps CycleTiming report-only and
all electrical stops. Adding automatic normal restart to it exceeds flash by
1408 bytes; no restart qualification claim yet.

## 2026-09-18 reverse midpoint update

Current installed reverse 24-kHz diagnostic image SHA
9948D82825C4F95F404389461A34885BCBFE1AD0914E37E832A8FDA19FCCD4A is
OFF, with postrun p/i outputs0 and nFAULT1. One live-ramp run held 35% for
~20 s, then 37.5% for ~12 s before a deliberate host stop; no fast sag,
foldback, tracking, rail, or nFAULT stop. IT86/BS85 captures decode correctly
(`captures/reverse_direction_2026-09-18_375pct_hold_24k.txt`). This places
the observed sag onset above 37.5% in current evidence, since 40% at24k has
stopped three times on the fast 5%/3-scan sag guard. It is diagnostic evidence,
not lean-image qualification. The 20k one-variable A/B lost tracking at35%,
so 24k remains the better-supported carrier. Keep all electrical guards and
do not flash old-direction firmware.

## 2026-09-18 current reverse status (supersedes older status below)

The replacement motor is driven with physical A/B swapped throughout the
control and sense path. Electrical reverse mapping is checked; operator-visible
shaft direction has not yet been confirmed. Reverse BEMF held 10–25% for 10 s
each. At 24 kHz with bounded real-comparator level revisit, two 30% commands
held ~31 s without electrical stops (~1.39 keHz), whereas disabling revisit
repeatedly lost one event at 30% after 8.75–11.85 s. A 35% command held 10 s.
Three 40% attempts stopped on coherent >5% bus sag (three 226-us scans), not
nFAULT or tracking. Aligned IT86/BS85 capture shows late electrical cycles and
large phase-current samples near the short sag; causal order remains under
investigation. Switching only the carrier to 20 kHz was worse: Tracking8 at
35%, before 40%, without fast bus sag. The current installed reverse 20-kHz
diagnostic image SHA is A1C950E911C93996E63A0DB96989F611F82C05418F8A65CF2E348294F47529D5,
OFF after the stop. Restore 24 kHz for the next test. Keep all electrical
guards, especially fast bus sag; do not use the old-direction image. Reverse
50% remains unqualified. Detailed captures are in `captures/reverse_direction_*`.

## 2026-09-18 reverse-direction reset of qualification

The operator replaced the overheated motor, then required the opposite shaft
direction. The earlier old-direction envelope below is historical only and
does not transfer to the replacement motor. A/B physical phase relabeling is
installed; disabled checks confirm six role/mux mappings, but shaft direction
still needs a human observation. The current reverse diagnostic image SHA
ACC923F833896940355BFFB4DDB58097E94D20994BB11431F817A1248CA209D9
is installed and OFF. It has not yet held BEMF lock: at 10% acquisition duty,
20kHz carrier and a 280us low-speed comparator blank, it produced five COMs
then 25 genuine COMP events in a 1ms bucket and safed on reason13 after
2.896ms powered. Fast bus-sag, current foldback and nFAULT did not lead the
stop. The prior 6.1% attempt also stormed, so ON-time alone is not the cause.
Removing only the 280us blank (same20kHz/10%/cap24) worsened the burst:
two COMs and reason13 at0.592ms powered, versus five COMs/2.896ms blanked.
Normal-restart, running-level-revisit and event100 diagnostics were omitted to
fit this startup-ADC experiment in 128kB; no electrical stops were removed.
No reverse duty point is qualified. Next: resolve comparator sequence/
qualification mechanism offline before another bounded powered attempt. The
old-direction firmware must not be run.

Current objective: map and qualify binz from5% through50% without touching the
sibling rm32 implementations. The last configured PSU limit was4.5A. E872
reached50% but stopped after a complete50-scan block averaged below the bus
threshold. Immediately afterward,
the operator powered off and reported that something burnt (E873). All powered
work and auxiliary connections are prohibited until power-off inspection
identifies the damaged element/path and a safe re-energization plan exists.
50% remains unqualified.

## Current map (2026-09-16)

| Requested duty | Result | Electrical / protection evidence |
|---|---|---|
| 30% | 20s pass | E791,20kHz,no foldback,bus min11.760V,~1.17keHz |
| 32.5% | coarse-ramp foldback | E796/E797 reached target then folded32.5->27.5; E797 max residual24489/10801 |
| 35% | diagnostic2/2 x60s | E868/E869 level-revisit,~1.55keHz,zero faults; baseline E866 dropped events twice |
| 40% | diagnostic60s pass | E870,~1.72keHz,zero faults,13864 bounded level rescues |
| 45% | diagnostic60s pass | E871,~1.9keHz,zero faults,17632 bounded level rescues |
| 50% | reached, not qualified | E872 ACKed/held ~23s, 50-scan bus-average stop reason25; E873 physical burn report |

## E873: physical burn report supersedes the supply-fold pause

After E872 had safed and emitted its post-run report, the operator powered the
bench off and reported "something burnt." The exact source is unknown. The
last firmware evidence remains useful but cannot certify the hardware:

- E872 stopped when a complete50-scan block averaged below the bus-ratio
  threshold (`reason=25`). Raw bus/vref/vcal863/1508/1662 is the final trip
  sample;110 low-bus codes is the cumulative run count, not the block average.
- No Tracking8 or nFAULT led the stop. The accepted-event tail remained within
  its dynamic watch bound.
- Commanded50% speed had fallen to roughly1.52keHz versus roughly1.9keHz at45%,
  consistent with loss of electrical authority before shutdown.
- The fixture subsequently read ENABLE, MOE, CCRs and gate commands off and
  nFAULT high. That is a historical firmware readback only; it neither locates
  the burnt component nor proves the present board safe.
- The capture prints `physical_psu_limit_ma=4000`, while the operator's latest
  physical setting was4.5A. Treat that field as stale build metadata, not a
  measurement of the PSU setting.

No further serial, flashing or powered diagnosis is allowed. Preserve the
physical layout and inspect only after cooldown and isolation. The restart gate
is recorded in HARDWARE_INCIDENT_2026-09-16.md.

Offline E873 decoding adds one independent observation. The sparse drive ring
contains startup ticks19..4674ms only, so its11.50..11.95V range cannot be used
to contradict or characterize the terminal collapse. The2kHz coast recorder
starts0.506ms after bridge disable: bus voltage was8.796V, then9.628V at1.010ms,
10.645V at1.514ms,11.371V at2.018ms,11.801V at2.522ms and12.023V at3.025ms.
The bus therefore really was depressed at shutdown and recovered within a few
milliseconds when drive was removed. That narrows the event to a loaded power-
path collapse but still does not identify its physical source.

## E862-E872: the old wall was lost edge delivery; 50% exposes bus power

The retained E849 aggregate image had already run, but folded35->25 and could
not answer the high-speed CPU question. E862 combined fixed advance20,
report-only current and speed-watch with aggregate CPU accounting. Its ramp
stopped Tracking8 at31%; observer-affected IRQ union was53.98% (guard3.58%,
COMP24.04%, COM15.43%, DMA10.94%). CPU remains expensive, but saturation did
not cause this fault.

Exact E861 traces separated good from bad. E865 held31% for60s with95..143us
event gaps and zero watchdog violations. E866 at35% dropped tracking twice:
the final suffix ran normally at95..136us, accepted one late332us event against
a311us three-period deadline, then received no next edge. Current, bus, nFAULT
and fast-cycle guards were clear. The causal defect is intermittent comparator
event delivery, not a rising persistence burden or supply current.

E867 tests a bounded level-sensitive repair. After the legitimate half-
interval gate, foreground may request one ordinary comparator service for the
current command only if EXTI is not pending and the real comparator is already
at the expected post-ZC level. The normal ISR persistence reads and real timer
timestamp remain mandatory; no commutation, timestamp or safety deadline is
synthesized. Pure policy tests4, existing host tests12 and M0 audits pass.
E868/E86935%, E87040% and E87145% each held60s without restart or electrical
fault. Every revisit became a normal acceptance; activity rose from~2.4% to
~3.0% of events. This actively clears the old35-45% wall on a diagnostic hybrid
edge+level image, but does not yet qualify a lean production image.

E872 reached and acknowledged50%, then stopped reason25 after a complete
50-scan block averaged below the bus-ratio threshold at powered51.022318s.
Cause5 final raw bus/vref/vcal was863/1508/1662;
current max residual18812 against17308 nominal allowance. No Tracking8 or
nFAULT led. Final cycles slowed to646..665us (~1.52keHz) from45%'s515..565us,
while the watchdog remained internally consistent. Under the operator's hard-
fold rule, further powered work paused here. E873's burn report escalates this
to a potential-damage stop; 50% reached is not50% qualified.

E796 is a diagnostic image, not a qualification image. Its35% nFAULT boundary
had a final11.3ms residual23445 counts against allowance10831. This is nominal,
not calibrated amperes. The H-device nFAULT is aggregate. The EVM schematic
leaves VDS Hi-Z through DNP R17; the cached DRV8304 datasheet maps Hi-Z to0.6V
VDS OCP with4.5us deglitch and4ms retry. That makes VDS OCP the leading cause,
not a uniquely decoded fact.

## Measurement correction

The24kHz209us ADC cadence is retired: its50 samples do not distribute uniformly
over the carrier. The226us cadence was selected exhaustively; all50 phases are
unique and the largest circular gap is60/2666 ticks. Host and Rust tests enforce
the accepted cadence and geometry. This correction did not remove the35% wall,
so sampling bias was contributory but not the root boundary.

## E797 correction: acceleration, not a35% steady wall

The old fixture changed duty by5% every0.5s. A binz-only advance18 image still
nFAULTed at35% with that ramp, and a coarse32.5% final step produced a24489
raw maximum current block. Changing only the host ramp to1% every0.25s let35%
complete30s with its maximum current block at7303, no nFAULT, foldback, phase
rails, low-bus samples, fast cycles or tracking fault. The live AM32 foreground
filter map was already active (postrun filter5 at35%); the old printed12-read
value belonged to acquisition. Advance18 remains bundled, so the data does not
attribute the whole win to ramp shape.

At40%, one block was only2.2% over the configured nominal2.5A allowance and
correctly folded to35. No sustained/hard PSU-fold signature occurred. The next
powered step requires the operator's manual PSU change to3.5A and a matching
software guard; do not silently raise only one side.

E798 prepared that matching guard but was deliberately not flashed. Frozen SHA
`5C2AC6F5D9589F14A8E4716A212A6A5662F88509E84B881E2EC3855AC84638D8` is a
release-s/thin-LTO/codegen1 build with explicit nominal3.5A foldback and a clean
four-root M0 arithmetic audit. It was superseded before installation.

E799 superseded staged E798 with a monotonic current ceiling. One
first-over block lowers duty by1%; a successfully published lower ceiling
rearms the warning so later blocks may reduce again. A second complete
over-limit block arriving before acknowledgment still hard-stops and remains
latched. This preserves fail-closed timing while avoiding the old40→35
overreaction. E799 was installed for E803 and remains the current safely-off
board image. E803 then falsified the fixed1% actuator for severe excursions;
E804-E806 supersede it for future work. Its frozen SHA is
`4B151FC2660693F68BB5E047E61FCDB6FC1E412C4CBD878E65E411E9F7056A02`.

E800 was the corresponding fixed1% lean qualification image. It is superseded
by adaptive E805 and must not be used for future qualification. Frozen SHA is
`3E6BD0C7217F95BFC4EBD1947DC7A33E6EAEDD142CAB66342B56A56A504C2CD9`.

E801 was the separate fixed1% aggregate CPU image. It is superseded by adaptive
E806 and must not be used for future measurements. Its entry/exit clock reads
perturb the result, so disabled `cpucheck` cost would accompany every powered
result and its envelope could never transfer to E800. Frozen SHA is
`CA33BD31B860DA7EF65A9E37F100C9B7F3341D10B5AD462BCCCBEF5B117A40C3`.

## E803: 4 A supply headroom exposed a power-path collapse before 40%

The operator set the PSU current limit to4.0A, while E799 retained its3.5A
signed-average firmware ceiling. Flash/reset and disabled guard3/18 plus all
six2666-tick role checks passed. During the smooth1%/0.25s ramp, live status
ACKs reached38%. One average-current warning correctly reduced38->37. Before
the next39% command could be accepted, the independent bus-sag guard stopped
the run at10.793412s powered.

This was not a single switching notch: the guard compares a complete50-scan
block (about11.3ms at226us cadence) with the8.4V floor. Its cause5 snapshot was
bus656/vref1506/vcal1662, approximately6.3V instantaneous-equivalent, and it
counted41 low bus samples. The separate retained `POWERFEEDBACK` stream printed
11.677V minimum, but source audit shows that is only the handoff seed:
`bench-lean-irq` deliberately compiles out ongoing `FEEDBACK_N`/`BUS_MIN`
statistics. The bus/current guard still consumes every coherent DMA frame
before that omitted bookkeeping, so the seeded summary cannot negate the
guard's complete low block. Tracking remained current, nFAULT stayed high, and
fast-cycle/event counters were zero. Final safing passed.

The maximum signed-current residual was28065 against the3.5A nominal allowance
15145:1.853x, or about6.49A nominal-equivalent. That is not a calibrated amp
claim (`AVGNOMINAL` explicitly says uncertainty unbounded), but together with
the full low-bus block it is internally consistent with demand exceeding the4A
supply setting before a1% foldback could recover the bus.

This is a hard power-path fold signature, but the present capture alone cannot
separate PSU current limiting from lead/connector resistance or a controller-
created current transient. Per the operator's rule, powered climbing pauses
here. Do not retry, raise a threshold, or attempt45/50 until the physical event
is corroborated and the power-path cause is understood. Same-rig stock-AM32 at
matched speed remains the attribution control before blaming motor or stage.

E804 is staged offline as the causal current-actuator correction, not as a
license to rerun. E803 disproved a fixed1% reduction for severe excursions.
E804 preserves the complete-block residual and chooses a bounded1..5% step via
division-free severity bands; E803's1.853x residual maps to5%. DMA still only
publishes, foreground still owns PWM, and all thresholds/terminal stops remain
unchanged. Frozen SHA is
`389B0BFE9AC8848C849B891F32883BC0B246DE85DF963312D4BB8F9540A8D10D`.
It remains unflashed until the operator resolves or corroborates the physical
power-path fold and explicitly resumes powered work.

E805/E806 complete the matched adaptive set offline. E805 is the lean
qualification successor to E800 (SHA
`39FF82CD266582473B2FFEAC99578273D46EEA712BD39C83DD6E23C585AE03CE`); E806 is
the aggregate CPU successor to E801 (SHA
`CABD2BB3463E87CEAD66A25A235194DA2E8E69EAFFED4DB79AF6958B3FA1DB20`). Both
reuse the exact old feature sets, pass release and four-root M0 audits, and stay
unflashed under the same E803 pause. E805 alone can later qualify10/25/50;
E806's observer-affected results are CPU characterization only.

## Superseded next decision

This was the correct stop after E796, but E797 supplied the causal ramp change
and a clean35% result. It is retained as history, not the current instruction.

## E850-E855: persistence falsified as the speed wall; independent bus fold at 36%

Fixed advance20 was retained after the earlier A/B. E851's protected ramp did
not reproduce the diagnostic43% result: adaptive nominal-current control folded
and then stopped before the lock boundary. A bounded diagnostic histogram was
therefore added: accepted events plus persistence rejection index0..11 for each
electrical sector, reset at every coherent live-duty publication and dumped
only after shutdown. The final u32 form adds no calls or software arithmetic
helpers to any motor ISR root.

Matched stable epochs show the opposite of a persistence-limited ceiling:
10%/filter12 produced7.4..9.4 rejected passes per accept;29%/filter6 produced
1.2..2.8;31%/filter5 produced1.16..2.46. Sector structure is measurable, but
qualification work gets easier with speed. Raw persistence-reject count is not
the cause of the42-44% loss-of-lock event.

Raising the explicit nominal estimator from3.5A to4.0A still caused one
software foldback and a stable31% ceiling, without bus-low or nFAULT evidence.
E855 therefore made only signed-current excess report-only for one diagnostic;
the physical4A PSU and bus-sag/nFAULT/raw-validity/tracking/deadline/watchdog
stops remained. Its1%/s ramp stopped at36% on the independent bus guard after
35 low-bus samples and a complete50-scan block below8.4V (bus743/vref1506/
vcal1662), reason25 at29.823280s. The terminal36% persistence ratios were only
1.01..1.28 rejects/accept, so persistence did not deteriorate first.

This satisfies the operator's hard-fold pause rule. Further powered climbing is
paused at a power-path/supply condition, not declared a silicon, wiring, CPU,
or BEMF ceiling. E855 SHA is
`3BD142AB79C8AEF2FC05C08458D3DE2B61B246E051BD9FF49771D28EBC1C4B30` and the
board was verified with all gates, ENABLE, MOE and CCRs off after the run.

## E856-E857: classify the lock collapse, then bound energized dwell offline

The new graybeard transit-surge memo asks for an interval suffix before fault
taxonomy. That experiment was already completed in E832: the last complete
electrical cycles grew from roughly624..667us to998..999us over about20 cycles,
with sector structure. It is a growing lock cascade, not one late/early pair.
The memo's proposed persistence and advance levers were also already tested:
persistence rejection burden falls sharply with speed, and fixed advance20 was
the best bounded point without removing the collapse.

E855 exposed the remaining protection mismatch. The accepted-event watchdog
was a fixed1000us. At the proven40% point, one expected commutation is about
96us, so a missed-event sector could remain energized for roughly ten expected
steps before Tracking8 safed it. E857 adds an opt-in diagnostic policy: each
accepted event may tighten that deadline to three controller intervals,
bounded200..1000us. The limit is monotonic downward within the segment, so the
growing slowdown cannot extend its own deadline. TIM6 still polls independently
every100us; electrical, bus, current telemetry, nFAULT, feedback and deadline
guards are unchanged. Uninitialized estimates below64 half-us are ignored. The
existing128-event tail is enabled to show whether a
future stop occurs before the old current/bus collapse.

The first arithmetic form used `saturating_mul(3)`. A full objdump scrub—not
the direct vector-root audit—caught LLVM emitting `__aeabi_lmul` inside the
Recorder. It was retired before flash. The installed bounded const form only
multiplies values <=666 (product<=1998), has no arithmetic helper in the emitted
Recorder/watch path, and passes all four ISR-root audits. Host boundary,
monotonicity, wrap and deadline tests pass. E857 SHA is
`9D8CD681636F59BCFBFBF4BA2F531972447891C5EB9D2477480E3E087EAF1520`, release-s/
thin-LTO/codegen1, text122060/data1180/bss23976. Disabled guard3/18 and role6/6
passed; final outputs/ENABLE/MOE/CCRs were off and nFAULT high. No motor run was
made, so this is a staged diagnostic safety candidate, not an envelope result.

E856 separately tested preserving comparator mux pending state and found no
physical EXTI latch from a forced masked comparator transition. That candidate
is retired without powered use.

## E858/E860: interval-tail analysis and replay correction

The first E858 analysis incorrectly treated IT85 `reference_half_us` as the
value consumed by E857 and published exact hypothetical trip points. That table
is retracted. The field is EV_ACC `this_zc`: the newly measured accepted-event
interval. E857 consumes foreground `S.average_interval`, derived from six
sector-indexed, smoothed commutation intervals. The recorded value is nearly
twice `gap_us`; substituting it made the replay look exact when it was not.

`scripts/drv_interval_tail.py` now names the field
`measured_interval_half_us` and limits itself to claims the wire data proves:
strict ordinal/bounds/CRC decoding and complete ordinal-aligned groups of six
accepted events. Three host tests pass. The valid cycle-period results are:

| run | first complete cycle | last complete cycle | observed maximum |
|---|---:|---:|---:|
| E832 advance18 | 624 us | 999 us | 999 us |
| E834 advance16 | 712 us | 1,166 us | 1,208 us |
| E836 fixed advance20 | 548 us | 589 us | 851 us |
| E841 scheduled advance | 678 us | 1,049 us | 1,049 us |
| E843 scheduled35 | 750 us | 1,086 us | 1,091 us |

Thus E832/E834/E841/E843 still prove sustained slowdown; E836 remains a tight
suffix interrupted by two bad cycles rather than the same monotonic cascade.
The archives do not prove an E857 trip time or false-trip rate. Clean E809's
40%/60s and E854's31%/60s images omitted both the needed average history and an
event tail. No powered run was made under the E855 hard-fold pause. The first
eventual powered use must be a known-clean E857 control, not a ceiling climb.

## E859: source-check the proposed missed-edge behavior

The full transit-surge memo was read. Its diagnosis agrees with the retained
trace: the electrical interval diverges before the current/bus messengers. Its
instrument, persistence-depth and advance requests have already been completed
by E832 and E850-E855. The remaining phrase "step on the estimate" was checked
against the cached AM32 G071 source before adopting it.

Stock behavior is narrower. `interruptRoutine` accepts a comparator event and
arms exactly one COM at `waitTime+1` (`AM32/Src/main.c:945..972`). The COM
callback disables its own interrupt, commutates once, updates the estimate and
re-enables comparator input (`:919..940`). If the following edge is absent,
there is no ordinary next COM; the foreground path waits until the separate
45,000-tick BEMF timeout and then returns to polling/re-kick (`:2663..2678`). A
diff of the local AM32 bench tree shows board UART, telemetry and safety changes
but no edits to those three control paths.

Therefore an indefinitely predicted commutation stream would be a new control
architecture, not AM32 parity, and was not implemented during the power-path
pause. E857 instead provides the source-compatible conservative action: stop a
known missing-event cascade sooner. Any future bounded predicted-step design
needs its own safety and lock evidence rather than borrowing the AM32 label.

## E861: record the watchdog's actual decision inputs

The corrected offline audit showed that the historical IT85 ring cannot replay
E857 because it omitted `S.average_interval`. E861 upgrades the diagnostic ring
to versioned IT86-v2. Each accepted event retains four u16 values: actual TIM17
gap, EV_ACC measured interval, the foreground average passed into the watchdog,
and the deadline in force before the event. UART serialization remains entirely
post-run. The strict decoder supports historical IT85 without inventing the
missing fields and only offers exact violation checks for IT86.

The exact E857 release feature closure was recovered from Cargo's fingerprint
and rebuilt. Recorder grew16 emitted bytes; the full image changed from
text122060/data1180/bss23976 to122212/1188/24488. The +512 BSS is the two added
u16 fields across128 retained events. Full disassembly found no arithmetic
helper in Recorder or transitively from the four live ISR roots. Four speed
policy and five interval-wire/transition host tests pass.

E861 SHA is
`8CA10D96ACFF5C9CA00F64B9E0241084AD222B1F516EF92713C871EBE97CBCF4`.
It was flashed and verified on the explicitly selected G071. Disabled guard
3/18 and role6/6 passed; final outputs, inputs, ENABLE, MOE and CCRs were low/
zero and nFAULT high. UART was closed. No powered run was made under the E855
hard-fold pause. This prepares an exact known-clean control once the pause is
resolved; it does not itself expand or qualify the envelope.

## E879-E888: replacement motor, fast sag, and the first 45% boundary

The post-incident image now has an active 4 A signed-current foldback/terminal
stop plus a separate DMA-side bus stop: three consecutive coherent 226 us scans
more than 5% below the same-wake bridge-off bus/VREF reference. The old 50-scan
absolute bus guard remains. E879 ran 10% BEMF for 5 s without a false trip.
E880's diagnostic-only three-frame bus-code injection latched reason 26 and
disabled ENABLE and gates; the third frame's acquisition stamp was 85 us before
completed shutdown. This tests the actual digital stop path, not the physical
analog response to a collapsing supply. E881 restored the non-injecting image.

All points below are on the replacement motor and use the protected diagnostic
image; they are **exploration**, not lean repeated-hold/restart qualification.
The source is the retained MCP captures named `duty50_879` through `duty50_888`.

| Entry | Commanded duty / powered time | Final-cycle period | Max current residual / allowance | Minimum bus | Result |
|---|---|---|---|---|---|
| E879 | 10% / 5 s | not decoded | 2732 / 17272 | 11.617 V | timed stop, FASTBUS0 |
| E882 | 10% / 30 s | not decoded | not captured | 11.665 V | timed stop, 96737 COM |
| E884 | 10->20% / 30 s | not decoded | not captured | 11.557 V | timed stop, ~1051 eHz final |
| E885 | 10->25% / 45 s | 776..811 us | 5861 / 17282 | 11.581 V | no electrical fault, FASTBUS0 |
| E886 | 10->35% / 45 s | 579..629 us | 6712 / 17340 | 11.713 V | no electrical fault, FASTBUS0 |
| E887 | 10->40% / 60 s | 512..550 us | 11415 / 17251 | 11.737 V | timed stop, FASTBUS0 |
| E888 | 10->45%, stopped near 27.397 s total energized | 474..546 us | 11258 / 17261 | terminal bus code 1123 | FASTBUS1 / cause6; postreport partial |

E882 and E883 are retained failed **transfer admissions**, not higher-duty
tests: driven acquisition released normally (reason22) but transfer result2
had zero powered events. A later live-armed entry succeeded, so this is
intermittent. Post-run DRIVENENTRY refusal telemetry was added without ISR
cost; its cause has not yet been reproduced on that image.

E888 is the first physically observed fast-sag event with the new guard.
Terminal bus/VREF 1123/1501 versus baseline 1211/1506 is a normalized drop
of about 7%; the complete-block absolute-bus counter remained zero. Thus the
new guard stopped a sub-11 ms excursion the old absolute block would not yet
have classified. The last IT86 cycles contain a wobble and late slowdown, but
the report has no synchronized per-scan bus/interval ordering, so neither
"loss of lock caused the sag" nor "sag slowed the motor" is proven. The MCP
capture ended mid-D85 before POWERPATH/DONE; after the run `off/p/i` read all
gates, ENABLE, MOE and CCRs off, nFAULT high. Do not infer an exact POWERPATH
reason code for this run from a line that was not captured. No replacement-
motor 50% attempt has been made.

### E889-E891: the early stop is real; timing and bus now share a clock

E889 changed only the 40→45% ramp shape to 1% per second on the protected
non-recorder image. It still tripped the fast bus guard at 45% after roughly
10 s at target: exact reason26 at40.450052s powered, with normalized bus ~6%
below the same-wake baseline and maximum signed-current residual11600/17340.
So the E888 stop was not solely an instantaneous 5%-step surge.

E890 added an opt-in 16-frame DMA scan ring (226 us cadence) and dumped it
ahead of the large waveform archive. It stopped at44% before45 was ACKed.
The ring shows nine bus scans at98–100% of baseline, several at96–97%, then
three consecutive below95%; all carry duty440. This diagnostic adds DMA work,
so its 44% stop cannot be transferred to the non-recorder envelope.

E891 added the final accepted-event timestamp to that early ring header.
It stopped at43% before44 was ACKed, reason26 at41.003959s powered. Both the
16 bus frames and128 IT86 accepted events pass CRC/ordinal checks. With
`last_event_us=41003885`, the final event tail can be placed on the bus scan
timebase:

| Powered timestamp | Observation |
|---|---|
| 41.001630 s | bus99.06% of baseline, last sampled normal level |
| 41.001735 s | accepted-event gap135 us versus ~80–86 us normal |
| 41.001856 s | next bus scan97.11% |
| 41.002013–41.003359 s | repeated long/short gaps, average interval grows |
| 41.002986 s | first scan below95%, followed by one recovery above95% |
| 41.003438/.003664/.003890 s | three consecutive low scans90.18/91.46/91.13%; trip |

This makes a timing disturbance the leading software hypothesis: the first
*observed* long accepted interval precedes the first *observed* bus decline,
and the bus degrades over multiple coherent scans while VREF remains steady.
The226 us bus sampling gap leaves room for an unseen dip before that event,
so this is not proof that timing caused the sag. Raw phase-shunt excursions
coincide with the dip but are PWM-asynchronous pulse samples, not calibrated
DC-link current. The signed50-scan maximum remains11382/17308 and the old
absolute-bus low-code count is zero. The new stop is doing its intended job;
neither bus threshold nor current ceiling should be loosened. Next is a
protected aggregate CPU/scheduling measurement at a known-clean duty, then
one-variable timing A/B before another high-duty attempt. The E891 recorder
image is not a qualification build.

### E892-E893: protected CPU probe at replacement-motor duty

Installed diagnostic SHA `C4755AEA2B43766B0E1A3B27C401159533FAD963A87868CED895532C4862FF5A`
combines the E867 control path, active 4 A signed-current actuator, fast
relative bus stop, and aggregate CPU accounting, without the interval/bus
tail recorders. Release `opt-level=s`/thin LTO/codegen-units 1 and four
motor ISR-root soft-arithmetic audits pass. Disabled `cpucheck` measured
software pair costs 2/7/10 us; guard 3/18 and roles 6/6 passed.

E892 ramped 10->35% but stopped at powered 24.595469 s with `POWERPATH
reason=26`. `cap0` omitted the detailed CPU and FASTBUS summary. The report
therefore proves only a fast-sag stop on this observer-affected image, not
its CPU utilization or the cause of sag. The prior non-CPU diagnostic image
held 35%; neither result is lean qualification.

E893 ramped 10->30% and its CPU meter completed a fault-free 20.000009 s
window. CRC-decoded IRQ union was 10.975258 s, **54.88% observer-affected
occupancy**. Nested-inclusive roots were guard 0.700692 s (3.50%), COMP
5.053882 s (25.27%), COM 2.958670 s (14.79%), DMA 2.262014 s (11.31%).
FASTBUS tripped0, maximum signed-current residual5688/17350, phase rails0,
absolute-bus low codes0. The serial capture lost later post-run summary
lines during the large `cap1` dump, so it is not a complete DONE/POWERPATH
record. Explicit `off/p/i` afterward found ENABLE/gates/MOE/CCRs off and
nFAULT high. A 55% union at30% does not support CPU saturation as the
immediate43-45% wall; worst-case comparator acceptance latency remains open.

Retained captures: `captures/duty50_892_cpu_replacement_35pct_mcp.txt` and
`captures/duty50_893_cpu_replacement_30pct_mcp.txt`. Next timing A/B must
retain active electrical guards and report the image-specific observer cost;
do not use the E892 35% stop to downgrade the non-CPU envelope.

### E894-E895: DMA-below-comparator scheduling A/B fails its low-duty gate

An opt-in diagnostic variant kept the E891 control, recorders and all stops,
changing only ADC-DMA NVIC priority from64 (COMP/COM peer) to128 (below
COMP/COM); TIM6 guard stayed0. It built release-s/thinLTO/codegen1, passed
four motor ISR-root arithmetic audits and disabled guard/role checks.
Postrun `COREPRIORITY comp=64 com=64 guard=0 dma=128` confirmed the change.
E894's `cap1` report was incomplete after the large D85 archive; it shows
only2 controller COM, so is not a successful low-duty check.

E895 repeated the 10% control with concise `cap0`. It entered powered BEMF
but stopped after6.808 ms with reason11. Its diagnostic readback is
`ADCFAULT stage=27` / `DMAFAULT code=7 service_gap_us=227`, while FASTBUS
did not trip and the 16 CRC-valid bus scans remain near baseline. Source
audit maps code7 to `dma_snapshot::Lease::finish`: the transfer changed while
the handler copied the prior frame. Thus simply lowering DMA priority lets
the next scan overtake feedback service even at10%; this candidate is retired
before any high-duty attempt. The guard safed outputs. Source inspection also
found that `average_current_live::scan_raw` holds `interrupt::free` across
fast-bus, absolute-bus and current-block work. NVIC priority cannot preempt
the middle of that DMA critical section, so this A/B does not settle the
full comparator-vs-DMA latency question.
The original-priority protected control was rebuilt (SHA `4DE8034F...`),
verified/flashed on G071, and reset; `off/p/i` confirmed outputs disabled and
nFAULT high. No powered test has yet been run on the restored hash. Captures
`captures/duty50_894_dma_low_10pct_transfer_partial_mcp.txt` and
`captures/duty50_895_dma_low_adc_fault_mcp.txt` retain the negative A/B.

### E896: COMP-top scheduling A/B also fails its low-duty gate

A second one-variable feature changed only COMP priority64->0, leaving
COM/DMA at64 and TIM6 guard0. Build, four ISR-root arithmetic audits,
disabled guard and role checks passed. E896 10% entered powered BEMF but
stopped after3.717 ms with `POWERPATH reason=3` (`TickGap`) and six COM;
FASTBUS did not trip. The guard correctly revoked outputs. The top-priority
comparator can delay the equal-priority 100 us guard tick enough to violate
its deadline; this is not a candidate for a 43-45% run. The original-priority
control was rebuilt and restored (SHA `F7E9DF6F...`), G071 flash verified,
and `off/p/i` confirmed disabled outputs and nFAULT high. No powered run has
yet exercised this exact restored hash. See
`captures/duty50_896_comp_top_tickgap_mcp.txt`.

### 2026-09-19 reverse32k advance22-high control A/B

The opt-in release-s/thin-LTO image SHA `6FAF3C39...` retained the reverse
32k carrier, 4 A signed-current actuator, fast 5%/three-scan relative bus
stop, absolute-bus, nFAULT, tracking and watchdog protections. Its one
control change was scheduled advance level20 below35% and22 at/above35%.
The minz-core polling-mode `TEMP_ADVANCE=18` means startup is not byte-
identical to the fixed20 image. Four ISR-root M0 arithmetic audits and a
host schedule-boundary test passed. After flash/disabled checks, a separate
ordinary-start 10%/15s BEMF hold passed its deadline, no electrical trips,
43,884 COM and bus minimum11.545V.

The next ordinary-start90s window live-ramped to35, dwelled, smoothly
reached40 and dwelled about12s, then ACKed45%. It stopped at powered
67.326891s on `POWERPATH reason=26`: the active fast normalized bus guard
observed three226us-spaced scans below95% of the1210/1506 raw bus/VREF
baseline. No current foldback, phase rail, absolute-bus-low or tracking
event fault was reported; nFAULT stayed high. Post-stop all gates, ENABLE,
MOE and CCRs were off. The `DONE reason=8` wrapper is not a Tracking8
verdict; `POWERPATH reason=26` is authoritative. Final estimated1587eHz
is a *fault-time* estimate, not a matched steady-state speed comparison.
The change did not remove the upper-duty transient; no50% attempt followed.
Capture: `captures/reverse_direction_2026-09-19_32k_advance22_45_fastbus.txt`.

The exact fixed20 reverse32k lean SHA `06B1C1D6...` was restored and
flash-verified on the explicit G071, hard-reset, then passed disabled
guard3/18, role6/6, roledu100 6/6, duty6/6, p/i off/nFAULT1. No powered
run after restoration. The carrier40k, advance22 and simple IRQ-priority
variants have all failed to clear the wall; these negatives do not yet
identify which comparator/commutation timing event initiates the collapse.

A subsequent single run on the restored exact fixed20 reverse32k lean
image smoothly reached42.5% and completed the full60s BEMF window on
deadline. The 42.5% dwell itself was approximately40-45s by host timing,
not MCU-timestamped. There was no fast sag, current foldback, phase rail,
absolute bus-low, tracking or nFAULT event; bus minimum11.832V and final
estimate~1792eHz. Final p/i showed all outputs off/nFAULT high. This
places a clean exploratory midpoint above40%, but is not repeated
qualification and does not prove45% safe. Capture:
`captures/reverse_direction_2026-09-19_32k_lean425_60s.txt`.

The next one-variable timing A/B rebuilt the same reverse32k lean feature
closure with fixed advance18 instead of20 (SHA `1B43148B...`), retaining
all electrical stops. Four ISR-root soft-arithmetic audits, explicit-G071
flash verify/reset and disabled guard/role/duty/off checks passed. A
separate ordinary10%/15s BEMF gate passed. On its FIRST smooth high-duty
run it ACKed42.5% but fast bus sag stopped at35.148728s powered
(`POWERPATH reason=26`). No current foldback, phase rail, absolute-bus-low,
tracking event fault or nFAULT was reported; final outputs off. No45/50
attempt on fixed18. The single fixed20 clean42.5% hold and single fixed18
failure do not establish a statistical ceiling, but reducing advance was
not a straightforward cure. The exact fixed20 SHA `06B1C1D6...` was
restored/verified/reset and passed disabled checks; no powered run after
that restore. Capture:
`captures/reverse_direction_2026-09-19_32k_fixed18_425_fastbus.txt`.

An additional fixed20 reverse32k run used a30s serial dwell at ACKed42.5%,
then ACKed45% through five0.5% steps. The operator paused while the45%
terminal read was in progress. That read was terminated without retaining
its terminal report, so the attempt is **unclassified**: neither a45% pass
nor a diagnosed fault. The host requested abort, explicitly sent `off`,
verified all gates/ENABLE/MOE/CCRs0 and nFAULT high, then closed COM41.
Retain the partial result without repeat-until-pass. No further powered
work until the operator resumes. Capture:
`captures/reverse_direction_2026-09-19_32k_fixed20_slow45_operator_pause.txt`.

After the operator resumed, the exact same fixed20 reverse32k lean image
repeated ordinary startup and a smooth ramp toward a planned42.5% settle,
then45%. It ACKed42.5%, but the fast normalized bus guard stopped the run
at41.834987s powered before the settle/climb completed: `POWERPATH
reason=26`, FASTBUS tripped1, current foldback0, phase rail0,
absolute-bus-low0, tracking event fault0, nFAULT high. Final outputs
off. No43-45 commands were sent and the first failure was retained.
This same-image negative repeat means the earlier single42.5%/60s
completion is **not qualification** and42.5% cannot currently be treated
as a reliable safe rung. The paused45 ACK remains unclassified, not a
pass. Lean telemetry has no aligned event/bus tail, so the new failure
does not by itself settle whether timing leads the bus. Capture:
`captures/reverse_direction_2026-09-19_32k_fixed20_425_repeat_fastbus.txt`.

Offline scheduling read after this A/B: the CRC-valid 32k IT87/BS85 fault
tail contains long accepted gaps before the first sampled sub95% bus scan.
Several late accepts occur about100us after the preceding *ADC acquisition*
stamp, but that stamp is not DMA handler entry/exit, and there are also
non-late accepts at similar offsets. Do not label DMA preemption the cause
from this alignment. Local `HARDWARE_SENSE_FILTER.md` and
`capture_filter.rs` already establish a COMP2->TIM2_CH2 hardware capture
route, including disabled pulse tests and a low-duty observe-only motor
probe. That observer had material runtime cost and overcapture; it is not
an acceptance oracle. A narrow next diagnostic would freeze the latest
raw hardware-capture timestamp/overcapture flag only on the first large
accepted-gap anomaly, alongside the existing TIM2 accepted timestamp,
without per-event UART or a capture-based control decision. First verify
counter/reset/mux epoch and disabled behavior, then a low-duty powered
cost gate, before using its evidence near42.5-45%. If it cannot stay
observer-small, retire it and use a different timing discriminator.

## 2026-09-19 — sparse CPU measurement and rate-metric pivot

The exact fixed20 reverse32k feature closure plus opt-in `bench-cpu-sparse`
was built release-s/thinLTO/codegen1 and archived as SHA
`FCC62B4BA7FBC7B8A83D93672AD603F214B4B92AEFD696F304876F5A6F27B523`.
It gates the ISR clock reads until ACKed35% duty and samples every17th
invocation of TIM6, ADC_COMP, TIM16 and DMA. The four ISR-root M0 helper
audits passed. The earlier full-entry/exit aggregate CPU meter disturbed
startup or tripped fast bus shortly after35%; it was retired without a
steady-state CPU claim. Sparse measurement is also diagnostic, not lean.

One sparse run reached35% and completed the30s hold on deadline, with no
current foldback, fast-bus, absolute-bus, phase-rail, tracking or nFAULT
trip. Its sample window was30.151s. Inclusive estimated root occupancy,
from sampled mean duration multiplied by total calls, was TIM6 4.0%,
ADC_COMP 30.7%, TIM16 25.8%, DMA 12.3%, sum ~72.7%. The corresponding
40% attempt ACKed the target, then tripped fast bus after32.716s powered,
roughly20.433s after the sparse window began. It had zero foldback, rail,
absolute-bus, tracking or nFAULT trips. Estimated root occupancy there
was 4.0%, 30.1%, 28.6%, 12.3%, sum ~75.0%. Inclusive roots double-count
preemption and probes add timing overhead; these are not exact lean CPU
percentages. They do not demonstrate CPU saturation as the immediate
40-43% fault. Full retained captures are
`captures/reverse_direction_2026-09-19_32k_cpu_sparse35_pass.txt` and
`captures/reverse_direction_2026-09-19_32k_cpu_sparse40_fastbus.txt`.

The side review correctly noted that a single pass/fail A/B near the
stochastic42.5% boundary is weak evidence. Measure rates at a clean40%
point instead: coherent individual bus scans below97%/95% of the fixed
start reference, late accepted intervals, and their clustering per unit
time, with minimal hot-path cost and post-run reporting. Test one causal
variable at a time against those rates while retaining all stops. Two
corrections: revisit-off35 **was** previously tested and failed Tracking8;
fast-sag reason26 proves the MCU measured a sharp bus dip, not that the
PSU definitely entered CC. After the sparse run the exact lean SHA
`06B1C1D6...` was reflashed/verified/reset, p/i all outputs off and
nFAULT1, COM41 closed. No powered run after restore.

## 2026-09-19 — rate census at 40, 42.5, 45%

An opt-in `bench-rate-census` diagnostic counts complete DMA bus scans
below97% and95% of the fixed start reference, their sub95% streaks, and
accepted comparator intervals more than25% above the preceding smoothed
interval. It begins only after the live40% duty publication; counters are
reset at each run start. It adds no ISR clock read, division, print, or
protection change. Release-s/thinLTO/codegen1 ELF SHA `DBE64097...`,
text125908/data1200/bss17192; four ISR-root M0 arithmetic audits pass.
Disabled checks and ordinary10%/15s powered gate pass. The diagnostic
cannot qualify the lean image.

| Profile (same image) | Powered result | Census duration | DMA scans below95% | Longest sub95% run | Accepted >1.25x prior avg |
|---|---|---:|---:|---:|---:|
| 40% only | full70s, reason2 | 61.069s | 467 / 270217 | 1 | 51176 / 629117 |
| 40->42.5% | full70s, reason2 | 59.606s | 1315 / 263742 | 2 | 52018 / 623265 |
| 40->45% | reason26 at60.819517s powered, after45 ACK | 47.790s | 97 / 211459 | 3 | 42486 / 502397 |

No run had current foldback, absolute-bus or phase-rail codes, tracking
fault, or nFAULT. The faulted run had *fewer* isolated low scans than
either clean one; its decisive feature was one three-scan sequence, which
the existing fast-bus guard caught. Coarse late-accept fraction remains
about8.1-8.5% across all three and does not separate the fault. The
rate-metric suggestion was worth testing, but total notch rate alone is
not a predictive signal in these data; nor can this counter order a ZC
event against the electrical sag. Do not infer PSU CC or a board defect.
All three raw transcripts are retained under `captures/reverse_direction_2026-09-19_32k_rate*.txt`.

The exact fixed20 lean SHA `06B1C1D6...` was then restored/verified/reset
on the explicit G071, and p/i confirmed outputs off and nFAULT high;
COM41 closed. No more motor runs after restore. A better next diagnostic
is a small time-aligned first-low-scan + accepted-gap snapshot (or a
bounded matched-speed reference test), with a measured observer-cost gate
before returning near45%. 42.5 remains unqualified despite two clean
exploratory runs; reverse45 has no clean hold and50 is still unattempted.

## 2026-09-19 — priority-only low-gate recheck and exact offline alignment

`bench-dma-below-comp` alone (without the previous simultaneous lease
change) produced release-s/thinLTO/codegen1 SHA `53B47947...` with the
same text/data/bss sizes as lean. Four ISR-root math audits and disabled
preflights passed. Its first ordinary10%/15s protected gate stopped at
powered5.114ms: `POWERPATH reason11`, `DMAFAULT code5`, flags3/NDTR5,
service gap432us. Fastbus did not trip; no high-duty test was attempted.
This was not actually a new discriminator: buried E895 had already tried
the same priority-only move and failed the ADC/DMA class at6.808ms.
The new result independently confirms the low-gate constraint but should
not have consumed a flash/run. Candidate retired; exact fixed20 lean
restored and verified, outputs off/nFAULT high, UART closed. Capture:
`captures/reverse_direction_2026-09-19_32k_dma_below_only_adcfault.txt`.

Re-decoding the already retained CRC-valid44.5% IT87/BS85 fault
(`captures/reverse_direction_2026-09-19_32k_it87_bs85_445_fastbus.txt`)
puts a127us accepted gap at36,773,641us, a subsequent bus scan at
36,773,703us at97.46% of start reference, and the first sampled sub95%
bus scan at36,773,929us. Another152us gap ends at36,772,678us and is
followed by a97.21% bus scan at36,772,799us. The timing excursions thus
precede sampled severe sag, consistent with a timing-led event, but ADC
spacing226us leaves physical onset uncertain. Neither this alignment
nor the rate census proves a particular ISR or controller defect.

## 2026-09-19 — fault-triggered three-scan snapshot at 44%

`bench-fast-sag-causal` is an opt-in diagnostic that stores the most
recent one-to-three consecutive sub95% bus scans with acquisition and
DMA service timestamps plus the controller state read at service. It
does not alter the three-scan fast-sag stop, current actuator, nFAULT,
tracking or watchdog decisions. Release-s/thinLTO/codegen1 SHA
`70CF7D04...`; four ISR-root math audits and disabled guard/role/duty
checks pass. Ordinary10%/15s and40%/60s powered holds completed on
deadline with only isolated low-scan snapshots. This is a diagnostic
image, not lean qualification.

One stepped attempt ACKed44%, then fast-sag reason26 stopped it at
47.139408s powered before the attempted44.5% command was accepted.
Bus/VREF across the decisive three scans was1146/1508,1115/1505,
1128/1500 against the fixed1207/1504 baseline. COM count advanced
441040->441042->441044 and measured `this_zc` values were188/194/175
half-us: the controller was still accepting and commutating across the
sampled bus collapse, with no obvious held sector. Current foldback,
phase rails, absolute-bus-low codes, tracking and nFAULT stayed clear.
This does **not** prove good phase angle or exclude a preceding transient.

The timestamp limitation is material: `acquired_us` is the scan trigger
origin, not the bus-channel aperture, and the controller state is sampled
at later DMA service. Service was77-90us after origin; the `last_event_us`
in each row was25-69us *after* its scan origin. Thus these rows cannot
order the physical bus drop versus those events. Future alignment should
reuse the event timestamp already taken by the acceptance guard and
retain a tiny recent-event ring, then compare it with a bounded bus-channel
aperture estimate; do not add a clock read or bulk recorder to every
COMP service. The exact lean SHA `06B1C1D6...` was restored/verified,
outputs off/nFAULT high, UART closed. Capture:
`captures/reverse_direction_2026-09-19_32k_causal44_fastbus.txt`.

## 2026-09-19 — V2 accepted-event ring at the 45% fast-bus stop

The narrow V2 diagnostic adds eight accepted-event timestamps to the
three low-scan rows, frozen on the first and third low scans. It reuses
the acceptance path's existing timestamp; there is no extra clock read,
division, or change to the electrical protections. Release-s/thinLTO/
codegen1 SHA `FB6BC15A...`, text127100/data1200/bss17428; four motor
ISR-root soft-arithmetic audits and disabled preflights passed.
Ordinary10%/15s and40%/60s protected gates passed. The 40% run included
a single approximately92%-of-reference bus sample, correctly ignored
by the unchanged three-consecutive-scan stop.

The guarded gradual ramp ACKed45%, then stopped `POWERPATH reason=26`
at powered44.574632s. Three consecutive226us scans measured bus/VREF
1132/1507,1132/1504,1134/1510 against fixed1211/1507 baseline;
current foldback0, phase rails0, absolute-bus-low0, tracking0, nFAULT
high. Recent accepted timestamps were chronological through the dip:
first-low ring `44573445,44573540,44573618,44573701,44573810,
44573884,44574010,44574094` us; third-low ring `44573884,44574010,
44574094,44574217,44574282,44574370,44574478,44574550` us. There
is no conspicuous missing-edge or held-sector gap in this approximately
1ms window. That is a *local* negative, not proof of correct phase
angle, physical bus onset order, or a PSU fault. The bus conversion
aperture follows the stored scan-origin timestamp, and each controller
snapshot is made later at DMA service. The prior IT87 timing excursion
remains real; it may be outside this short window or may not be causal
in every fault.

The side review's suggestion to freeze eight preceding **bus scans**
is distinct from the eight accepted timestamps now captured and is
worth a bounded-cost diagnostic. A repeated cohort at45%, and sector-
conditioned fault comparison, would be stronger than a one-shot A/B.
Raw and normalized captures are
`captures/reverse_direction_2026-09-19_32k_eventring45_fastbus.txt`
and `captures/reverse_direction_2026-09-19_32k_eventring45_decoded.txt`.
The exact fixed20 lean SHA `06B1C1D6...` was restored/verified/reset;
p/i all outputs off, nFAULT high, COM41 closed. Reverse45 remains
unqualified and50 is still unattempted.

## 2026-09-19 — eight-scan pre-trigger cohort at reverse 45%

V3 extends the opt-in causal diagnostic with a DMA-owned ring of eight
coherent raw ADC scans (scan-origin time, IA/IB/IC, bus, VREF), frozen on
the third low-bus scan. It adds no clock read or soft arithmetic to the
normal scan path and does not alter the fast-bus, current, nFAULT,
tracking or watchdog decisions. Release-s/thinLTO/codegen1 SHA
`E33CBCA1...`, text128060/data1200/bss17632; four exact motor ISR-root
math audits pass. Disabled guard3/18, roledu1006/6, duty6/6 and p/i
off pass. The protected 10%/15s and 40%/60s gates both completed on
deadline, with no electrical stop or current foldback at40%.

On the same V3 image, three gradual ramps each ACKed45% and each
stopped on the unchanged fast-bus reason26 at powered40.643372,
38.310908 and39.433587s. Every stop disabled cleanly, nFAULT stayed
high, and no average-current foldback or tracking stop preceded it.
The eight pre-trigger bus raw codes as a percentage of the fixed
start reference are:

| Run | Eight chronological bus samples (%) | First-low sector | Accepted gaps during/after first low |
|---|---|---:|---|
| A | 98.4, 96.6, 96.6, 96.9, 98.9, **93.3, 93.9, 93.1** | 3 | 142, 88, 130, 89 us |
| B | 100.1, 97.2, 97.4, 98.4, 98.6, **92.9, 94.1, 92.1** | 5 | 118, 90, 95, 87 us |
| C | 93.6, 96.0, 93.1, 93.3, 95.5, **92.2, 90.9, 89.5** | 5 | 138, 80, 99, 92, 87 us |

VREF varies less than1% across these windows. A/B show several modest
notches before the terminal cluster; C had intermittent below-95% scans
through the preceding1.8ms. Accepted events and commutations continued
through the sampled sag in all three; no common held-sector or missing-
edge precursor appears in these windows. A/C show one late accepted
gap only after the first low scan origin; B does not. ADC scan origin
precedes the bus-channel aperture, so these records still cannot prove
exact physical onset order. This is a repeated 45% failure, not proof
that the PSU, motor, timing, or sense network alone caused the load surge.
The live 45% ACK averages were176/179/179 half-us (about1894/1862/
1862eHz), while stop averages were195/190/203 (1709/1754/1642eHz).
All three therefore slowed between the ACK and terminal snapshot under
the same requested duty. That supports a developing load/control event,
but these two endpoints do not establish whether sag led slowdown or
the converse.

The raw phase samples do show excursions near CSA rails: A's third
low scan IB=4073, B's second IA=35 and third IB=55; C's terminal
IB=0. IMPORTANT correction to the last-turn erratum: frozen V3
`ia/ib/ic` labels were already physical IA/IB/IC. The hardware DMA
scan is ADC0/1/4, but `adc_stream::poll` applies
`dma_snapshot::logical(raw)` and returns [ADC4,ADC1,ADC0,bus,VREF]
before `powered_timer` and this recorder see the frame. Do **not**
swap the archived IA/IC labels; the prior erratum was false. Raw codes,
bus samples and timestamps were always sound.
These are asynchronous PWM-phase samples; the nominal scale is about
87 counts/A (equivalently 11.5 mA/count, with uncalibrated zero/gain),
not the side review's dimensionally inverted "11.5 counts/A". One
sample is not a DC-link average or a safe peak-current
rating. The zero `phase_rail_codes` output on C is not exonerating:
the fast-bus guard returns from `scan_raw` before the separate rail
counter when the third low scan trips. Source audit confirms that
`powered_guard::phase_valid` deliberately accepts all 12-bit phase
codes under `bench-average-current`, with `EXPLOREPROTECTION
pulse_stop=0 phase_rail_stop=0`; the historical1.3V pulse clamp is
not linked. This was an explicit response to false trips from PWM
ripple, not an accidental skipped clamp. A calibrated pulse/SOA policy
still needs design work; do not quietly re-enable an arbitrary raw
threshold.

The side suggestion of48k carrier is a new hypothesis, not a result:
40k already passed startup but still failed45% with the same fast-bus
stop, and32->48k reduces fixed-duty ON-time by one third, not one
half. No carrier-only climb is justified on present evidence. The
offline phase-role map is now source-grounded: reverse core steps1..6
are B+/A-/Cfloat, C+/A-/Bfloat, C+/B-/Afloat, A+/B-/Cfloat,
A+/C-/Bfloat, B+/C-/Afloat. But each archived `core_step` is read at
DMA service, after the ADC channels. Source configuration PCLK/4=16MHz
and SMP160.5+12.5 cycles yields nominal physical IC/IB/IA aperture
endpoints around trigger+10/+21/+32us, with bus around+43us,
excluding trigger overhead; a COM can occur during that sequence.
Consequently the current rows cannot yet prove a hot floating phase
or phase miswire. The mistaken source IA/IC swap was built offline as
SHA `9BEE913C...`, NEVER flashed, and has been reverted. The only
remaining post-archive source correction is a misleading static
`fault_triggered=1` post-run label, also unflashed. Captures:
`captures/reverse_direction_2026-09-19_32k_scanring45a_fastbus.txt`,
`...45b_fastbus.txt`, `...45c_fastbus.txt`.

Exact reverse32k fixed20 lean SHA `06B1C1D6...` was restored and flash-
verified on the explicit G071, then p/i confirmed all gates, ENABLE,
MOE and CCRs0 with nFAULT high; COM41 closed. No further motor run
after restore. Reverse45 has no clean hold and50 remains unattempted.

## 2026-09-19 — replacement motor, reverse48k: advance moves the held boundary

The new motor's reverse48k fixed-advance20 lean staircase image (`F6F88F85...`)
had already held45% for60s but repeatedly stopped near48–49% on fast three-scan
bus sag. Two paired IT86/scan captures showed electrical-cycle growth before
the first sampled sub95% bus value. The IT87 origin capture's first extended
cycle ended on a **physical-only** comparator gap while bus still read about98%;
software-only level-revisit accepts became dense later in the collapsing
suffix. That clears level-revisit as the initiator in that capture, but neither
recorder proves whether the physical edge arrived late or the rotor slowed.
The CPU-union and every17th-IRQ timing probes moved the failure boundary;
their ~76% inclusive estimate is not lean-image occupancy.

A one-variable `bench-running-comp-top` priority image (`A840041D...`) made
ADC_COMP priority0 while DMA/COM remained0x40. Its first protected10% gate
stopped before hold with ADC/DMA lease code7 and a233us service gap. Outputs
were safe; the candidate was retired without a high-duty run. The exact
fixed20 lean image was restored and disabled checks passed. Do not infer that
priority alone was the49% cause from this invalid high-duty comparison.

The advance A/B kept reverse48k carrier, all electrical stops, and level20
through startup/low duty; at >=35% it set level22. Cargo feature
`bench-reverse-advance22-high` was decoupled from the old reverse32k carrier
dependency so carrier selection stayed independent. Release-hybrid opt-s/
thinLTO/codegen1 image SHA
`B9B5F6B06570B459DAEA54FEFB1C28E714916CF1AE2DE9745571ACCD462E8A23`
is frozen at `captures/reference/reverse_48k_advance22high_newmotor_20260919/`.
The four motor ISR-root soft-arithmetic audit passed, the bounded advance
boundary host test passed, and explicit-G071 flash/reset plus disabled guard
3/18, roledu100 6/6, duty6/6, p/i off/nFAULT high all passed.

On this same protected lean image, a10%/10s gate and45%/30s control passed.
The 48% result was **3/3 separate30s powered windows plus one60s powered window**,
without an MCU target-duty ACK timestamp, each ending
on the normal powered deadline with zero fastbus, average-current foldback,
phase rails, tracking stop and nFAULT; terminal speed estimates were about
2044, 2044, 2070 and2057eHz. The60s run had611593 COM and fastbus0.
This is a substantial exploratory envelope gain over fixed20's repeated48%
faults, not yet a fully calibrated lock/current qualification.

The same level22 image ACKed50% once and fastbus-stopped at powered25.167s
(only a few seconds at target after the long ramp, not about20s). It then ACKed49% once and fastbus-stopped at25.616s.
Both had no foldback, safe finaloff/nFAULT high, and terminal speed estimate
~1782eHz, below their ACK speeds of roughly2.1keHz. 50% was reached but NOT
held or qualified; 49% is also not a clean hold. An attempted level24-at>=35%
image (`0E2D8AB...`, identical protection) passed10% and45%/30s but
fastbus-stopped at48% before49 ACK. **CORRECTION:** source audit afterward
found minz-core `interrupt_advance_level` accepts only18..22; published24
silently falls back to `TEMP_ADVANCE=18`. The post-run `ADVANCEPROFILE`
reported the *published*24, not effective ISR advance. This is an INVALID
24-degree A/B, and its failure must not be cited as evidence about24.
The misleading feature was removed. The frozen level22 image was restored/
verified/reset. Disabled
guard/role/duty and p/i off/nFAULT high passed; COM41 is closed.

Do not claim a physical hardware ceiling or cure it by raising the4.5A PSU
cap. The 49–50% event is a delayed speed/current/bus cascade that the fast
sag stop safely terminates; the edge-vs-rotor onset remains unresolved.
The next useful experiment should distinguish timing execution/acceptance
from true rotor deceleration with a genuinely low-perturbation measurement,
then A/B a causal control change. Do not make the diagnostic image itself
the envelope qualifier, and retain the exact failed captures.

After the60s hold, the same restored level22 image passed a3/3 ordinary-
restart cohort at48%. Each trial injected a tracking loss at powered5.000s,
observed the expected safe Tracking8 stop, settled disabled, made a **fresh
ordinary startup** (not flying re-acquisition), restored the live target in
eight foreground-only5% steps from10% to48%, and completed the remaining
~19.235-19.236s powered deadline. All three had foldback0, no fast sag and
final outputs off/nFAULT high. Captures are
`captures/reverse_direction_2026-09-19_advance22high_48_restart_[abc].txt`.
This supports repeatable recovery at the48% exploratory operating point; it
does not mean a full19s hold *at restored48%*: eight2s-spaced restore steps
leave only the tail of that window at48%. It also does not qualify49/50 or
establish 48% calibrated thermal/current margin.
