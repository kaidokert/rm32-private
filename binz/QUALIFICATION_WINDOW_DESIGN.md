# E452 — small qualification-window diagnostic, staged pure logic only

The old completed-call ring adds per-call timestamp/ordinal/ring work and
spans less than one cycle. Full nesting instrumentation previously exceeded
CPU gates. Do not silently reactivate either while claiming low overhead.

New qualification_window::Window is <=8bytes: completedcallcount, number of
calls spanning guard-sequence progress, maximumcompletedcallus and explicit
saturation. It aggregates scalar evidence between acceptances; no per-read
work, ring indexing, division or extended-clock sampling in this pure layer.
Three host tests cover reset, wrap-crossing sequence inequality, saturation
and copied fault preservation. This is NOT wired into firmware or measured.

Integration contract before any powered test:

- Increment a diagnostic guard sequence on guard ISR entry. Read it immediately
  before/after the actual reference qualification call, not wholeCOMP dispatch.
  Reuse existing call timing where possible; do not include unrelated logging.
- Equality/inequality only proves observed sequenceprogress across the bracket.
  It does not count ticks, measure pre-entry latency or recover a full counter
  wrap. Valid measurement requires fewer than2^32 guard entries in the bracket.
- Retain one summary peraccepted event with sector/ordinal identity, enough
  records for preceding/current cycle. A rejected call belongs to the pending
  acceptance window, not a fabricated edge. Explicitly retain omissions.
- The accepted callback can safe the motor BEFORE the referencecall returns.
  Complete and retain that finalcall as fault evidence before freezing; don't
  let observation_reset or faultreport erase it. No control action depends on
  diagnosticdata. Foreground reset/dump only after producerinactive.
- Countallcalls versus gate-open qualification calls must be labelled honestly;
  this accumulator alone cannot distinguish them. If the first actual gate
  count is required, reuse the existing hook, not a replacement timerread.
- Compare preceding and faultcycle summaries, not globalmaxima. A longcall
  with guardprogress supports interference inside this bracket; absence does
  not exclude pre-entry masking or repeated PWM rejection between calls.
- Optionalbuild/quietpoststopcapture, strict framing/parser, generatedcode/
  stack review and actualdisabled timing gates before any motor run. Preserve
 32usarmfloor,16usarmcost, all electrical/tracking guards and profile350.

No hardware activity or embeddedbuild inE452; actualACEE/off. Next implement
bounded retained summaries and explicit completion/freeze ownership before
adding ISR hooks. Do not call this instrument available on the live board yet.

## E453 — bounded history implemented in pure logic

History retains16 acceptance-window summaries (more than two six-step cycles)
with diagnosticacceptedordinal, sector, accepted/stopped flags and explicit
omission count. Nonaccepting calls update scalarpending data only. Final
completion publishes before freezing, including a stopped call with no accept;
accepted-and-stopped is distinct. Idlefreeze retains pending without inventing
an event. Invalidmetadata/ordinaloverflow freezes diagnostics with invalidflag.
It does not shut down control or grant authority. Caller serialization and
no-inflight-call contract for idlefreeze remain integration responsibilities.

201library+34other RusttestsPASS (3 new historytests); hostHistory<=320bytes.
Tests cover rollover, finalfaultcall, refusedwritesafterfreeze, stopwithaccept,
idlepartialwindow, metadata/ordinaloverflow. Embeddedlayout/timing unmeasured.
No shell integration/build/flash/motor. Next optionalISRhooks, strictpoststop
framing and disabled completion/overhead tests; actualACEE/off unchanged.

## E454 — optional shell integration builds, NOT flashed

bench-qualification-window adds a guardentry sequence and poweredtrace0
Scope surrounding the dispatched body (including staticmotor setup), NOT
just the reference function. finish runs after callback returns, even if it
safed outputs; Drop invalidates an unexpectedly unfinished scope. Guard only
updates its atomicsequence, never HISTORY. No comparator per-read hooks.
Observationreset resets history; dump freezes only poststop. SnapshotQW85
rows containordinal/sector/accepted/stopped/count/overlap/max/saturation;
QP85 retains idlepending. Header explicitly saysdispatch_body=1,edge_time=0.
Records only when fixturecapture=true. Strictdecoder not implemented yet:
this build MUST NOT be treated as instrument-qualified by existing fixtures.

CandidateSHA256
`4EFB3DBAF9CA78F3413CA65C6DCB57ABE4F740ABD2E7CBE126A08B0EAF2701BD`.
Releaseopt-s/thinLTO text124976/data1104/bss29624. Firstbuild failed missing
atomicimports; corrected to existing portable_atomic type and rebuildpassed.
Runtimeoverhead, targetstack, framingprovenance and scopefreeze hardwarepaths
remain unverified. No motor/flash. ActualACEE/off; rootELF now4EFB.

Next strictdecoder plus disabledScope tests/timing and emittedcodeaudit before
poweredqualification. Explicitly reject unused/tracefallback/invalid captures;
an empty history is not evidence of no interference. Hook placement broadens
the measured bracket and must remain visible in every result.

## E455 — strict standalone decoder tested

drv_qualification_window.decode validates unique header, bounded counts/
omissions, capturemode, CRC/length on9wordQW85 and4wordQP85, ordinals,
sector/flags, overlap<=calls, finalstoppedrow placement and no pendingdata
after finalcall. Requiredmode rejects unused/quiet data. Orphan/duplicate/
invalid/unfrozen/corrupt records refuse. Four synthetictestsPASS including
16row rollover and stoppedwithoutacceptance. Saturation remains explicit,
not quietly treated as complete statistics.

This decoder only validates the diagnostic substream: it does NOT prove
outputs-off, powered success, correct epoch or guardbudget. The fixture must
combine it with existing campaign verification and explicit opt-in. There
is no retainedhardware QW85 capture yet. Next fixtureflag plus actualdisabled
Scope/completion timing tests before motor. No build/flash/motor inE455;
root4EFB, actualACEE/off unchanged.

## E456 — fixture opt-in and disabled Scope command built

--qualification-window requires trace0 before openingUART, then strictdecoder
after existing motorcampaign verification. Invalid/unused records do not pass.
qualcheck requires bridge_disabled/no poweredowner/no activecore and exercises
actualScope16times each: accept, guardsequenceprogress, stoppedwithoutaccept,
acceptedandstopped, unfinishedDrop. Prints per-mode maxscopeus and passcounts,
resets history afterward. This is not an ISR WCET or physical preemption test.

Candidate B6EF8BF2B341512FB0D6AEE80BE23B899007B7DFEBE6A244557A8FBFD5CD95CA,
release125952/1104/29624. Buildpassed after correcting private safety-helper
access (core_bench::bridge_disabled nowpub(super), implementationunchanged).
378Python testsPASS. NOTFLASHED; actualACEE/off. Next auditELF and run disabled
qualcheck plus existingpreflights before anypoweredcapture. Hostqualcheck
verifier/explicit timing budget remains to add; don't infer qualification from
mere commandavailability. Normalcampaignfeature set still excludes this probe.

## E457 predeclared disabled overhead gate

Scope mode1 (nonaccepting, guardsequence simulated) must<=2us; published
accept/fault and invalidDrop modes must<=4us, all16 semantictrials each pass.
At~20kdispatches/s,2us alone is~4% CPU before acceptpublication/guardhook;
this is a screening bound, not totalCPU/WCET proof or a waivedexistinggate.
ExistingCPU2/7/10 and guardchecks remain mandatory. Failure stops powered
qualification. drv_qualification_check.py retains allrawoutput/finaloff and
verifies both semantics and fixedlimits; limits must not track observed costs.

## E457 hardware result — semantics pass, overhead refuses

InstalledB6EF afterverifiedoff/hashmatch. Automaticmathaudit hashmatches and
no helpercalls attributed to qualification symbols (not wholepathproof).
Bootguard01UARTsilent; retainedsnapshot before08040000repair showsclockoff,
USARTconfigured,ENABLE0/BDTRc1a/CCRs0. Guard02PASS3faults18refusals.
qualwindow_scope01 allfive modes16/16 semanticPASS, maxima6/5/6/6/1us.
Fixedlimits4/2/4/4/4 refuse, as hostverifier correctly reports. Finaloff
verified,portclosed. NO MOTOR command; further poweredpreflights stopped.

Frequentpath5us is materially over2us gate; do not widenlimits or claim a
usable lightweight observer. Next inspect emittedScope/History hotpath for
removablecost, or replace percall work with cheaper sparse evidence. Keep
this rejectedcapture and its semantics separate from timingqualification.
ActualB6EF installed/off; NOTpowerqualified. ExistingACEE captures remain
last powered evidence; restore it before ordinary motorcampaigns if needed.

## E458 — separate publication from frequent path, staged

EmittedB6EF Scope::finish had52localstackbytes and carried publication state
across scalaraccumulation. Extracted only History::publish as inline(never);
all validation, counters, fault/overflow semantics unchanged. Frequent noaccept
returns before publicationcall. Newfinish220bytes vs328, localframe36vs52;
publish116bytes. This does not guarantee overheadgate success or totalstack
improvement on the less frequent publish path.

CandidateF6862A6DE02B4F01AF282A43F3FD2BBE7BC04D303C4F505A2D2B17E0D6F4A92B,
release125960/data1104/bss29624, automaticaudit hashmatches.201lib+34other
RusttestsPASS, existingcompilerwarnings/nonfatalincrementalnotes remain.
NOTFLASHED: actualB6EF/off stillinstalled and notpowerqualified. Next same
disabledqualcheck fixed4/2/4/4/4limits; no motor until allgatespassed. Do not
reclassify old6/5/6/6/1 measurements or infer speedgain from function size.

## E459 — publication split measured, overhead still refuses

Recovered the interrupted check without reflashing or assuming its FINALOFF
marker was readback. No relevant fixture/probe process remained. Read-only
captures/qualsplit_boot_snapshot01.jsonl showed RCC APBENR1=08000000,
configured USART3, ENABLE ODR=0, TIM1 BDTR=0c1a and all CCRs=0. Applied the
previously documented UART-clock write08040000 only after retaining evidence.
qualsplit_guard02.txt passed three timer faults/eighteen post-stop refusals.
probe-rs verify subsequently confirmed flash matches root F686 ELF (full hash
above). No reset/reflash/motor command during this continuation.

qualsplit_scope01.txt: all five modes16/16 semantics pass, maxima6/4/6/6/1us.
Fixed4/2/4/4/4us limits reject this candidate, with exit1 and final outputs-off
readback retained (all six gates/ENABLE/MOE zero, nFAULT1). UART closed.
The frequent-path maximum improved5->4us in this disabled check, but remains
twice its gate; publication modes remain6us. No whole-ISR or powered timing
qualification follows from this. Do not run the probe powered or expand limits.
Next investigate a materially cheaper/sparse instrument rather than another
publication-layout-only tweak. Last powered campaign evidence remains E450.

## E460 — scalar/outlined accumulator rejected; stop micro-rewrites

Tested replacing two guard-sequence inputs with their inequality before History
validation, and outlining Window accumulation. Existing semantic tests plus a
wrapper-consistency test passed (202 library +34 other Rust tests). The new
test was not independent behavioral proof; existing expected-value tests remain
the semantic evidence. Release size125976/1104/29624; linked finish148bytes,
accumulator88bytes, publication116bytes. Automatic arithmetic audit generated.
SHA55BA2CB7CB431BF28414EBA88E11CA7479B15B27CAB34E034B4B1644F101FB02.

Flashed after UART off readback. Boot UART clock-off recurred; retained
qualscalar_boot_snapshot01.jsonl before documented clock repair. ENABLE0,
BDTR0c1a, CCRs0. qualscalar_guard02 passed3/18. qualscalar_scope01 all five
modes16/16 semantic PASS but maxima7/5/7/7/2us reject unchanged4/2/4/4/4.
Final readback off/nFAULT1 verified by fixture before timing refusal; port
closed, no motor. This is slower, not a usable optimization. Removed only
this turn's source experiment and wrapper test. Source is back to E459 form;
root ELF and installed image remain rejected55BA, NOT power-qualified.

Next direction: sparse long-call evidence, not another all-call accumulator
rewrite. Candidate normal path should only timestamp/compare; expensive state
publication occurs for a long completed call. Before implementation specify
the trigger, retention policy, saturation/fault behavior and separate measured
normal/worst costs. Such records cannot prove total rejected-call counts,
uniform absence of guard overlap, pre-entry latency or physical edge timing.
Absence of sparse records must not be presented as absence of a timing fault.

## E461 — sparse contract and pure retention tested

Authoritative matched ACEE captures timelineinline_start61_reentry72_30s_01,
02 and03 report COREEXTI comp_max_us40,40,48 respectively. The last is the
CycleTiming failure. These are whole dispatched-path maxima, not qualified
edge timestamps or proof the48us call caused the failure. A new sparse probe
will select duration>40us OR a stopped call, measured at the same existing
`before` bracket, not reuse the narrower qualification_live bracket. Threshold
is diagnostic selection only: it neither rejects an edge nor changes a guard.

qualification_sparse.rs implements only pure retention, not live hooks. Tail
holds16 selected12-byte rows, host total<=224bytes; selected omissions explicit.
Rows retain absolute accepted-before count,16-bit entry tick, duration, sector,
accept/stop/guard-progress flags. Epoch must be nonzero and match; stale epoch,
invalid metadata, count reversal or overflow invalidate/freeze diagnostics.
The final stopped call is retained even if short; stop completion precedes
freeze. Idle freeze invents no stopped call. No per-normal-call state write.
Five tests plus existing suite pass:206library+34other Rust tests. Existing
nonfatal incremental-cache warnings remain. No embedded build/flash/motor;
installed/root55BA remains rejected/off, source normal recorder E459 unchanged.

Integration contract before powered use:
- Separate opt-in feature/protocol; never label this QW85 or all-call coverage.
- Reuse the existing comparator dispatch start/end timer reads where possible;
  additional selected-call publication must not be hidden from CPU accounting.
- Capture accepted-before, sector and guard sequence at entry; compare after
  completion. Do not obtain supposedly entry metadata after acceptance.
- Match a nonzero epoch across producer/reset/dump. Producer inactive for reset;
  unfinished scoped call invalidates rather than silently disappearing.
- Predeclared disabled incremental overhead limits: normal<=2us, selected/stop
  <=4us. Require existing full CPU preflights too. This is not a new allowance
  for the failed dense recorder. Selected-call frequency is unknown beforehand;
  an unexpectedly dense selection must be reported, not called cheap by design.
- CRC framing must expose threshold, epoch, selected count, omissions and flags;
  quiet output stays post-stop. Associate rows with retained accepted counts.
- Normal calls are NOT counted. No inference of total rejects, overlap rate,
  pre-entry latency, physical ZC time or uniquely unwrapped entry time. A fault
  may originate between calls, so no sparse long calls does not exonerate timing.

Next implement opt-in live reuse of the existing timing bracket, strict host
decoder and disabled budget check. Pure tests do not qualify the hardware cost.

## E462 — live sparse integration; ordinary path meets gate, selected fails

Separate bench-qualification-sparse feature excludes simultaneous dense probe.
Scope receives the existing dispatch start tick and pre-call accepted count,
sector, epoch and guard sequence. End timestamp before publication selects
duration>40 or stopped. A second timer read includes publication in COMP_MAX;
outer IRQ accounting also remains inclusive. Guard sequence is sampled at
Scope entry and selected finish, not exactly at timer reads: overlap is this
slightly different metadata bracket, NOT an exact preemption timestamp.
Unfinished Drop invalidates diagnostics. Reset moved inside observation_reset's
existing interrupt mask for this probe; caller quiescence still needs explicit
integration review across every reset/callback path before powered use.
QS85 packs8u16 and existing snapshot CRC. QUALSPARSE header names threshold,
epoch/count/omission/freeze/invalid/capture and missing normal-call coverage.
No strict sparse-data decoder yet; no powered fixture opt-in yet.

sparsecheck disabled-only executes actual Scope using synthetic duration inputs
(no delay/drive). Five modes16 times: normal, selected accepted, selected guard
progress, short stopped, unfinished Drop. drv_sparse_check.py predeclares
2/4/4/4/4us maxima, validates semantics and finaloff; two synthetic verifier
tests pass. Timings include begin/finish and check branching, not all dispatch
integration loads/clock overhead or hardware WCET. Full CPU preflights required.

C17C5B7496166478DEB1D55C81FCD36B3BE5237A20753A557D2188F70A6B8776,
release125900text/1316data/29344bss; automatic arithmetic audit generated.
Linked Scopefinish256bytes. Flashed after UART off readback. guard01 passed
3faults/18refusals without clock repair. qualsparse_scope01 semantics5x16PASS,
max2/5/5/5/2us: normal meets2us, selected modes exceed4us. Refusal retained;
finaloff readback verified before verifier exit1, UART closed. No motor run.
Installed/root C17C remains NOTpowerqualified. Next reduce selected publication
cost and finish decoder/epoch integration verification, not relax4us gate.

## E463 — strict sparse decoder and CRC-protected row epoch

drv_qualification_sparse.py validates the unique exact header, selected count,
ring omission bounds, flags, threshold, accepted identity monotonicity/overflow,
final-stop position, row length/CRC and epoch consistency. QS85 now10u16:
epoch low/high precede the E462 eight words. Old unprotected-epoch8word rows
are rejected. Optional expected_epoch checks external campaign binding; the
result explicitly reports whether that check was requested. Matching row/header
epochs alone do not prove the producer used the correct campaign epoch.

Empty captured history is valid sparse evidence, NOT proof that any path ran,
all calls were short or no timing fault occurred. Quiet capture cannot satisfy
required mode. Malformed/orphan protocol lines and duplicate headers reject.
The decoder does not verify outputs-off, full campaign lock or event coverage;
powered fixture integration must combine those separate proofs.

Five synthetic sparse-decoder tests pass; all387Python tests pass. Release
7E6FFAA0427B0BAFDC18094921C3C31491C3305EBCA6B5C89F7C75E98F54B5BE,
125876text/1316data/29344bss, automatic arithmetic audit generated. No flash,
motor or other hardware access. Actual C17C/off remains installed and selected
cost5us still fails4us. Next emitted selected-path cost and epoch reset/callback
integration audit; new protocol has not yet produced hardware QS85 records.

## E464 — reset ordering audit and metadata-only epoch reset

Reset call sites: observe_begin stops live COM/COMP and polling before reset;
physical_applied resets at status6 (awaiting forced boundary), before setting
status1; coast_run_inner resets before starting the powered guard and arming
the actual seed. The fresh seed already exists on this last path, so clearing
the entire sparse ring there unnecessarily consumes seed age. observation_reset
already masks interrupts around sparse reset. Foreground cannot resume while a
COMP handler remains in flight; final selected Scope completion runs after its
safing callback, before ISR return. Keep these assumptions scoped to the current
peer-priority powered trace0 path, not arbitrary nested experimental callbacks.

Tail::reset_epoch now resets only epoch/counters/flags; prior192byte row storage
is inaccessible with len0. Test explicitly keeps poisoned old storage unchanged,
checks every old index unavailable, then checks first new row and stale writer
refusal. Total207library+34otherRustPASS. EPOCH now saturates atMAX while invalid
epoch0 is passed after exhaustion; it cannot wrap0 back to1 on a later reset.
Emitted observation_reset contains scalar metadata stores (08007d6e..7c), no
bulk row clear. No measured seed-age improvement claimed.

Important scope limit: freeze_first_segment archives accepted-event evidence,
NOT sparse Tail. A subsequent recovery reset replaces sparse history; the
post-run QS85 dump describes only the final observation epoch. Decoder's
expected_epoch still needs an independently bound fixture/segment identity;
matching CRC row/header epochs is consistency, not that external binding.
Do not infer first-segment sparse coverage from resumed records.

Release9343EA66DBC504308C43AD4FF260428C2C899B265C90C3ECA653280BA2CBBED6,
125924text/1316data/29344bss, automatic arithmetic audit generated. Not flashed;
actualC17C/off still has measured5us selected cost and remains unqualified.
No hardware commands. Remaining immediate work: selected-call cost, external
epoch/segment framing and full disabled CPU preflights before powered diagnosis.

## E465 — cached ordering bound, cold timing unchanged; coverage gap found

Tail stores minimum_accepted established from the last selected row rather
than indexing/unpacking that row on the next push. Same overflow check precedes
bound addition; reset clears the bound. Explicitly mask ring index at store,
matching the private next invariant. Existing reset/ordering tests pass:
207library+34otherRust. Added4staticbytes; finish256->228bytes. This is not
measured time savings.

Release60C970F3BB651E307CC73EAA6E275A2E62C4ADF2C9D6465CFAE15E6B0DAF987F,
125836text/1320data/29344bss, automatic audit generated. Flashed afteroff;
qualsparse_bound_guard01 PASS3/18 without clockrepair. Scope01 semantic5x16
PASS but still2/5/5/5/2us, rejected unchanged2/4/4/4/4. Finaloff verified,
portclosed, no motor. Installed candidate unqualified; no measured gain.

Inspection of the fixture explains a coverage gap: each measured call starts
with a reset empty ring. It never exercises previous-row lookup in the older
build, nor populated/wrapped publication. Thus the cold result cannot assess
the intended lookup saving, and none of these checks is a full selected-path
bound. Next add populated/wrapped cases outside the timing bracket, keep their
selected-cost gate4us, and retain empty-case5us failure. Do not call this a
reason to discard the failure or qualify powered operation.

## E466 — populated/wrapped disabled coverage, timing remains rejected

Expanded sparsecheck to8modes. Existing five remain; mode5 preloads1row then
accepts, mode6 preloads16rows then selects guard-overlap, mode7 preloads32rows
then retains a short stopped call. Preparation is outside timing and its
success/count/omission state is checked. Completion validates final identity,
flags, len and selected omissions (0/1/17 for new cases). Header exposes
preloaded count; host requires all8ordered modes and exact preloads. Three
host verifier testsPASS. Selected gates remain4us; normal2us, drop4us.

Release1ECC1D4A7CB3E578510F25B1EB978D348700DFCF41B3AAB31BF24D4D02C83CF1,
126360text/1320data/29344bss, automatic audit generated. Flashed afteroff.
Boot UART fault retained in qualsparse_pop_boot_snapshot01 (ENABLE0,MOE0,
CCRs0, APBENR1=08000000) before documented clock repair; guard02PASS3/18.
qualsparse_pop_scope01 all8x16semanticPASS, timings2/6/6/6/2/6/6/6us reject
unchanged2/4/4/4/4/4/4/4. Finaloff verified before timing refusal, UARTclosed.
No motor. Actual/root1ECC remains NOTpowerqualified.

The expanded mode expressions add timed harness branches. Thus5->6us is not
proof the unchanged live publication implementation regressed. The result is
still a conservative failure of this check, not permission to ignore cost.
Next isolate actualScope timing from selection/setup harness work, using the
same generated live routine and an independently measured timer-read baseline;
do not subtract an assumed cost or silently reinterpret old results as passes.

## E467 — precomputed test inputs, unchanged refusal

Moved synthetic mode-to-duration/accepted/stop/overlap/drop selection before
timing, with opaque black_box inputs. Shared Scopefinish explicitly noninline
so test and live paths call the same implementation. Timer-read-only baseline
reported separately (SPARSEBASE,16samples,subtracted=0); raw full bracket still
must satisfy2/4/4/4/4/4/4/4us. Host requires baseline presence, no subtraction.
Three verifier testsPASS. This does not make disabled timing a whole-ISR WCET.

Release2D94BAA305F58716B5F5F42812AD1DCA82D0B242CD3937D23684FCAE68E9EB5D,
126620text/1320data/29344bss, automatic audit generated. Flashed afteroff.
qualsparse_inputs_boot_snapshot01 retained clock-off, ENABLE0/MOE0/CCRs0,
then documented UARTclockrepair and guard02PASS3/18. Scope01 semantics8x16
PASS, costs still2/6/6/6/2/6/6/6us reject unchangedlimits. Finaloff verified,
portclosed, no motor. Installed2D94 unqualified. No measured benefit from
input hoisting; stop harness-only tweaks. Selected publication still needs
actual cost reduction or a different bounded diagnostic implementation.

## E468 — bounded UART boot-fault investigation

Repeated silent boots consume every disabled campaign. Inspected explicit
APBENR1 writes in shell/support and HAL enable implementation: shell ORs bit18
with readback/DSB before usart(); HAL enable uses preserving modify. No clock
clear identified on subsequent ADC/TIM17/SYSCFG initialization source path.
This does not exclude indirect/debugger writes or an emitted-code defect.

Added four boot-only volatile APBENR1 snapshots in16bytes UART_BOOT_CLOCKS:
after explicit enable, after HAL, after ADC, after banner/flush. SentinelMAX
marks unreached checkpoints. No powered-loop work/output. Release
A3BDD94C35D8D9678A891649E132EC5A2787FEB9C1FB90F7F16A6AB550FC9F14,
126656text/1336data/29344bss. Symbol200003d4 in THIS ELF only; obtain symbol
from matching ELF before reading another build.

Flashed afteroff. uartbreadcrumbs_guard01 silent. Read-only SWD read of four
words returned00040000 00040000 00000000 00000000: clock present at first two
checkpoints, absent at later two. uartbreadcrumbs_snapshot01 retained current
APBENR1=08000000 and configured UART, ENABLE0/MOE0/CCRs0 before repair.
Documented clock repair then guard02PASS3/18; finaloff verified, UARTclosed.
No motor; actualA3BD remains unqualified sparse diagnostic. No hardware cause
asserted. Next isolate HAL-to-postADC interval/reset-debug interaction using
these breadcrumbs, not blindly add another permanent clock re-enable.

## E469 — emitted interval cleared; debugger writes the clock register

Read A3BD disassembly0800ceea(postHAL snapshot) through0800cfc0(postADC
snapshot). RCC stores atcef0/cef8/cf00 address40021040(APBENR2), not APBENR1
4002103c. Remaining stores target TIM17, ADC, COMP or RAM; intervening delays
are inline loops. No APBENR1 store/callee in this bounded interval.

After UART off readback, performed one reset with RUST_LOG=probe_rs=trace and
--log-file captures/uart_reset_trace01.log. The6MB JSON trace retains an
APBENR1 read_mem_32bit span at11:53:09.190042Z and write command at.190840Z
inside session_drop/debug_core_stop, after run operations. STLink command
[f2,8,3c,10,2,40,4,0,0] is paired with logged write_mem_32bit: address3c100240
little-endian=4002103c. Also writes at debug_device_unlock. Actual four-byte
write payload is not present, so no claim it cleared bit18 on this attempt.

This reset's uart_reset_trace_guard01 passes3/18 withoutrepair and verifiesoff;
logging changes timing, so one passing traced attempt doesn't exonerate a race.
External writer is established; stale read-modify-write racing startup remains
hypothesis, not proven rootcause. Next inspect debugger sequence/source or
controlled reset ordering before changing firmware clocks. No flash/motor or
firmware edit; actualA3BD/off unchanged, sparse probe remains unqualified.

## E471 — inlining helps1us; packed storage regresses and is removed

Current release had Scopefinish100bytes calling Tailpush192bytes through a
temporary Row. Added inline(always) to Tailpush; all checks unchanged. Linked
push disappears, finish228bytes. Candidate
9451A0A7FC2379B6C2E8807768845AFF4F7BA38CE85E7BD3D1E2E9D927E758BA,
126792text/1336data/29344bss. After verifiedoff/download/OpenOCDreset,
qualsparse_inline_guard01PASS3/18 withoutrepair; scope01 semantic8x16PASS,
2/5/5/5/2/5/5/5us. Measured1us improvement but selected4us gate still fails.

Then tested three-u32 packed row storage with unchanged public Row/validation.
Roundtrip flag/boundary test plus suite passed208library+34otherRust tests.
CandidateEAB5B314FA028CEBA63174B04E56AFE9854B31476CF04EFA61EF76D1D851A6BA,
126836/1336/29344. Same off/download/OpenOCDreset; words_guard01PASS3/18
withoutrepair; words_scope01 semantic8x16PASS but2/6/6/6/2/6/7/6us, worse.
Both refusals retained, finaloff verified, UARTclosed, no motor. Removed only
the packed-storage change/test, retaining inlinepush. Source back to inline
form; actual/rootEAB5 remains rejected/off (not rebuilt/reflashed afterward).
Do not repeat packing or claim aligned storage automatically faster on M0.

## E472 — SRAM execution meets disabled cost gates

Removed redundant next/len fields: total selected count determines slot via
mask, len=min(total,16), omissions=total-len. Overflow refusal unchanged;
post-stop row index arithmetic stays within total.207library+34otherRustPASS.
Flash candidateAAF249ED165643E4BDB9E7A5D42543A857F92118474A6A12686DEA9B67FAF861,
126920/1328/29344. count_guard01PASS3/18, count_scope01 still2/5/5/5/2/5/5/5
FAILunchangedlimits; finaloff, no motor. No measured benefit from counters.

Then optional bench-sparse-ram puts only noninline Scopefinish into
.data.sparse_finish. Inspected cortex-m-rt link.x: .data.* is RAM VMA with
FLASH LMA and __sdata/__edata/__sidata startup copy. ELF confirms200byte
function at20000000 within.data20000000..200005f8, LMA0801ef10. No controller
or guard logic moved, validations unchanged. Startup copy/branch-veneer costs
are not inferred away; actual disabled Scope checks execute this routine.

766D148B62E3E2C4B0D5BBFACDE3B59E3759D114543162172496B0370DDB60C5 installed.
.data1528bytes (includes executable function), .bss29344. `size` reports
text128260/data0 because.data is executable; do NOT interpret that as zero
initialized RAM. Automatic mathaudit generated; section markedCODE.
guard01PASS3/18; qualsparse_ram_scope01 semantic8x16PASS with2/4/4/4/2/4/4/4us
against2/4/4/4/4/4/4/4, no baseline subtraction (reportedbaseline0).
All five qualsparse_ram_preflight01 disabled gatesPASS, CPUmax2/7/10.
Both candidate flashes followed verifiedoff and OpenOCD reset, no UARTrepair.
Finaloff verified and serialclosed, no motor command. Installed766D is first
sparse candidate passing these disabled cost/preflight checks, NOTyetpowered
qualified. Strict hardware QS85/segment-epoch binding and fixture opt-in remain
before a powered trace0 diagnosis; fullgoal/current/parity gaps unchanged.

## E473 — hardware QS85 roundtrip passes; new build timing fails

After the timed eight modes, disabled sparsecheck now exercises20accepted calls
and one short stopped refusal through actual Scope, then actual dump encoder.
SPARSEWIRE names expectedepoch/count/omissions independently from QUALSPARSE.
Host checks CRC+epoch plus every field of16retained rows (accepted_before5..20,
entrytick5..20, durations41 except final1, step3 except final4, overlaponly19,
acceptedexcept20, stoppedonly20), omissions5. Four verifier testsPASS.

F33FF7D4901B2D36878654FD9F4A76AB5F0A305F300B236F9B02BFE13B537911 built and
flashed afteroff; OpenOCDreset, wire_guard01PASS3/18 withoutrepair. Actual
wire_scope01 retains valid QS85; independent strictdecode+exactrow assertions
PASS. Combinedfixture correctly fails timing first:3/4/5/4/2/4/5/4us versus
2/4/4/4/4/4/4/4. No claim previous766D qualification transfers to this build.
Finaloff verified, UARTclosed, no motor. New test/report code may change shell
main inlining/frame/call-site costs; that mechanism still requires inspection.
Next isolate disabled check/report implementation from main and remeasure;
do not hide timing refusal behind successful transport or relax gates.

## E474 — outlined check passes sparse+wire, broader CPU gate refuses

Added inline(never) to disabled check only. Before, check had no emitted symbol;
now check1432bytes at08017c60. Live Scopefinish remains200bytes at20000000
(same size is not proof identical bytes). Build
2CA571BA841DF48D8991ECBB7328BE296380F4EA7CF2765D39254244014DC072 installed
afteroff/download/OpenOCDreset. checkfn_guard01PASS3/18 withoutUARTrepair;
checkfn_scope01 passes all8modes2/4/4/4/2/4/4/4 and exact hardware wire roundtrip.
Baseline1us reported, notsubtracted. Sparse+wire are now qualified together for
these disabled checks, not a whole powered execution or worst-case guarantee.

Broader checkfn_preflight01 passes filter, atomic and phase checks, then CPU
fails: mode0 sum372/max2, mode1 sum1792/max7, mode2 sum2604/max11 across256
pairs, fault0.10us maximum gate retained. Archive-routing subsequent stage
not executed. Finaloff readback verified in retained CPUcapture, UARTclosed,
no motor command. Earlier766DCPU2/7/10 cannot qualify this different layout.
Next compare nested-accounting code/call layout; don't erase sparse successes,
ignore CPU refusal, widen its gate or repeatedly rerun unchanged build to pass.

## E475 — isolated CPU check passes alongside sparse/wire

CPU meter check was also inlined into giant shellmain. Marked only this
disabled function inline(never); accounting/critical-section/safety logic
unchanged. Emitted cpu_meter::check976bytes. Candidate
BDE2C4CE4F9C1E6A0A08C637DED65594E63490A49C3AC1D2C01A3619BF56D830 built,
flashed afteroff, OpenOCDreset noUARTrepair. cpufn_guard01PASS3/18;
cpufn_scope01 sparse8modes2/4/4/4/2/4/4/4 and hardwarewirePASS. Fullfive
cpufn_preflight01 gatesPASS: CPUmode maxima2/6/9us, mean1.34375/5.671875/
8.484375, archive routing/refusal3/3 eachof3trials. Finaloff verified, UART
closed, no motor command. The disabled test now uses its own frame/calllayout;
do not claim measured liveIRQ occupancy improvement from this result.

This exact build satisfies disabled prerequisites tested so far. Next powered
fixture opt-in and final observationepoch binding before a bounded72recovery
diagnosis. Host diagnostic parser alone is not fullcampaign/epoch proof; no
powerqualification or completedgoal claimed. Keep E474 CPUfailure retained.

## E476 — final-observation binding and first powered sparse failure

Core saves sparse epoch at observation reset independently of the dump's tail
header. Post-stop SPARSEBIND carries that epoch and final ACCEPTS count plus
RAM-feature provenance. Host --qualification-sparse requires trace0, excludes
dense capture, checks epoch/CRC/row identities and final-count bounds. This
binds only the final observation; no first-segment sparse coverage or physical
edge timing is claimed. Empty capture still cannot prove absence of faults.
390 Python tests pass, including malformed/missing/duplicate/overflow bindings.

Release opt-s/thin-LTO build and automatic hash-matched disassembly audit:
0F9F8A9F6E5BB021045A91AD864762BCBC8E937AF1134C79DA99ABC692819AFD.
.data1528 bytes (executable RAM recorder included), .bss29352. Installed after
verified off, OpenOCD reset. Existing qualsparse_bound_guard01 path refused
before UART was opened; preserved it and used new epoch476 capture names.
qualsparse_epoch476_guard01 PASS3/18; scope01 PASS2/4/4/4/1/4/4/4us,
CRC wire test PASS. Fullfive preflight01 PASS, CPU2/6/9us. No gate changed.

One guarded 30s 7.2% BEMF /6.1% startup /200eHz handoff /phase60 /24k carrier
injected-dropout/reentry diagnostic actually ran on G071+DRV8304H. Existing
11.7V/800mA supply setting assumed unchanged; no new external current reading.
Capture: captures/qualsparse_epoch476_start61_reentry72_30s_01.txt
SHA256 6DFFA54A6149BD62FEE0D8528921CB6914F88A16179FF23962296B4714F183DB.
FAIL, not a sustained pass: recovered then CycleTiming12 at947550us resumed;
1916COM/1915accepted,337.008eHz aggregate,sigma25.845us,rawpeak288,busmin11044mV,
stack untouched2244. Outputs-off independently verified, UART closed.

Strict sparse decode PASS: epoch134, final1915 accepts,3rows,0omissions.
At accepted_before222 and1050:41us,step1,accepted,guard overlap. Final row:
accepted_before1915,57us,step2,notaccepted,stopped,guard overlap. COMPmax60us
includes recorder publication;57us is dispatch body and includes fault safing.
Do not attribute its full duration to persistence or prove preemption from it.

Fault pair: previous3093.5/current2838.5us versus local median2962us:
+131.5/-123.5us, pair residual+4us. Guard delta2850<2858, closure11.5us
(larger than earlier3us; guard overlap/safing brackets need inspection).
This fault is step2, not previously selected step3. Guard overlap also occurred
on successful selected calls; normal calls are unrecorded, so no overlap-rate
or causal contrast can be computed. Next isolate final-call work and existing
guard/accept chronology before a priority change; no hardware blame, threshold
relaxation, higher duty or repeated attempts until passing.

## E477 — the final call passed persistence; acceptance was vetoed by guard

Source audit changes the interpretation of E476's accepted=false. In sibling
minz/core/src/am32_isr.rs interrupt_routine, persistence returns on mismatch;
only after all reads succeed does the reference mask COMP, sample/reset TIM2,
arm COM and call EV_ACC. In core_bench::Obs::record, EV_ACC first calls
powered_timer::accepted. That samples guard time/checks the cycle, then trip
stops TIM6/CPU measurement, cancels other owners/reference and clears bridge.
cycle_refusal_after_safing captures CYCLECORE while still inside EV_ACC. The
later recorder ownership check refuses and skips ACCEPTS increment. E476 also
reports RECORDGUARD late_accepts=1. Thus final sparse accepted=false denotes
no committed log increment, NOT a failed comparator-persistence loop.

The 57us dispatch bracket includes all that post-decision shutdown/snapshot
work; the guard overlap flag spans the call, not just persistence. No exact
57us breakdown can be recovered from this capture. Crucially CYCLEFAULT's
decision_us947512/delta2850 is captured BEFORE trip/safing: shutdown overhead
cannot manufacture that guard cycle. STOP_US947519 is sampled seven us later
before stop(); it is not a physical outputs-off timestamp. The11.5us closure
is the difference of guard/reference endpoint brackets, not measured shutdown
duration. TIM6 max4us,COM commitmax21us,DMAmax24us are whole-run maxima, not
timestamps proving which work occurred at this call.

Added scripts/drv_sparse_fault.py: joins strict epoch/count/CRC sparse decode
to validated CYCLEFAULT/CYCLECORE/tail/off, requires same final sector/count,
and reports reference-persistence success separately from recorder acceptance.
Three tests PASS using retained E476 capture, including missing callback proof
and mismatched final count rejection. No runtime edit/build/flash/motor.
Actual0F9F/off unchanged. Next focus on pre-guard acceptance-time redistribution
and a bounded priority A/B after deadline-slack audit; do not spend another
probe measuring shutdown as though it explained the already-latched fault.

## E479 — sparse coverage shifts investigation one electrical cycle earlier

Extended drv_sparse_fault.py to join the preceding same-sector accepted
boundary (total ACCEPTS minus6, zero-based) and its successor to retained
reference intervals. Baseline is median of earlier same-sector tail entries,
not physical ground truth or a large unbiased population. It separately
checks sparse binding equals full accepted-log total. Four host tests PASS.

E476 identity1909/step2 interval589us then417.5us, versus earlier sector
medians485.5/511: +103.5/-93.5us, pair residual+10us. E478 identity3475/step5
629us then363us versus509.25/481: +119.75/-118us, pair residual+1.75us.
These long/short adjacent intervals occur on the PREVIOUS visit to the
sector that later faults; do not assume the final short cycle began normally
and only its ending acceptance was anomalous. Both endpoint histories matter.

Neither earlier identity has a selected sparse record. E476 has no omissions.
E478 has1105 older omissions, but retained selected suffix starts identity3431,
strictly before3475, with records3474/3476 on either side. Since selected
history is monotonic and suffix-preserving, coverage of3475 is complete.
The host now proves absence only with no omissions OR identity strictly after
the first retained identity; equality cannot exclude omitted same-count calls.

Within this instrument's declared scope, neither accepted boundary involved
a >40us dispatch body at that identity. The ~100us relative displacement is
therefore not evidence of a single ~100us in-call stall. It can still involve
pre-entry delay, multiple short failed/gated qualification visits, or PWM
phase-dependent detection. No physical edge timestamp exists here; neither
causal preemption nor hardware noise nor rotor overspeed is established.
Next inspection/instrument should target the earlier acceptance's qualification
sequence, not the final fault safing cost. ActualF22D/off unchanged; no runtime
edits/build/flash/motor. Do not convert this into a guard-threshold relaxation.
