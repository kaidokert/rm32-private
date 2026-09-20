# Completed comparator decision tail - E407

## E415 diagnostic retired; baseline recovery restored

F730 preserved/hashverifiedreference/decision_f730. Installed archivedA2DE
DMApeer baseline (no decision/critical features), rootELF stillF730.
retire_decision_guard01 UARTsilent; exactRCC08000000/PD1zero/BDTRc1a/CCRs0
then documented08040000repair. guard02PASS3faults/18refusals; preflight01
fivePASSCPU2/7/10,finaloff. COM8 untouched, no competingCOM41/probe.

baseline_start61_reentry68_30s_01 PASS original30s,27.990602s resumed315.7165eHz,
53023COM/53022accepted,IRQ53.529949%,sigma24.118757us,raw260bus10901,
stack2676,seed1072/acquisition6708us/arm49cost12,DMA22queue2. Finaloff/exit0.
CaptureSHA E4C728095D9DE28B892D6673E8F0C0CDF29692A7B6046B6872EBE9CCD6329476.
This restores lower-overhead6.8 operation; no6.9 qualification or solecause
claim. Keep decisiontail available as diagnostic source, not enabledproduction.

## E414 recovery fault captured; no long qualification stall at refused event

decision_start61_reentry68_30s_01, sameF730/start61/BEMF68, original30s window.
Recovered with seed1058/acquisition6594us/arm37.5us/cost12, then2.666888s
resumed at316.346912eHz beforeCycleTiming12:step2 2663835->2666850=3015us,
below3031floor.5062COM/5061accepted,sigma37.287us,IRQ70.404862%,raw319
bus11056,stack2120,criticalmax48/refused0,DMA24queue3. Finaloff/fixtureexit1.

32CRC rows include final stopped/unknown call: gatecount828 vs reference
this_zc841 =13half-us ticks (6.5us). Previous accepted step1 gatecount1218
vs last_zc1231 also13ticks. No long gate-to-accept stall at the refused event.
These comparisons are within the reference timer; do NOT subtract guard
timestamps from observation-entry times (distinct epoch origins). Cannot
infer physical edge arrival or pre-entry latency. Several open-no-accept
visits precede both accepted events; rejected visits lack per-level sequence.

Decision44090total:7984closed/31044open-no-accept/5061accepted/1unknown.
Reference adjacent cycles6356/6019ticks; guard/reference residual5.5us.
No recovery qualification. Stop additional powered retries on this expensive
diagnostic; next retire instrumentation and unhelpful critical experiment to
an archived baseline with disabled requalification. Do not widen guards.
CaptureSHA A7163735522BB73C383FD2E8E022987BD4FCD9CA9B88D4C953C749F7707BF24F.

## E413 powered capture passes, overhead is material

E412 scan spans162us, but DA85 records scanstart/wholeage, not C aperture or
PWM phase. Cannot discard current spike as switching artifact. Controlled
startup62->61 change, BEMF68 and allguards unchanged, actualF730.
decision_start61_hold68_01 PASS10.000032s316.098372eHz,18966COM/18965accepted,
raw240bus10925,stack2748,criticalmax32/refused0,guard20/commit26/DMA22us,
queue3. Startupraw546/bus11450,arm58us/cost12. n1 notstartupreliability proof.
165204dispatches:29792closed (18.033%),116447open-no-accept (70.487%),
18965accepted.32CRCrows/165172omitted; host now checks aggregate epoch equality.
IRQunion70.397%, vs preceding~59%; diagnostic is materially costly, not an
exact causal cost estimate. Finaloffverified/fixtureexit0/portclosed.
Next preceding6.8 recovery withstartup61, not higherduty. CaptureSHA
1F2A82EE7E62B98C8CDDB229966EABCBBD6D10A8BFCD9E6A2C1E16CCEF4F28ED.

## E412 first powered attempt stopped during acquisition

decision_hold68_01 requested10s, startup6.2/BEMF6.8%, unchangedguards andF730.
StoppedCurrent5 at11237us before BEMFhandoff.70CRC-checkedADCrows; peak raw
deviation1256counts vs1200guard. Last row11063us logicalC3304,bus11605mV.
This is instantaneous ADC evidence, not calibrated phase amps or PSU current.
Decisiontotal0/criticalcalls0: the new recorder was never exercised. Thus no
combined overhead, powered capture or6.8%qualification result obtained.
Finaloffverified,fixtureexit1,portclosed. No blindretry/dutyincrease/guardrelaxation.
Next inspect startup sample timing/entry variation; do not infer recorder
regression or hardware fault from this refusal. SHA256
894653C0410E9BBED58FD5550A0BEE7A0D43660C77577D5A16771657C6B89A19.

## E411 installed, disabled hardware checks passed

ActualF7302F50D950378593765275CF88514923183C827B4E5C5AD9D8011067020818.
Previous044F preserved/hashverified at captures/reference/compcritical_044f.
Preflashoff verified, no competingCOM41/probe, COM8 left alone. Reset UART
worked without repair. decision_scope01 five modes16/16PASS maxima9/9/9/8/8us.
SHA256291DCE7F70DF6F03B814F90A05FF5E80715D545C598FA342E1F3483DF5E09FE0.
This is actual Scope lifecycle/timing, not full ISR or reference qualification.
decision_guard01 passed3faults/18poststop refusals; decision_preflight01 all
five checksPASS, CPU2/7/10us. Finaloff verified and fixtures terminal/portsclosed.
No motor command on this image. Next bounded preceding6.8% diagnostic hold,
strict --comp-decisions/--comp-paths/currentbuild markers. Inspect whole-run
occupancy, critical-body max, stack and all guards before higher duty. Do not
infer whole-ISR headroom from passing recorder or CPUCHECK independently.

## E410 actual-Scope disabled check implemented, not run

decisioncheck refuses owned/active/non-disabled controller state. Five modes
closed/open-reject/accepted/stopped/Drop exercise actual begin, first_count,
finish and destructor16times each. A second count must not overwrite the first;
checks row classification, inactive scope and stop freeze. TIM17 bracket covers
scope work, not reset/formatting, not full reference service or interrupt entry.
This diagnostic resets decision history and is only for an inactive epoch.
Host drv_decision_check.py retains exclusive captures and finaloff, validates
all modes, reports maxima without declaring a timing pass.344PythonPASS.

Release candidateF7302F50D950378593765275CF88514923183C827B4E5C5AD9D8011067020818:
text127732/data1128/bss29876,flashspare2212. Not flashed/run. Actual044F remains
installed/offE404. Archive044F before candidateflash, then run disabled scope
check and normal preflights; no motor qualification from host tests alone.

## E409 resource/code audit and quiet framing

Current unflashed1125C2E3F696E370D7817CB71FC4F6A77F94627B1C1BB454903A9E07C1EE9215:
text126860/data1128/bss29876, flash spare3084bytes, RAM after statics5860bytes.
These are capacity calculations, NOT measured runtime stack margin. TAIL is
532bytes on target. PrecedingCF23 Scope::complete uses44local stack bytes
plus saved registers, no division helpers, and duplicate path classification.
Ring index masks compile as integer masking; possible bounds panic retained.
This audit is not a timing measurement or proof of full-handler WCET.

Header now says capture=0/1. Optional quiet decode returns uncaptured metadata;
required fixture capture rejects quiet/unused paths. Orphan CD85 rows rejected.
343Python tests pass, release build succeeds. No flash/motor actions.
Remaining: disabled actual Scope begin/first_count/finish/Drop tests and timing,
including stop path, before powered use. CPUCHECK alone does not cover this code.

## E408 opt-in integration, not flashed

bench-comp-decisions requires existing comp-paths/static-comp. Scope captures
observation entrytime/sector beside begin, reuses actual Interval::count hook,
and pushes after reference completion. Drop closes earlyreturns as unknown;
stopped calls freeze in push, poststop dump freezes any still-open history.
Reset uses existing observation reset including recovery. No extra comparator
read work. CRC CD85 rows only when capture=true; compact header always on the
optional build. D85 was already reserved by older ADC capture and is not reused.
Strict host decoder and --comp-decisions fixture flag implemented. Release
build succeeds; not flashed. Generated-code timing/size and actual scope-path
disabled tests still required. Earlier unintegrated statements below are history.

Pure buffer implemented in examples/support/comp_decision_tail.rs and tested
through observer-replay. NOT integrated into shell-pwm, built into its ELF,
flashed or motor-qualified. Installed044F/offE404 unchanged.

32 records of16bytes each: epoch-relative entry timestamp, dispatched ordinal,
first actual interval count plus average, sector/path/count-presence metadata.
Uses existing comp_paths classification, not another comparator read. None
and valid count65535 are distinct. Ordinals and time cannot silently wrap.
Ring indexing uses a const power-of-two mask; no modulo or wide arithmetic.
Total structure <=544bytes asserted on host. Target layout/codegen pending.

Stopped calls are unknown, retained, and then freeze. Explicit external freeze
does not manufacture a call. Bad metadata freezes without publishing a row;
invalid is distinct from stopped. No row overwrites after freeze. Reset must
occur only outside active sensing; caller owns serialization and epoch setup.

At roughly19000 dispatched calls/s,32rows span only about1.7ms: this is a
local decision tail, NOT a full electrical-cycle record or uncensored sample.
It has no physical edge-arrival timestamp, per-read sequence, or IRQ source
nesting information. A decision gap cannot identify a preemptor by itself.

Remaining integration requirements:

- Opt-in only; preserve existing static real/inverted comparator implementation.
- Reuse the first actual Interval::count hook, never sample a replacement count.
- Timestamp entry in the observation epoch; store after the reference call.
- Retain the stopped call even if the reference callback safes the motor;
  prevent early-return/filter-refusal paths leaving an active scope behind.
- Stop/reset ownership must be explicit, including recovery and foreground abort.
- Post-stop fixture-only CRC framing, strict host decoder and optional provenance.
- Audit release s/LTO generated code, flash/RAM/stack budget and added handler
  latency; qualify disabled before preceding6.8% powered comparison.
- No widened guards or claimed independent rotor timing; do not silently enable
  trace1 or the previously rejected full-IRQ observer.

Tests cover wrap/omission chronology, stop retention, field bounds, gate
boundary, missing-count distinction, timestamp reversal, reset, ordinal overflow
and memory footprint. These establish buffer semantics, not ISR timing.
