# Direction: minz parity on G071, then rm32

## 2026-09-15 corrected campaign boundary: binz to 50% first

The operator assigned rm32 to another agent. This campaign now stays entirely
inside binz until the bring-up controller has explored and then qualified the
path to 50% commanded duty, with explicitly uncertain projections toward75%.
The former30% hard cap was a campaign limit, not the platform target. Extend it
deliberately, retain E777's current foldback and
hard stop plus bus/nFAULT/tracking/watchdogs, and use5 percentage-point
exploration steps unless a real failure calls for a finer bracket. Qualify
10/25/50% with repeated holds and normal-start recovery. Do not edit
or flash rm32 from this campaign. Portable findings remain documentation only.

## 2026-09-16 envelope map through the first hardware-fault boundary

The first meaningful post-30% rung is now characterized rather than guessed.
E791 holds30% for20s at20kHz with no foldback. Four35% attempts across20/24kHz
do not establish safe operation: two terminate on the signed-average current
policy and two assert hardware nFAULT, with healthy tracking and bus floors
11.64-11.69V. The diagnostic nFAULT run's final11.3ms signed-current residual
is2.16x its nominal2.5A raw allowance. The H-device cannot identify the exact
fault source; VDS OCP leads because the EVM leaves VDS Hi-Z (0.6V threshold),
but CPUV/GDF/thermal are not falsely excluded by the aggregate nFAULT pin.

The32.5% midpoint reached about1.38keHz, folded once to27.5%, and completed the
full20s window near1.25keHz with reason2 and all electrical/tracking guards
healthy. Thus30% requested is the clean point,32.5% is a recoverable current-
limited request, and35% is the present unsafe boundary. Do not proceed to40%
until current/commutation demand is reduced or the hardware fault configuration
is deliberately characterized. PSU hard fold remains a separate campaign stop.

## E777 current-limited 30% exploration

The first signed-average over-limit block now publishes a latched warning to
foreground, which lowers the live PWM ceiling by 5%; a second consecutive block
still hard-stops. A clean live ramp held 30% for 30 s with no foldback. In one
30% tracking-restart run, ordinary restart completed, 30% was restored, one
current warning folded it to 25%, and the original deadline completed without
the prior reason-25 stop. See `CURRENT_FOLDBACK_E777.md`. This is functional
evidence, not a 30% recovery cohort or precise current calibration.

E778 then traced this ownership into main rm32. Its existing foreground current
PID already publishes an ISR-consumed duty ceiling, and that ceiling has final
authority over command and stall boost. The new regression passes with the full
364-core/7-harness suite; the rebuilt G071 release again has zero forbidden math
reachable from all four motor IRQ roots. No bench current constant or one-shot
governor was copied. Powered rm32 qualification remains separate future work.

## E776 rm32 integration check

No second throttle-restoration state machine is needed in main rm32. Its native
10 kHz duty ramp already provides bounded restoration after `AllOff` and normal
startup, while the live receiver request remains authoritative and can supersede
the pre-fault value. A sequence-level regression test now covers that complete
handoff; all363 core and7 harness host tests pass, and the release G071 M0 audit
still passes. This is source/test evidence, not a powered rm32 qualification.

## E775 practical envelope and recovery status

Current frozen E775 (`2BD1C70...`) uses the lean COMP/COM path, explicit1.5A
nominal average-current policy and ordinary restart. Live throttle reaches30%;
a preceding E772 campaign held30% for30s at ~1.18keHz. On exact E775,
tracking-decision injection followed by normal startup and2s-paced5% duty
restoration passes3/3 at25% (~1.15–1.18keHz). At30% it passes2/3
(~1.33–1.37keHz); the retained failure is average current after restoration,
so30% recovery is not qualified and the threshold was not moved.

This closes the old “recovery above7% untested” gap for25%, and establishes a
real30% exploratory ceiling. Remaining practical control work is current
governor/foldback behavior or an explicit policy for requests above the
reliable current-limited recovery point. Archive-only minz comparison remains;
no disconnected reference-board reruns are required.

## E649 recovery architecture status

Current3E19 next-edge recovery completed3/3 fixed7% thirty-second campaigns
at337.5-338.4eHz after injected tracking loss. Original deadlines, electrical
stops and32us arm margin remain; actual margin34-43.5us and7us arm work.
The sampling/setup transition required continuous real comparator visits,
retained-context borrowing, and moving provisional-seed initialization before
the final checkpoints. See HIGH_SPEED_RECOVERY E647-649 and PORTABLE_WINS.

This does not transfer the older911A20%/22% results to3E19. Higher-duty recovery,
current calibration/limiting mechanism and full-envelope parity remain open.
Next expand recovery above7% with the current build, not repeat the same point
or require disconnected minz hardware. Archived reference conditions remain
the only minz comparison source.

## E628 passive speed corroboration (diagnostic build only)

6A2E fast-coast diagnostic measured128..129us sampling gaps with all three
comparator phases. After a22% Current5 stop, three-cycle early-coast average
bounds span917..1112eHz across phases, conditional on30us timing allowance,
valid transitions and no missed edges. No expected controller rate is fed into
edge selection. This corroborates real near1kHz rotor motion; not whole-run
lock/qZC or precise shutdown speed.6A2E22% campaign FAILED, not pooled with
911A passes. Exact captures, bounds and assumptions in auditE628.911A restored
and verified off. High-duty recovery/current-timing investigation remains.

## E626 current bench envelope update (not a new parity definition)

Current911A release-s/thinLTO has3/3 thirty-second live-ramp campaigns capped
at20% duty, final accepted-cycle tails923.755-926.586eHz. About150kCOM and
accepted events each campaign; original electrical/tracking stops retained,
fast-cycle ceiling explicitly report-only. IRQ safety publication removed the
foreground-delivery dependency seen at25% on544B. A23% attempt on911A stopped
on paired phase-current excursion near a453us accepted interval; no30% pass.
One subsequent22%-capped30s campaign also passed, finaltail1002.236eHz;
not a cohort. Existing2kHz coast sampling is alias-limited at this speed,
so it does not independently certify rotor speed or exclude harmonic lock.

These are mixed-duty campaigns, not30seconds at target, a full-speed lock map,
or a matched reference comparison. Do not compare their small tail-cycle sigma
to minz per-window sigma or infer qZC from COM:accept correspondence alone.
Higher-duty recovery on911A is untested. The earlier32B1/8% recovery3/3 below
remains a separate historical build qualification, not transferable to911A.
Archive-only reference constraint remains; no minz bench reruns needed.
Full goal still needs recovery/envelope evidence and a scoped final assessment.

## E615 refreshed archive-grounded comparison

Reverified frozen128-file source archive2799b284...9606c44 and historical
archive94df4240...355138; all three historical metrics reproduce. Five archive
testsPASS. Replayed all E612/E61332B1 recovery captures with explicit cycle450,
seed400, seed-timing and original30000ms/dropout/reentry checks: allPASS;
capture hashes match auditE612/E613. No reference hardware/live source used.

| Evidence | Archived minz FALCON | Current binz32B1 | Scope |
|---|---|---|---|
| Duration/rate | a9:4.016s/230.020eHz; a10:4.026s/257.476eHz |3/3 thirty-second campaigns, about27.992s resumed at387.193-387.512eHz | Different operating points/controllers; not matched parity |
| Progress | a9:5543 windows/1missing; a10:6220/0missing but1sequence gap |65029-65083 accepted, each COM count one higher | Binz correspondence does not create reference per-window qZC denominator |
| Spread | Per-window sigma66.899/63.399us | Full-cycle sigma20.518-21.415us | Not the same measurement; no superiority claim |
| Current | Nominal DC-link44.850/51.892mA | Raw phase peaks379-397; operator130mA on separate3A70/8.5%410eHz run | No calibrated phase-current/efficiency comparison |
| Recovery | No matched counterpart established in retained windows |3/3 injected-loss fresh-seed recovery;33us remaining against32us floor | Demonstrated cohort, narrow observed margin, not WCET |

Negative reference remains included: lockmap20_a9 at202.944eHz has20.548%
qZC,3917missing/4930windows,longest observed missing run5. Do not pool with
good captures. None of the FALCON captures has verified flashed-image identity
or embedded CRC. Exact hashes, prior archive commands and limitations remain
in E539 below; current captures and firmware are frozen in E610/E612/E613.

Remaining goal gaps: reliable wider startup/recovery envelope, more recovery
timing margin, phase-current calibration and genuinely aligned reference
quantities. Missing matched reference data is a stated comparison limit, not
a demand to reconnect the unavailable board. Full goal remains unfinished.
No new bench run inE615; installed32B1 lastoffE613, root6A10 staged differs.

E613: current32B1 now has3/3 eight-percent30s recovery campaigns,
387.193..387.512eHz; matched accepted/COM progress, allfinaloff checksPASS.
All three had33us remaining versus32us minimum, so wider margin is unproven.
No further identical cohort repeats; see auditE613 for exact hashes/metrics.

## E612 current update

Current32B1 const-reentry build: one7.5% recovery PASS361.814eHz and first
8% recovery PASS387.512eHz, about28s resumed after injected loss. Fresh edge
age76us;8% remaining33us versus32us minimum, so repeatability unproved.
All original seed/arm/electrical/deadline checks andfinaloff passed. Prior
3A70 steady8.5%410eHz/130mA result remains separate; not transferred to32B1.
Speed-policy decision (fast-cycle report-only) remains pending/unimplemented.

## E603 current status (supersedes status below)

E606 update: current3A70 now has one7.5%30s injected-loss/recovery PASS,
361.159eHz,60656COM/60656accepted. Fresh-arm margin35.5us against32usfloor;
not3/3 or higher-duty recovery qualification. No speed-policy change.

Current3A70, explicit cycle450 running envelope / seed400 acquisition:
8%30s PASS387.399eHz;8.5%30s PASS409.811eHz. Operator observed130mA during
8.5% run. Both approximately1:1 accepted/COM and finaloffverified. No current-
build recovery cohort or calibrated phase-current claim.
9% attempt refused2214us cycle against2223us floor after0.202s, not an
electrical or missing-event stop. Firmware envelope is not a hardware ceiling.
Old2329's7.5%3/3 recovery remains historical build-specific evidence.
See RECOVERY_POST_EDGE_AUDIT E601-603 for hashes, safeguards and failed runs.
Next resolve experimental speed-limit policy and recovery latency separately;
do not infer minz parity, actuallosslock, or permission to force30%.

E598 update: current2329 requested8%/60s hold stopped after1.512s on
CycleTiming (2493us versus2500us floor), mean383.901eHz, outputs-off verified.
Current8% sustained is NOT qualified. Reference cycle2489.5us also refuses;
this is not independent proof of physical overspeed or lost lock. See
RECOVERY_POST_EDGE_AUDIT.md E598. E5977.5% recovery cohort remains valid.

## E597 current operating evidence (supersedes older status below)

Current23298ec18646acfa1638186df218249ea858128aba516751d56804d8328f3c0c
has3/3 original30s startup/injected-loss/recovery campaigns at7.5%/20kHz,
359.551–360.711eHz. Fresh-arm margins35.5–37.5us retain32usfloor; alloriginal
electrical/tracking/deadline checks andfinaloff pass. See
RECOVERY_POST_EDGE_AUDIT.md E596–597 for denominator/hashes andlower10.160V
bus observation in run2. No calibrated-current or matchedminz-parity claim.

8% sustained operation has historical30s evidence on13B5 (E577), but8%
recovery refused fresh-arm margin on937F/3013 (E583/E585). Do not merge those
builds into currentqualification. Current range expansion remains unfinished.
Coldstate-only pre-final-edge preparation was tried and retired without gain
(E595); exact phase current bounds andcommon-clock age supplied modest gains.
Archivedminz remains the comparison source; no disconnectedboard rerun needed.
Remaining: wider reliable envelope, startup/recovery timing headroom, calibrated
current evidence and appropriately matched archived comparisons. Fullgoal open.

## E541 current-build qualification update

E540–541 now supplies3/3 original30s startup/injected-loss/fresh-recovery
campaigns on currentinline7E55 at6.9%/20kHz,333.437–333.579eHz in the resumed
segments. Each preserves original deadline and finaloff; sigma22.304–23.052us,
IRQunion50.660–51.119%,busmin10.829–10.937V,rawcurrentpeak343–350.
See COMP_READ_CADENCE_EXPERIMENT.md E541 for exact hashes/table/denominator.
This supersedes E539's current-build recovery gap, not its archive limitations.
No independent operating-current reading at this point or higher-speed
qualification added; full goal remains incomplete. Stop equivalent repeats.

## E539 — refreshed archive assessment, 2026-09-14

Supersedes E285's operating-summary status, not its historical evidence.
Reverified the frozen128-file source archive and all three historical FALCON
capture summaries, without reading live minz source or using its board:

```
python scripts/freeze_minz_reference.py captures/reference/minz_20260912.zip --verify
python scripts/minz_historical_baseline.py --verify-archive captures/reference/minz_falcon_historical_20260913.zip
python -m unittest discover -s scripts -p test_drv_minz_baseline.py
```

Archive hashes remain2799b284...9606c44 (source) and94df4240...355138
(historical), with full hashes in E263 below. Five tests pass, including
corrupt payload/metrics refusal and sequence-gap accounting. This checks
archive integrity, not current dependency equality or capture firmware identity.

Replayed all three carrier20_485_start61_reentry69_30s_01/02/03.txt files
through drv_driven_handoff.verify(expected_ms=30000,dropout=True,reentry=True).
Hashes match CARRIER20_EXPERIMENT.md E485. All three retain verified recovery,
original timeline and outputs-off. This is the older9ACC cohort, NOT a
recovery qualification of currently selected E537 inline7E55.

| Evidence | Frozen minz FALCON | Binz | Defensible comparison |
|---|---|---|---|
| Retained duration and rate | a9:4.01632s at230.020eHz; a10:4.02626s at257.476eHz | E485:3/3 original30s recovery campaigns, resumed27.991s at333.652–334.187eHz | Different operating points and controllers; capability evidence, not matched parity |
| Crossing denominator | a9:5543windows,1missing; a10:6220windows,0missing but1sequence/sector gap | E485:56036–56125accepted, COM counts equal or one higher | Scheduling correspondence does not supply minz's per-window qZC denominator |
| Spread | a9/a10 window sigma66.899/63.399us | E485 full-cycle sigma22.300–22.708us | Different quantities; no lower-jitter claim |
| Current | a9/a10 nominal time-weighted44.850/51.892mA | E188 operator~70mA at237.64eHz; E538 raw-only evidence at333.64eHz | Sensor/supply/mode/calibration differ; efficiency parity unproved |
| Recovery | No matched recovery counterpart established in these archives | Fresh-seed recovery verified3/3 at6.9%/20kHz on9ACC | Valid binz result; absent counterpart is a documented limit, not a request to reconnect minz |

Retain the negative reference too: lockmap20_a9.bin has4930 windows over
4.04874s,202.944eHz,3917 missing,20.547667%qZC,longest observed missing run5,
nominal weighted134.301mA. It is not pooled with the good a9/a10 runs. A speed
number alone therefore cannot stand in for sensing quality even in the
reference archive. Archive captures have no embedded CRC or verified flashed
identity tying them to frozen AM32; nominal current is not a calibration proof.

The separate same-rig AM32 E530 short response reached741eHz at input25
(actual compare26.519%). It rules against calling342eHz a demonstrated
rig-wide speed maximum, but later startup failures and absent matched sustained
metrics prohibit calling it a reliable full-envelope reference. Do not combine
AM32's short response, minz's qZC and binz's recovery into one fictional run.

Current binz7E55 has one6.9%10s hold; the call-boundary candidate failed and
was retired (E537). Remaining work: current-build repeatable envelope/recovery,
operating-current evidence at the expansion point, and a justified expansion
or stopping mechanism. Archive comparison is now current throughE539 and
does not depend on reference-hardware access. No full parity or goal completion
claimed. Portable lessons are indexed in PORTABLE_WINS.md.

## E285 current operating evidence (supersedes old build limits below)

On cached-comparator build807372B6 with tracingOFF, 5.4% completes10s and30s
startup/injected-tracking-loss/recovery campaigns near291eHz. Fresh recovery
arm40us/cost13 and original deadlines pass;30s includes~28s resumed operation.
At5.5%, three10s holds near297eHz passed, but a subsequent campaign stopped
on CycleTiming before its2s injection. Recovery at5.5% remains untested, not
proven broken or qualified. See TIMING_HEADROOM.md and LAB_REPORT E283–285.

The objective remains reliable duty-led expansion, max30% permission—not a
291eHz replacement target. Next expansion must address the intermittent
accepted-cycle timing boundary; raw-current calibration and independent
crossing-quality comparison remain open. Archived minz data/source only;
AM32 same-board testing is the operator's reserve, not an immediate dependency.

## E263 archive-grounded assessment and remaining scope

Supersedes historical implementation status below: real minz-core COMP/COM
BEMF, guarded driven entry and actual recovery are implemented. Powered-BEMF
duration is a finite engineering choice, not a5s authorization cap. There is
no250eHz objective ceiling and no obligation to force30% duty.

Reverified source archive2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44:
all128files match selected dependency (`freeze_minz_reference --check-source`).
Historical FALCON archive94df42404cd3e1735039fcba647bea93d795e6e11cd8823d446c921219355138:
hashes/membership and all3summaries reproduce (`minz_historical_baseline --verify-archive`).

| Quantity | Archived reference | Binz evidence | Assessment |
| --- | --- | --- | --- |
| Similar-speed operation | FALCON a9:230.020eHz,4.016s retained windows | E262234.442eHz30s hold; E258234.370eHz~28s recovered segment | Similar speed, different rig/controller/duration; not bar-for-bar parity |
| Crossing quality | a9:5543windows,1missing,99.98196%qZC; a10:257.476eHz,100% retained qZC with1sequence gap | E26242200COM/42199accepted | Near1:1 scheduling is not an independent rotor-crossing denominator |
| Timing variation | a9 single-window sigma66.899us | E262 full-cycle sigma48.174us | Different statistics; no lower-jitter claim |
| Current | a9 nominal time-weighted44.850mA;a10 51.892mA | E188 operator~70mA at237.64eHz; new phase sums uncalibrated | Different voltage/sensor/controller and uncertain calibration; no efficiency-parity claim |
| Recovery | No matched recovery benchmark established in these frozen captures | E258 injected-loss recoveries inside original10s/30s budgets | Binz evidence stands; missing counterpart is a comparison limit, not a reconnection requirement |

Historical FALCON records lack embeddedCRC and verified flashed identity tying
them to frozen AM32. Source archive is not a complete offline build closure.
Reproducible parsing does not establish matching firmware or calibration.

Keep envelope claims build-specific: older foreground cohorts passed5.3%
recovery~285eHz. E2455.4% stop has exact accepted-cycle3331us below3333us
software floor, not a physical motor/MCU maximum. Current phase/DMA build has
4.5%30s hold evidence; its17us ADC IRQ maximum/additional startup work do not
inherit earlier285eHz qualification.

Remaining: current-build recovery/repeatability and staged envelope, valid
current evidence and justified stopping mechanisms. Precision calibration and
flat histograms must not become invented requirements. Archive assessment and
portable candidates are updated throughE262; stated historical limitations are
not indefinite blockers. No rm32 port, full parity or goal completion claimed.

## E237 recovery and entry reliability improvements

Three same-setting5.0% startup/injected-loss/recovery campaigns now pass near
266eHz within their original10s budgets. Early statistics preparation saves
15us after the fresh recovery edge; actual remaining arm34.5..36.5us retains
the unchanged32us floor. One run also exercises a real one-time fresh-window
acquisition restart after a missing epoch, with twelve entirely new intervals.
See reentry_stage50_01..03 and LAB_REPORT E237. This is improved evidence for
the demonstrated point, not completed full-range qualification. Current remains
raw pulse evidence, not calibrated mean supply current. Archive parity and
portable-change consolidation remain deliverables; reference hardware stays
out of scope. Next work should move beyond equivalent266eHz repetitions.

## E235 measured instrumented duty-led extension

Release/s/thin-LTO under-drive entry now completes ten-second sustained windows
at4.6/4.8/5.0% commanded duty, approximately240.2/252.8/266.1eHz, using the
explicit coordinated bench-range300 guard profile. Raw evidence:
captures/cpu_union_range46_01.txt, cpu_union_range48_01.txt,
cpu_union_range50_01.txt. Each is ONE pass, not established entry reliability
or recovered operation across the range. The preceding normal-profile4.6%
CycleTiming stop remains a failure, not retroactively accepted.

Instrumented recovery at4.5% passes the original deadline with159us spare,
but actual arm remaining32us equals the minimum. This is a concrete constraint
for higher-speed recovery, not permission to refresh a seed or lower its floor.
Software IRQ-union partition stays about52.3..52.9% in these runs, INCLUDING
instrumentation effects. Foreground is not idle; total CPU headroom is unknown.
See TIMING_HEADROOM.md. Raw current peaks368..389 and bus floors10.77..10.97V
pass existing guards, but do not establish calibrated mean-current parity.

Next: qualify recovery at higher demonstrated points and current evidence,
repeatability and further staged expansion. Archived minz/frozen source only;
no reference hardware dependency. Full objective remains unfinished.

## Superseding direction: duty-led envelope, archived reference only

Operator revised goal after E193: experiment progressively up to30% PWM duty,
not a fixed200–250eHz ceiling. Speed is an observed result. Preserve existing
qualified limits until staged sensing/timing/current/tracking evidence permits
their expansion. Archived minz data/frozen source are the ONLY reference;
unavailable reference hardware is not a blocker and reruns are not required.
Driven open-loop BEMF qualification and validated handoff remain required.
Document archival mismatches without claiming nonexistent parity measurements.

## Current evidence — Entries188–189

Latest corrections E203/E208: two30s injected-loss/recovery campaigns at5%
completed near267.5eHz after coordinated acquisition bounds and latency fixes;
remaining handoff margin34us vs32us requirement is still tight. The old E201
refusal below is historical,not the current recovery status. Actual20ms
guarded under-drive observation now runs after200eHz startup. PWM-triggered
DMA acquires216COMP samples but still lacks a complete qualified crossing
sequence; no driven seed/handoff authority yet. Preserve the full objective:
repeatable entry/envelope/recovery,current/timing,archived parity and portable
wins remain required; these observer tests are not a substitute for completion.

Superseded range snapshot E200-201: experimental bench-range300 running guard
allows staged duty-led holds beyond250eHz. Ten seconds each at4.6/4.8/5.0%
produced242.89/255.10/267.20eHz,unchanged electrical/age/deadline guards.
Higher-speed recovery is NOT qualified: an injected-loss test at5.0% stopped
correctly and refused the fresh seed on its unchanged250eHz acquisition bound
(7578half-us measured cycle vs8000minimum). Coordinate acquisition and handoff
policy with the running profile next,with tests and explicit provenance.
No new PSU-current measurement; raw peaks are not calibrated mean current.
Independent qZC and driven acquisition requirements remain open.

Operator supply observation now exists:~70mA during a validated60s BEMF hold
at237.64eHz/4.5% duty,versus reported idle11.7V/0.7mA. Running voltage and
display precision remain unreported; this is not calibrated ADC mean current.
Following a startup current trip at6.5%,startup target reduced to6.2% with
catch still6.5%; guards unchanged. E189 advances powered duty to4.6% and
passes two30s holds at243.05/243.25eHz. Cycle sigma32.7/33.1us versus26.3us
at237.64eHz. Both validate finaloff and conditional early-coast half-rate
exclusion. No supply reading or recovery qualification yet at243eHz.
Older statements that the supply observation is wholly missing are superseded.
Full independent-quality/current calibration/matched-reference gaps remain.

## Current evidence — 2026-09-13, Entries184–186

This section supersedes the implementation status of older entries below.
Experimental coherent DMA/FIFO feedback has repeated30s dropout/reentry
campaigns near205 and238eHz (E181–182). E184 adds validated raw sums during
30s recovery near236.68eHz and a real queue-full shutdown followed by successful
same-boot reuse. Normal foreground-ADC firmware remains restored; no duty or
speed-envelope expansion. Current sums are not calibrated current.

E185 identifies the calibration epoch boundary in source: both initial BEMF
handoff and recovery cycle ENABLE. A pre-start idle zero cannot simply be
reused. Actual PSU readings/operator readiness for an independent known-point
hold remain pending. Do not substitute repeated raw sums for that measurement.

E186 rechecks the frozen128-file minz source against the live dependency:
match.111Rust/108Python tests pass. PORTABLE_WINS now includes the measured
DMA/FIFO ownership,aged-feedback and aggregate-delivery port contract,along
with timing/stack constraints and the explicit fault-injection exclusion.
These are portable candidates,not changes applied to sibling rm32.

Full-goal gaps remain independent whole-run rotor/quality evidence,qualified
mean current,and matched measured frozen-AM32 reference provenance. Neither
accepted/COM ratio nor historical FALCON data satisfies those missing items.

## Current evidence — 2026-09-13, Entry165

Supersedes the dated status below. E162 release now has two10s holds near205eHz
(E163) and a fixed3/3 cohort of30s campaigns near237.5–238.0eHz,each with injected
sensing loss at2s,guarded shutdown,fresh measured re-acquisition and successful
single re-entry for the remaining~28s. All retain original deadlines,separate
first-segment archives,CRC-valid timelines and final gates/MOE/ENABLEoff.
See captures/launch45_reentry30_cohort.csv. This is same-build repeated recovery,
not independent qZC/current parity. Stack span4108,measured untouched1520 on
recovery; no higher-duty or speed-envelope change was made.

Current-metrology work has established per-wake offset movement,voltage-domain
drift,nonuniform early ADC launch coverage and10us tracking time. These are
measurement limitations,not evidence that the comparator motor drive fails.
Do not make mid-ON current sampling a new prerequisite for existing-range
BEMF validation. Mean current still needs a calibrated unbiased measurement and
independent anchor; launch coverage is not that measurement.

PORTABLE_WINS.md now consolidates reusable policy,hardware invariants,test order
and M0 constraints for eventual rm32 work; no sibling firmware was changed.
Frozen minz archive and current source match reverified. Remaining full-goal
gaps are independent rotor/quality metrics,mean-current qualification and
matched frozen-AM32 measured baseline provenance. Historical FALCON evidence
does not resolve the latter while the minz bench is disconnected.

Entry166 adds independent early-coast full-period replay on all three E165
captures. With explicit100us read-bound uncertainty, each phase's5–6 measured
full periods has lower frequency bound>=184.4eHz,above the~119eHz half-rate
hypothesis. This supports rotor-related operation near the end of those runs,
conditional on valid/no-missed comparator edges and assumed timing uncertainty.
It does not replace whole-run independent qZC or prove exact shutdown speed.
Reproduce using scripts/drv_coast_periods.py with `--uncertainty-us 100`.
No polynomial extrapolation used.

## Current evidence — 2026-09-13, Entry153

This section supersedes the Entry137 status below, retained as history.
Real guarded recovery is demonstrated near206 and238eHz. Latest E152 firmware
completes a30s campaign at238.236eHz,including injected2s tracking loss and27.988s
recovered operation,40007COMs,original deadline retained,final off. E150 exposed
a post-stop recorder race; that capture is rejected. E151 serializes stop/record;
E152 directly rejects24stopped callbacks on hardware. Final-edge confirmation
saves40us without shortening20us dwell or32us arm margin. Failures are retained.

Historical minz raw data is separately frozen in
`captures/reference/minz_falcon_historical_20260913.zip`,with hashed parser from
the existing source freeze,session logs,notes and three raw captures. These are
FALCON measurements,NOT proven captures of our frozen AM32 revision.
Near230.020eHz:5543windows,99.982%qZC,one missing window,nominal44.85mA
time-weighted DC-link current. A separate203eHz capture has20.55%qZC and must
not be used as healthy baseline. Same motor/load and capture firmware hash are
not established for an apples-to-apples current comparison.

Remaining completion evidence:

- Time-resolved quality and independent crossing/rotor evidence on binz:
  accepted/COM ratio is not minz's independent-window qZC metric.
- Calibrated,de-cohered mean supply-current evidence; sequential phase feedback
  and peak guards are not a directly comparable DC-link mean.
- Matched-speed measured frozen-AM32 baseline provenance. Minz is disconnected;
  historical analysis is now reproducible but does not establish this provenance.
- Same-build sustained/recovery repeats and portable-change consolidation.
  A30s success alone is not the full parity criterion.

## Current status — 2026-09-13, after LAB_REPORT Entry137

This section supersedes the historical implementation/duration statements below.
The operator makes duration an engineering choice; neither5s nor10s is an
operator ceiling. Current firmware still bounds open-loop startup to5s and
supports finite powered windows20..600000ms with independent safeguards.

Measured: full minz-core COMP/COM path controls the motor near206eHz at4%
powered duty. Four/four10s attempts and one60s hold complete on E134 firmware;
the minute has74368COMs,319175ADC feedback scans and verified final off (E136).
Candidate-only acquisition waiting is exercised on hardware (E135). Other
documented operating point: roughly237eHz at4.5%, not a controlled speed setpoint.
Frozen128-file reference hash/source match reverified in E137. shell-sine intact.

Remaining completion evidence, not interchangeable with a long successful hold:

| Requirement | Evidence now | Missing / next action |
|---|---|---|
| Repeatable range | Repeated~206eHz; earlier~237eHz holds | Matched protocol across range, retain every failed entry |
| Lock quality | Whole-stream interval moments, ordered accepted events, independent coast speed | Time-resolved quality/dropout view and independent denominator; accepted/COM is not qzc |
| Recovery | Reference simulated re-kick; live guard stops lost tracking | Qualify a disabled re-acquisition and bounded re-entry; no powered recovery demonstrated |
| Current | Phase pulse maxima, bus floor,800mA supply setting | De-cohered/calibrated mean-current measurement; peaks are not mean supply current |
| Reference parity | Source snapshot verified; full logical-time tests | Comparable measured baseline/provenance; minz bench is disconnected |
| Portability | G0 safing, timers, measured-seed entry and replay tests | Consolidated portable-vs-board changes and remaining timing/RAM/stack budget |

Recovery constraint demonstrated by E137 policy replay: fresh ADC does not
prevent the independent1ms accepted-event watchdog from latching Tracking;
the reference's~22.5ms timeout re-kick is later. Production foreground exits
when guard ownership is lost; commit rechecks ownership and the policy latch
inside one critical section. Do not call absence of natural dropout a recovery
pass, and do not widen blind-drive time to make re-kick reachable.

Next recovery implementation/qualification sequence:

1. Add fixture-only scheduled comparator-input suppression during an established
   powered hold, with explicit applied/start/end counters. Independent guard,
   current/bus/driver checks and host abort remain untouched. First test must
   stop safely, with no automatic restart, and retain stop/last-accept timestamps.
2. After drive is disabled, clear/mask stale COM/COMP state and run the existing
   twelve-interval passive acquisition. No invented crossing or refreshed old
   seed; record elapsed coast time and all refusal reasons.
3. Only a newly qualified seed plus fresh ADC baseline can create new guarded
   authority at the already-qualified duty. Preserve the original absolute
   campaign deadline and bound retry count; electrical/host faults never retry.
   Current powered_timer startup budget accepts only elapsed<5s, so it cannot
   silently serve as the recovery budget after a60s hold. Refactor/test that
   accounting explicitly before implementing long-run re-entry.
4. Replay authority transitions and timer/pending-event races, then hardware
   stop-only, disabled acquisition, and finally one guarded re-entry. Measure
   loss-to-safe, acquisition latency, recovered rate/current and sustained
   post-recovery progress. Synthetic suppression tests are not a motor-load
   disturbance test or automatic parity with the reference's blind re-kick.

These are pending implementation gates, not a claim that recovery exists.

Operator objective, 2026-09-12: reproduce `../minz` performance bar for bar on
the Cortex-M0+ G071, then fold demonstrated improvements into `../rm32`
(the AM32-derived firmware). Open-loop sine rotation is a bring-up reference,
not completion of this objective. This document is direction, not permission
to expand the current motor envelope or run unattended tests.

## Sources and corrections

### Campaign redirection after GRAYBEARD_BEMF_MEMO.md (2026-09-12)

The renewed memo supersedes the old memo's stale architecture advice. Stop
making short-window ADC agreement or a perfect crossing in every open-loop
sector the production qualification gate. ADC limitations remain documented,
but further alternating-cycle ADC work is deferred. The hardware comparator is
the production sensing path; retain analog cross-check as supporting evidence,
not as a prerequisite for every comparator experiment.

Checked current minz-core: polling switches to interrupt mode when ci<2000
half-us ticks; commutate falls back when average_interval>2500. Thus50 eHz
is6667 ticks (polling),200 eHz1667 ticks,250 eHz1333 ticks. The existing binz
microscope itself polls COMP2 and has NOT yet implemented EXTI. Its two-sample
bench detector is not the full production polling or interrupt algorithm.

Next implementation: replay the actual polling and interrupt sequences with
timer/mux/edge semantics, interval-history average, mode hysteresis, timeout/
recovery and desync behavior. Reuse zcfoundroutine and COM/COMP handlers via
a simulated HAL where possible; do not replace them with an isolated blend.
The interrupt handler arms a commutation timer after accepted comparator input;
do not assume arbitrary missing ZCs are harmless free-running predictions.
Absence of desync_due alone is insufficient: the predicate is gated at avg<2000
and does not cover every timeout/recovery path.

Freeze a reference source snapshot including dirty-file hashes, not HEAD alone.
Inspected HEAD is2efda4911c4be8e238e16d192d821151bd775887; the minz tree has
modified core files, so this hash is NOT yet a reproducible reference snapshot.
Extract real matched-speed acceptance data before claiming parity.

Bench destination is approximately200–250 electrical Hz with staged V/f drive,
as recommended by the forwarded memo. Do not turn this into an immediate
unqualified high-duty run. Existing10% firmware ceiling and800 mA/<5 s gates
remain until the revised staged plan establishes the required current/tracking
evidence and envelope authority. Proportional6%@50 extrapolates to24%@200,
but that is not a calibrated motor schedule or an automatic setting. Prefer
reference-grounded interval-lock/dropout/recovery measures over invented23/23
crossing perfection. All six phases still need credible identified evidence.

Read `GRAYBEARD_FAST_RAMP.md` (the filename uses GRAY), `REUSE_PLAN.md`, and
the current DRV wire map. Reuse minz-core control decisions through board HAL
adapters; do not independently rewrite filter/blank/advance/recovery science.

The graybeard's Hall shortcut belongs to a different setup. No Hall reference
is established on this rig; PB3 is now BEMF A. Likewise the older EVLDRIVE
claim that full three-phase hardware-comparator sensing is impossible does
not apply: DRV A/B/C reach multiplexed COMP2 inputs PB3/PB7/PA2, with neutral
PA3. Do not port old board safing, current calibration, or ADC architecture
verbatim. Current capture has phase-C analog voltage, not analog A/B voltage.

## Instrumentation contract

- Terminal mode: quiet by default, bounded completion/guard summaries.
- Fixture mode: explicit opt-in and acknowledgement; one-shot bulk capture,
  automatic return to quiet. No encoding/formatting in the control hot path.
- Buffers and guards remain active even when transport is disabled. Quiet
  must not mean blind or unprotected. Retained decision counters and compact
  frozen fault context should precede closed-loop experiments.
- The inspected minz `scripts/gecko.py`, `waxwing.py`, and `magpie.py` decode
  Ascii85 via `base64.a85decode`; GECKO uses explicit header/body/end framing
  and little-endian u16 samples. This is evidence for Ascii85, not Base92.
- Next transport increment: versioned Ascii85 snapshot frames with explicit
  schema, lengths, sequence/timestamps and corruption detection. Preserve old
  replay, test round-trip/truncation/corruption and throughput before adopting.
  Four raw bytes encode to five printable characters, versus eight hex digits
  plus current separators. Do not reinterpret memory structs with padding.
- Continuous high-rate event tracing is a separate bounded binary transport
  (the graybeard points to ZC_TRACE), with sequence/loss/overflow counters.
  A higher baud rate is a separately qualified change, not assumed available.

## Evidence ladder

1. Freeze a minz reference revision and its actual acceptance metrics:
   startup reliability, lock/dropout map, speed/load envelope, transient
   current, ZC acceptance/rejection reasons, recovery, and worst-case timing.
   Record motor/supply/load differences; neither equal duty nor a matching
   endpoint alone establishes performance parity. Do not invent new targets.
2. Build the DRV sensing adapter and timed six-step observation mode. Sine
   drive energizes all phases; conventional floating-phase six-step BEMF
   needs an intentionally undriven phase. First observe without granting the
   detector authority to commute. Measure PWM-window validity, comparator
   mux settling/polarity, blanking and crossing ramps across every sector.
3. Replay identical events through reused minz-core and the reference.
   Count every veto/rejection; validate timing and phase relationships,
   not merely presence of crossings. Coast currently proves end-of-run
   rotation, not event-by-event commutation ground truth.
4. Enable sensorless commutation only after observation gates pass; compare
   repeated startup, hold, transitions and recovery with reference quantities
   calculated the same way. Instrumentation-on/off A/B must expose observer
   overhead. No wider duty/speed/current envelope without operator agreement.
5. Budget M0 worst-case ISR/critical-section time, TIM17 wrap handling, RAM,
   painted-stack high-water and soft wide-arithmetic helpers. G071 has no DWT;
   the M4's ADC/injected-group, DMA and arithmetic assumptions are not portable.
6. Promote only measured improvements into rm32, separated into portable
   control changes, G0 HAL fixes, and reusable capture/qualification tooling.
   Carry replay regressions and bench acceptance artifacts with each change.

## Current implementation boundary

Quiet mode and one-shot Ascii85/CRC snapshots are flashed and live-tested.
Relative gate/VSEN identity is qualified (Entry 015); paired-current tests
established logical A/B/C = ADC4/1/0 (Entry 016). Do not reopen those questions
without contrary evidence. Six-step observe-only captures and v7 shared detector
replay are implemented, but the detector does not control commutation and is
not the full AM32 sequence. Timing-qualified digital observations supersede
the earlier compulsory ADC-sign gate. shell-sine remains untouched.

Entry 034 freezes the modified minz control reference in
`captures/reference/minz_20260912.zip`; its README records checksum, scope,
verification commands and benchmark-provenance gaps. Source tests pass, but
integrated timed-HAL replay, matched-speed benchmarks and BEMF control remain
unfinished. The live dependency must match the frozen source before replay.

The replacement user goal explicitly authorizes staged increases above 10%
up to a hard 30% exploratory ceiling, conditional on preceding measured
current/bus/tracking evidence. Firmware remains capped at 10% until guards
(including loss-of-tracking) and stage gates are ready. Retain 800 mA and the
five-second energized limit; do not immediately apply extrapolated 24%@200 eHz.

## Current-state correction — E762

The paragraph immediately above is historical and no longer governs the live
campaign. The hard commanded ceiling is30%; the operator set the PSU and
nominal signed-average target to1A. Live control reached25%, where corrected
average current stopped it. The qualified representative point is15% for30s,
3/3 on the lean E762 image. Tracking loss now shuts down and completes one
normal autonomous startup restart within the original deadline (E761).
Flying reacquisition is not an envelope gate. See `AUTONOMOUS_STARTUP_E758.md`,
`NORMAL_RESTART_E759.md`, `CURRENT_SIGN_E756.md`, and
`LEAN_QUALIFICATION_E762.md`; their hashes and limitations supersede this
plan's older10%/800mA/five-second boundary.
# 2026-09-15 E765 current qualification

The division-free/rating-clamp lean image now qualifies20% for30 seconds 3/3
on the G071: 155,645–158,254 commutations, event age31–111us, feedback age
120–124us, no event fault/veto, deadline reason2 and final outputs off. The1A
sustained-current boundary is bracketed by this20% pass and a22.5% reason25
stop after20.085s;30% was commandable/tracked but current-limited. This is no
longer a BEMF or compute ceiling. Frozen SHA and captures are in
`M0_CLEAN_ENVELOPE_E764_E765.md`.

## 2026-09-16 E797 duty-50 correction

Meaningful-step exploration now reaches35% for30s at about1.56keHz with a
smooth1%-per0.25s live ramp, no foldback, tracking fault, fast-cycle event,
phase rail or low-bus sample. A40% request crossed the configured2.5A nominal
average allowance by2.2% in one block and folded to35, then completed the30s
window. This is a current-setting boundary, not a BEMF/CPU ceiling or hard PSU
fold. The operator must set3.5A before firmware adopts a matching guard and
continues toward45/50. Exact artifacts and caveats are in DUTY_50_CAMPAIGN.md.
