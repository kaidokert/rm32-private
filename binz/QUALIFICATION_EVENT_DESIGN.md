# E490 — qualification summaries per accepted boundary

Current status: E491 below stages the opt-in live adapter; it is NOT flashed
or timing-qualified. The E490 implementation notes describe the earlier state.

Pure accumulator implemented in `examples/support/qualification_event.rs` and
included in the host replay crate only. Five new tests pass; full Rust suite
213 library +34 other tests passes. No firmware build, flash, UART or motor
command. Actual C631 and its last verified-off state remain unchanged.

Purpose: explain the earlier accepted boundary implicated by E479/E489, rather
than retain only a costly final-dispatch or shutdown trace. Publish one summary
per accepted event, retaining at least 16 summaries in the future adapter
(more than two electrical cycles), instead of a ring row per dispatched call.
This reduces publication frequency, not necessarily total measured overhead.

Implemented semantics:

- Five distinct counters reuse `comp_paths::Path`: no gate read, closed gate,
  open/no acceptance, accepted, stopped/unknown. The last is never relabelled
  as a failed persistence test; E477 proves why that would be incorrect.
- First/last open-path interval-count observations and explicit presence bit;
  count65535 is a valid observation, not a missing-value sentinel.
- Row carries the accepted-before identity and sector. Successful acceptance
  closes the bucket; the next bucket must advance to the next sector.
- Changed sector inside a bucket, missing/unexpected count, and counter or
  identity overflow freeze and invalidate the accumulator. No silent wraps.
- A stopped dispatched call publishes an explicitly stopped/unknown bucket.
  An external freeze leaves a partial bucket, not a fabricated acceptance.
- No peripheral reads, timestamps, allocations, divisions, output authority,
  or edits to the reused controller. Host layout is <=24 bytes per row and
  <=32 bytes for the accumulator; target codegen and execution cost unmeasured.

Live integration contract (not implemented yet):

1. Reuse `comp_path_live`'s first **actual** Interval::count hook and its
   classification after the reference call. Do not add comparator reads or
   turn on trace1; preserve StaticComp and quiet-IRQ-stamp behavior.
2. Only collect the powered observation epoch. Reset before sensing is active,
   bind rows to that epoch and the recorder's accepted-before identity, and
   check final accepted count. Reset after recovery must never alias the first
   segment. Dump a partial bucket distinctly from completed rows.
3. Keep stopped-callback publication possible after safing without reopening
   ownership or resetting the accumulator under a live Scope. Scope early
   returns need explicit completion/invalidation, including critical refusal.
4. Dispatcher-skipped calls are outside `comp_path_live`: retain existing skip
   counts and label coverage dispatched-only. No-gate is not a skipped call.
5. Use a bounded power-of-two suffix ring with total/omission count and sticky
   invalid state. Post-stop fixture-only CRC framing must include epoch and
   accepted identities; terminal defaults remain quiet.
6. Before motor use, measure the whole begin/first-count/finish bracket,
   including populated/wrapped publication, and early-stop paths. Initial
   added-cost gates: <=2 us ordinary nonpublication and <=4 us publication;
   retain existing <=10 us nested CPU preflight. These are experiment gates,
   not claims the new code meets them. Do not subtract assumed harness cost.
   Audit release-s/LTO emitted code and stack, then disabled wire/semantic tests.

This summary loses the sequence and timestamps of individual rejected visits.
It can distinguish many open/no-accept visits from gate waiting, but cannot
measure physical-edge arrival, pre-entry delay, or identify a preemptor. A
first-to-last count difference is a reference-counter span, not ISR residency.
Its purpose is to choose the next causal test with enough accepted-event
history, not to certify lock, change a filter or relax a guard.

## E491 — bounded history and staged live adapter

Added 16-row suffix History with metadata-only reset, accepted-count binding,
epoch, total/omission counts, explicit partial bucket, and sticky invalid/frozen
state. Two additional tests exercise 40 accepts/wrap, terminal stopped row,
reset invisibility, zero epoch and wrong accepted identity. Full Rust suite
215 library +34 other tests PASS. Host History layout <=448 bytes.

Opt-in `bench-qualification-event` implies static-comp and comp-paths. It adds
powered/sector context to the existing dispatched scope, consumes the existing
first actual interval read and path classification, and accumulates after the
reference returns. No extra comparator read or physical timer read. Drop now
completes this feature's abandoned scope as unknown/stopped. The ordinary
non-feature path retains its old behavior.

Observation reset advances a boot-local nonwrapping epoch and resets history
with interrupts masked. Exhaustion invalidates rather than wraps. Guard safing
does not reset it, so the final callback can publish after safing. Post-stop
fixture-only `QUALEVENT` header includes final actual ACCEPTS; QE85 frames
CRC-protect epoch, accepted-before, five path counts, first/last open counts,
sector/presence/stopped flags and a partial-frame flag. The last frame is always
partial (may be empty or duplicate the stopped bucket), never another accept.
Only the final observation epoch is dumped; first-segment archive support is
NOT implemented for this diagnostic.

Release-s/thin-LTO candidate root ELF SHA256:
`63698283C0E82CDF3EE95F48C09723FDC2D0B7DD667770FDF93C2A460775C2E8`.
Features: E488 C631 set plus `bench-qualification-event`. Size text127232,
data1144, bss29764 bytes. Advisory emitted math audit SHA matches; no reported
soft-arithmetic helper sites attributed to new history/Scope code. Scope
complete reserves68 stack bytes in this build; it is not a free diagnostic.
No runtime speed or stack high-water result exists for it. Initial9484 build
was superseded by this header's explicit final-ACCEPTS binding, neither flashed.

Still required before power: strict host decoder/provenance integration,
disabled actual-Scope timing at empty/populated/wrapped states, stopped/Drop
checks, wire CRC hardware checks and existing full CPU/guard/ADC/role/archive
preflights. Keep <=2/4us experiment cost gates and <=10us CPU gate. Current
emitted code inspection is not a substitute for these measurements. Trace1
and unrelated instrumentation combinations are not qualified.

Actual installed firmware remains C631, last verified OFF in E488; no UART,
flash or powered command in E491. The running-cycle cause remains unresolved.

## E492 — strict host decoder

Added `scripts/drv_qualification_event.py` and four synthetic protocol tests.
All406 Python tests PASS. Decoder requires one exact header, n+1 CRC frames
(n complete rows and one partial), consistent epoch/count/omission geometry,
accepted identities, sector progression, counter/flag consistency and final
accepted-count closure. Optional expected epoch/count inputs are checked and
their presence explicitly reported; self-consistency alone is not an external
campaign binding. Malformed or missing required data raises an error.

A stopped/unknown terminal row permits final ACCEPTS equal to its identity
or identity+1: stopping may happen after acceptance, and path classification
alone cannot settle persistence success. Its partial frame must exactly
duplicate the stopped bucket with only the partial flag changed. Ordinary
partial buckets cannot masquerade as published acceptances. No physical edge
timestamp, lock verdict, or outputs-off proof is inferred from this substream.

Tests cover valid/empty data, explicit binding mismatch, wrapped suffix,
stopped callback count ambiguity, corrupt CRC, duplicate/malformed header,
bad flags, counter semantics and sector order. Initial test failure exposed
a tuple/list comparison in decoder partial validation; corrected before any
hardware use. No host fixture integration or hardware QE85 verification yet.
Disabled actual-Scope semantic/cost test remains the next implementation step.
Firmware/root6369 and actual installed C631 are unchanged; no UART or motor.

## E493 — actual Scope hardware check FAILS cost gates

Added idle-only noninline `qeventcheck`, exercising begin/context/first-count/
finish and Drop with no gate authority. Six modes (closed, open/no-accept,
accepted, stopped, Drop, no-gate) each run16 times with0/16/32 prior accepts
prepared outside the timing bracket. First-count called twice proves only the
first is retained. Semantics include row identity, counts, total/omissions,
freeze/invalid state and inactive Scope. The bracket includes harness branches;
no presumed baseline subtraction is made.

Release-s/thin-LTO actual/root ELF now:
`43182426F358632BD405A9A082CACE97C3870E283194B47587F3960F4B68CD01`.
Text128800/data1144/bss29764 bytes. Emitted math audit SHA matches.
Preflash off verified, download and OpenOCD reset succeed; no UART repair.
`captures/qevent_493_guard01.txt` passes3 timer faults /18 post-stop refusals,
SHA256 `411BA5E34A87C33ACBFAD053A43974771810BEC12D72E318DEB7DF2DD5285E75`.

`captures/qevent_493_scope01.txt`, SHA256
`5EB0429D897F7DE3ACB6C742FF62F5CFA831A460AAE98C9886ED8CD161137182`:

| Preloaded accepts | Closed | Open/no accept | Accepted | Stopped | Drop | No gate |
|---|---:|---:|---:|---:|---:|---:|
| 0 | 7 | 7 | 13 | 11 | 10 | 7 |
| 16 | 7 | 7 | 13 | 11 | 10 | 7 |
| 32 | 7 | 8 | 13 | 11 | 10 | 7 |

All288 semantic trials pass; all18 timing cases FAIL unchanged2us ordinary /
4us publication gates. Values are measured bracket maxima in microseconds,
not powered ISR WCET. No motor command or broader preflight was run after this
failure. Finaloff verified, UART closed. Installed diagnostic is NOT qualified
for powered use. Strict host cost verifier rejects this retained capture; do
not treat semantic success as permission to run it.

Next: reduce actual Scope/classification/accumulator work and publication cost
from emitted code, or replace the observer approach. Current high-frequency
cost is already unacceptable before ring publication. Changing only ring size
or dumping less often will not fix it. Keep static comparator sampling, all
motor guards and the cost gates intact. Hardware QE85 framing and powered
diagnosis remain unverified; none of this resolves the running-cycle fault.

## E494 — hot metadata layout saves roughly1us, still rejected

Emitted E493 History::visit used synthesized offsets around392..418 bytes
for frequently touched fields, because default Rust layout placed ring storage
ahead of accumulator state. History now uses repr(C), with accumulator and
metadata before the rows. No semantic or guard change. New host offset test;
216 library+34 other Rust tests PASS. History::visit shrinks468->368 bytes.

Actual/root release-s/thin-LTO ELF:
`7BF3399998E92A6CEACE4187B3686BC9D2E5762ED9F2D00A212153DCE4809E2D`.
Text128604/data1144/bss29764. Preflash off verified; download/OpenOCD reset
succeed. Guard3/18 PASS without UART repair, capture qevent_494_guard01 SHA256
`B06B08A9B90D0448DE807877A08E34C0111955F05BF09578CB6DF2F4419ECFCC`.

qevent_494_scope01 SHA256
`939B7B8D2C894E99B057C6ECEE3C4EA25E9DCF74911374DDC5EA3E1EF7196791`:
all288 semantic trials pass. Maxima for closed/open/accepted/stopped/Drop/no-gate:
preload0 =6/7/12/10/9/6us; preload16 and32 =6/6/12/10/9/6us.
Still FAIL every2/4us gate. No powered command or broader preflight follows.
Finaloff verified by fixture, UART closed. Installed image remains unsuitable
for powered diagnostics; previous failure not erased.

This isolates one avoidable M0 address-generation cost but does not close the
large frequent-path gap. Next simplify collection/publication architecture;
do not spend a sequence of motor trials or ring-layout variants on a diagnostic
that is still three times its ordinary-path limit. Preserve semantics and
measure before claiming a cheaper replacement.

## E495 — outlined publication does not qualify the observer

Split accumulator into a boolean-returning accumulation step and terminal row
completion; History::publish is noninline. Public Accumulator::visit retains
its semantics for host tests. History::visit now160 bytes with4 local stack
bytes; publication248 bytes. This removes row temporaries from ordinary calls,
but measurements below show no useful worst-case ordinary-path improvement.
216 library+34 other Rust tests PASS. Source split retained as explicit paths,
not as a qualified performance win.

Actual/root release-s/thin-LTO ELF:
`95FBAD72999201F49B0AA991FD7F8A9BD5651EF4E567B6253CFA172C889C2E44`.
Text128644/data1144/bss29764. Preflash off, download, OpenOCD reset and guard3/18
PASS; no UART repair. Guard capture qevent_495_guard01 SHA256
`411BA5E34A87C33ACBFAD053A43974771810BEC12D72E318DEB7DF2DD5285E75`.
Scope capture qevent_495_scope01 SHA256
`352EE263EE5D59580C77FE38BAEE1F117E52210CF098AA3AC789E47A019D32B9`.

All288 semantics pass. Closed/open/accepted/stopped/Drop/no-gate maxima:
preload0=5/6/13/10/10/6us; preload16=6/6/13/10/10/5us;
preload32=6/6/13/10/10/6us. Still FAIL2/4us gates; accepted publication is1us
worse than E494. No motor/fullpreflight. Finaloff verified, UART closed.

This is evidence against continuing to tune the extra Scope/classification/
general-purpose accumulator composition. Next alternative should count at the
existing first actual interval-read and acceptance hooks, with minimal state,
instead of a second per-dispatch classified-visit interface. Such a redesign
must specify lost coverage, stop/partial semantics and overflow/epoch binding,
and pass its own tests/timing. It is not implemented by E495 and must not
silently reuse QE85 semantics if the collected quantities differ. No guard
change, physical-cause verdict or operating-envelope progress claimed here.
