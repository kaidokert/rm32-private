# AGENTS.md

This file applies to `binz/` and everything below it.

2026-09-19 PERFORMANCE CURVE/CPU addendum: opt-in rate-curve diagnostic
SHA `10CD4D47...` completed protected single runs at every5%-rung10..50.
Final speed increased401->2096eHz, signed-current *uncalibrated proxy*
45->1603mA and approximate board bus11.66->11.52V; no fast-sag/foldback
fault and no sub95% scan streak longer than1. The qualified-image5% endpoint
ACKed duty then stopped Tracking8 at2.114s powered, so5% is not sustained.
Separate sparse-CPU diagnostic SHA `C1746D3D...` completed40% with inclusive,
observer-affected root sum82.1%; its climb toward50 fast-sag-stopped at last
ACKed47%, with no50% ACK. Do NOT transfer that stop or occupancy to the lean
qualified image. The latter was restored/flash-verified after diagnostics,
with disabled checks passing and outputs off. Full table, formulas, captures
and uncertain75% projection are at the top of `DUTY_50_CAMPAIGN.md`.

2026-09-19 CURRENT installed image: reverse48k/effective-advance26
high-duty COM-priority A/B SHA
`0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0`
(`captures/reference/reverse_48k_com_top_high_20260919/shell-pwm.elf`).
COM priority becomes0 only at live duty>=48%; COMP remains0x40, TIM6/DMA0.
The comparator snapshots its pre-arm sector so COM preemption cannot relabel
the accepted event. All electrical stops, fast three-scan5% bus-sag kill and
nominal4A signed-average foldback remain active. Same-image true>=30s target
dwell cohorts at10/25/50 each passed3/3;50% ordinary injected-loss restart
passed3/3, plus exact-image10/25 restart regressions passed. All runs safe
and final serial `off/p/i` verified gates/ENABLE/MOE/CCRs0, nFAULT1; COM41
closed. This qualifies the50% bench point under the defined hold/recovery
tests, not a75% envelope or a fully characterized performance curve. The
predecessor2/3 fault cohort remains evidence; the priority A/B result is
promising, not unique-cause proof. See top of `DUTY_50_CAMPAIGN.md` and exact
captures. Board remains OFF on this current candidate; do not silently flash
an older frozen image. Work only in binz, never sibling rm32.

2026-09-19 HISTORICAL COM-LAG DIAGNOSTIC (before the current image): protected opt-in images sampled actual ZC-to-COM
time against programmed wait at>=48% duty. The paired image `1F0F2369...`
found comparator ISR still active at wait expiry in1869/2031 paired50% samples
(92%). Its50% attempt fast-sag-stopped1.58s after target ACK; an earlier probe
image `AB89155F...` fast-sag-stopped2.81s after ACK. Both were diagnostic,
not qualification; all guards including the three-scan5% fast-sag hard stop
remained active. Firmware was restored to frozen lean image SHA `79BDA4C57...`,
explicit G071 flash-verified and reset, disabled guard3/18+role6/6+duty6/6
passed, p/i gates/ENABLE/MOE/CCRs off and nFAULT high, COM41 closed. Details
and captures are at the top of `DUTY_50_CAMPAIGN.md`. The observation supports
a scheduling floor; causation of the bus-sag events is not yet proven. Never
run with fast-sag report-only. Preserve accepted-edge ordering before any
COM-priority intervention. At this stage 50% was unqualified; the later
current-image cohort above supersedes that status.

2026-09-19 HISTORICAL predecessor image was protected reverse48k effective-advance26
SHA `79BDA4C57DCD8D097B25E084F96C1812582D38232768F25D5307332E36EC0CA9`
at `captures/reference/reverse_48k_advance26_ackdwell_newmotor_20260919/shell-pwm.elf`.
It is flashed/verified on the explicit G071, with disabled guard3/18,
roledu100 6/6, duty6/6 and p/i outputs off/nFAULT high. COM41 is closed
between tests. Physical PSU cap remains4.5A; active signed-average nominal4A
foldback, three-scan fast 5% bus-sag stop, nFAULT, tracking and watchdog remain.
The new postrun `LIVEACK` MCU stamp measures time at *target duty*, not merely
total powered time. True30s target-dwell cohorts passed3/3 at10%
(30.701017/30.689944/30.723852s) and3/3 at25%
(31.187478/31.198311/31.175032s), each zero sag/foldback and final outputs
off. The first true50% target-dwell cohort is 2/3: runs A/B completed
31.367032s conservative lower bound and31.359143s direct ACK-to-stop,
zero fastbus/foldback; run C fastbus-stopped at44.770010s powered,
21.158641s after its50% ACK, with phase_rail_codes2 and no current foldback.
All three safed outputs, nFAULT high. This is an intermittent 50% failure,
not 50% qualification; do not repeat-to-pass. Do not describe the earlier30s/60s powered-window runs as target
holds: the10→50% ramp alone takes roughly20s after startup, and those30s
windows gave only about6–7s at50. The prior60s powered-window 50% failure
at41.153s happened after roughly18s at50; exact older ACK time was not
recorded. A 50% target-dwell cohort has NOT passed. The fixture now requires
`--target-hold-ms` with `--qualify-hold` and checks the MCU ACK stamp.

2026-09-19 DIAGNOSTIC follow-up after the2/3 true50% cohort: separate
IT86 accepted-interval + BS85 coherent bus/current-tail image SHA
`A074D0E0B9CB0C049CCCB62A14F541B06EF2B03C3B526EC951BB9BC4F35193D4`
ran a fixed three-attempt50% set: normal55s powered deadlines A/C, fastbus26
at29.611156s powered /5.945094s after50% ACK in B. The two clean final
21-cycle tails had no complete cycle above520us; B's last two complete
cycles were549/562us. Its first136us accepted gap ended at29.609834s while
bus was still98.38% at the preceding scan29.609712s; bus then read96.74%
at29.609938s and first fell below95% at29.610616s. Timing/accepted-edge
stretch therefore preceded sampled bus collapse, but does not distinguish
rotor deceleration from late comparator acceptance. B's terminal phase
samples reached raw14/4089, asynchronous PWM-phase values, not calibrated
average current. No current foldback or nFAULT. Observer image is diagnostic,
not envelope qualification. Exact image/captures are frozen under
`captures/reference/reverse_48k_advance26_it86_bs85_newmotor_20260919/`
and `captures/reverse_direction_2026-09-19_advance26diag_it86bs85_50_[abc].txt`.
The low-observer SHA79BDA4... was then re-flashed and verified on explicit
G071; disabled guard3/18, roledu1006/6, duty6/6, p/i outputs off/nFAULT high
passed; COM41 closed. No motor run followed restoration.
Operator math/observer challenge audited offline afterward: the fast-sag
decision itself has no elapsed/event arithmetic, its 12-bit `u32` products
are <2^31, and a new regression replaying diagnostic fault B's six raw
pretrip/trip bus/VREF codes passes. The three terminal normalized samples
were91.81/94.35/90.41% of the fixed set reference with stable VREF; this
is not a fast-guard integer overflow. The diagnostic fault appeared5.945s
after50% ACK (not a fixed20–30s appointment), while a low-observer 50% fault
also occurred, so IT86/BS85 instrumentation is not necessary though it
shifts timing. Do NOT adopt GRAYBEARD_SAG_REFERENCE's filtered moving
reference on this evidence: in this capture its putative ~93.6%-of-set trip
line would let the middle94.35% sample break the three-scan streak despite
its91.81/90.41% neighbors, masking a measured sharp dip. Cause upstream of
the raw bus event remains open; no slow-thermal or PSU-capacity claim proven.
The primary sampled TIM17 clock passes a new60s/100us-cadence host regression
across ~915 hardware wraps and both2^24/2^25-us boundaries. Failure COM
counts were435347 (lean) and244271 (diagnostic), while same-image clean runs
reached564k and563–565k respectively: no deterministic large-counter
threshold explains this cohort. A state-dependent control arithmetic or IRQ
scheduling defect is still possible and should be investigated upstream of
the raw bus event.

Effective high-duty advance26 was implemented only in binz's post-COM wait
override (no sibling/core edits), with an in-powered-path witness and clean
M0 helper audit. On its prior image SHA ADCEE7E..., 50% completed three30s
powered windows, but a60s window fastbus-stopped at41.153s. Three ordinary
restart trials restored50% and completed the remaining powered window, not
a30s dwell at restored50. At effective24, two49% powered windows completed
normally; two50% attempts fastbus-stopped. The operator saw approximately
1.6A on the PSU display during the second49% run; this is an observation,
not ADC calibration or a reason to raise the4.5A cap. See campaign notes and
retained `captures/reverse_direction_2026-09-19_advance*` files.

2026-09-19 HISTORICAL protected lean reverse48k image SHA
B9B5F6B06570B459DAEA54FEFB1C28E714916CF1AE2DE9745571ACCD462E8A23
(`captures/reference/reverse_48k_advance22high_newmotor_20260919/shell-pwm.elf`),
formerly installed/OFF/COM41 closed. Explicit G071 verify/reset; disabled guard3/18,
roledu100 6/6, duty6/6 and p/i outputs off/nFAULT high PASS after restoration.
This is the replacement motor in the operator-confirmed desired reverse direction,
physical PSU limit4.5A, firmware signed-average nominal4A with active foldback,
fast fixed-start 5% bus-sag three-scan stop, nFAULT/tracking/watchdog stops.
The image holds advance level20 below35% and22 at/above35%; do not confuse it
with the fixed20 SHA F6F88F85... or the retired 24-high/COMP-top probes.
On this exact image, 48% completed three separate30s powered windows plus one60s
powered window (target-duty dwell was not stamped), all deadline stops, zero foldback/fastbus/phase rails, terminal
~2044/2044/2070/2057eHz. It ACKed50% once, then fastbus26 at25.167s
powered after ~20s at target, terminal~1782eHz; ACKed49% once, then
fastbus26 at25.616s, terminal~1782eHz. Both safed normally with nFAULT high.
Thus48% is a strong exploratory point, NOT a stamped30s target dwell or50% qualification. Normal-start
recovery at48% subsequently passed3/3 injected-loss trials: first stop at5s,
fresh ordinary startup, eight guarded5% duty-restore steps back to48%, and
remaining powered deadline completed without foldback/sag. Full5-50% curve
and50% hold qualification remain unfinished. Do not raise the
4.5A physical limit to chase49/50; the defect is a speed/current/sag cascade,
not proven supply headroom. See latest DUTY_50_CAMPAIGN.md section and retained
`captures/reverse_direction_2026-09-19_advance22high_*` files.

The opt-in COMP-top A/B SHA A840041D... failed its10% gate with ADC/DMA
lease code7/service gap233us and was retired before high duty. The attempted
advance24-high SHA0E2D8AB... is an INVALID advance A/B: minz-core's scheduled
ISR accepts only levels18..22 and silently falls back to18 when published24.
It passed10%/45% and fastbus-stopped at48% before49 ACK, but that says
NOTHING about actual level24. The misleading feature was removed from binz.
Any future scheduled-advance experiment outside18..22 requires an explicit
minz-core range change and effective-ISR verification; publishing alone is
not enough. Do not edit `../minz/core` casually under binz-only ownership.
Neither probe is installed. CPU sparse/union probes are
observer-affected and do NOT establish lean occupancy. The paired IT86/scan
captures remain timing-first at48% (cycle stretch before first sampled sub95%
bus); IT87 origin shows the first long gap was physical-only, with revisit
density rising afterward. This does not yet separate a late comparator accept
from genuine rotor deceleration.

2026-09-19 RESTORED CURRENT lean staircase SHA
F6F88F85019CA22F7EC37D7A2F7A1B1294984E887D2B35DC3DCD806B64C9DA26,
installed/OFF/COM41 closed after explicit G071 verify/reset and disabled
guard3/18, role6/6, duty6/6, p/i off/nFAULT high PASS. CPU-sparse diagnostic
SHA36CDF247BD15037ED5745023CE0F0B8CDF63457441BD6402FCB9E4B7E99F4D51
was flashed/tested and retired: two ACKed45% attempts fastbus-stopped near
powered7.084/7.076s, vs one lean45%60s clean. Every17th IRQ brackets agreed:
guard~3.96us, COMP~16.6-16.7us, COM~27.8us, DMA~27.6us sampled means;
sum(calls*sampled_mean)/(stop-first) yielded75.76/75.83% INCLUSIVE upper
bound across35->45. Nesting double counts and probe/ELF timing shifts the
wall, so not lean CPU occupancy or proof of saturation. Full aggregate
measurement below also~75.9% observer-affected. Four ISR-root math audits
PASS for sparse image, all electrical stops retained. Next one-variable
scheduling A/B: raise ADC_COMP to priority0 only; TIM6 remains0, DMA/COM64.
This tests late physical accepts at the high-speed regime; do not cite the
earlier low-duty E478 preemption falsification as a high-duty verdict.

2026-09-19 CURRENT installed/OFF CPU aggregate diagnostic SHA
329527C6E8B7754CA139D90F2F86808408090D047F41771F59CE7E4472B78233,
COM41 closed, frozen `captures/reference/reverse_48k_cpu_running_newmotor_20260919`.
Release-hybrid opt-s/thinLTO/codegen1, text129740/data1200/bss17220,
four motor ISR-root arithmetic audit PASS. Explicit G071 flash/verify/reset,
disabled guard3/18, role6/6, duty6/6, CPUCHECK3 modes, p/i off/nFAULT high
PASS. Meter is absent during startup and begins at first powered IRQ after
ACK>=35%. First45 target attempt refused late seed before power; second
ACKed40 and fastbus-stopped reason26@12.016922s BEFORE45. Observer affected:
lean image held45%60s clean, so this diagnostic cannot set an envelope.
Its valid2.016422s CPUUNION partition was IRQ1.529994s / foreground0.486428s
=75.88% IRQ union while traversing35-40%; roots guard37695, COMP621287,
COM591488, DMA279524us (inclusive nesting). Probe cost is unmeasured and
itself moved the wall; not a lean occupancy estimate or proof of saturation.
Next use the existing every17th sparse IRQ probe at45 before inferring an MCU
limit. No sibling rm32 edits.

2026-09-19 ORIGIN-TAIL FOLLOW-UP: installed/OFF/COM41 closed image SHA
07A427DAFBF38B942E1873D20DF98E247D2E2E82A507606A8201997DA36C541B
is a diagnostic only (IT87 accepted-origin tail +16-frame BS85 bus tail),
frozen `captures/reference/reverse_48k_origin_tail_newmotor_20260919`.
Explicit G071 flash/verify/reset, disabled guard3/18, role6/6, duty6/6,
p/i off/nFAULT high, four motor ISR-root math audit PASS. 10% gate PASS;
first48 ramp refused late seed before power, second ACKed47 then stopped on
fastbus26 before48. All captures retained. IT87/BS85 CRC and ordinals decode.
At47%, epoch origin counts physical-only2829, software-only83, both69;
late counts171/27/0 respectively. In final128 events first64 had1 software-only,
last64 had11, and final20 had8. BUT the first cycle extension to537us
ended22.455046s with a132us PHYSICAL-only gap; bus was still~98% at
22.455154s. First sub95% scan was22.456284s. Software-origin density then
rose in the collapsing suffix. Thus level revisit is not the initial late
event in this capture; it appears to respond to deteriorating physical edge
quality, though amplification remains possible. This diagnostic's extra
COMP-path work can move the wall (it failed at47), so no lean envelope claim.
Next measure aggregate IRQ occupancy in a separate, startup-clean diagnostic
at the45% clean regime before changing comparator timing.

2026-09-19 CURRENT reverse48k diagnostic SHA
E235D083253A39C5F58600B0B963B415CDA0A02D290B2C642942A0521DBCA911,
installed/OFF/COM41 closed after three protected48% attempts. Frozen
`captures/reference/reverse_48k_saginterval_newmotor_20260919/shell-pwm.elf`;
release-hybrid opt-s/thinLTO/codegen1, text129280/data1208/bss18700,
four motor ISR-root arithmetic audit PASS. Explicit G071 flash/verify/reset;
disabled guard3/18, role6/6, duty6/6, p/i off/nFAULT high PASS. Adds ONLY
fault-only eight-scan sag record and 128-accepted-event IT86 tail; recorder
overhead means no lean qualification transfer. Electrical stops unchanged.
First10% gate refused a late seed, retained; second10%/10s gate PASS.

At48% on this diagnostic: attempt A fastbus26@22.685115s; attempt2
fastbus26@23.873627s; attempt3 reached48 then Tracking8@22.756605s and
ordinary restart completed at10% (only1.48s remained, no restoration to48).
All three first segments ACKed48 and stopped before30s; no current foldback
or nFAULT. A IT86 cycle grew from ~500us to615us ending22.684486, with a
phase ADC rail at22.684338 and first sub95% bus scan22.684564. Earlier
extended gap118us ended22.683989 before either. Attempt2 cycles grew from
~474-504 to534/521/522us ending23.870620/23.871141/23.871663, before the
first sub95% scan23.873073; final cycle563us overlapped the dip. Thus two
independent captures support timing deterioration BEFORE terminal bus collapse,
not bus-first as a necessary cause. This does not prove whether comparator
acceptance or mechanical rotor motion initiated the timing change. Attempt3
tracking-only reinforces a marginal control regime. IT86 payload CRC/ordinal
decoded; its speed-watch transition replay failed on attempt2 at171366->171367
(236 expected,239 recorded), likely foreground estimator interaction but NOT
yet explained, so do not claim exact watch-policy replay from that capture.
No further blind48/50 retry; next useful A/B is accepted-event origin vs
interval/bus timing. Captures `captures/reverse_direction_2026-09-19_saginterval_48_*`.

The preceding protected off-at50 image SHA
1EE27AFD486E909D7A98BF7909F8AA3DD417F298363D433548E31BEC95283E22
passed a10% gate and a45%/60s hold (0 foldback/fastbus, ~1926eHz), but
fastbus-stopped at49% in one ramp and at48% in another. In the49% attempt
the50 ACK never occurred, so the off-at50 branch was NOT exercised; no
revisit A/B verdict. The48% attempt ACKed48 then stopped fastbus26 at
24.968686s. Retained captures `captures/reverse_direction_2026-09-19_staircase_off50_*`
and `*_staircase_on45_hold60_a.txt`, `*_staircase_on48_hold30_a.txt`.

2026-09-19 SEED-TIMING FOLLOW-UP / CURRENT FLASH: current installed/OFF image
is the diagnostic single-clear A/B SHA
EB4EB75CC8F205AA78B0A7B960FF40F4764F6CA5532F6B92C5041A649CD20097,
COM41 closed, explicit G071 verify/reset and disabled guard3/18, role6/6,
duty6/6, p/i outputs off/nFAULT high PASS. Do NOT transfer its envelope result
to the lean staircase baseline below. Two separate retained cohorts: the
double-clear seed-stage SHA43FDFD5C... had10/12 fresh10% starts and full10s
protected holds;2/12 refused before power. The detailed double-clear prefix
SHA DB4BBBE4... had6/8 starts and two late-arm refusals. The single-clear
SHA EB4EB75C... had11/12 starts/10s holds and one late-arm refusal. This small
A/B difference does not establish an improvement; retire the single-clear
feature for envelope work. Failed detailed prefix shows selected edge already
90-91us old at handoff entry, then ~37-41us setup; final arm age256-262
half-us ticks against wait288-289, leaving13-16us below the unchanged20us
reserve. The second bridge clear accounts ~5-6us but its omission did not
eliminate failure. Stage evidence is in `captures/reverse_direction_2026-09-19_seedprefix_*`
and `*_clearonce_*`; all failures were safed before timer arm.

Host `--qualify-hold` now recognizes BOTH normal deadline paths: TIM6 guard
reason2 or foreground `coast_stop=1`/POWERPATH reason0 with an accepted event
within1ms of requested duration. The former verifier mislabeled two normal
10s completions as failures; every retained capture was replayed, giving the
correct10/12 startup cohort. This change does not reclassify a real fault.
Next flash a protected baseline plus revisit-off-at50 A/B, with the existing
fastbus/current/nFAULT/tracking/watchdog stops unchanged. 50 remains unqualified.

2026-09-19 CURRENT REPLACEMENT-MOTOR REVERSE48K STAIRCASE: installed protected
lean image SHA256 F6F88F85019CA22F7EC37D7A2F7A1B1294984E887D2B35DC3DCD806B64C9DA26,
frozen at `captures/reference/reverse_48k_staircase_newmotor_20260919/shell-pwm.elf`.
It uses release-hybrid opt-s/thinLTO/codegen1; four motor ISR-root soft-math
audit PASS. Flash/verify/reset on explicit G071 SN066CFF343433464757233430,
disabled guard3/18, roledu1006/6, duty6/6, p/i off/nFAULT high PASS. Board is
currently OFF, COM41 closed. Physical PSU setting last reported4.5A; firmware
signed-average nominal4A with adaptive foldback/hard stop, fast >5%/three-scan
bus stop, nFAULT/tracking/watchdog retained. This image replaces flat startup
with the existing autonomous 50->200eHz startup staircase for both first and
ordinary second starts; no sibling rm32 edits.

On this exact image, three 10%/30s uninterrupted holds PASS and three injected
Tracking8 ordinary-restart trials PASS: outputs disabled1s, fresh startup,
remaining run completed, no foldback/fastbus/nFAULT. Three 25% recovery trials
also PASS, restoring10->15->20->25 in three2s steps. The 25% uninterrupted
hold cohort is ONLY2/3: two 30s holds clean; one refused before powered hold
at `DRIVENENTRY refusal=4`, `coast_stop=8`, `fly_age=260` half-us, no timer arm.
Successful arms had age196..236 ticks and remaining54..102 ticks. Approximate
handoff wait was288..290 half-us ticks, so the failed remaining~28 ticks was
below the existing40-tick/20us reserve. Do not call25% startup/hold qualified
and do not loosen that reserve without causal evidence. Strict fixture
`--qualify-hold` now rejects pre-power refusals, restarts, foldbacks and fastbus.
Retained captures `captures/reverse_direction_2026-09-19_staircase_newmotor_*`.
50% remains reached but unqualified on earlier diagnostic images. Next: explain
the intermittent late seed arm with post-stop stage ages, preserve protections,
then resume meaningful envelope work.

2026-09-19 REVISIT48 A/B INCONCLUSIVE: matched protected reverse48k control
SHA FBDEC9608B37FBBBA9FD8D6D8507B75D1B2E4B5E3B2E6ACD75AF78DBE4B2A76A
and revisit-off-at>=48 SHA
1187161A49279BA7C690FF7C59AF5FB2D65929D14FFC4328B6A76147416FDB37
each reached49/48% respectively before Tracking8 and a refused second startup.
One stochastic stop per image cannot establish revisit benefit or harm, and
neither reached50. Both retained with final outputs off/COM41 closed. An
intermediate diagnostic removal of `bench-fast-cycle-report` accidentally
turned the single-cycle floor back into a kill; its reason12 at11% is INVALID
as a revisit comparison. The report-only feature was restored for the matched
images. See `captures/reverse_direction_2026-09-19_revisit48_*.txt`.

2026-09-19 REPLACEMENT MOTOR SMOOTH50: Same CURRENT installed diagnostic
SHA72FF21BB...; a protected 1%-per-0.5s climb ACKed50 then fastbus-stopped
reason26 after167307us (ACK23641419, stop23808726), with no current
foldback, tracking, or nFAULT stop. This joins two 5%-step ramps whose
post-50 ages were131851/155129us. The final5% jump is not necessary;
different climb durations still yield a ~0.13–0.17s post-50 event.
This is not proof of deterministic latency or of causal ordering. Existing
32k rate-census study found coarse low-scan and late-gap rates did not
predict the fault; do not repeat that instrument unchanged. 50 unqualified;
all final p/i off/nFAULT high, COM41 closed.

2026-09-19 REPLACEMENT MOTOR 50%-ACK TIMING: CURRENT board diagnostic
SHA72FF21BB57709F9CC332FC2D36F551762D51659CA7F27118891E98C4894C0327,
installed/OFF/COM41 closed after explicit G071 flash and disabled
guard3/18, role6/6, duty6/6, p/i off/nFAULT high. It fixes the upward-live-
command restart bypass in source and adds foreground-only `LIVEACK50` time;
four ISR-root arithmetic audits and normal-restart policy7/7 PASS. Protected
10%/15s hold PASS. One injected Tracking8 stopped at5s and ordinary restart
waited1s disabled, but new-motor second startup refused BEMF acquisition; no
powered retarget validation yet. Two identical ACK-paced10→50% ramps both
ACKed50 and fastbus-stopped reason26 at131851/155129us after the MCU50%-ACK,
with zero current foldback/tracking/nFAULT stops; final p/i off/nFAULT high.
Phase rails3/0, so phase ADC rail is not necessary for fast sag. This
reframes the earlier total-time63ms similarity: on this faster ramp the stop
is within~0.15s of45→50 step, not demonstrated as seconds-long build-up.
No50 qualification. Captures and interpretation in
`captures/reference/reverse_48k_restart_ackstamp_20260919/README.md`.

2026-09-19 RESTART-OWNERSHIP FIX STAGED, NOT FLASHED: CURRENT board is restored
reverse48k cache-owner SHA42C9D49E3C8494D926341D08D39416808D52B2F2D0360F59FCCF4BE3B073154F,
installed/OFF/COM41 closed, p/i all gates/ENABLE/MOE/CCRs0/nFAULT high.
The CPU diagnostic SHA5CC8AB4B stopped Tracking8 near40% and a live45%
request during its ordinary restart was immediately applied at low speed;
`NORMALRESTART3 resume_target=450 resume_applied=450 resume_steps=0` confirmed
the recovery-command bypass. The following fastbus stop invalidates CPU
characterization, not the known cache-owner envelope. Source now defers live
upward commands during normal-start restoration into its existing5%-per-2s
resume schedule and ACKs actual published duty; lower commands remain
immediate. Policy harness7/7 PASS. New opt-in foreground-only50%-ACK stamp
candidate SHA72FF21BB57709F9CC332FC2D36F551762D51659CA7F27118891E98C4894C0327
is frozen at `captures/reference/reverse_48k_restart_ackstamp_20260919`,
release-hybrid release-s/thinLTO/codegen1, four ISR-root math audits PASS,
NOT flashed or powered-qualified. Do not attempt another climb using the old
recovery bypass; first disabled checks and a bounded protected restart test.
The two prior50% fastbus stops were63ms apart in total powered time, but no
MCU50%-ACK timestamp existed; fixed time-after-50 is not yet established.

2026-09-19 NEW MOTOR 50% FAST-SAG TRACE: CURRENT board opt-in diagnostic
SHA796707248532CA4A700711A9092A7F1A649E9987241C418A105D05CD01E3B12C,
installed/OFF/COM41 closed. Same reverse48k/cache-owner electrical policy as
SHA42C9D49E plus only eight-scan sag ring/event ring/PWM snapshot. Release-s/
thinLTO/codegen1 and four ISR math audits PASS; disabled guard3/18,
roledu1006/6, duty6/6, p/i off/nFAULT high PASS. One protected
10->25->40->45->50% run ACKed all rungs, then fastbus reason26 at powered
19.956141s with no current foldback, tracking or nFAULT. The eight bus/VREF-
normalized scans spanning1.8ms were97.29/95.77/97.57/95.71/95.36/92.62/
92.36/92.90% against fixed start reference. The final three physical phase
frames had IA19, then IA4038/IB568/IC3067, then IC0: large alternating
phase excursions; signed-current average did not fold. Accepted-event gaps
over the captured interval were68/67/88/87/59/116/84/110/78/87/120/61us,
and commutations continued. One116us gap preceded the first sub95% scan,
but bus was already95.36% at the prior scan. The trace does NOT establish
whether current/timing or bus began first. At least one low bus sample was in
PWM OFF time; not a single-edge notch. Full retained capture:
`captures/reverse_direction_2026-09-19_new_motor_48k_sag_ring50.txt`.
50% remains unqualified. Do not raise guards or assume PSU/motor cause from
these frames. Next useful A/B should target controller-timing/current-demand
causally; existing `running-level-revisit` is one separable candidate.

2026-09-19 NEW MOTOR CACHE-OWNER A/B: CURRENT board exploratory diagnostic
SHA42C9D49E3C8494D926341D08D39416808D52B2F2D0360F59FCCF4BE3B073154F,
installed on explicit G071 and OFF after run; COM41 closed. Pure
`Latest` tests5/5 and four ISR math roots PASS. Disabled guard3/18,
roledu1006/6, duty6/6, p/i off/nFAULT high PASS. One protected live
10->25->40->45->50% run ACKed every rung; at50 it stopped on the independent
fast5%/three-scan bus guard reason26 at powered20.019369s. One phase exact
rail was counted (`phase_rail_codes=1`) without the previous cache ADCFAULT,
confirming the ownership change worked. No current foldback, tracking,
nFAULT or fast-cycle stop. Final average185 half-us (~1801eHz), while the50
ACK interval172 half-us (~1938eHz); only an endpoint slowdown, not an onset
trace. 50% still NOT qualified. Final p/i gates/ENABLE/MOE/CCRs0, nFAULT1.
Do not infer the physical source of sag or safe pulse current. Next diagnostic
is the existing opt-in eight-scan fast-sag ring at50 with this cache policy;
retain all electrical stops. Archive README under
`captures/reference/reverse_48k_cache_owner_20260919` has exact test result.

2026-09-19 NEW MOTOR EXPLORATORY50 / CACHE-OWNER A/B: Installed reverse48k
diagnostic SHA69BB6A10... passed disabled guard3/18, role6/6, duty6/6,
idle p/i and active4A `avgnominal`. One protected run ACKed25/30/35/40/45/50%
in5-point steps with ~6.5s dwells. No stop during the lower holds; after50
ACK it stopped reason11 (`ADCFAULT stage30`, `Latest::publish` invalid) at
powered40.459441s, before a qualified50 hold. Fault-only frame physical
IA/IB/IC=3289/0/4063, bus/VREF=1110/1510, TIM1 CNT1258/ARR1332/CCR666.
One bus code was ~8% under its start reference, but the independent
three-scan fastbus guard did not trip; no current foldback, tracking, nFAULT
or fast-cycle stop. Final p/i off/nFAULT high, COM41 closed. Retained
`captures/reverse_direction_2026-09-19_new_motor_48k_explore50.txt`.
Source audit found a protection-ownership inconsistency: `powered_guard`
explicitly accepts/counts exact phase rails under the signed-current owner,
and the owner averages them, while the non-authoritative `Latest` cache
independently hard-stopped on one exact phase rail. New opt-in experimental
candidate SHA42C9D49E... makes only phase 0/4095 cache-valid; bus/VREF rails,
impossible DMA words, stale/reordered data still latch; electrical stops are
unchanged. Pure cache tests5/5 and four ISR-root M0 audits PASS. Archived at
`captures/reference/reverse_48k_cache_owner_20260919`; NOT YET FLASHED or
motor-tested. This is not proof the instantaneous phase current is harmless;
an independent calibrated pulse-current policy remains open.

2026-09-19 NEW MOTOR 20% SMOKE: Same installed reverse48k diagnostic
SHA69BB6A10...; disabled guard3/18, roledu1006/6, duty6/6 and idle p/i
passed again. Active4A signed-current policy accepted (`avgnominal`). One
16.000004s powered run used a live10->15->20% sequence with both higher-duty
ACKs received. It completed on deadline reason2, final average354 half-us
(~941eHz), bus minimum11522mV, zero foldback, low-bus codes, phase rails,
tracking, nFAULT or fast-cycle stops; fast5%/three-scan bus stop did not trip.
Final p/i all gates, ENABLE, MOE, CCRs0 and nFAULT high; COM41 closed. This is
a single exploratory20% smoke, not repeated qualification. Operator suspects
the prior motor may have been damaged; treat this as a hypothesis, not a
demonstrated cause of earlier high-duty behavior.

2026-09-19 NEW MOTOR SMOKE TEST: After the operator replaced the motor and
requested one easy spin, the installed reverse-48k fault-only diagnostic image
(SHA69BB6A10D4CAC868B6918810189DCDA03B9B0A6C005E82F1B0312D7FDF8A18A2)
passed disabled guard3/18, roledu1006/6 at1333 ticks, duty6/6, and idle p/i
with all gates/ENABLE/MOE/CCRs0 and nFAULT high. Active4A signed-current
protection was confirmed by `avgnominal accepted=1`. One10% BEMF run completed
the 10.000005s powered window on deadline reason2, average840 half-us
(~397eHz), bus minimum11581mV, zero current foldbacks, phase rails, low-bus
codes, tracking/event or fast-cycle faults; fast5%/three-scan bus stop did not
trip. Operator reported it sounded much quieter and smoother. Final p/i again
showed all outputs off and nFAULT high; COM41 closed. This is a single
replacement-motor smoke test, NOT repeated10% qualification or evidence for
25–50%. The image remains installed; no further climb was run.

2026-09-19 48k FAST-SAG A/B: CURRENT board restored exact reverse32k
lean SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988,
explicit G071 flash/verify/reset, p/i all gates/ENABLE/MOE/CCRs0,
nFAULT1, COM41 closed. Matched fast-sag diagnostic SHA32683823EA380500449C2C22AD7EFAFA532891B5311D4D6DFE2E32B06C96F415
changed only reverse carrier32k->48.012kHz (ARR1332); release-s/
thinLTO/codegen1, text128648/data1200/bss17660, four ISR-root math
audits PASS. Disabled guard3/18, roledu1006/6 period1333,
duty6/6 PASS. First powered run ACKed40%, held18s, ACKed45%, held
about18s with no electrical stop/foldback, then intentional HostAbort9
`off`; capture `captures/reverse_direction_2026-09-19_48k_hold45.txt`.
Second run ACKed40%, held9s, ACKed45%, held5s, then tripped the
unchanged fast5%/three-scan bus guard reason26 at last ACKed46.5%,
before47/50. No current foldback, tracking or nFAULT. Its three
terminal normalized bus codes were93.24/94.21/92.58%; `SAGPWM`
stamp43500632us TIM1 CNT966 ARR1332 CCR619 and ADC aperture offsets
project bus PWM counts807,608,409 (OFF, near compare, mid ON), again
not one PWM-edge notch. Capture
`captures/reverse_direction_2026-09-19_48k_sag_pwm465.txt`.
One run/carrier cannot establish a statistical duty boundary: this
does not prove that48k reliably moves the32k44.5% stop, and45%
has not passed lean/repeated qualification. 50% remains unreached
with the replacement motor. Source/firmware timing or electrical
current-demand transient remains open; no PSU/hardware attribution.
No further powered run was made after the46.5% stop. See
`FAST_SAG_COHORT_ANALYSIS.md` and `DUTY_50_CAMPAIGN.md`.

2026-09-19 FAST-SAG PWM PHASE — CURRENT board exact known reverse32k
lean SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988,
explicit G071 flash/verify/reset, p/i all gates/ENABLE/MOE/CCRs0,
nFAULT1, COM41 closed. Opt-in `bench-fast-sag-causal` diagnostic now
adds one terminal-only `SAGPWM` timestamp+TIM1 CNT/ARR/CCRs/CR1
snapshot to its frozen8-scan ring; no ordinary-scan timer reads or
electrical guard changes. Diagnostic SHA47ECC83AEA6B42D35388EDEEAE835C3502C086D101D17EBF9EA5155EEE26A756,
release-s/thinLTO/codegen1, text128600/data1200/bss17660, four
M0 ISR-root math audits PASS; disabled guard3/18, roledu1006/6,
duty6/6 PASS. A30s control ended clean at30% only because host
command timing missed the window; do not call it40 control. Corrected
90s-window run ACKed40%, held18s clean, then 0.5%-stepped to last
ACKed44.5%; terminal reason26 fast5%/three-scan bus stop before45
ACK. The three final low-bus scans (`acquired_us`48361258/61484/61710)
normalized to resting bus/VREF are92.58/93.03/92.97%; VREF1505..1507
stable. `SAGPWM` stamp48361796us TIM1 CNT1284 ARR1999 CCR890 CR1=129
(upcount). At16MHz ADC, physical bus channel aperture is nominally
trigger+42.46875us. Back-projecting the three trigger stamps at64MHz
gives bus PWM counts1570,34,498: OFF, early ON, mid ON. Clock mapping
and aperture uncertainty cannot make these three the **same PWM-edge
notch**. The bus was already low across452us and three distinct PWM
phases; stop followed the first estimated low bus aperture by~512us.
Accepted event gaps immediately before/on the low scans remained
~81..106us, COM progressed476301->476304->476306, no held sector or
obvious missed edge. One older136us interval existed but the bus
recovered to98.25% in the intervening scan; it is not a proven onset.
No foldback, tracking or nFAULT stop. Do NOT interpret this as proof
of PSU, motor, wiring or controller cause; it only rules out the
single-PWM-edge explanation for the three-scan collapse. Retain
`captures/reverse_direction_2026-09-19_32k_sag_pwm445.txt`; 45/50
lean holds still unqualified. See `FAST_SAG_COHORT_ANALYSIS.md`.

2026-09-19 FIRST-PEAK TIMING: CURRENT board again exact reverse32k lean
SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988,
flashed/verified/reset on explicit G071 device0x460, p/i all six gates,
ENABLE, MOE, CCRs0, nFAULT1; COM41 closed. Corrected live-transfer
sequence (`drivex1`, `driveobs1`, `live1`) passed a protected12s low-duty
control. One diagnostic32k climb (SHA3769F5A...) ACKed40%, dwelled9s,
then stopped reason27 at last ACKed42.5% (no further duty sent): physical
IA/IB/IC=2846/2056/24, bus/VREF=1136/1510 against start1211/1507
(93.62% normalized), fast-bus three-scan guard not tripped (max low95
streak2), no foldback/tracking/nFAULT. `PEAKTIME` acquisition47780262us,
service47780337us, TIM1 CNT1011/ARR1999/CCR850, last accepted event
47780260us, guard sector5/core step6 at service. Back-projecting the
75us service gap at64MHz gives TIM1 CNT~211 at trigger; ADC0=physical
IC is first, with configured160.5-cycle sample at16MHz ending near
CNT853, only3 timer ticks (~47ns) after CCR850. Timestamp mapping,
trigger launch, register-read and aperture uncertainty prevent literal
47ns precision, but this one peak is **consistent with a PWM edge
sample**. C is a conducting sink in both reverse steps5 and6, not a
proven floating-phase miswire. The bus channel follows IC by three
~10.81us conversions; its nominal aperture ends near PWM CNT929,
~1.2us after the **next** compare edge. Thus this frame's93.62% bus
code may also be a switching-phase notch, not a sustained collapse.
At42.5%, 32k ON=13.28us; 48k ON=8.85us,
shorter than the ADC's10.03us sample window, so a48k carrier is not
an uncomplicated current-sensing cure. This diagnostic first-peak
stop is NOT a qualified42.5% ceiling, nor does one edge-aligned sample
explain the three-scan45% bus collapse. Frozen capture:
`captures/reverse_direction_2026-09-19_32k_peak_timing_425.txt`.
No 45/50 lean qualification; next experiment should correlate peak
incidence with PWM aperture/commutation timing without removing the
independent bus/current/nFAULT/tracking protection.

2026-09-19 PEAK-TIMING PROBE STAGED, NOT QUALIFIED. A new opt-in
`bench-phase-peak-stop` first-stop snapshot records DMA acquisition/service
timestamps, TIM1 CNT/ARR/CCRs, TIM17 CNT, controller step/interval, guard
sector/event and commit count before outputs are revoked. This is
service-time context: the five ADC channels were converted sequentially,
so it does not itself assign one step or PWM phase to every aperture.
Frozen reverse32k diagnostic SHA3769F5AC6202FD0B0872BC14383B457D735F262B838CD63DED9C6C71EF01D260
passes release-s/thinLTO/codegen1 and four ISR-root math audits; disabled
guard3/18, roledu1006/6 and duty6/6 PASS. Two attempted setup runs produced
NO usable peak data: the first refused before drive because `avgnominal`
was not set; the second was incorrectly armed with `engage1` rather than
the live `drivex1`/`driveobs1` transfer path. The latter gave a normal
startup banner, then UART went silent while the run was active. `off`
was sent without ACK. An explicit G071 SWD hard reset restored UART;
post-reset p/i read all gates/ENABLE/MOE/CCRs0 and nFAULT1. Exact known
reverse32k lean SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
was then reflashed/verified/reset; p/i again gates/ENABLE/MOE/CCRs0,
nFAULT1, COM41 closed. These were setup failures, followed by the
correctly armed bounded diagnostic above. Do not infer a carrier or
hardware ceiling from either setup failure. See
`captures/reference/reverse_32k_peak_timing_20260919/README.md`.

2026-09-19 FIRST-PHASE-PEAK / 48k CARRIER A/B; CURRENT BOARD EXACT
REVERSE32k LEAN SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988,
verified/OFF/COM41 CLOSED. After explicit G071 device0x460 restore,
p/i read all gates/ENABLE/MOE/CCRs0, nFAULT1. Do not edit sibling rm32.
Opt-in diagnostic `bench-phase-peak-stop` retains the first physical
IA/IB/IC/bus/VREF coherent DMA frame displaced >=1900 raw from the
same-wake zero and immediately stops reason27; all original fast5%/
three-scan bus, active4A signed-current foldback/stop, nFAULT,
tracking and watchdog protections remain. It is NOT a calibrated SOA
guard or a lean envelope criterion. Reverse32k peak image SHA7B2A67A1...
passed10%/15s and40%/60s with zero >=1500 phase samples at40; one
gradual climb stopped reason27 at last ACKed43.0%, before43.5:
IA/IB/IC=2047/1094/4087, bus/VREF=1159/1505 vs start1210/1505
(~95.8%); no exact rail, fastbus, foldback, tracking or nFAULT.
Capture `captures/reverse_direction_2026-09-19_32k_peakstop43.txt`.

48.012kHz timer ARR1332 was added as a one-variable carrier A/B, with
exact phase-bin and226us ADC coverage host tests7/7 PASS. The first
48k live images stopped HostAbort on the first duty request because
`live_duty::Prepared::new` omitted valid1333-tick geometry. Fixed and
host-tested4/4; disabled hardware `livedutycheck`24/24 PASS. A second
UART issue was classified: a full115200-baud `du150` burst produced
active RX error0x300; pacing six bytes over~148ms (<250ms parser
timeout) ACKed. Do not call either abort a motor/carrier limit.
Final 48k diagnostic SHA4D286156EE9E3A1E76FC6AEBC267ED6D10F29E07B97D8BFC0BF1E9D0D50164E4
passed disabled guard3/18, roledu1006/6, duty6/6, liveduty24/24,
10% startup and40%/60s protected control (52.03s after ACK40), zero
>=1500 phase samples at40, no electrical stop. One paced climb stopped
reason27 at last ACKed43.0%, before43.5: IA/IB/IC=1977/4019/1270,
bus/VREF=1188/1506 vs start1210/1507 (~98.2%), average185 half-us
(~1801eHz); no exact rail, fastbus, foldback, tracking or nFAULT.
48k did not move this first-peak stop beyond32k in one trial each;
it does NOT prove identical peak rates or a hardware ceiling. Capture
`captures/reverse_direction_2026-09-19_48k_peakstop43.txt` and
`FAST_SAG_COHORT_ANALYSIS.md`. Both diagnostic images retired; 50%
remains unqualified. Next useful causal step is align near-rail ADC
aperture to PWM/COM phase and characterize rates, not loosen stops.

2026-09-19 PHASE-CURRENT/BUS RATE DISCRIMINATOR RUN; EXACT REVERSE32k
LEAN RESTORED/OFF/UART CLOSED. Diagnostic SHA E62E4D4E2F7145081305578DD91488A43E022CF7A913B19AD20E495F66263E00
was flashed/verified on explicit G071, disabled guard3/18, roledu1006/6,
duty6/6 and p/i off/nFAULT high PASS. Ordinary10%/15s protected gate PASS;
40%/60s control PASS with 50.45s ACKed-40 census: 223220 DMA scans,
zero phase displacement >=1500 raw, 1418 isolated bus<95% scans,
max low streak2, no fastbus/current foldback/tracking/nFAULT. A second
gradual 10->40, 10s dwell, then 0.5%/1.5s climb did NOT reach45:
last ACKed duty43.5%, then POWERPATH11/ADCFAULT stage30/DMAFAULT code10
at powered35.259715s. 28.00s census since 40% ACK: 123903 scans,
five phase displacement >=1500, four >=1900 (IA4/IB2/IC0), one
exact ADC rail; four >=1900 cooccurred with bus<97% and two with
bus<95%. Fastbus did NOT trip, bus<95 max streak1, no foldback or
nFAULT. Source: DMA code10 is failed `Latest::publish`; it can reject
an exact rail as Invalid, but this image did not retain the subtype,
so the coincidence with exact_rail=1 is strong, not unique proof.
Do NOT classify this as a 45% fastbus result or clean 43.5% hold.
All outputs safe/nFAULT high after stop; diagnostic archived at
`captures/reference/reverse_32k_phase_census_20260919/`, transcript
`captures/reverse_direction_2026-09-19_32k_phasecensus.txt`. Exact
lean SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
then re-flashed/verified/reset on G071 device0x460; p/i all gates,
ENABLE, MOE, CCRs0/nFAULT1 and COM41 closed. Next: offline investigate
rail-frame subtype/phase-current-vs-PWM aperture and protect motor;
do not repeat high-duty run without a causal change. Cohort aperture
analysis is `FAST_SAG_COHORT_ANALYSIS.md`: `adc_stream::poll()` already
remaps hardware ADC0/1/4 to physical IA/IB/IC, so archived labels are
correct and the false prior erratum is explicitly retracted. At the
derived bus aperture (~scan origin+43us), the A/B interval spanning
last near-normal to first <95% bus samples contained only accepted
gaps90/77/93us and92/101/72us respectively; C's earliest low bus is
outside the retained event ring. No common missing-edge precursor in
these windows; physical onset and phase angle remain unresolved.
The diagnostic is release-s/thinLTO/codegen1, text126856/data1200/
bss17236; four exact motor ISR-root soft-math audits and two pure
current-bin tests passed. Same protected fixed20 reverse32k control,
report-only IA/IB/IC near-rail counts, no live UART during hold.
Do not conflate diagnostic results with lean qualification or edit
sibling rm32.

2026-09-19 THREE-RUN PRE-TRIGGER SCAN-RING COHORT; EXACT LEAN RESTORED
(CURRENT BOARD). Reverse32k fixed20 lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
is installed/OFF, COM41 closed after explicit G071 device0x460 flash
verify/reset; p/i all gates/ENABLE/MOE/CCRs0,nFAULT1. No powered run
after restore. Opt-in `bench-fast-sag-causal` V3 SHA
E33CBCA1A57E34D08DFA55F6EF7D8B4DC001BDDD887FC6AA277A44850E03D9BC
adds a bounded eight-scan DMA raw-ADC ring, frozen at the third low scan,
to V2's three low rows and two accepted-event timestamp windows. Same
5%/three-scan fast-bus, 4A signed-current actuator, nFAULT, tracking and
watchdog decisions; no new control authority. Release-s/thinLTO/codegen1,
text128060/data1200/bss17632; four exact motor ISR-root soft-arithmetic
audits PASS; disassembly shows no helper on new normal-scan path.
Disabled guard3/18, roledu1006/6, duty6/6 and p/i off PASS. Ordinary
10%/15s and40%/60s protected gates PASS; 40% POWERPATH2, no fastbus,
foldback, tracking or nFAULT.

Same-image gradual ramps each ACKed45%; all THREE stopped `POWERPATH26`
on fast 5% sag at powered40.643372/38.310908/39.433587s. This is a
0/3 diagnostic 45% hold cohort, not a lean qualification or 50% result.
All safed; no current foldback/tracking/nFAULT. Eight preceding coherent
bus samples as percentages of fixed raw start baseline (VREF varies <1%):
A=98.4,96.6,96.6,96.9,98.9,93.3,93.9,93.1;
B=100.1,97.2,97.4,98.4,98.6,92.9,94.1,92.1;
C=93.6,96.0,93.1,93.3,95.5,92.2,90.9,89.5.
Thus A/B had several3-4% notches before the terminal three-scan dip;
C had intermittent >5% dips already1.8ms before final trip. Accepted
events continued through each sampled dip; A/C have one~138-142us
interval just AFTER first low, B has no comparable late gap there.
There is no common missing-edge/held-sector precursor in these three
short windows. Bus conversion aperture is later than logged scan origin,
so strict physical onset order is still unproven. First-low sectors are
3/5/5: too little evidence for phase-specific attribution.
Live 45% ACK controller averages were176/179/179 half-us (about
1894/1862/1862eHz); at the later fastbus stops they were195/190/203
(1709/1754/1642eHz). Each run slowed under constant45% before the
terminal readback. This is a control/load deterioration signature,
not proof whether sag caused slowing or slowing increased load.

CORRECTION to the last-turn archive erratum: frozen V2/V3 `ia/ib/ic`
WERE physical IA/IB/IC. Although hardware DMA scans ADC0/1/4, the
return from `adc_stream::poll` is `dma_snapshot::logical(raw)`, which
remaps to [ADC4,ADC1,ADC0,bus,VREF] BEFORE `powered_timer` and the
sag recorder receive it. The prior claim that archived IA/IC were
swapped was false; do NOT swap the captured phase labels. The mistaken
source swap was built offline as SHA9BEE913C... but NEVER flashed and
has now been reverted. Only the benign post-run static
`fault_triggered=1` label fix remains in source, also unflashed. Raw
codes, bus data, timestamps and the frozen V3 motor evidence stand.
Physical role map for reverse `core_step`1..6 is respectively
B+/A-/Cfloat, C+/A-/Bfloat, C+/B-/Afloat, A+/B-/Cfloat,
A+/C-/Bfloat, B+/C-/Afloat. However `core_step` in SAGROW is read at
DMA SERVICE, while phase channels are sampled sequentially before it;
at ADC PCLK/4=16MHz and SMP160.5+12.5 conversion cycles the nominal
physical IA/IB/IC channel apertures are about trigger+32/+21/+10us
and bus is about+43us, excluding trigger overhead.
A COM may occur between those samples and the service snapshot.
Thus the archived rows do not prove a hot physical floating phase or
phase miswire, even when a mapped raw value appears in that role.

Side-review correction: `powered_guard::phase_valid` under
`bench-average-current` deliberately accepts all12-bit phase samples;
`EXPLOREPROTECTION pulse_stop=0 phase_rail_stop=0` reports this. The old
E765 nominal1.3V pulse clamp is NOT active in the current image; it was
retired because asynchronous PWM peak samples were false current stops.
Near-rail raw values at45% are real evidence of large sampled shunt
excursions, but one sample does not establish sustained DC-link current
or safe peak current. On C's terminal scan IB=0, `scan_raw` fastbus
short-circuited BEFORE the separate phase-rail counter, explaining its
reported zero; that counter does not exonerate the frame. Fastbus safely
disabled in that same wake. Do not claim a hidden clamp failure, nor
quietly restore the old tuned threshold without a calibrated SOA policy.
Side-review 48k recommendation is untested; 40k was ALREADY tried and
still reason26 at45%, and32->48k shortens fixed-duty ON time by one
third, not one half. No further carrier-only climb from the20/24/32/40k
trend. Read full captures `captures/reverse_direction_2026-09-19_32k_scanring45[a-c]_fastbus.txt`.
Next work is offline current/phase-role and control-vs-load analysis,
then a bounded causal change with existing fastbus stop retained; do not
repeat45% merely to fish for a pass. Do not edit sibling rm32.

2026-09-19 V2 CAUSAL EVENT-RING 45% FAULT; EXACT LEAN RESTORED
(CURRENT BOARD). Reverse32k fixed20 lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
is installed/OFF, COM41 closed after explicit G071 device0x460 flash
verify/reset; p/i all gates/ENABLE/MOE/CCRs0,nFAULT1. No powered run
after restore. Opt-in `bench-fast-sag-causal` V2 SHA
FB6BC15A4574430B8C4D443FB5ADEC8F010E41B29495E0A9B25382FF6689A179
adds an eight-accepted-event timestamp ring and freezes it on first and
third low scans; protection unchanged. Release-s/thinLTO/codegen1,
text127100/data1200/bss17428; four motor ISR-root M0 audits pass.
Disabled guard3/18, roledu1006/6,duty6/6, p/i off pass. Ordinary
10%/15s and40%/60s protected gates passed. The 40% run included one
isolated bus sample at about92% of start reference, correctly ignored
by the unchanged three-scan guard. A gradual ramp ACKed45%, then
FASTBUS reason26 at powered44.574632s; this is NOT a45% clean hold.
Three acquired_us scans44,574,102/328/554 had bus/VREF
1132/1507,1132/1504,1134/1510 vs baseline1211/1507. Current foldback,
phase rails, absolute-bus-low codes, tracking and nFAULT stayed clear.
Accepted-event timestamps in the roughly1ms before and across the dip
are chronological, with no conspicuous absent-edge/held-sector gap;
this falsifies that *simple local* explanation, not a preceding phase-
angle or control problem. `acquired_us` is scan origin, not the bus ADC
aperture, and controller snapshots occur at later DMA service; do not
claim physical onset ordering from the rows. Full normalized transcript
`captures/reverse_direction_2026-09-19_32k_eventring45_decoded.txt`.
The side review's eight *preceding bus scans* suggestion is still open;
three consecutive low rows plus two event windows are not that ring.
Do not turn one stochastic fault into a universal A/B verdict. Next
diagnostic should retain a minimal pre-trigger bus-scan history and
measure its ISR cost before a small repeated45% cohort; compare the
fault sequence and sector distribution, not merely pass/fail. Do not
edit sibling rm32.

2026-09-19 FAULT-TRIGGERED SAG/CONTROLLER PROBE; EXACT LEAN RESTORED
(CURRENT BOARD). Reverse32k fixed20 lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
is installed/OFF, COM41 closed after G071 device0x460 flash verify/reset;
p/i all gates/ENABLE/MOE/CCRs0,nFAULT1. No powered run after restore.
Feature-gated `bench-fast-sag-causal` image SHA
70CF7D0485025DF1DFB717C1E33749A596EB9A720201FB621C0918D90C080B13
adds only three last-consecutive-low-scan rows; no change to fastbus stop
or any current/nFAULT/tracking/watchdog protection. Release-s/thinLTO/
codegen1, text126116/data1200/bss17328; four motor ISR-root M0 audits,
disabled guard3/18, roledu1006/6, duty6/6 and p/i off pass.
Ordinary10%/15s and40%/60s powered gates completed on deadline,
with isolated low-scan rows and no electrical trip. A stepped run ACKed
44% then FASTBUS reason26 at powered47.139408s; host du445 arrived
AFTER shutdown and was refused, so44.5/45 were NOT reached. All three
low scans were captured at acquired_us47,138,869 /47,139,095 /
47,139,321; bus/VREF1146/1508,1115/1505,1128/1500 against start
1207/1504. COM counts441040/441042/441044 and recent measured intervals
188/194/175 half-us show control still advancing during the sampled
collapse; no current foldback, rail, tracking or nFAULT. At DMA SERVICE
times47,138,952/47,139,185/47,139,398, last accepted events were
47,138,894/47,139,164/47,139,379 -- all AFTER the corresponding
trigger-origin `acquired_us`. Therefore these rows do NOT prove the
event state at bus CONVERSION time or the physical onset order. This
fault has no obvious held-sector signature in the snapshot, but cause
remains unresolved. Full captures `captures/reverse_direction_2026-09-19_32k_causal10_pass.txt`,
`...causal40_pass.txt`, `...causal44_fastbus.txt`.
Do not treat this as a45% attempt or qualify44%; no further motor run
until a narrow control/analog timing discriminator is ready. The source
probe remains opt-in; lean image is installed safe. Do not edit rm32.

2026-09-19 PRIORITY-ONLY RECHECK FAILED LOW GATE; EXACT LEAN RESTORED
(CURRENT BOARD). Reverse32k fixed20 lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
is installed/OFF, COM41 closed after G071 device0x460 flash verify/reset;
p/i all gates/ENABLE/MOE/CCRs0,nFAULT1. No powered run after restore.
One-variable `bench-dma-below-comp` SHA
53B47947B118AAB06A03FAD9600BCF86A1EC473240104A894446BE924B99F23B
changed only DMA priority64->128; original100us lease and all electrical
stops retained. Release-s/thinLTO/codegen1; text124760/data1200/bss17144;
four motor ISR-root arithmetic audits and disabled guard3/18,
roledu1006/6,duty6/6,p/i off pass. Its first10%/15s protected gate
stopped after powered5.114ms on `POWERPATH reason11`, `DMAFAULT code5`
flags3/NDTR5/service_gap432us, only8 COM, no fastbus/current/tracking/
nFAULT trip. Postrun outputs off/nFAULT1; no high-duty run. This is the
same class as prior E895, which already tested priority-only and failed
DMA code7 at6.808ms; this recheck should not have been necessary.
Do not retry DMA-below-COMP or COMP-top (E896 TickGap low-gate failure)
as a casual lever. Capture
`captures/reverse_direction_2026-09-19_32k_dma_below_only_adcfault.txt`.
Offline CRC-valid IT87/BS85 re-read from the44.5% fault gives a127us
accepted-event gap ending at36,773,641us, then a bus scan at36,773,703us
still97.46% of reference, then the first sampled sub95% bus scan at
36,773,929us. A152us gap ended earlier at36,772,678us, followed by a
97.21% scan at36,772,799us. Thus timing excursions precede *sampled*
severe sag, but226us ADC spacing cannot prove physical onset ordering or
the cause. Avoid more single-run priority lotteries; next instrument
must resolve that ordering with bounded overhead and retained guards.

2026-09-19 RATE-CENSUS CAMPAIGN COMPLETE; EXACT LEAN RESTORED (CURRENT BOARD).
Reverse32k fixed20 lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
is installed/OFF, COM41 closed after explicit G071 device0x460 flash verify/reset.
Post-reset p/i: all AH/BH/CH/AL/BL/CL, ENABLE, MOE, CCRs0; nFAULT1.
No powered run after restoration. Opt-in rate-census image SHA
DBE640973129E056AD76AAF3C62B3FBB70E111969252C1A958F2E58F80EC89AC
was built release-s/thinLTO/codegen1, four motor ISR-root M0 arithmetic audits
PASS; disabled guard3/18, roledu1006/6, duty6/6, p/i off/nFAULT1 PASS.
Ordinary10%/15s protected gate PASS, census inactive as intended. One40%
70s run PASS: at/after40 ACK census61.069s,270217 bus scans,
low97=5127,low95=467, all95 lows isolated (max streak1),
late125=51176/629117 accepted, no foldback/fastbus/tracking/nFAULT.
One40->42.5%70s run PASS (not qualification): census59.606s,
low97=7524,low95=1315,max95 streak2,late125=52018/623265 accepted.
One40->45% run ACKed45 then FASTBUS reason26 at60.819517s powered:
census47.790s,low97=3138,low95=97,low95 runs93,max streak3,
late125=42486/502397 accepted; no current foldback, rails, absolute-bus,
tracking or nFAULT. Final p/i off/nFAULT1. Captures
`captures/reverse_direction_2026-09-19_32k_rate40_70s_pass.txt`,
`...rate425_70s_pass.txt`, `...rate45_fastbus.txt`. The faulted run had
FEWER low-bus samples than both clean runs but one decisive cluster;
the >25%-late accepted-event fraction stayed ~8.1-8.5%. Total notch
rate and this coarse late counter therefore do not predict the trip.
This is an n=1/campaign-scope observation, not proof of the initiating
mechanism or a new45% ceiling. Keep fast bus guard; do not qualify42.5/45
or attempt50 on this evidence. Next instrument should freeze a minimal
time-aligned near-fault sequence at the FIRST 95%-low scan and accepted
gap, or use a bounded reference comparison, not repeat pass/fail runs.

2026-09-19 SPARSE CPU PROBE COMPLETE; EXACT LEAN RESTORED (historical state).
The only board image now installed is reverse32k fixed20 lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988,
OFF/COM41 closed after flash verify and explicit G071 reset (device0x460).
Post-reset p/i show all gates, ENABLE, MOE, CCRs0 and nFAULT1. No powered
run after this restoration. Sparse diagnostic SHA
FCC62B4BA7FBC7B8A83D93672AD603F214B4B92AEFD696F304876F5A6F27B523
gates four ISR timestamp probes below ACKed35%, samples every17th call,
and leaves active4A current/fast5%-three-scan bus/absolute bus/nFAULT/
tracking/watchdog protection intact. Four M0 ISR-root math audits pass.
At ACKed35%, it completed30s on deadline (POWERPATH2), no electrical
fault or foldback; at ACKed40%, fast bus sag stopped POWERPATH26 at
32.716492s powered, roughly20s after first sparse sample, no foldback,
rails, tracking or nFAULT. Final p/i off/nFAULT1. Sparse inclusive
per-root estimates are ~73% at35 and ~75% at40; they double-count nested
preemption and include probe overhead, so are not exact lean CPU occupancy.
The 40% sparse failure cannot invalidate the separate lean40% clean hold,
but it makes this diagnostic unsuitable for envelope qualification.
Captures `captures/reverse_direction_2026-09-19_32k_cpu_sparse35_pass.txt`
and `...cpu_sparse40_fastbus.txt`. Next causal work should use cheap event-
rate counters at a clean40% point (low-bus scan rates and late-accept rates),
not n=1 pass/fail A/Bs at stochastic42.5%. The high-speed revisit-off35
A/B was already run earlier and failed Tracking8; do not call it untested.
Fastbus proves a sampled short bus dip, not PSU CC or physical cause.

2026-09-19 DMA-PRIORITY/LEASE A/B RETIRED; EXACT LEAN RESTORED (CURRENT BOARD).
Reverse32k fixed20 lean SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
is again installed/OFF, COM41 open. G071 flash verify, Cube reset
device0x460, disabled guard3/18, reverse roledu100 6/6 ARR1999,
dutycheck6/6 and p/i gates/ENABLE/MOE/CCRs0,nFAULT1 pass; no powered
run after restore. Candidate SHA50858F55... lowered DMA priority below
COMP/COM and allowed its226us-cadence snapshot lease200us (hardware
flags/NDTR/epoch checks unchanged); host lease tests5/5 and four M0
ISR-root audits passed. First10%/15s protected BEMF gate reached
deadline with no ADC/electrical fault. Second ordinary startup stopped
before first15% live ACK. Host displayed only the first1000 bytes and
lost the terminal reason: classify this as UNCLASSIFIED lower-gate
failure, not a pass. No upper-duty A/B was run; candidate retired
without a retry. Its experimental source changes were removed after
archiving the exact candidate ELF; current source keeps the original
100us DMA lease. Capture and limitation:
`captures/reverse_direction_2026-09-19_32k_dma_lease200_lower_gate.txt`.
For the next MCP live run, retain and emit the FULL serial read before
testing an ACK; never slice/truncate an unexpected reply in the host
script. Neither this
candidate nor the slow-ramp test below qualifies42.5/43/45/50 reverse.

2026-09-19 REVERSE32k SLOW-STEP A/B FAILED AT43% (CURRENT BOARD).
Exact fixed20 lean SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
remains installed/OFF, COM41 open. With all existing electrical stops
active, ordinary startup and live control ACKed40%, followed by20s quiet
dwell. Half-percent steps through42.5% each had5s quiet dwell;42.5 then
had a further25s quiet dwell. On ACKed43%, the fast normalized bus guard
stopped reason26 at powered118.546712s, with zero current foldbacks,
phase rails, absolute bus-low codes, tracking faults or nFAULT; final
estimate~1718eHz is fault-time. Postrun p/i all gates/ENABLE/MOE/CCRs0,
nFAULT1. Capture `captures/reverse_direction_2026-09-19_32k_fixed20_slow43_fastbus.txt`.
This is one slower-ramp negative A/B, not proof that the43% command caused
the collapse: the same image previously tripped at42.5 after a faster
ramp and once ran42.5 clean. 42.5 remains unqualified,43 failed,45/50
reverse unreached as clean holds. Do not repeat this profile to fish for
a pass or relax the fast bus/current guards. Next powered work needs a
different, causal control/scheduling variable or bounded diagnostic.

2026-09-19 OPERATOR RESUMED; FIXED20 REPEAT FAILED AT42.5 (CURRENT BOARD).
The pause below was lifted. Exact reverse32k fixed20 lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
remains installed/OFF, COM41 open. MCP reachable, pre/post p/i all
gates/ENABLE/MOE/CCRs0,nFAULT1. With active4A signed-current actuator,
fast5%-three-scan and absolute bus, nFAULT/tracking/IRQ/watchdog guards,
ordinary startup and smooth live ramp ACKed42.5%; before planned30s
dwell/45 climb completed, POWERPATH reason26 stopped at41.834987s powered.
FASTBUS tripped1 baseline bus/VREF1212/1506, scan226us; current foldback0,
phase rail0, absolute-bus-low0, tracking event fault0, nFAULT1.
Final estimate~1727eHz is fault-time, not steady performance. No43-45
commands and no repeat. This is a SAME-IMAGE failed42.5 repeat after one
clean42.5 hold: 42.5 is NOT qualified/reliably clean. Prior paused45 ACK
remains unclassified; do not promote it to pass. 50% reverse unattempted.
Capture `captures/reverse_direction_2026-09-19_32k_fixed20_425_repeat_fastbus.txt`.
Next work must explain/control the intermittent timing/load collapse, not
ratchet bus/current thresholds or supply amps. No old-direction/sibling
rm32 edits.

2026-09-19 OPERATOR PAUSE (CURRENT BOARD). On exact fixed20 reverse32k
lean SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988,
a normal-start protected run ACKed42.5%, had one quiet30s serial dwell,
then smoothly ACKed43/43.5/44/44.5/45%. During the long45% read, the
operator said "pause for now". The read was terminated, and NO terminal
summary was retained: this is NOT a45% qualification or classified fault.
A one-byte host abort request was sent, then explicit `off`; p/i verified
all gates/ENABLE/MOE/CCRs0 and nFAULT1. COM41 closed. Do not issue powered
commands or flash until the operator resumes. Retained command/stop record:
`captures/reverse_direction_2026-09-19_32k_fixed20_slow45_operator_pause.txt`.
Prior fixed20 clean42.5% run remains a single exploratory pass; 50% reverse
is unattempted. Do not touch sibling rm32 or run old-direction images.

2026-09-19 FIXED18 ADVANCE A/B NEGATIVE; SAFE RESTORE (CURRENT BOARD).
Only `bench-advance-20` -> `bench-advance-18` changed from exact
reverse32k lean feature fingerprint. Candidate SHA1B43148BE87CBD8F0E665169D6B68F28B524BC10FEA8125E679530D57722F3AC
release-hybrid opt-s/thinLTO/codegen1, four motor ISR-root arithmetic
audits PASS. Explicit G071 flash verify/reset, disabled guard3/18,
reverse role6/6 ARR1999, roledu100 6/6, duty6/6 and p/i off passed.
Ordinary10% startup/handoff and15s BEMF hold passed deadline with no
electrical trip. The first smooth high-duty ramp ACKed42.5%, then the
ACTIVE fast relative bus guard stopped POWERPATH reason26 at35.148728s
powered. FASTBUS baseline1211/1506, tripped1; current foldback0,
phase rail0, absolute-bus-low0, tracking event fault0, nFAULT1;
postrun p/i all gates/ENABLE/MOE/CCRs0. No45/50 attempt on fixed18.
One fixed18 fault versus one fixed20 clean42.5% hold is not a statistical
boundary but fixed18 is NOT an obvious improvement; candidate retired.
Capture `captures/reverse_direction_2026-09-19_32k_fixed18_425_fastbus.txt`.
Exact previously clean fixed20 reverse32k lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
reflashed/verified/reset on G071, disabled guard3/18, role6/6,
roledu100 6/6, duty6/6 and p/i off/nFAULT1 PASS. Installed/OFF,
COM41 open, no powered run after restore. 42.5% is one clean fixed20
exploratory hold;45% still fails,50% reverse unattempted. No old-direction
or sibling rm32 changes.

2026-09-19 REVERSE32k FIXED20 LEAN MIDPOINT (CURRENT BOARD): exact SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988,
installed/OFF, COM41 open. After flash verification/reset and disabled
guard3/18, reverse role6/6 ARR1999, roledu100 6/6, duty6/6, p/i OFF,
one ordinary-start protected 60s BEMF window smoothly ACKed42.5% and
completed on planned deadline. Exact42.5 dwell was not MCU-timestamped;
approximately40-45s by host ramp. POWERPATH reason0, BEMF core_stop1,
537902 COM, final estimate~1792eHz, POWERFEEDBACK busmin11.832V,
FASTBUS0, current foldback0, phase rail0, bus-low0, tracking event fault0,
nFAULT1. Final p/i all gates/ENABLE/MOE/CCRs0. This is a SINGLE clean
exploratory midpoint, not repeated qualification or a matched-speed
current/thermal pass. Capture
`captures/reverse_direction_2026-09-19_32k_lean425_60s.txt`.
45% still fails fast bus guard on several guarded images, including
advance22-high; 50% reverse unattempted. Keep electrical protections.

2026-09-19 SAFE RESTORE after negative advance22-high A/B (CURRENT BOARD).
Exact reverse32k fixed-advance20 lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
reflashed with `probe-rs download --verify` on explicit G071 ST-Link SN,
then CubeProgrammer HWRSTPULSE reset identified device0x460/Cortex-M0+.
Disabled guard3/18, reverse role6/6 at ARR1999, roledu100 6/6,
dutycheck6/6 and p/i gates/ENABLE/MOE/CCRs0,nFAULT1 PASS. COM41 open;
no powered run after restore. Prior candidate's45% fast-bus fault retained
below. No old-direction image and no sibling rm32 edits. 40% has one clean
restored-lean70s powered window but 45/50% remain unqualified/unreached.

2026-09-19 REVERSE32k ADVANCE22-HIGH A/B NEGATIVE; CURRENT BOARD is the
candidate SHA6FAF3C395C860191AE7B2E0DC682F64B2EED7BCF5A2D17663F68C48026CBD08F,
OFF, COM41 open. Disabled guard3/18, reverse role6/6, duty6/6 passed after
flash; separate ordinary10% startup/handoff and15s BEMF hold passed planned
deadline with no electrical trip, POWERPATH reason2, 43884 COM, ~485eHz,
busmin11.545V. Next normal-start90s BEMF-window run ramped smoothly through
40% (approximately12s dwell) and ACKed45%, then stopped on the ACTIVE fast
normalized bus guard: POWERPATH reason26 at67.326891s powered, FASTBUS
tripped1 with baseline bus/VREF1210/1506 and three226us scans below95%.
Advance profile readback retained22 at/above35%; current foldback0,
phase rails0, absolute-bus-low codes0, tracking event fault0, nFAULT1.
Postrun p/i all gates/ENABLE/MOE/CCRs0. `DONE reason=8` is driven_run's
generic BEMF wrapper, not Tracking8; POWERPATH26 is the stop reason.
This does NOT cure the45% wall and does NOT qualify45; 50% reverse unattempted.
Retire the candidate, restore exact reverse32k fixed20 lean SHA06B1C1D6...
before another control test. Retained capture
`captures/reverse_direction_2026-09-19_32k_advance22_45_fastbus.txt`.
Old direction remains forbidden; do not edit sibling rm32.

2026-09-19 STAGED reverse32k high-duty advance22 A/B, NOT FLASHED at
freeze. Exact SHA6FAF3C395C860191AE7B2E0DC682F64B2EED7BCF5A2D17663F68C48026CBD08F
at `captures/reference/reverse_32k_advance22_20260919/`. Same reverse
lean carrier32k and active electrical protections as baseline; uses
existing minz-core scheduled advance with binz publishing level20 below
35%, level22 at/above35%, level20 at observation begin. Polling-mode
TEMP_ADVANCE=18 caveat, so not byte-identical startup. Host boundary1/1,
four ISR-root M0 math audits PASS. Current board remains exact reverse32k
fixed20 lean SHA06B1C1D6..., OFF, COM41 open after restored disabled
preflights. Do not flash or power candidate without disabled guard/role/
duty/off verification. First gate ordinary10% handoff; if it passes,
compare bounded40% behavior before45. No old-direction/sibling rm32 edits.

2026-09-19 COMP/DMA TOP-PEER A/B RETIRED; SAFE RESTORE (CURRENT BOARD).
Opt-in `bench-reverse-comp-dma-peer` image SHA8F3BD75A... changed COMP
priority64->0 while DMA and TIM6 guard remained0, COM64. Full electrical
stops, including TickGap, unchanged. Explicit-G071 flash verify and
disabled guard3/18, role6/6, duty6/6 passed. FIRST ordinary10% startup
stopped after4 COMs at powered2224us on reason3 TickGap; actual NVIC
postrun readback comp0/com64/guard0/dma0. Thus this peer layout is
not viable with present ISR timing and guard deadline; no high-duty
verdict and no retry. Final p/i all gates/ENABLE/MOE/CCRs0,nFAULT1.
Exact prior reverse32k lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
restored/verified/reset on G071; disabled guard3/18, role6/6, duty6/6,
p/i all outputs0/nFAULT1 PASS. COM41 open, board OFF. Capture
`captures/reverse_direction_2026-09-19_32k_comp_dma_peer_tickgap.txt`.
Both tested scheduling reprioritizations fail startup for distinct reasons
(DMA-below: ADC lease; COMP-peer: guard TickGap). Do not loosen guards
or call this45% root-cause proof. 50% reverse unreached; old direction
forbidden; sibling rm32 untouched.

2026-09-19 REVERSE40k CARRIER A/B NEGATIVE; SAFE RESTORE (historical).
Exact SHA22AEFD4C... flashed/verified on G071; disabled guard3/18,
role6/6 ARR1599, duty6/6 and p/i off/nFAULT1 PASS. First protected
10% BEMF15s window PASS with transfer1, no fastbus/foldback/rail/
tracking/nFAULT; this proves its short2.09us nominal post-deadtime
10% pulse can operate on this run, not a universal margin. Second
smooth ramp ACKed45%, but fast bus sag reason26 stopped at powered
39.998459s (host command timing suggests about3s at45, approximate),
busmin11.510V, no current foldback/rail/nFAULT/tracking stop, final off.
40k therefore did NOT remove the45% transient and was not climbed to50.
Captures: `captures/reverse_direction_2026-09-19_40k_10_gate.txt` and
`captures/reverse_direction_2026-09-19_40k_45_fastbus.txt`.
Original reverse32k lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
reflashed/verified/reset; disabled guard3/18, reverse role6/6, duty6/6,
p/i all outputs0/nFAULT1 PASS. COM41 open, board OFF. This is the current
installed image. Its separate70s powered run reaching40% ended on planned
deadline with no electrical guard trip. Keep active protections and old-
direction ban. No sibling rm32 edits.

2026-09-19 STAGED reverse40k carrier A/B, NOT FLASHED at freeze. Exact
SHA22AEFD4CCAB2BC42971409A6740F751A103DF4626ED28B4B7FBDE5BAD7EE776A
at `captures/reference/reverse_40k_20260919/`. One variable against
the32k lean control: BEMF carrier ARR1599/40kHz; forced startup10kHz,
same226us ADC cadence, full active4A current, fast5%-three-scan/absolute
bus, nFAULT/tracking/watchdog, reverse phase mapping and normal restart.
At10% BEMF duty ideal high after deadtime2.09us, so startup/transfer
must be measured first; no assumption that40k is better. Host carrier/
role24/24, live writer5/5, role/ADC11/11, four ISR-root M0 math audits
PASS. Current board is still exact reverse32k lean SHA06B1C1D6...,
OFF, COM41 open, disabled preflights passed and one70s powered window
reaching40% ended on deadline with busmin11.689V/no electrical faults.
First powered40k action only after flash verify/reset and disabled guard/
role/duty/off preflights. Retain first startup fault; no high-duty climb
until lower gate passes. Old direction forbidden; sibling rm32 untouched.

2026-09-19 DMA-BELOW A/B RETIRED; SAFE RESTORE (historical). A lean32k
image with only `bench-dma-below-comp` added, SHA11777BAC..., changed IRQ
priority from guard/DMA0, COMP/COM64 to guard0, COMP/COM64, DMA128. Full
electrical stops retained; disabled guard3/18, role6/6, duty6/6 passed.
Its FIRST ordinary10% startup stopped at powered4453us, reason11/ADC
stage27/DMA code7, `Lease::finish` refused a scan with service_gap237us
versus226us period and remaining2; only7 COMs. No high-duty verdict.
Poststop gates/ENABLE/MOE/CCRs0,nFAULT1. Candidate retired without retry.
Exact earlier reverse32k lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
restored/flash-verified/reset on explicit G071, disabled guard3/18,
role6/6, duty6/6, p/i all outputs0/nFAULT1 PASS. COM41 open, board OFF.
Capture `captures/reverse_direction_2026-09-19_32k_dma_below_startup_fail.txt`.
Do not use DMA-below image for another powered attempt; its ADC producer
lease cannot tolerate that priority arrangement as built. 44.5/45% wall
still under investigation; 50% reverse not reached. Old-direction ban.

2026-09-19 REVERSE 32k ALIGNED DIAGNOSTIC (historical image): SHA
AAE4E8FE6EE5EEC2C61D6EEE0617B282ED2BD3034CEDB9A9E1A1B4A71DEFBE05
installed, OFF, COM41 open after p/i outputs0/nFAULT1. The operator
resumed work; prior pause no longer applies. Added IT87 accepted-origin/
interval and BS85 16-scan tails to the32k fault-frame image; all existing
protections active. Four ISR-root math audit, decoder9/9 and disabled
guard3/18, role6/6, duty6/6 PASS. One normal-start reverse ramp ACKed44.5%;
45% was refused `!busy` after fast bus stop reason26 at powered36.774451s.
No current foldback, phase rail, nFAULT or tracking stop; final busmin11.534V.
CRC-valid tails: last21 complete electrical cycles grow545->612us (max612),
no event-watch violation. Software-only accepts become more frequent before
the first sampled bus dip: software-only@36.773641s, raw IA34/bus97.46%
@36.773703s, software-only@36.773916s, then bus93.50%@36.773929s;
the next two scans are93.38/94.66%. Phase raw values become extreme
(3092/959/3360 at first sub95% scan). The trip followed the first
observed low scan by522us and the third by70us. The226us cadence cannot
exclude an earlier narrow bus dip or prove software revisit caused the
load surge. This is observer-affected evidence of a timing/load disturbance
before the guard, not45% qualification. Capture:
`captures/reverse_direction_2026-09-19_32k_it87_bs85_445_fastbus.txt`.
Keep all protections and old-direction ban. Next sensible causal A/B is
32k lean with DMA IRQ below COMP/COM (`bench-dma-below-comp`) while TIM6
guard stays highest; DMA currently priority0 and can delay physical COMP
accepts. This is a hypothesis, not a proven cause. Do not edit sibling rm32.

2026-09-19 OPERATOR PAUSE: no further powered tests for the next few hours.
Current board is reverse-only32k fault-frame diagnostic SHA
9C23657592678C2FCDBBFEDE049CDC4A72C622661DBF0085BE31C494A2927B11,
installed/OFF/COM41 closed. Poststop p/i all gates/ENABLE/MOE/CCRs0,
nFAULT1. It differs from the prior32k lean image only by the opt-in
fault-only ADC frame snapshot; full electrical protections remain. Disabled
guard3/18, role6/6, duty6/6 passed. One normal-start protected ramp to45%
stopped at powered35.625135s on fast bus sag reason26, baseline raw1213,
threshold95%,3x226us scans; busmin11.498V, no current foldback, phase rail,
tracking or nFAULT. No `ADCFRAME` emitted because the fast-bus guard won;
the earlier isolated phase-rail sample therefore remains uncharacterized.
Capture: `captures/reverse_direction_2026-09-19_32k_faultframe_45_fastbus.txt`.
Do not run more tests until the operator explicitly resumes after the pause.
Desired reversed airflow has been visually confirmed. 40% was crossed,
45% failed twice by different guards, 50% reverse remains unreached.

2026-09-19 REVERSE 32k POWERED A/B (historical lean image): exact lean SHA
06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
installed on G071, OFF after tests, COM41 open. Operator CONFIRMED the
reversed airflow is the desired direction; never run old-direction images.
Disabled guard3/18, role6/6 at ARR1999, duty6/6 and p/i off passed.
First protected60s window reached37.5%, ended at programmed deadline with
no fast bus/current foldback/rail/tracking/nFAULT, busmin11.653V; not a
60s dwell at37.5. Second120s window ramped through40%,42.5%,45% and stopped
at powered87.389155s while45% requested: reason11/ADCFAULT stage30,
DMAFAULT code10, `CURRENTQUALITY phase_rail_codes=1`, busmin11.605V,
FASTBUS tripped0, current foldback0, tracking event_fault0, nFAULT high.
`Latest::publish` latches any raw channel at0/4095 and calls stream_fault;
the lean image did not retain the exact raw frame, so classify as a
fail-closed ADC validity stop, not a proven DMA failure or current limit.
Final p/i gates/ENABLE/MOE/CCRs0,nFAULT1. Retained capture at
`captures/reverse_direction_2026-09-19_32k_45_adcfault.txt`.
40% was observed but not a stand-alone qualified hold;45% failed and50%
reverse was not reached. Keep active electrical protections. Next diagnostic
should capture the raw fault frame at this limit without weakening the stop.

2026-09-19 STAGED reverse-only32k carrier A/B, NOT FLASHED at freeze.
Exact SHA06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988
at `captures/reference/reverse_32k_20260919/`; startup remains10k,
powered BEMF carrier32k/ARR1999 versus qualified24k/ARR2665. Same reverse
lean control/safety feature closure:4A current foldback/terminal, fast
5%-3-scan and absolute bus, nFAULT/tracking/watchdog/IRQ-rate, normal
restart. At10% BEMF entry the ideal ON-after-deadtime is2.72us, so first
gate is actual startup/handoff reliability, not presumed. 226us ADC cadence
has50 unique PWM phases/max1.25us uncovered arc per50-scan block; host
carrier5/5, live roles22/22, live writer5/5, role/ADC6/6 and four ISR-root
M0 soft-math audit PASS. The current installed board is still exact
reverse24k lean SHA2B4E/OFF, COM41 closed. Flash candidate only with
disabled guard/role/duty/off preflights. One bounded startup->35->40
exploration if earlier gates pass; stop and retain first electrical fault.
No old-direction firmware or sibling rm32 edits.

2026-09-18 REVERSE IT87/BS85 BOUNDED TEST + SAFE RESTORE (LATEST).
Combined origin/interval/bus diagnostic SHA BBCC4F564C5564ACA46CAB01FC755B82189B6322808237740C9B9DA056245024
flashed/verified on explicit G071; disabled guard3/18, reverse role6/6,
duty6/6, p/i OFF/nFAULT1 PASS. One ordinary start, 1%/250ms to35,
3s settle, 0.5%/500ms to ACKed40. Stopped at powered33.272317s on existing
ADC/DMA publication reason11, NOT fast-sag reason26; final frame had phase
raw IB4095/IC11, bus1125/VREF1507. Source audit: `adc_stream::interrupt`
calls `Latest::publish` after `stream_feedback`; the exact IB4095 rail makes
`Latest::publish` return Invalid and call `stream_fault(10)` (stage30), so
this is a fail-closed *raw-frame validity* stop, not evidence of a broken
DMA transaction. `CURRENTQUALITY phase_rail_codes=1` corroborates it.
FASTBUS tripped0 because its three
below95%-baseline scans were interrupted by an intervening95.20% scan.
No nFAULT/current foldback/tracking stop; output off/nFAULT1. IT87/BS85
CRC and ordinal decode PASS, 128 accepted events/16 bus frames. A cluster
of software-only accepts begins33.270690s; first sampled bus decline is
33.271099s (~409us later), ratio95.65%; subsequent scans include92.98%,
93.31%,92.78%. Intervals/average also lengthen in the cluster. This
supports event-tracking disturbance BEFORE the sampled load surge, but
226us bus cadence cannot prove the first dip or causation. The aggregate40
epoch has physical7231/software291/both192/neither0; source1=software-only,
source2=both ambiguous. Capture retained at
`captures/reverse_direction_2026-09-18_it87_40_adcfault.txt`. 40/50 remain
unqualified. Exact prior reverse lean SHA
2B4EA00687706E272D3AE8B958338D9A58AB17EC24BA621BF093988870E4CD5F
reflashed/verified; disabled guard3/18, reverse role6/6, duty6/6, p/i
gates/ENABLE/MOE/CCRs0/nFAULT1 PASS. Current board this SHA, OFF, COM41
closed. No old-direction run; visible shaft direction remains unconfirmed.

2026-09-18 STAGED aligned reverse diagnostic, NOT FLASHED at freeze. Exact
SHA BBCC4F564C5564ACA46CAB01FC755B82189B6322808237740C9B9DA056245024
at `captures/reference/reverse_revisit_it87_bs85_20260918/` joins the
validated comparator-dispatch origin snapshot to the existing 128-event
IT86 interval ring and 16-scan BS85 bus ring. IT87 packs origin into the
event-limit upper2 bits (limit<=1000us); no extra per-event RAM row and no
control/guard changes. Decoder preserves old IT85/IT86 semantics; host
6+3+3 tests, old aligned capture replay and four ISR-root M0 soft-math audit
PASS. Accepted Recorder symbol +84 bytes versus origin-only; diagnostic,
not lean qualification. Current board remains exact reverse lean SHA2B4E,
OFF, COM41 closed. First powered action after flash requires disabled
guard/role/duty/off preflights; one bounded 35->40 attempt only, retaining
any fast-sag failure. Old direction forbidden.

2026-09-18 SAFE RESTORE after dispatch diagnostic. Exact previously
10/25-qualified reverse lean SHA
2B4EA00687706E272D3AE8B958338D9A58AB17EC24BA621BF093988870E4CD5F
reflashed/verified to explicit G071 and hard-reset. Disabled guard3/18,
reverse role6/6, duty6/6, p/i gates/ENABLE/MOE/CCRs0,nFAULT1 PASS. No
powered run after restore. Current installed image is this SHA, OFF, COM41
serial session closed. No old-direction run. 40% remains a repeatable fast-sag
stop on diagnostic builds; 50% reverse not reached. Visible shaft-direction
confirmation from operator remains absent, so only electrical reverse
mapping is proven.

2026-09-18 REVERSE DISPATCH PROBE POWERED RESULT (SHA698758C5 installed/OFF
at stop; restoration pending). Corrected snapshot image flashed/verified G071;
disabled guard3/18, reverse role6/6, duty6/6, p/i off/nFAULT1 PASS. Ordinary
start and 1%/250ms ramp to35 held until deliberate host stop at powered
32.371s, reason9, FASTBUS0/current foldback0/rails0/bus-low0. Final35 epoch
accepted-origin buckets: physical-only156677, software-only1705, both2316,
neither0; software-only is the direct-rescue lower bound, both ambiguous.
Next ordinary start ramped to35, waited3s, then0.5%/500ms to ACKed40. It
stopped at powered39.487s, reason26 fast bus sag (three coherent low scans),
no current foldback or nFAULT, p/i outputs0/nFAULT1. Final40 epoch buckets:
physical-only57342, software-only1985, both1445, neither0. Measured interval
>prior average+25% counts physical4733/software185/both243. Diagnostic
aggregates establish that real software-only accepts occur but do NOT place
one at the terminal collapse or prove causation. Final estimated speed at
40 fault1536eHz versus35 clean stop1579eHz is suggestive, not matched
steady-state evidence. Captures `captures/reverse_direction_2026-09-18_dispatch35_valid.txt`
and `...dispatch40_fastbus.txt`. Do not call40 qualified or run above40 based
on these. Restore exact reverse lean SHA2B4E before further work; no old
direction.

2026-09-18 REVISIT ORIGIN PROBE CORRECTION. Initial SHA0178 diagnostic was
flashed/verified, disabled guard3/18, reverse role6/6, duty6/6, p/i off.
First10% startup refused transfer (result2/coast_stop8), safely disabled.
Second ordinary start entered powered lock and live-ramped10->35 in 1%/250ms
steps; held35 until operator-style host stop at powered38.087s, reason9,
FASTBUS0/current foldback0/rails0/bus-low0, p/i off/nFAULT1. Its final
REVISITORIGIN said physical17431/neither178385: INVALID attribution because
minz-core clears EXTI *before* EV_ACC. Captures retained. Corrected candidate
SHA698758C5BF7840E1BCAE2810AB5A3DE4DBF608BF17491CCC3EC010F32153592B
at `captures/reference/reverse_revisit_dispatch_20260918/` snapshots
physical/software flags at comparator dispatch, before EXTI clear, and uses
the accepted dispatch's snapshot. Host2/2 and four ISR-root M0 audit PASS;
text125644/data1200/bss17184. NOT YET FLASHED at freeze. Diagnostic only;
neither prior hold nor corrected candidate qualifies lean35/40. Old direction
forbidden. Board remained SHA0178/OFF after host stop; nFAULT1.

2026-09-18 STAGED reverse-only revisit-origin diagnostic, NOT YET FLASHED at
freeze. SHA0178D6665A3AB0C917A56759F49F3191A72F588320201556C82C3DF725649782
at `captures/reference/reverse_revisit_origin_20260918/`. Same reverse
control/4A active current/5%-3-scan fast sag/normal restart as lean image,
plus four acceptance-time flag buckets and >25%-late subset, reset on live
duty change and printed poststop only. Software-only is a lower bound on
direct rescue; both-flags is ambiguous. Accepted recorder grows104 bytes;
four ISR-root M0 soft-math audit and host tests pass. Diagnostic only, not
lean qualification. Current installed board remains SHA2B4E/OFF; no
old-direction runs.

2026-09-18 REVISIT COUNTER INTERPRETATION CORRECTION (source audit only,
board unchanged lean SHA2B4E/OFF). `revisit_low_speed_level` sets
LEVEL_REVISIT_INFLIGHT and increments ATTEMPTS when it pends ADC_COMP.
`level_revisit_accepted` clears INFLIGHT and increments ACCEPTS on the next
EV_ACC, regardless of whether that EV_ACC came from the pended service or a
later real EXTI edge (`core_bench.rs:900..945,1100..1115`); a persistence
rejection returns from minz-core ISR before EV_ACC and does not clear
INFLIGHT (`../minz/core/src/am32_isr.rs:95..119`). Thus attempts=accepts
means each attempted sector eventually accepted, NOT that every software
pend caused the acceptance. Prior E868-E871 and reverse-run wording calling
the equality proof of active hybrid-event origin is overclaimed; treat the
count as an upper bound for directly rescued accepts. The reverse off35 A/B
still proves that removing the mechanism at35 led to two Tracking8 stops;
it does not quantify the rescued-event fraction. A future low-cost diagnostic
must attribute acceptance to the CURRENT comparator dispatch (synthetic-only,
physical-only, or coalesced), not to the persistent inflight flag. No source
edit or motor run was made for this correction; old direction still forbidden.

2026-09-18 REVERSE LEAN10/25 RECOVERY (latest installed/OFF). Same exact
corrected lean SHA 2B4EA00687706E272D3AE8B958338D9A58AB17EC24BA621BF093988870E4CD5F
as the four separate hold passes below. Deliberate Tracking8 injection at
10%/5.000010s powered: disabled1s settle, fresh ordinary startup/transfer1,
second handoff, resumed10% with0 steps, second POWERPATH reason2 at
14.236014s against14.236473s planned (459us rounding); no fast sag/current
foldback/rail/bus-low/nFAULT/second tracking. 25% injection at10.000053s
after live ACK: same fresh path, restored25 in3 foreground5-point/2s steps,
second reason2 at29.236004s against29.236325s planned (321us rounding),
same zero electrical faults. Both have unique LEANCORE marker; compact
normal-restart verifier + p/i off/nFAULT1 PASS. Captures
`captures/reverse_direction_2026-09-18_lean10_restart.txt` and
`...lean25_restart.txt`. Thus this exact reverse lean image now has two
ordinary-start holds plus one independently verified normal-start recovery
at each10/25 point. This does NOT qualify40/50 or independently witness
visible shaft direction. Current board remains this SHA installed/OFF.

2026-09-18 REVERSE LEAN HOLD COHORT (latest installed/OFF). Corrected exact
SHA 2B4EA00687706E272D3AE8B958338D9A58AB17EC24BA621BF093988870E4CD5F
flashed/verified explicit G071; disabled guard3/18, reverse role6/6,
duty6/6, p/i off/nFAULT1 PASS. Four separate ordinary-start holds, all
POWERPATH reason2 full powered window, unique `LEANCORE` marker, transfer1,
FASTBUS0, current foldback0, rails0, bus-low0, tracking event_fault0,
no nFAULT stop, final p/i all gates/ENABLE/MOE/CCRs0,nFAULT1:
10% hold1=20.000004s/63260 COM/~529eHz;
10% hold2=30.000015s/95016 COM/~529eHz;
25% live-ramped hold1=35.000004s/244376 COM/~1212eHz;
25% live-ramped hold2=30.000010s/207780 COM/~1230eHz.
The 25% ACK `D=00FA` occurred during the first few powered seconds, but this
compact firmware does not timestamp the duty epoch; do not assign an exact
25%-only dwell from the total powered window. Captures
`captures/reverse_direction_2026-09-18_lean{10,25}_hold{1,2}.txt`;
strict off/marker/stop checks4/4 PASS. These establish repeated lean holds,
not yet normal-start recovery on this exact image. Current board OFF on same
SHA. Next: one deliberate Tracking8+ordinary restart at10 and25 using the
unchanged electrical guards; retain all outcomes. No old-direction runs.

2026-09-18 REVERSE LEAN10 PROVISIONAL + marker correction. Initial lean
normal-restart SHA 9EF8DF7AABE7B9630285818F063D725972422CFDE5022C01C039DC954DABE8FA
flashed/verified G071 and passed disabled guard3/18, reverse role6/6,
duty6/6, p/i off/nFAULT1. Its ordinary-start reverse10% run held the full
30.000005s powered, reason2, 94983 COM, estimated532eHz, FASTBUS0,
foldback0, rails0, bus-low0, no tracking/nFAULT; p/i outputs0/nFAULT1 after.
Capture `captures/reverse_direction_2026-09-18_lean10_provisional.txt`.
Compact postrun output suppressed the `LEANCORE` witness despite lean
features, so retain this as provisional rather than strict-image qualification.
Added post-stop-only `core_bench::lean_marker` to compact terminal output;
corrected SHA 2B4EA00687706E272D3AE8B958338D9A58AB17EC24BA621BF093988870E4CD5F,
text124816/data1200/bss17144, four ISR-root math audit PASS. Both ELFs
frozen at `captures/reference/reverse_lean_restart_20260918/`; corrected
image is NOT flashed. Current board still has older SHA9EF8 installed/OFF.
Repeat formal holds/recovery on exact corrected SHA; old motor/old direction
data do not qualify this replacement.

2026-09-18 STAGED reverse lean normal-restart qualification image, NOT
flashed. SHA 9EF8DF7AABE7B9630285818F063D725972422CFDE5022C01C039DC954DABE8FA,
release-hybrid text124780/data1200/bss17144. Based on restored reverse
control, omits only fault-frame/seed-stage postrun diagnostics; keeps lean
core/IRQ, compact output, normal startup/restart, active4A signed-current
limiting, 5%/three-scan fast bus stop, absolute bus/nFAULT/tracking/watchdog,
real-level revisit and physical A/B reverse. Four motor ISR-root soft-math
audit PASS; 35 policy tests, five compact verifier, one current wiring, two
formatter and three fast-bus tests PASS. Frozen
`captures/reference/reverse_lean_restart_20260918/`. Current installed board
is still reverse control SHA6295 OFF. This candidate is for repeated separate
10/25% holds and normal-start recovery, not an upper-envelope verdict.

2026-09-18 RESTORED reverse guarded control after negative off35 A/B
(latest installed/OFF). Exact frozen SHA
629568553F62ED37E2960E12DD0D60E788FFE4370E5715C4C7617C8F3E236092
from `captures/reference/reverse_control_20260918/shell-pwm.elf` re-flashed
and verified on explicit G071 SN/device0x460, reset. Disabled guard3/18,
role6/6 flags127 ARR2666, duty6/6, p/i all gates/ENABLE/MOE/CCRs0,nFAULT1
PASS. No powered run on this restored SHA after reflash. Do not use retired
off35 or CPU images for upper-envelope work; no old-direction firmware.

2026-09-18 REVERSE OFF35 A/B FALSIFIED (latest installed/OFF). Opt-in cutoff
SHA 9D46A9B8AAFF128DE036489502E741E8CBCAF0B8092DC49F0035F3DFB0FA9CC8
flashed/verified explicit G071 SN/device0x460; disabled guard3/18,
role6/6 flags127 ARR2666, duty6/6, p/i outputs0,nFAULT1 PASS. Live-ramped
to ACKed35%, with foreground level revisits disabled only from that point.
Tracking8 stopped first powered epoch at10.295059s, automatic normal-start
recovery replayed settings and restored35%, then Tracking8 stopped the second
powered epoch at10.112055s; BEMFSTOP ~1564eHz, event_fault1. FASTBUS0,
foldback0, phase rails0, absolute bus-low0, nFAULT1 after stop; final
p/i all gates/ENABLE/MOE/CCRs0. `REVISITPOLICY cutoff_tenths=350` was emitted,
and total1424/1424 revisit accepts accrued only before the cutoff/restart
restoration. This is a negative controlled result: high-speed revisit cannot
simply be removed at35 to solve40; the physical edge path alone remains
intermittent here. Capture
`captures/reverse_direction_2026-09-18_off35_tracking.txt`. Do NOT test
37.5/40 on this candidate or repeat-until-pass; restore prior guarded reverse
control before next powered work. No claim that revisits cause or prevent the
40% fast-sag transient. Old direction remains forbidden.

2026-09-18 STAGED reverse high-duty revisit A/B, NOT flashed. New opt-in
`bench-running-revisit-off35` keeps the existing real-level rescue below35%
but refuses its foreground pend at ACKed duty>=35%; normal physical COMP
interrupt and all electrical stops remain. Pure cutoff test34.9/35/50 PASS;
release-hybrid SHA 9D46A9B8AAFF128DE036489502E741E8CBCAF0B8092DC49F0035F3DFB0FA9CC8,
text125644/data1200/bss17176, four ISR-root soft-math audit PASS. Frozen
`captures/reference/reverse_revisit_off35_20260918/`. This is a causal
diagnostic for whether high-speed level revisits contribute to the reverse
40% slowdown/sag; it is NOT lean qualification. Current board remains restored
reverse control SHA6295 OFF. Do not power this candidate without disabled
preflight; first35, then37.5, then bounded40 only if lower steps hold. A
tracking/fast-sag stop is retained; no threshold relaxation, no old direction.

2026-09-18 RESTORED reverse control (latest installed/OFF). After CPU35
observer-affected fast sag, rebuilt the prior BF06 feature closure without
CPU accounting. New SHA 629568553F62ED37E2960E12DD0D60E788FFE4370E5715C4C7617C8F3E236092,
same text125512/data1200/bss17176 as BF06; cfg-gated source edit changed ELF
debug/SHA, so call it a feature-equivalent rebuild, not byte-identical.
Frozen `captures/reference/reverse_control_20260918/shell-pwm.elf` and README.
Four ISR-root soft-math audit PASS; explicit G071 SN/device0x460 flash verify
and reset PASS; disabled guard3/18, reverse role6/6 flags127 ARR2666,
duty6/6 and p/i gates/ENABLE/MOE/CCRs0,nFAULT1 PASS. No powered run on this
new SHA. It retains reverse-only phases and all electrical protections.
Do not infer CPU saturation from intrusive probe; do not re-run old direction.

2026-09-18 REVERSE CPU35 FAIL (latest installed/OFF). Same corrected CPU
diagnostic SHA 3EAE27DF... live-ramped10->15->20->25->30->35. It ACKed35
(`D=015E`) then fast 5%/three-scan bus guard stopped reason26 at10.973529s
powered, roughly1.3s after the35 command; baseline raw1211/vref1506,
current foldback0, rails0, bus-low0, no nFAULT/tracking stop. CRC-valid
CPUUNION covers10.973529s: IRQ7.265896s(66.21% observer-affected),
guard0.295898s(2.70%), COMP3.296901s(30.04%), COM2.181604s(19.88%),
DMA1.491493s(13.59%), fault0; 409310 IRQ calls. This is a whole-ramp
aggregate, NOT a35%-only rate and not directly comparable with the longer
30% run. A prior separate CPU image also sagged at35 while a non-CPU image
held35, so the meter is materially observer-affected. Do not infer a lean35
wall or CPU saturation from this. Capture
`captures/reverse_direction_2026-09-18_cpu35_fastbus.txt`; postrun p/i all
gates/ENABLE/MOE/CCRs0,nFAULT1. Board remains this SHA installed, OFF.
Before another powered run choose a lower-overhead/single-question diagnostic
or restore a lean reverse image. Retain the 35 fault; no repeat-until-pass.

2026-09-18 REVERSE CPU30 MEASURED (latest installed/OFF). Corrected aggregate
diagnostic SHA 3EAE27DFBDD44B617068F0D2B4A445F93024FFEC2BB80813C586BCFCAE2BDAED
flashed/verified on explicit G071 SN/device0x460. Disabled cpucheck2/7/10us,
guard3/18, role6/6 flags127 ARR2666, duty6/6 and p/i outputs0,nFAULT1
PASS. Reverse live10->15->20->25->30, host stop at22.974335s powered;
fastbus0, foldback0, rail0, bus-low0, no nFAULT/tracking stop. CRC-decoded
CPUUNION elapsed22.974235s, IRQ15.978068s =69.55% observer-affected,
foreground/unattributed6.996167s; CPUROOT guard0.566685s(2.47%),
COMP6.921644s(30.13%), COM5.347643s(23.28%), DMA3.142096s(13.68%),
other roots0; all six roots exactly partition IRQ union, fault0. Calls865347
over window (~37.7k/s). Disabled software-pair probe cost is not calibrated
into the live result; do NOT call69.55% lean utilization, compare it only to
matched instrumented images. Capture
`captures/reverse_direction_2026-09-18_cpu30_fixed.txt`; decoder
`scripts/drv_cpu_meter.py`, its7 tests PASS. Postrun p/i readback is still
required. This is diagnostic evidence, not 30% lean qualification. The
previous entry's staged/installed status is superseded by this one.

2026-09-18 REVERSE CPU probe correction (latest installed state). The first
aggregate-CPU ELF SHA DF453D20FF42A6AC6581B2209C1DD82EDC166B396A891ED78428B002788643F4
was flashed/verified on G071. Disabled cpucheck2/7/10us, guard3/18,
role6/6, duty6/6 and p/i outputs0,nFAULT1 passed. Reverse live10->15->20
->25->30 ran to22.970576s powered with no fast sag, foldback, rail, nFAULT or
tracking stop, then host abort reason9. But `bench-compact-qual` suppressed the
CPUUNION/CPUROOT branch, so this is NOT a CPU measurement. Fixed the compact
terminal path to emit only the small aggregate rows after safing, removed
unused fault-frame and seed-stage diagnostics, and staged corrected ELF SHA
3EAE27DFBDD44B617068F0D2B4A445F93024FFEC2BB80813C586BCFCAE2BDAED
(text128776/data1200/bss17212, flash headroom1096, four ISR-root math audit
PASS). Corrected ELF is NOT flashed/powered. Current board still runs the
retired DF45 CPU image, OFF after host stop; check p/i before further powered
work. Artifacts and caveat in `captures/reference/reverse_cpu_20260918/`.

2026-09-18 OFFLINE reverse40 diagnostic preparation (superseded installed
state; timing analysis remains valid).
CRC-valid aligned IT86/BS85 from the earlier reverse40 fast-sag capture shows
an accepted165us gap at23.662141s while the sampled bus was still98.54% at
23.662244s; cycles subsequently lengthened and three normalized bus scans
fell94.61/94.57/93.04% at23.663826/.664052/.664278s. Sampling is226us,
so this favors but does not prove timing/load preceding the voltage dip. The
37.5% retained tail ended with a634us electrical cycle without a bus trip;
the40% tail ended623us after reaching627us, so a single terminal-cycle number
does not classify stability. Prepared separate reverse CPU-accounting ELF SHA
DF453D20FF42A6AC6581B2209C1DD82EDC166B396A891ED78428B002788643F4
in `captures/reference/reverse_cpu_20260918/`; release-hybrid text129472,
data1200, bss17244, 400-byte flash margin; four ISR-root math audit PASS.
This image is staged ONLY; it has not been flashed or powered and cannot
qualify the envelope. Current installed board remains the OFF BF06 image
documented next. No old-direction image or sibling rm32 was touched.

2026-09-18 REVERSE 40% RETEST (latest installed state). CURRENT installed/OFF
reverse-only hybrid SHA BF06A19BFD483CA92B8042073EA4D74EA4E72AAC36DDB333C83ED9C05A1B352A,
release-hybrid binz opt-s/dependencies opt-z, thinLTO/codegen1; four motor ISR-root
soft-math audit PASS. This image adds opt-in `bench-adc-latest-fault-frame`:
on a coherent-frame publication error it safes first, then retains the rejected
frame for postrun diagnosis; the successful DMA path has no extra recorder.
Flash verified on explicit G071 SN/device0x460; disabled guard3/18, six role
checks and six duty checks PASS. Its ordinary-start live10->15->20->25->30->35
->37.5->40% run ACKed40%, then stopped at22.078298s on fast bus sag reason26
(baseline raw1210/vref1506, 95%/three226us coherent scans). No current
foldback, nFAULT, tracking or phase-rail stop. No `ADCFRAME` was expected because
this stop was bus, not ADC. Capture
`captures/reverse_direction_2026-09-18_hybrid_40_fastbus_repeat.txt`; postrun
p/i confirms all gates/ENABLE/MOE/CCRs0 and nFAULT1. An earlier40% attempt on
the prior SHA820F hybrid image stopped on ADC/DMA publication reason11 at
28.828783s, with one phase-rail code and zero fast-sag count; capture
`captures/reverse_direction_2026-09-18_hybrid_40_adc_fault.txt`. Combined with
the earlier reverse 24kHz40% fast-sag stops, 40% is not qualified. Preserve
both fault classes; diagnose the late control/current/bus event before another
blind40% attempt. The old motor direction remains forbidden. Electrical A/B
phase reversal is built and verified; visible shaft direction has not been
independently confirmed. No reverse50% claim, no sibling rm32 edits.

2026-09-18 REVERSE UPPER MAP update. Same installed/OFF guarded hybrid SHA
820F901CD2538F5BD040E0989A8C050ADDFED0450D8EFD420B5EA49D4732939E.
One live-ramped ordinary-start run held35% ~15s,37.5% ~15s,38.8% ~15s,
then deliberate host stop at67.831s energized. ~1718eHz final,
582548 COM, bounded level revisits25578/25578; no fast-sag, foldback,
bus-low, rail, tracking or nFAULT stop. Postrun p/i outputs off,nFAULT1.
Capture `captures/reverse_direction_2026-09-18_hybrid_388_hold.txt`.
This is a single hybrid-image diagnostic hold at38.8%, not repeated lean
qualification. The earlier separate24k diagnostic image had three sharp-sag
stops at40%; this result narrows but does not erase that evidence. The bounded
40% revisit was subsequently run and is documented in the latest entry above.

2026-09-18 REVERSE30 RECOVERY update. Same installed/OFF SHA
820F901CD2538F5BD040E0989A8C050ADDFED0450D8EFD420B5EA49D4732939E.
Host ACKed live30% before the one-shot injected Tracking8 at10,000,129us;
disabled1s settle, fresh ordinary startup/second BEMF handoff, foreground
restoration10->15->20->25->30% in four2s steps, then planned remaining
29,236,576us vs actual29,236,008us (568us window rounding), 220204 COM,
~1400eHz final. No fast-sag, current foldback, bus-low, phase-rail, second
tracking or nFAULT stop. Compact deadline verifier PASS, capture
`captures/reverse_direction_2026-09-18_hybrid_restart30_deadline_pass.txt`;
postrun p/i gates/ENABLE/MOE/CCRs0,nFAULT1. This extends the single-run
injected-loss recovery evidence through30%, not a repeated recovery cohort or
40/50% qualification. The fast sag guard's digital path was independently
injected/latency-checked at E880 (third coherent frame to completed shutdown
85us) and had no false trip in repeated reverse10/25 holds and10/25/30
recovery runs; analog response to a collapsing physical supply is not implied.

2026-09-18 REVERSE NORMAL-START RECOVERY (latest, supersedes older installed
status below). CURRENT installed/OFF reverse-only hybrid image SHA
820F901CD2538F5BD040E0989A8C050ADDFED0450D8EFD420B5EA49D4732939E,
profile `release-hybrid`: binz opt-s, dependencies opt-z, thinLTO/codegen1;
text125044/data1200/bss17144 fits128k. `bench-compact-qual` removes ONLY
post-stop bulk waveform/coast formatter (terminal summaries and all powered
guards remain); `bench-reverse-flat-start` uses the replacement motor's proven
100Hz startup even with normal restart compiled. Four ISR-root M0 arithmetic
audit PASS. Explicit G071 SN/device0x460 flash verified; disabled guard3/18,
roledu100 6/6 flags127 ARR2666, dutycheck6/6, p/i outputs off/nFAULT1 PASS.
Fast bus 5%/3 coherent DMA scans, signed-average current foldback/terminal
stop, nFAULT, tracking, IRQ-rate and watchdog remain active. The image also
contains opt-in postrun-only `bench-reverse-seed-stage`; no powered extra read.

Reverse injected-loss normal-start recovery passed at10% and25% on this
image. At10%, first Tracking8 was injected at4,999,990us, disabled settle1s,
fresh ordinary startup/handoff, second deadline planned14,236,408us vs
actual14,236,005us (403us rounding), no sag/current/nFAULT fault, outputs
off/nFAULT1. At25%, host ramped10->15->20->25 before injected Tracking8 at
10,000,009us; restart re-established BEMF, restored25% in three foreground
5-point/2s steps, then planned29,236,358us vs actual29,236,004us (354us),
198106 COM, ~1225eHz, no sag/current/nFAULT/tracking fault on second run,
outputs off/nFAULT1. Compact deadline verifier PASS on both captures:
`captures/reverse_direction_2026-09-18_hybrid_restart10_deadline_pass.txt` and
`...restart25_deadline_pass.txt`. This is one recovered run per point, not a
repeated recovery cohort. Earlier lean opt-s ordinary-start10/25 holds2/2 and
30% one hold remain valid on a different SHA; do not transfer qualification
automatically across optimization/feature images.

Failure trail retained: pure opt-z fit but refused late measured-seed handoff
(stage ages152/182/192/288/290 half-us, arm age304; guard install took~48us);
hybrid speed/size fit restored handoff, then restart replay refused a legacy
6.2% duty cap, then replayed10% but omitted timed-ADC `startup_begin` and
safed on an absent first sample at1ms. Both defects are fixed in current binz:
replay uses same acquisition-duty bound as ordinary start, and restart uses
the same timed-ADC admission/first-frame check before MOE. Failed captures
are retained under `captures/reverse_direction_2026-09-18_*`.

NEXT: reverse 30% lean/recovery repeat and diagnose the genuine40% sharp-sag
onset before 50% climb. 35%/37.5% diagnostic holds are not lean qualification;
40% at24k has three fast-sag stops and20k was worse (Tracking8 at35%). Do
not raise the sag threshold or call50% qualified. Visible shaft-direction
confirmation from the operator is still pending; do not flash old-direction
firmware. Do not edit sibling rm32/rm32_stm32.

2026-09-18 REVERSE LEAN 30% HOLD (latest). CURRENT installed/OFF remains SHA
3BE434DD8B79EBDB04CA78F8CBAAC5FDCF31DD7370BB2863924046C68CB8EA9B.
Third independent ordinary startup/handoff, live-ramped10->15->20->25->30%,
then held30% about30 s before deliberate host stop; total energized46.878 s,
325126 COM, final estimate~1394 eHz. No fast sag, current foldback,
bus-low, phase-rail, tracking event or nFAULT stop; 10079/10079 real bounded
level revisits. Postrun p/i all gates/ENABLE/MOE/CCRs0,nFAULT1. Capture
`captures/reverse_direction_2026-09-18_lean_30_hold.txt`. One lean30% hold,
not a repeated30% cohort. Reverse 10/25% have two lean runs each below;
35/37.5% are diagnostic only, and40% has three sharp-sag stops at24k.
No claim of reverse 50% or physical direction witness.

2026-09-18 REVERSE LEAN second hold. Same installed/OFF SHA
3BE434DD8B79EBDB04CA78F8CBAAC5FDCF31DD7370BB2863924046C68CB8EA9B.
Second independent ordinary startup and BEMF handoff succeeded: 10% held
~12 s, then25% held ~30 s, total energized55.595 s, 308645 COM,
~1212 eHz at25%, real level revisits7199/7199. Deliberate host stop only;
fast sag0, foldback0, bus-low0, rails0, tracking event_fault0, outputs
off and nFAULT1 postrun. Capture
`captures/reverse_direction_2026-09-18_lean_10_25_hold2.txt`.
Together with hold1 below, ordinary-start 10/25% reverse low/mid-range
has two clean lean runs, but normal-start recovery and visual shaft-direction
confirmation remain outstanding. Do not extrapolate these to40/50%.

2026-09-18 REVERSE LEAN requalification (latest). CURRENT installed/OFF
reverse 24-kHz lean image SHA
3BE434DD8B79EBDB04CA78F8CBAAC5FDCF31DD7370BB2863924046C68CB8EA9B,
text127416/data1200/bss19588, release-s/thinLTO/codegen1; four ISR-root M0
arithmetic audit PASS. It retains the existing report-only single-cycle
CycleTiming policy via `bench-fast-cycle-report`, plus fast 5%/3-scan bus stop,
average-current foldback, nFAULT, tracking, IRQ-rate and watchdog stops. It
omits IT86/BS85 tails, not electrical guards. Disabled guard3/18, roledu100
6/6 flags127 ARR2666, dutycheck6/6, p/i outputs off and nFAULT1 PASS. One
ordinary-start live run held reverse10% about11 s, then25% about25 s before
deliberate host stop; 49.617 s total energized, 268780 COM, no fast sag,
foldback, phase rails, bus-low or tracking fault, ~1225 eHz at25%, outputs off
and nFAULT1 postrun. Capture:
`captures/reverse_direction_2026-09-18_lean_10_25_hold1.txt`. This is first
lean hold, not repeated qualification nor recovery. A preceding lean candidate
mistakenly omitted `bench-fast-cycle-report`; it was stopped by the old
CycleTiming floor at exactly2222us versus2223us ~71ms after handoff and is
RETIRED/INVALID for envelope inference. Capture retained as
`captures/reverse_direction_2026-09-18_lean_cyclefloor_invalid.txt`.
Automatic `bench-normal-restart` added to this lean feature set overflows
128k flash by1408 bytes; do not claim restart qualification or drop safety
code merely to fit it. Reverse 50% remains unqualified. Old-direction images
remain forbidden; physical shaft-direction witness is still pending.

2026-09-18 REVERSE MIDPOINT update. CURRENT installed/OFF reverse 24-kHz
diagnostic SHA 9948D82825C4F95F404389461A34885BCBFE1AD0914E37E832A8FDA19FCCD4A,
release-s/thinLTO/codegen1, text128544/data1208/bss20884, four ISR-root M0
math audit PASS. Disabled guard3/18, roledu100 6/6 flags127 ARR2666,
dutycheck6/6, p/i outputs off and nFAULT1 PASS. In one live ramp, 35% held
~20 s and 37.5% held ~12 s at 24 kHz with real bounded level revisit and no
electrical stop; deliberate host stop after 37.5% produced POWERPATH reason9,
fast bus tripped0, foldback0, bus-low0, rails0, tracking event_fault0, nFAULT1
postrun. IT86/BS85 CRC-valid captured in
`captures/reverse_direction_2026-09-18_375pct_hold_24k.txt`; final cycles
582..634 us, last bus scans ~97.1..99.9% of baseline. Diagnostic hold, not
lean qualification. 40% at24k still has three fast-sag stops; do not claim
40% or 50% reverse qualified. The fault-only IT86 gate now uses terminal
`dump_reason` (postrun output, no powered timing change). The 20k carrier A/B
Tracking8 stop at35% remains retained below. Physical shaft-direction witness
has not arrived; do not run the old-direction image. No sibling rm32 edits.

2026-09-18 REVERSE-DIRECTION CAMPAIGN — CURRENT (supersedes the older
reverse-direction status immediately below). Operator prohibits the old
direction; only images with `bench-reverse-phases` may be flashed or driven.
The mapping swaps physical A/B in drive, powered roles, COMP mux and acquisition;
visible shaft direction still awaits independent operator confirmation. The
replacement motor has repeatedly held reverse BEMF at 10/15/20/25% for 10 s;
with bounded `bench-running-level-revisit` at 24 kHz, two 30%-commanded holds
completed about 31 s each without tracking, bus, current or nFAULT stop
(9009/9009 and 9359/9359 real revisit attempts/accepts, ~1.39 keHz). Those are
diagnostic passes, not lean qualification. Without revisit, 30% repeatedly
stopped Tracking8 after 8.75–11.85 s; retained IT86 tails show a single late
crossing, not a growing slowdown. Live UART throttle works if host sends one
byte per write; a bulk `du150` write at high IRQ load lost bytes and caused a
HostAbort, so do not use bulk live writes.

At 24 kHz, 35% held 10 s; 40% ACKed but three diagnostic attempts stopped on
the new fast bus-sag reason26 after ~6–7 s at target. BS85 last-16 scans and
aligned IT86 show ~100% to 93% bus drop over ~3 ms, extreme phase-current
samples, and cycle excursions 621/627/623 us versus ~570 us before the largest
current/sag. This is a genuine coherent sharp dip caught in ~0.7 ms, not a
one-sample ADC notch; timing/loading causality is not yet settled. Do not raise
the 5%/3-scan threshold or treat extra PSU amps as the fix. A one-variable
20 kHz A/B was worse: it reached 35% then Tracking8 after ~20.17 s total,
before the 40% command, with no fast-sag/current/nFAULT stop. Capture:
`captures/reverse_direction_2026-09-18_35pct_tracking_20k.txt`.

CURRENT installed/OFF image is the reverse 20-kHz diagnostic SHA
A1C950E911C93996E63A0DB96989F611F82C05418F8A65CF2E348294F47529D5.
Release-s/thinLTO/codegen1, text128584/data1208/bss20884; four ISR-root
M0 arithmetic audit PASS. Disabled guard3/18, six roledu100 flags127 with
ARR3200, dutycheck6/6, p/i outputs off and nFAULT1 passed before its run;
terminal DONE gates/en off. The next flash should restore the better-supported
24-kHz carrier and retain fast sag, current foldback, nFAULT, tracking, IRQ-rate
and watchdog safing. The optional IT86 fault dump was just corrected to gate
on the shell's terminal reason (`dump_reason`), because POWERPATH may report a
different teardown reason11 after a Tracking8 stop; this change is not flashed
yet. The prior 24-kHz aligned capture is
`captures/reverse_direction_2026-09-18_40pct_fastbus_aligned_it86_bs85.txt`.
Do not edit sibling `rm32`/`rm32_stm32`; 50% reverse remains unqualified.

2026-09-18 REVERSE-DIRECTION UPDATE (supersedes E898 pause detail below).
The installed image is reverse-only SHA
ACC923F833896940355BFFB4DDB58097E94D20994BB11431F817A1248CA209D9,
OFF/UART responsive after the latest run. It swaps A/B consistently across
drive, roles, COMP mux and flying acquisition; do not flash or run any
old-direction image. The hardware does NOT yet have a qualified reverse BEMF
hold, and the visible shaft direction has not been independently witnessed.
E898's lost UART was traced to genuine comparator/EXTI18 chatter after
handoff combined with a missing lean-path rate stop, not successful running.
The lean rate stop was restored with distinct reason13; one-shot probe counts
real pending COMP events (ADC IER0), not serial/ADC vector noise. Low-speed
280us masking, 20kHz carrier and a stricter diagnostic cap24/ms allow several
accepted events/COMs, but do not suppress the later burst. A controlled
10% acquisition/BEMF duty A/B, with startup DMA feedback and unchanged fast
bus sag/current/nFAULT/tracking protection, reached five COMs before a true
25/ms comparator burst and reason13 at 2.896ms powered. Fast sag0, foldback0,
no nFAULT/tracking stop; final p/i all gates/ENABLE/MOE/CCRs0, nFAULT1. This
falsifies insufficient 6.1% ON-time as the sole cause. No further blind
powered cap/blank/duty ratchet: resolve the reverse comparator sequence or
qualifier behaviour offline, then perform a bounded low-duty retest. The
reverse image drops non-safety normal-restart/revisit/event100 features to fit
128kB flash; it is diagnostic, not a qualification image. Do not edit sibling
rm32. E898's instruction to stop on missing UART was obeyed; current runs have
independent rate safing and terminal summaries. One-variable no-blank A/B on
the current image (20kHz/10%/cap24) passed disabled guard3/18, roledu100 6/6
and dutycheck6/6. Valid driven transfer reason22, but the genuine comparator
burst hit25/ms and reason13 after only0.592ms and two COMs; with280us blank
the otherwise matched run reached2.896ms/five COMs. Blanking delays but does
not cure chatter; omitting it worsens the symptom. No reverse lock qualified.

2026-09-18 REVERSE-DIRECTION E898, CURRENT installed/OFF image SHA
90EEEEF21AA0D9911705ACA3063EA9F0C3F9CCC83C456F596E3A5CBBBC9B9B21.
Operator forbids the old direction. `bench-reverse-phases` swaps physical A/B
consistently in sine PWM, forced six-step, powered role writes, COMP input and
all three flying-acquisition mux sites; the logical six-step permutation is
1,2,3,4,5,6 -> 4,3,2,1,6,5. Pure reverse mapping tests2/2 PASS;
release-s/thinLTO/codegen1 build and four motor ISR-root arithmetic audit PASS.
Explicit G071 SN066CFF343433464757233430 flash verified, Cube reset identified
device0x460. ENABLE-low guard3/18 and all six roledu62 checks PASS, each role
flags127, comparator mux matches the reversed floating phase. This proves the
electrical command mapping, not visible shaft direction.

E898 powered low-duty reverse attempts are NOT qualified. First10% attempt
forgot `engage1` and remained open-loop sine until near the five-second cutoff;
startup foreground current rail/peak reason4 safed outputs. With `engage1`,
7% catch->10% target refused flying seed FLYTooSlow after2 intervals;
9% catch->10% target refused FLYTooSlow after9 intervals. A subsequent10%
catch/10% target attempt produced no UART summary or response after >15s and
`off` was not acknowledged. Do not infer a successful hold from silence.
Immediately reset the intended G071 through Cube SWD; UART returned and `p/i`
verified all gates/ENABLE/MOE/CCRs0,nFAULT1. Physical shaft direction was not
observed by the agent. Do not launch another powered run until the missing
terminal summary/possible foreground or IRQ starvation is understood and a
reliable independent stop path is established. Earlier old-direction E897
qualification does not transfer to reverse. Do not edit sibling rm32.

2026-09-18 OPERATOR DIRECTION CHANGE: the operator explicitly said not to
run this motor in the previously tested direction; reverse it first. The
next 45% attempt was only staged, NOT started. `off/p/i` confirmed gates,
ENABLE, MOE and all CCRs0, nFAULT1. Installed lean image SHA
C65941E07FA426BB02FA235BDBDB2DC147AC29EE376FBCCAD455CED9927E2DA5
still has the OLD direction and must NOT be used for any powered run. E897
old-direction lean 10->40%/35s had completed cleanly immediately before the
new instruction; its qualification does not transfer to reverse. Implement
and verify a consistent reversed phase mapping across forced sine/six-step,
powered role commutation, comparator mux and flying acquisition before any
powered test. Re-establish startup/low-duty operation first, then requalify
prior regimes as needed. Do not treat changing only one layer as reversal.

2026-09-18 E896 COMP-TOP A/B RETIRED. CURRENT installed/OFF after E896 is
protected original-priority control rebuild SHA
F7E9DF6F164286A6AE0E051AC99761E154B000099157F297023A06E4CD6901C7;
verified/flashed correct G071 SN066CFF343433464757233430, reset device0x460,
`off/p/i` all gates/ENABLE/MOE/CCRs0,nFAULT1. No powered run on this exact
restored hash yet. Opt-in `bench-running-comp-top` changed only COMP priority
0x40->0; COM/DMA stayed0x40, TIM6 guard0, all electrical stops active.
Candidate SHA8F427E663547E5F68719B9837970D3E6EEE28E5C247D4E56E1C29B5AA460D8B7
release-s/thinLTO/codegen1; four ISR-root arithmetic audits and disabled
guard3/18+role6/6 PASS. E896 10% low-duty gate stopped after3.717ms powered
with `POWERPATH reason=3` (TickGap), 6 COM; FASTBUS did not trip, BUSSCAN
16 CRC-valid frames retained, finaloff and nFAULT1. COMP sharing top priority
with TIM6 violates the existing guard-tick deadline in this setup; do not
relax TickGap or test this candidate at high duty. Capture
`captures/duty50_896_comp_top_tickgap_mcp.txt`. Both priority-only variants
are retired. Next work must address actual critical-section/ISR latency while
preserving fast bus/current authority; replacement-motor 50% remains untested.

2026-09-18 E894-E895 PRIORITY A/B RETIRED. CURRENT installed/OFF after E895
is protected control rebuild SHA
4DE8034FBE1CC42521923508882A92B811E1B444CEA55AF52148AC5473FFE661,
same E891 active feature closure after an opt-in priority feature was added
but NOT enabled; release-s/thinLTO/codegen1, text124976/data1208/bss24784.
Probe-rs verified the G071 image; Cube HWRSTPULSE identified device0x460;
`off/p/i` gates/ENABLE/MOE/CCRs0,nFAULT1. No motor run on this restored hash yet.
The one-variable `bench-dma-below-comp` candidate SHA
9FA46406FE005163E5E8D1D7FBDB558C7CA4982E4696C467A3614CEF3BFC1A11
changed DMA priority0x40->0x80 while COMP/COM stayed0x40 and TIM6 guard0.
Four ISR-root soft-arithmetic audits and disabled guard3/18 + role6/6 PASS;
COREPRIORITY confirmed64/64/0/128. E894 cap1 capture is PARTIAL after D85;
only2 controller COM and no sustained run, so it cannot validate priority.
E895 cap0 10% run stopped after6.808ms powered on `POWERPATH reason=11`,
`ADCFAULT stage=27`, `DMAFAULT code=7 service_gap_us=227`, with FASTBUS0
and terminal bus normal; outputs safed, nFAULT1. Source audit: code7 is
`dma_snapshot::Lease::finish` rejecting a transfer that changed while the
handler copied its frame, not a mere foreground stale timestamp. Lower-priority
DMA let another scan overtake service in this setup and is NOT a viable
high-duty fix.
Do not reflash this candidate or interpret its 10% failure as a motor/envelope
limit. `scripts/drv_bus_scan.py` now permits CRC-valid BUSSCAN-only reports
without an optional IT86 tail (3 host tests PASS). Captures duty50_894/895
retained. Next investigate high-duty control timing without starving safety
feedback; no 50% replacement-motor attempt yet.

2026-09-18 E892-E893 CURRENT installed diagnostic CPU image SHA
C4755AEA2B43766B0E1A3B27C401159533FAD963A87868CED895532C4862FF5A,
release-s/thinLTO/codegen1, text127372/data1200/bss23244, active4A signed-
current actuator, fast relative bus sag, absolute bus, nFAULT, tracking and
watchdogs. Four motor ISR-root soft-arithmetic audits PASS. Disabled cpucheck
software pair costs2/7/10us, guard3/18 and all six role checks PASS. E892
diagnostic live ramp10->35% stopped FASTBUS reason26 at powered24.595469s;
postrun `cap0` omitted CPU/FASTBUS detailed dump, so this run proves a 35%
fast-sag stop on this observer-affected image, not its CPU occupancy or causal
mechanism. E893 repeated at10->30% with cap1: CPUUNION CRC-valid20.000009s,
IRQ union10.975258s=54.88%, roots guard0.700692s, COMP5.053882s,
COM2.958670s, DMA2.262014s; meter fault0/max_gap93us. FASTBUS tripped0,
current max residual5688/17350, no rail or absolute-bus low codes. Serial MCP
capture lost some later postrun summary lines amid the large dump, so do not
claim a complete reason/DONE record for E893. Both runs ended with explicit
`off/p/i`: gates/ENABLE/MOE/CCRs0, nFAULT1, UART MCP COM41 working. Captures
`captures/duty50_892_cpu_replacement_35pct_mcp.txt` and
`captures/duty50_893_cpu_replacement_30pct_mcp.txt`. The 54.88% aggregate at
30% does not support CPU saturation as the immediate 43-45% wall, but says
nothing definitive about worst-case comparator latency. Do not transfer
diagnostic-image stops or passes to lean qualification. Next: inspect timing
and bus ordering/latency on protected control images; do not raise duty or
loosen fast-sag/current guards merely to force50%.

2026-09-18 E889-E891 43-45% FAST-SAG BOUNDARY; board CURRENT installed
E891 diagnostic `bench-fast-bus-tail` SHA
3041DED8ED1458C2618C65B53FC3626BE4C5F39D7A76F6E8D0322E13032C6EB1,
release-s/thinLTO/codegen1,text124976/data1208/bss24784,off after E891 with
`off/p/i` gates/ENABLE/MOE/CCRs0,nFAULT1,serial MCP COM41 working. It retains
active4A signed-current actuator, fast relative sag, absolute bus,nFAULT,
tracking/watchdogs; 16-scan recorder is diagnostic only and adds DMA work.
Four motor ISR-root soft-arithmetic audits PASS; disabled guard3/18 and role6/6
PASS. E889 kept original protected image and slowed40->45 to1%/s. It still
stopped at45 after ~10s: exact POWERPATH reason26@40.450052s powered,
FASTBUS1,cause6 bus1129/vref1499 vs baseline1210/1506 (~6% dip),max current
residual11600/17340,0/127 tail watch violations. Thus immediate5%-step
transient alone is falsified. E890 first bus-tail image stopped at44 before a
45 ACK;16 CRC-valid scans show bus98..100% for9 scans,then96..97%,then
93.81/94.61/94.53% on final3; duty440 throughout. E891 added final accepted
event timestamp and exact reason to early BUSSCAN header, then stopped at43
before44 ACK: BUSSCAN stop_reason26 stop_us41003959,last_event_us41003885,
FASTBUS1/cause6,baseline1212/1507, all16 DMA scans and128 IT86 events CRC
valid. Host alignment: normal bus99.06%@41001630; first large accepted gap
135us at41001735 (normal~80..86us); next bus sample97.11%@41001856;
sub95% scans at41002986 then41003438/03664/03890 with one intervening
95.81% scan; final3 consecutive lows trip. Repeated late/short event gaps and
interval-average growth appear before the first *observed* major bus dip,
supporting a timing-first hypothesis. A bus dip arising unseen in the226us
scan gap is NOT excluded; causality is not proven. Phase-shunt raw rails
coincide with sag but are asynchronous PWM samples, not a calibrated DC current.
E891 current residual max11382/17308 (<4A nominal), VREF steady, absolute-bus
block low-code0. Retained captures duty50_889..891; reports E890/E891 are
PARTIAL after D85 but early BUSSCAN/FASTBUS/IT86 are complete. No further
high-duty run until CPU/scheduling headroom and this control fault are studied;
do not loosen fast-sag or current guards. E891's43% threshold is diagnostic-
image-specific and not a lean envelope claim. Replacement-motor50% unattempted.

2026-09-18 E882-E888 REPLACEMENT-MOTOR ENVELOPE; CURRENT installed image SHA
7856455A0F85D8320DF1D4405B7E5339D3052C56B7AF5730B369BF7BE4B82AFF,
release-s/thinLTO/codegen1, text124436/data1208/bss24520, fast bus sag and
active4A current foldback/stop, no injection or report-only feature. The only
source addition vs E881 is post-run DRIVENENTRY refusal telemetry (no ISR work);
four motor ISR-root soft-arithmetic audits PASS. Board verified G0710x460 and
last E888 `off/p/i` all gates/ENABLE/MOE/CCRs0, nFAULT1. Serial MCP COM41
working. E882/E883 live-armed attempts released acquisition successfully
(reason22) but transfer returned2 before any powered event; one complete E883
report retained, cause not yet isolated. E884 live-armed transfer succeeded and
10->15->20% completed30s, final ~1051eHz, busmin11.557V. E88510->25%/45s
PASS, final tail cycles776..811us, 0/127 speed-watch violations, max current
residual5861/17282, busmin11.581V, FASTBUS0. E88610->35%/45s PASS, final
cycles579..629us,0/127 watch violations,max residual6712/17340,busmin11.713V.
E88710->40%/60s PASS, final cycles512..550us,0/127 watch violations,max
residual11415/17251,busmin11.737V. These are diagnostic-envelope runs, not
lean repeated-hold/restart qualification. Full MCP captures duty50_882..887
retained. E888 live ramp ACKed45%, then stopped near27.397s energized;
FASTBUS tripped1, AVGDIAG cause6 bus1123/vref1501 against same-wake baseline
1211/1506 (>5% normalized drop), max current residual11258/17261, slow
absolute-bus counter0. The IT86 suffix is CRC-valid, 20 cycles474..546us,
last five include541/482/518/541/546us; a timing wobble/downturn appears
near the sag but ordering/cause is not proven. E888 MCP report is PARTIAL:
serial capture ends mid-D85 before POWERPATH/DONE, so do not claim their exact
codes; `off/p/i` afterward proved all outputs disabled/nFAULT1. No50% command
has been sent to replacement motor. Next: distinguish transient 5%-step surge
from sustained45% control defect with a slower ramp and complete near-fault
capture before advancing. Keep the fast stop and current actuator active.

2026-09-18 E879-E881 FAST BUS-SAG BRING-UP. CURRENT installed E881 is the
non-injecting `bench-fast-bus-sag` image SHA
6DEDEF9973BF77B258EDFCEAB6301E02A5146D83A91F207EE919AD370901B44F,
release-s/thinLTO/codegen1, text123940/data1208/bss24520, four motor ISR-root
soft-arithmetic audits PASS. It is the E867 feature closure minus
`bench-current-report-only`, plus active 4A current foldback/terminal stop and
fast relative bus stop. Flash verified on G071 device0x460; after reset `off/p/i`
show all gates, ENABLE, MOE, CCRs off and nFAULT high. E879 disabled guard3/18,
role6/6 and basecheck4/4 PASS. E879 replacement-motor 10% BEMF/5s PASS:
`DRIVEX result=1`, powered reason2 at5.000005s, FASTBUS baseline1212/1506,
tripped0, residual max2732<allowance17272, bus minimum11.617V, final off.
Full retained report: `captures/duty50_879_fastbus_10pct_mcp.txt`. A second
waveform-only command after the one-shot transfer was consumed is NOT a BEMF
result. The diagnostic `bench-fast-bus-inject` image E880 SHA
83A00DB0044ECED8A40D064883DB19712F36DB09A544B2951045281E7735EAB2 was
flashed only for a single low-duty validation: exactly three synthetic low bus
codes at powered DMA scans9000..9002 caused reason26 at2.034550s, with gates
and ENABLE off. Acquisition-stamp-to-completed-off age was85us on the third
scan; nominal first-low-to-off ~537us, with226us scan cadence. Actual analog
collapse detection latency is not measured. Capture:
`captures/duty50_880_fastbus_inject_10pct_mcp.txt`. E880 `p/i` after trip were
all outputs off/nFAULT high. E881 then restored the non-injecting image and
verified idle off; the injection feature is diagnostic-only and MUST NOT be
used to qualify duty. Fast-sag reaction path and ordinary 10% false-trip check
are now evidenced; 10% repeated-hold qualification and the larger replacement-
motor envelope remain incomplete. Do not count E872 old-motor 50% as qualified.

2026-09-18 E878 FAST BUS-SAG GUARD STAGED ONLY; NOT FLASHED OR POWERED.
Operator ruled the connector sound and prioritized a >5% sharp bus drop from
the pre-drive set-voltage level, with no one-sample ADC-noise trip. Added opt-in
`bench-fast-bus-sag`: same-wake 128-scan bridge-off bus/VREF baseline, fixed
ratio threshold at <95%, three consecutive coherent DMA scans required at the
226us cadence. The third low scan takes the existing DMA-side terminal path:
gates and ENABLE off, latched reason26; 50-scan absolute bus guard remains.
Poststop FASTBUS reports baseline codes, threshold, cadence and trip flag;
AVGDIAG cause6 retains final raw bus/VREF/vcal. An invalid baseline refuses
startup; one zero bus code is treated as a possible notch and needs the same
three-scan persistence, while invalid VREF remains fail-closed. The feature cannot
compile with driver-fault-probe (which bypasses average guards) or report-only
current; the candidate restores active current foldback/stop. Pure guard tests
3/3 PASS; E867-closure-minus-report-only-plus-fast-sag release-s/thinLTO/
codegen1 build PASS, SHA2133F99DBDB3BBF81E34811E803D3489A64D78422035B281BA36875F0114358F,
text123940/data1208/bss24520; four ISR-root reachable soft-arithmetic audits
all0. Expected sample confirmation <=678us plus ADC completion/IRQ latency is
NOT a measured hardware reaction-time bound. No terminal E872 drive samples
exist to replay. Installed board remains E867/OFF after E877; no flash or
motor command this entry. The memo `GRAYBEARD_BUS_COLLAPSE.md` was read fully:
its current-report-only finding is valid and fixed in the candidate; its
"running average tracks collapse" description is false for the existing fixed
absolute 50-scan guard, and late-commutation braking remains a hypothesis.
Next: disabled preflights, measured ISR/stop timing, and low-duty false-trip
validation before any high-duty use. Do not treat build/tests as protection
qualification or resume the 5-50% climb on this basis alone.

2026-09-18 E877 POST-INCIDENT GENTLE RECOMMISSIONING PASS, NOT CAMPAIGN
QUALIFICATION. After the operator power-cycled the boards and installed the
replacement motor, serial MCP COM41/115200 recovered and the existing E867
shell answered. The staged E876 safe-UART image was therefore NOT flashed;
E867 remains installed. Initial `off/p/i` proved all six gate commands,
ENABLE, TIM1 MOE and CCRs zero with nFAULT high. The authorized test retained
6.2% forced startup and capped the BEMF segment at exactly10.0% for5s. MCP
round-trip latency initially left live startup ramps at150/190eHz; those were
open-loop transport/timing attempts and are not BEMF results. A local MCP
macro then required ordered F110..F200 acknowledgements before the4.7s
handoff. Final result: `DRIVEX result=1 power_reason=2 fresh_transfer=1`,
`POWERPATH reason=2 stop_us=5000005`,16008 powered commits, event_fault0,
DRIVENIRQ overruns0, and bus minimum11.617V. It completed by the commanded
deadline, not current, low-bus, nFAULT or tracking protection. Final explicit
`off/p/i` again proved gates/ENABLE/MOE/CCRs0 and nFAULT1. This establishes that
the replacement motor, G071, UART, DRV enable/gates, BEMF handoff and gentle
powered path are operational. It does NOT identify the E873 failure, clear the
old motor, qualify the new motor at10%, or authorize resuming the5-50% climb.
The serial MCP connection remains open; stop if MCP becomes unavailable.

2026-09-18 E876 safe-UART isolation STAGED, FLASH FAILED BEFORE PROGRAMMING.
Operator proposed a minimal UART echo reflash. Added `examples/uart3-safe-echo.rs`:
release-s/thinLTO/codegen1, text7920/data16/bss912, SHA
78D74B3C70A6EDDAEC24470FA0FA6446FA3B59929940FE7A63A52953BC9689E4.
It makes PA7..10,PB0..1 and PD1 GPIO outputs low before USART3, has no PWM/
timer/ADC/comparator/motor path, prints SAFE_UART_READY+nFAULT and uppercases
echo input. ELF standalone LOAD08000000/reset080000BC; unresolved M0 helper
scrub found no __aeabi/memcpy/memset. Explicit probe SN066C...3430 was selected,
but `probe-rs download --verify` returned repeated SwdApWait and failed while
connecting, before erase/program/verify. Therefore safe echo is NOT installed;
last installed image remains presumed E867 (not freshly readable). Combined
with MCP COM41 zero-byte replies, target is unpowered or electrically
unreachable. Stop hardware access and await operator power/connection check;
do not use another probe, blind reset, Python serial, or motor command.

2026-09-18 E875 recommissioning PAUSED. Operator identified the old motor as
the principal hot/burnt item, replaced it, and authorized one gentle <=10%
operational spin. Serial MCP was initially Transport-closed; operator requires
that as a hard blocker and explicitly forbids Python-serial substitution. After
MCP refresh, list_ports found COM41 FTDI and MCP opened it at115200. Two idle
attempts (`off/p/i`, first CRLF then CR-only) each returned ZERO bytes. Port was
closed. No `run`, ENABLE, gate, flash, or motor command was issued. Current
blocker is target/UART silence, likely unpowered or unresponsive board; await
operator confirmation of board/PSU power. On resume use MCP only, require sane
`off/p/i` (all outputs/MOE/CCRs/ENABLE off and nFAULT high) before the <=10%
spin. Do not infer hardware health from COM41 enumeration alone.

2026-09-16 E873 PHYSICAL INCIDENT / HARD STOP. Immediately after E872, the
operator powered the bench off and reported "something burnt." Treat the board,
driver, motor, wiring, connectors and supply path as potentially damaged. The
earlier E872 fixture readback (ENABLE/MOE/CCRs/gates off, nFAULT high) is only
the last pre-incident observation; it does NOT prove present health or a safe
state after the burn report. Do not open serial, attach USB/ST-Link/UART, flash,
or apply bench power: auxiliary cables can back-power damaged rails. E872's
complete low-bus block is now the telemetry immediately preceding a physical
incident, not merely a supply-fold data point. Preserve the setup and capture
photos before moving wiring once everything is cool. Resume only after a
power-off physical inspection identifies the burnt part/path and establishes a
safe re-energization plan. 50% remains reached but unqualified; the full5-50%
goal remains incomplete. Offline E873 decode validates the capture CRC/framing.
The246 drive records cover startup ticks19..4674ms only and say nothing about
the terminal event. The2kHz coast capture begins0.506ms after disable and sees
VBUS recover8.796V@0.506ms,9.628@1.010,10.645@1.514,11.371@2.018,
11.801@2.522 and12.023@3.025ms, then settle near11.6..11.8V. This independently
corroborates a real short-duration bus collapse at shutdown, but cannot
distinguish PSU limiting, leads/connectors, bridge or motor. See
HARDWARE_INCIDENT_2026-09-16.md.

E874 operator localization: the motor was the only component that became hot
and appears to be the burn source. This makes motor winding/lead/connector
damage the leading physical hypothesis, not proof. Keep all power and auxiliary
cables disconnected. Next evidence is a cooled, motor-disconnected inspection:
connector/lead damage, free rotor movement, relative AB/BC/CA resistance, then
each phase-to-motor-case isolation. Do not re-energize to "see if it still
works." Low phase resistance may be below an ordinary DMM's absolute accuracy;
equality and stable readings matter more than the displayed ohms.

2026-09-16 E862-E872 control-defect campaign. CURRENT board is E867 SHA
A910E9DE9B8193C9201EE0C4861D6D86BCF526DBA96A4B0CA075872E15760F45,
last known installed and firmware-OFF after E872, but physical health is now
UNKNOWN after E873; release-s/thinLTO/codegen1, text122912/data1188/
bss24512. Operator raised physical PSU limit to4.5A and resumed powered work,
explicitly treating amperage as diagnostic headroom rather than the presumed
control fix. E849 already contained an older aggregate-CPU run, contrary to
stale side advice, but adaptive current had folded35->25. New E862 matched
E861 fixed-advance20/current-report/speed-watch control, removed tail/hist and
added aggregate accounting. At ramped31% it stopped Tracking8 before35;
observer-affected IRQ union was53.98%, roots guard3.58/COMP24.04/COM15.43/
DMA10.94%, leaving foreground45.99%. Thus CPU saturation is not the immediate
31% fault. E862 SHA E522191F... is frozen; its result cannot qualify lean code.

Restored exact E861. E86531%/60s PASS: IT86 gaps95..143us, cycles705..728us,
watch limit348us,127 transitions exact/0 violations. E86635% reproduced the
defect TWICE in one window (normal restart after first): final suffix had normal
95..136us gaps, then one accepted332us gap against311us limit, then no next
edge and Tracking8. No current fold/bus/nFAULT/fast-cycle fault. This proves an
intermittent comparator-event dropout, not gradual CPU starvation or PSU cause.

E867 adds opt-in `bench-running-level-revisit`: after the real half-interval
gate, with owner active/unmasked, hardware enabled, no EXTI pending and the real
comparator already post-ZC, foreground may pend ONE normal ADC_COMP service per
controller command. Normal persistence reads, real timer timestamp, controller
acceptance and all guards remain; no ZC/commutation/deadline is synthesized.
Pure fail-closed policy4 PASS; CPU/speed-watch/IT86 tests12 PASS; revisit caller
and four ISR-root M0 helper audits PASS. E868/E86935% each60s PASS with11684/
11528 revisit accepts; E87040%60s PASS with13864; E87145%60s PASS with17632.
All attempts equaled accepts, about2.4..3.0% of events, proving active hybrid
edge+level rescue rather than a no-op. Final suffixes tight, zero watch faults;
no restarts/foldback/bus/nFAULT. This clears the old35-45 control wall on the
diagnostic image but is not lean qualification or stock-AM32 parity.

E872 reached/ACKed50% and ran roughly23s at target, then the independent
50-scan aggregate bus guard stopped reason25 at powered51.022318s. Cause5 final
raw bus/vref/vcal863/1508/1662, current max residual18812 vs allowance17308, no current actuator,
nFAULT or Tracking8. Final tail remained watch-clean but cycles slowed646..665us
(~1.52keHz) versus45%515..565us (~1.9keHz), consistent with loss of bus authority.
E873 then made this a physical-damage stop: the operator powered off and
reported something burnt. Do not run powered attempts or reconnect auxiliary
power until the damaged element/path is identified. 50% was reached, NOT
qualified. E872 final p/i had all gates/en/MOE/CCRs0,nFAULT1 and UART closed,
but that historical readback does not certify present hardware. Exact captures
duty50_863..872 retained. Do not edit sibling rm32.

2026-09-16 E861 CURRENT board SHA
8CA10D96ACFF5C9CA00F64B9E0241084AD222B1F516EF92713C871EBE97CBCF4,
installed/OFF/UARTclosed, NO powered run. Corrects E858 evidence gap with
versioned IT86-v2: each accepted event records gap, measured this_zc, actual
foreground S.average_interval handed to E857, and pre-accept event deadline.
Post-run dump only. Diagnostic mirror adds one atomic store; tail adds2 RAM
stores/event. Exact E857 feature closure rebuilt release-s/thinLTO/codegen1:
text122212(+152),data1188(+8),bss24488(+512); Recorder +16 emitted bytes.
Full helper audit clean in Recorder and transitively from ADC_COMP/TIM16/TIM6/
DMA roots. Speed policy4 + IT85/IT86 decoder/transition5 host tests PASS. Flash verified
on explicit G071 SN/device0x460. Disabled guard3/18 and role6/6 PASS; final p/i
all gates/en/MOE/CCRs0,nFAULT1. Frozen duty50_861_interval_tail_v2. E855 hard-
fold pause remains; first eventual powered action is known-clean control, not
ceiling climb. Candidate is diagnostic, not powered-qualified.

2026-09-16 E859 SOURCE AUDIT only; board unchanged E857/OFF/UARTclosed.
Read full GRAYBEARD_TRANSIT_SURGE memo and checked its proposed "step on the
estimate" lever against cached AM32 G071 source. Literal continuous stepping is
NOT stock behavior: `interruptRoutine` accepts one ZC and calls
SET_AND_ENABLE_COM_INT(waitTime+1) (AM32/Src/main.c:945..972);
`PeriodElapsedCallback` immediately disables COM interrupt then performs one
commutation (:919..940). With no next accepted edge there is no next normal COM;
foreground only re-kicks through the 45000-tick BEMF timeout (:2663..2678).
Local AM32 diff changes board/UART/telemetry plumbing but not these three core
paths. Do not implement an unbounded predicted free-run under an AM32-parity
claim. Memo's diagnosis remains useful; interval ring already proves cascade,
persistence-depth and advance levers were already A/B'd. Next powered action
remains E857 known-clean control after hard-fold pause resolution.

2026-09-16 E858/E860 OFFLINE correction; board remains E857 SHA
9D8CD681636F59BCFBFBF4BA2F531972447891C5EB9D2477480E3E087EAF1520,
installed/OFF/UARTclosed. Initial E858 replay was INVALID and its exact trip
table is retracted: IT85 `reference_half_us` is EV_ACC `this_zc`, the newly
measured interval, while E857 tightens from foreground `S.average_interval`
(six smoothed sector slots). The field is nearly2*gap and cannot substitute.
Tool is corrected to name `measured_interval_half_us` and only report strict
ordinal-aligned six-event electrical-cycle sums; 3 tests PASS. Valid retained
results: E832 cycles624->999us, E834712->1166(max1208), E836548->589 with two
isolated bad cycles max851, E841678->1049, E843750->1086(max1091). This retains
the cascade/transient classification but proves NO E857 trip time or false-trip
rate. Clean E80940%/60s and E85431%/60s also omitted the needed average/event
tail. First eventual powered use remains a known-clean E857 control after the
hard-fold pause, not another ceiling climb.

2026-09-16 E856-E857 CURRENT board E857 SHA
9D8CD681636F59BCFBFBF4BA2F531972447891C5EB9D2477480E3E087EAF1520,
installed/OFF/UARTclosed, no powered run. E856 preserve-mux-pending candidate
was falsified disabled: forced COMP output transition while masked did not set
EXTI18 RPR/FPR/pending; do not powered-test or promote it. Existing E832
interval tail already proved the high-duty failure is a ~20-cycle growing,
sector-structured slowdown, not a lone late/early pair; E850-E855 already
falsified rising persistence burden and tested fixed advance. E857 therefore
adds opt-in tighten-only missing-event protection: fixed1000us stale limit may
tighten to3 controller event periods, bounded200..1000us, and cannot loosen as
the estimator slows; uninitialized values below64 cannot tighten. Existing128-event tail is enabled. First saturating-math
draft emitted __aeabi_lmul in Recorder and was retired before flash; bounded
const replacement product<=1998 has no helper in Recorder/watch, four ISR-root
math audits PASS. Host speed-watch policy PASS; broad536 suite retains5
unrelated pre-existing brittle failures (bus_scale extraction, follow/adapter
stubs, whitespace assertion), so no whole-suite pass claim. Disabled hardware
guard3/18 + role6/6 PASS; p/i all gates/en/MOE/CCRs0,nFAULT1. E855 hard-fold
pause remains: no motor until this safety candidate is deliberately exercised.
First Cube reset without SN reset a separate F411 only (no flash); corrected
SN066C... reset identified intended G071 device0x460. Physical PSU cap4A.

2026-09-16 E850-E855 CURRENT board E855 SHA
3BD142AB79C8AEF2FC05C08458D3DE2B61B246E051BD9FF49771D28EBC1C4B30,
installed/OFF/UARTclosed. Physical PSU cap4A. Fixed advance20 remained the
best bounded timing point, but E851 with normal3.5A adaptive protection folded
near the high30s and stopped on a second nominal-current block. New opt-in
`bench-persistence-hist` counts accepts and persistence rejection indices per
sector for the current live-duty epoch; u32 revision E853 adds52bytes ADC_COMP
and16bytes accepted recorder versus E850, no calls/soft math in four ISR roots.
Matched E853: stable10% had7.4..9.4 rejected passes/accept with filter12;
stable29% had1.2..2.8 with filter6. E854 stable31% had1.16..2.46 with filter5.
Thus persistence rejection burden FALLS with speed and is not the rising-speed
wall; sector fingerprints remain real but require a fault-epoch comparison.
E852 u16 counters saturated and are retired.

E854 explicit nominal4.0A still folded once and settled31% for60s with zero
bus-low/nFAULT/tracking faults. E855 made only nominal signed-current excess
report-only; physical4A PSU plus bus-sag, raw validity, nFAULT, tracking,
deadline and watchdog stops remained. A1%/s ramp toward50 stopped at36% after
a complete50-scan/~11.3ms bus-sag block: reason25 at29.823280s, bus_low_codes35,
cause5 bus743/vref1506/vcal1662 versus963-equivalent8.4V floor. Final36% epoch
still had only1.01..1.28 rejects/accept. This is the operator-defined HARD PSU/
power-path fold pause condition. Do not run more powered ramps, weaken bus
guard, or claim50% until the physical supply/path event is resolved. Board is
safe; current image is diagnostic only, not qualification. Do not edit sibling
rm32/rm32_stm32.

2026-09-16 E832-E847 CURRENT board E846 SHA
9174CBDD2C28E897D1F34CEB64F3F0D3F24C9CF27CE3AF86D3D79AC2256F6E99,
installed/OFF/UARTclosed after E847. Physical PSU cap4A. E832 interval-tail
made the42-ish% event causal: final complete electrical-cycle periods grew
624..667us (~1.5-1.6keHz) to998..999us (~1.0keHz) over~20cycles before
self-clearing nFAULT (first-high92us). Accepted-event reference field equals
the measured interval and is NOT independent rotor truth. Six-event sums and
ordinal-mod6 sector positions prove sustained sector-structured slowdown, not
one late/early pair. Dynamic AM32 foreground persistence was already live
(filter5..8); old filter_reads12 is acquisition-only, so graybeard's static12
premise is false here. Fixed advance A/B on otherwise matched tail images:
advance16 E834 reached38 then collapsed to~0.83keHz/nFAULT; advance18 E832
reached39 then collapsed/~1.0keHz/nFAULT; advance20 E836 reached43, held a
tight~1.82keHz suffix, then two bad cycles triggered stock-AM32 desync before
nFAULT. Global advance22 failed startup twice with zero powered events. Added
opt-in minz-core scheduled advance: startup/<30%=18,30..34.9=20,>=35=22;
default minz remains16 and fixed features are mutually exclusive. Core cohorts
86/86 and four ISR-root M0 audits PASS. E841 original40% schedule never reached
22; E843 confirmed22 but still collapsed at39/nFAULT. Retiring interval tail
did NOT remove wall: E845 lean first hit old CycleTiming floor at10%; E847
using exact E804 exploration policy reached38, folded38->33 on real current
warning, then reason25 second-over stop; bus_low43, no nFAULT. Thus tail observer
is exonerated and current/sag remain downstream messengers. Current image has
real3.5A adaptive current/bus protection, report-only single-cycle metric,
scheduled advance, no interval ring/fault probe; finaloff verified. Next:
bounded aggregate CPU measurement near known35-40% regime, then choose ISR/
scheduling vs controller-timing lever. Do not edit sibling rm32/rm32_stm32.

E833-E847 tooling correction: STM32CubeIDE's HLA OpenOCD config recurses with
the installed0.12 binary; stlink-dap misidentifies the G071 asM4 and reports
bogus PC/MSP. Use STM32_Programmer_CLI `mode=HWRSTPULSE -hardRst` after
probe-rs download; it identifies device0x460/Cortex-M0+ and restores UART.
`live_armed_baseline.py` now accepts explicit --carrier-hz20000/24006 instead
of hard-coding20k. Failed captures are retained; no repeat-until-pass.

2026-09-16 E812 STAGED fast-block candidate, not yet flashed at freeze. After
user clarified external observation is never a stop unless hardware is
unreachable, E804 ran E807-E811. E809 reached/held40% for full60s with1%/s
ramp, zero foldback/low-bus/nFAULT/tracking/timing faults; final average193
half-us ~=1727eHz. E8101%/s toward45 reached44 then Driver7/nFAULT at38.099s;
simultaneous residual21670/15172 selected3% foldback but hardware won. E811
0.5%/s reached42 then Driver7/nFAULT at68.280s; residual17015/15172 selected2%
foldback but hardware won. Both finaloffPASS; slower ramp moving trip lower
rejects pure acceleration explanation and exposes11.3ms software warning vs
fast hardware trip. E812 adds only bench-current-fast-20:20x226us=4.52ms
complete current/bus block,20 unique24k phases,max gap278/2666ticks=4.35us;
same3.5A nominal/current/bus/nFAULT/tracking/watchdog thresholds and adaptive
1..5% foreground actuator. SHA04A111BD573DE5CA5B45712E5BF4528C9A311893727F2F68355143A0AF98FC3A,
text121492/data1180/bss23156,release-s/thinLTO/codegen1,four-root M0 auditPASS;
29-policy-test feature cohortsPASS including fast20. Board currently E804/OFF/
UARTclosed after E811. Next flash E812, disabled guard3/18+roles2666, then a
bounded1%/s45 exploration; do not change hardware thresholds.

2026-09-16 E805/E806 STAGED ONLY, NOT FLASHED/MOTOR-QUALIFIED. They supersede
staged fixed1% E800/E801 so all future images match E804 adaptive protection.
E805 lean SHA39FF82CD266582473B2FFEAC99578273D46EEA712BD39C83DD6E23C585AE03CE,
text120672/data1180/bss23124, exact E800 feature set. E806 aggregate CPU SHA
CABD2BB3463E87CEAD66A25A235194DA2E8E69EAFFED4DB79AF6958B3FA1DB20,
text124608/data1180/bss23192, exact E801 feature set. Both release-s/thinLTO/
codegen1 and four ISR-root M0 audits PASS. E805 omits current-max/fast-event/
tail diagnostics and is eventual10/25/50 qualification only. E806 adds only
aggregate CPU accounting, requires disabled cpucheck, and cannot qualify E805.
Board remains E799/OFF/UARTclosed. Physical E803 fold pause still controls: do
not flash or power E804/E805/E806 without operator response/resumption.

2026-09-16 E804 STAGED ONLY, NOT FLASHED/MOTOR-QUALIFIED. Frozen exact-E799
diagnostic successor SHA389B0BFE9AC8848C849B891F32883BC0B246DE85DF963312D4BB8F9540A8D10D
at captures/reference/duty50_804_adaptive3500/shell-pwm.elf; text121492,
data1180,bss23156,release-s/thinLTO/codegen1. E803 falsified fixed1% foldback:
one1.853x nominal block received only1% reduction and bus collapsed before a
second correction. E804 retains the actual first-over residual and uses
division-free severity bands:1/2/3/4/5% at <=110/125/150/175/>175% allowance,
bounded at5%. DMA only latches magnitude; foreground still owns coherent PWM
publication/acknowledgment; all electrical thresholds and the unacknowledged
second-over terminal stop are unchanged. Policy/wire tests PASS at30/50 feature
ceilings; average policy9PASS. Four ISR-root math audit PASS and new record path
has no uidivmod. Board remains installed E799/OFF/UARTclosed after E803. Do NOT
flash/power E804 until operator corroborates/resolves the E803 hard bus fold and
explicitly resumes; the candidate does not waive the pause.

2026-09-16 E803 CURRENT board E799 SHA
4B151FC2660693F68BB5E047E61FCDB6FC1E412C4CBD878E65E411E9F7056A02,
installed/OFF/UARTclosed. Operator set physical PSU limit4.0A; firmware nominal
signed-average ceiling remains3.5A, leaving0.5A supply headroom. Flash/reset,
disabled guard3/18 and all six2666-tick role checks PASS. One smooth1%/0.25s
ramp toward40 retained. Status ACKs reached38%; next39% command arrived after
stop and was refused !busy, so40 was NOT reached. Firmware first reduced38->37
on one average-current warning, then independently stopped reason25 at powered
10793412us because the bus-sag guard's complete50-scan/~11.3ms mean fell below
the8.4V floor. Diagnostic cause5 fault sample bus656/vref1506/vcal1662 is about
6.3V instantaneous-equivalent; bus_low_codes41. Postrun POWERFEEDBACK busmin
11677mV is only the handoff seed: bench-lean-irq deliberately omits its ongoing
FEEDBACK_N/BUS_MIN updates, while average_current_live still consumes every
coherent DMA frame. It therefore cannot invalidate the complete low block. No
nFAULT/tracking/fast-cycle/event fault; commitmax8us;
final gates/ENABLE/MOE/CCRs off and nFAULT high. Capture
captures/duty50_803_e799_smooth40_30s_01.txt. Per operator rule, PAUSE powered
campaign on this hard power-path fold signature; do not retry/raise firmware
threshold/current or continue45/50. Ask whether PSU display showed CC/voltage
collapse. Stock-AM32 matched-speed evidence is still required before assigning
the limit to motor/power stage rather than supply/leads.

2026-09-16 E802 OFFLINE HANDOFF, NO FLASH/MOTOR COMMAND. Serial MCP is restored:
list_ports finds COM41 FTDI; COM41 opened at115200, an idle150ms read returned
zero bytes normally, and close succeeded. No UART data was written. Frozen
E799/E800/E801 SHA256 values were rechecked and match their READMEs/handoff;
the previously missing E799 README was added and GOAL_5_50_AUDIT now says
current through E801. Board authoritative state remains installed E7972.5A,
last verified OFF/UARTclosed. The sole next physical gate is operator
confirmation that PSU current limit is set to3.5A. Only then flash E799, reset,
run serialized guard3/18 plus all-role2666 disabled preflights, and attempt the
smooth1%/0.25s ramp to40%. Do not flash E800/E801 first and do not issue a
powered command before that confirmation.

2026-09-16 E801 STAGED CPU DIAGNOSTIC ONLY, NOT FLASHED/MOTOR-QUALIFIED.
Frozen SHA CA33BD31B860DA7EF65A9E37F100C9B7F3341D10B5AD462BCCCBEF5B117A40C3
at captures/reference/duty50_801_cpu3500/shell-pwm.elf. Control/protection
matches staged lean E800; new bench-cpu-aggregate permits lean-core/lean-irq
bodies plus only nested IRQ union/root RAM accounting and poststop dump. No
event/tail/current-max recorder or live UART. Text124332,bss23188 vs E800
text120396,bss23120; four motor ISR-root math audit PASS. Accounting adds
entry/exit critical sections+clock reads, so runtime is observer-affected and
requires disabled cpucheck cost plus CPUUNION/CPUROOT fault-free output. Never
transfer E801 envelope qualification to E800. Board remains E797/OFF; do not
flash until physical PSU3.5A confirmed and E799 exploration justifies the rung.

2026-09-16 E800 STAGED LEAN ONLY, NOT FLASHED/MOTOR-QUALIFIED. Frozen SHA
3E6BD0C7217F95BFC4EBD1947DC7A33E6EAEDD142CAB66342B56A56A504C2CD9 at
captures/reference/duty50_800_lean3500/shell-pwm.elf. Same3.5A/advance18/
1%-governor control as E799 but omits average diagnostic, fast-cycle report,
event100 and per-event/tail recorders; lean-core/lean-irq retained. Text120396,
bss23120 vs E799 diagnostic text121224,bss23152. Four motor ISR roots math-audit
PASS. Generic IRQTRACE header formatter remains, but lean selection produces no
IRQ rows; runtime strict lean verifier remains mandatory. E800 is intended for
eventual10/25/50 repeated hold+normal-start recovery qualification only after
E799 diagnostic exploration establishes50 safe. Board remains installed
E7972.5A/OFF/UARTclosed; neither E799 nor E800 may be flashed until operator
confirms physical PSU3.5A.

2026-09-16 E799 STAGED ONLY, NOT FLASHED: supersedes staged E798. Frozen
diagnostic/exploration SHA4B151FC2660693F68BB5E047E61FCDB6FC1E412C4CBD878E65E411E9F7056A02
at captures/reference/duty50_799_governor3500/shell-pwm.elf. Same explicit
nominal3.5A,advance18,release-s/thinLTO/codegen1 and four motor ISR-root math
audit PASS. Current governor now lowers its learned live ceiling1% per warning,
not5%; after PWM publication the exact current policy acknowledges/rearms the
first-over streak, permitting repeated bounded reductions. Two complete over
blocks before acknowledgment still latch terminal current fault and cannot be
cleared. Actual policy tests cover repeated acknowledgment, late under-limit
arrival, terminal non-clear, invalid values, and2500/3500 feature builds.
Fixture accepts historical step50 and new step10 summaries. Board remains
installed E7972.5A/OFF/UARTclosed. Do NOT flash/power until operator confirms
physical PSU limit3.5A; then E799 guard3/18+role2666 preflight and smooth40%.

2026-09-16 E798 STAGED ONLY, NOT FLASHED: explicit nominal3.5A/foldback
candidate SHA5C2AC6F5D9589F14A8E4716A212A6A5662F88509E84B881E2EC3855AC84638D8
at captures/reference/duty50_798_advance18_3500/shell-pwm.elf. Release-s/
thinLTO/codegen1, four motor ISR-root soft-arithmetic audit PASS. Added explicit
bench-current-3500 + bench-current-foldback-3500; host fixture accepts exact
3500mA ACK; actual policy modules compile/test at both2500/3500 high-range
settings. bench-duty-50 now requires foldback policy but not one fixed current
setting; campaign builds must explicitly name their nominal current. Current
physical PSU remains unconfirmed at3.5A and board still runs installed E797
2.5A image, last verified OFF/UARTclosed. Do NOT flash E798 or issue powered
commands until operator confirms PSU current limit3.5A. Then flash, reset,
guard3/18+role2666 preflight, and use smooth1%/0.25s ramp; first bounded rung40,
not an immediate50 jump.

2026-09-16 E797 CURRENT board SHA
F386E95EDB1F185334DBCFDFF5DC4701D482C5C2D85EEE4433FF2C259B0261C7,
installed/OFF/UARTclosed. Release-s/thinLTO/codegen1; four motor ISR-root
soft-arithmetic audit PASS; disabled guard3/18 and all six2666-tick roles PASS.
Opt-in minz-core feature sets compile-time advance18; default remains16. Added
postrun LIVEPARAM and maximum50-scan residual only; no live ISR recorder.

Smooth host ramp (1% every0.25s) removed the apparent35% wall. E797 35%/30s
PASS deadline: average214 half-us (~1558eHz),234395COM,filter5,advance18,
max residual7303/10817,no foldback/rails/bus-low/fast-cycle/tracking fault,
busmin11748mV,finaloff. E797 40% target reached, one max residual11063/10824
(2.2% over nominal2.5A allowance) caused one correct40->35 foldback, then full
30s deadline; busmin11605mV,no rails/bus-low/fast-cycle/tracking fault,finaloff.
This replaces the old inference that35% was a steady hardware wall: coarse5%
acceleration steps produced transient current/nFAULT. 35% is now a strong
exploratory pass, not yet an instrumentation-free repeated qualification.
40% is limited by the configured2.5A software ceiling, not BEMF or hard PSU
fold. Operator has been asked to set PSU3.5A before a matching3.5A firmware
guard and further powered work. Do not power again until that manual change.

E797 causal controls: advance18 with old coarse ramp still nFAULTed at35 after
6.76s, though max complete-block residual10397/10837 and no foldback;32.5 with
coarse final step folded32.5->27.5, max residual24489/10801. Therefore smooth
ramp is necessary in current evidence; advance18 benefit is not isolated.
LIVEPARAM proved sustained filter5-7 from the existing AM32 foreground map;
the older DRIVENIRQ filter_reads12 describes acquisition, not sustained lock.

2026-09-16 E791-E796 duty-50 campaign update: CURRENT board is E796 diagnostic
SHA28E3B2BA277D2651B4C41E8164EB4DEECD9189976E77334B1F46F87A207A5CF5,
installed/OFF/UARTclosed. Release-s/thinLTO/codegen1; DMA1/ADC_COMP/TIM16/
TIM6 motor-root soft-arithmetic audit PASS; disabled guard3/18 and all six
2666-tick role transitions PASS. PSU limit remains2.5A. No sustained bus-fold
signature was observed; if the PSU enters hard/current-limit fold, safe and
pause/report per the goal.

Envelope evidence: exact E791 20kHz 30%/20s PASS, no foldback, busmin11760mV.
E791 35% folded35->30 then nFAULT reason7 near6.09s powered, busmin11641mV.
E794 24kHz/209us folded35->30 then reason25 average-current stop near5.92s,
busmin11665mV. The209us cadence was proven biased over a50-scan block and is
retired, not a valid current-measurement improvement. Exhaustive integer phase
geometry selected226us:50 unique samples around2666ticks, maximum uncovered
arc60ticks (<1us), lower DMA rate. E795 exact226us still folded35->30 then
reason25 near7.60s,busmin11653mV. E796 diagnostic35% reached/held target but
nFAULT reason7 near5.90s; terminal11.3ms raw residual23445 vs10831 allowance
(2.16x nominal raw threshold),busmin11689mV,tracking current. nFAULT is aggregate,
so VDS OCP is leading, not proven unique: local schematic has R17 DNP leaving
DRV8304H VDS Hi-Z; datasheet maps Hi-Z to0.6V VDS OCP,4.5us deglitch/4ms retry.
This historical instruction is superseded by E797 above.

E796 midpoint32.5%/20s: target ACK and about1377eHz before one signed-average
foldback32.5->27.5; completed full20.000004s reason2 near1248eHz final
(average267 half-us ticks),126890 COM,tracking event_fault0,fast cycle/event0,
busmin11569mV,phase rails3,bus-low codes0,final residual3623<allowance10837,
finaloff verified. This is exploratory characterization, not32.5 qualification;
effective sustainable request on this attempt was27.5. Next work is causal
current/commutation-demand reduction or explicit hardware VDS configuration,
not threshold ratcheting. See DUTY_50_CAMPAIGN.md. E789 archive is INVALID:
failed build followed by stale binary copy; never flashed.

2026-09-15 ownership/goal correction after E778: this agent owns **binz only**.
Take the qualified bring-up controller from its current30% exploratory point to
50% commanded duty, then make evidence-based projections toward75%. Another agent owns
rm32 and its older/newer platforms; do not edit, build, flash, or otherwise
modify `../rm32` or `../rm32_stm32`. Leave source-grounded portable findings in
binz documentation for that agent. Explore upward in meaningful5 percentage-
point steps with E777's signed-average current foldback/hard stop, bus, nFAULT,
tracking and watchdog protections. Retain every failure; fix causal binz
control/CPU/timing defects rather than loosening protections. Qualify representative
10/25/50% points with repeated holds and normal-start recovery. Qualification
follows envelope discovery and uses lean images. The old active-goal phrase
about documenting portable rm32 improvements does NOT authorize rm32 changes.
Today's unflashed attempted rm32 UART/board/current edits were fully removed;
E777 remains installed/OFF/UARTclosed. PSU ceiling is now2.5A. If the PSU enters
sustained/hard current limiting, safe outputs, retain the run, and pause/report
the power-path blocker rather than treating folded operation as valid data.

E778 main-rm32 current ownership audit, no board flash/run. Do NOT copy E777's
bench-only one-shot5%/session-monotonic governor: rm32 already has the production
form. Main's continuous current PID owns `current_limit_adjust`, publishes it
atomically, and TIM6 applies it as the final duty ceiling after command,
rate-limit,duty-max and stall boost. Added regression proving high command and
positive stall boost cannot override1250 ceiling;364core+7harnessPASS. G071
release-s/thinLTO/codegen1 rebuild SHA B701A77B59A0D7257D238DBE3F1B30A8D65B2FC8A012287C0A94E25990203650,
text40024/data1784/bss1144; current audit hash matches, all TIM6/TIM14/ADC_COMP/
DMA roots zero forbidden reachable (69 helpers outside roots/707 direct edges).
No bench1.5A constant or2s pacing ported. Main rm32 still lacks powered board
qualification. Board remains E777 installed/OFF/UARTclosed.

E777 CURRENT/installed SHA919BEB3C8AE1E13C1B0A8CA489FB931FBE6286206DA009C0EB3245EC32D226BB,
OFF/UARTclosed after powered tests. Added opt-in foreground average-current
foldback: first complete over-limit block latches warning; foreground lowers
coherent PWM by exact5%; second consecutive over block remains existing reason25
hard stop. Ceiling is monotonic for session and caps host/restart requests; no
auto-release. DMA performs no PWM/math. Bus/nFAULT/tracking/watchdogs/pulse
clamp unchanged. Release-s/thinLTO/codegen1, frozen currentfold_777; all4 ISR
math roots zero; disabled guard3/18PASS. Valid30s live7->10->15->20->25->30
run reason2, foldback0,busmin11689mV,finaloff. One30% tracking-decision restart:
reason8@5.000052s,1s disabled settle,fresh ordinary startup/handoff,restored30,
then one30->25 foldback and completed remaining19.238005s reason2,bus11760mV,
finaloff/nFLT1. This replaces the E775 intermittent reason25 with measured
derating but is one exploratory run, not30% recovery qualification/calibrated
amps. First host attempt was cut at30s before capture, immediately safed and
guard3/18 rechecked; excluded. See CURRENT_FOLDBACK_E777.md. Next portable work:
map this warning/foldback ownership onto main rm32 current policy without
copying bench timing constants or weakening its existing hard protections.

E776 main-rm32 integration audit, no board flash/run. Traced confirmed tracking
loss through AllOff->Armed->ordinary10kHz startup. Existing DutyState sets
min_startup then rate-limits every move toward current receiver command; newer
command supersedes naturally, and low-ZC reclimb ceiling remains additional
bound. Added sequence test proving AllOff tick has no authority, retained high
request does not jump duty, first restart duty120+2=122, and new minimum request
replaces it through ramp. Full host363 core+7 harness PASS. G071 release build
from rm32_stm32 PASS opt-s/thinLTO/codegen1, ELF SHA64B6787903EC1406FBA493B01B86FDB1DDEBCAE53D1B533C48F58F430D447E47;
linker M0 motor-root audit remains PASS. Found Cargo cache could leave an old
sidecar beside a reused ELF; audit tool now supports explicit --audit-existing,
records ELF SHA, and current report hash matches64B67879 with all4 roots zero
(69 unreachable helpers/707 direct edges);4 audit-tool testsPASS. No second recovery controller, bench
injector, or2s pacing constant ported. E775 board image remains installed/OFF;
main rm32 still lacks powered qualification.

E775 CURRENT/installed SHA2BD1C70CCEE48C95C5D58A409279ED5347E67F4FFECA8294C93DB329B4651C62,
OFF/UARTclosed. Operator PSU ceiling now1.5A; explicit bench-current-1500
sets nominal signed-average target1500mA (ADC remains~10-15% conservative),
ratings-derived sample clamp/bus/nFAULT/tracking/watchdogs unchanged. Release-s/
thinLTO/codegen1, frozen captures/reference/m0clean_775; mandatory ISR math
audit passes DMA1/ADC_COMP/TIM16/TIM6, disabled guard3/18PASS. E772 same
1.5A control path live-ramped7->10->15->20->25->30% and held full30s,
~1182eHz estimate,busmin11748mV,reason2/finaloff. Normal restart now snapshots
the last ACKed live duty, uses ordinary low-duty startup, then foreground-only
restores in exact5% steps every2s through the existing guarded live writer;
host updates supersede retained target. Configurable fault scheduling separated
real comparator-suppression from immediate tracking-decision injection:
suppression at20/30 caused legitimate average-current reason25 before the
tracking watchdog, so no restart; direct reason8 injection omits the induced
stalled-sector dwell and tests recovery policy only. Exact E775 25% tracking
recovery cohort3/3: first reason8 at~5.000s,1s outputs-disabled settle,fresh
handoff,4 steps to25%,full19.237..19.238s deadline reason2,est1153..1182eHz,
busmin11737..11904mV,commitmax8us,finaloff. Exact E775 30% recovery is2/3:
two full reason2 at1344..1366eHz/bus11677..11737; retained run03 restored30
then average-current reason25 at14.043567s,bus11641. Do NOT call30 recovery
qualified or retry-until-pass/raise threshold. Lean recorder fields omitted,
not zero CPU. See TRACKING_RECOVERY_E775.md. Goal active: productionize current
limiting/foldback or establish honest high-duty policy, then port practical
startup/live/restart wins into rm32; no more duty inching or routine PSU reads.

E770 CURRENT/installed SHA6DA455B3E090C3A9720245D9D0627DD9BF2594466D180E43587F8F7870AAB272,
OFF/UARTclosed after normal-restart3/3. Autonomous staircase now starts at
50eHz, matching the proven host path, instead of the old100eHz fixed catch
followed by a ramp backward toward50. No protection/threshold change. Each
30s campaign: first BEMF handoff, injected tracking reason8 at~2.001s,
outputs disabled for exactly1s, ordinary sine startup, fresh powered BEMF
handoff, exact replay of acquisition61/BEMF70/+60deg settings, then
22.230004..011s powered deadline reason2. All three strict host validations
and final-off checks PASS; bus minimum11748mV in run01 and no electrical guard
fired. Release-s/thinLTO/codegen1; frozen captures/reference/m0clean_770 with
math audit. Disabled guard3/18PASS. This is an exploratory restart cohort, not
higher-duty qualification. E767 forensic correction: its run02 did NOT reach
second powered handoff; the second sine startup hit average/current reason4 at
4.056s, while printed power_reason8 was stale first-segment state. Run03 was
powered average reason25; run04 passed. E768 added1s settle and strict marker;
E769 fixed exact-setting replay. E768/E769 initial attempts failed average
current during old100eHz catch; catch70 failed faster. See
NORMAL_RESTART_E770.md. Board ADC current/bus remain routine instruments;
current roughly10-15% conservative, PSU only for disagreement/saturation/new
regime. Goal active: carry reliable ordinary restart into practical live-duty
exploration/lean production path, not more identical restart repeats.

E767 CURRENT/installed SHA0B0C180FC0CFE6D6BF02759C674D96D4BB8BF208BB1211B6BC005A4BCF4CA82F,
OFF/UARTclosed after normal-restart campaign. Operator accepts board current/
bus ADC for routine decisions; current is conservatively about11% high at the
15% PSU anchor, retain10-15% uncertainty and ask for PSU only on disagreement,
rail/nonlinearity or a new regime. Fixed host false-pass: NORMALRESTART result1
means attempt launched, not completed. Strict verifier now requires fresh
DRIVEX result1/power_reason2 and authoritative POWERPATH reason2 at retained
COASTREF deadline; accepts only benign COASTREF stop1/7 ordering. Actual E765
retained captures: one pass, two rejects;2Python tests plus driven19/handoff33
PASS. Per goal, driven acquisition's64 dispatch/ms cap is report-only in the
normal lean path; peak retained. Real50us handler-overrun stop and independent
command/feedback/watchdogs remain. Diagnostic rate-snapshot feature retains
old hard-cap behavior. Exact E765 features, release-s/thinLTO/link audit;
frozen captures/reference/m0clean_767. All DMA1/ADC_COMP/TIM16/TIM6 direct
roots pass forbidden soft/wide/float audit. Disabled guard3/18PASS. Normal
restart actual powered cohort (files02/03/04) is still1/3: run02 peak71,
handler22us/overrun0, second sine startup average/current reason4 before handoff;
run03 peak52 then average-current reason25 at2116us; run04 peak65 now allowed,
handler22us/overrun0 and full22980005us deadline reason2. Setup file01 asked
idle bemfdu200, was refused before arm/no motor. Do not claim restart fixed or
widen current/tracking guards. Next compare/reset first-vs-second startup/
handoff state and startup current trajectory; the cap was one blocker only.
See NORMAL_RESTART_E767.md.

E766 portable rm32 integration, no board flash/run: accepted onboard current
and bus ADC as normal campaign truth after PSU anchor; retain roughly10-15%
current uncertainty and only request PSU corroboration for disagreement/rail/
new calibration regime. Ported M0 arithmetic discipline to ../rm32 +
../rm32_stm32. Prechange emitted G071 motor roots had7 soft divisions (TIM14
one,TIM6 six including normal20kHz PWM). Added exact bounded fast_math /3,/2000,
/1400 with exhaustive domain/control-equivalence tests;362core+7harnessPASS.
Release now opt-s/thinLTO/codegen1. New target linker audit emits S/JSON and
fails forbidden division,64/128bit,float helpers reachable through direct call
graph for supported motor vectors;3 audit testsPASS. Current G071 release
SHA64B6787903EC1406FBA493B01B86FDB1DDEBCAE53D1B533C48F58F430D447E47,
text40024/data1784/bss1144; TIM6/TIM14/ADC_COMP/DMA each0 forbidden reachable.
69 helpers remain outside IRQ roots, not declared harmless. Direct-call audit
cannot see indirect or inline-wide operations. See ../rm32_stm32/
M0_ARITHMETIC_AUDIT.md and PORTABLE_WINS top. Board remains E765 installed/OFF;
no UART/SWD/flash/motor in E766. Confirmed BEMF timeout now requestsAllOff,
transitions toArmed, resets startup interval, then persistent throttle uses the
ordinary startup path next tick; no main-path CommutateKick after confirmed
tracking loss. Decision+two-tick sequencing unit-tested, NOT powered-qualified.
Goal remains active; next portable candidate is calibrated average-current
integration, not more binz envelope repetition.

E764-765 CURRENT/installed E765 SHA964E160B9C038358E54D6F5A105FC89AD84DC23C5BFEB278DF6327C0D89A42DC,
OFF/UARTclosed. Ratings-derived pulse clamp replaces historical+/-1200 raw
when average-current owns protection: same-wake per-channel zero,1.3V CSA
displacement (~18.6A nominal), ADC rails immediate. Raw ratio bus guard never
false-accepts old<8400mV decision. Foreground-prepared sine scale + exact/255
fold; driven successor no%6. Linker now FAILS if DMA1/ADC_COMP/TIM16/TIM6
directly reach soft div/64/128/float; frozenE765 all roots0. Host avg10,
sinescale2,guard37,audit6 PASS; disabledguard3/18PASS. E76415%10sPASS; live
10->15->20->25->30 ACK,30 stopped avgcurrent reason25@5.362190s,bus11.569V;
25 stopped reason25@7.262030s,bus11.653. One20 startup stopped on remaining
foreground+/-1200 shadow; removed causally. E76520% full30s reason2,
155645COM,bus11.617;22.5% tracked20.084624s thenavg reason25,bus11.820.
Thus1A sustained envelope bracket20..22.5%;30 commandability proven, not
sustained. E76520% now formal3/3 lean cohort: reason2 stops30000009/004/005us,
COM155645/157269/158254,eventage64/31/111us,feedback124/120/120us,alloff.
Verifier parameterized duty and accepts only same-deadline COASTREF1/7 race
while requiring POWERPATH reason2; originalE76215% stillPASS. No bus/nFAULT/
tracking/watchdog faults. Memo M0_CLEAN_ENVELOPE_E764_E765.md. Goal active:
fold portable wins into rm32 and remaining practical integration, no PSU
readback needed.

E763 nohardware: operator acceptsADC current/bus as rough bench telemetry.
15% hold PSU230mA/11.8V vs firmware final signed block~255mA: board reads
~1.11x PSU at this point, so retain10-15% uncertainty and DC-supply-vs-bridge
sampling caveat;1A threshold conservative. Stop requesting routine PSU
readback. `drv_math_audit.py` now supports direct-call reachability roots;
6testsPASS. FrozenE762 audit exposes DMA1->convert uidiv, ADC_COMP startup
authorize uidivmod, TIM6 startup pwm_sine uidiv; TIM16 clean. This is
reachability not active-branch proof and excludes indirect/inlined math.
Next raw division-free DMA guards, then startup IRQ arithmetic. No build/
flash/UART/motor; actualE762 B216... remains installed/OFF. Memo
ADC_CALIBRATION_E763.md. Goal active; ratings-derived pulse clamp and30%
current-bounded envelope remain.

E762 representativeLEANqualification15% 3/3. Image actual/root
B216931BED6F786DD182C98F62FD8FAD13E2A68C54CF64388C83613BF7341B00
quallean_762 installed/OFF. Features lean-core+lean-irq, noaverage diagnostic;
functional current/bus/nFAULT/tracking/DMA/watchdogs retained. Release-s/
thinLTO/build/audit/download/reset0/guard3/18PASS. Three autonomousstartup+
live10->15%+30s runs all reason2 deadline/no veto/fresh event+feedback/
finaloff/Uartclosed: commits121265/120935/120669, eventage187/134/231us,
feedbackage120/120/121us. Captures quallean_762_*_01..03. Strict
verify_lean_qualification.py requires3, CRCs, lean markers, empty ACCEPTLOG,
absentdiagnostics, ACK, deadline/progress/freshness/finaloff; PASS. This is
representativepointqualification, NOTfull30%/qZC/sigma claim. Memo
LEAN_QUALIFICATION_E762.md. PORTABLE_WINS appended E755-762. Goalactive:
30%current-bound,ratingspulseclamp/currentuncertainty/rm32fold remain.

E760-761 NORMAL STARTUP RESTART FUNCTIONAL. Integrated bench-normal-restart:
trackingreason8only, waitsdisabledcoast, oneattempt, freshsamewakebaseline+
averageinstall, fullautonomousstaircase, remainingoriginaldeadline, noflying
seed; retainsfirststop. E760 firsttry DIDNOTexercise restart: isolated10.05ms
averageblock6512>4329 stopped beforehandoff. Average policy now needs2
consecutiveoverlimitblocks=20.1ms; goodblockresets, railsimmediate, physical
targetstill1A.34shared+10adapter+3restarttestsPASS. E761 actual installed
057764B9A6C85FAE63332296BF69225ACDBC50AF90D16DB003FAA5F1D2C020CE,
release-s/thinLTO/audit/download/reset0/guard3/18PASS. Injectedtracking:
firstreason8@2000605us/4057COM; disabledcoast; freshnormalstartup; second
BEMF22979003us withinoriginal30s, flying_seed0, finalresidual1575<4327,
freshlast-event/feedback, finaloffPASS/Uartclosed. Capture
normalrestart_761_direct200_30s.txt. Exploratoryfunctionalproof not qZC/sigma.
Posttest external-run resetsone-shot sourcefix root2CB707AD... build/auditPASS,
NOTflashed; actualE761/OFF singlecampaignproved. NORMAL_RESTART_E759.md.
Goalstillunfinished: representativequalification/noISRdiagnostics,
ratings-basedpulseclamp,currentuncertainty,portable rm32 wins,30%duty currentwall.

E759 normalrestart policy foundation added/includedshell, NOTintegrated/live.
Onlyreason8tracking, oneattemptconsumedevenrefusal, fullstartup>=4.7s+200us
mustfit originalpoweredend, returnsremaining>=20ms; checkedno-wrap.
3hosttestsPASS successfularithmetic/duplicate/electrical/time/invalid/wrap.
No gate/wake/faultclear authority, noflash/UART/motor thisentry; actualboard
avg1a_758/OFF historical. RootDA300E37CF81CA57BA694B01B15B588E810A1615260485C27858AC50D9948A6F
builtrelease-s/thinLTO/directTIM16auditPASS butNOTfrozen/flashed/qualified.
NORMAL_RESTART_E759.md integrationrequirements. Goalactive/notblocked.

E758 nominalphysicaltarget1000mA nowmatchesoperatorPSUexact1A, announced
beforechange; correctedpolarity/gain/shunt/VDDAconversionunchanged. Notraw
fault-fit/calibration. Host --nominal-target-ma1000 requiresACKexacttarget
andconvertedraw; default800retainedforolderimages.34policytestsPASS.
Actual/root756ADF564F065438DD02EF47C3FD1D024535500075CAF8DE8F457FFC46FDAFAA
avg1a_758 installed, sameautonomousstaircasefeatures, release-s/thinLTO/
build/audit/download/reset0, own guard3/18PASS. Direct200/ramp30 ACK10/15/
20/25 thenaverage25stop4853057us:5125>4323/~1185mAnominal,13376COM,
busmin10937/finaloffPASS. Sameimage20hold ACK20 andran22239545us then
average25:4352>4332/~1004.6mAnominal,111943COM,busmin10722/peak748,
freshfeedback/lastacc22239443/finaloffPASS/Uartclosed. NOT30s20qualification.
Autonomousstartupworks2attempts intoBEMF withoutUARTehz, notfullcohort.
AUTONOMOUS_STARTUP_E758.md exactcaptures/currentfaults, goalactive.
Currentpulselegacyrawlimitstillpresent,
normalrestart/calibration/qualificationstillunfinished. Goalactive.

E757 operator15% anchor clarified230mA/11.8V uncapped. Operator accepts
ADCcurrent/voltage benchestimates forroutine testing: stop redundantPSUasks,
notprecisioncalibration. Directfirmware100->200 onE756 failedbeforehandoff
(acquisition21/2commands/0candidates, averagecause0), finaloffverified.
Newcfgbench-startup-staircase replayssuccessful100catch->50ramp then60..200
at3200..4400 envelope ticks, nohostehz/no runtime division innewhelper.
All15boundariesconstasserted +5campaigntestsPASS. Existingguards unchanged.
Actual/rootFAF085209FF2F780152564157E315EC6271A0625B5A9FABEFF3EA4F492434228
staircase_757 installed; release-s/thinLTO/build/audit/download/reset0 and
own guard3/18PASS. Direct200/ramp15/30s TERMINAL: averagecause3 atinitial
forcedacq9956us,3771>3458 (~872mAnominal),12cmd/12IRQaccepts,
handler22us/zerooverrun; noBEMF/live15ACK. Not samefailure asstraightdirect
acq21. Finaloffverified/Uartclosed, no thresholds changed. Notautomatic
restartyet. Capture staircase_757_direct200_ramp15.txt. Autonomousschedule
compiled/runtimeexercised butstartupnotqualified; retainE756workingarchive.

E755/756 DATASHEET SIGN FIX supersedesE754polarity uncertainty. Cached
SLVSE39Bp28Eq3/Figure30: I=(VREF/2-VSO)/(gain*R), SPxinverting. Former
measurementplan PLUS sign wrong. average_current residual nowzero-sum,
baselineceilconservative<1rawcount/block. Noabs/gain/thresholdratchet.
34startup/9adaptertestsPASS; renderedpagecaptures/drv_csa_page28_e755.png.
Actualcurrentsign_756 SHA73190F5F6483EECD062F08559D16C67B830E44E6449B31F241E103712BF93F8E
installed, sameE753bfeatures; release-s/thinLTO/build/audit/download/reset0,
own guard3/18PASS. OperatorPSUexact1A confirmed, firmwaretargetnominal800mA.
Ramp30 ACK10/15/20 only thenaverage25 at4372052us:3541>3468 (~817mAnominal),
busmin10948/peakraw635/10621COM/freshfeedback/lastacc4371951, finaloffPASS/
Uartclosed. NOT30%success. Sameimage15hold completed30s deadline2 at30000005,
122664COM/busmin10865/rawpeak618/finaloffPASS/Uartclosed. Operator230mA;
lastAVGblock1104/3468*800=254.7mAnominal, direction/scalematch notcalibration.
Capture currentsign_756_start62_bemf70_ramp15.txt. Goalactive.
CURRENT_SIGN_E756.md evidence. RootELFsamepolicy; postbuildtest-onlyaddition.

E754 read-only analysis of E75325% capture: S85 CRC/cadence/count validated
149253scans at201us, oneunaggregatedpublication retained. Rawbusmean1141.318,
VREFmean1506.727,factoryvcal1662 => approximate segmentmeanVM11.0095V
(ratio of means, not mean of per-sample ratios). Idle11.7 means~0.69V mean
sag; actualfirmwareminimum9.790V means~1.91V deepest recorded sag. Supports
operator voltage-sag observation, not exactpersistent1V at25endpoint: ramp
included and no per-segment voltage histogram. Current rawmeans relative2048
A=-8.090/B=-8.963/C=-6.960 contradict positivePSUanchor; no signflip or
thresholdratchet justified. NoUART/flash/motor this analysis; E753bOFF remains
historical verified state, notfreshreadback. Previousturn progress, goalactive.

E753 restored ACTUAL ROTATION: operator "spun pretty sportily now" on
softwarestartup_753b, non-startup-adc/non-lean-irq, lean-core retained.
Average-current feature/avgnominal retained (uncalibrated sample mean), legacy
raw phase limit still present; not target production protection semantics.
Installed/rootDD860DA9FBC4FED6B3623CD4828AD8076B3962D8750A4D41F2648A06C77C0BE0.
Release-s/thinLTO/build0/TIM16mathaudit/download/reset0/own guard3/18 PASS.
Archived host run50, ACK60..200 at3.2..4.4s, sine62/forced61phase60/BEMF70.
Live100/150/200/250/300 ACK all; finalI=284halfus (~1174eHz controller
estimate). Actual30% reached, NOT30s qualification: powered6024087us stops
Bus6, busmin8381<8400mV; 21576 commits, peakraw888, nominalaveragecause0.
Tracking last6024057/feedback6023801 do NOT indicate tracking-loss stop.
Capture softwarestartup_753b_start62_bemf70.txt, finaloffverified/Uartclosed.
Replacement timed startup's buzzing is a software-path regression, not a
hardware ceiling. E753a full-recorder image D8D8A8AB notflashed: TIM16uidiv
auditfail; lean-core E753b removes it. Same-image25% completed full30s:
reason2 deadline,177445COM,lastaccepted29999998/feedback29999885us,
busmin9790mV,peakraw823,finaloffverified/Uartclosed. Operator reports812mA
capped atPSUlimit. AVGDIAG residual=-3670/cause0 does NOTcorroborate positive
PSUcurrent; nominalaverage protection uncalibrated/notindependentlyvalidated.
Capture softwarestartup_753b_start62_bemf70_ramp25.txt. Lean-core accepted
statistics absent(events0 NOTzeroactualevents); no qZC/sigma qualification.
STARTUP_ROTATION_E753.md retains both runs. Goalactive.

E752 operator correction: last test DID NOT SPIN. Accepted comparator events
and measuredseed therefore are NOT evidenceofrotorBEMF/rotation. Stop citing
seed success as mechanicalprogress; validate actualstartupwaveform against
working path. Actualfirstaccept_752/OFF/Uartclosed unchanged, no newmotorcmd.

E752 actual/rootBCBCE971810006930D6240315BBCE2586769A9E39552DD906C344ED8FC394634
firstaccept_752 diagnostic installed/OFF/Uartclosed. Optionalbench-first-accept
retainsone3u16row AFTER persistence, no perreadtrace;observercostunmeasured.
Release-s/thinLTO/build0/TIM16audit/download/reset0/own guard3/18/finaloffPASS.
Directsine40->100/forced100/60/BEMF150 seed1681/age254/arm7; FIRSTACCEPT
step4 interval901halfus vsseed1681 (450.5vs840.5us). SixCOM/latestacc2360
sector2 thentracking3405/freshADC3158/avgcause0/no300ACK. Earlyfirstaccept
confirmedrelativeestimator, NOTproofphysicaledge/noise/rotorspeed. Userasks
why motor runsless: currentstartup100->200~4.7s thenBEMFmillisecondsstops;
historicalE69620%940eHz30s islostcapability, nothardwareceiling. Goalactive.

E751 actual/root87E07DAB6816045E97ADD33537E40808825AA67FFD77181B21790588DAD38EB0
estimator_751 installed/OFF/Uartclosed. Dedicatedstartup estimator12contiguous
measuredintervals, cyclequalityreport, original40msdeadline/timercorroboration/
gap-reset/seedmeanbounds retained; strictflyingconstructors unchanged.
Mean32bitreciprocal exact0..24000/indexbranch notmod;60policytestsPASS.
Release-s/thinLTO/build0/TIM16audit/download/reset0/own guard3/18 PASS.
Directsine40->100/forced100/60/BEMF150 release11098us13cmd14accepts,
seed1670/age196/ARR222/arm7/reanchors0/handler18us. Powered2COM/latestacc345
sector4,tracking1405/freshADC1223/avgcause0/no300ACK. Seedrefusalresolved
THIStrial, notsustainedhandoff/15%lock. STARTUP_ESTIMATOR_E751.md scope.
Nextposthandoff chronology; normalrestart/calibration stillunfinished.

E750 actual/root6D34311DB617319F8362AFB834A2011FC2FBB7B1518782E135821D4FE35F3280
fullseed_750 installed/OFF/Uartclosed; constructorstartup_window12 notcycle6.
Release-s/thinLTO/build0/TIM16audit/download/reset0/guard3/18/57policyPASS.
Directsine40->62/forced100/60/BEMF70 timeout40ms48cmd42acc; sine40->100/
requestedBEMF150 timeout48cmd46acc, NOhandoff/300ACK/avgfault. Full12NOTfix.
Stronger sine earlytraceALLsixstepsaccept inclstep1 (notbadwireproof);
spacing1082/1006/1024us individually>1ms resetsacquisition despite~5mscycles.
Finalseed10/5 and8/3 intervals/cycles/fault0. STARTUP_FULL_WINDOW_E750.md.
Next dedicatednormalstartupestimator, not lucky12edgecohorts/flyingrules.
HistoricalE696ELFlacksrequiredaverageguard, inspectedNOTflashed. Onecomment
changedafterbuild; installedELFfrozenabove. Goalactive/nohardwarewall.

E749 actual/rootE36C2BA93CFC4F7F86BFBC0B76DB8738633DAF73CFE5A8C4950AC418866BB09D
unchanged/OFF/Uartclosed. CORRECTION E748 earlyehz commands all!step:
firmwarehold-onlysteps after3s; earlier/slowertrajectoryNOTexecuted. Memo
corrected, fixtureexactFtargetACK+pendingpacing/rejection/0.5stimeout added,
3ramp/8capturetestsPASS;newACKbranchnotmotor-tested. START_HZ100not50.
Validdirect100->200 catch40->62/forced61/phase60/BEMF70 timeout40ms32/48acc.
Sameforced100 measuredseed1558/arm7us at5744us,2COM/latestacc279sector4,
tracking1305/freshADC1026/avgcause0/residual2721<6931. Bothfinaloff/no300ACK.
STARTUP_ACK_E749.md exactcaptures. Nohardware/currentcalibrationclaim,
goalactive/sustainedhandoff+normalrestartunfinished.

E748 actual/rootE36C2BA93CFC4F7F86BFBC0B76DB8738633DAF73CFE5A8C4950AC418866BB09D
practical_748 installed/OFF/Uartclosed; foregroundprobe omitted/sharedconst
ceiling rebuilt. Release-s/thinLTO/build0/TIM16audit/download/reset0/guard3/18.
Direct62/forced100/phase60/BEMF70 seed1667/arm7,3COM/latestacc806,
tracking1905/freshADC1779/avgcause0. Earlierhost50->2000.2s+3s catch62
stoppedaverage1.527s7183>6936. Catch40->62/freq0.2s+2s stopped2.673s7180>6913,
bothbeforehandoff. No300ACK/sustainedqualification/hardwarewall. Hostadds
--startup-ramp-start/span/--catch-duty, defaultsunchanged;2validationtests+
8capturePASS. STARTUP_RAMP_E748.md evidence. NominalcurrentNOTcalibrated;
nofirmwarethresholdchange. Goalactive, normalstartuprestartunfinished.

E747 actual/root75835561CAF35BCD63AE257FC73355B0BDEE1E132081B0CFDBA7D9F50A64D026
handoffreg_747b installed/OFF/Uartclosed. Fixed first-COM100 rejection before
live preload (E746genericHostAbort). Firstcom30_747 nowcommit1/tracking1005.
Separateoptionalforeground-once register diagnostic, noISRhooks/masks/TX:
phase0 initial200 reached11COM/latestacc3500 thenreference desync(avg731
vsseed1464);snapshotTIM2CEN1/COMPunmasked. Phase60 initial200 reached4COM/
latestacc1077 thentracking2106/freshADC1834/avgcause0. No300ACK/sustained20%.
Bothown release-s/thinLTO/build0/TIM16audit/download/reset0/guard3/18/finaloff.
STARTUP_FIRST_COM_E747.md hashes/captures/observerlimits. Aftertests source
constsegment_max refactor sharedCOM/handoff,33purePASS NOTrebuilt/flashed.
Installed/rootfrozenabove differslatestsource; goalactive, nohardwarewall.

E746 actual/rootAC7F5E5F0AEB37E7864503D1CFA168268F955860720AA32BC1570E8E5D1E6962
initial30_746 installed/OFF/Uartclosed. Timed startup initialBEMF40..300,
acquisition<=100/legacy/recovery limits unchanged, compare once-per-segment.
Release-s/thinLTO/build0/TIM16audit/download/reset0/own guard3/18 PASS;
32policy/21startup-role/8capture tests PASS. Initial20% phase0 drive100 trials
seed1666/1595, arm6/7us, then reason9 at125/91us ZERO commits/accepts.
Quiet second trial no live requests: do not infer UART caused HostAbort;
core Recorder::all_off also uses abort(). Avgcause0/finaloff verified.
E745 referencepending restores13/13 startup, then10%BEMF COM1/trackingstop.
STARTUP_HANDOFF_DUTY_E746.md exact hashes/captures. No20%run/300ACK or
envelope qualification. Next actual software disable decision at handoff.

E744 actual4877BE755BBEB90D384D041917BC1E17B0CAB3235A7D12DFE83D3C5A144B8DC7
leanfg_744 installed/OFF/Uartclosed/download/reset0/guard3/18PASS. 62sine/
100drive/100BEMF directphase60 timeout40ms/48commands/40acc/no seed;
phase0 stopped15at21587us/25commands/19acc. Bothavgcause0/no300ACK/finaloff.
Lean scans/age_max0 intentionalnotfeedbackabsence. Source15islate>50us fixed
margin. Candidategoalaligned startupADC reportlate50, killonlyexpiredwhole
nextsector late>=plan.delay beforewrite; idealdeadlinesretained/otherguards
untouched. NOTflashed. STARTUP_FOREGROUND_E744.md; newpolicyuntested.
Candidate root5412DBDF9095503716A3B2E1ED5F9527C1EE0B1D69AC7D83B9A55C402005EF52
release-s/thinLTO/build0/TIM16auditPASS, notinstalled/qualified.

E743 actualF937B69486004C8C20D7CF5F99DA791FFFF915FD80E14682EE80FC45C287A6EF
compeer_743e OFF/Uartclosed. VDDAconversiondeployed800nominal3313mV; oldramp
stillavg7610>6923 beforehandoff. Measured61drive40mstimeout28/48accepts.
StartupCOMP0experimentfailedsectorreason15/700us; retired/restored743b.
743bdrive100/60deg yieldedmeasuredseed1565/age290/ARR101/arm7,40/46accepts,
thenoneBEMFaccept355sector4/COM2step5/tracking1402/freshfeedback1230/avgcause0.
COMpeer oldprioritytestfailedstart5451usreason15 beforehandoff, UNEXERCISED
poweredprioritycomparison. Allinstalledrelease-s/thinLTO/TIM16audit/build/
download/reset0/own guard3/18/finaloffPASS/no300ACK. Candidate removeslean
foregroundmaskedADCrow/stat/capacitystop work; retainsoriginal-agehandoff
token/DMAguardproducer, lean scans/age_max omittednotfeedbackabsence.
Root4877BE755BBEB90D384D041917BC1E17B0CAB3235A7D12DFE83D3C5A144B8DC7
build0/auditPASS/NOTflashed. STARTUP_MEASURED_E743.md. Nextshorterforeground
maskedpathverification andpracticalstartup, nothardwarewall. Goalactive.

E742 actualC6E98EA9F451575026D2E247520E76FCC14E4DB07ED65427AC5BD3EE67A4F007
oldramp_742b OFF/Uartclosed. Correctedpollcandidate stillCOM1/noaccept/stop,
retiredfor edge-onlyoldterminalrun50→200/62/61/60/70 (E69613orderedaccepts/
seed1539). Newprotectedoldtrajectory stopped sine1929537us average6620>6371,
nohandoff/300ACK. SavedVREF yields3302..3315mV vsassumed3600; nominalfault
~765mA NOTcalibratedamps. Candidateavgnominal usesidlefactoryVREF-derived
ceilVDDA/2700..3600/fixed800mA formula, hostexactACK, noadaptive/rawratchet.
Pure32PASS/release-s/thinLTO/TIM16audit/build0 root0707B885D7D5A24F7C5E2988C77415D4B929220CA5B9980EC2B1AD35DB95D9F7
NOTflashed. STARTUP_VREF_E742.md. Nextcorrectedconversion oldtrajectory test;
gain/sign/aperturecalibrationstillunfinished. Goalactive.

E741 actualE74063B1CF9FCD2BE83529E1F09A116A9FDE3166FB74C53134B55EAF7652FA98F4BD
OFFfreshSWD PD1ODR0/BDTRc1aMOE0,resumed/noUARToverlap/no motor. ExactELF
gdbDriveStorelayout thenSWD ninewords: step5/bemf1/bad11/min3/filter12/
rising1/oldroutine1/zc7. 19polls→8matches/11mismatches, notabsentlevel.
Sourcepostcallzc delta canfakewatchdogaccept onCOMpreemption; candidate
explicitreferencepollingband publishesonlyatqualificationdecision. Race
notobservedinE740(noaccept). WaitfirstbootstrapCOMbeforepollband; root
FA516D9D355E64BDF51A9733405EF1015E867CB7282D0A45029BEB804A5C6A65
release-s/thinLTO/TIM16auditPASS/NOTflashed, STARTUP_COUNTERS_E741.md.

E740 actual/root63B1CF9FCD2BE83529E1F09A116A9FDE3166FB74C53134B55EAF7652FA98F4BD
startpoll_740 installed/OFF/Uartclosed. E739DI85steps6,1,3,4,6,1 gaps; six
cumulativeaccepts notconsecutivelock. E739c62/80/0/100 alsoCOM1/noaccept/
trackingstop. Optionalstartup-polling usesreferenceTIM7qualification, feeds
watchdogonlyzero_crosseschange/currentpreCOMsector; defaults/flyingunchanged,
nofree-run/fakeaccepts. Release-s/thinLTO/build/download/reset/TIM16audit0/
guard3/18PASS. Test19polls/maxcall16/maxgap82us, COM1/step5/noaccept,
tracking1002us/feedback918, avgcause0/finaloffPASS/no30%ACK. Phase-locked
50uspoll limitation, notuniversalpollingfailure. STARTUP_POLLING_E740.md.
Next realcomparatorqualificationstate/alignment, notblindduty/hardwarewall.

E739 actual/root9AA9609DC7FB0271C887A1DBF1CF248563F4CD3BA009F100743932A32CC27902
start10k_739c installed/OFF/Uartclosed. Restored10kstartup→gateoffonceARRUG20k
BEMF, TIM15ADCpreserved. Initial50scan10kstop954890us avg3312>3185; found
half-carrier coverage, corrected100scans/6371nominal800mA (20.1mswindow),
50unchanged20kstartup. Default/cfg100 pure31/31PASS, release-s/thinLTO/TIM16
audit/build/download/reset0/guard3/18PASS. 739bhostfailedpre-motorhardcoded
scans50label; fixedlabels/fixtureexactpairs. Final62/61/60deg/100BEMF sine
reachedhandoff avgcause0, assumedcommandedseed1666/age264/ARR153/arm7; then
trackingstop/no30%ACK/nohold. STARTUP_CARRIER_E739.md. Next startupactual
sequencevscommandedbootstrap/referencepolling, notcarrier/hardwarewall.

E738 actualE736c18FA024E4292FD713A0AA643CD61CA8FF6F837C030E0D50A8040B389F0603EB7
OFF/Uartclosed,no flash/guardchange. 62sine/61drive/60deg/100BEMF armedCOM1/
step5/noaccept,trackingStale1002us/feedback756. Startup8acceptedonlysectors
3/4/6epochs3..22, notcontinuoussixsectortracking. Flat100sine/100drive/60deg
stopped1115291us in sine averagecause3/residual3889>3185, nohandoff. Both
targetACKFalse/finaloffPASS. SourceCORESEED foreground provenancecorrected
commandedassumed1 vsmeasured0; release-s/thinLTO/TIM16auditPASS rootBF6040C53F76824E372C2939BFD7E7312CF7E33844D475136307A25FB9B2F43E
NOTflashed. STARTUP_ALIGNMENT_E738.md.
Nextsparsestartupacceptancecausevsworkingpath, nothardwarewall. Goalactive.

E737 actual/rootE736c18FA024E4292FD713A0AA643CD61CA8FF6F837C030E0D50A8040B389F0603EB7
OFF/Uartclosed,no flash/guardchange. Fixture --bemf-duty40..100/default70
and dynamicinitialACK added. 62sine/80drive/0deg/initial100 test released
7873us/6accepts; BEMFcom2/step5, lastaccept342/sector4, poll1402/feedback1242,
trackingStale8at1405us. Avgcause0/finaloffPASS/targetACKFalse. Nohold/30%claim.
RoleMODER and AFrestore masks preservePA2/PA3/PB3/PB7 analog; puretestadded.
STARTUP_INITIAL_DUTY_E737.md retains evidence. Next restoreworkinghandoff,
notblinddutysweep/hardwarewall. Goalactive.

E736 actual/root18FA024E4292FD713A0AA643CD61CA8FF6F837C030E0D50A8040B389F0603EB7
startpub_736c installed/OFF,Uartclosed. Startupguard now receives eachrealDMA
frame originalage, noforegroundcachedreplay (LAST_FEEDBACK/ADCrecordsretained),
averageonceperframeunchanged. Final62sine/80drive/0deg released8862us/6accepts/
11changes/no startupstale; thenBEMFcom1/step5/noaccept/tracking8at1005us,
lastpoll1002/feedback782fresh. Avgcause0/finaloffPASS/no30%ACK. Allcandidates
release-s/thinLTO/TIM16audit/build/download/resetPASS. AS85initial736snapshot
afterENABLElow invalidcauseevidence(quiesced);736b orderfixed/resetbutbranch
UNEXERCISED. STARTUP_PUBLISH_E736.md. Nextsector5/phaseApostCOM comparator
configvsreference, notproducerfailure/hardwarewall. Goalactive.

E735 actual/rootB277C7F44AD9A9546039164AF5C0D46333D786B60E875A89EE18724BCA2BA211
period_735 installed/OFF,Uartclosed. Fixed concreteforced-sixstep6400tickCCR
on20k3200period: nominal61 previouslyactual12.1875%,80actual16%; BEMFCCRwas
correct. Hardwareplan nowreadsactualARR+1; defaultpurelegacy6400 unchanged.
31startup tests inclallperiod/step/duty combinationsPASS,release-s/thinLTO/
TIM16auditPASS/build/download/reset0. Two62sine starts61drive/60deg and80/0deg
FAILEDdrivenfeedbackstale4at1479/1442us,3/2scanrecords beforetransfer. Avgcause0,
finaloffPASS/noBEMF/30%ACK. STARTUP_PERIOD_E735.md. Fixvalidbutnotlockproof;
correctoldactualdutyclaims. Nextstartupfeedbacktimestamp/cadence refusal,
notomittedleanmetrics diagnosis orhardwareblame. Goalactive.

E734 actual/rootD4468D8D1B95D6ABA4A0CFC7FA746F7C904BFCAA1B895BF1595C9EBB8158B4E0
lowlevel_734 installed/OFF,Uartclosed. Optionalforeground low-speedlevelwake
avg>=1000halfus through sameCOMP persistence/eventguard, flagsclear/stop;
NOTfullAM32pollingport. Release-s/thinLTO/TIM16auditPASS/build/download/reset0,
guard3/18PASS. Known62/61longstartphase60 FAILEDdrivenfeedbackstale4/5accepts
beforerelease; phase0 didrelease6accepts/COM2 then poweredtrackingStale8,
lastaccept336us/sector4,lastpoll1402/feedback1162fresh. Avgcause0/finaloffPASS,
noBEMFhold/30%ACK. LOW_SPEED_REVISIT_E734.md. Levelwake insufficient; next
actualrotor-vs-command/startupmodesequence, notunlatchedeventassumption,
trackingremoval orhardwareblame. Goalactive.

E733 actual/root3486F8E67534F1DE56B1022CAFB2EA542B161975E5FA29D7ABE9D1C730A2750B
trackstop_733 installed/OFF,Uartclosed. Readonlypoststop getter noISRwrites
proves liveeventfaultStale1,lastaccept328us/sector4,lastpoll1402us,feedback
acquired1319us (83usold),COM2/then sector5/no nextaccept; tracking8at1405us.
ADCproducerWORKING, notfirstsector/order/feedbackstale. Oneknown62/61/60deg
longstartup, avgcause0/finaloffPASS/noBEMFhold/30%ACK. Release-s/thinLTO/
TIM16auditPASS/build/download/reset0,27startup+57seedtestsPASS. NextpostCOM
inputadmission/normalpollingIRQchangeover, notADCdebug/flyingcohorts orclaim
1000usguardprovenwrong. TRACKING_STOP_E733.md currentevidence.

E732 actual/root9F482E7CE5D25BBAB7E35EB4A0C3416794AB160997A765CA777DD8803C754049
normalstart_732 installed/OFF,Uartclosed. Correct E731 inference: lean omits
FEEDBACK_N/FIRST_DELIVERY andACCEPT_STATS/tail updates; zero metrics doNOT
prove absentDMA or zeroacceptedintervals. Fastknown62/61/60deg armed then
tracking8at1805us. Originallongtiming sameprofile armed then tracking8at1405us/
COREOBScom2/avgcause0/finaloffPASS, noBEMFhold/30%ACK. Release-s/thinLTO/
TIM16auditPASS/download/reset0.27purestartup tests incltimed-duty-splitPASS.
STARTUP_KNOWN_E732.md. Nextnormalstartup tracking/restart, notdisabledcounter
diagnosis, phasegrind orhardwareblame. Actualfastfeatureoff, goalactive.

E731 actual/root0422884A2C598B65DEB8CEAA11C53CB5B2DF3CF4B5733AC71006057207A8DF08
carrieradopt_731d installed/OFF,Uartclosed. Fixed stale duty_split62acq cap:
startup80 nowpreservesBEMF70, default62path unchanged. Then split731cPANIC,
UARTtimeout/finaloffFAILED; debuggerhalt PANIC_LINE36/PD1ODR0/BDTRc1aMOEoff
convicted prepare_carrier ADC-DMA-owner check. Added explicitinitialtimed
carrieradoption validateactualARR/zeroCCR/outputsdisabled, noUG/DMAstop.
Final731d release-s/thinLTO/TIM16auditPASS,download/reset0. One8%phase0
released6accepts; poweredtimerARMED age238ticks/remaining179/arm6us, then
tracking8at1405us/no nextacceptedinterval/FIRSTFEEDBACKseen0. Avgfault0,
finalUARToffPASS. NoBEMFhold/30%ACK. ExistingCORESEEDmeasured_flying label is
WRONGforbootstrap, STARTUPBOOTSTRAPcommanded1666authoritative. Nextstartup
tracking/continuousproducerhandoff, notflyingcohorts. STARTUP_TRANSFER_E731.md.

E730 actual/rootE68893F86BF002E58F1AEE4D407C431C030D266EDE2B4329B6B924BF169DE7DC
bootstrap_730 installed/OFF,Uartclosed. Optional normalstartup-bootstrap after
6realqualified accepts/epoch>=6/currentvalidinterval uses commanded1666halfus
initialestimate +freshactualaccepttimestamp, NOTmeasuredflyingseed/lock. Default
strictseedpath unchanged, electrical/freshness/tracking/watchdogs retained.
57puretestsPASS,release-s/thinLTO/TIM16auditPASS,download/reset0,guard3/18PASS.
One8%phase0faststart crossedseedgate:reason22release5181us/6accepts/6cmdchanges,
avgcause0. Poweredtransfer REFUSED DRIVEX2/power_reason0, noBEMF/30%ACK.
FinaloffPASS. STARTUP_BOOTSTRAP_E730.md distinguishes commandedbootstrap from
existingDS85measuredqualification. Nextpoweredtransferadmission/setup, not
sixedgecertification orhighercurrentthreshold. Goalactive.

E729 actual/root4C30A1271796519A987B6E479C2140DEBA78B62E0884221C5A8DC00CEB8C8BFC
startlevel_729 installed/OFF,Uartclosed. Optionalstartup-level timerrevisit
unaccepted samecommand postlevel afterstrictgate throughNVIC/persistence,
oneacceptpercommand state; no timeraccept/gatewrite/reset. Release-s/thinLTO/
TIM16helperauditPASS, download/reset0, ownsource4+guard3/18PASS. One8%phase0
faststart FAILED40msseedtimeout reason2,33acc/47cmd,9reanchors,avgcause0,
handler33us/overrun0/finaloffPASS. NoBEMF/30%ACK, NOTprovenAM32pollingport.
STARTUP_LEVEL_E729.md evidence. Levelrevisit insufficient; nextnormalreference
startupchangeover, not phasegrind/extraobserver/thresholdraise/flyingcohortgate.

E728 actualE727995A5B51 unchanged/OFF,Uartclosed. No firmware/flash/guardchange.
Livefixture exposes existing --drive-phase -30/0/30/60 default60 (idleACK).
8%faststarts at0/30deg both40msseedtimeout reason2,30/29accepts of48cmds,
9/14reanchors,avgcause0/handler33us/overrun0/finaloffPASS. NoBEMF/30%ACK.
Earlyfirst26trace missingsteps2/3 at0deg,3/5 at30deg; boundednotwholecoverage.
STARTUP_PHASE_E728.md findings. Next compare oldworking entry adapter, not
currentthresholdraise, hardwareblame or optionalrecovery certification.

E727 actual/root995A5B51E699ED740C8CE5D933A31158D55244F1F8814440244A2ADCC5A9C5BE
startfast_727 installed/OFF,Uartclosed. Optional bench-startup-fast uses stage
20/100/300/400ms, computed handoff520ms; original timing remains available.
Release-s/thinLTO/TIM16 helpergatePASS,25startup+56seedtestsPASS, own guard3/18.
Direct7/8%starts both reached driven handoff WITHOUTaveragefault, then40ms
seed timeout reason2:30/36accepts of48commands,7/11reanchors,handler33us/overrun0.
NoBEMFentry/30%ACK/envelopegain. Bothfinaloffverified. STARTUP_FAST_E727.md
has exacthashes. Next isolate sine-to-sixstep transition against earlier
workingstartup, not highercurrentthreshold or optionalflyingrecovery cohorts.

E726 actual8F72ABE67EF8980A23F3D43A91BD01A680AF016181138380029C5BA53045323D
startduty_726d installed/OFF,Uartclosed. Disabledhardwarecheck E726 software
EXTI wake FAILED pending0 (other3casesPASS); retained deferred_check_726_source.
Replaced SWIER18 wake with NVICpend + separate RETRY_PENDING token in Input
CompExti; real timinggate/persistence/intervalcheck stillrequired. Command and
stop clearretry; stop uses actualcomp_input NVICunpend. 726b/726c disabled4
sourcechecksPASS, actualowner callbackstillnotdisabledtested. 726c realstartup
returnedcleanly (no724silenthang), butSTOP2/40ms seedrefusal:29accepts/48cmds,
33us handlermax/overruns0,8reanchors/3freshintervals, noaveragefault.
Legacy6.2 startupdutycap lifted to10% in timedexplore; sine cap10/BEMF cap30
remain. Fixture exposedstartup/drive duty controls. 8%normalstartupattempt
FAILED average25 at0.693685s; 7%intermediate FAILED avg25 at1.568480s,
residual3215>3185/nonrail, beforehandoff. NohigherBEMFACK/envelopegain, no
nominalthresholdraise. First8%hostinvocation rejectedoldhost62cap beforeRUN;
second syntaxerror beforeUART; third is ONLYpowered8%attempt. Do notpool.
Allcompletedmotorcaptures finaloff+snapshotcount/orderPASS, UARTclosed.
Allcandidatebuilds release-s/thinLTO/TIM16mathgatePASS. Actual726d maytest
furtheronlywithretainedprotections; currentcalibrationstillunverified. Next
normalstartup waveform/mode architecture, notflyingrecovery orhigheravgcap.

E725 ROOT BEFEC320E132D181D5B54C3031327BF1C3EA34DE672AD113928ABD3B4D6282BF
deferred_startup_725 frozen/release-s/thinLTO/TIM16math gatePASS, NOTflashed.
56hostpolicytestsPASS inclstrictgate/owner checks. Found actualreference
gate-closed postlevel pendingcamp (minz am32_isr.rs70-84); removingstartup
ratekill leaves that structuralbusyIRQ mechanism. NOT proven724silentcause.
Timedstartup nowmasks/clears that earlypending input, retains deferredsame-
command obligation; existing50us startupguardtimer reenables/syntheticwake
onlyafterstrictreferencehalfintervalgate. Postlevel stillrequired beforewake;
normalCOMPISR persistence/realintervalread/seed validation stillrequired.
Noaccept/intervalreset/gatewriteintimerhelper. Newcommand/resetcanceldeferred;
nonowner cannotresume. Defaultreferenceadapterunchanged. Runtime/race/revoke/
softwareEXTIwake timing NOTverified yet; hosttestscope onlypuregatepredicate.
Actual723d restoredlastoffE724 remainsinstalled, noUART/SWD/motorE725.
Next disabledhardwaredeferred-source/cancellation tests beforepoweredcandidate.
724silentfailure rootcause remainsunknown; do notcarrycandidateasqualified.

E724 actualRESTORED723d5EF1F7F665CB1647F47DE186ECEF86CFB17EA943BEFA59D572AC9C8BCF13C257
installed/OFF/Uartclosed, restore download/OpenOCDresetexit0, own restored
guard3/18/finaloffPASS captures/startrate_724d_restored_guard.txt. ROOT B2B6
startrate_724d is FAILED candidate, NOTinstalled; do not motorrun it asqualified.
Normalstartup window candidate40ms, rollingforwardmissingepochs and timer-
corroborated longtransient resets, neverjoinsgaps/freshseedtimestampfrozen.
724/724b40ms tests failed beforehandoff; second43accepts/48commands butonly4
freshintervals after6reanchors, no electricalfault/handleroverrun. 724c uses
one measuredfullcycle(6intervals) forNORMALstartup only; flyingdefault12 stays.
Exactboundedmean6 reciprocal32bit,12001inputs exhaustivetest; coreinitial
zero_crosses6 matches seedcount. Stillstopped65dispatches/ms/ratecap64.
724d changed startup rate toobserve/report only, existinghigherprioritytick,
DMA/current/40ms/percall50us watchdogs retained. Test SILENT: hosttimeout,
noCOASTEND/no30ACK/noverifiedBEMF; initialFINALOFF UARTreadbackFAILED.
FirstSWDhalt/memorywriteattempt failed at0xf0000fe4; reset halt reachedPC
080000bc butmemorywritesstillfailed (DO NOT claim direct safingwrites ran).
Restored exact723d and verified actualoff viaUART asabove. Cause unknown,
reset lost livefaultstate; do not infer motorlock or hardwarefault. RootB2B6
release-s/thinLTO/TIM16mathPASS,55hostpolicytestsPASS, notpoweredqualification.
Next offline silentfailure/rate/watchdog/seedhandoff investigation. No more
poweredattempts onfailedB2B6 beforecause/containment; fullgoal unfinished.

E723 actual5EF1F7F665CB1647F47DE186ECEF86CFB17EA943BEFA59D572AC9C8BCF13C257
nopcmp_723d installed/OFF,Uartclosed, flash/OpenOCDresetexit0. New cfg-only
average diagnostic retains zero/allowance/residual/cause/fault raw, poststop.
723 nominal500 start tripped residual2369>1991, cause3/nonrail. Nominal average
target changed to800mA to match operator PSU ceiling (not tuned past failures),
3185 const raw allowance; calibration stillunverified. Timedexplore rawpulse
cutoff nowreport-only/nonrails allowed, rails/average/bus/fault/tracking stay
kills. 723b800/10k startup stilltripped4834>3185; NO furtherthresholdraise.
Found actualrestartbug: prepare_sine restoredmodes butnotARR afterBEMF changed
carrier. Now restores/checksARR beforeUG; startup20k selectable separately.
723c20k reacheddrivenentry/noaveragefault, thenSTOP20 comparatorDMA recorder
full at12.8ms (256samples20k). Removed recorder prepare/start/healthy kill
fromleanIRQ; nohandoffauthority, defaultsretainit. 723d20k reacheddrivenentry
thenSTOP2 local20msdeadline, DS85seedfault2/missingepoch, only2freshintervals;
18accepts/24commands, handlermax25us/overrun0, averagefault0/23485DMAframes.
NoBEMFhandoff/no30ACK/noenvelopegain. Four distinct captures avgdiag_723,
avg800_723b,start20k_723c,nopcmp_723d_direct300 retained. Allrelease-s/thinLTO/
TIM16math gatePASS;25purepolicytestsPASS onnominal800, notfullIRQqualification.
Next normalstartup/handoff policy (reference mode/seed), notflyingrecovery
cohorts or highercurrent threshold. Current calibration and default diagnostics
removal remain incomplete; original32us arm/rate cap stillneedgoalreview.

E722 actual524226CAAA8C6EB2C899A40F3CA4CC01151F071F84D3BFB718C50F9C5B138B54
startup_dma_722b installed/OFF,Uartclosed. Timed TIM15 startup producer now
connected: sine/driven readers use fresh cached coherent scans, no software
ADC transactions during this owner; average consumes each DMA scan ONCE.
Sine->driven validated transfer preserves ADC while revoking output owners;
faults quiesce, foreground coast restores ADC. Original acquisition timestamp
preserved for driven guard; duplicate cache reads skipped. VSENC/neutral are
unavailable in startup capture, marked validity0; new exact ADCorder metadata.
Release-s/thinLTO/TIM16 math gatePASS;25puretestsPASS. Earlier722 image8D36
own baseline4/guard3+18/missinglimitrefusal PASS. Two722b normalstartup tests
FAILED beforeBEMF/higherduty: host50->200 startup average refusal25 after6350
DMA scans/1.276783s; direct firmware100->200 ramp samefault25 after4100scans/
0.824843s. No20ACK/no envelope gain. Captures startup_dma_722b_ramp200 and
direct200, fullsnapshot decode/count/order PASS, finaloffverified. Code25
is monitor refusal, NOT calibrated500mA proof. Nominalgain/sampling/current
calibration remainsunverified; do not blindlyraise1991 or blamehardware.
Next inspect measured residual/calibration before furtheraverage policy work;
no flyingrecovery gate. Legacyphasepulse stop stillneedsgoal reconciliation.

E721 acqclock_721 06B0CBD8AB15C6F740F872747C46D1FF2632FDBF67F54D6F8C5B53FDE4CB2DDC
release-s/thinLTO/audited/frozen NOTflashed. Latest backend ADC clock now
independent of poweredsegment clock reset. Latest retains originalADCstamp;
DMAguard/FIFO receive age-preserving mappedsegmentstamp, both clocks sampled
from ONE serializedTIM17read via Clock::map_stamp. Default backendunchanged.
25puretestsPASS inclmapping acrosssegmentreset/softwarewrap. Needs clockfed
<65ms and liveadaptertiming/race tests; no blackoutwatchdogclaim. Startup
owner/readers remain unported. Actual2B13 lastOFFE720; no UART/SWD/motor.

E720 actual2B137A23DDEEB8FBF65F847E28C6077BBD5F725CD403ADB5D4DB548AB98542C3
quiesce_720 installed/OFF,Uartclosed, checkedflash/resetexit0. bench-adc-latest
fast quiesce onphysicalbridgeoff/ENABLElow masksDMA/stopsTIM15trigger/revokes
cache; noADCSTOPwait. QUIESCED blocks latest/ensure_started untilforeground
boundedstop restores config and clearsstate. Startup ownership stillunported.
Own TIM15 baseline4casesPASS complete128x5@25631us,abort73/wakeinterrupt75us;
own guard3/18PASS. No realpoweredfaultISR latency/equivalence or motor run.
Release-s/thinLTO/math/TIM16 gatePASS; captures quiesce_720_baseline/guard.
Next startup producer/admission and reader replacement; do not activate
DMA besidelegacyADC reads. Goal remains unfinished.

E719 latest_719 7EE81A00CCD90DECB51ADB73A000943A7A4A68F0B43A29B2A1D6A06D556ED991
release-s/thinLTO/audited/frozen NOTflashed. bench-adc-latest opt-in connects
Latest policy to valid coherent BEMF DMA frames AFTER existing currentguard,
before FIFO push; failures stop via stream_fault10, physicalreasonADC11.
latest() serialized/owner+active checked/originalage; resets onmaskedstart/stop.
Average stillconsumesproducer once, never cachedread; FIFO/overflow retained.
20purepolicy testsPASS, no liveadapter race/timing qualification. Startup
owner/admission and readers remain unported, no continuousstartup claim.
ActualDC876 lastOFFE717; no UART/SWD/motor E719. Next bind dedicatedstartup
owner and convert foregroundreaders, not just enablefeature onoldstartup.

E718 pure startup_feedback.rs Latest/Frame cache staged, NOT wired tofirmware.
20realpurepolicytestsPASS (4new cache): originalacqstamp preserved onrepeated
reads, stale>1ms,duplicate/reordered/>1msproducergap latch,wrap,all5rails.
One serialized clockdomain required; cache overwrite is intentionallylossy,
average must consume EVERY producerframe separately, not foregroundcachedreads.
No ownership/peripheral integration or startup-average fix claim. No UART/
SWD/motor/build E718. ActualDC876 lastOFFE717. Next bind dedicatedstartup
producer and replace all foreground ADCreads before activating timedfeature.

E717 actualDC8767D4AA98691F828C5D2D3E3DD3BFE41C0B82099CF9E7BB4B80135C2EE302
TIM15 backend installed/OFF,Uartclosed, checkedflash/resetexit0. Disabled
baseline4casesPASS: none0,abort0@71us,wakeinterrupt0@73us,complete128x5@25633us
then oldbaseline invalidafterrewake. Saved tim15_717_baseline.txt; new exact
--tim15 verifier replayPASS, one test+4backendnegative variantsPASS.
Own guard3/18PASS tim15_717_guard.txt. No spin or continuousstartup proof.
Next exclusive5-channel ADC stream acrossstartup/driven/BEMF: lean startup
must replace foreground ADC reads (including VSENC/neutral diagnostic reads),
not runDMA alongside them. Current ISR/take admission poweredowner-only;
source remains that way. Whole registerrestore readback not independently
tested beyond successful boundedstop/baseline return. Goal unfinished.

E716 actualDA8EB1E7F1099213091363EDC419C730D3B3D25624495E64110123ACC285FC3B
715b installed/OFF,Uartclosed; checkedflash/reset/guard3/18PASS. One ramp20
attempt stopped normal startup@2659091us,tick2650,CAPreason4 beforehandoff.
Finalretainedraw1651/2093/2855,peak807<1200,no rails => average_failed caused
current branch (notpulse/no20ACK). No measured500mA or analog-bias proof.
New bench-adc-tim15 backend usesnative PAC TIM15/APB2 and EXTSEL4; default
TIM3 unchanged, all owneradmission stillstrict. Notcontinuousstartup yet.
CandidateDC8767D4AA98691F828C5D2D3E3DD3BFE41C0B82099CF9E7BB4B80135C2EE302
tim15_716 release-s/thinLTO/audited/frozen NOTflashed. Timer-specific raw/
phase probes compile-refused. Next disabledTIM15 route/restore validation,
then coherentcontinuousstartup ownership; do not simply raise average limit.

E715 actualF96D86F2FEA7ECD703DB35EB077E294D58403FC22D49F706D6FB751AA5AA19C1
avgcause_715 installed/OFF,Uartclosed; flash/reset/guard3/18PASS. Fixed
driven average refusal before ADC row retention: now saves offending sample,
average reason25 vs legacypulse5. E714 retained70rows peak485<1200 but failed
average row was omitted, so oldexactcause not proven. One new ramp20attempt
entered BEMF then Current5 at98383us/184COM, no dutyACK; NOT20% wall.
DMA avg still shared5 on installedimage. Root DA8EB1E7F1099213091363EDC419C730
D3B3D25624495E64110123ACC285FC3B avgcause_715b built/audited/frozen NOTflashed:
DMA average refusal now25 via firstfault-preserving trip_reason, same shutdown.
Code25 means average MONITOR refusal (threshold/missing/rail), not calibrated
overcurrent proof. No threshold change. Next install715b and inspect refusal.

E714 actual39B9F7CED7FB5B190EB6F93CF38053EE74887DD3901D3776C5E8503B26F2ECAB
startavg_713 installed/OFF, flash/resetexit0. Own guard3/18PASS. Live fixture
now --nominal-average ACKs avgnominal and --lean-explore reports finaloff/
controller output without false diagnostic qualification; one reporttestPASS.
One7% normalstartup/10sBEMF exploratory run completed deadline,20269COM,
average987ticks, finaloffPASS (startavg_714_hold70.txt). No sigma/IRQ/ADCamps
qualification; omitted diagnostics zero are not measurements. Next ramp20%
attempt failed INITIAL acquisition Current5 at9410us, no BEMF/no targetACK;
NOT20% wall (startavg_714_ramp200.txt). Average/pulse share code5; subcause
needs disambiguation before retry. UARTclosed. No flyingrecovery cohort.

E713 startavg_713 39B9F7CED7FB5B190EB6F93CF38053EE74887DD3901D3776C5E8503B26F2ECAB
release-s/thinLTO/audit/frozen NOTflashed. Average accumulator now receives
existing startup foreground capture samples and driven foreground feedback,
plus BEMF DMA; same instance/pending block preserved, ENABLElow revokes.
Startup mean/mixed-producer block is SAMPLE average, not uniform timedDMA
10ms average; acquisition-mode/baseline/coverage errors unbounded. Nominal
setting remains uncalibrated. No full protection qualification claim.
16purepolicy +8adapter testsPASS; TIM16 directhelper gatePASS. Existingpulse/
bus/nFAULT/freshness/deadline unchanged. Actual7119 lastOFFE708; no UART/SWD/
motor. Next lean-aware live fixture and hardware trial, not recovery cohort.

E712 nominal_712 877510AF41541A2FA8E42ACFC18FCCD32B795F721D1D4D7D26D02348F4ECE8A3
built/release-s/thinLTO/audited/frozen NOTflashed. Idle avgnominal provisions
1991 raw-count sum/50 scans using nominal500mA,VDDA3600mV,gain10,7mOhm;
emits calibrated0/uncertainty_bounded0. No automatic default or measured gain
claim; offset/sampling/gain errors not bounded. Existing avgraw/missing-config
refusal retained. Wide const intermediate compile-time only; first u32
expression overflowed and failed compile, corrected constu64,16puretestsPASS.
TIM16 helper gatePASS; no nominal helper callers. Actual7119 lastOFFE708.
No UART/SWD/motor. Startup average coverage remains missing; do not call this
nominal setting a verified500mA safety boundary or complete exploration stack.

E711 leanstartup_711 3CD3E0CEB3778168EE490CD7B820DBC7FDC96B17667FAEA619CB2FA8573D4CC4
release-s/thinLTO/audit/frozen NOTflashed. Lean driven TIM6 now omits the
observe-only polling microscope entirely; actual comparator-IRQ seed/handoff
path unchanged, guard/DMA health checks retained. Observer command validation
and other startup diagnostics still present; no zero-all-ISR qualification.
Standalone startup_policy harness linked real minz-core:15 tests PASS, pure
detector/observer/average policy only, NOT full hardware ISR equivalence.
Full host lib attempt after panic cfg hit MCU-vector linker failures; reverted
own lib changes, firmware panic behavior unchanged. No UART/SWD/motor.
Actual7119 lastOFF E708. Calibration/startup average coverage still pending.

E710 leanstartup_710 CF38B901A4E89411055E7838C97F59EB03E2D4430E4EC3227B58605C1D66F4BD
release-s/thinLTO/audit/frozen NOTflashed. Lean driven TIM6 omits READS row
writes/read-count/MAX and bufferfull stop; retains observer.sample and all
preexisting electrical/freshness/deadline/DMA checks. Default path unchanged.
TIM16 direct-helper gate PASS. cargo test --lib host FAILED panic_impl clash
with std; no test executable run, not a semantics pass. Observer internal
counters and other startup diagnostics remain; not zero-all-ISR qualification.
Actual7119 lastOFF E708, no UART/SWD/motor E710. Average startup coverage and
calibration unresolved; do not equate irregular foreground scans with timedDMA.

E709 no firmware/UART/SWD. drv_math_audit.py --forbid-caller TIM16 now enforces
direct emitted helper exclusion; current7119 CLI exit0, oldD5FD exit1.
Two regression tests PASS (saved before/after and exact-name/all-category).
Not transitive ISR analysis, inline-wide-math proof or CPU measurement.
Current plan rechecked: negative same-DMA residual still invalidates naive
gain fitting; calibration/startup average coverage remain open. Actual7119
lastOFF E708. Goal active; no fresh motor/envelope claim.

E708 actual71192D3AB849B33CBB21FC532B125F150D6C4AFA50FEC3E6B51E783F29F4C95E
notrace_707 installed via checked flash/reset exit0, OFF/UARTclosed.
notrace_708_refusal.txt missing-average-threshold startup refusal/finaloff PASS;
notrace_708_guard.txt 3 timerfaults/18 poststop refusals/finaloff PASS.
No spin, gain calibration or CPU measurement. Python process 57848 identified
as pdf-mcp, not a competing UART owner. Goal still unfinished.

E707 candidate71192D3AB849B33CBB21FC532B125F150D6C4AFA50FEC3E6B51E783F29F4C95E
notrace_707 built/release-s/thinLTO/audited/frozen, NOT flashed.
Shared minz COM still recorded ZC_TRACE despite lean binz recorder removal;
batch gating emitted TIM16 uidiv. Added const TRACE policy, default API true,
lean-core false; 22 reference ISR tests PASS including four paired control
sequence cases. New ELF audit has no TIM16 soft-arithmetic calls. No measured
CPU gain or motor qualification. Polling trace/startup diagnostics still open.
Actual D5FD remains installed, last verified OFF E706; no UART/SWD E707.

E706 actualD5FDFC6F3EC0152165786361E39A6FE2CCA55835A89E7F5F253C8EBE7F34BACC
leanirq_706 installed/OFF,Uartclosed. Release-s/thinLTO/audit/freezePASS.
Realno-threshold runrefusalPASS,3timerfaults/18poststopwritesrefusedPASS.
Disabledtest nowcountsactualrefusalslocally, not omittedISRvetocounter.
GUARDPROFILE timing_measured0 explicit; noWCET/CPUclaim. Leanacceptedcounter
also rechecksACTIVE/OBSSTATUS beforeprogress. No calibratedcurrent/motorspin.
StartupIRQcleanup,currentgain/threshold,normalstartup recovery stillpending;
no fullgoal orpoweredleanqualification claim.

E705 ROOT7D6B3B07AA06171BEC381456D8EFAE44F828B178F1FA3E1B8E2E8DE1494C613A
leanirq_705 built/audited/frozenNOTflashed; actualFFFC lastOFFE703.
bench-lean-irq removessteadyDMA/guard max/sums/firstdelivery/queuepeak and
fast-event/cycle counters; retainsdeadline100uscommitcheck/ADCfresh/order/
averagecurrentpolicy. LEANCORE marker preventsdiagidentity confusion.
41normalguard+7averageadapter+1targetedleanpolicy testsPASS; targettest38
othersfiltered, notfullleansuite. Text114948bytes. StartupISR/operational
counters/adapters stillneedaudit; do notclaimzeroallISRdiagnostics orCPUgain.
NoUART/motor. Next hardwarecomparison/remainingstartupISRcleanup; calibrated
currentthreshold and normal-startuprecovery stillpending.

E704 lean-core D413F29734EED0CA1ED6191308658742D8C1045BC8B2F1257DC176FAEE0097A6
built/audited/frozenleancore_704 NOTflashed. ActualFFFC lastOFFE703.
Optinbench-lean-core removescoreEVACCstats/timelines/tail/COMP-COMmax/
COMPcountcap fromrealphysicalBEMFpath; retainsoperationalacceptedcounter,
source/owner checks/referenceISR/guard. NotallISRslean: DMA/guarddiagnostics
remain. Text116928vs125372 bytes, noCPUmeasurementclaim.41guardtestsPASS.
Afterfreeze sourceMonitorsector%6 replacedboundedbranch,testsPASS; NOTyet
inD413ELF. Sourceahead/rootcandidate!=installed. NextfinishotherISR separation
and explicitleanreportidentity beforehardwarequalification; calibrationpending.

E703 actualFFFC averageguard_702 installed/OFF,Uartclosed. No-config run200
refuses beforephase drive, finaloffPASS; savedcapture+negativeverifiertestPASS.
Guard3/18/timer/filter/carrier/CPU2/6/9/ADCroute3PASS. Fullfivepreflight
FAILED onlyarchivecheck ?cmd (optionalcommandnotlinked); not fullPASS.
No calibratedgain/provisionedthreshold/motorspin. E546savedanchor baseline
residual2.762counts forreported70mA, softwarebaselinevsDMApower; inadequate
gainvalidation, nothardwareblame. Next gain/offsetcoverage and disabled
thresholdcrossing, keepuncompletedaverage/startup/leanrequirementsvisible.

E702 ROOT FFFC59AE058DBA8E6E084D37F1544371F97DCD6BE6C165D1AEBD44C7DFFA7733
averageguard_702 frozen/audited/NOTflashed; actualF487 lastOFFE700.
Optinbench-average-current wires freshprestartzero->DMA50scan average->Current
shutdown. avgrawN idleprovision required,no default,no ampcalibrationclaim.
ENABLElow revokes Limit, configretained;7actualadapter/policyhosttestsPASS.
Release-s/thinLTO buildPASS withoutoptional flyingreentryfeatures. Existing
peakguardstillkill, startupaveragingnotcovered; gain/currentratingvalidation
and normalstartup recovery stillunfinished. Next calibrate/provision and
disabledno-config/refusal checks beforepowereduse; not flyingrecoveryqueue.

E701 average policy corrected50scans/10.050ms for201usADC/20kPWM,
not32partialphasewindow.6hosttestsPASS. prestart_baseline::average_zero
returnsfloor(128scan3channel sum*50/128) onlycomplete/nonrailed/sameepoch.
Releasecheck baselineDMA/live/singlecorePASS; firstcheckwithoutatomicbackend
failedCASconfiguration, correctedflag,noflash. Average stillNOTshutdownlinked
or gaincalibrated. ActualF487 lastOFFE700,noUART/motor. Nextconsume freshzero
and calibratedthreshold inADCguard; no nominalamp calibrationclaim.

E700 actualF487 eventreport_698 installed/OFF,Uartclosed, resetOpenOCDexit0.
Own guard3/18,fullfive3200 CPU2/6/9,ADCroute3PASS; disabledonlynomotor.
Average-current policy stillNOTintegrated/calibrated. Existingprestart_baseline
already128scans beforephase drive,epochtoken invalidatedbyENABLEwrites;
gain/independent anchor stillmissing. Do not treat standalonezero_check as
validfuturewake calibration. Next integrate protection, not flyingrecovery.

E699 host-only average_current.rs policy4testsPASS, NOTlinked/livecalibrated.
Samewake zero_block and calibrated limit_block required; signed3shunt sums,
32scans/6.432ms, boundarystraddle up12.864ms; no runtime divide/widemath.
Exactboundary/latch/rails/recirculation/reset tested. CurrentcandidateF487
remainsNOTflashed; actual2F1B lastOFFE697. Next same-ENABLE zero and
calibrated threshold integration, not nominalgain pretending calibration.

E698 stagedeventreport_698 F4870BA620360B7B9156BAC061B131F21E77BD2A1CFA610FDB2EFA22870A9CEC,
NOTflashed. Actual2F1B lastOFFE697. Updatedgoalread from2006cabb attachment:
5%explorationsteps, diagnosticthresholds report,normalstartup recovery,
noISRdiagnosticsqualification, averagecurrent protection required.
REPORT_FAST nowalso reports eventminimum, FASTEVENT count/min postrun;
stale1ms/order/defaultstrict kills preserved.41policytests+3hosttestsPASS,
release-s/thinLTO build/audit/freezePASS. Ramp10->15->20->25->30 steps.
Next candidate preflights thenexploration; averagecurrent calibration/control
stillmissing, do not claim newgoal protection complete or calibratedclamp.

E697 actual2F1B/OFF,Uartclosed. Live25%target ramp stoppedat23%ACK230,
Tracking8 at7.019612s,22856COM/22855acc,raw732,bus10972mV.
NOT25%failure or current/bus trip. Lastaccepted5 at7019507us,
guardstop7019586us:79us later; pendingstep6/counter38. StrongTooFast100us
candidate, but current Tracking enum conflates stale/order/fast; not proven
exactsubcause. CRC/timeline/finaloffPASS. Next separate eventminimum metric
from stale/order protection under revisedgoal; do not retry identicalramp.

E696 actual2F1B/OFF,Uartclosed. Separated exploration from flyingrecovery:
unchangedimage live ramp20%/30sPASS,ACK200,153358COM/153357acc,
finaltail940.632eHz/sigma18.587us,IRQ71.272%,raw754,bus10853mV.
CRC/timeline/finaloffPASS. Host seed500 CLIadded; initialoldloaded argument
failedpostrunvalidation, samecapturedecodedPASS,norerun.3hosttestsPASS.
Nextmeaningfulhigherduty exploration, not9%recoveryqueue. Fullgoalretained.

E695 host-only reserve review, actual2F1B lastverifiedOFFE694, noUART/flash.
53seed500policytestsPASS including20usreserve exact127allow/128refuse,
u32wrap, absolute167tickdeadline,16usarm+4usslack,too-slowrefusal.
Live reserve remains32us; newtest does not deploy or qualify20us.
46usage measured before12usarm; reducing reserve alone cannot fit1kHz.
HIGH_SPEED_RECOVERY E695 identifies untimedpre-arm work and next action.

E694 CURRENT/root2F1B seed500_692 installed/OFF,Uartclosed. Predeclared
two additional9%30s recoveries bothPASS; withE693 currentbuild cohort3/3,
444.299..444.633eHz, sigma24.896..25.339us. No replacement retries,
firmware/flash/guard changes. Second74622COM/74621acc, third74647/74646.
CRC/coherentfeedback/timeline/tracking-loss recovery/finaloffPASS.
Original32usreserve/16usarm retained. HIGH_SPEED_RECOVERY E694 details.
This qualifies9%recovery only, not higher-speed recovery or wholegoal.

E693 CURRENT/root2F1B seed500_692 installed/OFF,Uartclosed;ownpreflightsPASS.
One9%30srecoveryPASS444.633eHz,74677COM/74677acc,sigma25.075us,
IRQ59.560%,raw425,bus10877,seed761,edge-to-arm46us,confirmation9us,
arm12us. Originalguardsretained. Initialhosterror oldseed400 shareddecoder;
genericexplicit400/450/500decoderfixed,samecapturefullreplayPASS,norerun.
Savedprofileselection/duplicatechecksPASS. CRC/finaloffPASS. No3/3claim.
HIGH_SPEED_RECOVERY E693 hash/scope. Nextdeclaredrepeatabilitycohort before
fasterrecovery; nonewpermissionneeded. Fullgoalactive.

E692 ROOT2F1B55C0924C11A247E886856626F6218CA13D3E0268EA55756ABE4A4AF39016
seed500_692 frozen/auditgenerated/NOTflashed;ACTUAL6A20 lastOFFE691.
E691RF85CRCdecoded;cycle4436equiv450.857eHz. Newexplicitbench-seed500/
--seed500 profile667mean/4000cycle/476individual leaves64ticksreserve/
16usarm/12intervals. 52policytestsPASS,release-s/thinLTOPASS. Nominal500
latestarmage51.5us vs E68946usobservation,notWCET. Nextownpreflights then
9%recovery; no guardkill waived/no9%qualification. HIGH_SPEED_RECOVERY E692.

E691 CURRENT/root6A20 seed450_690 installed/OFF,Uartclosed;ownpreflightsPASS.
One9%30scampaign FAILED recoveryacquisitionCycleTooFast:4436ticks<4445,
3checkedcycles/7intervals,elapsed3293us;beforefollow/arm. InitialBEMF2s
untilinjection5298COM/5297acc,raw451,bus11020. CRC/finaloffPASS.
NOTarmfailure/currentfailure/hardwareceiling. No identicalretry/guardchange.
HIGH_SPEED_RECOVERY E691 hashes. Next review acquisitionfullcycle envelope
vs observedvariation before selectingwiderdomain; do not claim9%recovery.

E690 ROOT6A202774EDCB0ED6E624C7D948C04A74EC75E4CF6841CE6D48ED8034C89DD232
seed450_690 frozen/auditgenerated/NOTflashed;ACTUALC7E4 lastOFFE689.
Newbench-seed450/--seed450 explicit741mean/4445cycle/476individual profile,
requirespersistentmode,original64ticksreserve/16usarm/12intervals retained.
52policytestsPASS incl741allow/740refuse and exactage121allow/122refuse,
release-s/thinLTOPASS. Nextownpreflights then9%recovery comparison.
NoUART/motor/flash/no450recoveryclaim. HIGH_SPEED_RECOVERY E690 details.

E689 CURRENT/rootC7E4 persist_direct_688 installed/OFF,Uartclosed;ownpreflights
PASS. One8%30srecoveryPASS388.166eHz,65193COM/65192acc,sigma25.942us,
IRQ58.398%,raw360,bus10972. Confirmation9us,edge-to-arm46us,arm12us,
policy11us. Measured18us loweragevs E68764us,NOTWCET/pairedcausalproof.
CRC/finaloffPASS. Original32usreserve/seed400unchanged. Next bounded seed
domain450 review+qualification leveragingmeasuredmargin. HIGH_SPEED_RECOVERY
E689 hash/scope. No9%recoveryyet/no repeatabilityclaim.

E688 ROOTC7E45AB369060DC7A298F2259CDB1660C99F702C1FA3E20F451433064C9868D0
persist_direct_688 frozen/auditgenerated/NOTflashed;ACTUAL36FA lastOFFE687.
Persistentmode removesprepare_candidate clone before12reads; witness then
NextEdge::edge_persistent runs sharedchecks withoutspeculativerollbackcopy.
Continuationpromotion same directpath. 51policytestsPASS incl validstate
equivalencevs transactional;actualfollow132x8PASS,prevalidationcount0 innew
modeasserted. Release-s/thinLTOPASS. No measuredlatencygain yet;needown
preflights+8%comparison. HIGH_SPEED_RECOVERY E688. Guards/seed unchanged.

E687 CURRENT/root36FA persist_cont_686 installed/OFF,Uartclosed;ownpreflights
PASS. One8%30srecoveryPASS388.217eHz,65201COM/acc,sigma25.720us,
IRQ58.460%,raw351,bus10960. FOLLOWage27us,armage64us,arm12us,policy12us.
CRC/finaloffPASS. Consecutivereadsdidnotreducecompleteedge-to-arm latency.
Next remove speculativeclone/prevalidatework inpersistentmode: once read
proofexists directcheckededgecommit can avoidrollbackcopy. Numericguards
unchanged; HIGH_SPEED_RECOVERY E687 hash/limits. No9%recoveryqualified.

E686 ROOT36FA2EFF417C59210EE30C877E6644BAFC061C0C4FA242F0247F7F95B8225DEA
persist_cont_686 frozen/auditgenerated/NOTflashed;ACTUAL2B5D lastOFFE685.
continue_filters now12reads+sample_candidate+privatewitness carriedwith
edge tuple;promotion requireswitness innewmode,none refuses. Actualhelper
132x8PASS. Firstlink64Boverflow;cfgremovaloldsampledpromotionfallbackfits.
Release-s/thinLTOPASS. NoMCUtiming/newpoweredrun. Needcandidatepreflights
and8%comparison. HIGH_SPEED_RECOVERY E686 scopes. Guards/seed unchanged.

E685 ACTUAL/root2B5D persistence_684 installed/OFF,Uartclosed. Ownguard/full
five/ADCroutePASS. One8%30srecoveryPASS388.538eHz,65255COM/65255acc,
sigma24.202us,IRQ58.259%,raw401,bus10984. Seed860,age61us,arm12us.
FOLLOWonset11636confirmed11676=20us: earlier continue_filters sampledpath
can complete edge before newloop. NOTevidence12readmodeexecuted/timinggain.
CRC/finaloffPASS. DMAmax77us scopeincludespreemption,notbodyWCET.
Nextconvert earlier continuation/proofretention consistently before9%recovery.
HIGH_SPEED_RECOVERY E685 hash/scope. No numericalguardchange.

E684 ROOT2B5DD7839A9DDCF7A90769464047334D8E2DFC4ED1190284B6A00E3AFEDD3A33
persistence_684 frozen/audited/NOTflashed;ACTUAL566A lastOFF unchanged.
FOLLOWPERSISTreads12sampled_dwell0/--follow-persistence explicit identity,
fixtureconfirmationminimum0onlywhenmatchingmarkerselected;oldmode40.
50policytestsPASS,actualfollow132x8PASS. Release-s/thinLTO/codegen1PASS.
Emitted0x080095ae..c8 exactly12volatileCOMPreads,13instructions/read,
nohelper/div/widemathcalls inloop. NOTMCUtimed. Nextdisabledtiming+own
preflights thenpowered8%comparison;32usarm/seed400unchanged. NoUART/motor.
HIGH_SPEED_RECOVERY E684 scope. Initialacquisitionstill20usdwell.

E683 ACTUAL566A lastOFF unchanged,nomotor/flash. Optinbench-follow-persistence
connects12volatileCOMPreads toPendingFollow::finish_persistent; existing
deadline/identity/rollback preserved. sample_candidate cannot autoaccept
throughold40tickdwell. Actualfollow132casesx8PASS. Earlierintermediate
releasePASS; latestcandidate-onlyfixNOTrebuilt/audited yet,rootELFstale.
Needexplicitwiremarker/fixturemode+MCUtiming beforeflash/powereduse.
32usarmreserve/seed400unchanged. HIGH_SPEED_RECOVERY E683 details.

E682 ACTUAL566A lastOFFE680/E681guard,nomotor/flash/UART. Stagedprivate
PersistentLevel witness from exactly12matchingreads,earlyexitfirstflip;
EdgeFilter::confirm_persistent preservescandidateonset/200tickgap/static
levelrefusal. 50policytestsPASS incl12rejectpositions,wrap,gap. NOTlivewired,
notMCUtimed/codegenaudited. NextPendingFollow witness acceptance+follow_sweep
integration; current finishstillrequires40ticks. No fasterrecoveryclaim.
HIGH_SPEED_RECOVERY E682 describes limits. Existing sampled path unchanged.

E681 ACTUAL566A/OFF,Uartclosed; freshdisabledguard3/18PASS, nomotor/flash.
Operatorreviewaccepted:20usconfirm+32usreserve benchchoicesmustbeexamined.
StagedSeed::handoff_with_budget<MIN,BUDGET>,existingcallers64ticksunchanged.
48policytestsPASS incl exactbudget40/64 boundaries,wrap,onset/deadline,
invalidbudgets/profile. No releasebuild/audit yet aftersourceedit.
Next implement real consecutive comparator persistence for recoveryfollow;
do not merely lower two-sample dwell and claimreferenceequivalence.
HIGH_SPEED_RECOVERY E681 design/scope;9%recoverydeferred,notattempted.

E680 same566A/OFF,Uartclosed. One9%30s HOLD PASS444.386eHz,
79988COM/79987accepted,sigma37.663us,IRQ59.997%,raw436,bus10937mV,
149254phase records,DMAmax35us. Startup raw914; no calibratedamps claim.
No dropout/recovery in this run; currentseed400 deliberately unchanged.
Fastcycles11594,min2145us REPORTONLY, notfault. CRC/finaloffPASS.
No flash/guardchange. HIGH_SPEED_RECOVERY E680 hash/command/timing rationale.
Next recovery profile review:450eHz permits onsetage<=60.5us under existing
32usreserve vs recent52..62us; do not claim9% recovery or steady450 ceiling.

E679 CURRENT/root566ACF5BC3CB4A9A21B985238D4EB3638FA558139CCAA681831B1CE7A27FE604
rate_snapshot_679 installed/OFF,Uartclosed. DisabledRATESTOP16cases max5us
failed0 (notfullshutdownWCET), own guard/fullfive/ADCroutePASS. One direct
startup FAILED reason21 IRQ65/64; terminalCNT1711>833 gateOPEN,pending1,
preZC,epoch1,bracket3us. Not preceding64history. One originalsteppedstartup
8%30s recoveryPASS387.860eHz,65141COM/65140acc,sigma25.202us,IRQ58.530%,
raw417,bus10960,age52us/arm12us. BothCRC/finaloffPASS. No guardchange.
Keep established steppedtrajectory; direct experiment didnotimproveentry.
DiagnosticbuildonepassNOTcohort. HIGH_SPEED_RECOVERY E679 exacthashes.
Rate-only harness replaces old5case irqbudgetcheck ONLYin optinimage.

E678 ACTUAL0FD7 lastOFFE677 unchanged; ROOT525327b453eb4ac3de7363832e73adddadb738c10d07cc4b5a25584e6246e7cb
rate_snapshot_678 STAGED,NOTflashed. Opt-in bench-driven-rate-snapshot captures
TIM2count/avg/EXTI18R-F/HALlevel/rising/epoch/bracket ONLYonratefailure,
beforestopclearsflags. RG85 CRC decoder,5hosttestsPASS; release-s/thinLTO
build+mathauditPASS. Snapshotbranch scalar/nohelpercalls, NOTtimed; bracket
excludes finalstores/shutdown. Need disabled terminal-branch timing before
poweredtest. No historical IRQcauseclaim: reference can campclosedgate but
E677 has no snapshots proving that. No UART/motor/guardchange thisentry.

E677 same0FD7/OFF, fixture process finished, saved CRC/finaloff verified.
Host-only --startup-direct run200 removes100->50->200 deceleration; one8%30s
campaign FAILED initial driven qualification: reason21, IRQ65/limit64,
accepts0,max10us,overruns0. No BEMF/recovery reached, no current trip this run.
Ramp105 retained samples100.30->199.10eHz monotonic. Not startup-cause proof.
Original E676 cohort remains2/3. No flash/firmware/guard changes or retry.
Replay currently reports missing FOLLOWMODE (downstream never reached);
use retained DRIVEOBS/DRIVENIRQ for actual failure, not build mismatch.
HIGH_SPEED_RECOVERY E677 captures/hash. Next inspect qualification entry
pending/edge handling and rotor/forced-waveform alignment, not raise rate guard.

E676 CURRENT/root0FD7setup_phase_675 installed/OFF,Uartclosed. OwnpreflightsPASS.
Fixed8%30s cohort2/3: two recoveryPASS388.777/388.898eHz,~28sresumed,
65294/65293 and65315/65315COM/acc,age59/62us,arm12/13us,policy6us,
IRQ59.194/59.255%,raw345/360,bus10889/10841. ThirdFAILEDinitial6.2%openloop
at1.625s,currentADCrail/peakCAPreason4,Craw3271 bracketPWM6192->2243,
elapsed38us. NOTrecoveryfault/NOTprovenharmlesswrapartifact. AllCRC/finaloffPASS.
No identicalretry/guardchange. Fixtureearlystopclassifierfix+regressionPASS
replacesmisleadingmissingbuildmarkererror. HIGH_SPEED_RECOVERY E676 hashes.
No3/3claim. Nextseparatestartupcurrentrefusalfromremainingrecoverytiming;
seed400/32usremaining/16usarmlimitsunchanged, nohigherdutyqualifiedhere.

E675 ACTUAL400712followlast_674b installed/OFF,Uartclosed. OwnpreflightsPASS.
One8%30sFAILEDfollow5stage3reads3gap232ticks116us; reordermerelymovedgap
upstream. CRC/finaloffPASS; no retries/numericguardchange. ROOT0FD7ACB0784E76EC01A3469D5BF2FF4AE1887399BBAA9DD2C3C10DA5267F7316
setup_phase_675 STAGED/audited/NOTflashed. bench-follow-setup-phase sensesonly
expectednextphase atALLpostqualifiedcheckpoints; 12intervalacquisitionunchanged.
Otherphasepostqualificationobservability intentionallyomitted, notclaimed
allphasegapcoverage. FOLLOWSETUPexpected/--follow-setup-phase explicit;132casesx7
and5identitytestsPASS. Needownpreflights+one8%test; watchretainedearlyedgeage.
HIGH_SPEED_RECOVERY E675 completehashes/scope. No highspeedrecoveryclaim.

E674 ACTUAL5CD4D79ADFAF5C0358D4096C2E0095D27B3849B996AAD9FC8278FCC217E0117D
followdirect_674 installed/OFF,Uartclosed. Mergedstage4intofinalwait, explicit
FOLLOWDIRECTv1/--follow-direct. OwnpreflightsPASS;one8%30sFAILEDfollow5gap210
ticks105us,stage5reads1, BEFOREconfirmation/arm/firstDMApublication. CRC/offPASS.
No retries/guardchange. ROOT4007127556E29FCCD2CF0D2662E377A8D198080C089466E0051CB262C07F48D2
followlast_674b STAGED/audited/NOTflashed: stage3allphaseorderrotatedsoexpected
phaselast; retains3samples+originalgapchecks.132loopcasesx6PASS inclstage3order.
Needownpreflights+one8%test; watchupstreamgaps,notassumereorderfix. HSR E674.

E673 CURRENT/root15C99prevalidate_672 installed/OFF,Uartclosed. Own guard3/18,
five3200CPU2/6/9,ADC3PASS. One7%30srecoveryPASS337.518eHz,56683COM/acc,
policy6us/confirm20us/age52us/arm12us,IRQ57.951%,raw320/bus10984. One8%
FAILEDarmmargin:seed878,age206ticks103us,remaining14<64,armed0. Policy6us,
confirm20us BUTstage5reads0: edgealreadycapturedinsetup,notlateconfirm/dwell.
No retries/guardchanges. CRC/finaloffbothPASS; HIGH_SPEED_RECOVERY E673 hashes.
No standaloneprevalidationWCET/cancellationtimingcertificate; poweredtestused
originalgap+armadmission/fullguards atprioroperatingpoint. Nextremove delayed
setup-completededge traversal, notmorepolicyarithmetic orspeedcapincrease.
PORTABLE_WINS adds E669aggregatecopy lesson (codegenonly).

E672 ROOT/staged15C99B5469D9178018AB6C589B9FB03ACC0491631893D1CA63678A6F8FE55A0D
frozenprevalidate_672 BUILT/audited NOTflashed. Actual8DD2lastoffE669. Flat
snapshot+boolworse832Boverflow,revertedOption. Candidateonlyomitmanualsine/rN/c
commands: fitsrelease-s/thinLTO; a/ADCcapture/run/livecontrol/guardsretained.
FOLLOWPREVALIDATE v1 +fixture--follow-prevalidate required;3identitytestsPASS,
132loopcasesx5PASS. Emittedfollow_sweep stack268vs92bytes;two memcpycallsstill
exist(baselinealso2), no standalonePendingFollow symbols(inlined). NoWCETgain
claim. Nextown disabledpreflights/timing/cancellationgap evidence beforepower.
NoUART/flash/motorE672. HIGH_SPEED_RECOVERY E672 fullstate/limits.

E671 SOURCEopt-in bench-follow-prevalidate connectsPendingFollow toactual
follow_sweep duringexisting40tickwait. Onlyfinalcandidate; completed-firstread
keepsdirectpath; cancel/samplefault dropsrollbacktoken.132actual-loopcasesx5
modesPASS withhostcounterprovingnewbranchexecuted; NOTtiming. Release-s/thinLTO
candidateLINKFAILED780Bdata/800BtotalFLASHoverflow. NOcandidateELF/flash/UART.
Root8DD2 isSTALEprevioussuccess,actualsame lastoffE669. No guards/features
removedtofit. Needcode-size/emittedcostwork and explicitfixturewireidentity
beforepowereduse; no prevalidationmarker/flag yet. HIGH_SPEED_RECOVERY E671.

E670 SOURCEONLY PendingFollow transaction staged in flying_acquire.rs; NOlive
caller/build/flash/UART/motor. Actual/root8DD2 lastverifiedoffE669, notfreshly
queried. prepare_edge speculatively runs originalAcquisition::edge whileholding
exclusiveborrow +fullsnapshot; drop/cancel restoresallfields; finishrequires
samephase/level/onset, originaldeadline+dwell, latchesoriginalresult/refusal.
11520 differentialcasesacross8000/666,5716/476,5000/476 comparefullstate/results
PASS,47RusttestsPASS via --next-edge --seed400; cancellation/mismatchtested.
This movesworkconceptually into20uswait, NOTmeasuredsaving. Snapshot/returncopy
cost, RAM/FLASH and cancellationextraMUXgap remainunmeasured. Next actualfollow
loop opt-in integration+M0emitted/timing review; donotflash anunusedpolicyclaim.
HIGH_SPEED_RECOVERY E670 scope. No guard/speed/confirmationchange.

E669 CURRENT/root8DD2B04882A5FCA8AB7CBA38B19A305A60C19043E5F2C13F03CF78D4FD1C76DB
followscalar_669 installed/OFF,Uartclosed,NOTrecoveryqualified. NextEdge::edge
explicit scalar Result match removesTWO11byte __aeabi_memcpy calls inemitted
code,stack44->28; allacquisitionchecks/history unchanged.45policytests+
132followcasesx4modes PASS, release-s/thinLTO/audited, ownpreflightsPASS.
One8%30s attemptFAILED beforearm:seed862,edgeage162ticks,remaining54<64,
confirmedage66ticks33us,policy46ticks23us/dma0,arm0. CRC/finaloffPASS.
No identicalretry/guardchange. E66827uspolicy vs23us isnotpairedWCETgain;
confirmationdelaygrew, endtoend81us. KnownqualifiedB1E54frozennotinstalled.
HIGH_SPEED_RECOVERY E669 hashes. Needshrinkpostconfirmation path,notseedcap
ratchet or declarecopyfix sufficient; repeatedCPU arithmetic was notprovenwall.

E668 sameB1E54 installed/OFF,Uartclosed,nocode/flash/guardchanges. Two declared
additional7%30srecoveriesPASS =>3/3withE667:337.790..338.576eHz,arm12usall,
IRQunion57.629..58.069%, originaldeadline/ADC/CRC/finaloffPASS. One8%30s
recoveryPASS388.112eHz,65183COM/65182acc,resumed27.991496s,seed865,age124ticks,
remaining92ticks,arm12us,raw360/bus10614,IRQ59.021%,COMP40/COM38/commit14us,
DMAmax77us (preemptible duration), firstfeedback830+91us within1ms. One8%pass
notcohort. HIGH_SPEED_RECOVERY E668 hashes/evidence. Next higher-speed recovery
architecture: currentseed400 and32usremainingfloor remain; age62us already
exceedsconfigured~42uswait at1kHz. Do not justraise seedcap or repeat7%.

E667 CURRENT/rootB1E54D5FB0FF5FE0B44C4C3241CECEC5BD5BCB0A046417352814FADF09897D74
preparedfinal_667b installed/OFF,Uartclosed. One7%30s recovery PASS338.576eHz,
56862COM/56861accepted, resumed27.990734s, arm12us<=16, remaining102half-us,
raw363/bus10984, ADCphase139257, CRC/timeline/finaloffPASS. NOTrepeatability
orhighspeedqualification. DMAmax77us includespossiblepreemption,notmeanCPU.
Earlier667 B21D duplicateinitremovalalone FAILEDsame122us stage4gap; retained.
Finalstage4 nowexpected-phase-only forprepared+expectedfeatures; stages1/2/3
unchanged, no gap/dwell/deadline relaxation. Ownpreflights3/18,five3200CPU2/6/9,
ADC3PASS;132followcasesx4modes and96activationcasesPASS. Postrunfixturefirst
failedoldCOM64expectation; addedexplicitprepared+armedCOM0check/mutationtests,
SAMEcapturefullyreplayedPASS,no rerun. HIGH_SPEED_RECOVERY E667 provenance.
Next boundedrecoveryrepeatability/fullCOMfeedbackbudget; original32usadmission
stillretained, notyet1kHzrecovery. Memo correctionsremainE634response.

E666 CURRENT/rootAC54842890D3D3D573947E0905D71A860F1CB4092D3C7D4F7B7C021D8DEEB22A
preparedhandoff_666 installed/OFF,Uartclosed,NOTrecoveryqualified. Fitsrelease-s/
thinLTO afteromitirqtail,removeuncalibratedshellmA,omitLEDdemo incandidate.
ADCvoltages/guards retained. Marker+--prepared-handoff exactidentitytestsPASS.
Own guard3/18,five3200CPU2/6/9,ADC3PASS. One7%30srecovery FAILED follow5 at
stage4,reads3,gap244ticks122us>100us; BEFOREpublication/arm. DMAmax26us.
CRC/finaloffPASS,no retries. Initialsegmentreached2sinjection; no recoverypass.
Newcounterprepare betweenservice_feedback andfollow4 is placement toreview;
do notblameTIM16hardware orrelaxthegap. HIGH_SPEED_RECOVERY E666 hashes/flags.

E665 STAGED bench-prepared-handoff realrecoveryadapter,NOTbuilt/flashed.
PrepareTIM16 inhibited afterguard/DMAstart beforefollow4; consumesone-shot
latch, mapsqualifieddeadline, finalmetadata/unmaskatCOMpriority0. Timerstop
revokeslatch. Ownershipchecksatomic atprepare; publishcalledinsidearmCS.
Old32usremainingfloor/16usfullarmbudget RETAINED. FLY_SEEDED nowtrueonlyafter
successfulpublication+timingcheck. Mockedactualpublishwrapper32cases+late/
duplicatePASS,5purepolicyRust+M0PASS. Fullbuildoverflow384B; noirqtail192B;
noinlinevariant288B,reverted. LatestatomicprepareeditNOTrebuilt. ROOT04BC
isSTALEdiagnostic, actualEA332lastoffE664; nohardwarecommands/no freshreadback.
HIGH_SPEED_RECOVERY E665 scope. Needcode-sizecleanup, candidatewiremarker/
fixture support,fullpathtiming/firstCOM+agedfeedback beforepowereduse.

E664 actualEA332 restored/OFF,freshguard3/18PASS,Uartclosed; ROOT04BC49
diagnosticDIFFERS. Sameimage COMpriorityA/B:128 maxlate23us/DMA23;0 maxlate4us/
DMA25. Both32/32one-shot,guardfault0,>=3realfeedback/trial,gatesoff. Guard0
andDMA64unchanged. Own192disabledPASS. Supportspriorityblocking inTHISprobe;
notE478COMPtheory revived/notfullCOMWCET or agedbaselinequalification.
Frozenpreparedpriority_664/HIGH_SPEED_RECOVERY E664 exacthashes/tests.
Next integrate preparedtransaction/first-COM ownership with real body and
freshnessbudget; no moreidenticaldiagnosticcohorts or blanketlivepriorityflip.

E663 actualEA332 restored/OFF,freshguard3/18PASS,Uartclosed; ROOT246E96
diagnosticDIFFERS. Newpreparedload actualguard0/DMA64+diagnosticCOM128,
allgatesoff,realADCbaseline/600us windows,32deadlineoffsets. Own192disabled
casesPASS; loaded32/32one-shotIRQs,guardfault0,>=3feedback/trial,COMmaxlate24us,
DMAmax24us. TimingNOTqualified/no fullCOMbody/no oldbaselineage830us tested.
Freeze preparedload_663; HIGH_SPEED_RECOVERY E663 exacthashes/tests.
Next bounded COMpriority A/B while retainingguard and trackingADCfreshness,
notmoreARRmicro-optimization or transferringdiagnosticlatencytopoweredCOM.

E662 actualEA332 restored/OFF,freshguard3/18PASS,Uartclosed; ROOT5F8C41
diagnosticDIFFERS. Prepared mappingTIM17-before/TIM16/TIM17-after underIRQmask,
late-endpoint mapping reports<=4half-us ticksquantization,widebracketrefuses.
5pureRusttests incl3200mappingphase/wrapcases, M0emittednohelpers PASS.
One disabled192casehardwarePASS64accepted/128refused, mappingmax2us,
publicationmax3us,origin1us,deadlineerror1us. Frozenpreparedmap_662/fullhash
HIGH_SPEED_RECOVERY E662. NoCOMISR/DMA/guardload inprobe; no poweredintegration
or changed32uslivefloor. Next loadedpreparedtransaction and one-shotCOM
ownership/firstfeedback budget,notmoreidenticaldisabledcohorts.

E661 actualEA332 restored/OFF,freshguard3/18PASS,Uartclosed. Exact4B6B
disabledprototype rerun again0accepted/160refused/32failed,2us; reproducible
badcandidate,NOTfixed. Emittedpredicatesinspected; no provenbadregister/cause.
Breakpointattempt(c958,setthenOpenOCDshutdown,UARTcommand/close,reconnect)
failedtoreadPC: no registerevidence/no completedcapturefromthatattempt.
Restore/reset succeeded. Root0FA65 diagnosticdiffersactual. HIGH_SPEED_RECOVERY
E661 retains capturehash. Retireopaque4B6B ratherthanmoreidenticalruns;
reasoncodedcandidate stillneeds cross-timerorigin+loadeddeadlineverification.
No motor/no guardchange/no highspeedrecoveryclaim.

E660 actualEA332 restored/OFF,guard3/18PASS/Uartclosed; ROOT0FA65 diagnostic
DIFFERS. bench-prepared-timer adds disabledonly preparedcheck, no powereduse.
First4B6B image all32nominalREFUSED(capture660); reasoncoded0FA65 then32accepted/
128invalidrefused/failed0,publicationmax3us,origin1us,deadlineerror1us(660b).
EarlierfailureUNEXPLAINED, notdeclaredfixed/reliable. NVICmaskedthroughout;
tests coverlate/staleNVIC/staleUIF/stoppedcounter,notCOMISR/freshnessunderload.
Release-s/thinLTO builds/auditsfrozen preparedtimer_660/660b. Parserretainsfail,
mutationsPASS; purepolicy4PASS. Nomotorcommands. HIGH_SPEED_RECOVERY E660
exacthashes/limits. Nextbisectnominalrefusal/codegen andcounter-originmapping;
do nottransplant3us intoliveguard orclaimhighspeedrecoveryqualified.

E659 HOST-ONLY prepared_handoff.rs prototype, NOT linked/flashed. PriorE658
wasprogress: coherentcounter/timing evidence. Currentedge-then-fullarm cannot
fit1kHz evenwithoutDMA (20usconfirm+32usfloor>41.5usreferencewait).
Newpurepolicy startsinhibitedtimebasebeforeedge, publishesEXACTqualified
onset+referencewait intoabsoluteARR; no predictededge/free-run. Missing/late/
cancelled/expired/duplicateeventsrefuse.4Rusttests(2280wrap/deadlinecases),
M0-s emittedpublish/service no BL/BLX/div/aeabi; actualreference3tests+
conditionalbudget4PASS. Hypothetical8uspublicationbound NOTmeasured/notlive.
No guardchange/UART/motor. ActualEA332lastoffE658. HIGH_SPEED_RECOVERY E659
records missing hardwareadapter,timerorigincoherence,qualifiedinput/ownership,
freshness/ISRdeadline evidence. Next disabled-only preparedtimer primitive,
notanother8%cohort or lowering32us inexistingfullarm.

E658 CURRENT/rootEA33255560112765FC0EFAF2214B10CC493BD3B61BFAC9CE0CC1EA2881A0EFDB
snapshot_658 installed/OFF,Uartclosed. Short COMP/TIM17/DMA snapshots now
IRQ-coherent; endpoint too. Emitted5/7/5 loads/stores inside masks,no calls/
loops; NOT measuredWCET. Qualification/dwell/arm remainunmasked/unchanged.
132simcases in3modes+parser/cleanupPASS; ownguard3/18,five3200CPU2/6/9,
ADC3PASS. One8%30srecovery armREFUSED:seed878,age115us,remaining-5us;
policy45us/dma1,DMAmax26. Thisplacescompletedscaninsidetimedbracket,not
all45usDMA nor aWCET. CRC/firstarchive/finaloffPASS; noidenticalretry.
No8%recoveryqualification. HIGH_SPEED_RECOVERY E658 hashes/evidence.
Next eliminate actual deadline-path work or justified scheduling conflict;
do not return to ambiguous attribution or maskthrougharm withoutfreshnessbudget.

E657 CURRENT/root443836A7E9F86E61673AC12EEDE51A4E52F41911AED9DDC9A0ED108E73605058
cleanup_656 installed/OFF,Uartclosed. Own guard3/18,five3200CPU2/6/9,ADC3PASS.
One8%recovery armREFUSED margin-4.5us,age114us,policy17us/dma1. Afterfailure
NOreset: MCPa returnedVDDA3317/VBUS11760mV,p/i alloff/nFAULT1;closedUART.
Thenone7%10sholdPASS338.000eHz,20280COM/20279acc,raw321/bus10972,finaloff.
CRC/offPASSboth. HardwareevidencecleanupallowsforegroundADCandrestartafter
armrefusal; notproofallhistoricalstartupcauses/recoverypass. Counterspan
includesreadsoutsidetimer: dma1+policy17 disprovesusingcountaloneasexact
preemptionlocation. Nexttimestampsmustbecoherent COMP/clock/counter before
furthercausalattribution; mainrecoverydeadline remainsopen. HIGH_SPEED_RECOVERY
E657 exactcaptures/MCPdata; PORTABLE_WINS cleanup lesson. No thresholdchanges.

E656 ACTUAL305B(busscale_655)installed/OFF,Uartclosed. Own guard3/18,five3200
CPU2/6/9,ADC3PASS. At8% first30s recoveryPASS388.224Hz/65201COM=acc,DMAmax35,
policy16/dma0,margin35us. TWOdeclaredrepeats:02armFAILmargin-4us,policy45/
dma1,DMAmax27;03startupFAILCAPreason5,n1,lowbusmessage. Cohort1/3,notfixed.
AllCRC/finaloffPASS. FOUNDsourcecleanupbug: coast_run_inner earlyreturnsafter
DMAstartbypassadc_stream.stop atnormalbottom; laterforegroundADCmayinherit
DMAconfig. STAGED/root443836A7E9F86E61673AC12EEDE51A4E52F41911AED9DDC9A0ED108E73605058
frozen cleanup_656 NOTflashed. Wrapperallreturns gatesoff/enlow THEN adcstop;
originalnormalcleanupkeptbeforecoastcapture. Hostwrapperorder/resultsPASS,
release-s/thinLTO/audit. Needownpreflights+refusal->ADCread/nextstartup test,
notqualificationtransfer. HIGH_SPEED_RECOVERY E656 captures/provenance.

E655 STAGED/root305B7CF05A45F6A6580179DC934420C648910CB7AB9510203A4B3F993DB69086
frozen busscale_655 NOTflashed;actualD562lastoffE653. Exactbus_scale helper
normaln<=4095 replacesfloor(n*1194/100) with12n-ceil(6n/100),32bitbounded
reciprocal20972/2^21; constoverflow/errorproof. Exhaustive0..65535+widewrap
tests2PASS,release-s/thinLTO/audit. Emittedconvert0800c03c hasONEvrefuidiv,
normalbusscale nohelper/64bit/division; outofrangefallback preserveslegacy
wrapping1194multiply/div100. Constantsfolded125832/2076228. NoUART/flash/
motor,freshsafeclaim or timinggain. Needcandidateown disabledchecks then
one8%diagnosticcompareDMA/policy/arm;do notclaimarithmeticfixsolvespreemption.
HIGH_SPEED_RECOVERY E655 explainsdomain/proof. Goalunfinished.

E654 offline schedulingreview, NO firmware/flash/UART/motor. D562lastoffE653.
4conditionalbudgettestsPASS scripts/test_recovery_schedule_budget.py,NOTWCET
orproductionadmission. Counterexample baseline832/pending53/defer60/DMA42
leaves13usfreshnessslack; addCOM45 =>-32us although201usscanbudgetpasses.
pending53illustrative,notmeasuredIRQarrival. Policy-onlymaskmovesDMAafter
unmaskbeforearm,sametotaldelay. Needindependentfirstfeedback/lease/firstCOM
deadlines. NextinvestigateEARLIERfirstDMApublication withcontinuousCOMPand
singleADCowner,ormeasuredshorterpublicationpath; no naivepriorityflip.
HIGH_SPEED_RECOVERY E654 sourceconstraints; fullgoalactive,notblocked.

E653 CURRENT/rootD562078278F8E07E39BB8A9F28F16B1E5E0BD1A4FBEF5AE5C83DB8B570404BA5
frozen nextdmaobs_653 installed/OFF,Uartclosed. ExistingSTATE.count volatile
loadsaroundqualification,NOnewISRwork. One8%30sdiagnostic FAILED armage119us/
remaining-9.5us,policy60us,dma1,handlermax42us. ThusDMAprogressoverlapswide
qualificationbracket; preemptionleadingcause,notisolatedCPU60us. Counter
bracketslightlywiderthantimer; do notclaimexactinterruptentrytimestamp.
Own guard3/18,five3200CPU2/6/9,ADC3PASS;132simcases3modes/4hostPASS;
release-s/thinLTO/audit. CRC/firstarchive/finaloffPASS. Noarm/pass/retry.
NextreviewWHOLEqualification->arm->firstCOM scheduling,notjustmaskNextEdge:
pendingDMAwouldotherwise runafterunmaskbeforearm; DMA64 outranksCOM128,
guard0 andfreshnessdeadline muststayserviced. NoIRQpolicychangestagedyet.
HIGH_SPEED_RECOVERY E653 exactevidence. StartupE65203separateunresolved.

E652 CURRENT/root3F3E34FB7442955B6FA2B5949DE131D5D850D966A738A6FA331D13C51A2AFA1A
frozen nextcost_652 installed/OFF,Uartclosed. One finalread->policyreturn timer
probe,postrunFOLLOWREADS policy_ticks. At8% first30srecoveryPASS387.210eHz,
65032COM=accepted,remaining35us/arm7. policy16us,resttoarm37us. TWOdeclared
repeats:02 armFAILED remaining-9.5us,policy60us,rest38us;03 startupFAILED
CAPreason7,n0,energized7993us,maxcontrolinterval6992us,noDRIVEX/FOLLOW.
Cohort1/3campaigns,notrepeatable8%. AllCRC/finaloffPASS,Uartclosed. ~44us
extra delayINSIDEfinalread->policyresult onfailedcase; DMAmax42us plausible
preemptor,NOTproven. Timerprobe includespreemption,notpureCPU/WCET. Next
checkexistingDMAIRQprogressacrossthisbracket orboundedmaskA/Bwithguardbudget;
do notoptimizeafterreturnbasedonold97usclaim. Startup03 separateunresolved
class,noidenticalretry. Own guard3/18,five3200CPU2/6/9,ADC3PASS;132simcases3
modes/4hostPASS;release-s/thinLTO/audit. RemovedduplicateRTTbootprinttofit
32Boverflow;UARTbannerretained. HIGH_SPEED_RECOVERY E652 hashes/limitations.

E651 CURRENT/root0C5A88E49D21EE58CCC107C3CDB8D39F5D675FD95EF4D26D6D454DEAA5357EA7
frozen nextdefer_651 installed/OFF,Uartclosed. DMAguard+readyseed skipsstats
drainonly,keepsowner/offchecks; waiting/nonDMA pathsunchanged,FIFOretained.
Captured-stageprobe retired afterE650answer;historicaldecoderkept.132simcases
inTHREEmodes+4hosttestsPASS;release-s/thinLTO/audit. Initialbuildwithmarker
overflow32B; nofailedELFflashed. Own guard3/18,five3200CPU2/6/9,ADC3PASS.
One8%30srecovery FAILED armage117us/remaining-7.5us,seed876,realstep1->2,
onset11724/confirm11764,reads0. Noarm/pass. CRC/firstarchive/finaloffPASS.
CaptureSHA EB9D3D2D57B349FB5CB65CBBA3BFCFEB31BDBFADEABAF94FD81534A927941FA0.
IMPORTANT confirmed isFINALCOMPread BEFOREEdgeFilter/NextEdgequalification,
NOTsoftwareacceptancecomplete; previous107us includespolicyexecution too.
ELF NextEdge.edge callsAcquisition.edge+two__aeabi_memcpy; nohotuidiv found
inthatdirectcalllisting(mean_twelve_fallback conditional remains). NoWCET
claim; nextsplit finalread->policyreturn->arm beforemoreoptimisticmicrofixes.
HIGH_SPEED_RECOVERY E651. Old3E19 7%cohort nottransferred; fullgoalunfinished.

E650 CURRENT/root7123F4547EB27C4EF6C75E2D855075F37C38469196C6B8EFB71D2A0EF982811B
frozen nextorigin_650 installed/OFF,Uartclosed. One8%30srecovery on3E19 FAILED
armage126us/remaining-16.5us. Stage-marker7123 own guard3/18,five3200CPU2/6/9,
ADC3PASS thenone8%diagnostic FAILED armage127us/remaining-18.5us. FOLLOWREADS
count0 captured4: realedgeconfirmedINSTAGE4,20usconfirmation then107ustoarm.
No recoveryarm/pass. RealDMAfirstframeacq15/accepted95us. CRC/firstarchive/
finaloffPASS both. Marker addsoneforegroundstore/read+postrunfield,notISR;
132simcasesEACHmode/4hosttestsPASS,release-s/thinLTO/audit. Old3E19 7%3/3
nottransferredto7123. HIGH_SPEED_RECOVERY E650 exacthashes/failures. Next
investigate deferring stats-only service_feedback AFTERreadyseed onDMAguard
path: DMAalreadyvalidates/publishesguard; preserveFIFO/allframes/overflow/
abort/owner/offchecks, draininresumedloop. No suchoptimizationstagedyet.
No marginwaiver/retry or highspeedrecoveryclaim. Goalactive,notblocked.

E649 CURRENT/root3E19D0544FEA332D5E6A050A6E52C48B5CC233788D85A87092AA17ACD9219A01
frozen nextseed_649 installed/OFF,Uartclosed. Moved seed-only S/history/wait/
12count initialization BEFOREsetup_follow3/guardsetup, exactsameprovisional
seed/wait, noearlytimerarm/timestamprefresh. One7%30s recoveryPASS thenTWO
predeclaredrepeatsPASS =>3/3,currentbuildonly,337.518..338.364eHz. Realnextedge
qualified; armremaining43.5/34.5/34us,arm7us each,original32usfloor/deadline.
Resumed~27.990s each,COM=accepted56708/56826/56683,raw317/331/309,busmin
10889/10925/10925. Originalinjectedfault8/CRC/ADC/phase/timeline/finaloffPASS.
IRQunion~59.2..59.4% softwarebracket,NOTCPUutilization. Own guard3/18,
five3200CPU2/6/9,ADC3PASS,132simcasesEACHmode/4hostPASS;release-s/thinLTO/audit.
No failuresexcluded/no guardschanged. Cohort captures/nextseed_649_cohort.csv;
HIGH_SPEED_RECOVERY E649 hashes/table. PORTABLE_WINS/BEMF_PARITY_PLAN updated.
Nextone8%recovery onthisbuild toexpandnext-edge range,notmore7%repeats. No
highdutyrecoveryclaim; older911A20%cohort/22%holdnottransferred. Goalunfinished.

E648 CURRENT/root45B486599F34D8CA4B6806712276180391D6633FBEB7D7905579FDF94F49E7C9
frozen nextbridge_648 installed/OFF,Uartclosed. One7%30s recovery reachedREAL
FOLLOWEDGE1 step3->4,onset14230/confirm14270,interval998half-us. No samplinggap;
cached duringstage4,stage5reads0. Then armREFUSED remaining62ticks31us<32us,
edgeage188ticks94us. NOTrecoverypass/noarm. RealDMAacq15/accepted95,age849.
Initial4030COM/4029events,raw302/bus11140;CRC/archive/finaloffPASS. Previous
same-turn23B2 nextlast_648 FAILED stage4/read3/gap129us: successorLASTonly
movedgapupstream. Retired. Currentkeepsoriginal3visitorder thenadditional
real successorvisit atstage4; noreset/timestamprefresh/thresholdchange.
Bothown guard3/18,five3200CPU2/6/9,ADC3PASS.132simcasesEACHmode+4hostPASS;
release-s/thinLTO/audit. Initialbridgeoverflow32B fixedbysharedresult/refusal
branch,notguardremoval; nofailedbuild flashed. HIGH_SPEED_RECOVERY E648 hashes.
Nexttrace workafterrealconfirmation toarm; retains32usfloor/originaldeadline.
No repeatedattempt or marginwaiver. Bridge succeedsatsensing,notfullrecovery.

E647 CURRENT/rootB26D0FBA0BB6CD7E72986D32B49326DD3EC325556FCE153D18D88E1B5E09B952
frozen nextborrow_647 installed/OFF,Uartclosed. Two distinct builds/one7%30s
recovery each: counter0C5C FAILED stage5/read1/gap122us; in-place borrowB26D
FAILED stage5/read1/gap110us. Bothown guard3/18,five3200CPU2/6/9,ADC3PASS;
CRC/firstarchive/finaloffPASS. No followededge/arm/recoverypass. Different
priorsectors1vs3;12us delta NOTisolatedcopycost. RealDMA delivered both.
Borrow consumesFOLLOW_VALID once but keepscontext inplace; ISRonlyrevokes
validity,nevermutatescontext.132simcasesEACHmode/4testsPASS. Firstrelease-s
buildoverflow64B; ineffective noinline retired, shortened shellhelp only,
release-s/thinLTO/auditPASS thenflash. No staleELF flashed. Electricalguards
unchanged. Nextinspect lastsetup sweep: expectedphase sampledFIRST thenother
two+controllerpreparation beforefinalread; considerexpectedLAST atstage4
without resettingfiltertimestamps. No suchchange staged yet. HIGH_SPEED_RECOVERY E647.

E646 ACTUAL8138ECC(nextphase_645)installed/OFF,Uartclosed. Own guard3/18,
five3200 CPU2/6/9,ADC3PASS. One7%30s recovery FAILED follow5/stage5 gap244ticks
122us,expected_phase_only1. RealDMA acq14us accepted95us,initialage859,fault0,
scans1/raw18/bus11605,DMAmax41. NoFeedbackStale/nofollowedge/noarm. Initial
4029COM/4028events,raw315/bus10984,injected8;CRC/firstarchive/finaloffPASS.
Capture nextphase_646_70_30s SHA8C63868233B03B562BB0B360640EC4181C8BDCD453ACFD0509CE6143C145209D.
STAGED/root0C5CC8419406ECD225443BE44537868932AEDF3EBC901D79CA4B58B730001B25
frozen nextreads_646 NOTflashed. FOLLOWREADS foreground sample-attempt counter
reset perstage distinguishesfirstreadsetupgapfromlaterwaitgap; notISRprobe,
not timingneutral.132simcasesEACHmode+helperchecks/4hosttestsPASS;release-s/
thinLTO/audit. SameE645features. Needownpreflights thenone7%recovery withsame
fixtureflags; inspectFOLLOWREADS beforechanging scheduling. No guardchange.
HIGH_SPEED_RECOVERY E646 exactevidence; no recoveryqualification transferred.

E645 ACTUAL74128B(nextround_644)installed/OFF,Uartclosed. Own guard3/18,
five3200 CPU2/6/9,ADC3PASS. One7%30s recovery FAILED follow5/stage5 gap240ticks
120us. Roundrobinprovisional confirmedfinal_visits0; earlyDMA FIRSTFRAME real
acq15us accepted96us,initialage850,framefault0,scans1/raw23/bus11701,DMAmax41.
NoFeedbackStale; nofollowedge/arm. Priorstep1/seed998/12ints. Initial4031COM/
4030events,raw352/bus10877,injectedfault8;CRC/firstarchive/finaloffPASS.
Capture nextround_645_70_30s SHA8E0ADF11D6375743F9385BD85A42F54B70D469AFAFE6A67495F0102FB63061E7.
STAGED/root8138ECC57EDE6C1950800135AFCC94764E7E2B7D5F70634538364E1F4B787D8C
frozen nextphase_645 NOTflashed. Opt-in bench-follow-expected-phase dedicates
finalwait toknownsuccessorphasewithoriginalfilter;allthreeacquisition/setup
unchanged. OtherphaseedgesNOTobserved duringfinalwait;labelseparatecoverage,
sameorder/cycle/dwell/gap/deadline/electricalpolicy.132simcasesEACHmode+helper
checks+3provenancePASS;release-s/thinLTO/audit. AddfeaturetoE642build AND
--follow-expected-phase tofixture; strictFOLLOWMODE marker. Needownpreflights
thenfixed7%recovery. HIGH_SPEED_RECOVERY E645 tradeoff; no recoverypass.

E644 ACTUAL38812A(nextdma_643)installed/OFF,Uartclosed. Own disabledguard3/18,
five3200 CPU2/6/9,ADC3PASS. One7%30s recovery FAILED upstreamfollow5/stage6
gap222ticks111us,priorstep5/seed999/12ints; REENTRY5/resumetime0, DMAchange
NOTreached. Initial4030COM/4029events,raw287/bus11020,injectedfault8;CRC/
firstarchive/finaloffPASS. Capture nextdma_644_70_30s SHA
6B5572EFF4BBD4DD8654569A1BBC855E4533D4C11B2F121AE423FEA709FF78DD.
STAGED/root74128B9917525D4D980008136A70AB2D00074580033DD7199A3596A6B85AA6D8
frozen nextround_644 NOTflashed: skiplegacy final_candidate stationary20us
shortcut ONLY next-edge recoveryprovisional acquisition; normalroundrobin
EdgeFilter confirmation same40tickmin/200gap,12interval/cycle/order/deadline.
Real finalfollowshortcut andinitialhandoff unchanged.45policytestsPASS incl
sequentialmux,release-s/thinLTO/auditwithE642features. Needownpreflights+one7%
recovery; expectRECOVERYCYCLECONFIRM final_visits0. No retry/guardchange.
HIGH_SPEED_RECOVERY E644 distinguishesunexercisedDMAchangefromupstreamrefusal.

E643 ACTUAL586A(nextscan_642)installed/OFF,Uartclosed. Own guard3/18,five3200
CPU2/6/9,ADC3PASS. One7%30s recovery FAILED follow4/stage5,POWERPATH4
FeedbackStale at209us. Checkpoints1..4 reached withoutgaprefusal;baselineage893us
leaves107usfreshnessslack; DMAseen0/max0/queue0/scans0. Priorstep4/mean999/12ints,
nofollowedge/arm. Initial4026COM/4025events,raw287/bus11044,injectedfault8;
CRC/firstarchive/finaloffPASS. Capture nextscan_643_70_30s SHA
DC15622C070B639C77341909B1889E82DBE35B70F2DA06580964DD9FCAF968BD.
STAGED/root38812A23A3103BC0CB0AE08B3BC9C0D308421A09F6FAFC09C6B0413BCD1F4407
frozen nextdma_643 NOTflashed: service realDMA immediatelyafterguardinstall
BEFOREcheckpoint4/finalwait, no timestamprefresh/limit change.43guardtestsPASS
incl893usbaseline regression (firstframe108usrefused,80usrealacq50accepted).
Release-s/thinLTO/auditwithE642runtime-onlyfeatures. Nextcandidateown disabled
preflights thenfixed7%recovery; no cohorttransfer. HIGH_SPEED_RECOVERY E643.

E642 STAGED/root586A2B6A9B6946AFA53331CFCEB4178DF801AE08E46702CC94A0674AA260B5E4
frozen nextscan_642 NOTflashed; actualBA3B lastoffE641. On ready inoriginalscan,
snapshot12intervals then boundedcontinue_filters visitsnext3phases before
promotion/cleanup; samefilters/10ussettle, stopsfirstrealqualifiededge and
buffersphase/level/onset/confirm. Pendingedge fedtoNextEdge afterpromotion;
neverdiscarded/refreshed. Stage6 identifiesearlycontinuation,hostallows0..6.
132simloopcases+helperwrap/onset/gap/driver/refeedchecks+3provenancePASS.
Originalfeaturesoverflow480B; new bench-recovery-runtime-only removesoptional
disabled recoverpwmcheck harness/entry only, retains prepare_recovery_inner/
stage/revalidation/guards. Command explicitlysaysnotlinked. Release-s/thinLTO/
auditfitswithnewfeature. NoUART/flash/motor/fresh offreadback. Snapshotbefore
continuation stillcoststime; improvementNOTmeasured. Newbuild needsown
disabledguard/full3200/ADC beforeonefixed7%recovery, notqualificationtransfer.
HIGH_SPEED_RECOVERY E642 exactfeatures/state.

E641 CURRENT/rootBA3B6DD90E4B1DF723D025D961494A526697C5CB77A13297E24A980C2BFBC1F0
installed/OFF,Uartclosed,frozen nextearly_641. Inplacequalification+firstsweep
moved BEFORE compcleanup/preparedchecks/reportpublish,original12intervalsnapshot
retained. FOLLOWSTAGE1finalize/2returned/3coldcore/4guard/5wait; strict host
optionalstage validation preservesoldarchives.132simcases+3provenancePASS.
Initial linkoverflow32B fixedbyshorteningnewpostrunlabelonly;release-s/thinLTO/
audit. Own disabledguard3/18,five3200 CPU2/6/9,ADC3PASS. One7%30s recovery
FAILED follow5 gap248ticks124us atstage1;priorstep5/mean999/12ints,REENTRY5,
no followedge/arm. Initial4034COM/4033events,raw340/bus10972,injectedfault8;
CRC/firstarchive/finaloffPASS. Capture nextearly_641_70_30s SHA868790334442C5F5974583BC7B18FBA4473EA13726B524490053A5E25190CE26.
Cleanup/reportpublication NOTnecessarycause; stage1 runsbeforeboth. Different
priorsector means124vs130notpairedcostmeasurement. Needcontinuephase sampling
insideoriginalscan throughqualification,notexit+snapshot+promote+restartolder
phases. No retry/thresholdchange/recoverypass. HIGH_SPEED_RECOVERY E641.

E640 CURRENT/rootCC9BE199B080B5080E2DE690B8AE62AFCE8B15C2933655C6D2509E89320F08CC
installed/OFF,Uartclosed,frozen nextinplace_640. NextEdge unqualified owner
allocatedbeforefirstsample; acquiring_mut onlyprequalification; qualify promotes
inplace, keepshistory/deadline, premature/repeatedcalls latch. Liveacquire writes
finalstaticcontextdirectly; no post-edge fullhistory/filtercopy.45policytests+
132simloopcases+3provenancePASS,release-s/thinLTO/audit. Own disabledguard3/18,
five3200 CPU2/6/9,ADC3PASS. One7%30s recovery FAILED FIRSTcheckpoint again:
follow5gap260ticks130us,priorstep3/mean1001/12intervals,REENTRY5/resumetime0/
stagedused0; nofollowedge/arm. Initial4021COM/4020events,raw333/bus11008,
injectedfault8,CRC/firstarchive/finaloffPASS. Capture nextinplace_640_70_30s
SHA3570C46E9F732EA4178F9E0A6BA53AAD9E8502C48EECB04ED73BD6BFE2726EFA.
DifferentpriorsectorfromE639 so20usdelta NOT measuredcopycost. Coldreport/
cleanup stillbeforefirstcheckpoint; move sampling beforehousekeeping, notmore
latecheckpoint microoptimizations. No retry/thresholdchange/recoverypass.
HIGH_SPEED_RECOVERY E640 details.

E639 CURRENT/rootAD3B04510697E5E3C81FADB2B28BE23B3200A51CE14E290B4A46BA6AF3A1E4EB
installed/OFF,Uartclosed,frozen nextsetup_639. setup_follow checkpoints after
acquire return,aftercoldcore,afterguardstart; samefilters, oldestphasefirst;
qualified follow cachedonce withrealonset. ISRcancel onlyrevokesAtomicBool,
nevermutatesforeground-borrowedOption; finaltakeconsumesvalidity.132actual-loop
simcasesPASS includingpreownercheckpoint/ISRcancel/latecachededgearmrefusal;
35adapter+3provenancePASS. Release-s/thinLTO/audit;own disabledguard3/18,
five3200 CPU2/6/9,ADC3PASS. One7%30s recovery FAILED FIRSTcheckpoint (REENTRY5,
resumetime0,stagedused0),follow5 gap300ticks=150us,priorstep2/mean1003/12ints.
No controllerstartup/followedge/recoveryarm. Initial4029COM/4028events,raw281/
bus11020,injectedfault8. CRC/firstarchive/finaloffPASS;capture nextsetup_639_70_30s
SHA1D40C38B907D376C945771E50E4599553B12C6B2047E254DEE61F08075194A1C.
No retry/thresholdchange. Gap ALREADY exists while finalizing/returning acquisition
context, before controller setup;oldestphase differsfromE638 so150vs114not
pairedregressionproof. Next continuity mustspan acquisitionfinalization itself,
preferpreallocated/inplace context before sensing, not morelatercheckpoints.
HIGH_SPEED_RECOVERY E639 scope; recoveryunqualified.

E638 CURRENT/rootF6545DEE436A96A6A5DD09B84AEF466CCCAB172BEACB9FCEA45FF5718624323F
installed/OFF,Uartclosed,frozen nextorder_638. Single orderingchange: retained
COMP sweep precedes first DMA startup/drain; successfuledge still completes
feedback service before return, actualonset/armage retained.96simulatedloopcases
+3hostprovenance testsPASS;release-s/thinLTO/mathaudit. Own disabledguard3/18,
five3200 CPU2/6/9,ADCroute3PASS. One7%30s recovery FAILED follow5 gap228ticks=
114us>100us, priorstep2/seed996/12intervals. E637130us samepriorsector;16us
difference is NOT isolated DMAWCET proof. No followedge/recoveryarm, initial
4032COM/4031events injectedtracking8,raw322/bus10853,CRC/firstarchive/finaloff
PASS. Capture nextorder_638_70_30s SHA498C55EAE8F1B745AF47B0D4AA700E7F244BD6C6134FA5D10D06B03165B912B0.
No retry/thresholdchange. DMA-first ordering notnecessary for gap; coldsetup
already too long beforefirstsample. Next service retainedsampling during
controller setup (not more comparator/DMA ordering guesses or filterreset).
HIGH_SPEED_RECOVERY E638 details; currentrecovery remainsNOTqualified.

E637 CURRENT/rootCD48B6E581BADC44766F81339CD133F77FB962E128B86FD59D78E330BF8C754E
installed/OFF,Uartclosed,frozen nextlive_637. Added finalcritical-section owner/
ready/reason/output check before vector/timer rearm; normalized followguard
refusal stop8. Actualfollowloop with simulatedIO+realfilters/acquisition96cases
PASS(allsectors/wrap/cancel/gap/feedback/missing/wrongedge), NOT timingproof.
Release-s/thinLTO/mathaudit; disabledguard3/18,fullfive3200 CPU2/6/9,ADC3,
recoverPWM4/4max10us,stats3/3,IRQbudget5/5 PASS. One7%30s recovery FAILED
FOLLOWEDGE result5 maxgap260half-us=130us>100us. Qualified priorstep2/seed1007/
12intervals; no followedge or recoveryarm. Initial4028COM/4027events theninjected
fault8,raw323/bus10662; firstarchive+CRC/finaloff verified. Capture
nextlive_637_70_30s SHA CD8F3A72F01CFF267DB1059AE24F332FD7090CEA322BDDCAC3D9CFB2CC10D326.
No retry/thresholdchange. Need preserve sampling DURING setup, not retainold
filters unserviced or reset them after setup. Hardwaregap not rotorlimit; no
recoveryqualification transfers. HIGH_SPEED_RECOVERY E637 details.

E636 STAGED/root47B22F2C2EC540B81520AC748B9536810D59BCA03D95AB50503105031B6F6AFD
frozen nextlive_636 NOTflashed; actual2DB8 lastoffE633. Opt-in
bench-reentry-next-edge-live now carries qualified acquisition+originalfilters
through setup, starts sole DMA feedback, waits real next edge, transitions guard
sector once and arms against real onset. Context cancellation is interrupt-safe;
no syntheticaccept or refreshed deadlines. FOLLOWEDGE postrun evidence + explicit
fixture --reentry-next-edge validates order/onset/confirmation/priorsector and
refuses non-success. 43acquisition/42guard/35adapterRust testsPASS,3provenance
Python testsPASS. Release-s/thinLTO/math audit frozen. Full oldfeatures overflow
2272B; omitted optional bench-current-epoch to fit (not guards/rawADC). NoUART,
flash/motor or fresh offreadback. NOT physical/timing qualified. Setup may
violate retained100us phase-sampling gap: must measure/refuse, never resetfilters
to hide it. Pending live-loop/cancellation/arm-owner review and own disabled
preflights before powered baseline. HIGH_SPEED_RECOVERY E636 exact scope.

E635 STAGED/root2ADD042BB8103EF2B3AD76EAF45AA4820250F268AAB3D7CD7D2303423CE38B0E
frozen nextguard_635 NOTflashed; actual2DB8 lastoffE633. Added cfg next-edge
guard expected-sector transition ONCE beforeacceptedprogress; polls existing
clock and retains feedback/tracking/deadlines, no timestamprefresh. Adapter
requiresactivehealthyowner,physicaloutputsoff,COMMITS0,guardpresent; actual
nFAULT/hostabort passed, allrefusals trip. 42RustpolicytestsPASS inclwrap,
deadline/stale/tracking/secondfollow andstagedpoisonreset; actualadapterbody
withrealpolicy+mockhardware35testsPASS incl10wrappercases. Release-s/thinLTO
next-edge featurebuild fits/mathauditfrozen. Functions still UNUSED by live
acquisition/controller, not timing/physicalqualification. NoUART/motor/flash.
Next carry qualifiedNextEdge+continuousfilters through fullsetup, switch
feedback to sole DMA owner before waiting (never manualADC+DMA concurrently),
then realfollowedge/guardsector transition/corestep/arm. First-event age still
starts at originalguardstart; cannotreset tracking clock tohide setup/wait.
HIGH_SPEED_RECOVERY E635 contains scope andintegration obligations.

E634 SOURCE-only next-edge protocol; no build/flash/UART/motor. Actual/rootELF
2DB8 lastverifiedOFF E633; source differs under new bench-reentry-next-edge.
Acquisition::into_next_edge consumes genuinely qualified acquisition, retains
cycle/order history+originaldeadline, clears cachedready only in private owner.
NextEdge acceptsONE further qualified edge, unchanged40..240tick confirmation,
actualonset+followinterval, original12intervalmean explicitlyprovisional. Latches
wrongorder/fastcycle/late/missing/confirmation/consumed errors. poll_filtered
retains realcandidate-only grace; no blinddeadlineextension.43RusttestsPASS
against actual hostminzcore (nonfatal incrementalAccessDenied note). No live
adapter or highspeedqualification; caller must retain continuous EdgeFilters,
not re-prime a post-edge level as an edge. Guard/controller integration pending.
Read entire GRAYBEARD_HIGH_SPEED.md; response saved GRAYBEARD_HIGH_SPEED_RESPONSE:
poweredDMA201us not128, maxima notmeans, referenceCOMdisablesinterrupt notfree
running, ci333 wait83HALF-us=41.5us not83us, currentgainuncalibrated/outliercause
unproved (quotedpeaks imply~31.5–33.1% not24% ifnaivelyfit). nFAULTPB14notPA5;
130mA anchor8.5%notAM3230%;8%3/3historical32B1notcurrent. No thresholds changed.

E633 CURRENT/root2DB80FEFE2D0CD63308379FC083885373236FF38F5150818407F3154FB5E68B2
installed/OFF,Uartclosed,frozen recoveryguard_633. Opt-in bench-reentry-guard-stage
constructs unhealthy guard sentinel during existing pre-sensing stats staging;
fresh unchanged admission then updates clocks/sector/limits without clearing
cold cycle storage. Consumes existing one-shot staging flag; stop revokes it;
missing/used/live slot refused; ordinary startup install unchanged.40Rustpolicy
testsPASS incl poisoned-state equivalence,wrap/age/limits;4Pythonmode/provenance
testsPASS. Actual disabled statscheck3/3 now includes dormant/stale/freshinstall
withouttimer/gates;guard3/18/fullfive3200 CPU2/6/9/ADC3PASS. One7%30s recovery
PASS338.015Hz/56768COM/56767acc,seed1010/age81us/remaining45.5us/arm7us,
originaldeadline163usspare,raw320/bus10686,DMA41/queue2,IRQacq24us/overruns0.
NO timinggain vsE63280us; no highspeed/reliabilitycohort claim. No threshold/
live-recovery/injection changes. HIGH_SPEED_RECOVERY E633 exactcapture/hash.
Next complete setup against qualified provisional seed THEN arm on subsequent
fresh observed edge; current20usconfirmation+32usfloor cannotfit1kHz41.5us.
Do not continue nine-store relocation/micro-optimization as the main strategy.

E632 CURRENT/rootE402FE6B706173808DEEBD75D2ECAAB32DDD779F158CD4FA1DD9C47A4B37334E
installed/OFF,Uartclosed,frozen recoverystage_632. Opt-in bench-reentry-pwm-stage
preloads actual-duty CCRs before recovery sensing; successful acquisition retains
only revalidated prepared-off state, rechecked before guard admission. Global
safing revokes compare token+tags+CCRs; failure remains full shutdown. Normal
startup unchanged; recovery duty still<=100,seed400/arm32us/deadlines unchanged.
CORRECTION toE631 audit: outputs_disabled checksMOE+physicalgatepins,NOTzeroCCRs;
zeroCCRrequirement was prepare_carrier/oldperiodonlyhelper. No guard weakened.
Disabled4/4 checks include wrongduty,tamperedCCR2,cleanup revocation;max9us
preparation only. Guard3/18,fullfive3200 CPU2/6/9,ADC3PASS. Snapshot wrapper
3tests each baseline/stage buildsPASS;provenance missing/duplicate/offrefusalsPASS.
One7%30s recoveryPASS338.246Hz/56807COM/56806acc,seed995/age80us/remaining44.5us/
arm7us,deadline156usspare,raw306/bus10984,DMA41/queue2,IRQacq24us/overruns0.
No latencygain or highspeedrecoveryclaim; comparepreload now survives sensing,
but fullguard/controller staging remains. No8%retry or threshold change.
HIGH_SPEED_RECOVERY E632 exactprovenance. Next stage guard/controller before
finalfresh edge; do not spend more turns optimizing this compare helper.

E631 CURRENT/root0D64952053325867D49C483063FF36F02CCC595A7B9B48A8D2521FA42BB736F5
installed/OFF,Uartclosed,frozen recoverypwm_631. E630 actual-duty snapshot now
tested on board: one8%/30s recoveryPASS387.704Hz,65116COM/65115accepted,
fresh12intervals/7cycles,seed872/age77us/remaining32us/arm6us,originaldeadline
214usspare,raw358/bus10925,DMA41us/queue2. Not current3/3 or high-duty recovery.
New bench-recovery-duty-check idle-only helper loads equal CCRs without MOE/
ENABLE:4/10/22/30% checks4/4,max5us,duplicate refused,cleanup revokes metadata.
NOT integrated into normal recovery; success cleanup still revokes preparation.
23 actual-module host testsPASS plus10 related regression tests; initial harness
missing sixstep dependency fixed,not firmware fault. Own disabled guard3/18,
fullfive3200 CPU2/6/9,ADCroute3PASS; release-s/thinLTO audited. No threshold,
injection schedule,live-reentry authorization changes. HIGH_SPEED_RECOVERY E631
details/provenance. Next stage complete guard/controller/CCR state before fresh
edge with explicit prepared-off lifecycle; do not repeat old9-store/no-gain
experiment or raise seed limit alone. Full goal remains unfinished.

E630 STAGED/root71BCD7C67134129E8E44907F9ECFBD009C55A8BB1303B7763973E1FEB6F07052
frozenrecoveryduty_630,NOTflashed; actual911A lastoffE629,Uartclosed. resume_once
no longer takesoriginalduty argument: snapshotsstopped POWER_DUTY before
observe_begin clearsit; unsupportedoutside40..100 returnsREENTRYresult8
beforeprepare/wake,neverclamps. Liveupdatesinretry campaigns STILLdisabled.
3actualwrapperhosttestsPASS all61validduties/resetpreservation/originalbudget/
invalidandabortrefusals;38guardpolicyPASS. Fullbaselinebuildoverflow256B,
no-baseline/current-epochvariant release-s/thinLTO fits. No fast-coast feature
inthiscandidate;sourcefeature retained. Ownpreflight/timing/benchstillneeded.
No firmwareinstalled/UART/motor thisentry; no oldqualificationtransfer.
HIGH_SPEED_RECOVERY E630 explains nextcoherenthighdutyPWMpreparation+short
handoff: acquisitioncleanupcurrentlyrevokespreparation, so don'tcacheauthority
acrossit ormerelyraiseMAX. Goalunfinished; nosafety/injectionlimit changes.

E629 actual911A/OFF,Uartclosed;root6A2E stilldiffers. Onecurrent8%/30srecovery
PASS:12freshintervals/7cycles,seed871,age77us,remaining32usEXACTfloor,arm6us;
resumed27.992068s at388.161Hz,65193COM/65192acc,raw338/bus10817. Original
deadline241usspare,CRC/timeline/finaloffverified. Singlepassnot3/3transfer.
No firmware/flash/guard changes. HIGH_SPEED_RECOVERY.md audits4constraints:
seed>=834/cycle5000/individual476,20usconfirmation+32usarm>41.5uswindowat1k,
live updatesrefusedinreentry (resumeusesoriginalduty),fixed2sinjection before
highduty ramp. Extractedactualreferencewaitmath3testsPASS. Not hardwarelimit;
need stagedshort finaledge transaction orqualifiedunpoweredtracking,notseed
thresholdonly orfake timestamps. AuditE629 capturehash;nextimplementrecovery
architecture whilepreservingactualduty/originaldeadline/late-refusals. No more
coastinstrumentation oridentical8%cohort justtoavoidthehighspeedwork.

E628 Actual911A restored/OFF,guard3/18PASS,Uartclosed. ROOT6A2E fastcoast
diagnostic differs. 6A2E own disabled fullfive3200/guard/ADCphase/IRQbudgetPASS;
70/10sPASS336.643Hz. Fastcoast500rows measured128..129us spacing,earlyCOMP
scanmax82us,nominal8k actual~7.8k. Then22%-capped30s attemptACK220 butFAILED
Current5 at7.026695s,22836COM/22835acc,NOTcompleted campaign. Bothfastcoasts
cadence/CRC/offvalid. SWD faultscanlogicalA3299/B2079/C1679,limit3248,buffer
count34959,queueempty; guardfeedback205usold,notstale. This differsfromE625
two-phaseexcursion; do notmergefaultsignatures orclaimcause.
Newoffline samepolarity3cycle span bounds on3phases:917.151..1112.347Hz
earlycoast,conditional30us timestampallowance/validtransitions/no missededges.
No controller-rate input/extrapolation; corroborates~1k rotor motion,notwhole
runlock/qZC/shutdownspeed/WCET. 70 first32rows insufficient2periods retained.
5coastperiod+2fastcoasttestsPASS incl faultnotclassifiedcompleted. AuditE628
hashes/table/reproduction. No30attempt,22notqualifiedrepeatable;20%911A3/3
retained. Next high-duty recovery budget /current-timing mechanism, notmore
coast instrumentation. BEMF_PARITY_PLAN updatedconditionalcorroboration.

E627 STAGED root6A2E46DFC7E6BE47A5D1F6025F1C346AB5AAF8EE080244BF3120D6E33AB968B5
frozen fastcoast_627,NOTflashed. Actual911A lastoffE626/Uartclosed. Opt-in
bench-fast-coast:500bridge-off scans atnominal8kHz,10us TIM17 mux settle
(samebudgetasflying acquisition),4ADC channels+3COMP retained,first32COMP
completionbrackets andallCTIME retained. Eachscan assertsENlow/outputsdisabled;
no powered ISR sensing work. Plaincomp_read retainslegacy3000delay.
Generic helper/fullfeature buildoverflow448B; sharednoinlineboolhelper384B.
Diagnosticbuild omitsoptionalbench-current-baseline,retainsbench-current-epoch
andALLrun guards; release-s/thinLTO/codegen1fits. Not samequalificationbuild.
--fast-coast fixture opt-in, decoder checks500timestamps/gaps125..150us and
earlyscan<125us;2new+3coastperiodtestsPASS,NOTmeasuredcadence/settling proof.
Mathaudit retained. No UART/flash/motor E627. Next own disabledpreflights+
boundedbaseline to measure8k scan timing,then22%coast corroboration; do not
transfer911Acohort or claimindependentlock yet. AuditE627 detailedscope.

E626 same911A/OFF,Uartclosed;NO firmware/flash/guard changes. Two predeclared
additional20%-capped30sramps bothPASS =>3/3withE625; finaltail923.755..926.586Hz,
~150kCOM/acc each,raw760/762/815,bus10913/10829/10877,DMAmax52/38/47us,
IRQunion~72.77..72.90%. Unaggregatedvalidated tails1/1/0, allCRC/offverified.
Thenone22%-capped30scampaignPASS160883COM/acc,finaltail1002.236Hz,sigma9.349us
(26overlappingcycles,NOTwhole-runquality),raw838/bus10889,DMA38us/queue3,
IRQunion74.965%,1unaggregatedtail. Not30satfinalduty,not22cohort/recovery.
E62523%Current faultstillretained/unresolved; oldIRQtail flaghas0I85rowsin
trace0 build, so no per-dispatch evidence to distinguish delay/rejections.
Coast data2kHz and~474us scan (COMP offsets190/332/474) at~1002Hz is
Nyquist/alias-limited; cannot claim independent speed/lock or extrapolate away.
Next suitable fast passive coast corroboration + high-duty recovery scheduling,
or targeted lateaccept investigation; no more20identicalcohorts. AuditE626
hashes/table; PORTABLE_WINS/BEMF_PARITY_PLAN current scope refreshed.

E625 CURRENT/root911A3198CA48F7FD28B1FC108D19B90E525034DB327869DD03E7CB0BC11EC2EE
installed/OFF,Uartclosed,frozen dmaguard_624. Own disabled guard/fullfive3200/
CPU2/6/9/ADCphase/IRQbudget PASS. Fixed70/10sPASS337.188Hz,20231COM/20230acc,
DMA38us/queue2,49751sums/publications,raw310,bus10304. Then ramp250 FAILED
Current5 at7.392150s afterACK230,before250;25017COM/25016acc,DMA38us/queue3,
rawsummary873 excludes rejected scan. SWD retained newest ring mapslogical
A3612/B2110/C434 (limits848..3248),bus1200/vref1508; count36777,queueempty,
guardfeedbackage288us/eventshealthy,notstale. Finalacceptedinterval453us after
~150..170us intervals; current/timing association,notproved rootcause.
Next cappedramp200/30sPASS:150162COM/acc,finaltail926.586Hz,sigma30.303us
(26overlappingcycles only),DMA52us/queue3,raw760,bus10913,IRQunion72.899%.
149254safetypublications/149253summed contiguousframes;1unaggregated tail
explicitlyreported,notfullcoverage. Alloff/CRC verified. No30attempt/recovery
qualification;20%one mixed-duty30s campaign,not3/3 or30sat20%.14sumdecoder
testsPASS, oldequalityretainedunlessguard_irq1. AuditE625 hashes/evidence.
Next20%repeatability + investigate453us lateaccept/currentevent; no stale
guard relaxation orautomaticcurrentthresholdincrease. Source=currentimage.

E624 STAGED ONLY root911A3198CA48F7FD28B1FC108D19B90E525034DB327869DD03E7CB0BC11EC2EE
frozen dmaguard_624,NOTflashed; actual544B lastverifiedOFF E623,UARTclosed.
bench-dma-guard moves complete coherent scan validation+age publication into
DMA handler, retaining immediate phase check, exact bus/VREF conversion and
original1ms guard. FIFO retains every frame; foreground sums only, never
replays an old timestamp into guard. Queue overflow still stops. Safety sample
count now DMA publications, sums count FIFO consumption; queued tail can differ.
35actual-publisher/policy Rust tests + explicitwiremode test PASS,38event100
guardtestsPASS. Release-s/thinLTO fits after shortening shellhelp only; initial
default-feature conflict and32Boverflow bothfailed/no flash. No-gain inline
attribute reverted. Audit confirms2uidiv now in DMA1_CHANNEL1 (0x800204a,
0x8002146); cost NOTyet measured. Need own disabled preflights, then bounded
baseline handler/guard/lease evidence before25%; do not transfer544B qualification.
--dma-guard explicitfixtureflag, DMAFEEDBACK guard_irq=1 marker. Existing
fixture full ADC/stat count assumptions need review beforequalification.
No UART/SWD/motor E624. AuditE624 records scope/nextchecks. Goalunfinished.

E623 CURRENT/root544BBBF004F74A2180CDC22864E26E18DC4E6A10AB365A2573355A03F7CD6800
installed/OFF,Uartclosed,frozenliveclock_623; release-s/thinLTO/codegen1.
Old68F8 live12 Tracking8 diagnosed post-stop SWD: events.fault=TooFast,
238us event minimum, NOT demonstrated desync. bench-event100 explicitly
changes event minimum to100; order/max1000/slowcycle6000/electrical/seed400
unchanged, fastcycle2223 report-only. 38 event100/report guard tests PASS.
Intermediate23C9 event100 stopped on live writer122us before ACK12. Retired.
544B times live writer INSIDE its critical section, excluding unrelated
unmasked ISR time;100us transaction limit unchanged. Writer tests PASS.
Own disabled preflights PASS. Live70->120/150/200 each completed10s campaigns,
final accepted-cycle tails ~585/712/932eHz; these include startup/acceleration,
NOT10s fixed-duty qualifications or recovery cohorts. rawpeaks500/682/1139,
busmins10877/10889/10865mV; no calibrated amps claim.
ACK-paced70->250 ramp FAILED FeedbackStale4 at7.715279s after ACK250;
27190COM/acc,raw877,bus10674,queuepeak4/nooverflow. No30attempt.
Post-stop SWD: guard now7715253,last_feedback7714223 (1030us); newest queued
ADC acquired7715027 (226us old), event age41us/faultNone. Fresh ADC exists;
foreground delivery lag is proven, specific execution cause still unresolved.
Do NOT widen1ms or refresh timestamps/discard frames to hide this. Next audit
foreground service/interrupt load, preserve actual electrical checks.
UART live TX source is single nonblocking byte per pass, not a baud-length
busy wait. GDB postmortem resumed already-off target then shutdown OpenOCD;
GDB exit1 is expected remote-close AFTER valid prints; OpenOCD exit0.
AuditE623 contains builds/captures. Continue campaign; no new approval blocker.

E622 OPERATOR approved fast-cycle report-only, then clarified do NOT invent
approvals: pursue30% with sane measured tradeoffs, meaningfulsteps, retainreal
electrical/tracking protection. PriorE604policyblockerRESOLVED. Do not re-ask.
CURRENT/root68F80A2FFC3BD28B5D7D03156D45A1EEFB3BEAA02675E10D55FE5F74738A5CCC
installed/OFF,Uartclosed,frozenfastreport_622. Release-s/thinLTO/codegen1,
reportFASTCYCLEr1 threshold2223/count/minus;slow6000/event238..1000/electrical/
seed400/arm32/16/deadlines unchanged.37report+37defaultguardtestsPASS,
ownpreflightsPASS. Fixed90/10sPASS438.238Hz;live70->120ACK thenTracking8
stop3.409s,NOTcurrent/bus,finalacceptedtail~570Hz,not12sustainedpass.
Live70->100completed10s;fixed100/30sPASS481.428Hz,86655COM/86654acc,
sigma34.84us,raw512/bus10925,IRQunion54.622%,86518fastreports,min1983us.
Allraw/offretained;no recoveryqualificationonnewimage. AuditE622hashes.
Fixedbemfdu pathstillmax100;use generalizedlive --step-duty up to300.
--fast-cycle-report requiredfixtureflag;parserdecline120fixed issuedNOUART.
Nexttrackingrefusalmechanismat12%,notwiring/anotherapproval/smallfloorratchet.

E621 actual32B1 restored/OFF,Uartclosed;root4303A151A0BA2DCFFEACCD7291766835EAB9DCD80B6586BC4521EA5A063FB6F8
opt-z baseline differs. Matchedoldcaller4303 vsE0D1 bothownpreflightsPASS,
one70/30srecoveryeachFAILEDfresharm:edgeage97usboth,remaining28.5/26<32.
No timinggain;retiredE620caller specialization,sourceOption restored.
AuditE621 exactcaptures/hashes;bothCRC/firstarchive/finaloffPASS,not30spasses.
Globalopt-z worseobservedthan32B1s76us. No repeat/thresholdwaiver.
Restoreguard3/18/offPASS;E618feature-gatedmetrologyretained,defaultopt-s.

E620 ROOT E0D10419D4749CE58EC1A5302EEA2631632E96CC99A47DF07DC58231C8A99DBC
stagedNOTinstalled;actual32B1 lastoffE619. coast_run_inner constREENTRY+
Limits replacesOption:onlyresume_once true,4othersfalse/unusedzeroLimits.
Noage/deadline/guardchange. Release-s overflow2528;opt-z/thinLTO/codegen1fits,
2emittedcoastbodies0x88c/0x878,frozencoastreentry_620+audit.36guardtestsPASS
notcallerexecution. NoUART/motor. AuditE620: needownpreflights andopt-z
baselineforspeedattribution;E619DMAopt-znotmatchedcontrol. Notiminggainclaimed.

E619 actual32B1 restored/OFF,Uartclosed;root213D opt-z diagnostic differs.
213D ownbaseline4cases/fullfive3200CPU2/7/9/guard3/18/ADCphase3/IRQbudget5PASS.
One70/10s holdPASS338.959Hz,20337COM/acc,raw298,bus10734,sigma42.980us.
Same-DMAbaseline stillnegative residualsum-2.178904counts/49751scans;NOTamps.
CURRENT_MEASUREMENT_PLAN E619 hashes: sequence/cadence match insufficient,
no same-runPSUreading or driftcauseclaim. No repeatzero/settlinggrind.
--prestart-dma explicitfixtureselection added;realcapture regression.
Restoredfrozen32B1 download/resetexit0,guard/offPASS. No recoverytransfer.

E618 ROOT213D27559DF04C863158D6835E5E89AE45CD7CB8533BD53F0CC595932F7FF94C
stagedNOTflashed;actual32B1 lastoffE616. bench-prestart-dma usesexisting
start/poll/stop128scans predrive,owner/output/epoch/abort/50ms preserved,
no offsets or DMAIRQunmask. BASEACQ provenance decoder+cadencemismatchcheck;
5residual+5baselinePythonPASS. Release-s overflow416;per-commandopt-z/
thinLTO/codegen1 fits,emittedauditdone. DefaultCargo stayss. NoUART/motor.
CURRENT_MEASUREMENT_PLAN E618: nextdisabledabort/cleanup/epoch+preflights,
fixturemarkerrequired beforepower;opt-z timingqualificationnottransferred.

E617 offlinecurrentaudit: operator130mA belongsE60285/30s3A70,notidle.
Capturehash/CRC/fullmotor+baseline/sums rechecked:149254scans,residualsum
-5.430051counts,NOTamps or ADCcorroboration. Regression retainsuncalibrated.
CURRENT_MEASUREMENT_PLAN E617: SWbaseline vs DMAsequence confound already
known;E287/288mode/settling comparisons gave no stablecorrection,do notrepeat.
Nextmetrologystep same-DMA predrivebaseline withboundedownership,notfitted
offset. No hardware/build/guards;actual32B1 lastoffE616,rootretired6A10.

E616 actual32B1 restored/OFF.6A10 successor candidate ownpreflightsPASS,
one75/30s recoveryPASS361.735Hz butedgeage76us unchanged;retired,no retry.
Source flying_acquire reverted E614 helper only;constREENTRY retained.
Rootstillretired6A10,notinstalled. Restorefrozen32B1 download/resetexit0,
guard3/18/offPASS,Uartclosed. AuditE616 hash. No newgain or higher recovery
qualification; stop successor microvariants. Speedpolicydecision pending.

E615 offline archiveassessment refreshed inBEMF_PARITY_PLAN:128files/frozen
hashes +3historicalsummaries revalidated,5testsPASS;3current32B1 recovery
captures replayPASS withhashes/original30s/profilechecks. No matched387Hz
reference or comparablewindow-vs-cycle sigma/current claim. Current130mA is
operator3A70/8.5 observation,not32B1 phasecalibration. No hardware/sourcechange;
actual32B1 lastoffE613,root6A10 staged. Fullgoalopen,policydecisionpending.

E614 staged/root6A10FBC5873CAD10BC0143DEB79782FADF2A158E0908106017F043CF4209A3D0
NOTinstalled;actual32B1 lastoffE613. flying_acquire private-validsector
successor replaces3mod6 withconsthelper+all6compiletimeasserts.31finaledge/
30seedpolicytestsPASS,release-s/thinLTO/audit,frozenacquiresuccessor_614.
IMPORTANT oldu8mod6 alreadymul/shift (notsoftdivide); helpercountunchanged.
Edge size500->496,stack36->28; acquire_inner2712->2728. Untimed small
candidate,not assumedmicrosecondgain. Nohardwarecommands/guardchanges.

E613 CURRENT32B1/OFF: two predeclared additional80/30s recoveriesPASS=>3/3
withE612,no exclusions.387.193..387.512eHz,sigma20.518..21.415us,
IRQ52.713..53.762%;all freshage76us/remaining33us/arm7us. Only1us margin,
notWCET. Run2raw397/bus10960,run3raw379/bus10805. CRC/ADC/timeline/offPASS,
Uartclosed;no code/flash/guardchange. AuditE613 cohort/hashes. Stopidentical
80repeats. Higherrecoveryneedsmorelatency+seedrange review; speedpolicy pending.

E612 CURRENT32B1/OFF:75/30s recoveryPASS361.814Hz,margin40us/age76;
thenONE80/30s recoveryPASS387.512Hz,65084COM/65083acc,resumed27.992132s,
sigma21.415us,raw389,bus10793,seed871,age76us,remaining33us/arm7us.
Only1usspare above32floor,notrepeatability. CRC/ADC/timeline/finaloffPASS,
Uartclosed. No thresholds/code/flashchanged. AuditE612 hashes. Nextbounded
repeatability at80,not85recovery or seedlimit/armwaiver. Speedpolicy pending.

E611 CURRENT/root32B1 installed/OFF. Own guard3/18/fullfive3200CPU2/6/9,
ADCphase3/IRQbudget5PASS. One70/30s recoveryPASS339.344Hz,56991COM/acc,
raw342,bus10757,sigma22.710us. SEEDLAT90/106/118/138/142;edgeage76us,
guardbracket10us vs prior13..14,remaining49us/arm7. Observedgain notWCET/
pairedproof. CRC/ADC/timeline/finaloffPASS,Uartclosed. Frozenconstreentry_610.
Next75recovery tocheck retainedtiming before80; no transferredcohort or guard
policychange. AuditE611 hash/capture; source/root nowmatchinstalled.

E610 staged/root32B1188090EA7D70EE83B9CB4EBECBE2C4C63EB863B8B5D38FD8E2EF810475DE
NOTflashed;actual3A70 lastoffE608. start_inner constREENTRY replaces runtime
Option<Limits>;3privatecallers explicitfalse/false,true/false,true/true.
Originalresume limits/ages/checks retained. Release-s/thinLTO/audit,frozen
constreentry_610.36guardtests+2source/algebra testsPASS(notPACexecution).
No standalonepowered_timer::start_inner symbol innewaudit (inlining/layout
changed),NOTtiminggain proof. Needs disabledpreflights then70recoverytiming
before higherduty. NoUART/motor. Report-only speedpolicy stillpending.

E609 sourceaudit: outputs_disabled already grouped MOE/GPIOA/GPIOB reads;
do not propose grouping sixpins again. Guardstart consumes one-shot stage/
adoption,checksownership/output/feedback/originallimits theninstalls/enables.
Moving earlier requires inertprepare/finaladmit lifecycle,notcheckremoval.
No code/build/hardwarechange; actual3A70 lastoffE608,root2F35 diagnostic.
AuditE609; no measured new gain. Fast-cycle policy choice stillpending.

E608 actual3A70 restored/OFF,root2F35 diagnostic differs. Existing acquire-
timing feature only added; ownpreflightsPASS; one70/30s recoveryPASS338.016Hz.
ACQUIRELAT brackets31/6/3/2/5us(edgequal/clear/publish/return/entry),then
SEEDLAT9/5/14/2/5us toarm;total82us includesobserver,notremovableWCET.
Reportpublication3us not10us target. Retire diagnostic fromcampaign; frozen
acquiretiming_608/hash auditE608. Restore3A70 download/reset/guard3/18/offPASS,
UARTclosed. No source/guardchange. Next larger setup scheduling,notreport-only
optimization as assumed10uswin. PolicydecisionE604 stillpending.

E607 offline recovery budget3testsPASS:80us observed edgeage requires4us
saving at387eHz,10us at410,19.5us at450 (constant-speed planning only).
410+ also independently outside seed400; changing runfloor cannot fix either.
Script drv_recovery_budget.py,no admissionauthority/WCETclaim. Actual3A70
lastoffE606; nofirmware/hardwarechange. Target acquisition-exit/report/setup
scheduling,not repeating failed9storeprep. E604 report-only decision pending.

E606 current3A70/OFF: one75/30s injectedloss/recoveryPASS361.159eHz,
60656COM/60656acc,resumed27.991436s,sigma22.413us,raw317,bus10662,
IRQ52.099%. Freshseed924,age80us,remaining35.5us,arm7us;32usfloor unchanged.
CRC/ADC/timeline/finaloffPASS,Uartclosed. First current recovery,not3/3.
No firmware/guardchange; E604 report-only choice stillpending. Higher recovery
blocked by post-edge freshness budget,not running speed profile. AuditE606.

E605 host-only live_armed_baseline now accepts --cycle450 with exact profile
validation; conflicting cycle flags rejected before output-file/UART access.
2profile+2live-line+1writerPython/4RustPASS. No live trial, firmware/flash/
guardchange;3A70 lastoffE603. E604 report-only decision still unanswered;
automatic continuation is not approval. No new motor or repeated9 attempt.

E604 offline policy audit36RusttestsPASS: ordered278us (~600eHz) accepted
events pass independent Monitor but fail450cyclefloor. Tracking checks doNOT
replace speed veto or certify rotorlock. Proposal asks operator to make ONLY
fast-cycle floor report-only,retain slowcycle/event/electrical/deadline/duty/
seed/arm stops. Notimplemented; explicit decision pending,not blockedstatus.
Current3A70 lastoffE603, no hardwarecommands. AuditE604 rationale/counterexample.

E603 current3A70/OFF:90/30s attempt FAILED CycleTiming at201596us,
493COM/492acc,raw417,bus11223. Step4 guard2214<2223,reference2211,
prior2313/twocyclemean2262; controlleravg757ticks(~440eHz),wholeattempt
408.62includesacceleration. CRC/timeline/finaloffPASS,Uartclosed. No retry/
flash/guardchange. Experimental speed ceiling again,not hardware or losslock
proof. AuditE603 hash; current steady evidence80/85 only,samebuild recovery
untested. Stop floor-ratcheting; separate experimental envelope from safety.

E602 current3A70/OFF:80/30s heldPASS387.399eHz,69731COM/69730acc,
raw401,bus10913,IRQ52.375%,ADCphase149254,finaloff verified. Historical
CycleTiming failures remain; runningfloor now2223.85/30s PASS409.811eHz,
73765COM/73764acc,raw404,bus10841,finaloff/timelinePASS,Uartclosed.
Operator explicitly confirms130mA during85run (PSU reading,notADC calibration).
No recovery qualification at80/85. No further ceiling/duty change this entry.

E601 CURRENT/root3A701EEF4AFAAAD2654436B438649AE39E55CF592CCF3CA2E13C6A4532EE395A
cycle450 installed/OFF after75/10s PASS360.624eHz,21637COM/21636acc,
raw316,bus10960. Explicit runningfloor2500->2223 only; seed400/32us arm,
238..1000event/electrical/deadlines unchanged. Release-s/thinLTO/audit,frozen
cycle450_601;35Rustguardtests,24targetedPython plus2capture/profiletestsPASS.
Own guard3/18/fullfive3200CPU2/6/9/ADCphase3/IRQbudget5PASS. Initial host
summary lacked2223 whitelist: CLI failed postrun; corrected/replayed same
capture PASS (no motor retry). New --cycle450 mandatory. See auditE601.
This is deliberate experimental envelope expansion,NOT hardwarelimit claim
or retroactive E598/E600 pass. Historical7.5recovery not transferred.

E600 historical13B5 exacthash found in Cargo cache,frozen cycle400_13b5;
2329 frozen current_2329. Oldimage own disabledpreflightsPASS, one80/30s
FAILED CycleTiming3.136678s,385.507eHz,7254COM/7253acc,raw382,bus11032.
Step2 guard2493/reference2489.5us,prior2652.5; late1017ticks then697.
Same fault exists without E580+ edits, not recent-code-regression proof or
hardware blame. No retries/guardchanges. Restored2329 download/resetexit0,
guard3/18/offPASS,Uartclosed. AuditE600 exactartifacts. Do not bisect later
recovery optimizations as necessarycause; investigate common timing/envelope.

E599 offline: oldE577/currentE598 recorded PWM/duty/guard metadata match,
but final26overlapping cycles mean2672.115/2584.423us,sigma19.532/18.840;
IRQunion53.262/52.390%. Differentbuild/time,not controlledregression or CPU
slack proof. Two-cycle average candidate stays host-only; E598 regression
added,5testsPASS. AuditE599. NoUART/flash/firmware/guardchanges; lastoffE598.
Next isolate operating-rate difference,not averageguard deployment or retry
untilpass. Historical13B5 must be hash-verified before any referenceflash.

E598 same2329/OFF: one80/60s steady attempt FAILED CycleTiming at1.511794s,
3481COM/3480accepted,mean383.901eHz. Step4 guard2493<2500us; reference2489.5,
prior2597us,two-cyclemean2543.25. Not independent physical overspeed/loss-lock
proof. Raw346,bus11032,CRC/timeline/finaloff verified,Uartclosed. No retries,
code/flash/guard changes. AuditE598 capture/hash. Current8% sustained NOT
qualified; old13B5/E577 success not transferred. User permits Python or MCP:
choose by experiment, no requirement to rebuild for every operating point.

E597 CURRENT2329/OFF: two predeclared75/30s recoveriesPASS =>3/3 withE596,
359.551..360.711eHz,sigma23.180..23.484us; freshremaining35.5..37.5us,
arm6..7us,age80us. Run2busmin10160mV retained (notexcluded),raw376;
run3bus10913/raw325. CRC/timeline/finaloffPASS,Uartclosed,no code/flash.
AuditE597 exacttable/hashes; parityplan+portablewins refreshed. Stopidentical
75repeats. 8%recovery stillunqualified; no calibratedcurrent or fullgoalclaim.

E596 CURRENT/root23298ec18646acfa1638186df218249ea858128aba516751d56804d8328f3c0c
installed/OFF; final-edge-prepare excluded, livecontrol remains. release-s/
thinLTO/audit andownguard/fullfive3200/ADCphase/IRQbudgetPASS. One75/30s
recoveryPASS359.551eHz,60387COM/60386acc,sigma23.484us,raw344,bus10925,
IRQ51.697%. Seed937/age160ticks/remaining74=37us/arm7us; alloldguards,
CRC/timeline/finaloffPASS,Uartclosed. Onepassnotcohort. Nextfixed2additional
75recovery attempts (retainallfailures),notnewcode or8retry. AuditE596hash.

E595 actualE6EC/OFF: finalprepare70/30s recoveryPASS335.301eHz,56312COM/
56311accepted,raw315,bus10925,sigma23.833us;FINALPREPused1 verified. Age162
ticks81us vsprior80us:NOlatencygain. Recoverygap174ticks<200,previous166.
Ownfullfive3200CPU2/6/9/ADCphase3/IRQbudget5PASS. CRC/deadline/finaloffPASS,
UARTclosed. Coldstate-onlyexperiment RETIREDforenvelope; no8retry. Image
stillinstalled, notbaseline restoration. AuditE595capturehash. Preservefailed
gainverdict; don't spend anothercohort on these fewmovedstores.

E594 CURRENT/root e6ec96d42123abe5d5ca4f0af8a9240414a824d38dff838e378c11a0fa4340c6
livecontrol/finalprepare installed/OFF; own guard3/18PASS, NOTpoweredqualified.
DiagnosticF6BE installed thenfinalprepcheck31/31PASS ENABLElow; transcript
finalprep_594_check.txt. Injectedmetadata lifecycleonly; notawakeadmission.
Disassembledcoldreset onlyRAMstores/nohardwarewrites. Rebuiltrelease-s/
thinLTO/audited campaign,download/resetPASS.Uartclosed.No motor thisentry.
Nextfullfive3200/ADCphase/IRQbudget then70recovery --final-edge-prepare.

E593 ROOTf6be73bb6494912967700d6bc6fc0de289829e81f3ccfb34c68ddd6e19c3e3b7
diagnostic built/audited NOTflashed; actual0835lastoffE588. Addsfinalprepcheck
behindbench-final-edge-check; diagnosticomitslive-control tofit, notproduction
parity. Expected31cases:disabledrealprepare refusal,6sectors coldstate/consume
once/wrongsector/polling/stopcancel. Injectedmetadataonly,ENABLElow; notsuccessful
awakeprepare admission orpositiveownerhardwareproof. Liveuses extracted
take_final_step; wrong/missing/pollingconsumption revokesmarker. Host3testsPASS.
Needrun disabledharness+registereffectinspection thenproductionownpreflights.
NoUART/motor/flash thisentry. AuditE593 scope; rootnowdiagnostic notliveimage.

E592 host FINALPREP strictselection implemented: --final-edge-prepare required
for marker; exactunique used0/1/interval11/noauthority/checks1. Successful
recovery requiresused1,nonrecoveryused0; marker alone neverqualifies run.
3protocoltests+2retainedrefusaltestsPASS. No firmwarebuild/flash/UART/motor;
rootB78Bstaged,actual0835lastoffE588. Remainingdisabledadapter lifecycletests
and candidatepreflights beforepower; hostmarker tests are not hardwareproof.

E591 ROOTb78b7adee1461053e1d4988c6260e59351c2a62c7acadee11ebdf6626f414df9
built release-s/thinLTO/mathauditPASS NOTflashed; actual0835lastoffE588.
coldstate noinline reducedoverflow480->448; separated idle seedmaskcheck to
bench-seedmask-check (dependsmasked-seed-arm), live maskingUNCHANGED. Final
text129784/data1120/bss29348. Added poststop FINALPREP used0/1 provenance;
notyetstrictlyhostvalidated. Needdisabledadapter lifecycle tests beforepower.
NoUART/motor. See AuditE591; seedmaskcheck absentfromdefaultcampaign now,
explicitdiagnosticfeature required; notremovedfromrepository.

E590 staged bench-final-edge-prepare adapter, NOTbuilt/NOTflashed: release-s
linkFAIL480bytesFLASHoverflow. Actual0835lastoffE588; rootELFstale,DO NOTflash.
Recovery11intervalhint calls output-disabledcoldstate reset once; finalresume
consumes matchingsector marker, absent/wrongrefuses;live_stop revokes marker.
Observationclock/guard/COMPmux/timers/finalseed remain atoriginalhandoff.
31pureacquisitiontestsPASS only, NOTadapter/lifecyclehardwareproof. NoUART/
motor. Needfit+poststopprovenance+disabledcancel/repeat/mismatch checks before
poweredtest; noopt-zswitch or guardremoval. AuditE590 exactscope/debts.

E589 offline final-edge preparation hint: Acquisition::preparation_step returns
expectedsector onlyafter11intervals, noSeed/noauthority; edge12 remainsunchanged.
31actualpolicytestsPASS across6sectors/wrap/order/gaps/cycle/deadline andfilter
persistence/blackout. No livecaller/embeddedbuild/UART/motor. Actual0835lastoff
E588; newsourceonly. See RECOVERY_POST_EDGE_AUDIT E589 integrationcontract.
Prepare coldstate while continuingfilters; do NOTstartguardearly or mutate
scanCOMPmux/timers. Finaledge/livefeedback/budget/owner/32usarm checksremain.

E588 CURRENT0835cea626bcdca3f6e65b505f4c09b0cb8b8bdaf83ae6161f7011fa859ce0f0
installed/OFF. Exactphasepredicate848..3248 explicit3constchecks removesloop/
abs/tempstack;35guardtests inclall65536values eachchannelPASS. Release-s/
thinLTO/audit,disabledguard/fullfive3200/ADCphase/IRQbudgetPASS. One70/30s
recoveryPASS335.427eHz,56334COM/acc,sigma23.389us,raw321,bus10901.
Seed997/age160ticks/remaining89/arm6us; guard13us vs15,IRQ51.588vs52.115%.
ObservednotpairedWCET. No8retry;finaloff/Uartclosed. AuditE588 hashes.
Next consider prepare-before-final-fresh-edge handoff (measured new edge,
continuous qualification/order/freshfeedback/deadline intact), NOTtimestamp
refresh or32usfloor relaxation. Architecturecandidate notimplemented yet.

E587 CURRENTcde8f03d99250a89ed42d28463d67c98b69ed15965bb67c58d5d73ea39b65db5
installed/OFF. Disabledguard3/18/fullfive3200CPU2/6/9/ADCphase3/IRQbudget5PASS.
One70/30s recoveryPASS335.566eHz,56358COM/56358acc,sigma23.714us,raw335,
bus10901. Seed1008/age162ticks/remaining90/arm7us;guardbracket15us unchanged.
No measuredlatencygain; no8%retry. CRC/timeline/finaloffPASS,Uartclosed.
Hostrefusalclassifier now explicitly reports margin on E583/E585 archived
failures;2testsPASS, failures remain failures. AuditE587 exactcapture/hash.
Next substantivepostedgework,notmoreconstpreparedvariants or floorwaiver.

E586 ROOTcde8f03d99250a89ed42d28463d67c98b69ed15965bb67c58d5d73ea39b65db5
staged NOTflashed; actual3013lastoffE585. start_inner PREPARED nowconstbool;
all3callersliteral false/true/true, no livecheck/guard removed. release-s/
thinLTO/mathauditPASS, preparedsymbol0x22cbytes vsold0x248, stack44vs52.
35currentguard/admissiontestsPASS via new failfast runner. EarlierPSquoting
compilefailed and accidentallyranold33testexe: NOTcounted ascurrentproof.
NoUART/motor. Next disabledpreflights+7%recovery timing, notassumedgain.

E585 CURRENT3013338ca36c6e9ea50a1df7246582940755eab779d8bab63eb3e0513e0cec6f
installed/OFF. Disabledguard3/18,fullfive3200CPU2/6/9,ADCphase3,IRQbudget5PASS.
One70/30s recoveryPASS334.672eHz,56207COM/56206acc,sigma23.819us,raw329,
bus10698,seed1019/age162ticks/remaining93/arm7us. Observedage81us vs old84.
Thenone80/30s recoveryFAILEDfresharm: valid12seed886,age164ticks,remaining
58ticks=29us<32us;armed0. CRC/firstarchive/finaloffPASS,Uartclosed. No retries
or guardchanges. RECOVERY_POST_EDGE_AUDIT E585 exacthashes/captures. Small
observedgain, notenvelopeexpansion orWCET; remainingpath guardsetup15us.

E584 staged/root3013338ca36c6e9ea50a1df7246582940755eab779d8bab63eb3e0513e0cec6f
NOTflashed; actual937F lastoffE583. baseline age uses one TIM17 read after
snapshot instead of five; originaltimestamps/200ussetup/1000uslimit retained.
Pure helper2testsPASS all65536clockvalues atboundaryages/everyoldestchannel.
Release-s/thinLTO/math auditPASS; emitted6342singleclockload outside bounded
maxloop, constants800/200 resolved, no mathhelper in bracket. Not measured
latency gain; array stackstores remain. Need disabledpreflights and oldpoint
recovery measurement before8%. No UART/motor this entry; no armfloor change.

E583 CURRENT937F/OFF: one80/30s recovery FAILED fresh-arm margin after valid
12interval seed874ticks/cyclemin5222. Edgeage168ticks leaves51ticks=25.5us,
below unchanged64ticks/32us; CORESEEDarmed0/arm0, no second powered segment.
CRC/firstarchive/finaloff independently verified; CLI correctly fails but
reports generic missing fresh measured core arm. Capture seed400_583_recovery80_30s.txt
SHA256 AE6C70A21692B9128F2A2FF5E049B32C4FD3D1DC2517A8D5A0978916BAEC9747.
No retry/rebuild/guardchange. Next reduce measured post-edge delay, not32usfloor.

E582 CURRENT937F seed400 installed; disabled guard/IRQbudget/filter/atomic/
roles/CPU/archive/ADCphase preflights PASS. One70/30s recovery PASS330.080eHz,
55437COM/55436accepted, sigma24.337us, raw336,bus10877; freshseed1031,
age168ticks/remaining90ticks/arm6us. Originaldeadline/finaloff validated.
Capture seed400_582_recovery70_30s.txt SHA256
7A990E984AFDD456C68D55AE037651D89407205881C6430155BC7343AEC00863.
Not repeatability cohort. Operator permits Python or MCP: choose faster useful
feedback; flexible UART preferred, not mandatory MCP or repeated rebuilds.
E583 outcome recorded above; do not duplicate run.

E581 root937fc1df45b196a91cec8622ae2a891752a81602867b18039ca6e23ae23a7eea
seed400 built release-s/thinLTO/mathauditPASS NOTflashed; actual13B5lastoffE577.
SEEDPROFILE poststop marker constasserted834/5000/476,remaining64/arm16/shared1.
Strict --seed400 host selection; separatecycle400; profileprotocoltestPASS.
Sustainedrecovery decoder+drivenrestartcheck use explicitmarker (fixed missing
2500profile mapping too). No motor/UART. Need disabledcandidatepreflights then
oldpoint recovery before8%; newseedprofile notqualified merelybybuild.

E580 stage bench-seed400 (dependscycle400): sharedseedmin834/cycle5000;
individual476 unchanged,32usremaining/16usarm intact. Affectsdriven+passive
measuredseeds,notstartupwaveform. Hostactualpolicy30testsPASS, includingold350
explicitprofile (testwasusingRuntimealias, correctednotweakened), twelveedges,
cycle/individualrefusal,age64/63ticks+wrap. No embeddedbuild/flash/UART/motor;
installed13B5lastoffE577. Need explicitseedprofile telemetry/hostrecognition,
releaseaudit/disabledpreflights thenoneoldpointrecoverybefore8%attempt.

E579 actual13B5codegenaudit: acquire_inner3uidivcalls are2ongoingscanADC
conversions+1track-onlyfrequency, NOT3dividesonrecoveryfinaledgepath. Full
RECOVERY_POST_EDGE_AUDIT read: carrierpreparation E425 alreadynoobservedgain,
do notrepeat. Currentreport/cleanup/budgetworkrequirespreservationcontract;
oldE564seedage83us ishistoricnotcurrentWCET. No firmwareflashUARTmotor.
13B5lastoffE577. Next scopedadmission/timingexperiment with32us/16us intact.

E578 offline: failureverifier nowreports recoveryresult14/5intervals/3112us,
notdownstreamstagingerror; testPASS retainsfailure+requiresCRC/archive/off.
RuntimeAcquire/SEED_MIN shared by driven+passive acquisition: globalchangeis
NOTrecoveryonly. TEMP_ADVANCE16 timing:ci952 age<=87us,ci895<=80us,ci834<=72.5us
to retain32us floor; arm<=16us unchanged. E577 lacksfinalseedlatency (failed5
intervals). No firmware/flash/UART/motor;13B5lastoffE577. CYCLE_ENVELOPE E578.

E577 same13B5/OFF/UARTclosed: fixed80/30s holdPASS376.845eHz67831COM/67830acc,
raw392,bus10805,IRQ53.262%,sigma33.537us(fullrun),tail13.77ms sigma19.532.
Thenone80recoveryFAILED CycleTooFast14:5370ticks<5716 at5intervals/3112us,
no recoveredpower. Injectedfirstsegment4491COM/4490acc,raw431,bus10889.
CLIreported stagingprovenance because reacqrefused beforeguardinstall; CRC/
firstarchive/finaloff separatelyvalidated. Notstartupfailure orMCU/hardwarewall.
captures/cycle400_577_hold80_30s.txt SHA2E23166017C14AA868B1A3A4F907A389D54368B9EA8674981B8861E3C35F3D01;
recoverySHA4DEB6E2EEAD7682C4E0F34335BF304CFBC2B4E2A861888D2595458B34CEC2BCD.
No retry/flash/guardchange. Next recoveryadmission timing review,not more80
recoverylottery. Running8%passedonce; recovery at~372eHz unqualified.

E576 same13B5cycle400/OFF/UARTclosed.70baseline10sPASS329.970eHz19798/19797;
thenlive70->80ACK full10sPASS21603COM/21602acc,finalavg885(~376.65eHz),raw353,
bus10889,ADC49750,commitmax26,IRQ52.071%. Aggregate360.065eHz/sigma185.186
MIXEDduty/acceleration,notsteady8%stats. captures/cycle400_576_step80.txt SHA
280FA7CAA859D9E450A8B6CA82CE166C7E9FA6EF5F36D8976466A02DD8768BF4;
baselineSHA486F1C24194BB37DB289FBF1E6E7FC3B443D9EF20D8F9334D5692FFAF80123AA.
ArmACKsretained,ADC/timeline/transfer/offvalidated. No furtherguard/flashchange.
Next sustained8%/timingquality; recoveryadmissionstillold350band,notqualified.

E575 CURRENT/root13b5f0447080264d57db66c4d742f76fad62880fbc1ad8921fe7b3c344de206b
cycle400 installed/OFF/MCPclosed; release-s/thinLTO/mathauditPASS. Hostexplicit
cycle400 verifier+syntheticprofiletestPASS, old350seedcontract retained.
Disabledguard3/18,IRQbudget5,filter,atomic256,role6/3200,CPU2/6/9,archive3,
ADCphasePASS in captures/cycle400_575_preflight.txt; finalalloff/nFAULT1.
No motor oncandidate yet. Next live_armed_baseline.py --cycle400 baseline70
thenonlyifvalid --step80. Require2500/238 header; nottransfer029Fqualification.

E574 stageonly bench-cycle400=>RunGuard2500/238 +matchingRUNLIMIT. Current
029Fstillinstalled/rootoldELF/offE573; no buildflashUARTmotor. Evidence review
E573IRQ51.8325%,COM/writer22us,DMA22,gap93; measurednotWCET. Runningenvelope
only; startup/recovery/current/bus/freshness/event/arm unchanged. Exactboundary
andotherguard harness scripts/cycle400_guard_test.rs. Need hostexplicitprofile
recognition,buildaudit/disabledpreflight thenbaseline before8%candidate.
CYCLE_ENVELOPE_EXPERIMENT E574 rationale/limits; notE573reclassification.

E573 same029F/OFF/UARTclosed: live70->80ACK thenCycleTiming12 at3335122us,
6583COM/6582acc,raw311,bus10960,ADC16592,commitmax22. Guardcycle2755<2778,
reference2752.5/prior2825us,closure2.5us; avg935ticks (~356.5eHz estimator).
Retainedsame-sector cycles shorten towardfloor; not electricalwall or proven
physicaloverspeed/lockloss. No repeat/guardraise. live_573_step80.txt SHA
04677CE28DD6B63D66F53AB4E2C6CF2A2AD8D46C301C99C0AE5A8D3B2D18369B plusarm.
RawCRC/ADC/timeline/acquisition/transfer/finaloff validated. Next reviewcurrent
timing-envelope expansion evidence, not identical80retry orhardwareblame.

E572 same029F/OFF/UARTclosed: live70->69->73 acknowledgements thenoperator-
styleoff HostAbort9PASS at6272113us of10sbudget.12435COM/12434acc,raw318,
bus11008,ADC31204,commitmax23us/veto0. Aggregate330.429eHz/sigma89.318us
MIXEDduty,notsteadyquality. captures/live_572_up_stop.txt SHA
2A72CCA379E9F93264814058611EF6B1EF23231F4999D6B449FA5EB34E76AE9B plusarmACK.
Parse/ADC/timeline/acquisition/transfer/finaloff validated; stoplatency NOT
measured. No codeflash/guardchange. live_armed_baseline.py --up-stop retains
sequence; oneup/down/stopfunctionaltest,notreliabilitycohort. WriterhosttestsPASS.

E571 same029F/OFF/UARTclosed: FIRSTlive70->69ACK and10sdeadlinePASS.
Nativefirmware replies D0046/I03F0 thenD0045/I03F9, timedhostquery7s/change8s
aftercompletefirstreply.19410COM/19409acc,10000036us,raw316,
bus10746,ADC49750,commitmax22us includeswriter,acq25/overruns0. Aggregate
323.514eHz/sigma52.592us MIXEDduty,notfixedqualification. FullhandoffCRC/ADC/
timeline/offreplayPASS; fixedverifier doesn'tmodeldutytransition. captures/
live_571_change70_to69.txt SHA F2A7BAEC82C085A136FBA36D20E1FB9A1B3E9BFBC514C6CF258F6C60DD109808
plus.arm.txt retained. One livewrite functionalproof,notWCET/repeatability.
No firmware/guardchange. Next interactive up/down +stop within existingbounds.

E570 same029F/OFF/UARTclosed: one live-armed70/10s baselinePASS328.433eHz,
19705COM/19705acc,sigma45.494us (not equivalent to prior22us),raw313,bus10937,
ADC49751,commitmax22us,acqhandler24/overruns0,stack3440. Fullhandoff verifier
withcycle360/seedreanchor/dmapeer/20k PASS; no live duty request sent, writer
stilluntimed. captures/live_570_armed70_baseline.txt SHA77F601C16575964DDE614DE9A5B12AD0978CC769CEF52153C330EF06A0F4A338.
Pythonwrapper live_armed_baseline used existingstartup pacing. ArmACKchecked
but not retained E570; wrappernow saves separate.arm.txt forfuture, no rerun.
No flash/guardchange. Next enabledwriter timing/small duty request,notcohortclaim.

E569 CURRENT/root029ff46fd77ade89d65b8e46091948fa0a9f711df486d98fb2b1bcd712dd783a
installed/OFF/nativeMCPclosed. Fixed off tocancel pending livearm; release-s/
thinLTO/auditPASS, download/reset exit0. live1/0 acknowledge. Disabledguard3/18,
IRQbudget5,filter,atomic256,role6at3200/62,CPU2/6/9,archive3x,ADCphase3x report
PASS; captures/live_569_preflight.txt. Not powered qualified, no motor. Actual
enabled update writer/timing remains unexercised; do not transfer E5682us PAC
timing to fullguardedwriter. Need fullwriter timing + bounded powered control
attempt. Existing guards unchanged; rootnowequalsinstalled, supersedes41C9.

Live UART staged/root8d9855e1300b46293b553e9450ffa19bcaf50a682f652f198ff76b3e67a6863f
release-s/thinLTO/math-auditPASS, NOTflashed; actual41C9lastoffE568. live1/0
one-shot optin for next driven campaign; parser only powered+realIRQ; acquisition
anybyteabort. du<tenths>,?;16byte hex D/I replies one nonblocking TX byte/callback,
RXfirst; busyreply/newrequest stops. Parser5+reply1hosttestsPASS. Firstlink1504
overflow; split livecontrol from livedutycheck=>224; exclude idle seedcheck
harness in livebuild=>fits. Runningseedpolicy/guards unchanged. No newmotor/UART.
Need disabled newbuild preflights/fullwriter timing and command-path checks;
no livecontrolhardwareclaim. LIVE_CONTROL.md current; root!=installed.

Actual update_live_duty body hosttested via scripts/test_live_duty_writer.py:
11adapter scenarios +3preloadtests PASS. Mask/poll/write/publication ordering,
refusal unchangedmetadata, deadline unchanged, firstfault preserved, >100us
stop and timerwrap tested with mocked guard/PAC/core. Not realguard/WCET/IRQ
raceproof. No build/flash/UART/motor;41C9 lastoffE568. UART callback spans
acquisition+BEMF: enable parser only powered realIRQ, notacquisition. Next
disabled guarded-writer timing and optinUART. LIVE_CONTROL.md details.

Staged bench-live-control guarded update_live_duty API, no UART caller yet.
Checks powered owner/reason/ENABLE, realIRQ/non-recovery, matching old metadata,
fresh guard.poll then CCR transaction + PREPARED/POWER_DUTY publication under
one mask. Refusals abort, existing fault wins; no deadlines reset/syntheticCOM.
Live-prepared latch cleared on bridge revoke; only matching live COM permits
40..300. Legacy/startup limits unchanged. Fullfeature cargo check --release
PASS (existing warnings); NOT linked/timed/flash/poweredqualified. Actual/root
ELF remains41C9/offE568/UARTclosed. Need actual guarded-path refusal tests and
disabled timing, then optinUART; do not advertise livecontrol available.

Live next-COM policy added, not wired: phase_role_sequence::apply_live_prepared
requires matching nonzero40..300 metadata; never CCR-loads/UGs. Legacy limits
unchanged. scripts/live_role_tests.rs actualmodule19testsPASS (all261duties x6,
stale/unprepared/invalidnowrites). No firmwarebuild/flash/UART/motor; installed
41C9 lastoffE568. Next guarded owner publication + optinUART, not availableyet.

E568 current/root41c9c31fb111ef2656a416135e16ea7fb045050fabe6b5109742220664516653
installed/OFF, nativeMCPclosed. bench-live-duty-check PAC adapter disabled24/24
PASS,max2us: allsixroles,4/8/30/7% preload writes, timercontinuity/config stable.
Not active shadow/pulse readback, poweredWCET or liveUART support. First045A
test8/24FAIL retained; test wrongly compared live COMP2.VALUE bit30, now masked.
Bothrelease-s/thinLTO/audited; no motor. LIVE_CONTROL.md and LAB_REPORT E568.
Next guardedowner/PREPARED/POWER_DUTY + optinUART integration; guards unchanged.

Added unlinked live_duty.rs prepared-request/coherent-preload transaction.
3hosttestsPASS (all40..300 atthreecarriers;320wrap/duty modelscenarios;
invalidstate no writes). No firmware/flash/UART/motor. InstalledD755/offE567.
RM0444Rev6p577 TIM1 UDIS officialindexedtext confirms shadowhold; UG resets
counterevenwhenUDIS, so forbidden. FullPDFdownload stalled; canceledownfetch.
LIVE_CONTROL.md source/evidence/nextPAC+guardintegration. No claim hardware
coherence/timing or live duty support from model tests alone.

Operator clarified MCP is optional: Python/scripts allowed when faster; flexible
UART exploration is the intent. LIVE_CONTROL.md tracks implementation.
Added unlinked request-only live_command.rs (du<tenths>,?,immediate stop,
malformed/250ms partial timeout stop,max300); standalone rustc host5testsPASS.
No firmware build/flash/motor or live duty authority added this turn. Current
D755 still installed, lastoffE567/UARTclosed. Next guarded coherent TIM1 duty
update and foreground integration, NOT merely changing POWER_DUTY (existing
PREPARED/fixed-carrier rejects it). Preserve session deadline and all guards.

E567 CURRENT installed/root D75568CCD5BF8B36BF3807DA26336E031562E6D1D82E2ED75849542A3FDF3D2E
compact summary build/OFF/nativeMCPclosed. Download exit0 then OpenOCDreset;
native disabled guard3/18,IRQbudget5,filter/atomic/roles3200/CPU2,6,9/archive/
ADCphase3 pass (terminal_567_preflight.txt). One quiet run200 built-in ramp,
startup61/BEMFrequested70/window10s FAILED startup FeedbackStale(reason4)
after5947us,30scans,rawpeak759,busmin11498. BEMF transfer0, never applied70.
terminal_567_quiet70_attempt.txt retains complete quiet UART: new DRIVESTOP/
DRIVEADC/DRIVETRANSFER correct, no old COREOBS/POWERPATH leak. Final native
off/p/i all gates/en/MOE/CCRs0,nFLT1. Summary failure path bench-verified;
successful BEMF summary/repeatability NOT verified on newbuild. No guard or
live-control implementation change. Direct run200 startup differs from old
run50+late50->200 serial ramp; do not attribute this refusal to7%duty.

Terminal summary candidate ROOT D75568CCD5BF8B36BF3807DA26336E031562E6D1D82E2ED75849542A3FDF3D2E
built release opt-s/thinLTO/codegen1 and emitted math-audited, NOT flashed.
Actual board remains C7D0, last explicitoff via MCP after native attempt566;
MCP connection closed. Added driven_run::terminal_summary post-coast/off ONLY,
quiet mode: DRIVESTOP(acquisition reason/time/duty/requested BEMF), DRIVEADC
(retained startup scans/raw peak/busmin,explicit uncalibrated), DRIVETRANSFER.
Only transfer result1 prints existing POWERPATH/POWERFEEDBACK +new BEMFSTOP
(corestop/interval/controller estimatedeHz/COM,not lock proof). Early/refused
transfer cannot print old powered/core snapshot. Existing fullcap1 path kept;
quiet driven runs suppress verbose/stale core observe_summary. No live TX,
ADC reads, new ISR work, guard changes, or live throttle support added.
scripts/test_drv_terminal_summary.py: actual formatter compiled/executed with
host stubs, zero-scan/rawpeak/busmin/transfer0,1,2/active-output refusals PASS;
postcoast-only caller test PASS. Existing57driven+13seed PythonPASS. New ELF
still needs bench preflights/quiet successful+failed-attempt verification.
Do not mistake current build for installed image or these host tests for
hardware timing. Next live control must preserve fixed session deadline,
electrical/tracking guards and actual30% maximum; existing BEMF anybyte abort
and segment-fixed carrier remain until explicitly implemented/tested.

Native MCP experiments after refresh, 2026-09-14: no code/flash/guard changes.
Retained captures/mcp_565_ramp80_attempt.txt and mcp_565_macro_result.json:
server byte-paced ramp reached only140 by handoff; DRIVEOBS reason18, zero
commands/reads. Prior CORE counters leaked into summary; NOT a BEMF run.
Do not repeat this macro: requested short per-byte delays did not meet
startup schedule (Windows scheduling granularity is a hypothesis).
Then mcp_566_direct200_bemf80_attempt.txt: firmware run200 built-in ramp;
DRIVEOBS reason4=FeedbackStale,stop4266us,15scans,feedback age614us,5accepts,
observermax21us/overruns0. No BEMF handoff; requested80 never applied.
SEEDTIMING discarded_fault3/count1/anchor3 DID execute, but failed startup
is NOT branch or handoff qualification. Native off/p/i verified alloff/nFLT1;
MCP connection closed cleanly. No retry/guard relaxation/higher duty.
Next enable useful compact firmware-only stop/current/bus summaries and
safe live BEMF duty control, not pretending current any-byte-abort shell
already supports it. Exact prior quiet8% stop remains unknown.

MCP REFRESH RESOLVED: native serial MCP list_ports/open/write/read now work.
COM41/115200 open connection eb10aa93-342b-474a-8286-cbefcad2d483 (revalidate
before reuse). Native off/p/i freshly confirms mode0,coast0,all gates/en/MOE/
CCRs0,nFAULT1. No new motor run or firmware change. Historical transport
block below is resolved; use native MCP, not Python fallback.

2026-09-14 live-terminal / MCP handoff (after E564): user explicitly wants
interactive MCP testing, not more fixture-only tiny duty increments. Existing
C7D0 BEMF holds abort on ANY UART byte; duty is segment-fixed. Do not claim
live BEMF throttle adjustment exists. No firmware/config/guard change made.
Direct COM41 terminal experiments with cap0: first startup mistimed by host
read pauses (target only90 at handoff), failed before BEMF; corrected live
50->200 ramp acknowledged all steps. 8% entered BEMF then stopped at63723us,
120 accepted/121 COM; quiet COASTREF stop7 means powered owner ended, NOT
the exact electrical/tracking fault. 7% then ran10000032us,19545 accepted/
19546 COM,order_bad0. These are exploratory summaries, not qualification;
no calibrated current or exact8% fault claim. Final explicit off/p/i verified
all gates/en/MOE/CCRs0,nFAULT1. Fallback Python UART closed (SERIAL_CLOSED True).
User redirected to FIX MCP before more motor testing. Serial MCP list_ports
repeatedly returns Transport closed; no serial-mcp-server process exists.
Config executable E:/m/rust/serial-mcp-server/target/release/serial-mcp-server.exe
args serve,cwd its repo. Fresh subprocess MCP initialize + tools/call list_ports
PASS, finds COM41, exits0 cleanly. Thus fresh server starts; this session's
client transport is dead. Cause of original exit unknown. Installed CLI mcp
has no restart subcommand. Need client reconnect/restart, not random config
or firmware changes. No motor commands while restoring MCP.

E564 C7D0/OFF/Uartclosed.73/30srecoveryPASS340.004eHz,74/10sholdPASS346.242.
Then74recoveryattemptFAILED INITIALseedacquisition:missingepoch2consumes
reanchorat3,laterlongintervalfault3 with11intervals/6cycles; no poweredhandoff.
Handler23us/overrun0,discardedfault0,startupraw1121/bus11486,CRC/finaloffPASS.
No retries/code/flash/guardchange.74steadyholdworks,recoverynotqualified;
not74dutycausingstartupfault(startup61). ExperimentE564 exacthashes/metrics.
Next entrywindow/deadlineanalysis,not75step or thresholdrelaxation.

E563 C7D0/OFF:72/10sholdPASS337.700eHz,72/30srecoveryPASS336.191eHz,
73/10sholdPASS338.904eHz. Allactualacqhandler23us/overrun0/discardedfault0,
strictADC/phase/CRC/timeline/finaloffPASS,Uartclosed. 72recoveryseed1000/
remaining42us/arm7/deadline146usspare. No guards/code/flashchanges. Exact
captures/hashes CYCLE_ENVELOPE_EXPERIMENT E563. Next73recovery, not more71
cohorts. Long-gaprestartstillunexercised; onlyone72recovery,not3/3.

E562 C7D0/current/OFF:69/10sholdPASS319.55eHz; fixed71/30s recovery3/3PASS
329.416..331.621eHz,actualacqhandlermax23us/overruns0,IRQ50.92..51.41%,
allADC/phase/CRC/timeline/originaldeadlines/finaloffvalidated,Uartclosed.
Run3missing-epochreanchor(count1,anchor3,discardedfault0); long-interval
restartUNEXERCISED. No falsebranchqualification or calibratedamps claim.
CYCLE_ENVELOPE_EXPERIMENT E562 exactcohort/hashes. Nextone72/10s sameguards;
no more71repeats justtorollrarebranch. Old10us microbenchmark staysfailed.

E561 CURRENT/rootc7d0b31adacd6edfed04bafc5b35aefb1b9540d407a55d2fb14bb9397451827e
installed/OFF,frozenseedtiming_561. Disabled5durationboundarycasesPASS exact
50allow/51stop,syntheticnotWCET. Guard3/18,fullfive3200CPU2/6/9,ADCroute3PASS.
Removed rejectedinline tofitFLASH. Firstbuildoverflow352 erroneouslyfollowed
byold4BCdownload;explicitreset/offguardresolved,nomotor. Checktool exitcodes.
No motoroncandidateyet. Nextone69/10s wholehandlermeasurement with--cycle360
--seed-timing-reanchor --dma-peer --carrier-hz20000; actualmax50/overrun0,
alloriginalguards. Oldisolated10usfailuresremainfailed;no reliabilityclaim
unless discardedfault3actuallyexercised. ExperimentE561 scope/commands.

E560 ROOT4bc0c363d8242fbb8d1046b43027f56974581a0ab154abff7f1a0fa81c3dfcab
staged/frozen seedtiming_560,NOTflashed;actualDC0DlastoffE559. Existinghost50us
observerlimit nowimmediatestop+overruncounter beforehandoff. DRIVENIRQBUDGET
mandatoryzero for--seed-timing-reanchor.13seed+57drivenPythonPASS,release-s/
thinLTO/audited. NoUART/motor. Nextdisabled50/51stoptest+preflights. Isolated
10us failuresstayfailed; fullpathmeasurement needsownrationale and doesnot
cover outerIRQ/release/blackout. ExperimentE560 scope; root!=installed.

E559 CURRENT/rootdc0db63c9b63770cac3811d5caaeeed49387a48faed6d2052de78aa085ba410e
installed/OFF,NOTpoweredqualified. Duplicatecheck removal4a26still11usFAIL10;
explicitinlineacceptDC0Dregresses12usFAIL10.64semanticsPASS both,40hosttests,
finaloffverified/UARTclosed,no motor. DC0Dfrozen seedtiming_559b;4a26notfrozen.
Stop tiny speculative variants; inspect timedbracket/fullISRbudget.10us is
self-imposed allocation notsiliconwall; retainfailedresult,nowaiver/no pass
without separately justified fullpath evidence. ExperimentE559 details.

E558 CURRENT/root19b8e8301ea486ac4969d92eb6dd92817625b78fb5ee0e0822757e86561990e8
installed/OFF,NOTpoweredqualified. Inplacereset preservesstart,seen0 hidesold
sectorhistory;216poisoned-state combinations+existing40testsPASS. VariantA
750909resetcost13us; B commonanchor/no recursivechecks11us, bothFAIL10.
64onboardsemanticsPASS each,finaloffverified/UARTclosed,nofullpreflight/motor.
Frozen seedtiming_558a/558b; experimentE558 details. Gate unchanged; inspect
remainingduplicatechecks/codegen,notrounding/subtraction/waiver.

E557 CURRENT/root BF5f9527143c84c8e6a56e6cdb2a28662d4d3a4219ed245e4a6d4fc9efa83587
installed/OFF,NOTpoweredqualified. Exactconstsectorhelpers removeacceptuidivmod,
compiletimeallvalidinputs/39RusttestsPASS. Frozen seedtiming_557. Disabled64
semanticsPASS,cost18->15us stillFAIL10;nofullpreflight/motor. Finaloffverified/
UARTclosed. Next resetinvalidation equivalence before skipping historyclears;
do not widen gate. CYCLE_ENVELOPE_EXPERIMENT E557 provenance.

E556 CURRENT/root E752a7c56ee425a2f6b22efb5fed7fd878ec73873e0447e76d8124ab2d4ffe02
installed/OFF,NOTpowered-qualified. Newseedcheck64semanticsPASS but18us>10us
isolatedpolicygate FAIL; no motor/fullpreflight. Frozen seedtiming_556.
Addingharness secondcaller outlinedQualification::accept forLIVE too, now
uidivmod0800908a +resetmemorywork. E555inlinecodeviewdoesnotapply. Nextbounded
sectorremainder removal/timing,not gateincrease. Finaloffverified/UARTclosed.
CYCLE_ENVELOPE_EXPERIMENT E556 exactstate/provenance.

E555 ROOT0fe48ab540f544aee2d269c62a50be0d1231c2a31c479377a8c7efceaa51cae0
candidate seedtiming_555 built/frozen/audited,NOTflashed; actual09C3lastoffE553.
Feature bench-seed-timing-reanchor nowselectsliveconstructor; postrunSEEDTIMING
discardedfault0/3 + existingcount/anchor, mandatoryfixture--seed-timing-reanchor.
13seed+57drivenPython/39RusttestsPASS. Release-s/thinLTO/codegen1. Newreset
branch emitsmemclr helpers+backbranch,untimed; disabledtimingbeforepowereduse.
No UART/motor. CYCLE_ENVELOPE_EXPERIMENT E555 fullstate. Root!=installed.

E554 staged bench-seed-timing-reanchor pure policy, NO live constructor use.
39RusttestsPASS via scripts/test_seed_timing_policy.py. E553 failed gap is
809us command spacing+262us acceptance-offset shift,not1071us schedulerstall.
Candidate one shared restart discards corroborated long interval, original
deadline retained,12freshintervals/7cycles; discardedfault3 retained. Replay
oldfails/newseed1543ticks at11785us,not hardware/handoff proof. Need live
metadata/strict host validation/codegen/timing beforebench.09C3lastoffE553.

E553 current09C3/OFF,Uartclosed. Two additional71/30s recovery attempts:
02PASS325.443eHz/54658COM/54657acc,IRQ50.426%,raw397,bus10984,ADC139259;
03FAILED initial acquisition,seedfault3:epoch1->2 gap1071us>1000,notpowered
CycleTiming. CRC/finaloff verified, no retries/flash/guardchange. WithE550,
71cohort2/3campaigns,notrepeatable345eHz qualification. Exacthashes/commands
in CYCLE_ENVELOPE_EXPERIMENT E553. Average-only policy NOT deployed:9hosttests
include1428us single-cycle counterexample.32us armfloor is handoff-only,
not an independent steady speed bound. Next retained startup timing failure.

E552 host-only cycle_window_policy.rs + harness8testsPASS (4existing monitor).
121050 constant-acceleration pairs: max extra detection2772us, not400us;
old474->new462, refusal36->42. Warmup/wrap/latched-window refusal tested.
Two-cycle average does NOT preserve instantaneous floor; alternating cycles
can pass. No firmware/flash/UART/motor;09C3lastoffE550. See
CYCLE_ENVELOPE_EXPERIMENT E552 bound/assumptions and required timing review.

E551 offlinewindowstudy4testsPASS. E537/E545/E550 single-referencecyclefails,
twocyclemeanpasses localboundary;originalfailuresretained,NOTwholerunreplay
or safe-deploy proof.100us syntheticjittertolerated/150ustrips; acceleration
detected400uslaterinexample. No firmware/hardwarechanges;09C3lastoffE550.
CYCLE_ENVELOPE_EXPERIMENT E551 scope;needlatency/warmup/wrap/safetyproof
before changingrunningguard measurementwindow.

E550 current09C3/OFF: one71/30srecoveryPASS344.929eHz,remaining39us/arm7,
raw369,bus10948,IRQ50.920%,ADC/phase139259,deadline208usspare. Thenone72/
10sholdFAILED CycleTimingat8.627s:guard2761<2778,ref2758.5/prior2926.5us,
twocyclemean2842.5;avg945ticks belowoldseed952(recovery72NOTattempted).
Raw304,bus11044,IRQ51.366%,ADC/phase42922,allfinaloff/UARTclosed. No retry/
code/flash/profilechange. CYCLE_ENVELOPE_EXPERIMENT E550 hashes/limits.
Reviewcycleestimator/envelope andseedbudget, notautomaticfloor-ratchet.

E549 same09C3/OFF: twofixedadditional70/30srecoveryPASS =>3/3withE548,
340.532..340.714eHz,IRQ50.955..51.001%,remaining40..41.5us/arm7,
alloriginaldeadlines/finaloff/ADCvalid. Thenone71/10sholdPASS345.338eHz,
20720COM/20719acc,mincycle2807,raw292,bus11008,IRQ51.068%,ADC/phase49751,
stack2668/UARTclosed. No code/flash/profilechange. Next71recovery not70repeats
ornewceiling; CYCLE_ENVELOPE_EXPERIMENT E549 fullcohort/hash/limits.

E548 samecurrent09C3/OFF: one7.0%30s recoveryPASS340.693eHz,resumed27.991285s,
57219COM/57218acc,sigma23.011us,minrecordedcycle2842,IRQ51.001%,raw321,
bus10913,ADC/phase139260,stack2668. Freshseed996/remaining41.5us/arm7us,
deadline200usspare; oldseedlimits+32usarmfloor unchanged. Freshguard3/18,
finaloff/UARTclosed. CYCLE_ENVELOPE_EXPERIMENT E548 exactcapture/hash.
Onepassnotcohort; nextboundedrepeatability thenstagedduty, noceilingratchet.

E547 CURRENT/root09C32D2F8508C3C36EFBC87629CB474C38B68A20F718101F74713D05C6971ABE
cycle360_547 installed/OFF. Explicit runningcyclefloor2858->2778 only;
event238..1000,seed350,arm32us/electricalguards unchanged. OwnpreflightsPASS,
33Rustguardtests+hostprofiletestPASS. One7.0%10sholdPASS340.585eHz,
20435COM/20434acc,minrecordedcycle2844us,IRQ51.041%,raw371,bus10948,
ADC/phase49751,stack3460/finaloff/UARTclosed. CYCLE_ENVELOPE_EXPERIMENT
exacthashes/command/limits. Next7.0recovery notceilingratchet. --cycle360
mandatory fixtureflag; COMkeep/readcall off. PSUblocker resolved E546.

E546 PSU observation blocker RESOLVED: operator watchedactual30s6.9%hold
and reported11.7V,about70mA,noCC/CVblips. Currentinline7E55PASS335.122eHz,
60322COM/60321accepted,raw322,busmin10996mV,stack3460,finaloff/UARTclosed.
Capture psu_anchor_546_start61_hold69_30s_01.txt. Thisisnewcurrentpoint
observation, not reuseofE188. Approx0.819W supplypower, notADCcalibration
or proof ofpeakcurrent/efficiency. Do not keep requestingthissameanchor.
No flash/profilechange; actual7E55,rootstillretiredDB3B. Higherboundaryopen.

Goal handoff afterE545: blocked pending independent PSU observation at current
~334eHz/6.9% point (running voltage/current/CV-CC). Same blocker revalidated
three consecutive continuations; no operator reply, no identified supply
tool/script/interface. Serial enumeration foundCOM3,7,41,42; unidentified
ports not probed. Old~70mA observation is~238eHz, not this operating point.
Resume with operator-observed existing-setting hold, not automatic higher
profile or equivalent unobserved repetitions. Fullgoal remains unfinished.
Actualboardinline7E55,lastoffverifiedE545; rootDB3B retiredcandidate differs.
Archive assessment/portable inventory current, 7E55 recovery3/3 at6.9%.
No hardware commands during blocked audit; do not claim fresh off readback.

E545 CURRENTboard restoredinline7E55/OFF,freshguard3/18PASS/UARTclosed.
RootstillDB3B retiredcomkeepcandidate, notinstalled. Candidate69/30srecovery
PASS333.873eHz thenone70/10s FAILED CycleTimingafter2.041s,step6 delta2853
<2858;reference2857.5us,closure-4.5. No retries/guardchange. COM_ARM_EXPERIMENT
E545 exactcaptures/hashes. Keep-runningCOM didnotremoveboundary;retired.

E544 CURRENT/root DB3B8AAD13F1A715D795299103CD4BD4B6C212B6429C5C88224BBAB23F2F9055
comkeep_544 installed/OFF. Idlearmcheck128casesPASS,armmax1us/slackmin31us,
stopped/running/staleUIF-NVIC/stoprearm tested withNVICmasked,NOTISRWCET.
Freshguard3/18/fullfive3200CPU2/6/9/ADCroutePASS. One6.9%10sholdPASS
334.777eHz,20086COM/acc,IRQ51.296%,raw307,bus11020,COMP40/COM44/commit21,
ADC49750/phase49751 validated,stack3468/finaloff/UARTclosed. No recoveryyet
oncandidate; old7E55 cohortnottransferred. COM_ARM_EXPERIMENT E544 exacthashes.

E543 stagedbench-com-keep-running NOTflashed; omitsonlyinitialTIM16CENclear
atarm, keepsDIER/SR/NVICclear/finalCEN/init/stop. Root65D4783DB480A7542DB55FEB79F650762D906D0D89E89155431DE9EFBB39EC2B
frozencomkeep_543 withaudit; actualstillinline7E55/lastoffE541. Release-s/
thinLTO/codegen1,2newhosttests+2regressionPASS. Emittedwriteschecked,NOTtimed.
COM_ARM_EXPERIMENT.md specifies missingdisabledtimerqualification beforepower.
Fixture --com-keep-running requiredforcandidate. NoUART/flash/motor E543.

E542 acceptance timestamp audit: core mask/read/reset/arm order matchesfrozen
minz/AM32. Guard sampleslater atEV_ACC. E537 guard2846us vsreference2842.5us
(closure+3.5us), BOTHbelow2858: movingguardtimestamp alonewouldnotfixrefusal.
15cyclefaulttestsPASS. COMadapterstop/restart/pendingclear differsfromAM32,
not measuredcause. No code/flash/UART/motor; COMP_READ_CADENCE_EXPERIMENT
E542 retains source anchors/limits. Do not equateclosuretoISRlatencybound.

E541 currentinline7E55 now3/3 original30s6.9%/20k recovery cohort(E540one+
E541two predeclared, no exclusions).333.437..333.579eHz,IRQ50.660..51.119%,
sigma22.304..23.052us,busmin10829..10937mV,raw343..350,stack2668.
Freshremaining43us/arm7us allthree,deadlines131..261usspare. ADCandphase
139259/139259/139261 validated. Finaloff/UARTclosed, noflash/code/guardchange.
COMP_READ_CADENCE_EXPERIMENT E541 table/hashes. Stopidenticalrepeats;
currentanchor/higherboundarymechanism stillopen, not fullgoalcompletion.

E540 currentinline7E55 one30s6.9% recoveryPASS:333.437eHz resumed27.991064s,
56000COM/55999accepted,sigma22.304us,raw350,bus10925mV,IRQ50.660%,stack2668.
Freshseed1007ticks,remaining43us/arm7us,deadline236usspare. ADC139259 and
phase139259 validated; initialbaseline revoked onrecovery, noampsclaim.
Freshguard3/18PASS; finaloffverified/UARTclosed. No flash/code/guardchange.
Capture inline_540_start61_reentry69_30s_01.txt; firstcurrentbuild recovery,
not3/3cohort. Exactcommand/hash in COMP_READ_CADENCE_EXPERIMENT E540.

E539 archive deliverables refreshed: BEMF_PARITY_PLAN currentthroughE539,
PORTABLE_WINS adds invariantcarrier/const-boundedseed lessons and retiredcall
warning. Frozen128files verified; historical3summaries reproduce;5archive
testsPASS. E485three saved69/30s recoveries replayPASS withmatchinghashes.
Retained reference's poor20.548%qZC run too; no binzcycle-vs-minzwindow sigma
comparison or pooledbuild qualification. Current7E55 recovery stilluntested.
No motor/UART/flash; PSUobservation request pending, not a liveprocess wait.

E538 offline replay corrects E537 counts: inline49750 deliveredADC/49751
phase records; call43802/43803. Original table conflated them, now corrected
in COMP_READ_CADENCE_EXPERIMENT.md. E53549751/49751 remains correct.
Initial inline baseline residual sum7.029252 rawcounts, NOTcalibratedamps;
minimum accepted cycle2900us leaves42us above2858 guardfloor. Guard is an
accepted-cycle envelope, not CPU/independentrotorspeed proof. Requested PSU
observation at current~334eHz; existing~70mA was~238eHz. No hardware commands,
firmware changes or speed-limit change. E537 installed/off state is historical,
not freshly queried this entry. Do not restart offset/settling grind.

E537 CURRENT/root inline7E55F5D0799CE677519928E4CC61E7F486B7A27E1FB7B8738EC09AC3D3E82CA6
restored/OFF; frozen compinline_537. CallDADD5C1D1E34979B5724EE1631305212C32EFE9A2E045B72974255D23EF43405
frozen compcall_537 RETIRED after6.9%hold CycleTiming2846<2858 at8.804s.
Inline same6.9%10sPASS333.638eHz; candidate335.692eHz untilstop. No7%attempts,
no retries/gate changes. Both own disabled guard/fullfive3200/ADCroutePASS.
Added disabled-only12readspan check: inline3..4us,call8..9us; NOT fullISR WCET.
cpucheck only instrumentation pair overhead2/6/9, not comparator timing.
Three host testsPASS, bothrelease-s/thinLTO/emittedaudited. No newlive recorder.
COMP_READ_CADENCE_EXPERIMENT E537 exactartifacts/failure/limits. Restoredinline
guard3/18/finaloffPASS. Do not carry call variant asfix; this cadence intervention
didnotremovefault. Baseline short holdqualified, recoverynotretested on7E55.

E536 staged comparator call-boundary A/B, NOflash/UART/motor. ActualBC876/off
unchanged. Root built6D82 inline baseline; frozen compinline_536_6d82 and
compcall_536_20f5. Candidate bench-comp-read-call only noinline StaticComp read,
same fresh COMP2 load/polarity/filter count; no new recorder. Bothrelease-s/
thinLTO/math-audited,2hostmarker testsPASS. Baseline15instruction loop; candidate
BL/helper onevolatileload verified, cycles NOTmeasured. See
COMP_READ_CADENCE_EXPERIMENT.md exactfeatures/hashes/bounded preflights/A-B plan.
No direct/reject/sparse collectors in pair; existing CPU/feedback evidence kept.
Binz changeover-only test NOTisolated: measuredseed entersIRQ and wrapper
refuses returntopolling. Sharedcore untouched. Bothnewimages need own disabled
timing/fullchecks beforepowereduse; no borrowingBC876 qualification.

E535 same frozenBC876 installed/OFF: one6.1%startup/6.9%BEMF20k10s holdPASS,
333.299eHz,19998COM/19997accepted,rawpeak310,busmin11140mV,IRQunion54.736%,
ADC49751/3200 validated,tailcycles2923..3048us median3000.5,stack2972.
Initial host invocation omitted --dma-peer and failed AFTER motor completion;
same saved capture revalidated with proper flag PASS, no rerun. Fullcore
trace0/QD85epoch/count/timeline/finaloff verified. Artifact restorebinz_535_start61_hold69_10s_01.txt.
Binz driven startup works now; recent AM32startupfailures not a demonstrated
rig-wide inability to spin. No new probe or guard change. Next return main
binz software comparison, not prolonged AM32 startup tuning. Memo E535 in
INSTRUMENT_DISCIPLINE_RESPONSE has exact commands/hash/limits.

E534 CURRENT BOARD BINZ frozen BC876BF0919EDC8B3FF00019A8D44DB8D3DE7142EB72C84151103EB19689B630,
NOT AM32; disabled/off. Guard3/18, fullfive3200preflights CPU2/6/9us,
ADCroute3checks and direct raw128/256cycle gate/semantics/wirePASS. No motor.
Artifacts restorebinz_534_*.txt. Root ELF differs; use frozen direct_bc876.
Operator's GRAYBEARD_INSTRUMENT_DISCIPLINE read fully, retained response in
INSTRUMENT_DISCIPLINE_RESPONSE.md. Adopt small question-specific observables,
separate qualification/diagnostic builds, measured cost/retirement. Caveats:
bidir also changes startupfilter/desync; E527 didn't prove167eHzhandoff;
u8 perCOM30s340Hz=61200B not16k; observereffects don't automatically cancel;
1kHz foreground can miss events. No adoption of record-only CycleTiming or
new guard waiver from advisory memo. E478 mid-persistence hypothesis stays
falsified as necessary cause. Next choose one software experiment, not suite
of new instruments. COM41 binz off/p/i; all finaloff verified/UARTclosed.

E533 CURRENT AM32 02592720FB8A1DCA2BEA0F2915BED553FE3399ECA2DBE8E541D2D5804D4AB2FC
installed/PD1low UARTverified. Corrected UART boot comStep(2) mismatch with
initialized step1/rising1: board now comStep(step)+changeCompInput while disabled.
One10input2s stillFAILED(zc0/running0/CCR0). Not a demonstrated startup fix.
NativeO3/math audit,3decoder testsPASS. No guard/timing threshold change.
Local DRV datasheet pp6/35 confirms1ms wake specification; existing delay1ms
is not evidence of a wake fault. AM32 build memo E533 details. Avoid more
unmeasured startup tweaks; known-config E530741eHz remains short-response only.

E532 CURRENT BOARD restored frozen E530 E2BFFF04D90DB475173018357E4DC271B5F80FC0868022BF3A26774713E61E99,
disabled UART+SWD verified. Root AM32 obj/source still E531 97307B38 (differs).
Fixed six-attempt startup cohort E531 order10,7,7,10,10,7 input,2s each:
all failed zc0/running0 at stop; each PD1low. Lower input did not help.
Restored exact frozen E530 then one10input2s also FAILED; snapshot addition
is not necessary for failure. Stopped SWD uart0/adjusted0/latch0,PD1ODR0;
ADC voltage raw1227, DMA1084/1227/942 (not calibrated current or loaded bus).
No threshold/code change, no repeat-until-pass. AM32 build memo E532 full
cohort/hashes. Next initial-startup state/timing, not high-speed or hardware claim.

E531 CURRENT AM32 97307B38AAD24A9E0213661E4A673307286D3763D3B5429791C9B95363BDCAB7
installed/PD1low UART verified. Added foreground-only command-boundary RAM8x7
snapshots, post-stop AM32SEGS/AM32SEG; no live TX/masking/controller changes.
One10->30 diagnostic ramp1.5s per5pointstep FAILED already at first10 endpoint:
zc0/running0/CCR0/stuck-rotor latch102, same every later endpoint. This run
never established high-speed operation; not evidence30%is too fast. Prior E530
failure stage remains unknown. No retries. NativeO3 build/audit,3decoder tests.
Next startup cause, not ceiling tuning. Binz guards unchanged; E530 known-config
741eHz short response remains valid separate evidence, not sustained parity.

E530 CURRENT AM32 E2BFFF04D90DB475173018357E4DC271B5F80FC0868022BF3A26774713E61E99
installed/PD1low, frozen captures/reference/am32_e530_e2bf with emitted audit.
UART-only setInput now bypasses receiver mapping entirely: dummy newinput48
was selecting reverse in bidir mode. Configured direction1 preserved; stopped
SWD confirms forward1/input0/adjusted0/latch0/PD1low. Known defaults+bidir1.
Four attempts retained:10input2s PASS331eHz;10->20 ramp PASS586;10->25 PASS741
(actualPWM26.519%);10->30 ramp FAILED zc0/running0. Ramp1.5s per5pointstep.
No per-segment telemetry, so failure stage UNKNOWN, not a30% speed-wall claim.
No identical retry. Full table/hashes AM32_DRV8304H_BUILD.md E530. Hardactual
30%cap/PSU-only AM32 exception remain, NOT binz production qualification.

E529 CURRENT BOARD AM32 BC0772AD4E83A485374D077FE42297734B37EF0B4CA37C5EF2699CB9D2454B8A,
installed/PD1low verified UART and stopped SWD. Known defaults+bidir1 remain.
E528 10input response334eHz, subsequent20 and10->20 ramp failed startup.
Found UART stop0 overwritten by receiver adjusted_input46; stuck latch102.
E529 mirrors UART after receiver mapping, preserves stock stuck-rotor guard.
Stopped SWD now adjusted_input0/uart_input0/latch0: stop reset fixed, NOT entry.
One10input2s still failed(zc0,running0). No further25/30 on known settings.
Earlier E52625~687/30~831 responses remain observed, contaminated-config only.
AM32 build memo E528-529 records failed attempts; next configuration/startup
bisection, not more instrument optimization or hardware blame. COM41 AM32 s
stop only; no binz commands. Hard30% compare cap and PSU-only exception remain.

E527 CURRENT AM32986157652613BE207FCAB270E8302A015376FE055FB7FA359B62101A5B144532
installed/ENABLElow. Found E524-526 settings contaminated by old binz code at
AM32 EEPROM f800: live48-byte fields match former84DF text except loader edits.
Auto-advanceF0,comp_pwmD5,bi_direction05,advance22 vs binz16. Faster earlier
runs remain observations, NOT known-default baseline. Now board-only loader
uses repository DroneCAN48-byte defaults (2.19 baseline), zero rest, current
version tags, bypasses foreign flash. One10 and one20input/2s each yielded
zero_crosses2/~21eHz: entry failures, NOT successes despite old fixture exit0.
Fixture now rejectszc<=20. No30attempt on this config. Both finalPD1low.
AM32 build memo E527 evidence and pending configuration/startup bisection.

E524-526 CURRENT BOARD IS AM32, NOT BINZ. Operator explicitly authorized
PSU-only reference response tests up to20%, then25/30%;800mA setting retained.
Installed AM32 ELF7826B72DD521DC7E1CB4F2DE59C84A0BBAC6009A3D0D89DC2AD8CEBC526F1DD7,
standalone vectors08000000. UART COM41 accepts integer percent1..30, s=stop;
NO binz shell commands. Hard compare cap30%, PD1low on stop/deadman/boot,
HardFault PD1low/MOEoff. AM32STOP sixhex words: avg,zc,ARR,CCR1,running,PD1ODR.
All snapshots emitted after PD1low; no CRC/independent qZC claim. Deadman fixed
to advance in periodic TIM6, not foreground setInput calls. Native AM32 O3
compiler settings retained (not binz opt-s/LTO). Board last verified ENABLElow.
2s response tests: input10~268eHz,15~432,20~564; then new30cap build25~687,
30~831eHz. No premature stop reports in E525-526; each finalENABLElow.
Short controller-reported response, NOT sustained lock/recovery qualification.
See AM32_DRV8304H_BUILD.md E524-526 and captures/am32_52*.txt. Firmware guards
remain absent for this explicitly authorized reference exception; do not
silently apply the exception to binz or infer100% permission.

E521 operator review redirects priority from diagnostic micro-optimization to
AM32 same-board control readiness. No AM32 flash/run yet. Prepared ELF exists
in E:/m/robot/esc/AM32/obj, UART port is implemented; links08001000 and requires
bootloader/standalone entry handling. Current/LVC protection not bench-qualified;
do not substitute PSU-only operation for the goal's firmware safeguards.
30% actual PWM ceiling remains, not reviewer's100%. Before review, local edits
outlined reject-time callback and snapshot record/CRC: opt-s still fails288B
overflow (first variant264B data/288B total; CRC outlining288B). No flash.
Installed/root remains E520 opt-z84DF, OFF/not powered-qualified. Source differs.

E520 latest actual/root84DFB2A391E6CB13BA139061593BA538A0DBE81BA41463B54412EC47CD9FEF48
installed/OFF, NOT powered-qualified. New bench-reject-time late-only callback
ring8 records interval/PWM/TIM6 bracket, RT85 strict host decode.231lib+34Rust,
429PythonPASS. opt-s link overflow960bytes; opt-z/LTO fits but disabled timing
FAIL:174/546/387/81/77cycles versus128/256. Guard3/18,240semantics,QD85+RT85
wrapped sequence PASS, finaloff verified. No motor/fullpreflight. Probe memo
E520 hashes/limits. Return to opt-s and reduce footprint/cost; do not use this
z build for powered runs. Campaign fixture does not yet require RT85 decode.

E519 offline emitted-loop audit: drv_persistence_codegen.py recognizes static
ADC_COMP COMP2 read loop, no calls;15 successful-iteration instructions BC876
vs16 current7C7E (index-preserving mov). Active probe is NOT cadence-neutral;
E514 no-op identity doesn't cover it. Three audit tests PASS. No flash/UART;
7C7E last verified OFF. Probe memo E519 addresses/reproduction/inference limits.

E517-518 supersede probe-cost refusal: current/root7C7E773BB21E7CE50D00C5E1F2306228883CBFDB04E7EA20F72AAA362AAC32EC
installed/OFF. State distinguishes open from closed/consumed first read;
reject126<=128cycles,accept236<=256; all disabled preflights PASS.229lib+34
Rust,85core,422Python before new capture regression test. One69/30s recovery
PASS333.903eHz,sigma24.979us,IRQ54.411%,56078COM/56077acc,raw338,bus10948,
stack2200. One70/10s diagnostic FAILED after222154us CycleTiming12,step4
2844<2858,438COM/437acc,raw312,bus11462. Both off/CRC/identity verified.
QD85-v2 boundary431 step4 rejects at0,1,11; last read rejection is now directly
observed, not inferred. Late rejections also in passing run; presence mask
does not prove count/order/cause/preemption. No retries/guard relaxation.
PERSISTENCE_REJECTION_PROBE.md E517-518 has artifacts and next investigation.

E515-516 rejection-position probe integrated behind bench-qualification-reject,
QD85-v2 same11words with12-bit index-presence mask; strict host parser and
--qualification-reject flag.227lib+34otherRust/422PythonPASS. Actual/root
61402B05D9B5C25E5C99F69005994F054961D800CEAA296945E97BF8FE708C42 installed/OFF,
NOT powered-qualified. Guard3/18,240semantics/v2wrapped+partialwirePASS.
Reject callback136cycles E515, READ-first reorder140 E516: FAIL128; accepted236
passes256. No motor/fullpreflight. PERSISTENCE_REJECTION_PROBE E515/516 hashes.
Next emitted callback/state cost audit, no gate relaxation or powered use.

E514 shared minz-core Recorder default-noop persistence_rejected(index) seam
added at existing mismatch return.85reference+224/34binz RustPASS; tests every
read0..11 and no extra reads/accept actions. Binz does NOT override yet.
RootF21FBE7EAECD1737B0466108217BCF1B4071276517E739815C3B0B90BFDDECE9 built
NOTflashed; all five PT_LOAD segments identical to frozenBC876. ActualBC876
last verified OFF; noUART/motor. PERSISTENCE_REJECTION_PROBE.md E514 proof and
next opt-in12bit rejected-read presence mask, versionedwire/cost requirements.

E513 offline source/reference audit +same-sector join, no hardware/build.
ActualBC876 remains last verified OFF. Open visits clear EXTI before reference
persistence; raw edge selector configures one edge, not both. Filter12 matches
reference clamped map atavg~977, not a divergence. 9us last-open-to-reference
includes hook/clear/mask/CS work, NOT persistence duration. Reference targeted
tests4+3+1 PASS;421PythonPASS. E512 boundary873 vs867: +6open,-1closed,
first-open-20.5us,last-open+111us,reference+111us. Added same-sector_changes
strict six-ID comparison. Design E513 source evidence and limits. Next isolate
actual persistence rejection/read timing; don't shorten filter from9us claim
or revive closed-gate pending-storm explanation without new evidence.

E512 sameBC876/OFF: one70/10s diagnostic FAILED CycleTiming after439797us,
step2 guard2810<2858,880COM/879accepted,raw304,bus11343,stack2200. No retries.
QD85 epoch248/final879+ADC3200/off validate. Earlier boundary873 step2 has
11open vs prior same-sector867's5: first-open266.5us vs287 (not late),
last-open569vs458us; final read-to-reference9us BOTH. Long578us interval
followed404.5us at874 with5open. Actual qualification extends before final
successful gate read; not a late first gate or long post-gate accept in this
pair. No physical-edge/noise/preemption proof. drv_direct_intervals.py joins
strict IDs/sectors;4tests,all420PythonPASS. Design E512 exact capture/hash.
Next source/timing investigation of extra qualification visits at boundary873,
not another70retry, duty/guard increase, or final shutdown-call speculation.

E511 actual/rootBC876BF0919EDC8B3FF00019A8D44DB8D3DE7142EB72C84151103EB19689B630
installed/OFF; frozen reference/direct_bc876/shell-pwm.elf. Const-mode disabled
measurement removes harness dispatch from bracket, raw74/88/236/60/11cycles
PASS128/256 WITHOUT subtraction. OuterTIM17 still5us (extra observer work),
legacy gate stays failed; explicit --cycle-gate uses original2/4us in rawcycles.
Guard/fullfive3200 CPU2/6/9/ADCroute PASS. All416 PythonPASS. One69/30s recovery
PASS333.827eHz sigma24.09us IRQ53.907%,56066COM/56065accepted,raw347,bus10937,
stack2200. DIRECTBIND246/QD85 final16+partial validates. Alloff/UARTclosed.
Design E511 exact proof/limitations. Next one diagnostic70hold at prior fault
point with --qualification-direct, not speed/current guard increase or retries
until pass. Diagnostic costs ~3.44IRQ percentage points vs old E488 (not paired).

E510 actual/rootBBC24B6D0C96C8AB6F7D63843506836CD2532B3A493FDE4EFC26D4E618B6E0F6
installed/OFF, NOT powered-qualified. Accepted checks READ first (implies
!frozen); open>0 proves dispatched>0, removing redundant hot checks. Frozen
partial still not invalidated. 224lib+34other RustPASS; guard3/18 and240
semantics/QD85wirePASS. Raw118/134/264/80/50cycles: acceptance saves13 but
open/accept still6/8cycles over128/256. No motor/fullpreflight. Design E510.

E509 actual/root87E0DC493662A6764DE9FCC52DFC70A23CEA7813B9B3C9E47DCE7F04FC020110
installed/OFF, NOT powered-qualified. Counter unifies ACTIVE/seen into
inactive/unread/read state; exit revokes acceptance too. 224lib+34other Rust
PASS, guard3/18,240semantics/QD85wirePASS. Worst118/134/277/80/50cycles:
open/accepted stillFAIL128/256. No motor/fullpreflight. Design E509 hashes.
Next acceptance happy-path redundant checks: READ implies !frozen; open>0
implies dispatched>0. Preserve frozen-vs-invalid refusal semantics if changed.

E508 actual/root2EB49AB91E5FA3EEF58207E14D8A5D50F3A84B7DA179B51E24BE39FC03B6A635
installed/OFF, NOT powered-qualified. E507 inline begin saves cycles; E508
full-u32 half-threshold removes clamp exactly (223lib+34other RustPASS).
Guard3/18,240semantics/QD85wirePASS. Raw worst121/134/281/79/61cycles;
closed now<128 but open/accepted still FAIL128/256. No motor/fullpreflight.
Design E507/E508 hashes/tables. No gate/subtraction change. Begin inline,
first_count inline, accepted RAM. Remaining open6/accepted25cycles over.

E506 actual/root7BB98B24F73D512698A53E88F9523B66EB41F6005A01F6F43CD4ABD122CD9BA9
installed/OFF, NOT powered-qualified. Read-only existing SysTick adds raw cycle
bracket inside disabled TIM17 check. Closed146/open160/accepted299cycles max
=2.28125/2.5/4.671875us: FAIL128/256cycles too, not merely rounding. No overhead
subtraction or gate change. Guard3/18,240semantics/QD85wirePASS; no motor.
Design E506 hashes, scope and outer-bracket observer overhead. Next use cycle
evidence rather than more one-us-granularity optimization guesses.

E505 actual/root9001C2B57F8A9737166302E485DA124971761CD8D9BDD81EF1684C85C66431BF
installed/OFF, NOT powered-qualified. Counter stores three native packedwords;
accepted helper224->200B, begin88->92B. 222lib+34other RustPASS, guard3/18,
240semantics/QD85wirePASS. Costs3/3/5/2/2us all preloads STILL FAIL2/4:
no measured timing gain. No motor/fullpreflight. Design E505 exact hashes.
E504 binding now installed but not exercised by standalone directcheck.

E504 stages DIRECTBIND final-observation provenance +strict campaign decoder
and --qualification-direct fixture opt-in. All414 PythonPASS; release-s/
thin-LTO root0BDAC79211D624D73066C4F070C65FC0909BB2C8BF881CE62837DADDFBC6D3D0
built/audit SHA matches, NOT flashed. ActualD31F remains last verified OFF,
unqualified. No UART/motor. Design E504 explains harness conservatism, not a
cost waiver. New binding is not hardware-verified; gates2/4 remain unchanged.

E503 actual/rootD31FAB7A4B4FA09ED4B3F929CC92FCF8479DE70FE49EAF121C580EDF2D92C707
installed/OFF, NOT powered-qualified. First-read seen latch now closes on
dispatch exit/freeze/rejection; redundant ACTIVE/frozen checks removed from
first-read path. 221lib+34other RustPASS, guard3/18,240semantics/QD85wirePASS.
Costs3/3/5/2/2us all preloads STILL FAIL2/4 gates. No measured worst-case gain,
no motor/fullpreflight. QUALIFICATION_DIRECT_DESIGN E503 hashes/details.
Do not repeat minor collector variants without a new cost-based hypothesis.

E502 actual/root8ED4A603F9B60CD606078D74CFC45EDED234EEFA6558AB7345E59ECC23935E5C
installed/OFF, NOT powered-qualified. Four-u32 retained rows keep QD85 unchanged;
Guard3/18,240semantics/wrappedwirePASS. Costs ordinarymax3us/publication5us
stillFAIL2/4 gates. No motor/fullpreflight. QUALIFICATION_DIRECT_DESIGN E502
hashes/table. First_count inline, begin/accepted RAM. Do not round away
one-tick failures or treat smaller code/correct wire as powered qualification.

E501 actual/rootEB6562FCC02A812834E1B8D4BCE49436C9F6BAB4B09CFC51A77801D747C9F700
installed/OFF, rejected. Out-of-line RAM first_count76B regressed ordinary3->4us;
costs4/4/6/2/3us all preloads FAIL2/4. Guard3/18,240semantics/QD85wirePASS.
No motor. Source first_count reverted inline WITHOUT rebuild/reflash; source
E500-style, actual/rootEB65 differ. QUALIFICATION_DIRECT_DESIGN E501 evidence.
Next collection/packing cost, not repeating rejected RAM-first-read placement.

E500 actual/root06AB830F4849E49A4BFD26D07E8C41D9051497573D10C74B44BADEAA03E72CEF
installed/OFF, NOT powered-qualified. bench-direct-ram moves begin88/accept216B
to RAM. Guard3/18 and240 semantics/QD85wirePASS; publication7->5..6us, ordinary
still3us: FAIL2/4 gates. No motor/fullpreflight. .data1440 includes RAM code,
bss29608. QUALIFICATION_DIRECT_DESIGN E500 hashes/details. Next remaining
first-count/full-bracket cost, no timing-gate or motor-guard relaxation.

E499 actual/root74E824C4BCB9885FFDA82D2FAD896D509004F5C2ACA297A42BEF174C225377A0
installed/OFF, NOT powered-qualified. Guard3/18PASS; directcheck240semantics
and real QD85 wrapped/partial wire PASS. Costs3/3/7/2/2us at0/16/32preload
stillFAIL2/4 gates. No motor/fullpreflight. QUALIFICATION_DIRECT_DESIGN E499
hashes/details. Next direct-hook emitted-cost optimization; no raised gates.

E498 strict QD85 decoder +four tests, all412 PythonPASS. Epoch/accepted binding
optional and explicit; CRC/identity/sector/count/partial invariants checked.
Partial open observations do not imply acceptance or persistence rejection.
No hardware wire/timing proof. QUALIFICATION_DIRECT_DESIGN E498 details.
No build/flash/UART; root4E1C staged, actual95FB remains last safed/unqualified.
Next disabled actual-hook cost and wire harness, before powered fixture use.

E497 direct collector live hooks staged behind bench-qualification-direct;
no comp-paths/QE85 coexistence.16-row packed history, QD85-v1 and partial until
unwind; acceptance hook after guard/ownership veto before ACCEPTS increment.
Release root4E1CB559292D624D018589A356236796A6F5571223DAF99A84BCFEB9E8FA67C0
built NOT flashed, actual95FB remains last verified OFF/unqualified. No UART.
QUALIFICATION_DIRECT_DESIGN E497 details. Next strict decoder and disabled
actual-hook cost/wire tests, codegen/stack/fullpreflight before any motor.

E496 pure qualification_counts direct counter, four tests;220lib+34RustPASS.
Counts dispatched/first actual gate-open/closed, not QE85 outcomes. <=16byte
hoststate; checked total bounds bins. QUALIFICATION_DIRECT_DESIGN.md live
ownership/threshold/commit/wire/cost contract. NOT live integrated or timed.
No build/UART/motor; actual/root95FB remains last verified OFF and unqualified.

E495 actual/root95FBAD72999201F49B0AA991FD7F8A9BD5651EF4E567B6253CFA172C889C2E44
installed/OFF, still NOT powered-qualified. Split accumulation/publication:
visit160B/localframe4B, but ordinarymax6us and accepted13us stillFAIL2/4.
Guard3/18 and288 Scope semantics PASS;216lib+34RustPASS. No motor.
QUALIFICATION_EVENT_DESIGN E495 evidence. Next direct first-read/acceptance
hook design rather than more Scope/general-purpose accumulator tuning; changed
quantities require explicit new semantics/wire, no guard relaxation.

E494 actual/root7BF3399998E92A6CEACE4187B3686BC9D2E5762ED9F2D00A212153DCE4809E2D
installed/OFF, still NOT powered-qualified. History repr(C) hot-metadata-first
cuts visit468->368bytes and about1us. Guard3/18 PASS; Scope semantics288PASS,
costs6/6..7/12/10/9/6us stillFAIL2/4 gates. No motor. 216lib+34RustPASS.
QUALIFICATION_EVENT_DESIGN E494 evidence. Next simplify frequent collection,
not more minor ring layout variants or timing-gate relaxation.

E493 actual/root43182426F358632BD405A9A082CACE97C3870E283194B47587F3960F4B68CD01
installed/OFF, NOT powered-qualified. Guard3/18 PASS after OpenOCD reset,
no UART repair. qeventcheck 18x16 semantics PASS, costs7/7/13/11/10/7us
(wrapped open8) FAIL fixed2/4us gates. No motor/fullpreflight after failure.
QUALIFICATION_EVENT_DESIGN E493 exact hashes/table. Host verifier preserves
failure. Next reduce frequent-path Scope/accumulator and publication cost;
not ring-size tweaks, dump cadence, or raised gates. QE85 wire still unverified.

E492 strict QE85 host decoder added, four tests; all406 Python PASS.
Checks CRC/epoch/identity/sector/partial/final-count closure. Unknown stop may
follow an actual ACCEPTS increment; never infer persistence rejection from it.
External epoch/count validation optional and explicitly reported; substream
is not whole-campaign/off/lock proof. QUALIFICATION_EVENT_DESIGN E492 details.
No build/flash/UART; root6369 unflashed, actual C631 remains last safed.
Next disabled actual-Scope timing/semantic harness then wire/fixture binding.

E491 staged bench-qualification-event live adapter +16 accepted-row suffix,
epoch/final-ACCEPTS/partial framing. 215 library+34 other Rust tests PASS.
Release root63698283C0E82CDF3EE95F48C09723FDC2D0B7DD667770FDF93C2A460775C2E8
built NOT flashed; actual C631 remains last verified OFF. Scope stack68bytes,
runtime overhead unknown. QUALIFICATION_EVENT_DESIGN E491 exact scope/gates.
Next strict decoder and disabled real-Scope cost/wire checks; no powered use
until <=2/4us added-cost and unchanged <=10us CPU/full preflights pass.
No new comparator read, only existing first-count hook; dispatched-only,
final epoch only, not physical-edge delay or causal proof.

E490 pure qualification_event accumulator added (host replay only), five tests;
213 library+34 other Rust tests PASS. Per-accepted path counters distinguish
unknown stop, overflow/gap invalidation and partial bucket; no hardware reads.
QUALIFICATION_EVENT_DESIGN.md integration/epoch/skip/stop and cost contract.
NOT live-connected or target-timed; no firmware build/flash/motor this entry.
Actual C631 remains last verified OFF. Next bounded live adapter and emitted/
disabled cost validation, preserving static comparator path and all guards.

E489 offline pair comparison: passing E485 recovery tail also has +56.25/-64.25us
adjacent reference-interval residuals. Pair existence alone is not fault-specific.
E486 largest pair identity393/step2 is preceding visit to later-refused sector.
drv_boundary_pairs.py validates CRC/off and ranks retained same-sector-baselined
pairs without bridging gaps; four tests. BOUNDARY_PAIR_COMPARISON.md six-capture
table and limitations. No motor/build/flash; actual C631/off unchanged. Next
per-accepted qualification history design, not final-shutdown tracing, and no
guard relaxation or causal claim from selected short tails.

E488 supersedes E487 installation status: actual/root C6311D58B44A797252C5F89479A74EFA95FF947252F6999478389E30A884C5B2, outputs OFF.
Masked-seed candidate now has fixture provenance and disabled register check:
3 trials x 6 sectors pass, pending request injected, enable primitive tested
without gate authority (NOT a synthetic COM ISR test). Guard 3/18, full five
3200-tick preflights, CPU 2/6/9 us and ADC route pass; 398 Python and 242 Rust
tests pass. Release-s/thin-LTO emitted audit SHA matches; masked specialization
retains software latch and only unmasks TIM16. Refusal cleanup source reviewed.
One matched 69/30s dropout/reentry PASS: 333.791 eHz, sigma 21.938 us,
56058 COM/56057 accepted, IRQ 50.467%, raw peak350, busmin10781, stack2668.
Measured seed age83 us versus baseline85; arm body7 us unchanged. Single-run
gain, not WCET or CycleTiming fix. Original deadline/ADC3200/finaloff verified.
MASKED_SEED_ARM_EXPERIMENT.md E488 has provenance and limitations. Next validate
the small setup gain separately from running-cycle redistribution; no duty or
guard increase justified by this result. Current calibration/parity remain open.

E487 stagedbench-masked-seed-arm removes enable/remask insidegloballymasked
freshseedsetup; explicitsoftwareMASKED=true retainedalonghardwaremask.
ReleaseBEA2B6A9ABD9362F02D101ACDF28253C57C28668575185727CC14DFA5BB616E5
built/rootNOTflashed; actual9ACC/OFF unchanged. No measuredgain or CycleTimingfix.
MASKED_SEED_ARM_EXPERIMENT.md sourcefinding/safetycontract. Nextprovenance+
disabledactualmask/pending/bootstrap checks+codegen beforeflash/power; unchanged
85usage/32usfloor/350profile. Initial2779 unflashedcandidate correctedlatch.

E486 same9ACC/OFF: first70holdFAILED203236us CycleTimingstep2 delta2844<2858,
400COM/399accepted,raw297bus11354,stack2668,ADC3200/finaloffPASS. No retries.
Localtail341.763eHz; old24k7F4B73fault342.818eHz,also2844delta/~0.2s.
Carrier changesduty-speed/workload, no demonstratedspeed-envelopeextension.
CARRIER20_EXPERIMENT E486hashes/paircompare. Root6D02unchanged. Nextaccepted
timing/freshseedarmbudget/referenceaudit, notmorecarrier/duty ratchet or guard
raise tofitfailure.69recovery3/3remainsvalidregime-specific; calibrationopen.

E485 same9ACC20k/OFF now69/30srecovery3/3PASS333.65..334.19eHz,
sigma22.30..22.71us IRQ50.50..50.88%,raw311..314 busmin10889,stack2668,
armspare>=9.5us. AllCOMP40COM45commit21/ADC3200/finaloff/archive/timelinePASS.
No excludedtries. AskedasyncPSUcurrent/voltage,noanswerasofentry; don'tblock
orassumecalibration. CARRIER20_EXPERIMENT E485hashes. Root6D02unchanged.
Nextone70/10shold unchanged350/currentguards; no70recoveryuntilholdpasses.

E484 actual9ACC20k/OFF,root6D0224k. Restoreguard/fullfive3200CPU2/6/9/
ADCroutePASS. Fixed68recoverycohort now3/3 inclE483:02/03PASS330.174/328.954eHz,
sigma22.41/22.87,IRQ50.40/50.23,raw301/310bus11032/11056stack2668spare10.5/9.5.
Thenone69hold10sPASS335.061eHz/IRQ50.656raw283bus10984,COMP40COM59commit21.
AllADC3200validated/finaloff/UARTclosed. CARRIER20_EXPERIMENT.md E484hashes.
Nextone69/30srecovery unchangedguards; no70yet. No calibratedcurrent/rare-faultfix
claim; stack69cumulativepaint fromearlierrecovery. No failuresexcluded.

E483 actual/root6D02lean24k/OFF. Both68/30s dropoutreentryPASS(oneeach):20k
330.757eHz/sigma21.996/IRQ50.656%,24k315.751/23.632/52.713%; raw352/265,
bus10925both,stack2668,armspare11/17us.20kADC3200/24k2666validated.24restore
freshguard/fullfiveCPU2/6/9/ADCroutePASS. Allfinaloff/UARTclosed, no failures.
CARRIER20_EXPERIMENT.md E483 hashes andscope. Next restore9ACC+preflights,
exactlytwo68/30s recovery repeats failfast tocompletefixed3attemptcohort
before69step. Equal-duty comparisonnotmatchedspeed/rare-faultfix/currentparity.

E482 actual9ACC20k installed/OFF; root6D02 lean24k. BOTHexactimages guard/
fullfiveCPU2/6/9/ADCroutePASS thenone68/10s holdPASS.24k315.428eHz/IRQ52.724%,
20k330.282/IRQ50.426%, sigma39.18/38.95 includesacceleration, raw294/314,
bus10984/11068,stack3460. Capturecarrier{24,20}_482_start61_hold68_10s_01.
ADC20k bins995..1991 reflects50phase grid across32bins, notuniformityproof.
No reliability/jittercausalclaim. CARRIER20_EXPERIMENT.md hashes/details.
Next68/30s originaldeadline dropout/reentry matched comparison; no72jump.

E481 integratedbench-pwm-20k override +ADCselectedgeometry+stricthost20000/3200.
396PythonPASS. Lean20k9ACCA8CEBF9A975D50E28F75D4312CEC99B1D37A09A17235E36E34BF34097BCA
and24k6D026D5659AEC6746C14454509C0FCD880CCAF0BC09478AB14A231CB0E6B1B47
releasebuilt/frozenreference/carrier20_9acc andcarrier24_6d02; root6D02.
NOflash/motor; actualF22D/off unchanged.20kbin emittedconst10486/MULS/LSR20.
Next24kbaseline disabledguard/fullfive2666/ADCphase then68hold10s; candidate
needs own3200preflights. CARRIER20_EXPERIMENT.md details/features/confounders.

E480 pureCarrierKhz20 geometry staged3200ticks; exhaustivephase/duty/continuity
testsPASS208lib+34Rust. NOTlive selectable/no firmwarebuild/flash/motor.
ActualF22D/off unchanged. CARRIER20_EXPERIMENT.md next intervention: matched
lean24k/20k carrier A/B at preceding68 before72; notmore costlydecisiontrace.
20k deadtimefraction changes effectivepulse too, so no unique causal claim.
Next feature/ADCgeometry/hoststrictperiod integration, emittedaudit anddisabled
preflights beforepower. Preserve10kstartup andallguards; no speedcap widening.

E479 offline boundary join: fault reference compares earlier same-sector visit.
E476 prioridentity1909 step2 long589/next417.5us vs485.5/511median =>+103.5/-93.5.
E478 identity3475 step5 long629/next363 vs509.25/481=>+119.75/-118us.
BOTH earlierboundaries have no selected sparsecall with complete identity
coverage (E478 olderomissions precede retained3431). Thus no >40us dispatch
at those accepted identities; not a100us in-call stall. Preentry/delayed
qualification across shortcalls stillpossible. drv_sparse_fault.py4testsPASS.
ActualF22D/off unchanged; no motor/build. Next inspect qualification sequence
around earlier boundary, not finalshutdowncall; designmemoE479.

E478 actualF22D53B01247D5F2D4257E1FCF62E379D1386BDA547B8AF954B1A1F69DDF74AC
installed/OFF. Added limiter starvation counterexample test;208lib+34RustPASS.
Enabled EXISTING comp-critical on E476 sparse profile. Exactguard/sparse/wrapper/
fullfive preflightsPASS CPU2/6/9. One72/30s reentry FAILED1.724832s resumed,
CycleTimingstep5 delta2856. Protected31060calls max45us/refused0. Final sparse
51us/NOguardoverlap; reference persistencepassed. Thus midservice preemption
notnecessarycause at thispoint; do notrepeat broader masks/priorityflip.
16rows/1105selectedomissions: wrapper perturbs selection workload, not fair
timingperformance comparison. Raw qualsparse_critical478_start61_reentry72_30s_01.
Baseline0F9F frozen reference/qualsparse_0f9f. Next preentry/qualification-time
acceptance redistribution; COMP_CRITICAL_EXPERIMENT.md E478 full details.

E477 offline source/capture join: final E476 sparse accepted=false is NOT a
persistence rejection. CYCLECORE proves EV_ACC reached guard; reference has
passed persistence/resetTIM2/armedCOM before guard refuses/log increment veto.
57us includes safing, but guard delta2850 sampled BEFORE safing: shutdown cannot
cause that measured refusal. drv_sparse_fault.py validates join,3testsPASS.
Actual0F9F/off unchanged, no motor/build. Next investigate pre-guard acceptance
timing/priority experiment slack, not subtract assumed shutdown cost or treat
normal unrecorded calls as clean. QUALIFICATION_WINDOW_DESIGN.md E477.

E476 actual0F9F8A9F6E5BB021045A91AD864762BCBC8E937AF1134C79DA99ABC692819AFD
installed/OFF. Sparse final-epoch/count binding + fixture opt-in;390PythonPASS.
Exact-build guard/sparse-wire/fullfive preflightsPASS CPU2/6/9. One72/30s
recovery diagnostic FAILED CycleTiming after947550us resumed, step2 delta2850.
Sparse epoch134 valid3rows/noomissions: two41usaccepted guard-overlap calls,
final57us nonaccepted stopped guard-overlap call. Finalcall includes safing;
not preemption proof. Pair+131.5/-123.5us nearly cancels; closure11.5us.
Capture qualsparse_epoch476_start61_reentry72_30s_01.txt retained, finaloff.
Next isolate fault-call timing/guard overlap from shutdown cost and compare
reference chronology; no duty/guard increase or repeat-until-pass. Designmemo.

E475 actualBDE2C4CE4F9C1E6A0A08C637DED65594E63490A49C3AC1D2C01A3619BF56D830
installed/OFF. CPUdisabledcheck noninline976B; accountinglogic unchanged.
guard3/18PASS, sparse8modes2/4/4/4/2/4/4/4+wirePASS; fullfivepreflightsPASS
CPU2/6/9us. No motor. Improved disabledcalllayoutNOTliveIRQoccupancygain.
Next poweredfixture sparse opt-in+finalepoch binding, then exactbuildpreflights
and bounded72diagnosis. Priorfailedbuildsremainfailed; designmemo E475.

E474 actual2CA571BA841DF48D8991ECBB7328BE296380F4EA7CF2765D39254244014DC072
installed/OFF. Noninline sparsecheck emitted1432B; ScopeRAM200B unchangedsize.
sparsecheck8modesPASS2/4/4/4/2/4/4/4 ANDhardwarewirePASS. guardPASS3/18.
BUT broaderpreflightCPU2/7/11 FAIL10us gate; archivedroutingstepnotrun.
No motor, finaloffverified. Do not transfer766DCPU pass. Next nestedCPU
accounting/code-layout investigation, preserve sparse+wire successes separately.

E473 actualF33FF7D4901B2D36878654FD9F4A76AB5F0A305F300B236F9B02BFE13B537911
installed/OFF. Extended sparsecheck emits20accepts+shortstop throughactualScope
and QS85; hardwaredecoder verifies16exactrows/5omitted/epoch/finalstop PASS.
BUTcost regressed3/4/5/4/2/4/5/4us FAILunchangedgates. No poweredqualification.
guardPASS3/18 OpenOCDreset withoutrepair, finaloff.4hostverifiertestsPASS.
Next isolate disabled check/report function from huge shellmain inlining;
do not treat wire success or previous766D timing as thisbuildpassing.

E472 metadata-derived ringcount AAF2 stillselected5usFAIL. Added opt-in
bench-sparse-ram places onlyScopefinish200B in.data startupcopy (VMA20000000).
Actual766D148B62E3E2C4B0D5BBFACDE3B59E3759D114543162172496B0370DDB60C5
installed/OFF: sparse8modesPASS2/4/4/4/2/4/4/4; full5preflightsPASSCPU2/7/10.
OpenOCDreset noUARTrepair, guardPASS3/18. No motor. .data1528RAMbytes,
bss29344; sizeclassifies executable.data astext, don'treportzeroRAMinit.
Next strictcapture/epoch fixture binding and hardwareQS85 framing beforepower.
QUALIFICATION_WINDOW_DESIGN.md E472 fulltwohashes and remainingqualification.

E471 inline Tailpush improvesselected6->5us (9451hash in designmemo), stillFAIL.
Packedwordstorage candidateEAB5B314FA028CEBA63174B04E56AFE9854B31476CF04EFA61EF76D1D851A6BA
thenworse2/6/6/6/2/6/7/6us; sourcepacking reverted, inlinepush retained.
Actual/rootEAB5/off rejected; sourcebacktoinlineform. BothguardPASS3/18 after
OpenOCDreset, noUARTrepair, finaloffverified, no motor. Do notretrypacking.
QUALIFICATION_WINDOW_DESIGN.md E471 fullhashes/tests/evidence.

E470 exactprobe-rs3c10cd38 source confirms G0 debug_core_stop wholeAPBENR1
RMWclearingbit27; concurrentUARTenable can be lost (failedpayloadnotcaptured).
OpenOCD resetalternative fixed3/3 guardPASS withoutrepair, unchangedA3BD/off.
UART_RESET_WORKFLOW.md exactcommand/source/evidence. Preferthis tested reset
afterverifiedoff; serialize hardware. No flash/motor. Return to sparsecost,
no further blindHALclockpatches. All prior failures remain retained.

E469 emitted A3BD postHAL->postADC interval has no APBENR1 write. Trace reset
captures/uart_reset_trace01.log shows probe-rs session_drop/debug_core_stop
READ then WRITE APBENR1 afterrun; payload value notlogged, failingraceNOTproven.
Loggedreset guard01PASS3/18 withoutrepair, offverified. No flash/motor/codeedit.
ActualA3BD/off unchanged. Next inspect debuggersequence/source or controlled
reset ordering; do not blame HAL or blindly re-enable in main. Designmemo E469.

E468 UARTbootbreadcrumbs A3BDD94C35D8D9678A891649E132EC5A2787FEB9C1FB90F7F16A6AB550FC9F14
installed/OFF,126656/1336/29344. UART_BOOT_CLOCKS at200003d4 forTHIS ELF:
preHAL/postHAL/postADC/postbanner =00040000/00040000/0/0 onfailedguard01.
Snapshot then documentedclockrepair, guard02PASS3/18. No motor; sparsecost
stillunqualified. No APBENR1 sourcewrite betweenHALandpostADC found; distinguish
reset/debuginteraction from MCU init, don'tclaim culprit. Fourboot-only reads/
16RAMbytes, no poweredloopwork. Root/actualA3BD. Designmemo E468 evidence.

E467 precomputed opaque synthetic inputs outside timed bracket; shared
noninline Scopefinish. Actual2D94BAA305F58716B5F5F42812AD1DCA82D0B242CD3937D23684FCAE68E9EB5D
installed/OFF NOTqualified. guard02PASS3/18 afterretainedUARTsnapshot/repair.
scope01 still2/6/6/6/2/6/6/6us rejects unchangedlimits. No motor. Baseline
reported separately, never subtracted.3verifiertestsPASS. No claimed gain;
stop harness-only tweaks; selectedpublication remains the unresolved cost.

E466 sparsecheck8modes nowincludes1/16/32preloadedrows outside timing,
checkslen/omission/finalrow.3hostverifiertestsPASS. Installed1ECC1D4A7CB3E578
510F25B1EB978D348700DFCF41B3AAB31BF24D4D02C83CF1/OFF NOTqualified.
Release126360/1320/29344. UARTclocksnapshot+repair then guard02PASS3/18.
scope01 semantic8x16PASS but2/6/6/6/2/6/6/6us FAIL2/4/4/4/4/4/4/4.
Expanded harness branches are included,5->6notproofpublicationregression.
No motor. Next separate measured harness overhead fromactualScope cost;
no subtract-assumed baseline or gate relaxation. Designmemo E466.

E465 cached accepted-order bound instead of previous-row lookup.207lib+34Rust
PASS.60C970F3BB651E307CC73EAA6E275A2E62C4ADF2C9D6465CFAE15E6B0DAF987F
installed/OFF,125836/1320/29344. guard01PASS3/18, scope01 still2/5/5/5/2us
FAIL2/4/4/4/4. No measured gain; no motor. Current sparsecheck resets before
each call, so it times EMPTY ring only: extend to populated/wrapped cases
before claiming selected-path coverage. E465 new bound affects that unmeasured
case; cold case remains refusal. No guard/gate raised. Designmemo details.

E464 sparse reset now metadata-only (old192byte storage hidden bylen0), avoids
clearing ring afterfreshseed.207lib+34RustPASS; emitted observation_reset uses
scalarstores, no ringclear. Epochcounter exhaustion sticky, Tail invalid0.
Release9343EA66DBC504308C43AD4FF260428C2C899B265C90C3ECA653280BA2CBBED6
125924/1316/29344 NOTflashed. ActualC17C/off stillselected5us rejected.
Sparse only final observationepoch survives recovery; FIRST_SEGMENT archive
does NOTinclude sparse tail. Do notclaim first/resumed sparse coverage.
No hardware. Selectedcost5-><=4 and fullCPU/epoch capture binding remain.

E463 strict sparse decoder+five tests, all387PythonPASS. QS85 now10u16:
CRC-protected epoch prepended to8rowwords; old8word form rejected. Empty
sparse capture allowed, never no-fault proof. External expected_epoch optional
and its validation status explicit; not fullcampaign/off/lock verification.
Release7E6FFAA0427B0BAFDC18094921C3C31491C3305EBCA6B5C89F7C75E98F54B5BE
125876/1316/29344 built NOTflashed; actualC17C/off remains rejected5usselected.
No hardware thisentry. Next selectedcost+epoch reset/callback audit beforepower.

E462 sparse live C17C5B7496166478DEB1D55C81FCD36B3BE5237A20753A557D2188F70A6B8776
installed/OFF NOTqualified. Release125900/1316/29344. guard01PASS3/18 without
UARTrepair; qualsparse_scope01 semantics5x16PASS, costs2/5/5/5/2us FAIL2/4/4/4/4.
Normal path meets gate; selected rows1us too expensive. No motor. Separate
QS85 format, epoch reset masked, publication included in COMPmax; strict data
decoder/epoch integration audit and fullCPU preflights still missing. Two host
disabled-verifier testsPASS. Next selected-publication codegen, not gate raise.

E461 pure qualification_sparse Tail added, five tests;206lib+34otherRustPASS.
Trigger>40us or stopped backed by matchedACEE COMPmax40/40/48 (notcausality).
16selectedrows, epoch/count validation, omissions, shortfinalstop preserved;
normalcalls absent, not counted as good. NOTliveintegrated/hardwaretimed.
QUALIFICATION_WINDOW_DESIGN.md contract: reuse dispatchbracket,2usnormal/
4usselected incremental gates, quietdistinctCRC/epoch required beforepower.
No build/flash/motor; actual/root55BA/off remains rejected diagnostic.

E460 rejected scalar-overlap/outlined-accumulation candidate55BA2CB7CB431BF2
8414EBA88E11CA7479B15B27CAB34E034B4B1644F101FB02 installed/OFF, NOTqualified.
Source experiment reverted to E459 form; root ELF remains rejected55BA.
qualscalar_scope01 semantic5x16 PASS but7/5/7/7/2us FAIL4/2/4/4/4 (worse).
UART clock-off snapshot retained before repair, guard02PASS3/18, finaloff.
No motor. Next sparse slow-call diagnostic, not more all-call accumulator
layout tweaks. Specify lost coverage and normal/worst cost before implementation.

E459 F686 flash contents verified with probe-rs verify; installed probe remains
NOT power-qualified. Interrupted qualsplit_guard01 contains only FINALOFF,
not an off proof. Read-only qualsplit_boot_snapshot01 retained UART clock-off
with ENABLE0/MOE0/CCRs0; documented clock repair then guard02 PASS3/18.
qualsplit_scope01 semantics5x16 PASS, costs6/4/6/6/1us FAIL unchanged4/2/4/4/4.
Final outputs-off readback verified, UART closed, no motor run. Publication
split saves1us on this disabled frequent-path check, still twice its limit.
Next cheaper/sparse qualification evidence; no powered probe or gate widening.

E458 stagedF6862A6DE02B4F01AF282A43F3FD2BBE7BC04D303C4F505A2D2B17E0D6F4A92B
Historypublication split noinline, semanticsunchanged. Scopefinish328->220B,
localframe52->36, notmeasuredtimingpass. Release125960/1104/29624,auditmatch,
201lib+34RustPASS. NOTFLASHED,actualB6EF/offNOTpowerqualified. Nextsame
disabledqualchecklimits4/2/4/4/4. QUALIFICATION_WINDOW_DESIGN.md details.

E457 actualB6EF installed/OFF, NOTPOWERQUALIFIED. BootUARTsnapshot+documented
clockrepair then guard02PASS3/18. qualwindow_scope01 semantic5x16PASS but
max6/5/6/6/1us FAIL fixed4/2/4/4/4limits. Finaloff/closed, NOmotorcommand.
Do notrunpoweredprobe/widenoverheadgates. Next emittedScope/History cost
optimization or cheaperinstrument. QUALIFICATION_WINDOW_DESIGN.md evidence.

E456 stagedB6EF8BF2B341512FB0D6AEE80BE23B899007B7DFEBE6A244557A8FBFD5CD95CA
release125952/1104/29624; fixture--qualification-window requires trace0 and
strictdecoder. MCUqualcheck5modesx16 actualScope built, NOTexecuted.378PythonPASS.
Next hostcheckverifier/timingbudget/codegen and disabledhardwarepreflights,
no motoruntilqualified. ActualACEE/off,rootB6EF. QUALIFICATION_WINDOW_DESIGN.md.

E455 standalonequalification decoder+4synthetic testsPASS: strictheader/CRC/
counts/ordinal/stop/pending checks; requiredquiet/unused reject. NOTfixture
integrated, notoutputs-off or epoch proof, nohardwareQW85 yet.
Next fixtureflag and disabledScope/overhead tests. QUALIFICATION_WINDOW_DESIGN.md.
No build/flash/motor;actualACEE/off,root4EFB unchanged.

E454 optionalbench-qualification-window integrated/build4EFB3DBAF9CA78F3413CA65C6DCB57ABE4F740ABD2E7CBE126A08B0EAF2701BD.
Release124976/1104/29624. NOTFLASHED, actualACEE/off. Scope measuresdispatchbody
including setup, notpurecall; poweredtrace0only. PoststopQW85/QP85capture.
QUALIFICATION_WINDOW_DESIGN.md: decoder/disabledScope+overhead/codegenstill
required beforemotor. RootELF4EFBnotinstalled; no evidence fromemptyhistory.

E453 purequalification History16rows implemented, host<=320B, lastfaultcall
retained beforefreeze, idlepending distinct, explicitomissions/invalidity.
201lib+34RustPASS. NOTshellintegrated/hardwaretimed. QUALIFICATION_WINDOW_DESIGN.md.
Next optionalhooks/framing/disabledtiming, preserve completion-beforefreeze
across safingcallback; no claim liveinstrument. ActualACEE/off unchanged.

E452 purequalification_window <=8B accumulator staged; callcount/guardsequence
overlap/maxduration/saturation, 3tests. NOTfirmwareintegrated or hardwaretimed.
QUALIFICATION_WINDOW_DESIGN.md requires finalfaultcall completion BEFOREfreeze,
acceptedidentity/precedingcycle history and disabledCPUproof. No perreadwork.
ActualACEE/off unchanged. Next boundedhistory/ownership thenoptionalhooks;
don'tclaim preentrylatency or tickcount from sequenceinequality.

E451 offlinefaultpair script+2testsPASS. Reference/inline long-shortpair nearly
cancel(localpairresidual+1.5/-8.75us), outlineddoesnot(-37.25us). Allguard/ref
closure3..3.5us: not100usguardtimestampoverhead. No preemption/rotorcauseproof.
ACCEPTED_PAIR_COMPARISON.md selectedfaultbias/sectorchanges. Next bounded
preacceptance evidence instrument, reuse costlyobserver lessons, no blind
IRQmax attribution. No hardware/runtimechange;actualACEE/off.

E450 actualACEE/off: inline72cohort2PASS/1FAIL.02PASS336.361eHz;03FAILED
24.757799s recovered336.506eHz CycleTimingstep3 2842<2858,raw290bus10972,
stack2676. Allthreeage85/arm7, so measuredarmgain repeats but notsustainedfix.
Bothfinaloff/closed,noexcludedtries. RECOVERY_TIMELINE_AUDIT.md hashes.
Next recurringacceptedcycle redistribution/step3 investigation, notmore
constructorvariants/repeattillpass/guardraise. No higherduty authorizedbyresult.

E449 actualACEE installed/off: guardPASSwithoutrepair,fivepreflightPASS2/7/10.
Inline72recoveryPASS336.409eHz,56498COM/56497acc,sigma23.718us,IRQ53.015%,
raw311bus10853,stack2676,DMA24queue2. Age85/arm7 vsreference85/13, outlined86/8.
Seed1000/spare8us;finaloff/closed. ONEpass,notsustainedfix/cohort.
RECOVERY_TIMELINE_AUDIT.md hash/details. Next exactlytwo72inline repeats
failfast, no moreduty/guardchange. Candidate7C37failure remainsretained.

E448 stagedACEEC96C19C90E1979E1A641FAF27F3D0FE40787F70F473D3AE3E45F560DF111.
InlineTimeline::new removesoutlinedsymbol; exactmath/guardsunchanged.
Release124044/1104/29336,auditmatch,195lib+34Rust/370PythonPASS. NOTFLASHED.
Actual7F4B/off; source/rootnowACEE,7C37frozen. RECOVERY_TIMELINE_AUDIT.md.
Next disabledguard/fivepreflights thenone72recovery ifPASS; compareage/arm/
stack, no73retry or inferredjitterfix. No hardwareworkthisentry.

E447 frozenELF comparison: ADC_COMP/DMA/TIM16/TIM6 addresses+sizes unchanged;
handlerbody diffs onlydirectcall relocations and TIM16source-locationpointer.
commit312B onlycallrelocation afteraddressnormalization. Noaddedbodywork shown,
NOTrecursivecallee/state/flash-timing equivalence or candidateexoneration.
RECOVERY_TIMELINE_AUDIT.md table/pointerverification. No runtimeedit/hardware;
actual7F4B/off, root/source7C37. Next isolateconstructor call/layout or bounded
matchedcomparison; don'tcall onecandidatefault provenISRregression.

E446 actualFROZEN7F4B restored/off, rootELF/source remaincandidate7C37.
BootguardPASSwithoutrepair;fivepreflightsPASSCPU2/7/10. Matched72reference
PASS336.307eHz,56481COM=acc,sigma23.907us,IRQ53.172%,raw307bus10937,
stack2676,seed1012age85/arm13/spare9.5. Allfinaloff/closed.
Timingdifference returns, but onepassdoesnotconvict candidate rarefailure.
RECOVERY_TIMELINE_AUDIT.md hashes; candidate1PASS/1FAIL remainsunqualified.
Next emittedsteady-path/call-layout comparison, not repeattillpass/guardraise.

E445 actual7C37/off: candidate72repeat02FAILED24.287464s recovered336.414eHz,
CycleTimingstep3 delta2854<2858,49024COM/49023acc,raw300bus10948,stack2644.
Age86/arm8 repeats (seed1000/spare7us), notarmfailure. Finaloff/closed, third
cancelled; cohort1PASS/1FAIL. RECOVERY_TIMELINE_AUDIT.md hash/details.
Next archivecandidate then frozen7F4B matched72reference/preflights, no guards
widened; don't infer patch/hardwarecause from onefailure. Goal incomplete.

E444 actual7C37 installed/off. UARTbootfault snapshot retained then documented
clockrepair;guard02PASS3/18,fivepreflightPASSCPU2/7/10. Matched72recoveryPASS
336.395eHz,56496COM=acc,sigma23.885us,IRQ53.531%,raw303bus10877,stack2644.
Age86us/arm8us vs7F4B85/13: singleobservedcombined98->94us,notWCET/cohort.
Seed1013/spare8.5us,DMA24queue2,allfinaloff/closed. RECOVERY_TIMELINE_AUDIT.md.
Next exactlytwo72candidate repeats failfast; no73retry/guardchange yet.

E443 staged7C3724E846DBF562E734485D6FC158080D6EF67432A9391AE99BD6C6E10DFB48
timeline exact32bit radix4096 /6.195lib+34other Rust/367PythonPASS.
Release123892/1104/29336,auditmatch; constructor nohelper but outlined52Bframe,
notmeasuredgain. RECOVERY_TIMELINE_AUDIT.md proof/caveats. NOTFLASHED; actual
7F4B/off. Next disabledguard/fivepreflight thenmatched72recovery ifPASS; compare
85usage/13usarm/stack, no73retry/guardchange basedsolelyon arithmetic.

E442 hashmatched7F4B audit found Timeline::new runtime/6 in maskedarm block
after age sample/timer programming (08006160). Constfn didnot eliminate it.
RECOVERY_TIMELINE_AUDIT.md distinguishes16usarmcost from85us edgeage: moving
work beforeage can worsen margin, not a gain. Next exactbounded constructor
optimization/tests/codegen; no runtimeedit/flash/motor. Actual7F4B/off unchanged.

E441 offline actualreference budget/test: observed85usage gives prospective
355floorseed938 wait117.5us/remaining32.5/spare0.5;356zero;357/360refuse.
Existing350 admission unchanged.194library+34other replayPASS. Noembedded
build/flash/motor;actual7F4B/off. SPEED_ARM_BUDGET.md refreshed, notspeedcap.
Next remainingpostedge acquisition/guard path or fresh-edge rendezvous audit;
carrierrelocation alreadynogain, statsalready staged, no inventedreportcopy.
Do not refreshseed/feedback timestamps or pregrantauthority.73failed/72cohort.

E440 actual7F4B/off: one73hold FAILED197217us CycleTimingstep3 delta2844<2858.
387COM/386acc,raw268bus11199,stack2676,finaloff/closed; no recovery/retry.
Aggregate327eHz includesacceleration; last15ms median2917us/~342.82eHz,
notsteadyproof. Reference5993/5681halfusticks, guardminusreference3.5us;
notphysicaloverspeed/ISRcause. RANGE350_EXPERIMENT.md. Highest3/3 remains72.
Next review local343 operation and latearm budget before any profilechange;
do not automatically ratchet guard. Calibration/parity stillincomplete.

E439 actual7F4B/off:72recovery02/03PASS; declared72cohort3/3original30s,
336.40..336.54eHz,sigma23.75..23.77us,IRQ53.02..53.20%,armspare>=7.5us,
cost13,stack2676,allfinaloff/closed. No guard/firmware changes/excludedattempts.
Reference7F4B archive now qualified72cohort too. RANGE350_EXPERIMENT.md details.
Next assess one73hold within same350profile, failfast; noautomaticguardraise.
Calibrated current/archiveparity/ultimate mechanism stillincomplete.

E438 actual7F4B/off unchanged:72hold10sPASS336.229eHz includingacceleration,
then72recovery30sPASS336.451eHz resumed27.991286s56506COM=accepted,
sigma23.754us,IRQ53.195%,raw310bus10805,seed996age85/remaining39.5/spare7.5,
cost13,stack2676,DMA24queue2. Startupraw1023/902 below1200, notamps.
Bothfinaloff/closed, noexcludedattempts. ONE72recoverypass notcohort.
Next exactlytwo72recovery repeats failfast beforehigherduty; no guardchange.
RANGE350_EXPERIMENT.md hashes/details. Calibration/parity stillincomplete.

E437 actual7F4B/off: fixed71recovery02/03 bothPASS, cohort3/3original30s
startup/injected-loss/recovery near332.20..332.23eHz. sigma22.89..22.97us,
IRQ52.93..53.15%, armspare>=9us/cost13,stack2676,allfinaloff/closed.
Frozen reference/range350_7f4b. DMAmax37us in03 (24in02), COM54in02 (45in03);
not causal/WCET proof. No more duty tried. RANGE350_EXPERIMENT.md details.
Next assess next same-profile duty step against timing/current evidence;
calibrated current, archive parity and ultimate limiting mechanism stillopen.

E436 actual7F4B installed/off.350guard01PASS WITHOUTclockrepair;fivepreflights
PASSCPU2/7/10.70recoveryPASS325.722eHz.71recovery01PASS27.991537s resumed
332.201eHz55793COM/55792acc,sigma22.968us,IRQ52.961%,raw312bus10901,
seed1007age85/remaining41us/spare9cost13,stack2676,DMA24queue2. Bothfinaloff/
closed. ONE71pass NOTcohort/jitterfix;next fixedtwo71recoveries failfast before
moreduty. RANGE350_EXPERIMENT.md hashes/details. Old34571failureunchanged.

E435 staged3507F4B25F6022C9A15DF004F59FD8CAC6394C9A3E3B208A5925A1ED099D903F9D9
NOTFLASHED.2858/238runtime,5716/476acq,952seed;wait119us,age87passes32floor,
87.5refuses. Observed85leaves2usnominalspare,NOTWCET. Allotherguards unchanged.
Release123836/1104/29336,auditmatch;364Python+227Rust350PASS,345lib192PASS.
RANGE350_EXPERIMENT.md:next disabledpreflights,70recovery,conditional71recovery,
failfast,nohigherduty. ExpansionNOTjitterfix. ActualFAA0/offE434 unchanged.

E434 actualFAA0 installed/off.345guard01UARTsilent;snapshotexactsafe then
clockrepair08040000;guard02PASS3/18,fivepreflightsPASSCPU2/7/10.
70recoveryPASS325.585eHz;71hold10sPASS331.995eHz. Then71recoveryFAILED8.245294s
resumed332.572eHz,CycleTiming2881<2899,16453COM/16452acc,sigma23.383,
raw300bus11032,stack2676,DMA24queue2. Seed1013age85/armspare9.5cost13 PASSED:
failureissustainedcycle,NOTreentry. Ref6220/5758ticks,residual2us. Allfinaloff/
closed,noretry. Highest3/3cohort remains14FE/340/70. RANGE345_EXPERIMENT.md.

E433 staged345FAA0FE858FE21A8456A647FEF36C3818A0A12D7A15B14F94FC0516033FF017B4
NOTFLASHED.2899/241runtime,5798/482acq,966seed;referencewait121us,89agepasses
32floor,89.5refuses. All electrical/tracking/deadline/16uscostguards unchanged.
RANGE345_EXPERIMENT.md rationale/cohort+localtail,NOTjitterfix. Release123836/
1104/29336,auditmatch,362Python+226Rust345PASS,old340lib191PASS. Next disabled
preflights then70recovery,conditional71hold/recovery failfast. Actual14FE/off.

E432 local-cycle offline evidence: failed71last154..170ms median3016us/~331.56eHz,
NOTaggregate315.355steady. Prefixmedian3867us;257eventsomitted, nointerpolation.
Rejectedguard2930 is~2.85%shorterthanlocalmedian,ref2934us;stillnotrotor/ISRcause.
drv_local_cycles.py separateswindows/overlap,2testsPASS. STARTUP_BOUNDARY_COMPARISON.md.
Nohardware/codeprofilechange;actual14FE/off. Next review345candidate near332
operation withsameguards;966seedwait121us leaves4/2usmargin at85/87age,90refuses.
345NOTimplemented/qualified; don't characterize lastfailure as steady315Hz.

E431 actual14FE/off.70recovery02+03PASS:cohort3/3original30s at325.65..325.79eHz,
COM~54700matchesacc,sigma23.86..24.00us,IRQ52.97..53.13%,armspare>=12us,
stack2676. Frozenreference/range340_14fe. ThenONE71hold10s FAILED170375us
duringacceleration,CycleTiming2930<2942,322COM/321acc,raw242bus11319,stack2676.
Refcycles6136/5868ticks,residual-4us. Allfinaloff/closed;noretry.7.1unqualified.
2retaineddatatestsPASS. Next reviewacceleration/nextboundary/fresharm before
expansion, notsteady-speed/hardwareclaim. RANGE340_EXPERIMENT.md hashes.

E430 actual14FE installed/off. range340guard01UARTsilent;snapshotexactsafe
APBENR108000000/CR10dBRR22b ISR006000c0 thenclockrepair08040000;guard02PASS3/18,
fivepreflightPASSCPU2/7/10.69recoveryPASS319.906eHz,then70hold10sPASS325.762,
then70recovery30sPASS27.991037s resumed325.793eHz54716COM/54715acc,
sigma23.855us,IRQ53.129%,raw289bus10877,seed1034age85/armspare12.5cost13,
stack2676,DMA22queue2. Allfinaloff/closed. One70recoverypass NOTcohort/jitterfix.
Next fixedtwo70recoveryattempts failfast beforemore duty. Details/hashes
RANGE340_EXPERIMENT.md; retainedthreecapturetestPASS. Old335failuresunchanged.

E429 staged340candidate14FEB2195EAD922D1451B630962FFF95254AE858547BF2DA4F1CA0114642A0FF
NOTFLASHED. RANGE340_EXPERIMENT.md:2942/245runtime,5884/490acq,980seed.
Referencewait245,age181passes32usfloor,182refuses; unchanged16uscost/current/
bus/tracking/deadlines. Profileexpansion NOTjitterfix;oldfailuresunchanged.
Release123832/1104/29336,audithashmatches;357Python+225Rust340PASS,335lib190PASS.
Next disabledpreflights then69recovery; onlyifpass one70hold10s thenrecovery.
ActualEC51/offE428 unchanged, frozenreference/range335_ec51.

E428 actualEC51/off. range335_start61_reentry69_30s_02+03PASS; declared69cohort
3/3original30sstartup/recovery near319.82..319.87eHz,COM~53712matchesacc,
sigma23.91..24.25us,IRQ52.72..52.87%,armspare>=14.5,stack2676. FrozenELF
reference/range335_ec51. ThenONEhold70_10s_01 FAILED3.702165s325.193eHz,
CycleTiming2964<2986,raw274bus10925,stack2676,ref6332/5921ticks,residual3.5us.
Allfinaloff/closed,noretry.7.0unqualified.2retained-evidencetestsPASS.
Next reviewnextboundary/fresharm beforeprofile expansion; nohardware/CPUcause
claimed. RANGE335_EXPERIMENT.md hashes/details.

E427 actualEC51 installed/off. range335guard01UARTsilent;snapshotexactsafe
APBENR108000000/CR10dBRR22b thenclockrepair08040000;guard02PASS3/18,
preflight01fivePASSCPU2/7/10. range335_start61_reentry68_30s_01 PASS27.991039s
315.703eHz53021COM=acc,raw272bus11032,armspare17.5. Then69_30s_01 PASS
27.990912s319.832eHz53715COM/53714acc,sigma24.251us,IRQ52.866%,raw288bus10829,
armspare15.5/cost12,stack2676,DMA22queue2. Bothfinaloff/closed. First69pass
undernew335profile NOTjitterfix/cohort. Next fixedtwo69recoveryattempts failfast
sameconfiguration before moreduty. SeeRANGE335_EXPERIMENT.md hashes/details.

E426 range335 candidateEC510EAC16A5DFDD20713A3FAA7AE0726E346DC3A5D78524920D2CE88A3BC460
NOTFLASHED. No-gain1717 archived reference/earlycarrier_1717; feature omitted.
RANGE335_EXPERIMENT.md declares expansion NOTjitterfix:2986/248 runtime,
5972/496 acquisition,995seed;32usarm/16uscost/current/bus/deadlines unchanged.
Late seeds refuse; WCET proof not an invented prerequisite for guarded trial.
Release123836/1104/29336,audithashmatches,354Python+224RustPASS,old330lib189PASS.
Next disabledpreflights then68recovery; onlyafterpass one69campaign undernew
explicitprofile. Alloldfailuresretainoldcontract. Actual1717/offE425 unchanged.

E425 actual1717 nowinstalled/off. earlycarrier bootguard01UARTsilent; snapshot
APBENR108000000 USARTCR10d/BRR22b ISR00600010,PD1zeroBDTRc1aCCRs0 BEFORE
clockrepair08040000. guard02PASS3/18;preflight01fivePASS inclnewcleanup5/5,
CPU2/7/10. earlycarrier_start61_reentry68_30s_01 PASS27.991123s recovered,
315.704eHz53022COM/53021accepted,IRQ52.426%,sigma23.513,raw274bus10889,
stack2676,DMA22queue2,seed1059/acq6647/armspare14.5. Finaloff/portclosed.
SEEDLAT88/106/120/152/156 identicalquietstamp68: NOobservedlatencygain.
No6.9retry/profileexpansion justified. Next retire no-gain campaignfeature;
retain implementation/evidence, target actual remaining acquisition/guard path.

E424 opt-in bench-reentry-carrier staged1717BA38D78466DF61A56D99F92A5428BA81239275A681EF5335A91F50751865,
NOTFLASHED. Early recovery ARR setup; success-only clear preserves period but
revokes outputs/roles, generic/failure reset unchanged. Live ARR/CCRs/owner/DMA
admission; rolecheck adds actual ENABLE-low 5-check cleanup diagnostic (NOTRUN).
Fixture --reentry-carrier exact marker. Release124808/1104/29336, matching
advisory mathaudit (100unreviewed),353Python+223Rust testsPASS. Next disabled
candidatepreflights thenmatched68recovery; no gain/profileclaim. Actual137F/offE421.

E423 RECOVERY_POST_EDGE_AUDIT.md source+three retained captures: edge-to-entry
44us, entry/reset9us, feedback7us, guard15..16us, reference2us. Grouped wall
times, NOT WCET/function attribution. Bulk resets/seed division already fixed.
Next concrete candidate: recovery-only early carrier preparation with successful
cleanup preserving ARR but revoking ALL output/role authority; generic/failure
cleanup unchanged, live ARR/owner checks required. Not implemented/qualified.
No flash/motor/profile change; actual137F/offE421 remains authoritative.

E422 ENVELOPE_BOUNDARY_REVIEW.md:3031us isexperimental330envelope, notsilicon
ceiling or automaticbugcertificate. Prospective335 actualreferencewait124.5us;
85/87usages leave7.5/5.5usarmmargin,93usfails. RusttestPASS andcurrentadmission
stillrejects995seed; NOprofile/guardchange. Actual137F/offE421 unchanged.
Next bound variablepost-edge recoverywork before coordinatedprofiledecision;
distinguish wideningqualifiedenvelope from claimingjitterfixed.

E421 actual137F: quietstamp_start61_reentry68_30s_01 PASS27.990639s resumed
315.395eHz52969COM/52968accepted,IRQ52.738%,sigma23.731us,raw277bus10925,
stack2676,seed1073/acq6749/arm49cost12,DMA22queue2. Then ONE69recovery
FAILED27.133800s resumed319.662eHz,52042COM/52041accepted,CycleTiming12
step1delta3017<3031. Raw286bus10972,stack2676,reference6408/6029ticks.
Bothfinaloff/portsclosed; nohigherduty. Timestampomission notfaultfix;
0.79ppIRQdifference fromlatestbaseline not causalreliability proof.
Next investigate timing/acceptance mechanism beyondreportstamp, notunchangedretry.

E420 actual137F candidate installed; bootguard01PASS withoutclockrepair,
uart_candidate_before_repair01 snapshot matchesworkingbaseline (postguard),
so priorbootfailure unresolved/intermittent. preflight02allfivePASSCPU2/7/10.
quietstamp_start61_hold68_01 PASS10s315.435eHz18926COM=accepted,IRQ52.826%,
raw274bus10901,stack3468,COMP40COM44commit21DMA22queue2. IRQSTAMPomitted1.
Finaloff/exit0/portclosed. No clear gainclaim vsbaseline~53.5 (differentwindow).
Next same61startup/68BEMF30srecoveryqualification, nothigherduty.

E419 boot audit: source+emittedUSART setup setsclockbeforeBRR/CR; no culprit
write established. Added read-only drv_uart_snapshot.py (noRDR/ICR/writes),
baseline uart_baseline_snapshot01.jsonl:APBENR1=08040010,CCIPR0,PCmoderf3afffff,
AFRH0,USARTCR1=0d/CR2=0/CR3=0/BRR22b,ISR006000d0;PD1zero/BDTRc1a/CCRs0.
ActualA2DE unchanged, no motor/flash. Next candidatefailedboot snapshot BEFORE
repair for comparison; no inference from priorUARTsilence alone.

E418 quietstamp137F flashed but UARTqualification FAILED, no motor. Emitted
branch080019d8 skips maskeddiagnostictimestamp080019de..1a04 onpoweredtrace0.
guard01silent; exactRCC08000000/PD1zero/BDTRc1a/CCRs0 thenclockrepair.
guard02 andpreflight01_pulse stillsilent despiteRCC08040000/safeoutputs.
RESTOREDactualA2DE archive. restore_guard01silent/exactsaferegs/clockrepair;
restore_guard02PASS3faults/18refusals/finaloff. Root137F NOTinstalled.
Candidate unqualified, UARTcause unresolved, not timingregression proof.
Next inspect boot/UART qualification issue before anothercandidate campaign;
do not repeat flash/repair blindly. Allfailedcaptures retained.

E417 bench-quiet-irq-stamp staged137F0E10 NOTFLASHED. Skips report-only
LAST_IRQ_US maskedclock sample onpoweredtrace0 dispatch; onlyconsumer is
COAST_END_STATE report. ExplicitIRQSTAMP/strictfixtureflag, resetzero inclreentry.
Accepted/stop/safetyclocks unchanged; seeQUIET_IRQ_STAMP_EXPERIMENT.md.
Release123836/1104/29336. ActualA2DE/offE415. Next emittedbranch audit then
disabledpreflights/bounded6.8qualification; no measured gainyet.

E416 static pointer-bundle experiment NO COMPcodegen gain. Candidate517BEB72
releaseADC_COMP080016a4 size0x850 exactdemangleddisassembly matchesA2DE;
referencecomp_isr08004438 size0x1d0,sizes123648/1104/29336unchanged.
Do not claim wholeELF identical (debug/source-dependent data differs).
Optin staticbundle feature removed; pure sched/drive constructors nowconstfn.
No flash/serial/motor. ActualA2DE/offE415; root517B experimentNOTinstalled.
Next inspect actual repeated dispatch/control loads rather than pointer bundles.

E415 restoredactualA2DE7D983DF4F248BCC5C524B17FC52E24D6A8913CE5E4904E08BDD17B896846
from reference/dmapeer_a2de. F730 preservedreference/decision_f730; rootELF
stillF730 NOTinstalled. Preflashoff/noCOM41conflict/COM8untouched.
retire_decision_guard01 UARTsilent; exactRCC08000000/PD1zero/BDTRc1a/CCRs0
then documented08040000clockrepair. guard02PASS3faults/18refusals;
retire_decision_preflight01 allfivePASSCPU2/7/10/finaloff.
baseline_start61_reentry68_30s_01 terminalPASS original30s,27.990602s resumed
315.7165eHz,53023COM/53022accepted,IRQ53.52995%,sigma24.1188us,raw260bus10901,
stack2676,seed1072/acq6708/arm49cost12,DMA22queue2. Finaloff/portclosed.
Experimentalcritical/decisionrecording removedfrominstalledimage. No6.9claim.
Next reduce actual qualification/dispatch cost or define targeted timing A/B;
do not reintroduce expensiveobserver or repeat known6.9failure withoutchange.

E414 decision_start61_reentry68_30s_01 FAIL after2.666888s resumed316.347eHz,
CycleTiming12 step2 delta3015<3031.5062COM/5061accepted,IRQ70.405%,raw319
bus11056,stack2120,criticalmax48/refused0,DMA24queue3. Finaloff/exit1.
Decisionlastcount828 vsrefthiszc841 =6.5us; previousaccepted1218 vs1231 also
6.5us. No long gate-to-accept stall at THIS fault; pre-entry/physicaledge
unknown. Diagnostic does not qualify6.8recovery; park furtherpowered retries.
ActualF730 installed/off. Next retire expensive diagnostics/critical experiment
to archived baseline with disabled requalification; preserve all failure data.

E413 decision_start61_hold68_01 PASS10s after startupduty61 (onlyruntime
change fromE412); actualF730.316.098eHz18966COM/18965accepted,IRQ70.397%,
raw240bus10925,stack2748,criticalmax32/refused0,DMA22queue3. Startupraw546
bus11450,arm58us/cost12. Singlepass NOTstartupreliability proof.
Decision165204total:29792closed/116447open-no-accept/18965accepted;32CRCrows
strict epoch matchesaggregates. Postgate nonacceptance dominatesCURRENTbuild.
Finaloff/fixtureexit0/portclosed. Recorder iscostly diagnostic, notproduction.
Next preceding6.8 recovery qualification withstartup61 before6.9 orhigher.

E412 decision_hold68_01 FAILED startupCurrent5 at11237us beforehandoff;
70CRC ADCrows peak1256>1200, lastlogicalC3304 at11063us,bus11605mV.
COMPDECISIONtotal0/COMPCRITICALcalls0: recorder/fullBEMF notexercised.
Finaloffverified/fixtureexit1/portclosed,actualF730 unchanged. Do not call
this recorder overhead failure or qualified6.8; no retry/higherduty/guardchange.
Capture894653C0410E9BBED58FD5550A0BEE7A0D43660C77577D5A16771657C6B89A19.
Next examine startup current sample timing/entry variation before reattempt;
instantaneousraw counts notPSUamps, no electricalfaultcause established.

E411 actualF7302F50D950378593765275CF88514923183C827B4E5C5AD9D8011067020818
installed diagnostic.044F archived/hashverifiedreference/compcritical_044f.
Preflashoffverified/noCOM41conflict/COM8untouched; UARTreset workedno repair.
decision_scope01 allfive modes16/16PASS max9/9/9/8/8us; lifecycle notfullISR
qualification. decision_guard01 3faults/18refusals,decision_preflight01 allfive
PASSCPU2/7/10. Finaloff/portsclosed. No motor on this image yet.
Next bounded preceding6.8 diagnostic hold with --comp-decisions/--comp-paths
and currentprovenance; check overall IRQcost/criticalmax/stack and guards
before any6.9. No claim that9us recordercost is free or fullWCET.

E410 decisioncheck implemented, NOTRUN/NOTFLASHED. Actual Scope begin/first
count/finish/Drop five modes x16 checks state/row/freeze/firstcount retention;
disabled/ACTIVE/owner admission in core. No reference core or gate calls.
Host drv_decision_check.py exclusivecapture/finaloff verifier reports timing
NOTqualified.344PythonPASS. CandidateF7302F50D950378593765275CF88514923183C827B4E5C5AD9D8011067020818
release text127732/data1128/bss29876,flashspare2212. Actual044F/offE404.
Next archive actual044F before flash, safechecks then disabled decisioncheck;
no motor until scope timing plus standard preflights pass.

E409 decision candidate1125C2E3F696E370D7817CB71FC4F6A77F94627B1C1BB454903A9E07C1EE9215
builtNOTFLASHED, text126860/data1128/bss29876: flashspare3084, RAMafterstatics5860.
PriorCF23 codeaudit Scopecomplete44localbytes/no divisionhelpers, duplicate
classification observed; no measuredWCET. TAIL532bytes. Captureheader now
explicitcapture flag, quiet accepted only optional, requiredcapture refusesquiet;
orphanrows refused.343PythonPASS. Actual044F/offE404 unchanged.
Next disabled actualscope lifecycle/timing check before any flash/powercampaign.

E408 bench-comp-decisions opt-in integrated/release builds, NOTFLASHED.
Reuses firstactualcount/comp_paths, Scope context entrytime/step; completed
calls pushed after reference, Drop closes earlyreturns unknown. Reset perobs;
dump freezes outside sensing, CD85 CRC rows capture-only (D85 alreadyused).
Strict drv_comp_decisions + fixture--comp-decisions added. No per-read hook
added; static reference path preserved. Actual044F/offE404 unchanged.
Next emittedcode/memory audit and disabled live-scope tests BEFOREpower.

E407 pure comp_decision_tail implemented/tested in observer-replay, NOTshell
integrated/built/flashed.32x16byte bounded ring, explicit omitted/invalid/frozen,
stopped call unknown retained thenfreeze, no per-read work/hardware access.
Read COMP_DECISION_TAIL_DESIGN.md for integration/epoch/earlyreturn requirements.
32rows only~1.7ms at19kvisits/s, not fullcycle/physicaledgehistory. Actual044F
and offE404 unchanged. Next opt-in integration and codegen/disabled qualification.

E406 COMPARATOR_CAPTURE_AUDIT.md: existing COMPPATH aggregate cannot locate
individual misses; trace1 bypasses static/protected path and adds per-read work.
Offline rejection_cadence added, strict consecutive same-sector rejects only.
Archived passing AND failing traces have100us gaps; presence not discriminator.
No hardware/firmware change, actual044F/offE404. Next bounded decision-level
capture preserving static read path; do not silently substitute trace1 or
repeat full-IRQ observer rejected by CPUCHECK. Old E299distribution is notcurrent.

E405 offline CYCLE_CLOCK_COMPARISON.md: four6.9 failures guard minus six
reference intervals only3/3/2/2.5us. Short cycle exists before recorder;
callback-only delay not sufficient. Both clocks follow qualified events,
not independent rotor/edge timing. Threshold-selected failures CANNOT prove
PWM quantization. drv_cycle_fault clock_closure regression added. No hardware
or firmware change; actual044F/offE404. Next audit pending/gate/reject path
and uncensored capture options, not another expensive full-IRQ probe.

E404 actual044F: emitted PRIMASK scope reviewed; compcritical_hold68_01
PASS10s and reentry68_30s_01 PASS original30s (27.990539s resumed),
315.863eHz,53047COM/53047accepted,IRQ59.034%,sigma28.461us,raw254bus10996.
Masked body max33us/refused0; stack2676, arm48us/cost12. Then ONE
compcritical_reentry69_30s_01 FAILED before dropout at159790us:
CycleTiming12 step2 delta3029<3031us;293COM/292accepted,raw233bus11343,
critical3048calls/max48/refused0. All outputs off verified, fixture exited1.
No higher duty/retry/guard relaxation. Protection does NOT eliminate fault;
no hardware or sole scheduling cause proven. Read COMP_CRITICAL_EXPERIMENT.md.
Installed044F remains diagnostic, NOT qualified6.9. Next offline compare
acceptance/dispatch timing and startup transient against prior failures.

E403 actual044FF992156A607940B1B49BFAC29D6A22DB76085C768F050F55CDDEB3659D04
DIAGNOSTIC installed, no motorqualification. Samehelpercritical_service used
byproduction anddisabledcheck. wrapper01silentUART/exactsafeclockrepair;
02fourmodes16/16PASS maxima1/7/6/68us earlyreturn/full/nested/intentionaloverrun.
Wrapperonly, NOTfullserviceWCET. guard01threefaults/18refusals,preflight01
fivePASSCPU2/7/10,finaloff/portclosed. Strict--comp-critical marker/verifier,
337Pythonbeforehardware. BaselineA2DE archivedreference/dmapeer_a2de.
Next emittedfullservice scope/callgraph/stack review beforebounded68power;
don't misrepresent wrapperdummybodyasrealaccept/record path proof.

E402 bench-comp-critical stagedNOTFLASHED; readCOMP_CRITICAL_EXPERIMENT.md.
Wraps one existing static traceoff powered referenceCOMP call INCLUDING
recorder, notjustreads. Exactfilter12 admission; maxbody60us posthocabort,
countersresetobservationepoch, COMPCRITICAL poststop. No sharedminz edits.
ReleaseautoauditPASS, actualA2DE/lastoffE401 unchanged. Next strictfixture
and actualwrapperdisabled reject/accept/PRIMASK/duration tests; CPUCHECK
doesNOTcover wrapper. No power untilscope/stack/guard qualification.

E401 sameA2DE: dmapeer_reentry69_30s_01 FAILCycleTiming12 after13.088722s
recovered320.007eHz,25131COM/25130acc,IRQ53.125%,sigma24.428us,raw263bus10937,
DMA22queue2/record19/commit21,stack2676,seed1058/acq6574/arm47.5cost12.
Guardstep3 13085672->13088683 delta3011<3031; reference6473/6018ticks.
Finaloff verified,portclosed; no higherduty/profile. DMApreemption NOTsolecause.
Reference am32_isr.rs99..108 persistence loop outsideCS; CSstarts109 for
mask/reset/COMarm. Next bounded persistence exclusion feasibility, preserving
guard servicing between handlers; NOT wholesaleguardprioritylowering.
No sourcechange toreference or newmaskyet. Allfailure evidence retained.

E400 actualA2DE7D983DF4F248BCC5C524B17FC52E24D6A8913CE5E4904E08BDD17B896846
DMApeer installed. Actualvectorpriority01 3trials123/132/213 PASS/restored,
noUARTrepair;guard01threefaults/18refusals,preflight01fivePASSCPU2/7/10.
hold68_01PASS10s315.365eHz,initialraw1188/1200guard distinctrunning291.
reentry68_30s_01PASS315.568eHz52999COM/52998acc,IRQ53.135%,sigma24.417us,
COMP41COM48commit21record19DMA22queue2,raw285bus10972,stack2676.
Seed1071/acq6673,arm49cost13(17spare),SEEDLAT85,deadline188spare,finaloff.
334PythonPASS. Same3031/252/allguards, ONLYDMA priority64 instead0 plusidle
probehooks. Next69recovery, nothigherduty/profile. ReadDMA_PEER_EXPERIMENT.md.

E399 bench-dma-peer stagedNOTFLASHED; readDMA_PEER_EXPERIMENT.md.
OnlyDMA0->64,COMP/COM64 and independentguard0 unchanged; no expensiveprobes.
ActualPACvectorsDMA9COMP12TIM6=17COM21; disabledactualvector orderingtest
needed BEFOREpower, oldTIM2/TIM16probeinsufficient. Leasecopy100us/flags/NDTR,
acquisitionage/FIFOguards unchanged.332Python/releaseautoauditPASS.
Actual9789,lastoffE398 restoration. Next disabledprobeimplementation then
preceding68qualification, not blind69retry/guardprioritychange.

E398 packedoverlap FA4762A54749F0468C4B81C7142912F1366E598099C16554205126526ADB75D4
alsoFAIL11usvs10; parkprobe. Folded bits into unusedmeter mask0/7, noextraRAM,
218Rust/releaseautoauditPASS,text122372/data1104/bss29324. cpu01UARTsilent/
exactsaferegs/clockrepair;02max2/7/11,nestedsum2616vs2632previous,negligible.
No motor. RESTOREDactual9789 archive;packed_restorecpu01PASS2/7/10/noUARTrepair/
finaloff. RootFA4762 notinstalled. Next controlledpriorityA/B feasibility
including guardstarvation/deadlines and pendingNVIC ordering, not more tiny
observeriterations. Noprioritychange implemented orpoweredauthorizationproof.

E397 compactbench-comp-overlap E283F2878ED93232CDADCBD823FA575572E47C798EEB459A6BD2FAA13B625E30
disabledCPUFAIL11usvs10, no motor.218Rust/330Python/releaseautoauditPASS,
text122396/data1104/bss29328. Stopcontextonlybits viaexistingmeter mask,
noextraclocks/history; staticadapterunchanged. overlap_cpu01 max2/7/11.
Restoredactual9789;restorecpu01UARTsilent/exactsaferegs/knownclockrepair,
restorecpu02PASS2/7/10/finaloff. RootE283 NOTinstalled. Do not spinprobe or
relaxgate; next emittednestedpath audit or different causal experiment,
not another fullchronology build. No faultcause identified.

E396 schedulingtail97A69B44 DISABLED OVERHEAD FAIL26usvs10gate, no motor.
cpu01UARTsilent/exactPD1zeroBDTRc1aCCRs0RCC08000000 thenknownclockrepair;
cpu02max2/15/26fault0, finaloff. RESTOREDactual9789 archive; restorecpu01
PASS2/7/10/noUARTrepair/finaloff. Root97A69 remainsdiagnostic NOTinstalled.
328Python beforehardware; strict--scheduling-tail and CPUepoch/countchecks.
Fullchronology tooexpensive; do NOT relax10usgate or spinthisprobe. Next
lower-cost nested-handler presence bits/accept-boundary evidence, not full
timestamped ring at everyIRQ. Retain allfailures; no motor thisturn.

E395 optionalbench-scheduling-tail integrated, BUILT NOTFLASHED78846172C07BACC81E6E06F8173123B20F72A4E1486D9989EE36A01A6B7C20D4.
Actual9789 archivedhashverifiedreference/binmath_9789;lastoffE393 unchanged.
CPU boundaries sharecurrentstamp withtail, nested additionalreads intentional;
reset/firstguardstart/frozenfinish/poststoprowwiseCRC dump. CPUCHECK exercises
tail includingnestedpair andfailsontailfault.327PythonPASS incl strictdecoder.
No hardware/overheadqualification. Next strict fixture-required provenance,
codegen/stackreview then disabledCPUCHECK unchanged10us beforepower. Do not
flash assuming instrumentation ischeap. StaticComp/trace0/guards unchanged.

E394 pure scheduling_tail.rs + hosttests, NOT livewired/flashed. Read
SCHEDULING_TAIL_DESIGN.md before integration.64x8byte rows,<=544total,
posttransition nesting context/omission/fault/frozenstop,217RustPASS.
Existing CPUunion intentionally skipsnested clocks; new chronology must
pay/qualify their cost under unchanged10usCPUCHECK gate. KeepStaticComp,
trace0, guardedpreceding68qualification before69faultreplay. No hardware
action; actual/root9789,lastoffE393. Fullwrapblackout unsupported; tail not
physicaledge latency/foregroundmask evidence. No outliercause proven.

E393 same9789: binmath_reentry69_30s_01 FAIL after fresh recovery, resumed
5005492us/320.144eHz/9615COM/9614accepted. CycleTiming12 step4 delta3012<3031,
guard5002442->5005454; recorder previous+29us, reference6410/6018ticks.
Finaloff verified, process terminal/portclosed. IRQ53.427%,sigma26.062us,
raw256bus11104,commit37/recorder35/DMA14,stack2696,seed1059/arm47.5cost12.
Arithmetic reduced overhead but did NOT eliminate intermittent cycle fault.
No higherduty/profile/priority change or blind retry. Next scheduling evidence
instrumentation must qualify observer cost; existing trace1 changes adapter.
Capture SHA B1FEB1B58D8432BD1A804E6A2E7477AE65C030FD9697B19DB0C8C20205D22888.
E392 hold pass remains true; 6.9% recovery is NOT qualified.

E392 actual9789F836549D2039AAE6F44DC8F2342E2C782D4525EBDB9EB6BF6D4E5AC3AADD.
BINMATHstrict;priority01PASSnoUARTrepair,guard01threefaults/18refusals,
preflight01fivePASSCPU2/7/10. binmath_reentry68_30s_01PASS315.824eHzIRQ53.247%,
COMP59COM63commit37recorder35DMA14,arm49cost12(17spare),SEEDLAT85,stack2696.
Then binmath_hold69_01PASS10s319.911eHz19195COM/19194acc,IRQ53.233%,raw264
bus10901,COMP59COM61commit37,stack2696;startupraw977 distinct. SAME3031/252
profile/priorities/guards,finaloffverifiedboth. Next69recovery BEFOREhigherduty.
OneboundaryPASSnotcauseproof/cohort;E383failure retained. ReadADC_BIN_MATH.md.

E391 recorder maskeddivision found! ArchivedCC71 Obs::record uidiv08003a6e
is sixbin timelineindex everyaccepted event. Replaced exactboundedthresholds,
213Rust/release+autoauditPASS. NewcandidateNOTFLASHED05BA9841616BE6A0EBE73869B0F99D00087101D0DEEF753B7A3BE7DC32207079 includesADCbin.
ActualCC71,lastoffE389 unchanged. Read SCHEDULING_OUTLIER_AUDIT.md E391:
UARTnotblockingpath; conversionoutsidefeedbackmask; acceptedstatsinside.
I85lacksotherISRentry/exit and trace1changesadapter. No outliercauseclaim.
Next strictprovenance/disabled/matched68qualification beforefaultreplay.

E390 stagedNOTFLASHED9B140D993A452E2E77FA9960F87399FCD2029DD5282FF7FC74B505E242807B91.
ActualCC71 archivedreference/carriermath_cc71,lastoffE389. ADC_BIN_MATH.md:
exactconstbinreciprocals/all9066phases tested,212Rust/release+autoauditPASS.
Nohardwaregainyet;marker/finalreviewpending. User redirectsattentiontoscheduling,
NOhardwareblame/caps. Read SCHEDULING_OUTLIER_AUDIT.md: poweredUARTpoll is
nonblocking/noTXinrunloop,bytedurationNOTmaskedtime. Guard/DMApreemptCOMP;
trace1changesadapter/perreadwork, qualifyobserver before69faultreplay.

E389 actualCC71BC841C327AF82EA599C8C528692B3748FD1AE40F51AADD792FF0790203E9.
CARRIERMATH/--carrier-math strict. Priority01PASSnoUARTrepair,guard01threefaults/
18refusals,preflight01fivePASSCPU2/7/10. carriermath_reentry68_30s_01PASS
315.805eHz53038COM/acc,IRQ54.724%,sigma27.804us,raw284bus10937,stack2696.
COMP65COM67commit41vs69/45baseline. Arm49cost13(17spare),deadline191,SEEDLAT85.
N1favorabletimingnotexclusiveCPU/WCET. Alloff/portclosed; no profileincrease.
Next ADCphase live-math exactdomain audit, see M0_ARITHMETIC_SWEEP.md.

E388 readGRAYBEARD_M0_ARITHMETIC fully; see M0_ARITHMETIC_SWEEP.md corrections.
ActualA727 reentry68_30s_02PASS316.478eHz,85usSEEDLAT,17usarmspare,finaloff.
Plannedcohort2completed/2PASS,thirdNOTattempted aftermemo. NoPSUreplyyet.
Carriercompare now onlyinitial sideeffectinghelper; preparedbranchc0b4skips
divide/helperc0bc.212Rust/release+autoauditPASS,NOTFLASHED05B73D4C4A60A85EAE5510D265BEF36CAF50ED144AC03D57DCD22C2619CF22CF.
IncludesE387. Text121944/data1104/bss29324. Next disabled+matchedqualification
beforeCPUgainclaims; memo/200formula WRONG at1199, no approximation deployed.

E387 bounded successor stagedNOTFLASHED07AB01E6B970E51402B687E1A84F0A27644E88A32073E124BEF603B988C9D1F1.
ActualA727 archivedreference/clearonce_a727,lastoffE386. Read BOUNDED_SECTOR_MATH.md.
Validatedsector%6+1 replaced privateconst successor in bothguardpaths; sixinput
compiletimeproof/all256admissionhosttest,211RustPASS. GeneratedCMP/branch/add
replaces2MUL/shift/sub,nothelpercall. Release+autoauditPASS,sizesunchanged.
No measuredgain/hardware thisturn. Guardbulksetup already staged; livechecks
cannotprestageruntimeauthority. Avoid a separate endlessmicrooptimizationcohort.

E386 actualA727F8D7 installed, clearonce_reentry68_30s_01 PASSoriginal30s/finaloff.
Codegen resume skipsduplicateclear, initialretained, localstack204unchanged.
priority01UARTsilent/exactdisabledclockrepair;02threePASS,guard01threefaults/
18refusals,preflight01fivePASSCPU2/7/10. Recovery316.398eHz53138COM/53137acc,
IRQ55.209%,sigma28.268us,raw247bus10937,COMP65COM69commit45,stack2696.
Seed1060/acq6635,arm47.5cost13(15.5spare),deadline215spare;SEEDLAT85vs87us.
N1smallgainNOTWCET/steadyCPUfix. No higherduty/profile,alloff/portclosed.
Read REENTRY_CLEAR_EXPERIMENT.md; next larger ownership-safe recovery delay
target, not blind69retry or automatic guard widening. Actual no longerF715.

E385 stagedNOTFLASHEDA727F8D7274EA33B7A1B54253E8F5D7639A2A467C7E985A82D5D5E6B1B5C7E6B.
ActualF715 archivedreference/range330_f715,lastoffE383. Read REENTRY_CLEAR_EXPERIMENT.md.
Optionalbench-reentry-clear-once omits only coast_run_inner's duplicateclear
after successfulawake acquisition; firstclear/livechecks/guard/carrier remain.
Release+autoauditPASS,318PythonPASS,strict --reentry-clear-once marker added.
No measuredgain. Next emittedbranch/stackreview then disabledpreflights and
matched68recovery, not higherduty/profile. No hardware this turn.

E384 offline recovery budget updated in SPEED_ARM_BUDGET.md using measured87us.
Prospective340 has3.5us armspare,345 has2us,350 zero,351 fails. NotWCET/speed
qualification; current seed admission still refuses below1010.210RustPASS.
ActualF715/lastoffE383 unchanged. Next recovery preparation/final-edge boundary
audit, preserving livechecks/ownership/32usfloor; no blindprofile expansion.

E383 retained range330_hold69_01 FAILCycleTiming12 after421292us on sameF715.
Step1 guard418247->421256=3009us<3031; reference cycles6424/6012ticks,
pairedmean3109us NOT independent rotor proof.797COM/796acc,raw247bus11044,
stack2696,finaloff verified. No recovery/higherduty/profilechange attempted.
Regression preserves refusal and unknown cause. Next audit acceptance timing
and recovery arm budget before any profile expansion; do not retry blindly.

E382 sameF715:67recoveryPASS310.116eHz,68holdPASS316.515,68recoveryPASS316.583.
Original30s recoveries retain deadlines/finaloff; no guard/profilechange.
67arm49cost12(17spare),68arm46cost12(14spare),bothSEEDLAT87us.
68recovery53169COM/53168acc,IRQ55.334%,sigma28.315us,raw248bus10984,
COMP65COM70commit45,stack2696,deadline135spare.67IRQ55.034/raw263bus10746.
Allportsclosed. Next bounded69hold on SAMEprofile; retain anyfailure before
recovery/higherduty, no automaticprofileincrease. Notrepeatcohort/330qualification.

E381 actualF7155BFB installed330profile. priority01UARTsilent/exactdisabled
clockrepair;02threePASS,guard01threefaults/18refusals,preflight01fivePASS CPU2/7/10.
range330_hold66_01PASS304.082eHz;reentry66_30s_01PASS304.043eHz51062COM/
51061acc,IRQ54.991%,raw272bus10925,arm52cost12(20spare),seedage87us.
Then range330_hold67_01PASS10s309.987eHz18599COM/acc,IRQ55.545%,raw265bus10913,
COMP65COM69commit45,stack2696,arm74.5cost12. Exact3031/252readback.
Alloff/portsclosed. Next67recovery beforehigherduty; no more66revalidation.
This is expandedprofile evidence, not jitterfix/erasureofolderfailures. Goalopen.

E380 range330 stagedNOTFLASHEDF7155BFB3A5E1CBEEDA8D9CADE8A7F91EFC290093970E7717517102DF279124C.
Actual4D43,lastoffE379; archive reference/seedlean_4d43 hashverified.
Read RANGE330_EXPERIMENT.md. Runtime3031/252us,acq6062/504ticks,seed1010;
actualarm32us and all electrical/age/tracking/deadline guards unchanged.
87us observedage leaves7.5us nominalspare atseedboundary, notWCET/330proof.
209Rust/316Python/release+autoauditPASS;text121780,data1104,bss29324.
Next disabledpreflights then matched66hold/recovery with exactprofile readback;
onlythen bounded67hold. No automatic furtherprofilechange afterfailure.

E379 actual4D43 lean:65/66 holds10s + original30s recoveries allPASS/finaloff.
Recovered299.879/304.320eHz,IRQ54.880/55.576%,sigma29.376/28.794us,
COM50362/51108 equalaccepted,raw276/250bus11008/10769,stack2696.
COMP65COM69commit45,arm54/52.5cost12(22/20.5spare),deadline244/174spare.
SEEDLAT44/10/8/15/2/8=87us both. Startupraw571/560/714/771 forfourruns.
Allportsclosed, no guardchange. Next coordinated speedprofile decision using
87us observedage (notWCET); no more equivalent65/66 repeats or blind67lottery.
Currentcalibration/independentquality/fullgoal remainopen.

E378 actual4D43E26B76B77B3C44C570ECA13866D67682ED6BFBB1DCD4CE07FF7BD65153A5
lean seeddiv installed (noacquiretiming). Prior6B71 archivedreference/seeddiv_probe_6b71.
Release+autoauditPASS text121768,data1104,bss29324; bound/MUL/shift verified.
priority01UARTsilent/exactdisabledclockrepair;02threePASS,guard01threefaults/
18refusals,preflight01fivePASS CPU2/7/10. seedlean_reentry64_30s_01 PASS:
292.845eHz49181COM/49180acc,IRQ55.208%,raw278bus10937,COMP65COM69commit45,
stack2696,arm57cost12(25spare),deadline179spare. SEEDLAT44/10/8/15/2/8=87us;
ACQUIRELATabsent. Fullfixture/provenance/finaloffPASS,portsclosed. N1, notWCET.
Next preceding65/66 hold/recovery then coordinatedprofileaudit; allguardsunchanged.

E377 actual6B71 installed, seeddiv_reentry64_30s_01 PASSoriginal30s/finaloff.
priority01UARTsilent/exactdisabledclockrepair;02threePASS,guard01threefaults/
18refusals,preflight01fivePASS CPU2/7/10. Recovered292.547eHz49131COM/49130acc,
IRQ54.960%,raw276bus10853,COMP65COM69commit45,guard21,stack2680.
ACQUIRELAT31/6/3/2/5vs36/5/3/2/5; SEEDLAT89vs94us. arm56cost13(24spare),
deadline144spare. N1 measured gain, not exclusive/WCET; seed1161vs1147.
Next lean build withoutacquiretiming, retain seed-div12 and allguards, qualify
before widerprofile. No more equivalent instrumented64cohort. Alloff/portsclosed.

E376 stagedNOTFLASHED6B71A8226E9964FF15287B97A3F5B19064CBCB61C23A79DFB66F15F2495464B3.
Actual283E,lastoffE373 unchanged. Read SEED_DIVISION_EXPERIMENT.md E376.
E375LLVM hoistedfallbackdivision unconditionally! Fixed cold/noinline opaque
fallback. Finalcode boundcheck0800b2ac/branchb2ae, MULb2b2/LSR18b2b4;
fallbackcallonlyb2f6. Const-derivedreciprocal/error/overflowasserts compiletime,
no runtime derivation. SEEDMATH/--seed-div12 strictprovenance added.
314Python/207Rust/release-s-thinLTO-codegen1+autoauditPASS. Text122160/data1120/
bss29324. Next disabledpreflights then one64recovery to measure gain; no
guard/profilechange, no measuredspeedup yet. Do not trustsourceconditionalalone.

E375 automaticM0link mathaudit added at operatorrequest. Read MATH_AUDIT.md.
.cargo target linker wrapper runs rust-lld then objdump -d -S -C; JSON/S
beside hashedELF, auditfailure blockslink, findingsadvisory/emptyallowlist.
Fullrelease rebuildPASS,313PythonPASS. Root148386B19BE2ABC77E8D33D3C8C015E1D7D0915BB73C4FC50589E47B6B95A8E1
NOTFLASHED; actual283E,lastoffE373 unchanged.102helpercalls84div/18wide64;
not totalwide-operation count. Every newM0link audited; cachehits no relink.
Continue E374 emittedfastbranch/provenance review, not assume visible
fallback division means fastpathfailed. No hardware this change.

E374 seed-div12 stagedNOTFLASHED1C974384B2E0C0F856BAFBC3B6D7BF0A6261EC6D0B5C7388AFD3DD69FF7BE6E9.
Actual283E,lastoffE373. Read SEED_DIVISION_EXPERIMENT.md. Baseline finaledge
__aeabi_uidiv at0800b286; optional exact (sum*21846)>>18 <=24000, otherwise
normaldivision. Exhaustive24001inputs+fallback test,207Rust/releasePASS.
Text122040,data1120,bss29324; includes acquiretiming. No measured gain yet.
Next emittedcode review + strict fixtureprovenance BEFORE flash/preflight/
matched64recovery. No guards/profilechanges. Fullgoalopen.

E373 installed283E5E7B timingprobe, one64recoveryPASS/finaloff. See
ACQUIRE_TIMING_EXPERIMENT.md. UARTresetclockrepair afterexactdisabledchecks;
priority02threePASS,guard01threefaults/18refusals,preflight01fivePASS CPU2/7/10.
Recovered292.833eHz49178COM/acc,IRQ54.785%,raw263bus10853,stack2680,
COMP65COM69commit45,arm49.5cost12,deadline150spare. ACQUIRELAT36/5/3/2/5us
edgequalification/clear/publish/return/entry totals51. SEEDLATtotal94vs91.
Qualification36 includesmandatory20dwell; no exclusive/removable claim.
311PythonPASSincl retainedcapture. Next inspect finalcandidate/edge checks,
notreportcopy/callback-only rewrite. Actual283E NOT archived957F; alloff.

E372 bench-acquire-timing stagedNOTFLASHED283E5E7B350D1AFB097507252AD6AEE3B8580997DD03B5B6CD3F333AE9645617.
Installed957F archived captures/reference/guardinstall_957f,lastoffE369 unchanged.
Read ACQUIRE_TIMING_EXPERIMENT.md. Recovery success-only fourclockreads;
ACQUIRELAT capture-only afterrun. Host strictoptional chronology/dwell/entry
checks;310PythonPASS, release-s-thinLTO-codegen1PASS --no-default-features.
Initial build omitted no-default and failed atomicfeatureconflict, corrected.
Text122016,data1120,bss29324. No measured probe cost/hardware qualification.
Next code/stack review, disabledpreflights then one64recovery and timingreport
requiring ACQUIRELAT; not higherduty/cohort. Baseline acquireframe508bytes.

E371 offline ownership/codegen audit, actual/root957F hashverified,lastoffE369.
SPEED_ARM_BUDGET.md E371 corrects recovery: awake but GATES OFF; initial
driven Transfer path is distinct. Early24k preparation is undone by bridge_clear
restoring10k. TIM6/reference/stats already staged. Reportpublication in ELF
080096e4..9718 directfields+16bytecycles, noextra64bytecopy; no borrow/inline
fix justified. Awake reacquire skips oldbaseline restore. No hardware/codechange.
Next quantify successful acquire epilogue/return within49us edge-to-entry,
before continuation/rendezvous redesign; mandatory20us dwell remains. Direct
callback retains large acquisitionframe on poweredstack and needs stackproof.
Do not stage guardauthority or mistake ENABLEawake for driven recovery.

E370 offline seed-arm boundary audit, installed957F/lastoffE369 unchanged.
Read SPEED_ARM_BUDGET.md. At observed91us seedage, candidate320/325/330/340
boundaries leave39/37/35.5/31.5us actualarm vs32floor.330only3.5spare;
340alreadyfails arithmetic, not proof of sustainedspeed/CPU ceiling.
Newreference-arithmetic regression plus existing admission refusals;206Rust
PASS. No profile/runtimechange or hardware. Next audit moving preparable
handoff work before finalqualifiededge, preserving livefeedback/ownership/
originaltimestamps and actualarm; stats staging alreadydone. Do not stage
Admission tokens or relax guard to rescue6.7%. Fullgoal remainsopen.

E369 same957F: hold65/66 PASS10s; reentry65/66 PASS original30s.
Recovered299.765/304.363eHz,IRQ55.283/55.490%,sigma29.211/28.785us,
COM65/66 counts50343/51115 equalaccepted. COMP65COM69commit45,stack2696.
Recoveryarm50/48us (18/16spare),cost12,SEEDLAT91us/guard15 both.
Runningraw278/247,bus10984/10769; hold66startupraw974/1200 guard (notamps).
All four fullfixtures/provenance/finaloffPASS, portsclosed. No guardchanges.
Next coordinated speedprofile/arm-budget audit using these preceding passes,
not more equivalent64/65/66 runs or a blind67 retry. Previous67 failure on
58B570 remains historical; lower handoff latency is not a cycle-jitter fix.
Currentcalibration/independentquality/fullgoal remainopen.

E368 same957F installed; guardinstall_reentry64_30s cohort now3/3PASS.
Added02/03, original30s deadlines and finaloff verified. CSV retains hashes.
Recovered292.728..292.870eHz,IRQ54.769..55.283%,sigma24.556..25.076us,
COMP65COM69commit45,raw267..290bus10817..10937,stack2696.
SEEDLAT49/10/7/15/2/8us=91 each; recoveryarmspare20.5..22.5us.
Startupraw675/449/519 distinct from running. Favorable measured handoff
repeatability, not WCET or steady IRQ-cost reduction. Alloff/portsclosed.
Next preceding65/66 hold/recovery before coordinated speedprofile decision;
no more equivalent64 cohort needed. Fullgoal/currentcalibration remainopen.

E367 actual957F6C5A1CD9F78CE818141263571500299B27C05A4492882A23E64556947F3F.
priority01UARTsilent/exactdisabledclockrepair;02threePASS,guard01threefaults/
18poststoprefusalsPASS,preflight01all5PASS CPU2/7/10.309PythonPASS.
guardinstall_hold64_01PASS292.860eHzIRQ55.055%,raw276bus10877arm72.5cost13.
reentry64_30s_01PASS292.870eHz49185COM/49184acc,IRQ55.246%,sigma25.076us,
raw267bus10817,COMP65COM69commit45stack2696,arm54.5cost12(22.5spare),
deadline187spare. SEEDLAT49/10/7/15/2/8us: guardstage15vs20,total91vs96.
N1 favorable latency,notWCET/cohort. Alloff/portsclosed. Next repeatability
then65/66 before speedprofiledecision. Read GUARD_INSTALL_EXPERIMENT.md.

E366 guard-install stagedNOTFLASHED957F6C5A1CD9F78CE818141263571500299B27C05A4492882A23E64556947F3F.
Private singleuse profile-specific Admission validatesfreshinputs inlegacyorder;
immediate in-place install insideexistingmaskedsection, publicationunchanged.
No globalprestaging/newunsafe;205Rust/306PythonPASS,releasePASS. Pre-marker
assembly callerstack52vs180,installer24byteclear/fieldstores,no fullguardcopy.
Maskedcostunknown;finaltext121628,data1104/bss29324. Actual58B570,lastoffE364.
Read GUARD_INSTALL_EXPERIMENT.md; next disabled inclguardrefusals then64hold/
recovery with --guard-install,notinline-guard. No higherduty untilqualification.

E365 offline borrowedOptionguard experiment rejected/REMOVED beforehardware.
RootAF4226C57A84C7C8202F6D799AC6C9739476C6B8C213BDE843D7594F26791A75
isNOTflashablecandidate(source reverted). Generatedtwo68bytecopies inmasked
installer plus76bytestack, baselineonecopy; caller180->188bytes,text+80.
Actual58B570,lastoffE364 unchanged. Next structural freshvalidation/inplace
installation audit, notmoreinlineflags/Optionpassing. Fullgoalopen.

E364 inlineguard7A6E hold64/reentry64 PASS but LATENCY WORSE: guardstage26vs20us,
armage102vs96us,actualrecoveryarm42us(10spare). Featureparked; RESTOREDactual
58B570 archive,restore_priority01threePASS/finaloff,noUARTrepair. Root7A6E
NOTinstalled. Candidatehold292.630eHzIRQ54.727%;recovery292.836eHz49180COM/
49179acc,sigma24.802us,raw264bus10889,COMP65COM69commit45stack2696.
No higherduty/repeats. Read GUARD_CODEGEN_EXPERIMENT.md. Smallerassemblyframe
didnotbuytiming; next inspect actual guardcopy/start work or ownership-safe
prestaging, notmore inlineflags/67lottery. Fullgoalopen.

E363 inlineguard stagedNOTFLASHED7A6E96AA0382BB0AD8BE69D5D1DBAE4AD67604402CCBA61687D29F9C8B9258B9.
Installed58B570 archivedhashverified captures/reference/staticcomp_58b5,lastoffE361.
bench-inline-guard only forces with_limitsinline; fresh admission/safetychecks
unchanged, no prestaged ownership.203Rust/305PythonPASS,releasePASS,text121668
(-88vsbaseline),data/bssunchanged. Preliminarycode localstack180->132bytes;
copy/clearstillpresent, no latencygain measured. GUARDCODE/--inline-guard strict.
Read GUARD_CODEGEN_EXPERIMENT.md before flash; next disabledpreflights then
64hold/recovery withstagecomparison, not67retry/profileincrease. Fullgoalopen.

E362 offline seed-stage audit, nohardware,58B570installed,lastoffE361.
ExistingSEEDLAT inrecovery66 yields brackets49/10/7/20/2/8us,sum96us actual
edgeage. Biggestpostentry bracket guardstartup20us, referenceonly2us. Dwell20us
isinside49us and MUSTremain. Timingreport now validatesoptional stages,304tests
PASS; see TIMING_HEADROOM.md. Statistics ALREADY staged; next inspect
RuntimeGuardconstruction for immutable prestaging with all fresh livechecks,
not repeat staging or weakenarmfloor. No current/fullgoalcompletion claim.

E361 same58B570 staticcomp_hold67_01 FAILCycleTiming12at304442us,555COM/554acc.
Exactstep4guard301275->304399=3124us<3125. Refcycles6486->6243halfus,
pairmean3182.25us notrotorproof. ShortfailedwindowIRQ54.769%,COMP69COM68
commit45,raw212bus11140stack2696. Alloff/portclosed. No higherduty/retry.
302PythonPASS incl exactone-us-refusal regression. Optimization reducesload
butdoesnotremoveboundary; no CPU saturation/physicaloverspeed causeproven.
Next acceptance-timing/recovery-latency audit, not threshold relaxation.

E360 same58B570. staticcomp_reentry65_30s_01PASS299.907eHz50367COM/acc,
IRQ54.911%,sigma29.081us,raw295bus11032,arm44.5us(12.5spare),deadline209spare.
hold66_01PASS10s304.289eHz18258COM/18257acc,IRQ55.387%,sigma40.315us,
raw239bus10960,arm67.5cost12. BothCOMP65COM69commit45stack2696.
reentry66_30s_01PASS304.197eHz51088COM/acc,IRQ55.512%,sigma28.726us,
raw270bus10865,arm43us(11spare),deadline120spare,COMP78COM69commit45stack2696.
Retain COMP78 outlier, not fixed65us ceiling. Alloff/portsclosed. No guards
changed. Next bounded67boundary test with existing3125/260 profile, preserve
failure not automatic profile raise. Calibratedcurrent/fullgoal remainsopen.

E359 same58B570 installed,64recovery cohort now3/3 original30sPASS. Added02/03,
CSV captures/staticcomp_reentry64_30s_cohort.csv retains hashes/fullverify.
292.772..292.969eHz,IRQ54.818..55.199%,sigma24.799..25.108us,COMP65COM69
commit45,raw271..287,bus10889..11116,armspare17..18us,stack2696.
Then staticcomp_hold65_01PASS10s299.738eHz17985COM/17984acc,IRQ54.858%,
sigma39.685us,raw232bus10793,arm67.5cost13,COMP65COM69commit45stack2696.
Alloff/portsclosed, no guard changes. Next65recovery before66, not more64
cohort runs. Currentcalibration/independentquality/fullgoal stillopen.

E358 actualinstalled58B570E06486AE16FB868AF83AFDEFCBED8A8BB270214623266E95C21E1CCA0B.
StaticComp loop reviewed liveCSR08001c90/no per-read modebranches. priority01
UARTsilent/exactdisabledclockrepair;priority02threePASS/preflight01all5PASS.
staticcomp_hold64_01 PASS10s292.764eHz/IRQ54.782%,sigma34.336us,raw279bus11151,
COMP65COM69commit45,arm68/cost13stack3356. reentry64_30s_01 PASS original30s,
292.772eHz49168COM/acc,IRQ55.199%,sigma24.799us,raw271bus11116,stack2696,
seed1160/acq7474us,arm49us(17spare),deadline157spare. Alloff/portsclosed.
One hold+one recovery, NOT cohort/higherduty qualification. See experiment doc.
Next repeatability then revalidate65/66 before67boundary; no guard changes.

E357 bench-static-comp staged NOTFLASHED58B570E06486AE16FB868AF83AFDEFCBED8A8BB270214623266E95C21E1CCA0B.
Read STATIC_COMP_EXPERIMENT.md before acting. Existing cached adapter retains
per-read mode branches in release assembly. New specialized real/inverted/
traceoff comparator calls same live CSR helper with constants; original shared
AM32 routine and all safety methods remain. Fallback retains diagnostics.
Text+492 vsarchive,data/bssunchanged,releasePASS/301PythonPASS. No measured
gain; shorter physical read aperture requires qualification. Installed28DA,
lastoffE355 unchanged. Next generated-code review then disabled preflights,
matched64hold/recovery, never higherduty before qualification.

E356 offline architecture audit/report refinement, no hardware. Installed28DA,
lastoffE355. Three fully verified archived instrumented captures: closed-gate
14.825/17.455/17.475%, open-no-accept73.627/73.132/73.082% ofdispatched.
NotCPUfractions/current-buildpartition. Pre-gate deferral alone leaves most
visits. bench-inline-comp ALREADY implies cached-comp; don't rediscover cache.
Report emits nullable decision fractions plus explicit notCPU flag;300testsPASS.
See ARCHITECTURE_BUDGET.md E356; next target open-gate work/sensing, preserve
raw persistence and pending/acceptance timing. Fullgoal open.

E355 restored actual28DAC42D archived ELF, HYST0 CSR40000281. Root ELF779C
is NOTinstalled. Initial hystzero_restore_priority01 UARTsilent; exactsafe
register checks then USARTclockrepair, priority02threePASS/preflight01all5PASS.
hystzero_hold64_01 PASS10s292.576eHz17555COM/17554acc, IRQ58.774%,sigma38.048us,
raw267bus10901,COMP71COM69commit45,entryarm65.5us/cost12,stack3300,finaloff.
Acquisition14accepts,seed1540ticks,raw500,bus11557. Known-good stillpasses;
one paired lowHYST refusal does not establish causality or sustained A/B.
LowHYST startup experiment parked; no guards changed. Next architectural
sensing-work audit on qualified baseline; avoid repeated hysteresis entry lottery.

E354 actualinstalled779C8978 lowHYST candidate, NOT BEMF qualified; finaloff.
CSR40010281 confirmsHYST1; priority01UARTsilent/exactdisabledclockrepair,
priority02threePASS/preflight01all5PASS. hystlow_hold64_01 FAILED acquisition:
reason2/window20011us,22IRQaccepts,seedfault3,one reanchor epoch4.
First fatal epoch4->5 gap4145->5264us=1119us exceeds unchanged1000us limit;
failure latched before window deadline, later regular edges cannot revive it.
Raw821/1200,bus11366,alloff. Not sustained jitter/CPU comparison; n1 does not
prove hysteresis caused refusal. Rust exact-prefix replay/Python fullcapture
regression added. Next controlled archived28DAC42D baseline comparison, not
seed threshold relaxation or identical retry lottery. Read experiment doc.
Prior E353 installed/staged claims below are historical, superseded here.

E353 hysteresisA/B staged NOTFLASHED; candidate779C8978BD2FA478D78B91FAFA73F7689E4080C9B11E7014FBF640F827A6CFC4.
Actualinstalled28DAC42D,lastoffE352 archivedreference/range320_28da hashverified.
AuditbootHYST0, PAChyst16:17/HALlow1; STdatasheettyp10mVlow. Newfeature
bench-comp-hyst-low bootbit16, startupAND BEMF, noISR/readcount/corechange.
COMPHYST actualreadback/--low-hysteresis strict;299Python/releasePASS.
Read COMPARATOR_HYSTERESIS_EXPERIMENT.md beforeacting: thresholdshift tradeoff,
notprovennoisegain. Next disabledCSR+preflights then64hold/recovery comparisons,
allguards unchanged. No hardware actionthisentry, no higherdutyqualification.

E352 same28DAC42D. range320_reentry66_30s_01 PASS304.198eHz51088COM/51087acc,
IRQ58.827%,sigma32.018us,raw307bus10984,arm43us/cost12,deadline210spare,
stack2696,COMP71COM69commit45,finaloff. Thenhold67_01 FAILCycleTiming12
at191465us,344COM/343acc,raw215bus11343,COMP68COM67commit45stack2696.
Exactstep4guard188315->191426=3111<3125; pairedrefcycles6658->6216ticks,
mean3218.5us notrotorproof. Alloff/portsclosed. No higherduty/recoveryafterfail.
OPERATING_ENVELOPE.md consolidates build/profile-specific passes AND failure.
Next timingvariation/recovery-margin investigation, not anotherautomatic
profileincrease or equivalent67lottery. Actualarmfloor stays32us; goalopen.

E351 same28DAC42D installed. range320_reentry65_30s_01 PASSoriginal30s,
299.969eHz50377COM/acc,IRQ58.758%,sigma32.794us,raw280bus10829stack2696,
arm44.5us/cost12 (12.5spare),deadline199spare. Thenhold66_01PASS10s304.222eHz
18253COM/acc,IRQ58.648%,sigma43.055us,raw242bus10877arm67.5/cost13stack2696.
BothCOMP71COM69commit45,alloff/portsclosed. Next66recovery beforehigherduty,
no thresholdchanges. AsyncPSUreading requested during65run; none received.
Initialhold65 baseline residualsum2.516584rawcounts/49751scans, NOTamps and
initialbaseline cannotapplytorecovery. Currentcalibration/fullparitystillopen.

E350 installed28DAC42D range320, priority01UARTsilent/exactsafeclockrepair,
priority02threePASS/preflight01all5PASS.298PythonPASS. range320_hold64_01
PASS10s292.493eHz/IRQ58.480%;reentry64_30s_01PASS292.787eHz49171COM/49170acc,
IRQ58.662%,arm48us/cost13,deadline188spare,raw289bus10948stack2696.
Thenhold65_01PASS10s299.971eHz17999COM/17998acc,IRQ58.375%,sigma42.460us,
COMP71COM69commit45,raw279bus10865,arm68.5cost12stack2696,alloff/portsclosed.
New3125/260 profile confirmed; not crossingjitterfix, not320eHzqualified.
Next bounded65recovery beforefurtherduty. Fullgoalcurrent/parityopen.

E349 optionalbench-range320 staged/built NOTFLASHED. Candidate28DAC42D32D6B6D321B47106AA1F6F8A4FE468D4771CA8BB20A25B7D5CAB7074.
InstalledBA1F,lastoffE348 unchanged. Profile runtime3125/260us, acquisition
6250/520halfus,seedmin1041. Otherprofiles unchanged; actualarm>=64halfus,
current/bus/age/tracking/deadline unchanged.202Rust/298Python/release-s-thinLTO
PASS; tests refuse age197 atci1041/wait260,accept196 exactly32us; underspeed
cycle/profile mismatches refuse. This is authorized incremental envelope
exploration after64cohort, NOT jitterfix or qualified320operation. Next flash
disabledpreflight then same64hold/recovery before65; allfailures retained.

E348 sameBA1F installed, fixedpeerexti_reentry64_30s cohort3/3PASS (02/03
added); CSV retainsallhashes. Recovered292.413..292.513eHz,IRQ58.368..58.738%,
sigma28.875..29.166us,COM69COMP71commit45,stack2696; arms47..49.5us,
spare15..17.5us,deadline190..251spare,raw275..280bus10841..10948.
Startupacquisitionraw1147 in02 close to1200 guard; others547/686. Notamps,
don't conflate lowrecoveredcurrent with startupmargin. Alloff/portsclosed.
No more same64cohort needed. Next timing-envelope audit/controlled profile
decision, not repeated65lottery or more meterwork. Independentactualarmfloor
muststay32us. Current source/root648191D9 remainsNOTinstalled. Fullgoalopen.

E347 sameinstalledBA1F, peerexti_hold65_01 FAILED CycleTiming12at3340816us
of10s. Mean299.233eHz5998COM/5997acc,raw247bus11151,COMP70COM69commit45,
stack2696,finaloff/portclosed. Exactstep5guard3337566->3340777=3211<3226.
Referencecycles6865->6416halfus,pairmean3320.25us; notphysicalrotorproof.
Priorityfix didnotremove cycleboundary; no higherduty/recovery attempted.
297PythonPASS inclrealrefusal regression; no firmware/guardchange.
Next consolidate repeatability at64 or source-grounded timing-envelope audit
before any coordinatedrange expansion; don't relaunch65lottery or blameCPU
floor. Root648191D9 NOTinstalled, knownBA1F remainsactual. Fullgoalopen.

E346 RESTORED actualmotorBA1F982B archivedELF, restorecpu01PASS2/7/10/noUART
repair. Root rebuilt current union source hash648191D9 is NOTinstalled; source
line/cfg changes exist, don't conflate with archivedBA1F. peerexti_hold64_01
PASS10s292.122eHz17528COM/17527acc,IRQ58.711%,raw264bus10913stack3300.
peerexti_reentry64_30s_01 PASSoriginal30s/recovery292.413eHz49108/49107,
sigma28.875us,IRQ58.738%,COMP71COM69commit45,raw275bus10948,stack2696,
recoveryarm49.5us (17.5spare),deadline251spare. Alloff/portsclosed.
Startup62/BEMF64 independent, guardsunchanged. n1hold+n1recovery; no repeat
cohort/currentcalibration/fullrange claim. Next bounded65hold on installed
BA1F beforemorequalification; historical65 hadcyclefloorfailure, retainany
failure and exactguardtimestamps, no automaticguardraise. Meterworkparked.

E345 installed70AB2CCDD1074F3C15F14A351BC5C4257AF686EDCF4A9524B9AA546BA2F55560
is DIAGNOSTIC ONLY, no motor qualification. Newbench-cpu-roots labels outer
union intervals by root handler INCLUDING nested work; CPUROOT/CR85 protocol
validated separately, notexclusive.200Rust/296Python pass. InitialDA0937BF
CPU nestedmax11 failed10us gate; folded redundantunion update into stop-only
elapsed-minusFG calculation, latestrootirq_fold_cpu02 STILL fails overhead.
BothflashesinitialUARTsilent/exactsafeclockrepair; allfinaloff/portsclosed.
No motor run. KnownmotorBA1F982B archived captures/reference/peerexti_ba1f.
Do not power this diagnostic until overhead gate passes; no thresholdchange.
Next review measured root bookkeeping cost or return baseline/range work,
not repeated identical probes. Fullgoal active; exactCPUattributionunmeasured.

E344 per-vector timing candidate973853D2 installed but DISABLED CPU gate FAIL:
exclusive_cpu01 maxima2/6/12us, nestedmean11.75>10usmax gate; no motor run.
Reproduces E233 limitation, don't repeat instruction-level tuning/loosen gate.
Restored archived knownBA1F982B (captures/reference/peerexti_ba1f/shell-pwm.elf);
restorecpu01UARTsilent/exactsafeclockrepair,02PASS2/7/10,finaloff/portclosed.
Root rebuilt union configuration; verify hash before future flash. No firmware
source change. Exact exclusive attribution remains unmeasured. Next possible
lower-cost attribution uses existing outermost union boundaries labeled by
root handler, INCLUDING nested work; must be a distinct protocol, not called
exclusive. No extra nested timer reads; overhead still requires qualification.
Do not let instrumentation repeat loops replace range/current goal progress.

E343 sameBA1F982B installed, peerexti_reentry62_30s_01 PASS original30s with
injected loss/fresh recovery, resumed280.473eHz47102COM/accepted. IRQ58.898%,
COMP71COM69commit45,sigma33.602us,raw266bus11068,stack2696,deadline198us
spare, recovery arm54us (22abovefloor), finaloff/portclosed. n1recovery.
Hostarchitecture report now supports existing per-vector meter with exclusive
software-boundary time/calls/means and explicit observer/exception limits;
295PythonPASS. Firmware unchanged. Next characterize per-vector mode by
replacingbench-cpu-union withbench-cpu-timing, disabled overhead gate FIRST;
do not blindly use filter_preflight.py which requires union. Preserve known
ELF before build; no attribution from current union captures. Goal stillopen.

E342 installedBA1F982B00D90E5AA103D90A07172794F3C08CE4E69F424C9E7826BCD235D4AE.
SameE341features minusbench-filter-bypass: control uses leanEXTI, source
diagnostics remain compiled. COREPRIORITY reads actualADC_COMP (TIM2 only
whenfilter-control). priority01UARTsilent/exactsafeclockrepair;02threePASS;
preflight01all5PASS,294Python/release-s-thinLTO PASS. peerexti_hold62_01 PASS10s
280.447eHz16827COM/acc,IRQ58.871%,COMP71COM69commit45,raw271bus11092,
arm65.5us/cost12stack3300,readback64/64/0/0,finaloff/portclosed.
VersusE340TIM2hold63.562%,COM75: lower aggregate cost despite18757vs16874
COMPvisits/s. n1hold, no recovery on newbuild. Next bounded recovery or
existing per-vector timing characterization; preserve guards/no duty expansion
without preceding timing/current evidence. Fullgoal open, current notamps.

E341 same EF4BFEC6 installed, peerprio_reentry62_30s_01..03 fixed cohort3/3
PASS original30s/injected tracking loss/fresh recovery/finaloff. Recovered
279.824..279.962eHz,46993..47016COM, accepted equal or one fewer;
IRQ63.305..63.446%, COMmax75us all, COMP73/73/76,commit45,stack2628.
Recovery arm spare above32us floor3.5/2/2us remains tight; no duty expansion.
Raw264..268,bus10686..10960mV,deadline101..200us spare.294PythonPASS.
Cohort CSV retains hashes. No firmware/guard changes; port closed.
Next lean EXTI peer-priority comparison (fix COREPRIORITY selected-vector
readback first), then existing irq_accounting per-vector mode if needed;
measure instrumentation overhead, do not treat IRQ union as total CPU/idle.
Full range/current/parity goal remains open; no more equivalent cohort needed.

E340 installedEF4BFEC6B91B872A5D9D7FBD003364CB5687E22900C99F5028F3593AA18D7429.
bench-com-peer COM0x40=sensing0x40, guard/DMA0. Disabledpriorityprobe3trials
provesnestedold123 vspeer132/restored. check01UARTsilent/exactsafeclockrepair,
check02PASS;source01/preflight01PASS.292Python/releasePASS. peerprio_hold62_01
PASS10s279.645eHz16779COM/16779acc,IRQ63.562%,COMP75COM75commit45 (priorCOM258),
raw258bus11092arm50.5us/cost12stack3196,readback64/64/0/0,finaloff/portclosed.
Promisingworstcaseimprovement,n1 notCPUgain (.17pointdifference). Next longer/
recoveryqualification andcarrypeerprioritytoleanEXTI foroverheadcomparison;
sourcebypasslatestprofile, nofilteredlockclaim. Guards/referenceunchanged.

E339 noflash/motor, installed034C6E94 unchanged,lastoffE338. Added3actualminz
counterexampletests: frozenCCRcount canneveropengate; heldcapturelevel falsely
acceptspersistenceflip; timestampreplacementchangesestimatorbutnotCOMdeadline.
199RusttestsPASS. Sourceaudit found referenceam32_clone.rs1080-1086 COMP/COM
equalprio0; binz observe_irq_start COMP0x40/COM0x80. COMenablesCOMP nearend
beforezero_crosses increment/return; pendingCOMP canpreempt binzCOM butnot
reference. Next isolated equalpeerpriority0x40 experiment on unfilteredsource,
guard/DMAstay0. Need disabledprioritybehavior/readback and knownpoint comparison;
notyetimplemented, notproven258usoutliercause orCPUgain. No capturedlevelshortcut.

E338 installed034C6E94F96624F0447F4A3BFD490FBED4A1AE82F99AB25535B168A15C181584.
bench-filter-latency mirrorsCH1/filteredTI2; prefix24atfirstactualrawread,
phase/reset/stopinvalidate, noCC2reads/authority.290Python/releasePASS;
source01 3PASS/preflight01 5PASS/noUARTrepair. filterlat_trace62_01 FAILtracking8
2040us3COM/2acc/15IRQ,raw127bus11581COMP79COM283commit25stack2196,finaloff.
15mirrorrows matchactualrawfirst(invertedref) all15. Validlatestcaptureages
2.5..40us; step6 35/23/19.5/40/29/23.5us; rawbracket.5..1us. Observeradds
latency andovercapturemeanslatestnotoriginalIRQ. Next timing-awarequalified
capture design/replay; don'ttreat delayedrawpollas equivalent filteredstate.
NoCPUgain/noqualifiedfilteredlock; portsclosed, fullgoalstillopen.

E337 installedB3997A140A8ECCCD4EDA7E29482CD6219A5976DEA518DBD9DB11DD5027A65B61.
DisabledFILTERACK proves CCR1 mirror read preserves authorityCC2IF: captured
casesflags6->4->0 (CCR1thenCCR2), rejected0->0->0 all8pulsecases.287Python/
releasePASS. filtermirror_ack01UARTsilent/exactsaferegistercheck/knownclockrepair;
ack02PASS/restored/off,portclosed. No motorrun orlive mirrorhook. Next bounded
capture-to-first-raw-read trace via CH1 samefilteredTI2, neverCCR2; phase/reset/
stop invalidate timestamps, reportovercapture/latestedge and observercost.
NoIRQreductionclaimed; E336bypass remainslatest10spoweredqualification.

E336 installedC8E4236D1CC929EFC16730C99FE3017FAF28798D2F3F5ED6512C2ECF2AB434F7.
bench-filter-bypass code0 sameTIM2adapter/earlyarm; conflictswithone-us.
source01 3PASS/preflight01 5PASS/noUARTrepair. filterbypass_hold62_01 PASS10s
279.549eHz16774COM/16773acc,IRQ63.732%,17084visits/s,COMP73COM258commit45,
raw253bus10865,arm48.5us/cost13,stack3264,refusals0,finaloff/portclosed.
Unfilteredadapter sustains; code5/12failed tracking. NotCPUgain (priorEXTI~59-60%).
Next timing-awarearchitecture investigation: filterdelay changesrawlevel at ISR
sampling; do notcontinue longerfiltertuning orclaim siliconfloor. Baselinebypass
now measured, recoveryunqualified; preservefullgoal and rawacceptanceguards.

E335 installedBEC723805FF65BB922F7F3B6B9A39C36325EF255174AECB6785E40DA1A4FB23C.
bench-filter-one-us selects5/CKD2,earlyarm0. Pulsecheck now8rows incl5; code5
capturesrequested2us(measured3) with3halfusticks=1.5usdelay; code12rejects.
286Python/releasePASS,source01 3PASS,preflight01 all5PASS,noUARTrepair.
hold62_01 FAILtracking8 at10140us15COM/14acc110IRQ; trace62_01 FAIL2039us3COM/2acc
15IRQ. Trace15rows:7closed,2accepted,6openreject; last5 firstreadwrong,final
read3 reverses0->1. STOPcaptureenabled/noCC2pending/rawhigh;off/portsclosed.
Code5restoredevents butnotlock/CPUwin. Next code0TIM2 control toseparate filter
delay from newadapter latency before morefiltertuning. Allrawchecks/guardsstay.

E334 installed4AD6E3D7FE5A540F3EA8664727478B3098E17086DD5D1FF43B77751C6836B5E3.
bench-filter-early-arm removesONLY10us softwarewait; code12/CKD2 unchanged.
source01 3PASS/preflight01 all5PASS. filterearly_trace62_01 FAILtracking8 at1040us,
1COM/0acc/0IRQ,raw184bus11617COM70commit44,arm101ticks/cost13,stack3832,off.
STOPsr27/dier4/ccer48/cr1513/ccmr49408/tisel256/cnt2276,armed/enabled/NVIC1,
pending0,rawlow(fallingselected). Earlyarming alone didn'tfix. Important next:
24k62/1000 CCR165ticks=2.578us (139ticks=2.172us timerhigh afterDT26), filter12
needs448..512ticks=7..8us. ON-onlyvalidpulses wouldbeerased. Notyetprovenactual
pulsewidth. Testcode5/CKD2 (56..64ticks=.875..1us) disabledpulsepassfirst then
bounded samepointmotor, preserve rawpersistence/guard. Puredurationtestadded,
196RusttestsPASS. NoCPUgain, noqualifiedfilteredlock; portclosed.

E333 installedAB53C5E3F5004E38A5233EA5C899774CCD99A20BF893B78589AD34C654B4C003.
FILTERSTOP snapshots12source fields before backendmask/clear, noCCRread.
Initialdisabledrestorationfailure isolated to liveCOMP VALUEbit30 (timerdiff0,
csr40000281->281), configcheck now excludesONLYbit30 withhosttests; original
failedcaptures retained. 285Python/195Rust/releasePASS. fixedsource01 3PASS,
filterstop_preflight01 all5PASS. trace62_01 FAILtracking8 at1039us,1COM/0acc/0IRQ,
raw146bus11605COM80commit24stack3808,finaloff. STOPsr27(CC2IF0),DIER4,CCER16,
CR1513,CCMR49408,TISEL256,armed/enabled/NVICenabled1,pending0,rawhigh.
Actual absenceofcapture, not deliverydisabled. Next compare immediate capture
arming aftermux vs new10us disabledsettle while preservingcode12/CKD2 andall
guards. Highatstop doesn'tprove priorlevelhistory. Portclosed,noCPUgainclaim.

E332 installed955AB08BB5EA9B92E900DB51A6B11D6AD47D337E7F63C5CBE57BABA08AF9165D.
bench-filter-control routes selectedInput + core dispatch + TIM2 to actualcore,
startup remainsEXTI. Adds10us masked muxsettle, preservedCNT/current rawreads,
phaseCURRENT ticket, selected-source stop/panic; no observer. Build/replaypass,
filtercontrol_source01 3trialsPASS, preflight01 all5PASS. TWO POWERED FAILURES:
hold62_01 tracking8 at1339us 2COM/1acc/7IRQ; trace62_01 at1040us 1COM/0acc/0IRQ.
Both selectedprepared1/refusals0/finaloff, portclosed. Do not qualify this build
or inferCPUgain. Next snapshotTIM2SR/DIER/CCER/CEN/filter/CNT atstop BEFORE clear
and investigate10us muxsettle/arming suppressing capture; tracehadzeroIRQ so
not persistence rejection that attempt. No guardchanges, broadergoalopen.

E331 installed54E95D70F514C13AD7BBD55102D8F99A2AF72981D5FA29E221443A6B665DA739.
Disabled filtersourcecheck actualTIM2 vector/backend passes3trials10checks,
2IRQvisits each: maskedpending retained/replayed then explicitclear, UIF
preserved, oldphase/stop tickets refused, latehandler safe, timerrestored.
filtersource_01 UARTsilent retained; exactsafe registers verified then known
UARTclock repaired. filtersource_02 passes finaloff, portclosed. 281Python
tests/releasebuild pass. No motor run or control-source hookup yet. Next wire
optional selected source through controller/Input/setup/stop/recovery; preserve
legacy driven EXTI and independent guards. CPU savings unmeasured.

E330 staged filtered_irq_hw PAC backend under bench-filter-source compiles;
no runtime callers/vector/controller hooks, no flash or motor. Preserves timer
timebase, explicit rc_w0 CC2 clear, masked pending retention, phase tickets,
refusal counters. Source+observer combination intentionally compile-errors.
Replay163 unit +31 integration pass, M0 library/release firmware checks pass.
Root ELF hash remains58E0080B; last hardware offE328. Next disabled TIM2 vector
diagnostic using actual backend before controller hookup. Read updated
FILTERED_IRQ_DESIGN.md. Occupancy gains remain unmeasured, guards unchanged.

E329 filtered_irq_source lifecycle +3 actualminzcomp_isr testsPASS. Strictgate
equalityretains postpending; prelevel/persistenceflip clearwithoutCOM; valid
accept currentCNT/reset/onearm; stalephase/masked/noedge don'tarm. See
FILTERED_IRQ_DESIGN.md hardwareobligations. No firmwarehook/flash/motor;
installed58E0080B,lastoffE328 unchanged. Next implement optionalTIM2source
adapter inclcorehardwaremaskdispatch, preventobserverCC2flagconsumption,
disabledpending/mask/clear/stop tests before powered use. No guardchanges.

E328 installed58E0080BEEF6758398AF33961A82E915E933A1F79BD7AD50F787CC732AF23417.
Rawcapture afterADCconfig iff observeractive; epochmux/reset/stop,first8raw
misssnapshots ageexplicitmod201. 277Python/releasepass. rawlive_preflight01
all7PASS,hold62_01PASS10s279.641eHz16779COM/16778acc,IRQ59.698%,
COMP76COM266commit45,arm59.5cost13,raw268bus10937stack3184. Filter16520/
16778,258miss,0over. 8/8missrawready/captured/over1,agemod29/6/4/1/0/1/1/2us;
prefix3raw_after flippedback(expected0 actual1). Softwareacceptnotcleanoracle.
201usaliasstilllimitsages. Alloff/noUARTrepair. Next timing-awarefilteredIRQ
design preservingreferencegate/COM semantics, not100%softwarecapturematching.
COM266walloutlier unresolved; noauthorityswap yet.

E327 installedF3CEBD1BA7F6B62159B7F7EEEA8B27CD259435B25CC386576E4826CC5C88767A.
rawadccheck realfivechannelADC+phaseDMA coherentlypolled ENABLElow,128raw
pulses/scans pertrial. rawadc01PASS3trials25662/25663/25662us,VREFmin1503,
noDMAfault,timerconfigpreserved,finaloff. 276Python tests/releasepass.
NoUARTrepair/no motor. Next connect rawcapture afterADCinit with observer
epoch/mux/reset/stop, preserveDMA/trigger and labelagemodulo201us. Disabled
ADCcoexistence proven, not motorISRload or captureauthority. Checkflashheadroom.

E326 installed51F1B6E54E5189FD76D85C6AED0568A750D8E809ACB919A8D61D8F54B56D8CC0.
bench-filter-raw adds TIM3CH2helper and disabledadcphasecheck extension,
notlivehooked. rawcoexist01PASS3x32captures alongsidephaseDMA32updates,
maxcounter0/0/0,PSC/ARR/CR1/CR2/DIER preserved,finaloff,noUARTrepair.
Hostverify strict3rows/rawcounts andmutations;275Python tests. No motor.
Next actualADCscan coexistence then rawage capture integrated afteradcinit;
helper currently lacks sessionepoch and must not be called by stalecallbacks.
NoADCload qualified yet. Main control/filterobserver unchanged.

E325 installed6B8696F29E7B2082A4E2BFA7C836A042CF8D780F398ACF3439E5EFCA75150B0B.
DisabledTIM3CH2 COMP2filter0 atPSC63ARR200 independently capturesall6pulses,
includingTIM2filter12/15shortrejects. rawlatency1us each, restore10TIM3regs
inclCNT/CCR2 pass. filtert3_01UARTsilent/exactsafeclockrepair;02PASSfinaloff.
274Python tests/releasepass,no motor. adc_stream initializesTIM3CCER0,
CR2MMS2,PSC63ARR200; phaseDMA ownsUDE/ch3. Next live coexistence test must
preserveADCcadence/DIER/DMA, attachafterinit, rawagemodulo201us notabsolute.

E324 installedB58B250AE8FAE45CC3ED0BD7F39AB4AAD8EE4F20B4A7CCFFD674890DB88E7A61.
Disabledfilterpair02 proves CH1indirectTI2 sharesTI2filter: IC1F0 but short
pulses rejectedwithCH2 and captured CCR1==CCR2 for0/15/12. Notraw/filtered!
pair01UARTsilent retained/exactsafeclockrepair;02allgates/finaloffPASS.
273Python tests/releasepass, no motor. Next TIM3CH2 COMP2 route confirmed
ST RM0444section22.4.29 TI2SEL1; TIM3alreadyADC201uscounter, preserve
PSC/ARR/CNT/SMCR/DIER and DMA. Rawage wouldbemodulo201us, cannot claim
fullinterval age. Need disabled independence/timerpreservation test first.

E323 installed0F4629972F4B489B59A448E9898B69AD7CA68218258FCB0AC6C6B72A2DFFBC72.
Sectorcounts+first8missprefix added,272Pythontests/releasepass. Preflight01
UARTsilent/exactsafeclockrepair;02all5PASS. filtermiss_hold62_01PASS10s
280.304eHz16819COM/16818acc,IRQ59.716%,COMP75COM86commit45,arm46cost13,
raw286bus10793stack3628. Captures16539/16818,0over,279misses acrosssectors
71/23/57/11/82/35. First8raw_after==expected,armage323..415.5us; notlatearming
or isolatedphaseabsence, no proof of continuouslevel/pulsewidth. Alloff.
Next paired raw/filteredhardwarecapture timestamps to distinguish qualification
delay; audit TIM2CH1indirectTI2 route before use, no addedIRQ/authority.

E322 installed125B3C24C5FF6B25941F55F5E195F2920D2E5C4D46EEFCE6A261D0DBBEBC7D28.
bench-filter-short code12CKD2 observeronly. Pulsecheck now6rows0/15/12;
code12 rejects2us,capture40us delay16half-us ticks=8us. Preflight01UARTsilent
retained/exactsafeclockrepair;02all5PASS. 271Python tests/releasepass.
filtershort_hold62_01PASS10s280.324eHz16820COM/16819acc,IRQ59.059%,
COMP74COM86commit45,arm62.5cost14,raw241bus10853stack3892.
FILTEROBS16819samples/16499captures/0over=98.10% vsE32188.81%,320missing.
Finaloff/portsclosed. Supportsfilterdurationaffectsavailability, notqzc/CPUgain
orreadyforauthority. Next localizemissing events andcapturetiming, sameguards.

E321 installed81572D65E294CEA12B11CDCA47564357820F212424C9426650EB99F1831BD2CB.
irq_union lazy clock reads after validation; samefault/nesting/elapsed guards,
tests pass. Preflight01UARTsilentfail-fast retained/exactsafeclockrepair;
preflight02 all5PASS,CPU nestedmean9.46875/max10 vs10.0/max10prior.
filterclock_hold62_01 PASS10s280.137eHz16809COM/16808acc,sigma39.047,
IRQ59.551%,COMP74COM86commit45,arm62cost14,raw283bus10865stack4004.
FILTEROBS16808samples/14927captures/5overcapture,lag0..684ticks,alloff.
No overallCPUgain or old265us causeproof. Next investigatecapturemissing~11%
with filter-delay/sector-timing evidence; NOTreadyforauthorityswap. n1hold,
recoveryobserverstillunsupported. No thresholdchanges.

E320 installedBASELINE A1E963685400C670FCBCEF4BEEF2E9D0A2AB9257BAAE091056E20E406BED38A9
same source/features minusbench-filter-observe. Observer5D7C archivedELF
captures/reference/filterobs_5d7c/shell-pwm.elf hashverified. Baselinecpu01/02
UARTsilent retained; exactRCC08000000/PD1zero/BDTRc1a/CCRzero thenknownUART
clockrestore08040000. Baselinecpu03/04 PASSnestedmean10/max10, not evidence
observer caused01max11. Scope drop functions same addresses/code exceptMETER
literal pointer. Allfinaloff, no motor. Next finer timing or genuine accounting
cost reduction, notloosen10us; originalobserverbuild remainsunqualified.

E319 disabledCPU repeats filterobs_cpu02..05 allPASS unchanged5D7C2355,
mode2mean10.000/max10 vs01mean10.125/max11. Originalfailure retained,
not fixed/qualified by retry. cpu_meter::check invokes software scopes only;
no filter observer hook in bracket. Cause of first-repeat difference unproven,
possible timestamp/code-layout/interrupt effects require controlled evidence.
Preflight tests prove stops atCPUfailure, never commands motor. Boardfinaloff.
Next disabled matched baseline comparison or high-resolution bracket BEFORE
furtherpoweredruns. No thresholdchange, no COMmax265 explanation yet.

E318 installed5D7C23555C3249924DE9D03E1957A73546AEF5E6B8FAAAF2F49B3F53951F014B.
Disabledpulse/atomic/roles/archivePASS but CPUFAIL mode2max11>10. Sequencing
error launched hold despite this; retain as exploratory NOTqualified build.
filterobs_hold62_01 fullpoweredverifierPASS10s280.027eHz,16802COM/16801acc,
IRQ59.318%,COMP75/COM265/commit46,arm48.5cost14,raw287bus11056stack4004.
FILTEROBS16801samples/15063captures/2overcapture,lag1..263ticks,stopped.
Finaloff verified portsclosed. Next diagnose disabledCPU overhead regression
and COMmax rise, no furtherpoweredrun before preflight passes. New
drv_filter_preflight.py fail-fast subprocesscheck=True prevents maskedfailure.
Captureavailability89.66% isnotqzc; observeraddswork, noCPU-saving claim.

E317 bench-filter-observe separate flag integrates first driven handoff observer:
prepare after adoption/before outputowner start (ENABLE may stayhigh; outputs
disabled, allowners inactive, TIM2stopped/DIER0 required), mux tickets, stop,
before Interval.set_count (recordonlyn0), fixture-only summary. Restart now
ORsCEN preservingCKD. 267Python tests, releasebuild pass; NOTFLASHED.
Installed9AA099C1,lastoffE314. Next disabled diagnostics on candidate then
bounded62/62 trace0 no-recovery hold, verify fullcampaign+FILTEROBS and actual
sample/event counts. Observer overhead may affect arm timing; no guardchange.
Recovery observation unsupported, no CPUgain yet. Summary latestCCR includes
muxsettling and overcapture, notqzc. Verify prepare config in actual handoff.

E316 filter_epoch lifecycle now used by observer scaffold. One-use mux ticket
revoked by stop/new prepare/reset/new mux; handoff reset before arming doesnot
count as capture sample. 3 lifecycle tests pass, full replay/M0check and release
build pass. Still NOT wired/NOT flashed; installed9AA099C1,lastoffE314.
Next connect preparation at a genuinely stopped/noowner point, ensure release
doesnot cancel intended observer epoch, preserve CKD on restart, and strict
host FILTEROBS validation before powered test. No CPU/BEMF gain claimed.

E315 filter_observe.rs observer scaffold added, NOT wired/NOT flashed. API
prepare at ENABLElow/timerstopped/noowner, before_mux/after_mux(rawpolarity),
before_counter_reset snapshot latestCCR+overcapture/lag, stop/dump. NoIRQ/DMA
or gate authority. Immediate rearm includes mux settling; notqzc. Serial/probe
untouched; installed9AA099C1,lastoffE314. Next integrate lifecycle only after
testing stop/rearm/reset ordering and locating pre-ownership prepare hook.
Live recovery release currently rewrites TIM2 CR1; preserve CKD explicitly
or observer setup is lost. Root build is not installed identity.

E314 9AA099C1661E7FCB3EAED14C0DAA92981FC46E6E516E77DF14237A2F1892F020
NOW INSTALLED release/s/thinLTO. filtertime01 passes ordered pulse/capture,
post-low, pulse-local CNT continuity and timestamp checks. Filter15 long capture
latency30 half-us ticks from pre-write CNT; nofilter0 ticks (quantized), not
zero physical latency. Filter15 rejects2us. Finaloff verified, no motor run.
Next observe-only capture integration preserving TIM2 reset/owner semantics;
do not claim CPU savings or live filtering qualification. E313 regression
checks belong to previousEF1E image, not this one. Host suite264tests.

E313 EF1E586997CA28C49C8E802EEAA7563E4FB03739EEF7F4A864242BA6FD8AB9A9
NOW INSTALLED release/s/thinLTO text114164/data1104/bss29276. Filtercheck01
PASS: filter0 captures2/40us, filter15 rejects2/captures40; actual software
brackets2/40, levels0/1, no overcapture, restore+finaloff pass. 263Python tests.
Disabled atomic256/roles6/CPU2,7,10/archive3x3 pass; no UART repair needed.
No motor run; no live filtered sensing or CPU-saving claim. Next establish
capture counter continuity/edge timing, then observe-only BEMF comparison;
preserve TIM2 interval semantics and existing guards. Board outputs verifiedoff.

E312 historical: optionalbench-capture-filter implemented/compiled NOTFLASHED then.
UsesENABLElow/TIM2CH2polled/code0,15/requested2,40us; measurewidth/levels,
CC2IF/CC2OF. NoNVIC/DIER authority, rejectsDIERnonzero, configrestore.
Next strict hostverifier +stronger countercontinuity/restore checks before run.
Installed8D1AAABF unchanged,lastoffE309. RootELFnewcandidate not installed.

E311 purecapture_filter.rs plan+184Rusttests/M0checkpass, NOTlivewired.
PAC TIM2CKD0/1/2=Div1/2/4; maxIC2F15+CKD2 ideal14..16us samplingphase
at64MHz, excludes synchronizer/IRQ. NoPSCchange/CPUgain claim. Next actual
ENABLElow filter0/15 pulse/capture diagnostic with measuredwidth/level,
overcapture/counter and restorechecks. No hardwareaction;8D1AAABF unchanged.

E310 hardwarefilter route audit: ST RM0444Rev6p688 TIM2_TISEL TI2SEL1=COMP2.
LocalPAC IC2F offershardwareconsecutivesamplefilter. HARDWARE_SENSE_FILTER.md
records route/source +disabledexperimentcontract. TIM2 already2MHz interval,
mustpreservePSC/ARR/CNT/SMCR duringliveuse; filterclock isnotassumedPSCclock.
Next ENABLElow capture/polarity/pulse-filter test, no livepath replacement yet.
FullPDFfetch failed/cancelled0bytes; sourceisindexedSTtext,notlocalcache.
No firmware/flash/UART/motor actionE310;8D1AAABF,lastoffE309,traceOFF.

E309 trace1 diagnostic same62/65/8D1AAABF FAILCycleTiming12 after393795us,
step2guard390558->393757=3199<3226,695COM/694acc,raw227/bus10996.
TailCRC/fullfault/off verified,portsclosed,cleanuptraceOFF.24rows:18persistence
reject(16firstread),3blank,2accepted,1guardrefused after12correctreads.
Step2CNT610/608/610 recurs393549/393590/393674us (41/84us,carrier41.656us).
PWM-related IRQ traffic evidence, not analogcause/whole-runfraction proof.
Referencecycles6909->6389halfus,notrotorperiod. Next audit hardware routing/
filter or timed sensing for cheap rejection without losing validcrossings;
don't assume12readloop dominates each visit or widen cyclefloor. No higherduty.

E308 same8D1AAABF acquisition62/BEMF64 PASS10s292.326eHz,IRQ58.430%,
sigma38.396us,raw280,bus11116,arm68.5cost13. BEMF65 FAILCycleTiming12
after714760us/~297.074eHz,1274COM/1273acc,step5delta3221<3226us.
raw227/bus11116/DMA18queue2/stack4180,alloff/portsclosed. No escalation.
CPUgain didnotcure prior~297timingwall. Referencecycles6837->6436halfus
notphysicalrotorperiod; causeunknown. Next sensing/timing diagnosis, not
wideningguard. Latest6.4n1hold only, recoveryqualification remains open.

E307 8D1AAABF NOWINSTALLED. dutysplit_atomic01UARTsilent retained,
exactsaferegs+knownclockrestore; atomic02pass256. role62/70 both6flags127/
12us/CCRs165,186,select6/6,CPU2/7/10,archive3x3PASS. Equal62/62 explicit
10shold PASS280.4386eHz,sigma39.226us,IRQ58.355%,16827COM/16826acc,
COMP72/COM99/commit46,arm57cost13,raw271,bus11092,stack4180,DMA18queue2.
DUTYSPLIT62/62 verified, alloff/portsclosed. No powered higherduty yet; latest
build recovery stillunqualified. Next bounded independent duty increment with
existing310profile guards; ifguardfails retain and diagnose, don't widen blindly.

E306 disabled duty diagnostics added NOTFLASHED. Root8D1AAABFCEF0D20CC9986E8C8721BBECC58C1013894E313CCC353C40CE06CCB5,
release/s/thinLTO text112316/data1104/bss29276. Installed1E13B559,lastoffE304.
off clears pendingBEMFduty; generic gates_off preserves it through startup.
dutycheck6cases and roledu40..100 ENABLElow CCR/counter test implemented,
drv_role_check --duty validates compare.261Python pass. Next disabled board
checks then equal-duty regression. No hardware actionE306.

E305 duty split implemented NOTFLASHED: root09E470D5509BCD9C7509BB78D1D91C499EC3BAB6382A2822C05EDADA90E7BF9B,
text111536/data1104/bss29276 release/s/thinLTO. Installed1E13B559,lastoffE304.
bemfdu40..100 or0inherit one-shot; drivedu acquisition stays40..62.
Driven handoff/recovery receive independent fixed segment duty. No speedguard
or electrical changes. --bemf-duty ack/cleanup +DUTYSPLIT strict decoder added;
marker is selectedsetting not independent waveform readback.181Rust/261Python/
M0checkpass. Next disabled setter/abortcleanup/one-shot/CCR check then equal62
regression/recovery. Always explicit --bemf-duty on new live tests; stale pending
settings survive rejected setter/abort before consumed run. See DUTY_SPLIT_EXPERIMENT.

E304 installed1E13B559CEE7566B3DA36C593D52D075267E90B218B42B80557F0A8C719321E6,
E301features MINUSbench-comp-paths; release/s/thinLTO text110740/data1104/
bss29260. singlelean_atomic01/roles01/cpu01/archive01 allpass,noUARTrepair.
singlelean_hold62_01 PASS10s280.562eHz,IRQ58.47159% vs62.47127instrumented,
sigma39.503us,16834COM/16833acc,COMP72/COM98/commit46,arm68cost12,
raw272,bus10996,stack4124,DMA18queue2. COMPPATH absent. Finaloff/portsclosed.
Lean build n1hold only; E0FE2277 owns3/3recovery cohort. Next separate BEMF
duty control from acquisition duty before expansion, preserve bounded initial
profile and all speed/current/age/actualarm guards; recovery requalification open.

E303 sameE0FE2277 fixedtwo confirmations PASS; combined singlecore_reentry62
cohort3/3 original30s. Recovered279.919..280.085eHz,IRQ62.44..62.79%,
sigma35.09..35.19us,COMP75/COM105/commit46,DMA18queue2,stack3552.
Actualarms52..53.5us/cost11..12,deadline176..242spare,raw274..280,
bus10757..10972. Alloff/portsclosed. CSVhashes retained, timing/portablefronts
updated. No wider envelope claim. Next remove optional bench-comp-paths from
normal build and separate BEMF duty from startup acquisition's40..62 bound.
driven_run::run passes acquisitionduty directly to core_bench::driven_power_run,
which rejects>62; do not raise acquisition drive merely to expand BEMF duty.

E302 sameE0FE2277 backend NOWmotor-tested. record01 PASS12refusals.
singlecore_hold62_01 PASS10s279.45655eHz,IRQ62.47127% vsE29970.12571%,
sigma39.738vs51.147us,COMP75/COM105/commit46,arm65.5cost12,raw280,
bus11032,stack4076,DMA18queue2. Visits178131 increased vs145198 baseline.
singlecore_reentry62_30s_01 PASS30soriginalbudget/~27.989srecovered280.036eHz,
47028COM/acc,sigma35.186us,IRQ62.795%,COMP75COM105commit46,
seed1211/acq7833us/arm53.5us,raw280,bus10972,stack3552,deadline242usspare.
Paths498017 reset correctly,87028closed/363961open-noaccept/47028accepted.
Alloff/portsclosed. n1hold+n1recovery, notcohort or higher-dutyqualification.
Next confirm recovery gain with fixed cohort or strip diagnostic counters in
normal running build before duty-led expansion. Do not claim full CPU utilization.

E301 E0FE22777F94C78C05F89D664EE7F54F69ADCD2DBA93C1830CFDF93885006C67
NOW INSTALLED single-core backend +atomiccheck/ATOMICBACKEND marker,
release/s/thinLTO text111316/data1128/bss29260. NOmotorqualification yet.
atomic01/roles01 UARTsilent retained; exactRCC08000000/PD1zero/BDTRc1a/
CCRs0 then knownUARTclock08040000 restore. atomic02 PASS256 privileged/
entryunmasked/nestedPRIMASKrestoration; roles02 sixflags127/12us; cpu01
max2/7/10; archive01 3x3PASS. Alloff/portsclosed.258Pythonpass.
--single-core-atomics requires backend provenance; unknown markers rejected.
Next recordcheck preflight then bounded same-point motor comparison with all
guards, no duty expansion. Diagnostic uses local atomic state, not contention
proof; current-backend timing/persistence/recovery still need qualification.

E300 architectural finding: portable-atomic critical-section backend masks
interrupts even for ordinary loads/stores (confirmed assembly +cached1.15docs).
Optional bench-single-core-atomics builds with --no-default-features; default
atomic-critical-section retains old backend. HAL also forced critical-section:
root now patches URL to ref/stm32g0xx-hal at15aca632, ONLYmanifest backend
feature removed there. Ignored checkout reproduction patch documented in
ATOMIC_BACKEND_EXPERIMENT.md. No sibling minz changes. Root ELF9223AA96
NOTinstalled,text110520/data1128/bss29260 vs120256 old. No runtime gain claim.
Next add backend marker and disabled atomic/PRIMASK test; audit DMA/privilege,
then existing preflights before power. InstalledFDD9319B,lastoffE299 unchanged.

E299 FDD9319B NOW INSTALLED. comppaths_disabled_01 role6/6 12..13us,
cpu_01 max2/6/10, archive_01 3x3 pass; no UART clock repair needed.
comppaths_hold62_01 PASS10s6.2%/phase60/trace0/24k,279.448eHz,
16767COM/accepted, sigma51.147us,raw247,bus10925,arm51us/cost14,
COMP85/COM268/commit48,IRQunion70.126%,stack4072,DMA18/queue3,finaloff.
Paths145198 dispatched: closed21526(14.8%), open-no-accept106905(73.6%),
accepted16767, no_gate/unknown0. Counters perturb timing vs945 baseline;
not an optimization or clean performance comparison. --comp-paths now
requires provenance/full accounting. Next architectural focus is post-gate
qualification traffic, not assuming pre-gate blanking removes most load.
No duty expansion/recovery qualification on this instrumented image yet.

E298 optional bench-comp-paths implemented, NOT flashed. Root ELF
FDD9319B4099129C3DBA94F62EFAF1B30BD3C0FECD8D58FE4DD4841BFD3AE745,
release/s/thinLTO/codegen1 text120256/data1120/bss29264. Installed945D6788
unchanged, no UART/probe/motor action. Actual first Interval::count captured
per dispatched COMP, five aggregate classes; no per-level-read trace enabled.
Reset moved to observation_reset, including recovered segment. Optional host
decoder rejects malformed/active/saturated data; architecture report checks
accepted equality and dispatched<=calls. Not yet required by motor fixture.
180 Rust tests/M0 check and 255 Python tests pass.
Next finish fixture-required provenance and disabled preflights, then bounded
same-point measurement of instrumentation overhead BEFORE deferral changes.
No measured path fractions yet; open_no_accept is NOT proven noise rejection.

E297 operatorCPUarchitecturequestion supersedesimmediatedutyexpansion:
characterizebeforechanginginterrupts. ARCHITECTURE_BUDGET.md +newvalidated
drv_architecture_report.py: at279eHz15545COMP/s1674COM/s4975ADC/s,
9.286visits/COM,89.23%nonacceptingNOT89%removable. IRQunion67.37%,
exclusivepervector/blank-vs-persistencecountsUNKNOWN; don'tscale byduty.
Nextaggregateactualgate-decisioncountswithoutfullperreadtrace, measure
counteroverhead. Deferredwake mustuseTIM2accepted-age(strict>avg>>1),
preservepending/owner-epoch/cancel/guards; notfixedCOM-relativeblanking.
Nohardware/firmwarechangeE297,945D6788stillinstalled,lastoffE296.

E296 same945D6788 installed, inline24_hold62_01 PASS10s278.803878eHz,
sigma43.983us(vsE294samepoint58.945),COMP81COM134commit48IRQunion67.260%.
inline24_reentry62_30s_01..03 fixed3/3PASS30soriginalbudget, recovered
279.002/279.323/279.049eHz,sigma40.477/40.829/40.247us,
arms42/41/41us cost14,deadline208/189/131usspare,stack3992all,
COMP81COM135commit48all,IRQunion67.16..67.37%,raw265/284/279,
bus11032/10996/11044mV. Alloff/portsclosed. CohortCSVretainsidentities.
Timing/portablefrontsupdated. Thisqualifiesonepoint,notwholeenvelopeoramps.
Nextduty-ledexpansion needs decoupling BEMF duty from6.2% acquisition cap:
driven_run/core_bench/handofffixture currentlyreuseacquisitionduty, don't
increaseblindstartupjusttoexploreclosedloop. Keepalltiming/electricalguards.

E295 bench-inline-comp NOWINSTALLED945D67886B499176CBD199ABD4D9595E25E0C097B5FADBA9460FF7A9B908A624.
Onlyinline(always) CachedCompoutput_level/read_comp_level; samplecount,
volatileCSRread/polarity/core/guards unchanged. Assemblyloopfreshldr08004d94,
nooutlinedoutput_levelcall. Frozen/liveam32_isrhash54b9f495matches; filter
isconsecutiveagreementNOTaveraging.250Pythonpass,release/s/thinLTO
text119748/data1088/bss29264. inline24_disabled01all6pass12..13us,cpu2/7/10,
archive3x3pass;UARTworkedafterresetwithoutclockrepairthisflash.
inline24_hold58_01 PASS10s253.873708eHz,sigma43.497us(vs58.848old),
COMP81/COM135/commit48/guard25,IRQunion66.493%(vs67.882),raw228/bus10996,
arm57.5us/cost14,DMA18queue2,stack4128,alloff. n1comparison notcohort.
Nextconfirmhigher6.2point/longerrecovery beforecallingreliablegain; no
24krecoveryqualifiedyet. --inline-comp requiresCOMPREADfixtureprovenance.

E294 bench-pwm-24k NOWINSTALLED3DEAC2F08CCB386876C221756F4D3E5CE2EB9FC75DE6BD893E56F6C0D31C336F.
release/s/thinLTOtext119368/data1088/bss29264. Carrierconfiguredbridgeoff
beforeguard/ADCstart;startup10k restoredatsafing. ADCperiodsnapshot retained
fordump2666vs6400. StrictCLI--carrier-hz24006/rolecheck--period-ticks2666.
249Pythonpass. disabled01UARTsilent/exactsaferegs+knownclockrestore;
disabled02all6flags12712..13us,restorepass;cpu2/7/10,archive3x3pass.
carrier24_hold58_01 PASS10s254.266759eHz,sigma58.848us,raw245/bus10925,
IRQunion67.882%,COMP87/COM147/commit48. hold62_01 PASS10s278.837228eHz,
sigma58.945us,raw268/bus11128,IRQunion68.539%,COMP87/COM199/commit48.
Arms54/56us,cost13/14,DMA18queue3/2,stack4128/4120,alloff/restoredARR6399.
24k NOTdemonstrated improvement:moreCOMPtraffic/cost/jitterattheseconditions.
No recoverytested24k,no newduty/speedlimits. Next inspectqualification/IRQ
cost/reference timing beforefurtherexpansion; preserve10krolebaselineA54DCFEA.

E293 purecarrierprofile+DRVtimingaudit,NOhardware. RootELF593DA14E34B7F2F07959B4E74DFBAB8F72D385187D4B8BCF5BD832D25E4BBC4A
NOTinstalled; hardwareA54DCFEA,lastoffE292.178Rust/M0check/release-s-thinLTO
pass,text118712/data1088/bss29264. carrier_profile6400/2666ticks,exactHz,
compare/phasebins/idlecountertests; sequenceapply_carrier added butlive
still10k wrapper. No24k optionyet. CachedDRVpage23 says4usTDRIVEnotminPWM;
no guaranteedmininputpulsefound,250nspropdelaynotapulsecertificate.
Next establishcarrier BEFOREADCstart/firstCOM atdisabledsetupboundary,
retainADCperiodsnapshot acrosssafing/restore10k; updateidle/hostmetadata.
DoNOTchangeARRfirstCOMwithalreadyliveADCstream. Startup/DMA192targetstay10k.
PWM_ROLE_EXPERIMENT.md hasexactsourceidentity/timing/integrationnotes.

E292 sameA54DCFEA roles_hold60_01 PASS10s296.270732eHz,sigma39.531us.
roles_reentry60_30s_01 FAIL: actual2sdropout/freshseed1129/arm34us/cost14,
thenresumed92909us166COM/165acc CycleTiming12 step3delta3221<3226.
NOcurefromremovingperCOMUG. roles_reentry58_30s_01 PASS30soriginalbudget,
resumed27.990049s289.907922eHz48687COM/48687acc,sigma36.655782us,
raw453/bus11056mV,seed1167arm39us/cost14,deadline163usspare,stack4000.
COMP86/COM101/commit43/guard24,DMA18queue2. Allfinaloff/closedports;
samebuildinstalled. Newrolecarrierrecovery n1pass,notrepeatedcohort.
Nextdistinctlevercarrierfrequency; audit10kHz assumptions (sequence6400,
ADCPHASE6400,pwm_sample_dma192/320 andARR6399),DRVpulseminimum and
disabledchecks beforechange/run. NotaguardlooseningorAM32switch.

E291 A54DCFEAA449C83AF4300DF3600F7A858FC138026BDF9554633DA01FEAB3B365 NOWINSTALLED.
roles_disabled_01 UARTsilent retained/exactsaferegs+knownclockrestore;
roles_disabled_02 6/6flags127,8uscarriercontinuous,restore/offpass.
roles_cpu_01 max2/7/10;roles_archive_01 3x3pass. Strictrolefixture+PWMROLES
provenance centralvalidation/--pwm-roles added;247Pythonpass.
Three10s trace0 holds54/55/58 PASS268.347/273.691/287.794eHz,
sigmas40.545/39.742/39.681us;raw458/446/456,bus11092/11068/11044mV.
Arms57/59.5/58.5us,cost14,COMP86,COM100/100/101,commit49/44/44,
DMA18/queue2,stack4128,finaloffall. No recoverytestedonnewbuild.
Same-duty speedfell(~290->268 at5.4);NOTexpanded speed envelope or proven
jittercure. Near-speed sigma39.68vsold41.35modest,n1,COMmaxhigherthanold81.
Next timingtradeoff/nearold297edge comparison thenrecovery,keepallguards.
PSU/currentcalibrationstillopen. No cap/carrierfrequency/speedguardchanges.

E290 optionalbench-pwm-roles implemented/compiled,NOTFLASHED. Rootcandidate
A54DCFEAA449C83AF4300DF3600F7A858FC138026BDF9554633DA01FEAB3B365;
installed remains4DA1EE7E,knownmotor807372B6. Onlypowered_timer::commit uses
rolecarrier underexistingguardcriticalsection; forcedstartup unchanged.
EqualCCRs+UGonceperpreparedsegment,thenMOE/GPIOblank/roles/mux/MOE,
noongoingCCR/UG. bridge_clear revokespreparation andrestoresallAFafter
clearingMOE/CCRs/BOTHGPIOlatches. Unexpectedprepareddutychange safingpanic.
Idle rolecheck built butUNRUN: require6flags127,restored1,disabled1,finaloff.
Nextstrictfixture/provenance,disabledhardwarechecks,thenboundedmotorA/B;
NOhardwarequalificationyet.173Rust/M0check/244Pythonpass. Buildrelease/s/
thinLTOtext118720/data1088/bss29264. PWM_ROLE_EXPERIMENT.md hasfullcontract.

E289 waveformaudit+pureGPIOplan,NOhardwareaction. Frozenminz tim1_motor_pwm.rs
hashcb9f06a6 matcheslive; referencecom changesGPIOroles, allCCRs equal,
noCCRtraffic/UG. Currentbinz clears/rewritesCCRs+UG everyCOM; cachedG071PAC
confirmsUG resetscounter. NOTproofthiscauses5.5fault; don'tremoveUGalone
withper-phasepreloadzeros. PWM_ROLE_EXPERIMENT.md recordsintegrationcontract.
Newphase_gpio_plan.rs preservesqualifiedA=PA10/PB1,B=PA9/PB0,C=PA8/PA7
andbinzsectororder. Purefields only,NOTwiredintoshell;169Rusttests+M0checkpass.
Next optionalrole-onlycarrierintegration with initialequalCCRsloaded and
MOE/GPIO breakbeforemake, plusglobalstop/sineAFrestore/disabledpreflight.
Installed/root4DA1EE7E unchanged,lastverifiedoff; knownmotor807372B6.

E288 settleprobe4DA1EE7E3B8CAEDF8E7851E136DF197D9E93242CC1A05692845E4DE2AFB3FD70 NOWINSTALLED.
release/s/thinLTO/E287features,text118872/data1088/bss29260; NOmotorqualification.
Addedbasemode20/measuredTIM17delay. Initial2E4DB0DF image'sbasesettle_1ms_01
refusedat61us because readiness token requestedBEFOREwake settling;retained.
Correctedordering matchesexistingstartup: gatesoff/ENhigh checked duringdelay,
recordnFLTlow, requirehealthyreadiness AFTERfixed1/20ms beforeallsamples.
basesettle_fix_{1,20}ms_01..03 allPASS (order1,20,20,1,1,20),nFLTlowseen1
thenreadyafter1000..1001/20000..20001us. SumDMA-SWmidpoint1ms4.1094/3.4414/
2.875;20ms-1.0898/.6953/-1.0586counts. SWend-startvariationnotconsistently
reduced; nofixedoffset/amps/startupdelaychange justified. Bothflashpreflights
neededexactsaferegs+UARTclockrestore;correctedcpu2/7/10,archive3x3pass.
Alloutputsverifiedoff/portsclosed,nomotorspins. Knownmotorbuild807372B6.
Nextreturntowaveform/referenceaudit; donotextendbaselinesweepswithoutnew
question/independentreference. InitialPSUreadingrequeststillunanswered.

E287 optionalbench-baseline-dma NOWINSTALLED D40A2D883E133A57CD8B69DE3CF203A4EDBFFCAF058E34DCFBC03C16E9C76028.
E281features+tail/cachedcomp/baselinedma,release/s/thinLTO,text117976/data1088/bss29260.
basemode idleonlySW/DMA/SW128x5 each,sameCSAwake,gatesoff throughout,50ms
perphasechecked,coherentpoll/restore;DMAIRQneverunmasked,offsetsneverapplied.
cpu_01silent/exactsaferegs+knownUARTclockrestore,cpu_02max2/7/10 pass.
basemode_compare_01..03 allPASS;sumDMAminusSWmidpoint +2.0625/-2.09375/
+1.1796875counts,SWend-startsum3.40625/1.921875/1.09375counts. No fixed
modecorrection/amps justified. archive3x3afterprobes pass,finaloff/portsclosed.
THISIMAGE MOTORPATH NOTREQUALIFIED; knownmotorresultsremain807372B6.
Defaulttrace settingreset onflash; nextmotorfixture must explicitlycoretrace0.
No motor spinsthisentry. Nextsettling/repeatabilitygates-offdiagnostic,not
offsetapplication; independentPSUanchorstillabsent.165Rustpass.

E286 currentmetrology initialhold cachedoff_current54_30s_01 PASS30s5.4%,
290.863eHz52355COM/52354acc,sigma34.034us,raw588,bus10817,149253scans,
COMP86/COM82/commit46,queue2/DMA18,stack4128,finaloff. Same807372B6/trace0.
AsyncPSUcurrent/voltagereadingrequested; none received atrecordingtime. DoNOT
reuseold70mA orrepeatmotorholds merelywaitingforoperator. Baselinecentered
[8.6796875,10.6953125,9.6875],running[9.91685,9.12642,9.93092],sumresidual
-0.0883085rawcounts: NOTcalibratedamps. Priorcomparableinitialresiduals
-6.986..+4.381counts vary; noisolateddrift/biascauseproof. Sourcebaselineuses
singleSWchannels4/1/0/6/13; DMAusesascending0/1/4/6/13/201us. Nextbounded
same-modebaseline comparison can testthisconfound withoutchangingguards.
No waveform/firmwarechange,portsclosed. Fullobjective/currentcalibrationopen.

E285 same807372B6/trace0/no flash. cachedoff_reentry54_01 PASS10s original
budget,~8srecovered290.549eHz13929COM/13928acc,sigma30.099us,raw548,bus10937.
cachedoff_reentry54_30s_01 PASS30s original/~28srecovered291.010eHz,
48873COM/48872acc,sigma29.634us,raw580,bus10638. Bothseed1167age212,
arm40/cost13,deadline227/216spare,COMP86/COM81/commit45,stack4128,
queue2/DMA18,allfullverifiers/finaloff/portsclosed. Freshacq7454/7551us.
5.4recoveryworks oncachedpath;5.5intermittentinitialguardfailure remains,
notgenericrecoveryregression. Currenttiming/parity/portablewins frontsupdated.
No higherdutyattempt/guardchange/physicalceilingclaim. Next address5.5
accepted-cycle variation; don't treat291eHzcontrol as replacementgoal.

E284 cachedoff_reentry55_01 FAILED BEFORE INJECTION at1745911usCycleTiming12.
Same807372B6/trace0/no flash. step6 guard1742674->1745897=3223<3226,
3103COM/3102acc,raw584,bus11140,stack4284,queue2/DMA18,COMP92/COM81.
REENTRYresult1/injection0/used0; recoveryNOTexercised. Fullraw/finaloff verified.
E2833/3holds doNOT prove robust5.5startup/hold or recovery; retainthisfailure.
Before2s inject adds conditionalcomparison only; observation_elapsed sampled
unconditionally inbothmodes. No sourceevidence for a pre-injection filter/timer
change, no causalclaim fromthisn1 failure. Gateorigin/timestamps remainseparate.
Fixture now reports verifiedearlyinitialstop, not misleading stagingfailure;
234Pythonpass. Next5.4recoverycontrolonthisimage beforehigherduty or more
identical5.5trials. Fullobjective unchanged; current5.5notrecoveryqualified.

E283 SAME807372B6/no flash, explicitcoretrace0. cachedoff_hold54_01 PASS10s
290.336eHz17420COM/17419acc,sigma41.350us,raw577,bus10566,stack4284.
cachedoff_hold55_01..03 ALLPASS10s:296.747/296.900/297.522eHz,
COM17804/17814/17851,acc17804/17813/17850,sigma44.433/44.546/45.693us,
raw598/592/564,bus10937/10865/10901. EveryCOMPmax86/COM81/commit45,
queue2/DMA18/stack4284/fullverifier/finaloff. Manifestcachedoff_hold55_cohort.csv.
Initialpass followedby TWOpredeclaredconfirmationattempts, no failures inthis
configurationcohort; priortracedfailuresretained E280/E282. N=3short holds,
notlongduration/recoveryqualification. Observedrecordermin3223<guard3226
on03 due distincttimestamps; doNOTcallrecordermin guardmargin.
MeasuredIRQunion5.5~56.3..56.4%,notCPUutilization/idleheadroom.
Next bounded5.5recoverywithsameimage/trace0; actualarm>=32us/cost<=16,
originaldeadline/current/bus/trackingguards unchanged. No dutyincreaseyet.

E282 cachedcomp807372B6 NOWINSTALLED. cpu_01silentretained; exactsafeRCC/
PD1/BDTR/CCRreads beforeknownUARTclockrepair;cpu_02max2/6/10/archive3x3PASS.
cachedcomp_hold54_01 PASS10s289.422eHz17365COM/17364acc,sigma53.403us,
raw543,bus10996,arm47.5/cost13,stack4284,queue2/DMA18,COMP/COM115/81.
Same-observation-clock retained qualification brackets29.5..47.5us vs
E28037..56.5us: shortest reduced7.5us,NOTreliabilityorCPUutilizationproof.
cachedcomp_hold55_01 FAILCycleTiming12at1205461us,step5
1202230->1205453=3223<3226,2130COM/2129acc,raw504,bus11199,stack4284.
LastIRQreads12/passedsign; CYCLECOREavg1122/ci1129/this1114/last1055/wait282.
Bothfullcapture/finaloff/portsclosed;nohigherduty/recoveryafterfailure.
Next cachedcomp withtraceOFF toqualify normalrunningpath; shortertraced
bracket is not justification tokeep optimization withoutthat comparison.

E281 optional bench-cached-comp implemented/built NOTFLASHED.
Root807372B6F101925F2AA197690B1FA71975315F90B7716B1DDDB1B11640ED10BA,
release/s/thinLTO,E280features+cachedcomp,text115980/data1088/bss29260.
Snapshots source/polarity/trace flags once per motor() invocation; comparator
signal/pending/safety remain live,12reads/guards unchanged. COMPMODE explicit.
Read-mode writes audited outside controller calls; don't extend cache to any
signal or safety state. Timing/persistence aperture changes needqualification.
New host gate_accept_timing joins fullwidth IRQtail and acceptedtail in OBS
clock, never guardclock. E280fault normalaccepted brackets37/37/46.5/37/67us,
healthy37.5/56.5/37.5/46.5/37/37us; includesbookkeeping/preemption,NOTpurefilter.
231Python/165Rust pass. NoUART/probe/motor thisentry; installed915F90AE,
lastverifiedoff,traceOFF. Next disabledchecks then5.4%timingregressionbefore
any5.5%test. No claimed speedup untilhardwaremeasurement.

E280 optional bench-irq-tail NOWINSTALLED915F90AEC7BBB2E2743BD88AB55FD87E296D1243FBC3E86E22CFDBC6FD779FCC.
Same release/s/thinLTO/E273features+tail;115764text/1088data/29260bss.
Tail reuses24rows,addsfull32bitobservationtimestamp(us_hi15thword),48rowbytes
+8statebytes;IRQWINDOW explicit. Defaultprefix14word path preserved.
cpu_01 max2/6/10,archive3x3 pass WITHOUT UARTclockrepair after thisflash.
irqtail_hold54_01 PASS10s288.967eHz17338COM/17337acc,sigma55.788us,
raw554,bus10948,stack4300,COMP/COM120/109,commit45,queue3/DMA18.
irqtail_hold55_01 FAILCycleTiming12 stop439044us,step2 guard435815->439037
=3222<3226,767COM/766acc,raw510,bus11128,stack4300,queue2/DMA18.
Lasttailhandlerseq3548 OBSus439004 step2 gate859 avg1126 reads12 level0=expected0,
thiszc951;reference persistence passed then guardrefused (accepts0 isNOT
persistencefailure). OBS andguard timestamps have DIFFERENT origins; no
cross-clock latency subtraction. Bothfullcapturevalidated/finaloff/portsclosed.
Host fullsummaryvalidatestailCRC/width/accounting;229Python/165Rustpass.
Tail adds ISRcost; notperformanceoptimizedbuild. Next switching/qualification
timing investigation; no guardraise/AM32 switch. ShelltraceOFF aftercleanup.

E279 offline reference-cycle context implemented/tested; no hardware action.
Traceoff5.5 fault last12reference intervals sum7058 then6427half-us ticks:
3529/3213.5us,pairedmean3371.25us (~296.626eHz), NOTphysicalrotor periods.
Precedingstep4=930->1295 andfollowingstep5=1223->885 nearlycompensate
(pair2153->2180ticks). Suggests crossing-time redistribution, not causeproof.
Tracedfailure pair6896/6446ticks is different; don't generalize one sample.
drv_cycle_fault.reference_cycles uses contiguoustail+matchinglast_zc/refused
snapshot only, never mixesguard/recorderclock into sums;226Pythonpass.
Next useful diagnostic needs near-fault COMP/PWM timing rather than first32
IRQtrace records. No guardraise,physicalceilingclaim,or identicalmotorrepeat.
InstalledA948958F unchanged,lastverifiedoff; shelltraceOFF as E278.

E278 same installed A948958F, no flash/firmware changes. Explicit coretrace0:
traceoff_hold54_01 PASS10s289.801eHz17388COM/17387accepted,sigma43.259us,
raw533,bus10972,COMPmax92us. traceoff_hold55_01 FAILCycleTiming12 at4404543us,
step4 previous4401316 decision4404535 delta3219<3226;7827COM/7826accepted,
raw584,bus10769. CYCLECORE avg1127/prev1140,ci1104,this1060,last1093,wait276,
filter12,poll0/run1. Both queue2/DMA18/stack4356/finaloff,portsclosed.
Tracing off changes the twelve-read persistence time aperture, not just logging;
n=1 comparisons do not establish reliability or fault cause. Not solved at5.5.
Fixture --core-trace 0/1 now explicit and summary-validated; omitted inherits.
CURRENT SHELL TRACE MODE OFF after cleanup: explicitly choose mode next run.
224 Python tests pass. No guard changes/higher duty/recovery after failure.
AM32 remains reserve fallback, not immediate campaign or completion dependency.

E277 A948958F diagnostic NOWINSTALLED. Disabledcpu2/7/10/archive3x3 pass
afterexactsaferegs+knownUARTclockrestore;silentcpu_01 retained.
cycle_core_hold54_01 PASS10s288.958eHz17337COM/17336acc,raw519,bus10925,
arm60/cost13,stack4356. hold55_01 FAILCycleTiming at266542us,step1
263312->266533=3221<3226,462COM/461acc,raw531,bus11319,stack4356.
NewCYCLECORE valid:avg/prev1127,ci1113,thiszc1112,last1076,wait278,filter12,
zc474,poll0/run1. Last5referenceintervals899/1074/1132/1153/1076+1112=
6446half-us=3223us vsguard3221:shortcycle existsinreferencecounterstream,
notjustrecordertimestamp artifact. Differentsector disprovesfixedstep3.
No physicaloverspeed/electricalcause proof. Bothqueue2/DMA18/finaloff/nFLT1,
portsclosed,221Pythonpass. Next crossing/timing variation analysis,not
anotheridenticalrun or fixedphasewiringclaim. No higherduty/recoveryattempt.

E276 fault-onlyCYCLECORE added NOTFLASHED. powered_timer::accepted trips
CycleTiming first (fullsafing),then records core12fields beforeEV_ACCreturn:
step/rising/average/previousaverage/ci/thiszc/lastzc/wait/filter/zc/poll/run.
Referenceinterrupt_routine already resetTIM2/armedCOM beforeEV_ACC,so no
timer-ageclaim; lastzc is previousINTEREVENT,not previoussamephasecycle.
Snapshot one-shot/resetobserve_begin,quietcaptureonly. Host optionalstrict
metadata/step/profilechecks andcontextreport;220Pythonpass. Firstcompile
staticmutref correctedwithmatches!,releasebuildpasses.
RootA948958FD9C43472269B986AB42476BC8EABDA8BADDE8CB90C64E3E6C32B0258,
text115540/data1088/bss29204,release/s/thinLTO,E273features. Installedstill
7B1FB220,lastverifiedoff; noUART/probe/motor thisentry. Next disabledchecks
andhealthy5.4regression before fault-context5.5capture; no safetychanges.

E275 range310_hold55_01 FAILCycleTiming12at1125259us; exactstep3
1122029->1125251=3222<3226usfloor.1988COM/1987acc,raw501,bus11068,
queue2/DMA18/stack4232,finaloff/nFLT1,portclosed. No recoveryafterfailedhold.
Tailphase3recordcycles3429/3368/3352/3498; guardprevious torecorder+33us.
Not uniformcyclesat3222; variation suggested,physicalcauseNOTidentified.
drv_cycle_fault.py CLI validatescapture+reportsseparateclocks/tailcontext;
219Pythonpass. No firmwarechange;installed/root7B1FB220 remains.
OperatorclarifiedAM32 is reservefallback ifoutofideas/headroom,NOT next
campaign/dependency. Memo's~470CPU extrapolation unsupportedacrossbuilds,
~302recoveryconstraint NOTsiliconlimit; archivedminz includes230/257eHz.
Next bounded fault-time reference/IRQcontext investigation forlong-short
acceptedcycle pattern,not blindguardraise or anotheridentical5.5run.

E274 range310 image7B1FB220... NOW INSTALLED. Disabledcpu2/7/10/archive3x3
pass after knownUARTclockrestore (silentcpu_01 retained,exactsaferegschecked).
Threeactual10s attemptsPASS:range310_reentry53_01 284.006eHz13615COM/13614acc,
arm45us/cost13,deadline217spare,raw520,bus11068;hold54_01 289.125eHz
17347COM/17346acc,sigma52.196us,raw534,bus11032,arm55.5/cost13;
reentry54_01 289.525eHz13880COM/13879acc,sigma44.102us,seed1166age212,
arm40us/cost13 (8us spare),deadline198spare,raw535,bus10554.
Allqueue2/DMA18us/stack4232/fullverifiers/finaloff/nFLT1,COM41closed.
No further guardchanges/currentcalibration/newPSUreading. Newprofileclears
old5.4softwareboundary; singlepoints notcohort. Next5.5hold then recovery
onlyifpreceding evidencepasses; actualarmfloor stays32us, maylimitnextstage.

E273 optionalbench-range310 implemented and built NOTFLASHED. Impliesrange300
plumbing; runtimecycle/eventmin3226/268us, passivecycle/individualmin6452/536
half-us,seedmin1075. Existing250/300profiles unchanged. No current/bus/age/
tracking/arm/deadline/duty changes. Explicit RUNLIMIT and recovery metadata
hostchecked; mixed310/300cycleprofile fails recovery.165Rust/218Python pass.
Synthetic1076tickseed acceptsage205/rem64,REFUSES206and212; no guard bypass.
Root7B1FB220176C5261023F4842E491C3251212DB6EF9A60254968B5284D518ACAE,
text114804/data1088/bss29156,release/s/thinLTO,E266features withrange310
instead ofexplicitrange300. Installedstill6775326F,lastverifiedoff.
Next flash+disabledchecks then5.3% qualification before5.4% hold/recovery.
310 is testguardprofile,not measuredspeed/currentqualification; nofurther
rangeincrease without newevidence. Seedactualage refusal stays independent.

E272 constructor-inside-critical-section experiment A60D1B72... tested then
REVERTED: guard_construct_reentry53_01 PASS10s283.894eHz but guardstage27us
vs23prior,actualage220vs212,arm39us/cost14vs43/13. Not optimization win.
Raw511,bus10805,queue2,DMA18,stack4232,deadline208spare,alloff/nFLT1.
Compiled caller60+helper60 locals vsoldcaller188; NOT runtime stackgain,
wholetext+80.163Rust tests pass. No reference or guard-threshold changes.
Restored/reflashed EXACT6775326F...,currentroot=installed; finalcpucheck2/7/10
and off verified. Both flashesUARTsilent: retained construct_cpu_01 and
restore_cpu_01, exactsafeRCC/PD1/MOE/CCRs checks then knownclockwrite;
construct_cpu_02/archive3x3 and restore_cpu_02 pass. Portsclosed.
Do not repeat closure/copy microrewrites; both triedandfailed. Next assess
preacquisition inert guard storage with freshvalidation/publication, or a
small coordinated range profile retaining independentactualarm refusal.

E271 offline scheduling audit + rejected code experiment; NO hardware action.
E269_03 SEEDLAT100/120/134/180/188,actualage212half-us => preentry50us,
local10,baseline7,guard23,reference4,IRQsetup12. Guard block includes fresh
validation/publication,not23us removable work. Compiledstart_inner frame188,
size812bytes,guard copy into critical-section closure visible.
Tried pending Option+take borrowed closure: frame still188,copy persists,
text+40 (49E278F...); REVERTED,NOTFLASHED. Rebuild restores exact6775326F.
Next guard construction/publication design needs in-place initialization or
preacquisition inert storage, preserving freshsample/limits/step/age checks
and singleton IRQ publication. Don't retry identicalclosure rewrite or claim
timing win. Existing board remains lastverifiedoff; currentrange incomplete.

E270 fast_start_hold54_01 FAILCycleTiming12at689514us,requested10s.
Exactguardstep3 previous686170 decision689501 delta3331<3333; recorder
min3336 different.1188COM/1187acc,raw494,bus11092,3430ADC,queue2,DMA18us,
stack4232,finaloff/nFLT1,portclosed. Same6775326F,no guard/firmwarechange.
217Pythonpass inclnew realfailure regression (never countedcompleted).
TIMING_HEADROOM frontupdated: current5.3cohort proven,5.4softwarefloor;
separate predicted recoveryarmconstraint age212+floor64 requireswait>=276
half-us (~302eHz idealregularinterval),NOT measuredphysicalmaximum.
Next inspect acquisition/cleanup latency before coordinated range expansion;
wideningcyclefloor alone doesn't fix actualseedage. No moreidentical5.4runs.

E269 completes fixed3/3 fast_start_reentry53_30s cohort (CSV retained).
Additional02/03 PASS284.379/284.542eHz,47758/47786COM,47758/47785accepted,
raw554/519,bus10865/10793,arm43us/cost13,deadline199/117us spare,
firstpreviousage848/809us,queue2/3,DMA18us,stack4232. Allfullverifiers,
outputs/MOE/ENABLE/CCRs0,nFLT1,portsclosed. Same6775326F,no firmwarechange.
Acrosscohort recordercyclemin3351/3346/3337us (last only4us above3333floor,
NOT exactguardtimestamp). Actualguard decisions sampled separately,seeE245.
No moreequivalent5.3cohort needed. Next current-build5.4 bounded limit probe
withunchangedguards to distinguish currentsoftwareboundary, then targeted
range/current/timing review before any coordinatedlimit expansion.

E268 same6775326F fast-start build:5.3% hold10s and recovery30s PASS.
fast_start_hold53_01:283.875eHz17032COM/17031accepted,sigma52.595us,
raw517,bus11008,arm59us/cost13,firstpreviousage544us;IRQunion6053838/
10000008us valid (~60.5%,foreground NOT idle). Baseline residualsum0.780803
rawcounts,NOT calibratedamps. fast_start_reentry53_30s_01:284.217eHz,
47732COM/47731accepted,sigma44.946us,raw579,bus10901,139254phase samples,
recoveryarm43us/cost13 (11us spare),originaldeadline123us spare,
firstpreviousage829us. Bothqueue2/DMA18us/stack4232,fullverifiers/finaloff/
nFLT1,COM41closed. No flash/guard/PSU change. Next fixed bounded recovery
cohort at5.3% currentbuild (e.g.two additional30s attempts, retain failures),
then review measured range/current limit rather than endless samepoint tests.

E267 same6775326F fast-start build: three actual10s attempts allPASS.
fast_start_reentry50_01 recovered264.882eHz12697COM/12696accepted,
arm54.5us/cost13,deadline108us spare,firstpreviousage844us,raw489,bus10877.
fast_start_hold52_01 full276.479eHz16588COM/16588accepted,sigma49.411us,
arm59us/cost14,raw497,bus10853. Initialbaseline residualsum0.196561rawcounts,
NOT calibratedamps/standstill/drift proof.
fast_start_reentry52_01 recovered276.592eHz13259COM/13258accepted,
sigma42.986us,arm48us/cost13,deadline161us spare,firstpreviousage852us,
raw505,bus10829. All queue2/DMA18us/stack4232; fullverifiers/finaloff/nFLT1,
portsclosed. No firmware/guard/PSU changes or new supply reading.
Next current-build5.3% hold/recovery then bounded repeatability/longhold,
with prior5.4% cycle-guard failure guiding limit review, not blindguardraise.
Current-build points remain singleattempts; historical foreground cohorts
do not prove currentbuild reliability or calibratedcurrent/parity.

E266 fast-start image6775326F... now installed (release/s/thinLTO).
fast_start_reentry48_01 PASS10s original budget, recovered252.329eHz,
12095COM/12094accepted, first previous-feedback age896us (104us spare),
new frame198us; recoveryarm62us/cost13, originaldeadline189us spare.
fast_start_hold50_01 PASS10s265.012eHz,15901COM/15900accepted,
sigma52.916us,49751ADC/phase samples,queue2,DMA18us,raw489,bus11020,
stack4232; initialarm59us/cost14, firstpreviousage613us.
Both fullverifiers pass;216Python tests pass. Silentcpu_01 preflight retained;
known safe-register/clock workaround then cpu_02 max2/7/10 passes.
After continuation, missing session57386 and process inspection confirm fixture
finished; raw capture verified without rerun. Liveoff/PD1/MOE/CCRs0/nFLT1
reconfirmed, COM41closed. No guard relaxation/new PSU reading/calibratedcurrent
claim. Next5% recovery before further current-build duty expansion; these
singleattempts are not a repeatability cohort. E265 NOTFLASHED is superseded.

E265 adc_phase_reentry48_01 FAILrecoveryFeedbackStale4at355us; initialsegment
2s succeeds. Freshseed1344age210arm63us/cost14;firstdeliveryinitialage660,
decision352,acquired147,fault4=>previousage1012(newframe205). Exact12uslate,
NOTstaleframe/latearm.1ADCframe,1COM0acc,queue1,raw29,bus11665,stack4232,
finaloff/nFLT1,COM41closed. No5%attempt/no guardrelaxation.
New opt-inbench-dma-fast-start firstdelay1us (was101), steady201 unchanged;
stoppedCNT preload/timestamps alreadygeneralized. DMASTART accepts1/101,
215Pythonpass inclrealfailedcapture andrejectfalsefault0. Release/s/thinLTO
build6775326FD8420D4C35E2D4E360F36AF1377B8133E0838B83082D56FDADE23D47,
text114804/data1088/bss29156,E264features+fast-start. NOTFLASHED;
installed142AEAC3 unchanged. Next qualify1usfirsttrigger/cost/recovery.

E264 sameinstalled142AEAC3... phaseDMA build actual3attemptsPASS:
adc_phase_reentry45_01 10soriginalbudget234.451eHz11237COM/11237acc,
recoveryarm75us/cost13,deadline216usspare,firstpreviousage981us(19spare),
queue3,DMA17us,raw471,bus10507,stack4232. Reason0foregroundwindowcomplete
is accepted byfullverifier; notlackofguardproof. Thenhold46_01 10s239.818eHz
14389/14388,raw463,bus10948,arm66/cost13;hold48_01 10s252.080eHz
15125/15124,raw467,bus10948,arm50/cost14. Both49750ADCdelivered/49751phase,
queue2,DMA17us,stack4232. Allfullverifiers/finaloff/nFLT1;COM41closed.
No flash/guardchanges/newPSUreading. Next4.8%recovery beforefurtherduty;
singleattemptpoints notfixedcohort/reliabilityproof. Currentnotcalibrated.

E263 documentation consolidation completed: PORTABLE_WINS/BEMF_PARITY_PLAN/
TIMING_HEADROOM currentsections supersede oldstatus. Sourcearchive2799b284...
128selectedfiles verified; FALCONarchive94df4240...three metrics reproduced.
Comparisonexplicitlylimited: qZCwindow vsCOM:event notsame denominator,
windowsigma vscyclesigma notsame, nominalcurrentdifferentrig, flashedFALCON
identitynotfrozenAM32proof. MissingrefbenchNOTblocker. E262phaseDMAhold
COMP/COM/commitbrackets115/79/45us vsE258nophase100/73/39;notcontrolledA/B.
Portablecontracts includesresetallentrypaths,firstADCdelayvssteadycadence,
ownership,epochidentity andmeasurementsemantics. Nofirmware/UART/probe/motor
change; installed/root142AEAC3... unchanged. Next currentbuildrecovery/range
andcurrentevidence,notanotherdocumentation-onlyequivalentreview.

E262 adc_phase_hold45_30s_01 full30s4.5%PASS234.442eHz42200COM/42199acc,
149253ADCandphasewords,queue3,DMA17us,raw479,bus10471,stack4420.
Initialarm67.5us/cost13,firstdeliveryoldage582us. Sameinstalled142AEAC3...
No flash/codefirmwarechange;allfinaloff/nFLT1,COM41closed.
Hostdrv_baseline_residual requiresfullINITIALhandoff+completebaseline match,
REFUSESrecovery,neveramps. Newresidual+8.823588counts vsE260-2.202245;
differentdurations/startup confounded,notdrift/currentdifferenceproof.
214Pythonpass. AsyncPSUreadingrequestedforactualhold,NOreplyreceived;
do notlabelcaptureindependentanchor. Nextbaselinevalidity/reference/parity
andrange work,notrepeatidenticalholds waitingforoperator.

E261 IMPORTANT correctiontoE260:nonuniformTIM1triggerbins DO NOTprove
samplingbias. sixstep_write resetsTIM1eachCOM; actualcounterdwellunequal.
Flatbinweightingcouldbias timeaverage. Addedtestedpurehostpwm_time_occupancy
forEXPLICITresetintervals;syntheticcounterexample,NOTE260dwellreconstruction.
Decoderreports sampling_bias_provenFalse,time_occupancy_knownFalse.
212Pythonpass. E260rawsummean~16.126counts minusinitialbaseline18.328
=>~-2.202rawcounts,notcalibratedcurrent/causeproof. Needbaselinevalidity/
independentanchor,notinventedflatnessgate. No firmware/UART/probe/motorchange;
installed/root142AEAC3... unchanged. Othergoalrequirementsremainopen.

E260 installedphasebuild142AEAC31B7022BBB83F9241423590B58CC44426A0118819641D4CD5BC196095,
text114804/data1088/bss29156,release/s/thinLTO,E259features. adcphasecheck
idleTIM3self-counter source passes3x32;maxCNT0us(<1usquantized),NOADCload,
notmotorlatencybound. adc_phase_route_01/cpu_01max2/7/10pass,UARTresetworks.
adc_phase45_01 full1s4.5%PASS233.710eHz1402COM/1402acc,4973ADCdelivered,
4975phaseconsumed(preFIFO,2taildifference),queue3/8,DMAmax17us(old11),
raw432,bus11092,stack4420,arm59us/cost13. Firstdeliveryoldage875us.
HistogramALL32binsnonzero,min92/max401 vsmean155.47:nonuniformtriggerdensity.
Do NOTinfer unbiasedcurrent/aperture/sector or qualifylatencyfromidleprobe.
Allfinaloff/nFLT1,COM41closed. Next addressmeasuredcoverage bias/current
method andnewoverhead/recovery qualification,not blindlymorelongholds.

E259 optionalbench-adc-phase implemented NOT FLASHED. SpareDMAch3/mux2
TIM3_UP37(cachedHAL) readsTIM1CNT onADCtrigger,twohalfwordcircular slots.
ExistingADCIRQ consumes matchingindex/flags/NDTR,refusesphasefailure13,
initfailure12.32bins200TIM1ticks,wholeconsumedstream(not192earlycap).
No extraIRQ; ADCIRQ/setup cost andrequestlatency NOTbenchqualified.
AP85/ADCPHASE declares triggeronly,aperture/sector/latencyunknown; hostCRC/
counts/order checked,208Pythonpass. Counts referphaseconsumed beforeFIFO,
not necessarilydeliveredADCrows; no calibrationclaim.
Root48B4DEB225CABBB94B8ECAD0E53279DFF10C62A7694E35F05B9C44064111AC6B,
text113924/data1088/bss29156,release/s/thinLTO,E258features+bench-adc-phase.
Installedremains314A45...;noUART/probe/motor. Next disabledroute/latency
qualification thennewISR/startupbudget benchcheck BEFOREtrustingcoverage.

E258 earlyfirsttrigger314A45... NOW INSTALLED; two sameboot actual4.5%
recoverypasses10s/30s(originalbudgets). early_trigger_reentry45_01 and
early_trigger_reentry45_30s_01:233.873/234.370eHz,39739/139244ADCscans,
queue2/3,DMAmax11us,raw471/510,bus10996/10471,stack4376.
Firstdeliveryinitialage642/636+decision301/300=943/936us<1000;acquired142,
fault0. Recoveryarm77.5/76.5us,cost14;deadline172/164usspare.
BASEEPOCHchecked1/matches0,allfinaloff/nFLT1,COM41closed.
UARTcpu_01silent retained,exactsaferegs+clockworkaround,cpu_02max2/7/10.
206Pythonpass inclactualfirstdeliverytimestamp/staleclaimnegative checks.
Same features/profile/size asE257pendingimage. No calibratedmean,coverage,
higherduty201usqualification or independentqzc claim. Nextcurrentcoverage/
calibration andstagedenvelope work; no endlesssamepointrecoveryloop.

E257 diagnostic69F29... installed; first_delivery_reentry45_01 FAILreason4
at409us,recovery initialage674,FEEDBACKFIRSTseen0: expires326usbeforefirst
guard delivery. ADC1row/queue1 exists but guard alreadyrevoked atdelivery
criticalsection.1COM0acc,seed1452age212arm75.5us/cost13,stack4376.
Finaloff/nFLT1. UARTcpu_01silent;exactsaferegs+clockrestore,cpu_02 2/6/10.
NEW NOTFLASHED firsttrigger101us withsteady201 via stoppedCNT preload100;
timestamporigin+101+(count-1)*period,spacingunchanged. DMASTART explicit.
No UG/extra trigger/guardchange. Root314A45EAE39646396E3B955A3F225E566B7561A503C24D8B17E287609C6DD118,
text113328/data1088/bss29020,release/s/thinLTO,samefeatures. Installed69F29...
Next hardwarequalification first-trigger fix; no claimrecoveryresolved.

E256 first DMA-delivery diagnostic implemented NOT FLASHED. FEEDBACKFIRST
seen,initial_age_us,decision_us,previous_us,acquired_us,fault captures actual
first feedback_aged call inside existingcriticalsection, original timestamps.
Resetwithstats; initialage stored afteradmission. No guard/offset changes.
seen0 means no captured delivery (e.g. guard stopped first); cannot infercause.
Puretest proves freshframeage167us can stillfail expiredpreviousage1059us;
650us initialage is SYNTHETIC,not E255measurement.163Rustpass.
Root69F29BD9CFD833528BFDB65C1B152809645103125A0C29F904ABF3DEA76EA9C6,
text113220/data1088/bss29020,release/s/thinLTO,sameE255features.
Installed remains323A7CF8... No UART/probe/motor thisentry. Next qualify
diagnostic timing and capture recovery delivery; no causalhardwareclaim yet.

E255 sameboot bug fixed: stage_driven skipsprepare soCURRENT_SUMS retained
prior4974rows; hold45_01 failsfirstframe DMAcode10/countmismatch,notload.
reset_run_statistics nowclearsDMA sums beforeacquisition;prepareclearretained
forrefusals. Installed/root323A7CF866B845AF6F51F84E8135C2D97FF46D0FC2484259CBA8840DC111AA1C,
text112980/data1088/bss28992,sameE254features release/s/thinLTO.
reset_hold45_01 full10s234.256eHz14055COM/14054acc,49749scans,queue2,
DMA11us,raw491,bus10686,stack4660. NextSAMEBOOTreset_reentry45_01 starts2s
thenrecoveryseed1449age210arm76us/cost14 but FeedbackStale4at409us:
1COM/0accepted,oneADCframe242us. RecoveryFAIL,notstalesums(lastn1valid).
Initialfeedbackage/firststreamdelivery transition needsinstrumentation;
don'trefresholdtimestamps orwiden1msguard. BASEEPOCHchecked1/matches0.
Allfinaloff/nFLT1,COM41closed. UARTsilentcpu_01retained,exactsaferegs+known
clockwrite restores;cpu_02max2/7/10pass. No qualified201usrecovery yet.

E254 explicit bench-dma-201 profile installed. DefaultDMA remains101us;
PERIOD_US shared byTIM3ARR,derivedtimestamps,checked current_sums cadence
andDMAFEEDBACK. Host supports ONLY101/201,legacyabsent101; mismatchesrefuse.
No COMP/age/current/FIFO guard changes. driven_dma201_45_01 full1s4.5%PASS
233.335eHz1400COM/1399acc,4974scans253..999826us,queuepeak2/8(vs8prior),
DMAmax11us,raw456,bus11104,stack4660,arm66.5us/cost13. CPUIRQ567387 of
1000009us,FGnotidle. Baseline128/channel13266us sameepoch1,nooffsets.
Installed/rootA662E01E79CACE09CC301E608C20A42972ECB5C507312C550B91BAC23E7248AE,
text112964/data1088/bss28992,release/s/thinLTO,E252features+bench-dma-201.
ResetUARTsilentpreflightbase_01 retained; exactsafeRCC08000000/PD1ODR0/
BDTRc1a/CCRs0 thenknownRCC08040000 restored. base_02 4/4,CPU2/6/10pass.
Finaloff/nFLT1,COM41closed. Onepass NOT sustained/recoveryqualification or
calibrated/unbiasedcurrent. Next longerhold/recovery withtiming/coverage.

E253 combinedDMA image95449374... NOW INSTALLED, motor trial FAILED.
ResetUARTworks; driven_dma_base_01 4/4, cpu_01max2/7/10,
archive_01 3x3 pass. driven_dma45_01 requested1s4.5%,actual10844us,
reason4FeedbackStale,14COM/14accepted.97consistent101us scans:first153,
last9849;guardstop10809. Queuepeak8/8,DMAmax11us,raw319,bus11366,
stack4700. CPUunion6897/10809us IRQ,3912 foreground(notidle),fault0.
Initialarm54us/cost12,baseline128/channel13266us sameepoch1.
Allfinaloff/nFLT1,COM41closed. Data consistency is NOT motor qualification.
No code/guard change or repeated identicalrun. Investigate consumption cost
or explicitly slower phase-decohered sample profile; do not widen1msage or
drop/refresh queued samples. Historical101usDMA success used differentbuild.

E252 opt-in bench-driven-dma added (impliesdriven-handoff+dma-feedback),
NOT FLASHED. Other driven+DMA combinations still compile-refuse. Reuses
existing101us TIM3 ADC/FIFO after forced release; start refuses code11 if
driven/probe owner active,TIM3CEN/DIER nonzero,TIM1CC4DE or DMAch2EN set.
Masks/unpends old TIM3 vector before ADC trigger configuration. Existing
stream ADC/ch1-idle checks,FIFO/current/age guards and foreground ADC restore
remain. DMAOWNER fixture marker strictly parsed; declaration NOT timingproof.
200Python/161Rust pass; new live combined ownership/timing NOT tested.
Root954493745B8728D0B1A141A72060F88AE5155656D85904FC9FC1AD1EECF848EC,
text112944/data1088/bss28992,release/s/thinLTO,E251features+bench-driven-dma.
Installed remainsE251F7C7CA0D... No UART/probe/motor thisentry.
Next disabledchecks then finite low-point combined qualification; measure
stream start delay/first feedback,FIFO,ISR cost,stack and end/restore before
recovery/higherspeed. Coverage/aperture/baseline drift still unproven.

E251 recovery baseline provenance hardware-qualified on one10s5.3% attempt:
prebase_epoch_reentry53_01 completes injectedloss/recovery284.393eHz,
13633COM/13633accepted,cycle sigma36.419us,raw400,bus10972,stack4608.
PREBASE128/channel13690us entry_same_epoch1; new BASEEPOCH checked1/matches0
after recovery wake BEFORE fresh acquisition. Initial stats/token unchanged,
initial_records_only1; no offsets or guard changes. Actual recovery seed1192,
age210,remaining44us/cost14,originaldeadline93usspare. Finaloff/nFLT1.
Installed/rootF7C7CA0DECB2330EC95E1C0B56422DB742CDD73C89E2CB176299D57E80D479F9,
text110276/data1088/bss28788,release/s/thinLTO,samefivefeatures asE250.
UARTsilentpreflight_01 retained; exactsafeRCC08000000/PD1ODR0/BDTRc1a/CCR0
then knownRCC08040000 workaround restoresconsole. Baselinecheck_02 4/4,
CPUmax2/7/10 pass. No livefixture,COM41closed. Not calibratedcurrent;
next timed/unbiased powered sampling plus baseline stationarity/drift evidence.

E250 baselinehardwarepath qualified narrowly. basecheck actualcases off,
abortafter10conversions,ENABLEdropafter10,complete+rewake tokenrefusal. Both
builds4/4pass:partial2/channel~214..215us,complete128/channel13.54ms.
Firstprebase_handoff53_01 neverdrives: leftoverCRLF LF triggersabort at2us,
0samples. Retainedfailure/finaloff. RUNscan now ignoresONLY CR/LF terminators;
allotherbytesabort. Correctedprebase_handoff53_02 full1s5.3%PASS1697COM,
raw400,bus11128,stack4904,arm56us/cost13. PREBASEstatus2 n128elapsed13690us
entry_same_epoch1 atPRETRANSFERstaging;stationary_verified0,offsets_applied0.
Notstandstill/currentaccuracy proof. Hostfixture nowfailsfast/drainspartialBZ
onbaseline refusal instead of15stimeout/rampcommands.197Pythonpass.
Installed/root6729CBE34266057713DDF7B11E9A988CDC7FEA61A63BFE171690B77438E582E4,
text110072/data1088/bss28784,release/s/thinLTO,prior4features+current-baseline.
BothresetUARTworked;CPU2/6..7/10passes. Allfinaloff/nFLT1,COM41closed.
Next epoch-invalidation provenance forpoweredrecovery andtimed/unbiased
currentmeasurement; don'tderivecurrentfromrawbaseline+biasedlaunchhistogram.

E249 prestart_baseline optionalbench-current-baseline(impliesepoch) implemented
NOT FLASHED. RUNafterexistingwake BEFOREpwm_sine:128samples each4/1/0/6/13,
same poweredreader,50msdeadline/hostabort/offgates/epoch+faultchecks. Refusal
disables and cap1 prints partialraw evidence; successkeepswake,appliesNOoffset.
Tokenretained; driven pre-transferstaging entry_check records sameepoch1 or2
(0notchecked),neverauthorizeshandoff. FixtureonlyPREBASE/BZ85 rawmoments.
stationary_verified=0 explicit: don'tcall this calibratedzerocurrent.
Host decoder checksCRC/moments/counts/deadline,legacyabsencesupported;196Python
tests pass,normal+optionalrelease/s/thinLTObuilds pass. Sourceguardthresholds
unchanged; newentrycheck timing andbaselinepath NOThardwarequalified yet.
Root9BE065B5762F01F0C5A4AAA7F6A96A334F87DF1332F9DE2A5DD8D6BC6EC8D3CF,
text109356/data1088/bss28784. Installed8E119A1D...E248unchanged;noUART/probe.
Next qualifyboundedbaseline refusal/success thenfirstsamewakehandoffcapture;
needepochrecoverynegativeevidence,stationarity/drift andunbiasedpoweredsampling.

E248 optionalbench-current-epoch liveadapter wired atset_pin(3,1),singleton
calibration_live. Existingcriticalsection serializes physicalPD1write FIRST
then Epoch.commanded with begin/matches. Otherpins/defaultfeatureunchanged.
RawGPIODsupportaudit findsadapteronly; panic callsset_pin(false). Diagnostic
epochcheck wakesCSAonly/gatesdisabled,doesNOTrestoreoldtracker/tokens.
Hardwareepoch_check_01 5/5 on3trials,repeatedhigh256sum184us/max1us (initial
gate<=5us). archive3x3/CPU2/6/10pass. Actualepoch_reentry53_01 PASS10soriginal
2sinjection/~7.99s recovery284.951eHz,arm43.5us/cost14,deadline132usspare,
stack4752,COMmax59/commit25us. Allfinaloff/nFLT1,COM41closed. No baseline
captured/applied yet;epochcheck not proof of realbaselinethroughhandoff.
Installed/root8E119A1D18EA06D0B91AB5E058336EED7301AD2A427A7D182E9509E7A14505AD,
text107932/data964/bss28764,release/s/thinLTO,prior4features+current-epoch.
ResetUARTworkedthisflash,nopermanentfixclaim.161Rustpass. Nextbounded
prestartstationarybaseline withsameADCsettings/hostepochprovenance; keep
recoveryzero invalidation andunchangedrawguards. Goal incomplete.

E247 pure calibration_epoch prerequisite added,NOTwiredinto firmware.
Singleboard/boottracker/nonCopytoken: repeatedENABLEhigh preservesidentity,
everylow invalidates; badphysicalreadbackinmatches revokes permanently;
generationoverflow latchesrefusal. beginrequiresenabled/nFLT/gatesoff but
doesNOTprove stationarity or settling. 6newtests,161Rusttotalpass,M0checkpass.
No GPIO/offset authority; installed/root362CE448...unchanged,noUART/flash.
Next live singletonadapter mustserialize everyPD1write+metadata/tokenuse
(auditset_pin/raw/panicpaths),physicalshutdownfirst,noreusedtrackeridentity,
then boundedprestartbaseline integration. Tests NOT actualepoch/calibration
proof. Need performancequalification before addingadapter to hotCOMset_pin.

E246 currentevidence: driven_run fixture dump omitted AL85 despiteexisting
powered ADC_COVERAGE collection. Added coverage_dump afterpoweredsummary,
fixture-only/poststop; no new runtime sampling. current_coverage53_01 one1s
5.3%PASS283.48eHz1700COM/1700acc,raw392,bus11140,stack5052,finaloff/nFLT1.
192attempts/channel,reject9/9/5;bins A[8,10,16,33,24,20,23,49],
B[16,11,29,34,26,23,25,19],C[8,21,26,26,29,24,28,25]. Unevenlaunchcoverage;
not unbiasedmean/aperture/jointsector proof. Morelongholds addNOdata to192cap.
Important E185epochaudit update: successful underdrivepath preservesENABLE
fromstartup through reason22/adopt/stage/start. Thusprestartstationaryzero
canpotentiallyshareINITIALsegmentepoch now; recovery stillprepare+wake resets.
Need explicit epoch provenance,stationarity/drift/aperture/coverage before
claimingcalibration. Nextimplementsame-wakeinitialbaseline+timedmeasurement,
notrepeatstandalonezero orforcecurrentfromthesehistograms.
Installed/root362CE448EA84407BDB7D6813CE1AFACD6EA1C6F943291F9E467D05311C193847,
text104376/data964/bss28756,release/s/thinLTO featuresunchanged. ResetUART
silentpreflightretained;exactsafeRCC08000000/PD1ODR0/BDTRc1a/CCR0 thenclock
08040000 restored. Archive3x3/CPU2/7/10pass.194Pythonpasses;COM41closed.
One-secondpassnotnewrecoverycohort;priorE245recovery remainspriorbuilddata.

E245 exactguardfault hardwareproven. Installed/root6406EA73...E244 flashed;
UARTresetworked(no workaround),archive3x3 andCPU2/7/10 pass. Actual
cyclefault_reentry53_01 PASS284.916eHz,recoveryarm41.5us/cost13us,raw400,
bus10960,stack4752,deadline93usspare. cyclefault_range54_01 FAILreason12
after161956us271COM/270acc; CYCLEFAULTstep1 previous158616 decision161947
delta3331<3333,stop9uslater. Recorder min3348 DIFFERENT,notguardvalue.
Raw387,bus11462;allfinaloff/nFLT1,portclosed. No speedguardwidening.
Exactboundaryissoftwarecyclefloor,NOT provenmotor/MCUmaximum orphysical
300eHzoverspeed. Existing193Python tests include real3331vs3348 distinction.
No moreequivalentfaulttrialsneeded. Nextcurrentmeasurement/parity and explicit
sensing/timing/current case before coordinatedguardexpansion. Goal incomplete.

E244 OFFLINE correction/diagnostic. E243's3329us is ACCEPTMOMENTS recorder
minimum, NOT provenactualguardrejection delta. powered_timer::accepted(now)
runs BEFORE observation_elapsed recorder sample; rejected event omitted
(LATE_ACCEPTS1). Retained54tail same-stepcycles3346..3541;1034middleevents
missing. Don't claim3329causedstop or derive rotor speed from it.
New fault-only CYCLEFAULT [step,previous,decision,delta] captured from SAME
now passed to guard,inside existingcriticalsection,beforetrip. Guardgetter
doesnotaddinstanceRAM; static16bytes; resetwithstats. Labelonlyreason12,
zerosmeansnonsnapshotted/non-eventrefusal. No threshold/controller change.
Host drv_cycle_fault validateswrappingarithmetic/reason/profile; integrated
sustainedreport. Legacyabsencemeansunknown,notfabricatedsnapshot.
192Python/155Rust tests pass; experimentalrelease/s/thinLTO builds.
Root6406EA73851577B13F795AFDABA1A92B32A43CEE6A874EAF0F13672FCC07867E,
text104464/data964/bss28756. NOT FLASHED; installed460C0BD8 unchanged.
No UART/probe/motor thisentry. Next disabledchecks/timingqualification then
one5.4diagnostictrial; do notwiden3333floor from recorderextrema.

E243 dutyboundary: mask_range54_01 FAILCycleTiming12 at0.637s,1099COM/
1098acc,mincycle3329<3333us,mean287.76eHz,raw405,bus11056. No guardwidening,
no5.4recoverytrial. mask_range53_01 full10sPASS284.96eHz. Then fixed
mask_reentry53_01..03 ALL3PASS startup+2sloss+remaining~7.99s recovery at
285.46/286.27/285.23eHz. Seed1189/1187/1185,age210ticks,arm43.5/43.5/43us,
cost14us,deadline219/164/120usspare. Raw404/399/394,bus10877/10948/10984,
stack4768. Cyclesmin3343/3351/3346 only10..18usabove3333floor: expansion
currentlycycle-guard-limited,notlatearm. DoesNOTprovephysicalrotor>300eHz.
All5actualattemptsretained,finaloff/nFLT1,CPUfault0,COM41closed. Sameinstalled
460C0BD8... noflash/codechange. Nextcurrentcharacterization/shortcycleanalysis
beforeconsideringcoordinatedspeedguardexpansion;don'trepeat5.3cohort.
5.3%qualifiedinthisboundedcohort,notarbitraryduration/recoverydisturbance.
No calibratedmeancurrent or independentqzc claim. Goal incomplete.

E242 UART restored and actual recovery boundary cleared. Flash verify passes.
USART3 clock08040000/pinsAF0 correct,CR1/BRR0,CPUmain; no reset held. With
PD1ODR0/MOEoff/CCRs0 confirmed, wrote USART3 BRR0x22b (64MHz/115200) and
CR1d(UE/TE/RE). Console returns; underlying init/reset fault NOT solved.
archive_mask_restored_01 checks3x3,archive_mask_cpu_02 max2/7/10 pass.
archive_mask_reentry50_02 PASS265.79eHz,seed1267age210,remaining53.5us.
Then fixed5.2% cohort archive_mask_reentry52_01..03 ALL3 PASS complete10s
originalwindows with2s loss injection and~7.99s recovery at278.26/278.74/
278.46eHz. Newseed1212/1215/1211,actualage210/210/212ticks;arm46.5/47/45.5us,
cost13/14/13us;gap83/85/84us vs failed101. Originaldeadline95/114/174usspare.
Raw398/386/404,bus10948/11032/11104,stack4768. CPUunionfault0,sumvalidated;
not CPUutilization or calibratedmeancurrent. All4motorattemptsfinaloff/nFLT1,
COM41closed. Same installed/root460C0BD8...E241 now hardware-tested; noflash
thisentry. Maskedchecks fix observed sampling failure on shared/direct-report
build; no isolated attribution of all18us arm saving to one codechange.
Next current/parity and stagedenvelope work; no needmoreequivalent5.2%cohort.
Don'tforget UART reboot wart before newfixture. Goal incomplete.

E241 shared runtime-destination acquire_inner replaces const specialization;
nm confirms one function, direct archive writes retained. Disabledcheck3x3 and
CPU2/7/10 pass. archive_shared_reentry50_01 drives2s3176COM/3175acc,peak410,
bus10996,then recovery gap202 after11intervals/7904us:FAIL. Specialization
alone not cause. No latency win or5.2%retry. Finaloff/nFLT1 verified on thisrun.
Next narrow optimization outputs_disabled reads A mask0x780 and B mask3
once each rather than6generic reads; same6pins+MOE,enable/fault unchanged.
Exhaustive65536values/port equivalence passes. This build NOT UART-qualified:
archive_mask_check_01/_02,archive_mask_cpu_01,archive_mask_reentry50_01 ALL
preflight failures; NO motor on maskbuild. Do not treat fixtureinvocation asrun.
Known clockwrite no longer restoresUART: RCC08040000 but USART3 CR1/BRR0.
Safe readbacks PD1ODR0/BDTRc1a/CCR0. Briefhalt PC0800c7cc mapsmain; resumed
viaDHCSR. No faultstatus indication. Probe-rs debug CLI itselffailed DAPparse.
Need diagnoseconsole initialization before motor commands; not blindlyretry.
Installed/root460C0BD89A64ECF29E3D9A06F2CD3BFCFD87271AA38F559A9681C7799D9A561B,
text104208/data964/bss28740;release/s/thinLTO experimental features unchanged.
No new UARTfinaloff confirmation on maskbuild; registers prove output-off.
Shared failedcapture/preflights retained. Goalincomplete; no livefixture.

E240 actual archive optimization trials FAIL recovery acquisition. Added
idle-only archivecheck (bench-reentry-staging): actual initial/recovery sinks
and awake-disabled refusal; snapshots restored. Fixture drv_archive_check.py
retains raw and finally verifies off. 3/3checks on3trials pass both builds.
CPU overhead2/7/10 passes both. archive_reentry50_01/_02 start2s,inject loss,
then FLYresult7 maxgap202ticks=101us>100us limit, after3/2intervals. No seed,
no recovery power. Inline-never acquire_inner change DOES NOT fix it:
archive_noinline_reentry50_01 samegap202 after6intervals. Keep all3 FAILS.
Host error says reentry statistics provenance because used0; real refusal
is earlier EdgeFilter gap,not archive corruption or arm latency. Need isolate
acquisition sampling regression before more powered repeats/higher duty.
No measured copy-removal latency win. All3 finaloff/nFLT1,stack4784.
Both flash resets UARTsilent; exact RCC08000000/PD1ODR0/BDTRc1a/CCR0 checked
before known RCC08040000 workaround. Failedpreflights archive_routing_01 and
archive_noinline_check_01 retained. Installed/root C4F26BF28A73525DE526FBCC25B82C6001C18278687DF0D63C6D888EB98EF116,
text104960/data964/bss28740,release/s/thinLTO,featuresreanchor,cpu-union,
range300,reentry-staging. NOT recovery-qualified; don't claim installedE237.
189Python pass; latest experimentalbuild passes. COM41closed;goal incomplete.

E239 OFFLINE recovery-report optimization, NOT FLASHED. Acquisition now
specializes diagnostic destination: recovery writes its own REPORT/FAILURE/
EDGE directly, avoiding post-seed copy-out/restore of original evidence.
Selection/pending/sector and disabled-baseline restoration remain; awake
baseline timestamps unchanged. No guard, dwell, qualification/control change.
Old compiled path copies two64-byte reports and two26-byte failure arrays
after acquisition. NO measured time saving yet. Entry57us includes20us dwell,
qualification/cleanup/caller work, NOT57us removable reporting overhead.
Normal+experimental release/s/thinLTO builds,189Python/154Rust tests pass;
existing tests do NOT directly execute new firmware diagnostic routing.
Next disabled archive isolation/refusal check, then measured recovery with
unchanged32us floor. No5.2% recovery qualification claimed. New specialized
reacquire inlines acquisition,frame492bytes: remeasure stack, don't compare
against old wrapper148bytes alone (old acquisition had its own nested frame).
Root experimental DA6B5F4B45EA0A4CEF98BC8F9BD86438D9C97FB74423B7538193732C4E3EEF79,
text102776/data964/bss28740 (+1708text). Installed remains63801C85...E237.
No UART/probe/motor action this entry. Goal incomplete.

E238 staged_range52_01 full10s5.2%278.166eHz,16690COM/16689acc,raw391,
bus10913,IRQ53.785%,stack4856. staged_reentry52_01 FIRSTdrives2s3330COM,
Trackingstop,newseed1212/actualage246 leaves57ticks28.5us<32:REFUSEarmed0.
No higher-speedrecoveryclaim. Original5%3/3recoveryqualificationretained.
current_anchor50_01 full30s5%266.319eHz,47937COM/47936acc,raw403,bus10901,
CPU15561338/30000010usvalid. OperatorPSUreadingrequestedASYNC,notreceived;
filenameisopportunityNOTmeasuredcurrentanchor. No standalonezero taken:
E185ENABLEepochs preventapplyingit to laterpoweredsegments. Don'trepeat
zero/holdsjusttoavoidmissingindependentreading;othergoalworkcancontinue.
PORTABLE_WINS consolidatedE225..238 mechanisms/constraints. Frozenminz128
files--check-source passesarchive2799b284...;no referencebenchrerun.
189Pythonpasses;firmwareunchangedinstalled/root63801C85... all3attempts
retainedfinaloff/nFLT1,COM41closed. Nextmean-currentmethod/parity/envelope
work, with5.2%recoverylatency anactualidentifiedlimit,notwidenedaway.

E237 earlyrecoverystats optionalbench-reentry-staging lands. Acquisition's
gates_off revokesflags,so stageAFTERthatshutdown andBEFOREfirstmeasurement,
awakeTracking8only,outputsdisabled. Preservesreason8/STOP_USuntiladmission;
start_innerconsumesflagonce,skipsredundantgatesoff/statreset,stillfreshguard/
feedback/seedchecks. stoprevokesflagEVENinactive. Idlecheck3/3,transfer8/8,
poststop18/18,record12/12;CPUoverhead2/7/10passes. REENTRYSTATSused1marker.
reentry_stage48_01PASS252.823eHz,recoveryarm45us vs prior28.5refusal;
SEEDLATguardstage43->28us. reentry_stage50_01..03 ALL3PASS266.03..266.08eHz,
originaldeadline margins159/221/97us;arm36.5/34.5/36.5us,cost13us.
_01ACTUALLYexercisesreanchorcount1 epoch3 thencompletehandoff/recovery;
_02/_03count0. SeparateDS/DX/DFA/recoveryseed andfullCPU/archivechecks pass.
5%raw391/388/383,bus10937/10972/10841,stack4856. No widerangecompletionclaim.
Installed/root63801C8567644EC6327576F3AEECB75B7C51DE87C98F6E46213EB62248D94C36,
featuresreanchor,cpu-union,range300,reentry-staging;release/s/thinLTO,
text101068,data964,bss28740.188Python/154Rustrange tests,bothbuildspass.
All4motorattemptspassfinaloff/nFLT1;COM41closed. Nextcurrentcharacterization/
furtherstagedexpansion andportableparityconsolidation,notmoreequivalent5%runs.
Recoverymarginstillonly2.5..4.5usabovefloor;donotextrapolatehigherrecovery.

E236 4.6%instrumentedrange recoveryPASS cpu_union_range_reentry46_01:
240.525eHz,11529COM/11529acc,remaining77ticks38.5us/cost13,deadline137usspare,
raw368,bus10698,stack4872. 4.8%oldbuildattempt_01missesepoch2,DSfault2,
nohandoff;laterDI3..15wouldqualifyfresh12intervals inoffline replay.
AddedOPTIONAL bench-driven-reanchor (implieshandoff): ONEreanchoronlyon
forwardgap2..6withconsistentphase, beforecompletedseed;duplicate/badtime/
secondgaprefuse. NewRuntimeAcquirekeepsORIGINALstart/deadline;nojoininggap.
DRIVENSEEDRESTART count/anchor marker;hostselectsnew13edgesandverifiesrawDI.
Offlineactualtrace seed1566/edge24930 proved;oldcapturestillfailsmotorverify.
Installed/root28D440C07A444B6219D01862E16B66FCCE543A70CE0C54DCAAB206F87713709C,
featuresreanchor,cpu-union,range300;release/s/thinLTO,text100100,data964,bss28740.
reanchor_reentry48_01 starts2s/3026COM thenTrackingstop,passiverecoveryseed1338,
actualage278 leaves57ticks28.5us<32floor:REFUSES,recoveryarmed0. Reanchorcount0,
so newrestartbranchNOTbench-exercised. Allfinaloff/nFLT1,COM41closed.
Next recoverylatency: SEEDLAT114/146/160/246/254half-us; guardstage43us includes
start_inner gates_off/reset_run_statistics. InvestigatepreparingstatsBEFORE
passiveacquisition whilepreservingreason8/archive/one-shotcancel,notlowerfloor.
186Python/154Rustrange tests pass. Keep failedattempts;goalstillincomplete.

E235 realinstrumented4.5%recoveryPASS cpu_union_reentry45_01,234.30eHz,
originaldeadline159usspare;remainingarm64ticks=32us EXACTfloor,cost13us.
CPUunion52.524%,resumed11230COM/11229acc,raw357,bus10674,stack4864.
Normalprofile4.6% cpu_union46_01CycleTiming12stop1.244s (~239.5mean),retained.
Thenexplicitstaged bench-range300 profile enabled (3333uscycle/277event,
coordinatedpassiveacq6666/554),electrical/tracking/deadlinesunchanged.
Newbuildoverheadmax2/7/10passes. cpu_union_range46/48/50_01 all10sPASS:
240.196/252.791/266.095eHz;IRQ52.503/52.341/52.910%;raw368/389/383,
bus10972/10769/10913;COM14412/15167/15965,acceptedonefewer,stackmin5140.
Thesearesingle-point sustainedpasses,NOhigher-dutyrecoveryqualification.
Installed/rootD9E8C1C4CFF7F15AA86187CD381806818498A83DB1AC065A2D2E1032B558F4CE,
featuresdriven-handoff,cpu-union,range300;release/s/thinLTO,text99752,data956,
bss28740. UARTresetworked. 182Python/151Rust(rangeprofile)pass.
All5motorattemptsretained,finaloff/nFLT1,COM41closed. Nexthigher-pointrecovery
andcurrentcharacterization,thenstagedrange/repeatability;noforced30%target.
Don'tclaim52%IRQ=48%idle orrawcurrentpeak=calibratedmeancurrent.

E234 aggregate bench-cpu-union impliescpu-timing; distinctCPUUNION/CU85two
contexts0foreground1IRQunion, outerboundariescharge+nestedtimeonce. Separate
irq_union puremodule testedagainstpervector10sreference andall6nesting.
Idleunion_overhead_02max11FAIL; omitnestedTIM17reads ->_03max2/7/10PASS,
means1.5625/6.375/9.875us. HeaderCPUCHECKTYPEunion1 verified. _01UARTsilent
preflightretained,exactsaferegsbeforeRCCworkaround;finalflashresetUARTworked.
Installed/rootCAD4FB0C7AE48939A377AA94F8EC44E5AB0B10387C83061B01B874A840880AC1,
text99728,data956,bss28740. transfer8/8,poststop18/18,record12/12disabledpass.
ACTUALMOTOR cpu_union45_01=1s233.55eHz,1401COM/1400acc,IRQ522718/1000008us
(52.27%);_02=10s234.45eHz,14067/14066,IRQ5278743/10000010(52.79%).
Both4.5%/+60underdriveentry;CPUfault0,maxgap95/97us,nesting2,sumvalid.
Rawpeak347/359,busmin11151/10937,stack5160. Arm49/36.5us,cost12/13.
No calibratedCPUutilization: foregroundnotidle,probe/exceptioncostnotremoved.
CPU85legacysemanticsretained;decoderrejectsmixedprotocol.181Python/151Rustpass,
M0libcheckpass. Bothmotorcapturesfinaloff/nFLT1,COM41closed. Nextrecoverywith
instrumentation/currentcharacterization/stagedrangeexpansion;NOTmoreidleprobe
tuning or equivalentholds. Originalmotorlimitsunchanged;goalstillincomplete.

E233 const-generic vectorScope removesdynamicID/dropbounds;reprC hotscalars
first. cpu_overhead_05 improvesactive max9->6us,nested17->12us butstillFAIL
original<=10usallmodegate. Packed3bitnestingstack + knownIDleavecharging
cpu_overhead_07 stays6/12us. No motorcommands;allcompletechecksfinaloff/nFLT1.
_04/_06UARTsilent preflightretained;exactsaferegs checked beforeRCCworkaround.
Installed/root optionalSHA7B09BD5259A2C07AAA09A8B103D9336F9B6CA5F6E47A040295441FCA76223585,
text99580,data956,bss28784. NOTmotorqualified. 146Rust/178Pythonpass.
Next structural reduction: per-contextall-boundarybookkeeping stillcosts6us;
consider separate cheaper IRQ-union meter (outermost boundariesonly, nesting
preserved), since totalIRQpartition answers headroomwithout7way attribution.
Suchalternative needs explicitdifferentprotocol/tests;don'tmislabeloldCPU85.
No CPUutilization measured;foregroundnotidle. COM41closed,lastfinaloffverified.

E232 actual idle CPUCHECK fails predeclared<=10us allmode gate. NOmotorruns.
cpu_overhead_01 original5CE5...: inactive592/256us,max3;active2128/256,max9;
nested4288/256,max17. Inline hotmethods+nestingbitmask changedlittle:
cpu_overhead_03 inactive552/256,max3;active2104/256,max9;nested4240/256,max17.
Allfault0/finaloff verified. cpu_overhead_02 UARTsilent retainsfailedpreflight;
exactsafeRCC/PD1/MOE/CCRchecked then knownRCC08040000workaround used.
Installed/root instrumentedSHA C43B672BC2181BC72304B822B836D41DF3EDC85A7B2F594DB667703CC2026C11,
text98248,data956,bss28784. NOT motor-qualified;don'tstartspinwithmeter yet.
Next lighter accounting design / cost analysis, not moreinlinehints or relaxed
overheadgate. drv_cpu_check.py savesallattempts+finallyoff, refuses>10us,
178Python/145Rustpass. Noactiveprobe/fixture;COM41closed,lastoff/nFLT1.

E231 optional bench-cpu-timing implemented, NOT FLASHED/bench-qualified.
irq_accounting pure nested partition + cpu_meter wrappers for motor vectors;
reset powered stats, begin first powered guard (not seed arm), stop ownership,
last-segment CPU85 only. Context0foreground NOT idle;1guard2COMP3COM4TIM3
5poll6DMA. Capture-only CPUMETER/CRC CPU85, decoder in sustained reporter.
Idle cpucheck (requiresgates/ENABLEoff) measures256 inactive/active/nested
softwarepairs; NOT RUN yet. Probe/exception costs not separated, no CPU%claim.
Visible>1ms gaps/nesting/overflow invalidateaccounting, never alter guards;
16bitwrap blindness remains. Start wrappers cost evenbefore accountingstarts.
Next disabled cpucheck+overhead review BEFORE powered instrumentationrun.
175Python/145Rust tests;normal+instrumentedrelease/s/thinLTO andM0libcheckpass.
RootELF experimentalSHA5CE5A70A46D1EE867338B91427CC92F90B319B095AE3E780CC54781DB8E9E311,
text98040,data956,bss28784. Installed remains E2296A2B4D5C...;nohardwarecalls.

E230 offline timing report now replays E229 through full handoff/recovery
verification: scripts/drv_timing_report.py, TIMING_HEADROOM.md. Recovered
COMP12.829..12.845k calls/s, ADC4.201..4.208k scans/s; wall brackets COMP82us,
COM55us. These omit dispatch/early-return/exception overhead and include
preemption: NOT exclusive CPU occupancy. No max-times-rate utilization claim.
Recovery spare2..3us vs initial24..25.5us. Next runtime instrument must measure
whole-vector/nested work and its own overhead; do not widen guards from maxima.
172Python tests pass, old late recovery still rejected. No flash/UART/run.
Root ELF rebuilt optionalhandoff now matches installed E229 SHA6A2B4D5C...;
release/s/thinLTO unchanged. Goal remains open, no fresh operating qualification.

E229 THREE real under-drive entry+injectedloss+poweredrecoveries pass at4.5%,
10soriginalbudget/~235..236eHz. driven_reserve45_01..03+cohortCSV. Finaldeadline
margins145/198/232us; separateDFAinitialseed+CORESEEDrecovery+archivesverified.
SessionBudget::reentry_reserved<const R> allows200/300;legacyreentry uses200,
driven resume_once::<300> reservesadditional100us fromremainingdrive,NEVER
extendsoriginaldeadline. REENTRYRESERVE markerexplicit; missingmeanslegacy200,
unknown/duplicate markersrefuse. E228+6uslatecapture stillfails. Hoststrictend
unchanged. Recoveryarm34..35us (only2..3us above32floor!),not higher-speed
recoveryqualification. Resumed~7.988s,11265..11293COM,rawpeak356..367,
busmin10948,stack4908. 168Python/140Rusttests,bothrelease/s/LTObuildspass.
InstalledoptionalSHA6A2B4D5C7326643F258B7F5308C1132CC80CC2B0EA5E74DABCCDFDF0BE426100,
text96252,data956,bss28704. RootlastNORMALnotflashed. Allfinaloff/nFLT1,
COM41closed. Next remaininggoal: current/timing/parity measurements and staged
duty/speed expansion with identifiedlimits; not moreequivalent4.5%repeats.
Historicalstartupcurrent/missingepoch refusalsremain; new3/3doesn'terase them.

E228 connects explicit drivereentry1/0 to existing resume_once/SessionBudget,
oneattempt,trackingonly,wake+passivefresh12intervalseed,originaldeadline.
Fixture --dropout --reentry newack required. DFA85 stores FIRSTactualcorearm
(7u32pairs:ci,age,arr,arm,armed,firstelapsed,requestedwindow) before reset;
CORESEEDafterresume refersSECONDseed. Host validatesboth separately andfull
archive/deadline via existingreporter. No relaxedpasscriteria.
Fourattempts retained: reentry45_01startupcurrent3277/limit3248 abort;
_02realrecoveryseed1432 butlatearm25us refuses;_03missingepoch seedfault2;
_04physicallyrecovers~7.988s235.45eHz,11285COM/11284events,secondarm35us,
butfinal_elapsed14712092>original14712086 by6us: STRICTFAIL,notqualified.
Recoveredsegmentpeak384raw,bus10948,stack4908. Next tighten absolute deadline
accounting/reserve (200us currently consumed by setup+guard dispatch+return),
NOT widen verifier. SessionBudget::reentry hardcodes200; host equality assumes
resume+remaining+200=original. Any reservechange must be explicit/tested and
legacycapturesemantics retained. Or account actualsetup against remainingtime.
Recoveryonly coast_run_inner now usesbridge_clear forresumeSome because
resume_once already stoppedowners beforeacquisition; this saved resetstage
24->16us and allowedactualarm. Normalnonresume/non-driven stillfullgatesoff.
167Python/139Rustpass,bothreleasebuildspass. InstalledoptionalhandoffSHA
F302555E928C2BAEE4E40269A0DB4B0FB4B74C665C44990C2AD071EF94DC0E9E,
text95676,data956,bss28704. RootELF lastNORMAL(notflashed). Allfinaloff/nFLT1,
COM41closed. OtherPythonymodemprocesses onCOM8 unrelated;do notkill.

E227 explicit idle drivedrop1/0 one-shot added ONLY for handoff build; own arm
consumed by prepare_driven into existingDROP_ARM. off clearspendingarm.
Fixture --dropout permits driven_handoff ONLY,requires>2s and newack; cleanup
disarms. REENTRYstilldisabled/notimplemented here. New verifier dropoutmode
requires trackingstop+FIRSTSEGexact prefix/tail/total match,notwindowcompletion.
driven_drop45_01 passes: suppressed2000166us,disabledobservation2000943us
(777usafterinjection),2818COM/2817accepted,rawpeak359,bus11128,archivevalid,
disabledreacq12intervals8869us/mean1435,NO poweredrestart. Stackmin4956.
Sameboot control_01 missesepoch2 (epochs1,3..),seedfault2,20msobserverdeadline;
retained as failure. control_02 completes3s235.16eHz,4233COM/4232events,no
reinjection. Not a new cleanentrycohort. 164Python/139Rustpass.
Installed/rootoptionalSHA A66834FEE76C393DCE630CDE78C005D59B56D789C66FB0073A92456F06103992,
text94596,data956,bss28664. UARTresetworked; finaloff/nFLT1,COM41closed.
Next integrate existing resume_once/SessionBudget into driven_power_run,
explicitretryarming (currentlyprepare clearsREENTRY_ARM). Must preserveFIRST
seedarm metadata: CORESEED gets overwritten by resume's observe_begin,while
DS/DX describeinitialentry. Need dedicated initialarm snapshot/provenance and
originalrequestedwindow evidence before reusing fullrecovery verifier. Actual
oldrecovery is passive awake12interval acquisition,not forceddriven restart.

E226 fixed4attempt cohort at4.5%/+60/10s passes4/4 under-drive entries and
powered windows,235.23..235.56eHz,cycle sigma36.66..38.40us. Raw captures
driven_align45_01..04 and driven_align45_cohort.csv. _01 exercises initial
remainder18us,actual7us gates-off wait,nextsector recomputed from actualtime,
then full10s;others no wait. Boundary::initial_wait returns only<100us short
remainder; no extended sector,launch500us/initial100us/arm32us guards unchanged.
DRIVEALIGN provenance hostchecked,alllookup fractions/rates tested with overshoot.
Armremaining47..61.5us,cost13..14us,rawpeaks360..394,busmin10937,stackmin5248.
_03COM=accepted14131 because lastaccepted timer can remain pending atdeadline;
other runsCOM=accepted+1. No poststoprecord. This isn't independentqZC proof.
Next controlled dropout/recovery through new entry path. Existing prepare_driven
clearsDROP_ARM/REENTRY_ARM,driven_power_runsetsREENTRY_SESSIONfalse;fixture
rejects driven+dropout/reentry. Must deliberately wire/review archive/deadline/
freshseed ownership before testing; don't just send ignored dropout1.
162Python/139Rustpass,optionalrelease/s/thin-LTObuildpasses. Installed/rootELF
optionalhand-off SHA4FBE97BC43466A0FBFE59A7047B9DEF6F8364ADDB5939D78B6D0172E4E591161,
text94584,data956,bss28664. Finaloff/nFLT1verified,COM41closed,no livefixture.
Resetclock workaround again used after exact saferegister checks. Cohortdone,
don't substitute more equivalent4.5% holds for remaining recovery/range/current.

E225 acquisition ADC now yields BETWEEN completed single conversions when
driven ownership ends,discards partialscan and leaves LAST_FEEDBACK timestamp
unchanged. Normal powered reader unchanged; no ADC abort/register tricks.
DRIVENYIELD reports abandonment/channelcount. driven_yield45_01 explicitly
exercises yield after1channel: full1s,1409COM/1408accepted,234.88eHz,arm59us.
driven_yield45_03 full10s,14157COM/14156accepted,235.94eHz,sigma38.33us,
arm54us,cost13us,busmin10972mV,rawpeak361,stackuntouched5256. This run yielded
between scans(abandoned0),not evidence for partialscan branch by itself.
Samebuild45_02 initialpartialreason19 refusal:4.5% cohort2/3completed,
2/2poweredwindows complete,durations1s/10s not identical repeatcohort.
46_01 armed but CycleTiming12 at0.599s/859events (~239.36eHzaverage),retained;
no speedguard relaxation. Next resolve bounded initialpartial admission then
fixed repeatability/recovery; don't force higherduty past this measuredguard.
161Python/138Rust pass,normal/optionalrelease builds pass. Installed handoff
SHA80E6021108C8FE02103A114E55DD5CE316360265FE8DC3954E69215CEDEF6B99,
text94112,data956,bss28656. RootELF last rebuilt NORMAL (not installed).
Finaloff/nFLT1 verified by fixtures,COM41closed,no livefixture. Reset UART
worked this flash,not a proven permanent fix. CPUmax wallCOMP82us/COM69us
on10s,not exclusiveutilization. Current remains uncalibrated rawpeak.

E224 lands FIRST complete under-drive handoff20ms: driven_handoff46_05,
27COM/26accepted,221.20eHz cyclemean4520.75us,sigma66.78us,actualarm62us
remaining/11us cost,peak323raw,busmin11510mV,stackuntouched5272. NOT entry
reliability or sustained lock. Same finalbuild _04/_06 both latearmrefuse,
_06requested1s but neverpowered. Cohort1/3completed,allseedsqualified.
Stage powered recorder/coverage reset before forcedacquisition; adopt consumes
DRIVEN_STAGED and arms one-shot DRIVEN_ADOPTED,shared cancel revokesboth.
start_inner consumes adoption,skips redundant gatesoff/reset ONLY then; still
builds guard from real seed/feedback and unchanged age/deadline limits.
First staged build _03 armed56.5us but trackingstopped1ms,1COM/0events:
TIM2 frozen at286 (same as seedage),because driven release stopped it.
Fix restarts TIM2 CEN immediately after measured Interval.set_count in actual
seed critical section for explicit driven baseline. PSC31/ARR65535 shared.
_05proves actual comparator acceptance/continued COM after fix; _03 retained.
Next reduce foreground release latency (15us pass vs99/72us refusals),likely
in-flight synchronous ADC scan; do not refresh seed/timestamps or lower32us.
160Python/138Rust pass,both release/s/thin-LTO builds pass. Installed optional
handoff text93800,data956,bss28648,SHA
DD83FABEC571F9E8EC81CE917EAD1E99843ECFC522F464B0C7296A673B6EB073.
RootELF optional,lastfixturefinaloff verified,nFLT1,COM41closed/no livefixture.

E223 first real driven handoff attempts both qualify12intervals but REFUSE
actual core arm: driven_handoff46_01/_02,DRIVEXresult2,CORESEEDarmed0,
zero powered COMs/events. _01 ci1596 age360half-us leaves39ticks(19.5us),
_02 ci1599 age372 leaves28ticks(14us),both below unchanged64ticks minimum.
Second build skips redundant full gates_off in coast_run_inner ONLY for
explicit driven initial feedback after adopt; reset stage22->16us but later
foreground arrival erases savings. Next stage noncritical powered guard setup
ahead of final seed / reduce foreground return latency,not more same trials.
Added Transfer epoch invalidation and RELEASED clearing on shared cancel,
even old owner inactive. Host off unconditionally disarms NEXT_TRANSFER.
Idle transfercheck synthetic tokens never raise ENABLE;8/8 on3runs(firstbuild),
8/8 after secondflash. Existing poststop18/18 and recordcheck12/12 pass.
Full verifier now tested with explicit synthetic composite,not hardware proof.
Short DMA captures<128/256 correctly require flags0,>=128 flags5; all error/
complete flags forbidden. Both real acquisition segments verify; fullpowered
verifier correctly refuses.158Python/138Rust tests pass. All finaloff/nFLT1,
stackuntouched5320/span7260. Installed experimental handoff SHA
2DEF99CDB95F1DA33670363877B9BAD72267FD9077524A081812A2186C604791
text93560,data956,bss28644. RootELF matches. COM41closed/no fixturelive.
Both resets needed known UARTclock recovery after exact safe-register checks.

E222 adds explicit fixture drive_handoff/engagems/drivex acknowledgements,
cleanup and extended finite host timeout. New drv_driven_handoff.py verifies
reason22 acquisition separately from powered completion,DS/DX seed identity,
original ADC stamp,fresh CORESEED arm and singleton powered summaries/timeline.
Default observer verifier still refuses reason22; transfer mode requires live
seed and DMA evidence. Removed duplicate core summary in driven dump. Handoff
coast origin now explicitly unavailable (acquisition release is not finalstop),
no misleading CB85 even after16-bit wrap.156Python/138Rust tests pass; new
tests cover synthetic provenance/refusal/fixture handshake,NOT end-to-end
successful handoff or disabled hardware ownership. Optional release/s/thin-LTO
build passes,text92092,data956,bss28636,SHA5A6CAA68BF7B76000C6974D0D1E9E68D80F2212709C4208B891E7B2F45A2C835.
No flash or UART this turn; installed image remains E220. Root ELF is now
experimental handoff,NOT normal. Next qualify complete transfer verifier and
disabled ownership/late-token path,then first finite powered transfer. Inspect
fixture disarming across previous manually armed drivex sessions before bench.

E221 implements EXPERIMENTAL bench-driven-handoff (implies IRQ) + idle drivex1/0.
Not flashed or bench-qualified. prepare_driven stages core before forced owner;
IRQ qualification creates one opaque Transfer only after guard/age/step checks.
Reason22 stops forced timers/DMA/COMP and clears bridge while retaining ENABLE;
all other reasons disable. Foreground consumes token once,adopt_driven rechecks
freshness/awake/off,configures TIM6,then existing coast_run_inner gets original
feedback/acquisition stamp instead of flying_bench cache. Existing fresh arm
and minz control unchanged;late/abort/refusal disables. DX85 records transfer
timings. DRIVEX result1 means routine entered/returned,NOT successful lock.
Existing drv_driven_run verifier intentionally DOES NOT certify this path:
reason22 is shorter than20ms,CB85 origin references pre-powered stop,plus
powered summaries need their own verifier. Next implement fixture/verification
and disabled ownership/late-seed checks BEFORE first powered transfer.
Release/s/LTO optional/default builds pass;138Rust/147Python existing tests
pass but do not cover new hardware ownership path. Linker stack span7268
not high-water measurement. Installed image remains E220 IRQ observer.

E220 excludes epoch0(initial partial sector) ONLY as first acquisition anchor;
raw record retained,all subsequent gaps still latch refusal. DS85 header now
partial_epoch_excluded=1;host supports old policy explicitly,doesn't relabel.
Size-s hardware full60_01/_02 both qualify12intervals/7cycles,IRQmax33us,
seedaverages1605/1592,edges21728/23222half-us,late15/16us,peak555/522raw,
bus11510/11402mV. Neither raw trace starts epoch0: exclusion branch proven by
unit test,not those runs. No more observer repeats needed before transfer.
Installed OPTIONAL IRQ size-s SHA3677727D5D40938C7C25370B1432FDF098025B6F3719291BD2633C4E203EE3C6,
alloff via fixture;COM41closed. Reset UART wart again needed known clock fix
after exact safe-register checks. Fresh powered handoff remains next work.

E219 size-s IRQ observer completes two powered20ms trials,IRQmax23us,late9/7us,
peak455/481raw,bus11569/11534mV. BOTH seedrefuse fault2: acceptedepochs0,2..23,
initial partialsector0 then missing1. Not a handoff pass despite full later
sequence. Next explicitly exclude initial partial sector from acquisition or
bounded fresh reacquisition after a rejected sequence; never bridge the gap.
UART initialization issue recurred afterreset: RCC08000000. Added explicit
clockenable/readback before HALinit but resetrepeatfailed,NOT proven fix.
Known clockenable recovered UART when registersvalid. GDB TCP inaccessible;
safe halted DCRSR PCread showed mainloop,not startup hang;reset resumed.
Installed firmware currently OPTIONAL bench-driven-irq,size-s,LTO,not normal.
SHA16C971F4A4245A249D91B0828EDDD5226A5D45812ABB1CCFA5FC3020668F3A48.
No live process; UARToff,guard/record checks verified. Do not blindly keep
repeating these settings; handle partial-sector admission then fresh transfer.

E218 operator requests release+LTO+size optimization. Cargo release now explicit
opt-level="s",lto="thin",codegen-units=1; debug2 retained for symbolication.
Same optional driven-irq source builds at text+data90672bytes(s) versus84092(z).
z saves6580bytes but runtime cost unmeasured; s remains default pending timing.
No flash in E218. Installed normal firmware remains prior opt3 image. All
E217/earlier hardware timings apply to old optimization profile,NOT new s/z.
Next requalify timing on selected size profile before fresh powered handoff.
No claim that debug symbols consume those flash totals; use ELF section sizes.

E217 actual live qualification passes: driven_seed60_02 DS85ready1,step1,
edge21014half-us,average1595,12intervals,7cycles9408..9758;24IRQaccepts.
IRQmax33us (was14 before qualification),late24us,ADCage354us,peak623raw,
bus11498mV,stackuntouched5388.20.019ms guarded completion,NOT handoff.
Earlier _01 refuses first partial-sector setup(reason19),retained. No more
equivalent observer repeats needed. Next staged fresh-edge transfer: existing
prepare_early gatesoff/TIM6 mutation must precede driven ownership; existing
guarded_power_run reads flying_bench baseline and AWAKE token,neither supplied
by driven_run yet. Carry original driven ADC age and awake provenance explicitly.
Normal firmware restored,guard/record checks/alloff verified,COM41closed.

E216 connects driven_seed::Qualification into optional IRQ acceptance path,
reusing RuntimeAcquire,not a new controller.13acceptances ->12intervals,
7rolling cycles; first prepare-relative interval excluded. Actual bracket
start*2 conservatively timestamps seed; completed seed never refreshed.
CRC DS85 reports qualification; host independently checks against DI85.
137Rust/144Python tests pass;M0 check and optional/default builds pass.
New ISR cost UNMEASURED: noflash/motor in E216. Optional initially overflowed
flash1856bytes; old drivencheck/adc/comp/stop/phase commands now omitted ONLY
in bench-driven-irq (available in bench-driven-entry/power). Guards unchanged.
Next qualify live seed timing,then transfer fresh edge. prepare_early currently
disables bridge/reconfigures TIM6; cannot call it under driven ownership.
Need staged core storage initialization and explicit guarded timer/ENABLE/
feedback ownership adoption,not stale baseline or post-stop driver wake.

E215 bench-qualifies new IRQ observer: initial driven_irq60_01 safely aborts
late forced sector (reason15) after2accepts. Record-only COMP priority could
starve TIM3 via pending camp. Optional IRQ profile now guard0,TIM3=0x40,
COMP=0x80; production powered core priorities unchanged. No limits relaxed.
Next startup abort _02 retained. Same-build _03/_04 both complete20.040ms,
23/24accepted consecutive epochs,IRQmax14us,late8us,peak483/441raw,
busmin11283/11498mV,stackuntouched5316. Both first12real intervals and7rolling
cycles fit existing acquisition bounds OFFLINE; not a fresh seed or handoff.
Next integrate ordered live intervals plus a fresh accepted-edge handoff;
do not count first prepare-relative interval or grant power from coarse DMA.
Normal firmware restored,reset UART-clock wart repaired after safe register
checks;guardcheck18/18,recordcheck12/12,alloff/nFLT1,COM41closed.

E214 connects adapter behind NEW bench-driven-irq (implies driven-power).
driven_irq_live owns TIM2 2MHz/16bit interval and COMP input while forced TIM3
retains gates. Average1666/filter12 fixed diagnostic configuration,not estimator.
DI85 CRC rows carry physical epoch,step,ISR time bracket,actual interval,
requested ARR,previous-accept flag. First interval is prepare-relative NOT ZC.
Shared end masks/clears COMP and stops TIM2; commands/enables require live
driven ownership; rate64/ms and guard deadlines retained,reason21 on IRQ fault.
142Python tests pass; optional/default builds pass. Optional stack span7372
is linker space only,not measured runtime margin. NO flash/motor run in E214.
Next bench qualify shutdown/stack then same4.6%/+60/200eHz20ms trial; require
valid DI85 provenance/timing and actual IRQ delivery before claiming sensing.
ELF now normal build; rebuild optional explicitly before installing it.

E213 adds driven_irq_core::visit, a no_std record-only adapter calling the
actual minz comp_isr. Physical PWM/phase/COM timer implementations cannot enter
its bundle; acceptance returns measured interval and requested ARR only.
Six synthetic contract tests cover all sectors, immediate12read persistence,
each bad-read position, strict gate boundary/pending camp, invalid states and
no physical scheduling.133Rust tests pass for default/range300;M0 lib check passes.
NOT yet connected to firmware IRQs: no new motor evidence or fresh seed.
Next integrate serialized physical epochs/live interval clock/IRQ-rate guard
into guarded20ms driven run, retain timestamped accepts and stop revocation.
Do not feed100us DMA samples repeatedly to simulate immediate persistence.
No flash/UART/motor commands in E213; prior installed firmware not changed.

E212 adds CRC CB85 shutdown->coast-origin brackets. Actual gap10..11us rules
out clock-origin skew as explanation for large conditional phase discrepancy.
Idle-only drivephase{-30,0,30,60} one-shot shifts ONLY20ms observer,not startup;
fixture --phase-shift records/verifies applied value and resets on exit.
At4.6%/200eHz,+30produces7old-sampler candidates(consecutive) and2PWM candidates;
staged+60produces8old(max5consecutive),10PWM candidates,peak556raw,bus11366mV.
Both pass timing/electrical/finaloff; still no12-interval seed or handoff.
DMA records show early too-late confirmation and later too-early baseline;
next qualify fresh under-drive events with production timing,not more blind
phase sweeps or fabricated DMA-index seed timestamps. Normal firmware restored.

E211 adds offline drv_coast_phase.py interval-censored fit of first12early
coast rows (balanced sine/constant frequency assumptions,explicit uncertainty).
E210coast capture at100us uncertainty:823forward grid fits,178..219eHz,
coast-origin waveform phase167..220deg;0reverse fits within150..260eHz search.
Commanded phase at driven shutdown80.824deg,at a DIFFERENT time origin.
Exact stop->coast clock gap is not captured: do not apply the difference as
a phase correction. Fits are conditional model feasibility,not confidence/
lock/calibration. Next measure that clock relation before changing alignment.
No flash/motor command in E211; installed normal firmware unchanged from E210.

E210 separates observer duty from startup: idle-only drivedu40..62 (0=inherit),
consumed once when driven_run starts; fixture --drive-duty resets on exit.
Startup remains6.2%;20ms driven trials at5.4% and4.6% pass but still0qualified
inverted-polarity candidates. Peaks869/547raw respectively,not calibrated mean.
Fixed capture omission: COASTCOMP brackets now dump independently of stale
startup PHASEANCHOR (still suppressed for this mode). driven_du46_coast_01
passes20.021ms,peak667raw,215DMA/399oldCOMP/188ADC,0qualified candidates.
Independent early-coast full-period bounds on all3phases are consistent with
~200eHz (first bounds roughly170..243eHz,conditional100us timing uncertainty/
valid transitions/no missed edges); not controller lock or stop-speed proof.
Next unresolved issue is driven crossing/phase qualification,not absence of
rotation. Normal firmware restored,alloff and guard/record checks verified.

E209 corrects E208 inference: sustained powered path DOES invert raw COMP.
prepare_early->observe_begin sets PHYSICAL_OBSERVATION=true; recorded successful
range300_fastreturn50_02 COREPOL physical_raw_inverted=1 confirms. No polarity
change authorized by raw-polarity offline candidates. E208 body corrected.
Idle-only drivepwm192/320 selects already timing-qualified DMA target; prepare
and host require target+32counts within ON pulse. Fixture --pwm-target320
retains startup6.2%/200eHz/20ms. driven_pwm320_01 passes,216PWM/399oldCOMP/
188ADC,20.020ms,peak1143raw,busmin11486mV. Still0established-polarity candidates;
alternate raw replay12candidates is not lock/seed evidence. Shifting3->5us
did not solve qualification. Normal firmware restored,alloff verified.

E208 adds finite TIM1_CH4 DMA1 CH2 sampling in bench-driven-power. Idle-only
pwmdmatiming captures TIM1 CNT at192/320counts while ADC traffic runs;128/128
samples land target+4..6counts (0.063..0.094us). Gate/ENABLE remain off.
driven_run now also captures COMP2 CSR atCCR4=192 without a sampling ISR.
PC85 raw words/PD85 per-command DMA-count brackets retain ambiguous-boundary
samples but exclude them from phase-qualified replay. End/sharedstop revoke
CC4 requests and DMA before physical shutdown; errors/exhaustion stop the run.
driven_pwm_dma_01 passes20.032ms,216DMA samples(212 stable-epoch),399oldCOMP,
188ADC,peak942raw,busmin11438mV,stackuntouched5752. No live handoff authority.
72opposite-level samples after blanking but ZERO inverted-polarity candidates;
raw-polarity replay gives6scattered candidates,NOT a complete ordered sequence.
Sampling aperture access alone does not solve entry; do not flip polarity or
claim BEMF lock. Need reconcile driven phase/polarity with production path.
Normal firmware restored,guardcheck18/18/recordcheck12/12 and alloff verified.

E207 adds separate bench-driven-power (implies driven-entry,forbids DMA/TIM3
co-ownership). driveobs1 performs guarded20ms six-step after actual startup,
using stopped commanded phase/time extrapolated to launch,not synthetic phase.
First powered setting restricted200eHz/4..6.2%; no BEMF handoff authority.
DQ85/DA85/DC85 preserve comparator/PWM/read brackets,all five guard-feedback
channels,and physical command epochs/timestamps; dumps require cap1.
scripts/drv_driven_run.py captures proven6.2% startup50->200 and replays.
driven_power_01 retained but rejected: stop timestamp preceded final ADC
bookkeeping. end now timestamps AFTER bridge/ENABLE writes; no tolerance widened.
driven_power_02 passes20.030ms,24commutations,399reads,188ADC,ISRmax10/25us,
lateness8us,ADCage128us,peak657raw,busmin11271mV,stackuntouched7228.
BOTH have ZERO qualified candidates,not lock/seed evidence. First trace has
zero PWM-ON samples,second35/399:50us sample cadence + two consecutive opposite
samples may prevent arming when opposite level exists only in narrow ON pulses.
This is a sampling hypothesis,not wiring diagnosis or permission to relax guards.
Next: phase-aware BEMF qualification under actual drive,then validated handoff.
Normal firmware restored,alloff/guardcheck18/18/recordcheck12/12 verified.

E196 optional `bench-driven-entry` currently provides disabled-only
`drivencheck`,NOT a motor-entry mode. TIM3 command scheduler/TIM6 guard
passed3trials at20ms,synthetic feedback,no gate authority. Optional image
omits legacy sensezero/pwmcheck unless their probe features are selected and
uses24IRQ trace rows (normal32). TIM3 ownership must not overlap ADC DMA.
E197 adds gates-disabled `drivenadc`: ENABLE wakes CSA, real five-channel
feedback alongside both timers. Three trials pass199scans/20ms,max age102us,
ISR maxima5/9us. Software UG startup interrupt caused an extra command in
retained drivenadc_03; URS fixes it, drivenadc_04 passes unchanged criteria.
Normal firmware restored; driven COMP sensing/real gate integration remain pending.
E198 adds host/M0-only driven_observer around the existing minz-backed
detector: physical command epochs, read brackets, blank/gap checks and
one candidate per epoch. No firmware caller or handoff authority yet.
The100us sampling-gap contract requires timing qualification: E197's102us
ADC scan alone cannot prove a foreground-only COMP sampler meets it.
E199 connects that observer in disabled-only drivencomp: TIM6 guard/sample
50us, TIM3 synthetic sectors833us,real mux+ADC. Three trials have399reads,
maxgap63us,bracket1us,ADCage133us,ISRmax11/14us. Idle produces2..5candidates:
NOT rotor/BEMF qualification. No gate writes/handoff. Optional build also
omits phasecheck/sixcheck unless bench-adc-probes; normal build retains them.
Default firmware restored and finaloff verified. Actual driven runs pending.
E200-201 separate duty-range campaign: bench-range300 selects ONLY running
RunGuard<3333,277>; electrical/age/deadline limits unchanged. Ten-second
holds at4.6/4.8/5.0% reached242.89/255.10/267.20eHz. This is experimental
running evidence,not a300eHz qualification or30% target. RUNLIMIT labels
the profile in dumps; normal build remains4000/333. Recovery at5.0% safely
stopped on injected loss then refused CycleTooFast:7578half-us cycle below
acquisition8000minimum. Must coordinate acquisition/Seed::handoff/core_bench
limits and host verifiers before claiming higher-speed recovery. Do not
blindly change one bound or relabel the refusal. Four attempts retained in
captures/range300_campaign_01.csv; three holds pass,one recovery refuses.
E202 coordinates bench-range300 acquisition minima6666/554half-us ticks and
seed average1111ticks with powered handoff; default acquisition stays8000/666
and1333. Two5% recovery attempts acquire12intervals/seven cycles near263eHz
but BOTH refuse late handoff (SEEDLAT reference262ticks),zero resumed events.
range300_seed50_first_pair.csv retains both. Do not call result7 a success.
Moving ADC occupancy reset earlier saved no measured time and was reverted.
Next: stage guard/timer preparation before final fresh seed,retain32us arm
margin and measured timestamp. No new powered recovery qualification yet.
E203 supersedes that last recovery status: final sample-relative dwell removes
3us bookkeeping delay; bench-range300 caches bus conversion during acquisition
using original raw timestamps (no age refresh); successful awake acquisition
uses bridge_clear instead of repeating full scheduler shutdown (all schedulers
stopped at entry,no activation in acquisition; faults keep full gates_off).
Two30s injected-loss recoveries at5% pass near267.5..267.6eHz,stackuntouched1512.
Arm remainder68..69half-us ticks (34..34.5us) remains tight vs32us required.
Do not expand speed further without headroom. Intermediate dwell/cache-only
refusals retained. New code still uses passive acquisition,NOT under-drive.
E204 closes prerequisite stop ownership: shared gates_off now cancels the
optional driven_probe TIM3/TIM6 owner before physical clear. disabled-only
drivenstop tests actual shared cancellation at5ms plus two late callbacks;
3/3pass and3/3fresh comparator/ADC sessions pass afterward on same boot.
No gate authority added. Powered integration must use this revocation path,
not leave TIM3 able to resume command processing after shared safing.
E205 adds phase_schedule::next,host-tested commanded-phase boundary timing.
SINE_LUT moved unchanged to support/sine_table.rs; observation uses the shared
sector map. NEXT returns remaining partial-sector time,not a fresh833us hold.
No live caller yet: under-drive integration must extrapolate halt phase to
actual launch and account for setup latency. No flash/motor run in E205.
E206 disabled drivenphase now executes that plan in TIM3,updating ARR without
UG/counter reset so ISR delay does not accumulate. Three trials:initial75us,
nextdeadline20075us,24commands,maxdispatchlateness12us,COMPgap<=59us,
ADCage<=141us. Cancellation regression3/3passes. Synthetic phase only,no gate
authority or actual startup handover yet. Normal firmware restored/finaloff.

## Active objective revision — supersedes historical goal limits

Read the current goal attachment:
`C:/Users/kaido/.codex/attachments/90c16e2b-6f02-47e6-b689-ddc0f9a0895d/pasted-text-1.txt`.
Throttle means PWM duty,hard exploratory ceiling30%,not a demanded endpoint.
There is no250eHz objective ceiling. Expand running speed guards only after
staged current/bus/sensing/timing/tracking qualification; existing firmware
ceilings remain until deliberately requalified. Retain800mA PSU setting,
finite durations and verifiedoff. Improve driven BEMF acquisition/handoff.
Use archived minz measurements/frozen source ONLY: no minz reruns or reconnect
dependency. Missing archival evidence is a documented comparison limitation,
not an indefinite blocker. Older goal attachments/audits below are historical.

## What this repository is

`binz` is a bench-oriented, `no_std` Rust firmware crate for a
NUCLEO-G071RB (STM32G071RBT6, Cortex-M0+, 128 KiB flash, 36 KiB RAM). It is
used to bring up and characterize BLDC power stages, not as a polished single-
board application. The many examples form a chronological lab notebook: small
peripheral probes lead into open-loop drive, ADC/BEMF experiments, closed-loop
work, and board-specific interactive tools.

The crate builds for `thumbv6m-none-eabi`. `.cargo/config.toml` selects the
target and a fixed ST-Link probe. `cargo run` therefore flashes real hardware;
do not run it merely to test compilation.

The control strategy is six-step sensorless BLDC. `minz-core`, imported from
`../minz/core`, owns the established commutation/filter/blanking/advance/desync
logic. Board work belongs behind its HAL traits; do not rewrite that control
science locally without an explicit reason.

## Read first

- `CLAUDE.md` is the main bench history and safety/scar record. Its most
  important correction is that the operator never requested a 30% target.
  Do not revive, infer, or optimize toward that number.
- `REUSE_PLAN.md` explains the `minz-core` integration and the ADC-BEMF HAL
  seam. It describes the older EVLDRIVE102H campaign.
- `controlboards/BOOSTXL-DRV8304H/G071_DRV8304_WIRE_MAP.md` is the current
  DRV8304 free-wire map.
- `controlboards/X-NUCLEO-IHM08M1/REWORK_STEPS.md` is the clearest actionable
  IHM08M1 rework checklist. Cross-check it against
  `PIN_SURVEY_AND_HOTWIRE.md` before touching hardware.

The Markdown contains historical proposals as well as measured results. Prefer
later dated, bench-confirmed statements over earlier plans, but surface any
conflict instead of silently choosing one for physical work.

## Hardware generations in this tree

### EVLDRIVE102H (older characterized rig)

- High gates: PA8/PA9/PA10.
- Low gates: PA7/PD3/PD4.
- EN/STBY: PC9/PC8; EN shares the nFAULT node and must be open-drain.
- Phase ADC: PB1/PB0/PB2; bus PA1; current PB11; NTC PC4.
- Original VCOM: USART2 PA2/PA3.
- The shared `stage`, `harvest`, and `mzhal` modules, plus `closed-loop.rs`,
  are hard-coded for this platform.

### X-NUCLEO-IHM08M1 (reworked development rack)

- Native complementary gates: PA8/PA9/PA10 and PA7/PB0/PB1.
- Native board BEMF taps are ADC-only: PC3/PB11/PB13.
- Stock AM32/rm32 comparator operation requires hotwiring those taps to
  COMP2 INM PB3/PB7/PA2 and a resistor-star neutral to PA3.
- PA2/PA3 conflict with the on-board VCP, so USART3 PC10/PC11 is used after
  removing SB16/SB18.
- The latest checklist says R77, SB16, and SB18 have been removed; the star
  and BEMF jumper wiring remained unchecked when documented.

There is stale advice in the older pin-mapping document. Current rework intent
is a 3 x 47 kOhm neutral star and to leave R59/R60/R62/R81 installed, with JP3
open. One later survey checklist still says 10 kOhm; confirm with the operator
before changing physical hardware.

### BOOSTXL-DRV8304H (newest active rig)

- Boards are side-by-side and free-wired, never stacked.
- Gate map: A PA10/PB1, B PA9/PB0, C PA8/PA7.
- COMP2 BEMF: VSENA/B/C to PB3/PB7/PA2; star neutral to PA3.
- VBUS: PA6, multiplier about 11.94.
- Phase-current amplifiers: measured logical A/B/C = PA4/PA1/PA0 (ADC4/1/0),
  about 70 mV/A around 1.65 V. Entry 016 found A/C reversed relative to the
  original wiring table and capture labels; shell-pwm now uses CURRENT_ADC.
  Historical captures retain old labels; do not silently reinterpret them.
- ENABLE: PD1; nFAULT: PB14; status LED: PB5.
- Throttle: PB4; optional telemetry: PB6.
- Bench console: USART3 PC10/PC11 at 115200.
- MODE is strapped hard to ground for 6x PWM. Do not trust the stale
  `drv-spin6x.rs` banner/comment that calls it 3x mode.
- The board generates its own 3.3 V. Common grounds, but do not jumper the
  Nucleo and DRV 3.3 V rails together.
- Raw TIM1 complementary PWM is bench-proven on this wire map at 10 kHz:
  CH3/CH3N=A (PA10/PB1), CH2/CH2N=B (PA9/PB0), CH1/CH1N=C (PA8/PA7).
  The accepted open-loop profile is a 100 eHz / 7% catch, ramp to a 50 eHz /
  6% hold, then disabled coast capture; see LAB_REPORT Entry 011. One-kHz
  bit-banged 6% PWM produced long full-bus slices, PSU collapse, and nFAULT.
- `shell-pwm` is the dedicated hardware-PWM clone, validated in LAB_REPORT
  Entry 012. Keep `shell-sine.rs` unchanged as the successful reference.
  See `controlboards/BOOSTXL-DRV8304H/SHELL_PWM.md` for terminal commands,
  duty units, timing limitations, and capture/replay.
  Older ADC characterization shell commands now require Cargo feature
  `bench-adc-probes`; default builds retain motor ADC feedback and guards.
  Current dumps use one-shot Ascii85/CRC (`cap1`); terminal runs are quiet.
  Older hex/CTIME dumps replay.
  The 2026-09-12 BEMF campaign explicitly authorizes exploration toward 25–30%
  only after feedback/current gates pass; the old invented-target warning does
  not negate this later user request. Current drive ceiling remains 10%.

On 2026-09-13 the operator clarified that test duration is an engineering
choice, not a per-run permission gate: use longer holds and multi-minute
frequency/duty sweeps when useful. Neither 5 nor 10 seconds is an operator
ceiling. Retain continuous current, bus, driver-fault and tracking protection,
host abort, and an explicit finite timeout appropriate to each experiment.
The open-loop startup remains bounded to 5 seconds. Powered BEMF duration is
now configurable with `engagems20..600000` (default1000 ms), with an independent
finite campaign backstop and unchanged electrical/tracking guards. Entry129
records two complete10-second powered runs near236-238eHz at4.5% handoff duty,
plus an early handoff failure on the third attempt. Preserve that distinction;
this is not completed range/recovery/current parity qualification. Entry133
adds a fixed eight-attempt cohort at4.0% powered duty: five complete10-second
runs near206eHz and three passive-acquisition refusals; all five powered starts
completed. `captures/sustain40_stepped_sparse_cohort.csv` retains all outcomes.
Entry134 captures a real acquisition timing mismatch: an in-range980us
candidate was rejected at1010us wall age before its confirmation visit.
`poll_filtered` now permits only the expected pending candidate's bounded
qualification latency; physical onset limits and powered watchdog unchanged.
`FLYWAIT` counts deferred polls; fixture-only CRC F85 snapshots diagnose
refusals. Initial fix builds completed10s near206eHz, but repeat-cohort
reliability and hardware grace-then-confirm remain to be demonstrated.
Entries135-136 subsequently demonstrate grace-then-confirm on hardware
(two deferred polls, full12interval acquisition,10s powered completion),
four/four10s repeat attempts, and a separate full60s hold near206.6eHz.
Minute capture has74368COMs and319175powered ADC scans, final off verified.
These are retention/entry results, not completed recovery/current/qzc parity.
Entry138 adds explicit fixture --dropout (shell dropout1/0), suppressing COMP
delivery at2s powered with unchanged tracking guard and no restart. Hardware
stopped on Tracking within1082us of last acceptance (conservative disabled
observation), then a separate unarmed3s run passed. Recovery itself remains
pending. Old comirq diagnostic now joins comtiming under bench-adc-probes.
Entry139: dropout probe now performs gate/ENABLE-disabled re-acquisition after
Tracking8, preserving original capture. Two fresh12interval seeds qualified
in~10.4ms; neither was granted power. RECOVERYACQ distinguishes this from
recovered operation. Coast origin is delayed; old extrapolated-stop labels
must not be treated as true stop speed. Future wake-before-final-acquisition
is necessary because1.1ms driver wake exceeds a seed's handoff margin.
Entry141 warning: initial first-segment archive build produced corrupted FLY
state and a CRC-valid invalid accepted sector before injection. ELF leaves
2580bytes stack; a late archive temporary enlarged powered coast_run_inner
frame to796bytes. Non-inlined post-stop helper reduces it to140bytes; corrected
build flashed, off/guardcheck verified, NOT motor-qualified. Measure/protect
stack headroom before further powered testing. FIRSTSEG/P185/T185 archive
code and host tests exist, but hardware preservation/re-entry are unverified.
Old compirq/compirqref diagnostics now require bench-adc-probes.
Entry142 supersedes the powered-test pause: UART stress buffer moved behind
bench-transport-probes, RTT buffer reduced to256; default stack span4364bytes.
Boot-only paint/idle `stack` diagnostic measured1888bytes untouched through a
powered dropout, archive copy, disabled re-acquisition and dump. First archive
P185/T185 exactly matches primary event windows on hardware. One requalification
passed; keep measuring margin for new recovery paths, not assume all nesting
is covered. SessionBudget still has no live restart caller.
Entry143 now connects SessionBudget via explicit --dropout --reentry (shell
reentry1/0): ONE injected Tracking8 recovery completed remaining~7.99s near206eHz
inside original10s deadline. FIRSTSEG preserved, wake precedes fresh12interval
acquisition, new guarded authority, no recursive retries. Stack untouched1832.
REENTRY result7 means path entered, not success; require second-window guard/
events/deadline proof. Repeated recovery/current/reference parity still open.
Entry144 adds fixed3/3 repeat recoveries near206eHz, same firmware/4% duty;
all retained original deadline, valid separate archives, stack margin1832,
and final off. Recovery reporter now requires stack span>=4096 and untouched
>=512. Recovery at other operating points, mean current and matched-reference
quality remain unqualified; do not extrapolate to arbitrary disturbances.

Entries145-149 extend tested recovery near238eHz. Fresh acquisition now checks
seven rolling full electrical cycles8000..12000half-us ticks across12ordered
intervals, individual666..2000ticks. Fixture FLYCYCLE/RECOVERYCYCLE exposes
checks; fault14/15 distinguishes cycle speed. Two acquired seeds initially
expired during setup (E147-148), not an electrical failure. Final expected
candidate now gets one extra same-mux20us confirmation visit, retaining dwell,
10us mux settle, sampling guards and32us arm margin. E149 two successful
injected Tracking recoveries at4.5% powered duty,237.75/237.99eHz,original10s
deadline retained; stack span4272,untouched1836. SEEDLAT records setup ages;
actual remaining arm window38.5us,so headroom remains tight. Current flashed
source includes this change. Earlier206eHz cohorts are older-build evidence;
broader repeats,mean-current and matched-reference parity remain open.
E150 caught a real accepted record35us after frozen observation end; its238eHz
run is NOT a clean recovery pass. E151 serializes stop/record transactions and
counts refused late callbacks (fixture RECORDGUARD); one238eHz recovery passed,
one startup current abort retained. E152 adds idle-only `recordcheck`: invokes
the real recorder with stopped-state flags, without output authority;12refusals
and unchanged record counts/end required. Old standalone corebench/coreirq now
require bench-adc-probes to fit flash. This does not disable production core IRQs.

## Source layout

E180 experimental `bench-dma-feedback` uses TIM3/101us ADC scans, DMA IRQ
coherent copies/immediate raw-current checks and an eight-scan owned FIFO.
Foreground converts every queued scan with its original acquisition timestamp;
overflow refuses/stops, never overwrites. Three10s runs near205eHz completed,
DMA IRQ max10us,queue peak4/8,stack span4096/untouched1872. Longer runs,
dropout/reentry and calibrated current remain unqualified. Default firmware
still uses foreground ADC. Experimental IRQ trace capacity27 (normal32) and
legacy sensezero requires bench-current-probes only on experimental build.
Do not treat accepted-event rate or these completions as independent qZC parity.
E181 subsequently passes3/3 thirty-second injected-dropout/reentry campaigns
with FIFO near205–206eHz. Each preserves original deadline,valid archives,
DMA max10us,queuepeak4/8,untouchedstack1580. Higher-point FIFO qualification,
overflow injection,current and independent-reference quality remain open.
E182 additionally passes3/3 FIFO thirty-second dropout/reentry campaigns at
237.5–237.6eHz/4.5% powered duty,unchanged safeguards. DMAmax10us,queuepeak4/8,
untouchedstack1580. Cycle sigma~27us versus~21us in earlier foreground-ADC
cohort: retention passes do not imply zero timing cost or full parity.
E183 adds fixture-only S85 raw signed-current/bus/VREF sums to experimental
DMA firmware,final segment only,not calibrated mA. Host strictly validates
counts/timestamps/CRC. First capture dma_sums40_01 has a real one-scan delivery
race and remains rejected; corrected dma_sums40_02 passes10s,n99008. Aggregate
only when feedback_inner reports delivery; no broader interrupt mask required.
Experimental trace capacity now24,stack span4124/untouched1892 on that pass.
Normal trace capacity32 and default ADC path remain unchanged.
E184 corrected sums also pass30s dropout/reentry near236.68eHz,n277111 in
the resumed segment. Explicit `bench-dma-stall` is a FAULT-INJECTION feature:
it withholds FIFO consumption after20000 delivered scans once per boot.
Hardware reached8/8,stopped with ADC fault9/AdcTimeout11,then a separate
same-boot fixture start completed10s near205.13eHz,n99008,without reinjection.
`scripts/drv_fifo_fault_check.py` validates the retained fault/reuse pair;
same-boot provenance requires the bench log,not UART inference. Normal and
`bench-dma-feedback` builds do not inject this fault. Default firmware restored,
outputs verified off. Current calibration,coverage and independent supply
anchor remain open; raw sums are not calibrated current or reference parity.

E188–190 latest operating evidence: operator saw~70mA PSU during60s at237.64eHz
(idle11.7V/0.7mA). Two30s holds at4.6% powered duty reached243.05/243.25eHz.
One30s injected-loss recovery at243.31eHz subsequently passed,original deadline
retained,stackuntouched1520. Entry failures remain:6.5% catch/6.2% startup
target still tripped phase-current guard in E190. Both catch and target6.2%
then yielded one passive-acquisition refusal and one recovery success. Keep
these attempts,do not describe startup or243eHz recovery as fully qualified.
No guard threshold changed; normal foreground-ADC firmware remains installed.
E191 adds two more243eHz recovery passes at6.2% catch/target,4.6% powered.
Full same-settings cohort is3/4entry-to-completion (one acquisition refusal),
3/3powered recoveries; retain that denominator. All original deadlines/finaloff
validate,stackuntouched1520. Do not run endless equivalent repeats as a
substitute for remaining entry,independent-quality/current and reference gaps.

Long-term objective: match `../minz` performance on Cortex-M0+ and then port
proven wins into `../rm32`. See `BEMF_PARITY_PLAN.md` and
`GRAYBEARD_FAST_RAMP.md`; the latter's Hall/older-board assumptions are not
current DRV wiring facts. Default firmware output must be quiet; full dumps
require explicit fixture opt-in. Preserve raw replay and measure observer cost.

- `src/stage.rs`: single emergency safing primitive for the EVLDRIVE pin map.
- `src/harvest.rs`: phase-decohered TIM6 -> ADC -> DMA measurement spine,
  aggregation, current/bus/fault/temperature guards, and TIM17 timebase.
- `src/blackbox.rs`: 64-event ring recorder.
- `src/telem.rs`: 22-byte binary telemetry framing and nonblocking TX ring.
- `src/mzhal.rs`: EVLDRIVE-specific `minz-core` trait adapters, TIM1 six-step
  drive, synchronized ADC-BEMF software comparator, and timers.
- `examples/`: board probes and bench campaigns. Read each example's module
  documentation before running it; assumptions and protection vary.
- `scripts/`: host capture, plotting, and study generation for the older
  EVLDRIVE campaign.
- `captures/`: retained BEMF and closed-loop text/plot artifacts.
- `controlboards/`: datasheets, schematics, design files, images, and wiring
  notes. When reading schematics, inspect rendered pages; extracted text has
  produced incorrect resistor values before.
- `ref/stm32g0xx-hal/`, `data/`, and `target/` are intentionally ignored.

## Safety rules

Motor-running commands affect physical hardware. Treat them as hazardous.

- Never flash or run a motor example unless the operator explicitly asks.
- Do not invent a speed, duty, current, or envelope target. The operator sets
  targets; write a numeric pass/fail gate before a bench run.
- Safeguards precede waveforms. Every exit, panic, guard, timeout, and host
  abort must leave all gates low and disable the applicable driver.
- Keep the bench PSU current-limited for initial tests. A larger supply can
  feed a loss-of-sync heater; it is not a cure for bus collapse.
- Never infer average current from a PWM-synchronized single-shunt peak.
  Conversely, the phase-decohered harvest is suitable for average current but
  not for BEMF zero-cross detection.
- BEMF samples must be PWM-synchronized and interpreted as a ramp over a
  commutation step. One threshold crossing or one instantaneous sample is not
  proof of a usable signal.
- Use TIM17 for timing. Cortex-M0+ has no DWT cycle counter.
- Avoid `WFI` in bench/debug firmware because it disrupts RTT on this setup.
- Keep probe use serialized: never run two `probe-rs` processes and never kill
  one during a flash. Prefer `probe-rs download` followed by reset over a
  long-lived `probe-rs run` in scripts.

`src/stage.rs` is not portable safing. On the current DRV8304 map it does not
clear PB0/PB1 or disable PD1. The crate panic handler calls it, so a new-board
example that merely uses `binz` for the panic handler is not automatically safe.
Provide or refactor a correct board-specific safing path before relying on it.

## Implementation conventions

- Prefer raw PAC register access for timing-critical TIM1 work. The HAL TIM1
  PWM layer was observed enabling complementary outputs incorrectly; the
  known-good raw setup is documented in `spin-pwm.rs` and `mzhal.rs`.
- PAC `afrh().afr(n)` indexes pins 8-15 as `n = 0-7`; do not pass the physical
  pin number.
- Preserve constant and bounded ISR cost. Put analysis and formatting in main
  or on the host.
- Give every silent guard, veto, rejection, or fallback a counter or blackbox
  event when it is introduced.
- Keep hardware mappings explicit. Do not generalize an EVLDRIVE function for
  IHM08M1 or DRV8304 without separating the board-specific enable, fault,
  current, phase-sense, and low-gate pins.
- Preserve `no_std`, M0-compatible arithmetic, and `portable-atomic` support.
- Do not edit the sibling `minz/core` crate casually. Existing portability
  changes there belong to a shared dependency and must retain its host tests.
- This directory is currently untracked in a dirty parent worktree. Preserve
  unrelated changes and never use destructive Git cleanup/reset commands.

## Build and verification

Compilation only (does not flash):

```text
cargo check --release --example <name>
cargo build --release --examples
```

Hardware execution (only on explicit request):

```text
cargo run --release --example <name>
bash scripts/run_example.sh <name> <seconds>
```

At the time this guide was written, the current DRV examples (`shell`,
`shell-sine`, `drv-sense`, `drv-spin6x`, and `drv-vm`) pass `cargo check` with
Rust-2024 `unsafe_op_in_unsafe_fn` warnings. Checking all examples fails in
`examples/gate-test.rs` because the `match` in `low_bit` lacks a trailing
semicolon. Do not mistake that unrelated compile error for a toolchain or
hardware failure.

When reporting a bench result, record the exact board, wiring revision, supply
voltage/current limit, firmware example, constants, observed guard state, and
capture artifact. Separate measured facts from hypotheses and historical
notes.
