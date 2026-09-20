# Scheduling chronology probe - E394, 2026-09-14

## E398 packed overlap: negligible gain; park this probe

Folded overlap into unused bits0/7 of the existing nesting mask. IDs1..6
remain bits1..6; COMP entry clears evidence, leaves clear only active IRQ bits.
No separate byte or additional mask store.218Rust testsPASS including nesting,
stop/freeze/re-entry; release+autoauditPASS text122372/data1104/bss29324.
CandidateFA4762A54749F0468C4B81C7142912F1366E598099C16554205126526ADB75D4.
Disabledcpu01 silentUART/exactsaferegs/knownclockrepair;02 max2/7/11us,
nested2616/256=10.21875us vs previous10.28125us. Gate remains10us; FAIL.
No motor. Restored9789, packed_restorecpu01PASS2/7/10/noUARTrepair/finaloff.
Do not keep iterating tiny overlap probes. Next assess a controlled priority
A/B using the existing timing instruments, with guard-deadline/starvation
analysis and disabled behavior checks FIRST. Same-priority pending ordering
can starve a guard too; merely promoting COMP to0 is not a safety proof.

## E397 compact stop-context alternative also rejected on overhead

bench-comp-overlap uses existing meter nesting mask: COMP entry clears one
byte; nested guard/DMA entry sets bits2/64. No extra clocks/history/per-read
changes. Valid stopped meter reports only the current COMP handler, not prior
rejections or preentry latency.218Rust/330Python testsPASS. E283F287 release
text122396/data1104/bss29328, disabledoverlap_cpu01 measured2/7/11us, so FAIL
unchanged10us gate despite much lower cost than fulltail26us. No poweredrun.
Restored9789; restorecpu01 silentUART/exactsaferegisterclockrepair,02PASS2/7/10
with finaloff. RootE283 notinstalled. Next codegen audit of this small added
path or a different causal experiment; no gate relaxation/fulltail retry.

## E396 hardware overhead rejection - do not use for motor tests

Final candidate97A69B440DC62A13AB1CF956C613908343ED4BE4112F3B1D3222C63F6BB46ED2
adds required CPUCHECKTAIL provenance. Host --scheduling-tail requires the
observer and agrees with CPU elapsed/count/stopped-depth (count=2calls-depth+1).
328Python testsPASS before hardware. Release-s/thinLTO/autoauditPASS,
text123356/data1104/bss29860; dump_tail local100bytes, enter/leave local12,
stamp local4 (each has register saves; these are not whole-path stack bounds).

Preflashoff verified. cpu01 silent UART retained; exact RCC08000000/PD1zero/
BDTRc1a/CCRs0 then documented UARTclock08040000 repair. cpu02 actually runs
the recorder and FAILS: inactive max2us, single-pair15us, nested-pair26us
(mean26us), all diagnosticfault0. Existing max10us gate unchanged. Finaloff
verified, no motor command. Restored archived9789; restorecpu01 PASS2/7/10
without UARTrepair, finaloff. Root ELF remains97A69 but is NOT installed.

This rejects full timestamped chronology at every boundary, not the scheduling
hypothesis. Do not spend repeated powered trials on an unqualified observer.
Next lower-cost option: retain only nested guard/DMA presence during each COMP
body, preferably using already-serialized CPU nesting state and no additional
timer reads. Associate that compact evidence with accepted/refused decisions.
It would answer overlap, not duration or physical edge latency; overhead and
fault-path provenance still need qualification. Not implemented yet.

## E395 optional integration (not flashed)

Feature bench-scheduling-tail depends on bench-cpu-union. CPU meter serializes
entry/exit and shares one current TIM17 sample with both instruments; nested
boundaries add clock reads intentionally. First powered guard begins both at
the same timestamp. Reset/finish follow existing meter lifetime including
recovery. CPUCHECK starts the tail on its synthetic active cases, exercises
the new path and includes tail faults in its refusal result. Original normal
feature path remains unchanged. No comparator/core/guard modifications.

Post-stop SCHEDTAIL/ST85 dumps copy metadata and one8byte row at a time, not
the full array. Strict host decoder checks CRC, count/omission, stack uniqueness,
entry/exit continuity, timestamp order, frozen stop and provenance. A diagnostic
fault is retained but cannot be a required observer pass.327Python testsPASS.
Release-s/thinLTO build and automatic arithmetic auditPASS; no helper calls
attributed to scheduling_tail. Candidate78846172C07BACC81E6E06F8173123B20F72A4E1486D9989EE36A01A6B7C20D4
is NOT installed. Actual9789 archived at captures/reference/binmath_9789.
Next add fixture-required option/CPU crosscheck and review stack/codegen, then
disabled overhead measurement. No claim of observer cost or powered validity.

The original design status below describes E394, before this integration.

Status: pure recorder and host tests ONLY; not wired into firmware, not
flashed. Actual/root ELF remains9789F836, last hardware event E393 finaloff.
No measured observer cost, no new explanation for the cycle refusal.

E393 recovered at6.9% then refused a3012us same-sector cycle against3031us.
Arithmetic reduced wall costs but did not remove that fault. Existing I85
trace cannot identify nested guard/DMA executions and switches comparator
adapter when enabled. A separate IRQ-boundary record avoids per-read changes.

## Recorder contract implemented

examples/support/scheduling_tail.rs stores64 eight-byte records. Each contains
elapsed microseconds plus packed event kind/vector/post-transition nesting
stack. Total storage is bounded to544bytes by test. Ring indexing uses a
power-of-two mask, never division. All arithmetic is32bits or smaller.
Vector IDs match cpu_meter: guard1, COMP2, COM3, driven4, polling5, DMA6.

Every entry/exit has its own timestamp, including nested boundaries. Stop
records the still-active stack and freezes it; later RAII exits or a second
begin cannot mutate the capture. Invalid nesting, observed gaps above1000us,
600s elapsed overflow and record-count overflow freeze with explicit fault.
Omission count is explicit; a leading partial handler is not invented. Each
row carries its nesting context even if the corresponding entry was evicted.

TIM17 wrapping accumulation has the SAME essential limitation as the current
clock: at least one sample per65.536ms is required. A whole-wrap blackout is
not detectable from a16bit source. The observed-gap gate does not prove that
condition. The unchanged independent motor guards remain safety authority.

Four tests cover nested guard/DMA within COMP, stop inside nesting, late
callbacks, ring truncation/order/layout, malformed transitions, arithmetic
limits, full six-level nesting and400001 records over10s/repeated timer wraps.
Complete host replay suite:217testsPASS. No control/core/reference changes.

## Integration obligations before a powered trial

1. Optional feature only. Reuse cpu_meter's existing serialized boundaries;
   do not modify StaticComp or enable trace1. Existing union meter deliberately
   skips nested clock reads: chronology needs those additional reads, so it
   is NOT a zero-cost reuse. Share an outer timestamp where possible without
   changing accounting semantics; never feed an old timestamp as current.
2. Reset alongside cpu_meter::reset before acquisition; start at the first
   powered guard entry, recording the clock origin. Freeze through the existing
   powered_timer::stop -> cpu_meter::finish path, including stop inside an IRQ.
   A frozen first segment must not be mislabeled as the recovered segment.
3. Expose records only in post-stop fixture output with CRC, explicit origin,
   nesting/omission/fault metadata and strict decoder/provenance. Preserve raw
   capture on failure. Do not copy the whole recorder onto the powered stack.
4. Measure actual compiled SRAM/stack and emitted arithmetic. Existing disabled
   CPUCHECK must exercise the added boundary work; keep its10us maximum gate.
   If cost fails, do not spin or loosen the instrumentation gate. Guard fault
   injection/restoration checks must pass before a preceding-point powered A/B.
5. Qualify at6.8% before the6.9% failing recovery. Report observer overhead and
   compare the normal adapter unchanged. A changed-build failure alone is not
   retrospective proof of the E393 cause.

## What this can and cannot establish

Nested entry/exit order can show which instrumented handler interrupted a
COMP handler and its software-boundary wall duration.64 records cover only
a short, traffic-dependent tail, not necessarily a full electrical cycle.
Absence in an omitted prefix is not evidence of absence. Further linkage to
accept/refuse decisions may be required; don't claim the whole handler is the
persistence loop. This does not measure physical comparator-edge time,
hardware exception latency, foreground PRIMASK intervals or peripheral event
time. A delayed COMP entry could have no nested handler at all. Those limits
must survive every report; this is chronology, not an automatic causal verdict.
