# Full-goal acceptance audit — 2026-09-13

## Superseding goal revision (after E193)

Current goal is attachment90c16e2b-6f02-47e6-b689-ddc0f9a0895d/pasted-text-1.txt.
The previous reference-hardware blocker is CANCELLED by operator instruction:
archive-only comparison with explicit limitations is the required deliverable.
No250eHz goal ceiling; duty is experimental input,maximum30%. Existing safety
envelopes require staged requalification,not deletion. Completion still needs
reliable driven acquisition/handoff,repeatable startup/hold/recovery across
demonstrated duty/speed,measured current/timing,limiting mechanisms,archive
comparison and portable wins. Historical completion criteria below must not
override this revision. Goal remains incomplete; work resumes without minz.

## Current continuation boundary after E193

Not complete. Repeated BEMF holds and injected-loss recovery now reach~243eHz
at4.6% powered duty; same6.2% catch/target recovery cohort is3completed of4
attempts,including one passive acquisition refusal. All three powered windows
recover within the original deadline and verify finaloff. One60s hold near
237.64eHz has operator-observed~70mA supply current. These are actual motor
results,not just instrumentation,but not independent whole-runqZC or calibrated
matched-reference current parity.

The missing matched frozen-AM32 baseline persisted through E192,E193 and this
continuation. E193 exhausted the immediate historical replay check: retained
FALCON metrics reproduce,while their flashed-source identity and conditions
remain insufficient for the original requested comparison. More equivalent
binz holds,additional tests of the same parser or relabeled FALCON metrics
cannot supply that physical reference evidence. Pause the autonomous parity
campaign pending a matched reference capture with firmware/motor/load/supply
provenance,or availability of the disconnected minz bench for that measurement.
Do not interpret this pause as completion of remaining binz calibration,
independent quality or entry reliability work. No motor/flash action in this
continuation; last powered attempts ended verifiedoff.

E188–189 update: the pending supply-display observation was obtained (~70mA
during60s at237.64eHz,operator-reported). Thus the earlier no-reading blocker
below is historical,not current. Idle11.7V/0.7mA confirmed; running voltage,
precision and calibrated ADC means remain open. Two additional30s holds at
4.6% duty pass near243.05/243.25eHz,unchanged safeguards and verified finaloff.
Recovery at this new point and full independent-quality/reference parity are
not yet proven. See LAB_REPORT E187–189 for the retained startup failure and
the revised6.2% startup target; no claim of a physical speed limit.

Verdict: incomplete. This is not a replacement objective or authorization to
lower the completion bar. The user attachment remains authoritative; subsequent
operator instructions supersede its five-second duration ceiling. No duration
permission is needed per run, but each campaign remains finite and guarded.

## Requirement-by-requirement evidence

| Original requirement | Evidence / verification | Remaining qualification |
|---|---|---|
| Trustworthy comparator BEMF around200eHz | Current E162 raw launch40_01/02 revalidated: two complete10s windows at205.4–205.7eHz | Whole-run independent rotor/quality measurement remains missing |
| Staged startup/speed campaign, optional250eHz/30% exploration | E165 raw captures retain6.5% startup and stepped50→200eHz command followed by4.5% BEMF equilibrium near238eHz |250eHz and higher duty not qualified; no claim of a demonstrated physical speed limit |
| Preserve successful sine and phase mapping | shell-sine SHA256 B6BBA6604F9534C45B2581253FC5EBB49AE1F24DA3E48891AE3D1C161D54206E unchanged; new ADC coverage uses logical4/1/0 | Mapping is established historical hardware evidence, not newly remetered by this audit |
| Quiet default, fixture-only compact data, compatible replay | Current launch dump gated by capture opt-in; snapshot CRC and old parser retained;98Python tests last pass E166 | Every new wire extension needs its own tests; transport speed spike is not required for current-rate operation |
| Freeze actual modified minz source |128-file archive plus current source reverified, SHA2562799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44 | Freeze is complete; measured acceptance benchmarks for this frozen AM32 build are not |
| Full AM32 sequence via simulated HAL and observe-only before authority | Current Rust suite rerun:74unit +1recorded-polling +21sequence tests pass. Includes polling→IRQ, blank/persistence, average, COM scheduling, timeout and stop behavior; E054–056 and later entries retain bench chronology | Tests verify modeled events, not arbitrary hardware noise; historical observe-only capture provenance is not re-audited in full here |
| Current,bus,fault,tracking protection before expansion | E162 guardcheck18/18 and recordcheck12/12; E165 raw three recoveries validate latching stop/fresh re-entry/original budgets/finaloff | Safety phase peaks are not mean supply-current calibration; unchanged800mA is a setting, not a measured current |
| Measured M0 timing / appropriate comparator regime | Three same-build30s runs near238eHz; measured ISR/commit/recorder and stack evidence in E165 | No worst-case nesting proof; stack span4108,measured untouched1520 are constraints to preserve |
| Rotor-related evidence distinct from artifacts | E166 independent gates-off comparator full-period bounds exclude half controller rate under stated timing/edge assumptions | Not whole-run qZC; coast data cannot prove every powered interval or exact shutdown speed |
| Repeatable sustained BEMF and recovery | E165 launch45_reentry30_01/02/03 raw revalidated:3/3 complete, reentry_verified1, separate archives/timelines, original deadlines | Injected sensing suppression is one disturbance class; no general mechanical/load-disturbance parity claim |
| Matched lock/dropout/recovery/current benchmarks against frozen reference | Historical FALCON archive is independently preserved and analyzed | Different controller/board/supply and uncertain capture firmware provenance; minz bench disconnected. No matched frozen-AM32 benchmark exists in established evidence |
| Document gaps and portable improvements | BEMF_PARITY_PLAN.md,CURRENT_MEASUREMENT_PLAN.md,PORTABLE_WINS.md | rm32 implementation is eventual work, not claimed as already ported |
| Raw attempts and verified finaloff | Five current-build raw captures revalidated directly, not just CSV statuses | This audit does not retroactively certify every historical experiment; known failed captures remain failures |

## Missing evidence and next dependency

E180–182 update: experimental IRQ/FIFO ADC delivery now passes3/3 ten-second
holds near205eHz and3/3 thirty-second injected-dropout/reentry campaigns at
each of~205 and~238eHz. Retained cohort CSVs are dma_fifo40_cohort,
dma_fifo40_reentry30_cohort and dma_fifo45_reentry30_cohort. DMAmax10us,
queuepeak4/8,recovery stackuntouched1580. This advances sampling delivery and
restart qualification,not mean-current calibration or matched-reference qZC.
At~238eHz cycle sigma~27us exceeds the older foreground cohort's~21us; do not
claim timing parity from successful completion. Default image remains restored.

An independent supply reading can establish a useful current fact without
waiting for precision ADC metrology. The operator has been asked for actual
idle voltage/current and readiness to observe a known-point60s hold. No reply
has arrived; do not start that measurement assuming someone is watching.

This input is needed for the supply-anchor experiment, not because further
firmware work is forbidden. Whole-run independent quality and precision current
sampling still require engineering. Matched frozen-AM32 bench provenance
requires suitable retained reference evidence or an eventual reference run;
neither FALCON similarity nor repeated binz spins can manufacture that evidence.

Do not declare the entire goal blocked solely because the optional PSU reading
is pending if meaningful in-scope firmware/evidence work remains. Do not declare
completion from the successful current-build cohorts while these gaps remain.

## Continuation dependency audit after E186

The E185,E186 and following continuation still have no actual PSU reading or
operator readiness for the independent supply-anchor hold. E185 completed the
calibration-epoch audit; E186 completed the missing portable-delivery contract
and revalidated111Rust/108Python tests plus the frozen source. Those useful
offline actions are finished,not reasons to repeat equivalent verification.

Read-only tool/device discovery now confirms serial MCP,COM41 FTDI and the
expected ST-Link066CFF343433464757233430 are available. Windows CIM omitted
the FTDI ports,but both pyserial and serial MCP enumerate them; do not infer
a disconnected rig from that incomplete CIM result. No dedicated PSU or
independent rotor measurement tool is exposed. Other serial devices have not
been identified as instruments and were not opened or commanded.

Pause autonomous campaign pending external measurement input. Immediate resume:
actual idle PSU voltage/current and operator readiness for the documented
known-point hold. Full parity also still needs matched frozen-AM32 measurement
provenance and independent whole-run quality evidence; one PSU reading does
not complete those requirements. Do not manufacture that evidence by relabeling
raw sums,accepted-event ratios or historical FALCON measurements. No motor,
flash or output-state change was made during this dependency audit.
