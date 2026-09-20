# E496 — direct-hook counters

Pure qualification_counts::Counter implemented, not live integrated. Four new
tests;220 library+34 other Rust tests PASS. No build/flash/UART/motor; actual95FB
remains last verified OFF and unqualified for powered diagnostics.

Changed quantities, requiring a new feature/wire format rather than QE85:
dispatched visits, first actual reads with gate closed/open, first/last open
reference counts. Dispatched minus closed minus open means no first read yet,
not necessarily an anomalous call. Open minus one is NOT universally a count
of failed persistence tests. Only independently committed acceptance may take
a bucket; a veto/stop retains partial counters without fabricating an event.

Host state <=16bytes. Begin checks total overflow and clears first-read latch;
one bin increments on the first read only. Bins are bounded by checked total,
so separate bin-overflow branches are unnecessary. Tests cover65535 visits,
duplicate reads, strict gate boundary, missing gate reads, bad acceptance,
partial stop and sticky overflow. Acceptance clears counts but leaves seen=true
until next dispatch. Source/state size is not measured runtime performance.

Live integration still required:

- Separate opt-in feature, excluding comp-paths and qualification-event overhead.
- Begin after dispatcher refusal checks in powered reference mode only; first
  actual Interval::count hook, no added peripheral or comparator reads.
- Supply half-average consistent with the actual reference gate comparison.
  Audit COM/foreground writes and priority/serialization before caching it.
- Publish only at actual accepted-log commit after guard/ownership checks,
  with epoch/identity/sector. Final guard veto stays partial, not accepted.
- Retain16-row suffix and omissions, nonwrapping epoch, strict CRC decoder,
  post-stop fixture-only output. Explicitly label final-epoch-only coverage.
- Stop can preempt first-read collection: never reset storage under a live
  call; revoke mutation or defer snapshot until unwind. Timer reads outside
  COMP must not enter the collector. Active-dispatch ownership is still needed.
- Measure full begin/first-read/end and accepted publication with full/wrapped
  buffers. Retain2us ordinary/4us publication and10us nested CPU gates. Host
  tests are not live preemption, wire, output-safety or M0 cost qualification.

No controller, motor guard, speed envelope or physical-cause verdict changed.

## E497 — staged direct live hooks, not flashed

Added `bench-qualification-direct` (static comparator and peer COM required),
mutually exclusive with comp-paths/QE85 at compile time. New live adapter owns
a 16-row packed suffix, nonwrapping epoch, accepted identity/sector validation,
partial counter snapshot and separate `QUALDIRECT`/QD85-v1 framing.

Begin runs after dispatcher refusal checks and snapshots half-average. The
first actual Interval::count hook supplies the real count; later reads in the
same call do not count twice. Setup uses min(half-average,65535), equivalent
for the16-bit hardware count even if the threshold is larger. No comparator
read is added. Peer COM cannot preempt COMP; foreground and lower-priority
polling cannot change average mid-call, and higher-priority safety handlers
stop rather than update the estimator. This rationale applies to this chosen
configuration, not arbitrary future priority changes.

Publication is inside the accepted recorder's existing critical section,
after guard and recorder-ownership vetoes, immediately before ACCEPTS increment.
A guard-refused reference event cannot publish here. Counter storage is owned
by the COMP dispatch; guard safing does not reset/freeze it under a mutable
reference. An interrupted hook may finish after safing, but final partial
snapshot is taken only after dispatch unwind. Label: partial_until_unwind=1,
not a precise shutdown timestamp. RAII clears collection on return, including
early returns. The normal trace0 path explicitly drops collection before the
COMP_MAX endpoint. Trace1 combinations remain unqualified.

QD85-v1 carries epoch(2u16), accepted identity(2), sector, dispatched/closed/open,
first/last open counts, partial flag:11 words with existing CRC. Partial sector
is0 (bucket has no independently tracked physical-sector history); complete
rows have1..6. No unknown-stop row or persistence-outcome claim from QE85 is
carried over. Final header binds actual ACCEPTS. Terminal defaults stay quiet.

Release-s/thin-LTO root ELF:
`4E1CB559292D624D018589A356236796A6F5571223DAF99A84BCFEB9E8FA67C0`.
Text126124/data1136/bss29608. Features: E488 lean20k masked-seed set plus
bench-qualification-direct, without bench-qualification-event/comp-paths.
Build PASS after correcting Rust2024 formatting references to mutable statics
by snapshotting scalar header fields. No flash/UART/motor; actual95FB remains
last verified OFF and unsuitable for powered diagnostics.

Still required: strict QD85 decoder/fixture binding, disabled real-hook cost
and wire tests with full/wrapped suffix and post-veto partial data, target
codegen/stack audit, full guard/CPU/ADC/role/archive preflights. No timing gain
or live capture fidelity is established by this build.

## E498 — QD85 host decoding

Added drv_qualification_direct.py and four tests; all412 Python tests PASS.
Strict exact header, CRC/word count, epoch, suffix geometry, accepted identity,
sector progression, partial flag and closed+open<=dispatched checks. Committed
rows require an open observation; partial row is sector0 and may retain open
observations without granting acceptance. Open presence agrees with first/last
counts. Final accepted count must equal committed total, unlike unknown-stop
QE85 semantics. Missing requested data and malformed protocol lines reject.

Expected epoch/accepted inputs provide optional external binding, explicitly
reported. Self-consistent substream alone does not establish campaign identity,
outputs-off, lock or hardware timing. No-first-read is derived only as count
difference; no persistence rejection count or physical-edge delay is reported.
Tests cover empty capture, wrapped16-row suffix, unfinished open bucket,
external mismatches, CRC corruption, invalid flags/counters and sector gaps.

No firmware build/flash/UART/motor. Root4E1C staged, actual95FB remains last
verified OFF/unqualified. Next disabled actual-hook harness and hardware wire
checks before powered fixture integration; retain original cost/fullpreflight
gates. Decoder synthetic success does not satisfy these missing checks.

## E499 — real direct-hook costs and wire check

Added idle-only directcheck and strict host verifier. Actual begin/first-count/
accepted/drop hooks tested at0/16/32 preloaded accepts; five modes x16 trials.
Preloading is outside the bracket. A separate40-accept synthetic sequence
exercises wrapped suffix serialization plus one uncommitted open bucket.
No reference commutation ISR or motor output authority in this check.

Actual/root release-s/thin-LTO ELF:
`74E824C4BCB9885FFDA82D2FAD896D509004F5C2ACA297A42BEF174C225377A0`.
Text128048/data1136/bss29608. Preflash off verified, download/OpenOCD reset
PASS without UART repair. Guard3/18 PASS, capture direct_499_guard01 SHA256
`4F41953BA9C4E90B2AABC978A3335EF04D54F08AF9B788F20B70EEC2041D3FDE`.

direct_499_scope01 SHA256
`BDF4C75657EC17DAEF1D22731854B6F9E04E6ECE10E3ECD7865C32CD6B9BCC0F`:
all240 semantic cases PASS. Closed/open/accepted/no-gate/disabled maxima are
3/3/7/2/2us for all three preload sizes. Wire PASS: epoch241,16 retained rows,
40 accepted,24 omitted, identities24..39, expected sectors and count600;
partial identity40/open1/count777. Full CRC and finaloff verified; UART closed.

Still FAIL2us ordinary/4us publication gates. No motor or broader preflight.
Compared with E495 Scope path this is a substantial observer-cost reduction,
but not a qualified diagnostic or controller-speed improvement. Different
collected quantities are explicit; do not call this an equivalent QE85 trace.
Added verifier regression test retains hardware failure and tests synthetic
boundary costs/corruption (edited costs are not new measurements).

Next inspect direct first-count/publication emitted code and remove remaining
cost without weakening limits. Real powered epoch binding, guard-veto partial
coverage and fullpreflights are still required before investigating the motor
fault with this probe. Installed74E8 remains OFF and NOT powered-qualified.

## E500 — RAM helper execution improves publication, still rejected

Opt-in bench-direct-ram puts the same noninline begin88bytes and accepted216bytes
in .data startup-copy RAM at20000000/20000058. First-count hook remains inline
in its caller. No counter, validation, wire or guard semantics changed.
Actual/root release-s/thin-LTO ELF:
`06AB830F4849E49A4BFD26D07E8C41D9051497573D10C74B44BADEAA03E72CEF`.
.data1440bytes (includes304bytes executable helpers), bss29608; do not report
executable data as zero initialized RAM from arm-size's text classification.

Preflashoff/download/OpenOCD reset and guard3/18 PASS, no UART repair.
direct_500_guard01 SHA256
`A6B3414AE0BC124C0CD72A18C399B618E5F4BC4C5DF936546E89ECC9CE6D7634`.
direct_500_scope01 SHA256
`95653C73588804A3C86293AB0E130730F107CF8B41532626A9B81EF1D0D5796B`.
All240 semantics and wrapped/partial wire checks PASS. Closed/open/accepted/
no-gate/disabled maxima: preload0=3/3/5/2/2us, preload16=2/3/6/2/2us,
preload32=3/3/6/2/2us. Still FAIL2/4 gates. No motor/fullpreflight.
Finaloff verified, UART closed. Installed06AB is NOT powered-qualified.

Publication improvement7->5..6us establishes fetch/location cost as a lever,
not a complete solution. Ordinary gate-count path still reaches3us; its inline
first-count work and the complete bracket remain to be examined. No powered
speed benefit or full stack margin claim from this disabled experiment.

## E501 — out-of-line RAM first-read regresses; source reverted

Tested first_count as a76-byte noninline RAM helper in addition to existing
RAM begin/accepted. Actual/root ELF:
`EB6562FCC02A812834E1B8D4BCE49436C9F6BAB4B09CFC51A77801D747C9F700`.
Preflashoff, download/OpenOCD reset and guard3/18 PASS, no UART repair.
direct_501_guard01 SHA256
`A6B3414AE0BC124C0CD72A18C399B618E5F4BC4C5DF936546E89ECC9CE6D7634`.
direct_501_scope01 SHA256
`78BD845F3C84B05BE6A6E1E1E75D929E6DF4B708BBCA3716BF3E8A2BC4C00A54`.
All240 semantic cases and wrapped/partial QD85 wire PASS, but maxima are
4/4/6/2/3us for closed/open/accepted/no-gate/disabled at all preload sizes.
Still FAIL2/4 gates and ordinary path is worse than E500. No motor/fullpreflight.
Finaloff verified, UART closed.

Source first_count attribute reverted to inline; RAM begin/accepted retained.
No rebuild/reflash after this revert: source is E500-style, actual/root EB65
remain the rejected test image. The measured result rejects this out-of-line
placement; it does not prove RAM execution itself is slower (call/inlining
and placement changed together). Do not repeat this variant or call it a gain.
Next inspect the remaining collection/validation operations and publication
packing; neither more RAM moves nor relaxed timing gates are justified here.

## E502 — word-aligned storage, still one reported tick over each limit

Changed retained storage from eight u16 to four u32 words; same256 bytes,
now word-aligned (20005fd8 in this ELF). Post-stop dump unpacks the identical
11-word QD85 protocol. First-count remains inline (E501 revert), begin and
accepted remain in RAM. Acceptance helper220bytes. No validation/guard changes.

Actual/root release-s/thin-LTO ELF:
`8ED4A603F9B60CD606078D74CFC45EDED234EEFA6558AB7345E59ECC23935E5C`.
Preflashoff/download/OpenOCD reset and guard3/18 PASS, no UART repair.
direct_502_guard01 SHA256
`905C97D586B05902A919593E23137CEAB0670592359CA50F9E3D2393958F72D8`.
direct_502_scope01 SHA256
`7EB75AB0A65C92199B8454DFA36F65435A697D3FDF2593DDA39176E390935650`.
All240 semantics and actual wrapped/partial CRC wire PASS. Maxima
closed/open/accepted/no-gate/disabled: preload0=3/2/5/1/1us;
preload16 and32=3/3/5/2/2us. Still FAIL2/4 gates. No motor/fullpreflight.
Finaloff verified, UART closed; installed8ED4 is NOT powered-qualified.

The mixed table is a quantized bracket measurement, not evidence of a special
fast operating mode. Accepted publication is now5us at every preload (E500
was5..6), but ordinary worst case remains3us. One timer tick over is still a
failure. Further work must address actual remaining bracket cost, not silently
round away maxima or loosen gates; payload semantics remain unchanged.

## E503 — first-read ownership latch, no measured worst-case gain

Counter.end(), freeze() and rejection close the seen latch. Only successful
begin opens it. Dispatch drop calls end before clearing ACTIVE; first_count
therefore needs only the seen check, without separate ACTIVE/frozen tests.
Acceptance retains its ACTIVE check. Added regression coverage for late reads
after end/freeze, including a dispatch with no read. 221 library +34 other
Rust tests PASS; release-s/thin-LTO build PASS.

Actual/root ELF SHA256:
`D31FAB7A4B4FA09ED4B3F929CC92FCF8479DE70FE49EAF121C580EDF2D92C707`.
Preflashoff/download/OpenOCD reset and guard3/18 PASS, no UART repair.
direct_503_guard01 SHA256:
`DC86F7E047168BF4B018D43D40CB1E13D3F06915AF40A50DCFB7A0265AD2E35D`.
direct_503_scope01 SHA256:
`7809A74364B6E00F1B114A44F262F2FA43740BE605E6EB8F29ECF75F065CFD42`.

All240 semantic cases and wrapped/partial QD85 CRC wire PASS. Closed/open/
accepted/no-gate/disabled maxima are3/3/5/2/2us for each preload0/16/32.
Still FAIL2/4us gates; no measured worst-case improvement over E502. Finaloff
verified and UART closed before host cost rejection. No motor/fullpreflight.
Installed D31F remains NOT powered-qualified. Do not portray this disabled
instrumentation work as additional motor progress or a fix for CycleTiming.
Further tiny collector variants need a new measured/emitted-cost hypothesis;
the last powered evidence remains E488.

## E504 — cost-scope audit and final-observation binding

Source audit: minz/core/src/am32_isr.rs comp_isr reads interval once at the
gate, and interrupt_routine reads it again only after persistence passes.
The disabled directcheck currently invokes first_count twice for closed and
open-rejected modes as well as acceptance. This over-tests those ordinary
paths conservatively; it does not explain away acceptance5us or justify
subtracting guessed harness time. Keep2/4 gates unchanged. D31F emitted RAM
begin/accepted helpers were inspected; source-level simplification alone has
not established a passing full bracket.

Added DIRECT_OBSERVATION_EPOCH snapshot inside observation_reset's existing
critical section, and fixture-only post-stop DIRECTBIND with that epoch,
actual ACCEPTS, final_observation=1 and RAM feature identity. No per-dispatch
work added. The existing collector header is independently compared to this
saved observation epoch and final ACCEPTQUALITY events by decode_campaign.
Any direct protocol data present requires a valid binding, even when the
caller did not require the feature. Standalone disabled directcheck remains
a substream and uses decode(), not campaign proof.

Host --qualification-direct requires trace0 and excludes other qualification/
path collectors. This flag checks provenance; it does not qualify the image's
timing or authorize bypassing preflights. Decoder tests reject missing,
duplicated, wrong-epoch/count, non-RAM and wrong-scope binding data. All414
Python tests PASS. It proves only final-observation consistency, not first
segment coverage, physical edges, lock, or authenticity against a deliberately
spliced complete capture. Whole-campaign safety verification remains separate.

Release-s/thin-LTO root SHA256:
`0BDAC79211D624D73066C4F070C65FC0909BB2C8BF881CE62837DADDFBC6D3D0`.
Automatic emitted audit has matching SHA. Built NOT flashed; no UART/motor.
Actual D31F remains last verified OFF and NOT powered-qualified. New binding
wire is not hardware-verified. Remaining cost/full-preflight and powered
fidelity requirements are unchanged; this does not close the motor goal.

## E505 — native packed counter words, no measured timing gain

Emitted E504 acceptance code spilled three halfword values to local stack
then reloaded/packed them into rows. Changed Counter's internal representation
to three u32 words already in row order: dispatched<<16, closed|open<<16,
first|last<<16. Checked dispatched increment still bounds both bins, overflow
remains sticky; snapshot and accepted() preserve pure Counts API. Live code
uses accepted_words() with identical validation and QD85 words. No guards or
timing limits changed. New mixed65535-visits/maximum-count/reuse test PASS;
222 library+34 other Rust tests PASS. Counter remains <=16bytes on host.

Release-s/thin-LTO actual/root SHA256:
`9001C2B57F8A9737166302E485DA124971761CD8D9BDD81EF1684C85C66431BF`.
Automatic emitted audit SHA matches. Acceptance helper224->200bytes, begin
88->92bytes. Smaller acceptance code is NOT a measured runtime gain.
Preflashoff/download/OpenOCD reset PASS without UART repair. Guard3/18 PASS:
direct_505_guard01 SHA256
`97D324E1BE96F670433B77114AC830B0447141F4F304660FD267FB648693F0C9`.
direct_505_scope01 SHA256
`177502BD453AF5A21205B0DEF97A500809742EF2C7893514B2446617F6D7B23C`.

All240 semantic cases and wrapped/partial QD85 CRC wire PASS. Closed/open/
accepted/no-gate/disabled maxima remain3/3/5/2/2us at all preloads0/16/32.
Still FAIL2/4 gates. Finaloff verified before cost exception; UART closed.
No motor/fullpreflight. Installed9001 includes E504 binding but directcheck
does not exercise observation_reset/report binding. That wire proof remains
open. Do not claim cost improvement or perform equivalent packing retries.

## E506 — cycle-resolution check rules out rounding-only explanation

shell-pwm already configures SysTick for Core64MHz, reload63999, enabled
without IRQ. Disabled directcheck verifies CSR low bits5/reload63999 then
reads current counter immediately around actual hooks, inside the existing
TIM17 bracket. No timer writes, no new powered-path work. CSR read clears
foreground blink COUNTFLAG once; no safety clock uses it. Cortex-m local
peripheral/syst.rs verifies current/reload reads have no side effects.
Downcounter delta handles one wrap; outer TIM17 must be <1000us and the
inner delta consistent with its quantization. This is a short disabled test,
not a multiwrap blackout detector or powered WCET proof.

Host cycle_report checks15 ordered rows, clock geometry, validity and raw
nonzero bounds. Reports raw cycles/64 without subtraction; supplemental
128/256-cycle comparison does not replace existing microsecond gate. Added
tests for exact limits, one-cycle excess, malformed/duplicated/invalid rows.

Release-s/thin-LTO actual/root SHA256:
`7BB98B24F73D512698A53E88F9523B66EB41F6005A01F6F43CD4ABD122CD9BA9`.
Preflashoff/download/OpenOCD reset PASS, no UART repair; guard3/18 PASS.
direct_506_guard01 SHA256
`2D3C67C4929ECA6B4E443154B3EA3B5A4EEF0A1F6EA2A2EA8413B7BD34ABBFA7`.
direct_506_scope01 SHA256
`433ED9F7130E60836D70FB273C6FCE505E58440D3B4E2C285065E1D10EE16B05`.
All240 semantics and wrapped/partial QD85 CRC wire PASS; finaloff verified
before cost rejection and UART closed. No motor/fullpreflight.

Closed/open/accepted/no-gate/disabled raw maxima in core cycles:
preload0:146/160/294/105/91; preload16 and32:146/160/299/105/91.
Worst raw times2.28125/2.5/4.671875/1.640625/1.421875us. Thus measured bracket
still exceeds2/4 limits by18/32/43cycles for closed/open/accepted. No inferred
overhead subtraction; these remain hook-plus-harness measurements. Outer
TIM17 now includes two extra SysTick reads and reports acceptance6us; this
is not evidence the live acceptance path regressed from5us to6us. The cycle
data disproves rounding-only sufficiency for this measured build, not a
universal hardware floor. Next optimization can target exact remaining cycles.

## E507 — inline dispatch setup saves cycles, still fails

begin is now inline(always), allowing setup/first-read code to share state;
accepted remains200B in RAM. First-read hook already inlines (no separate
symbol/call in emitted ELF); do not try another redundant inline annotation.
Actual/root B2DB8535936299AC9B67B247E7B0DDC6B80D103D20A829883A043CF3C407776D.
Preflashoff/download/OpenOCD reset, guard3/18 and240semantics/QD85wire PASS.
Guard capture SHA18D1D1EC039E24524D0E349F302116F48BB76B26ED8E2A2530E86395F76C7D07;
scope SHA2DB843894B88252393B97D2E0B8ACEE06CB3B961D5A2754159AEE356818B2ADD.
Raw cycles preload0=130/143/285/88/61, preload16/32=130/143/290/88/61.
Gains16/17/9cycles worst closed/open/accepted vs E506, still FAIL128/256.
No motor/fullpreflight. Finaloff verified/UART closed. Actual B2DB remains
unqualified. Begin inline is retained on measured improvement, not a pass.

## E508 — full-width threshold removes an unnecessary clamp

Cache half-average as u32 rather than clamping to u16. Compare widened u16
count directly to it. For threshold>65535 every possible hardware count is
closed, exactly as before. Added exhaustive65536-count comparisons for eight
thresholds including0/1/500/65534/65535/65536/0x7fffffff/u32MAX. All223 library
+34 other Rust tests PASS. No change to observation counts/guards/QD85.

Release-s/thin-LTO actual/root SHA256:
`2EB49AB91E5FA3EEF58207E14D8A5D50F3A84B7DA179B51E24BE39FC03B6A635`.
Preflashoff/download/OpenOCD reset PASS without UART repair. Guard3/18 PASS.
direct_508_guard01 SHA256
`4044E845347127AE33116D615E99099412949F92870E03F2DA93D4728A086EC0`.
direct_508_scope01 SHA256
`437D7615B902669E0C59A9029683F5F9E1875E27F0B7A0456045F6049F45431D`.
All240 semantic cases and wrapped/partial QD85 CRC wire PASS; finaloff
verified before cost rejection, UART closed. No motor/fullpreflight.

Raw cycles preload0=121/134/276/79/61; preload16/32=121/134/281/79/61.
Nine-cycle reduction on active cases vs E507. Worst closed1.890625us now
below2us; open2.09375/accepted4.390625us remain over by6/25cycles. This is
measured progress, not a full pass. Outer TIM17 includes SysTick observer
reads and still rejects; no timer overhead was subtracted or limit changed.
Further optimization must preserve all mixed-bin/overflow/acceptance checks.

## E509 — unified dispatch ownership state

Counter now owns inactive/unread/read state instead of seen plus a separate
global ACTIVE. Successful begin enters unread; first read enters read; exit,
freeze and rejection become inactive. Adapter checks Counter.active() before
committing. Counter itself now rejects acceptance after exit, not merely its
adapter. Updated mixed-maximum test to commit before last exit; added explicit
after-exit and previous-open-but-current-unread refusal checks. All224 library
+34 other Rust tests PASS. No reference/guard changes and QD85 unchanged.

Release-s/thin-LTO actual/root SHA256:
`87E0DC493662A6764DE9FCC52DFC70A23CEA7813B9B3C9E47DCE7F04FC020110`.
Preflashoff/download/OpenOCD reset PASS without UART repair. Guard3/18 PASS.
direct_509_guard01 SHA256
`18D1D1EC039E24524D0E349F302116F48BB76B26ED8E2A2530E86395F76C7D07`.
direct_509_scope01 SHA256
`8C13C5F71D26D29A44E78AE7E17CA87E3BD023C0B550C91121AE176C816D1D6E`.
All240 semantics and wrapped/partial QD85 CRC wire PASS; finaloff verified,
UART closed. No motor/fullpreflight.

Raw cycles preload0=118/134/272/80/50; preload16/32=118/134/277/80/50.
Open unchanged, accepted four cycles better than E508; still FAIL128/256
raw-cycle comparison and original outer TIM17 gate. No claimed powered gain.
Potential next simplification follows actual invariants: READ implies not
frozen, and open>0 implies dispatched>0. If used, preserve frozen refusal
without invalidating a normal stopped partial bucket; no guard relaxation.

## E510 — invariant-based acceptance checks save13 cycles

Check state==READ before frozen; READ cannot coexist with frozen because
freeze revokes state. On non-READ, frozen returns None without invalidating
normal partial capture, while other states reject/mark invalid. On READ,
nonzero open bin implies nonzero dispatched (checked begin bounds bins;
publication clears all words), eliminating the redundant dispatched test.
All224 library+34 other Rust tests PASS, including ownership/freeze/overflow
and mixed65535-bin coverage. No reference/guard/format changes.

Release-s/thin-LTO actual/root SHA256:
`BBC24B6D0C96C8AB6F7D63843506836CD2532B3A493FDE4EFC26D4E618B6E0F6`.
Preflashoff/download/OpenOCD reset PASS without UART repair. Guard3/18 PASS.
direct_510_guard01 SHA256
`C59B03A5C672BB2B10919200400B5C27F0749BBD487C01D079488F048CE74927`.
direct_510_scope01 SHA256
`8087AED158FC5A78749E1DDD5B70DA8FD5AC91FEC29B59AC15E76EE5A95916A3`.
All240 semantics and wrapped/partial QD85 CRC wire PASS; finaloff verified
before cost exception, UART closed. No motor/fullpreflight.

Raw cycles preload0=118/134/259/80/50; preload16/32=118/134/264/80/50.
Accepted improves13cycles vs E509. Worst open2.09375/accepted4.125us still
FAIL128/256cycles, by6/8cycles respectively. Original outer TIM17 verifier
also rejects. No subtraction, rounding, limit change or powered-gain claim.

## E511 — explicit hook bracket, full preflights, first powered direct capture

Moved runtime harness mode selection before noninline const-mode measurement.
Opaque inputs, real begin/first_count/accepted/drop remain inside raw SysTick
endpoints. Same two first-count invocations for read modes (still conservative
for ordinary reference calls). Controller hooks unchanged. Emitted accepted
measure at080182a8: SysTick reads080182d4/08018350 enclose actual Counter work,
RAM accepted call08018344 and end store0801834c. Frame/setup occurs before
timestamps; LLVM also places cycle-delta arithmetic before outer TIM17 read,
so outer TIM17 includes work outside the measured hook bracket. No assumed
observer subtraction. Compilation specializes four functions (closed/open
share one), not synthetic constant counters.

Actual/root release-s/thin-LTO SHA256:
`BC876BF0919EDC8B3FF00019A8D44DB8D3DE7142EB72C84151103EB19689B630`.
Emitted audit SHA matches. Frozen ELF: captures/reference/direct_bc876/shell-pwm.elf.
Preflashoff/download/OpenOCD reset PASS. Guard3/18 PASS, SHA
`A51A712F718447FF47CAE3B14FDE1A27A448122E0EAFF4E4D02CA88B9247D587`.
direct_511_scope01 SHA
`750ECBBC0ED699870C13CD596C9053F3EA2400B9F0052B07706413503E152D8B`.
240semantics and actual wrapped/partial CRC wire PASS. Raw cycles preload0
74/88/231/60/11; preload16/32 74/88/236/60/11: all <=128/256 cycles, with
no subtraction. This is a measurement-scope correction, not controller gain.

Legacy outer TIM17 verifier remains FAILED5>4 at preload16 acceptance. New
explicit --cycle-gate enforces the SAME2/4us budgets as128/256 raw64MHzcycles,
requiring valid clock/rows/semantics/wire/finaloff; it never subtracts timer
cost. E510 still fails that gate. Tests retain both verdicts, reject one-cycle
excess, CRC/semantic corruption, missing cycles and malformed FINALOFF (now
anchored with optional actual shell prompt). Initial regression test exposed
the old suffix-match weakness; fixed, then all416 Python tests PASS. Older
failed builds remain failed, not retrospectively qualified by this change.

Exact-build broader preflights direct_511_preflight01: five pulse/filter cases,
256 atomic checks, six PWM roles, CPU2/6/9us, archive3x3 PASS. ADC route3checks
PASS in direct_511_adcphase01. All disabled stages verify finaloff.

One matched powered campaign direct_511_start61_reentry69_30s_01 SHA256
`E163DEE974975E5413E35A1341AB57B8B607997C9D9ABABC4ECAC0D0E9F8D328` PASS.
6.1% driven startup,+60degrees,6.9% BEMF/20kHz, dropout+fresh reentry, original
30s deadline unchanged. Resumed27.991438s,333.827223eHz, cycle sigma24.090582us,
56066COM/56065accepted, IRQ53.907326%,rawpeak347,busmin10937mV,stackuntouched2200.
COMP45/COM45/commit21us, armspare8us. ADC139261samples/3200period CRC histogram
PASS. Current is raw, not calibrated amps. Startup rawpeak705 retained.

DIRECTBIND epoch246 matches QD85 and final ACCEPTQUALITY56065. Final16 rows
identities56049..56064 have6..9 open visits, first-to-last open count spans
roughly350..520 half-us ticks; multiple open visits are not individually
classified persistence failures. Partial56065 has6dispatch/1closed/4open/1no-read,
preserved until unwind. No first-epoch direct archive or physical-edge proof.
All timeline/recovery/whole-campaign/finaloff validation PASS; UART closed.
This is one successful diagnostic run, not a reliability cohort or a fix for
CycleTiming. IRQ increase vs E488~3.44percentage points is an unpaired observed
cost, not an isolated overhead estimate. Next one diagnostic70hold at the old
failure point, with identical guards and direct binding; no speed-cap increase.

## E512 — fault captured: qualification extension precedes the final gate read

Same installed/frozenBC876 image, no rebuild/flash. One10s requested70hold
(6.1%driven,+60degrees,7.0%BEMF,20kHz,no dropout) FAILED after439797us;
CycleTiming12 step2, guard delta2810<2858.880COM/879accepted,rawpeak304,
busmin11343mV,stackuntouched2200. All raw captures retained, no retry. Fixture
correctly rejected powered completion; separate diagnostic/ADC/fault analyzers
validate the failed record and finaloff. UART closed.

captures/direct_512_start61_hold70_10s_01.txt SHA256:
`9CEAA65E83E15EB43814BDB8810DC9C12929607EF29EE8090E537F80E3955DC1`.
QD85 epoch248/final879 validates16rows863..878 plus partial879; ADC2188 samples
with3200period CRC histogram validates. Partial879 has7dispatch/2closed/5open,
first581/last898ticks. CYCLECORE this_zc916,last_zc999,filter12,IRQ mode,EV_ACC
reached guard before recorder refusal; partial is not a persistence failure.

Added drv_direct_intervals.py: full summary/off check, strict campaign binding,
accepted identity and sector join to retained reference tail. Ranked adjacent
long/short pairs explicitly exclude pairs outside16-row direct coverage.
Four tests exercise actual pass/fail captures, cross-stream total/sector/
coverage errors and CRC corruption. All420 Python tests PASS. No physical-edge,
persistence-failure-count, first-epoch or causal claim from this join.

Relevant measured chronology (microseconds on the reference interval counter):

| Accepted identity / step | Open visits | First open | Last open | Reference interval | Last open to reference |
|---|---:|---:|---:|---:|---:|
| 867 / 2, preceding same-sector visit | 5 | 287 | 458 | 467 | 9 |
| 873 / 2, long boundary | 11 | 266.5 | 569 | 578 | 9 |
| 874 / 3, following short boundary | 5 | 286.5 | 395.5 | 404.5 | 9 |

Boundary873 is the earlier same-sector visit referenced by the later guard
failure879, not the final safing call. Against same-sector tail medians,
873/874 residuals are+112.5/-96us (sum16.5). Reference adjacent full cycles
3026/2804.5us, local recorder median2942.5us; guard-reference closure5.5us.
These are distinct timestamp brackets, not independent rotor measurements.

What this establishes for this boundary: first open was earlier, not later,
than the preceding same-sector visit. The extra111us in its reference interval
matches extra111us at the last open read; final read-to-reference stays9us.
Thus the extension occurred before the final successful gate read, alongside
six more open visits, not in a long post-gate accepted path. It does not tell
whether intermediate visits rejected persistence, whether preemption delayed
an earlier visit, or what physical signal/PWM phase caused them. Passing E511
also has multiple open visits, so their mere presence is not an error.
Next investigate the source/timing of these extra visits, not another70retry,
hardware blame, guard relaxation, or interpreting final partial as rejection.

## E513 — source audit constrains the next intervention

Read actual comp_input::Input.change_input, Comp/StaticComp adapters and
minz/core am32_isr::comp_isr/interrupt_routine. Raw mode configures RTSR18
or FTSR18 according to expected polarity, not both. Every reference open-gate
visit clears both pending registers before entering persistence; closed gate
leaves pending only when the sampled level is already post-ZC. The latter
mechanism cannot be inferred from increased open counts: boundary873 actually
has one fewer closed visit than preceding same-sector867. This is source
behavior, not proof that all electrical edge/acknowledgment timing is ideal.

Also checked minz/core am32_loop::filter_and_duty_max and am32::map. At observed
average977ticks, map(average,100,500,3,12) clamps to12; early-start branch also
selects12. Thus12 is reference-consistent here. Do not lower it on the premise
that binz forgot a reference speed-dependent reduction at this operating point.
rm32 main_state uses the same map. No sibling sources modified. Targeted
reference tests: comp_isr4, interrupt_routine3, filter_and1 all PASS on host.

Clarification: E512's9us is from the first actual Interval.count sample in the
final open visit to the later reference acceptance Interval.count sample.
It includes first-read diagnostic bookkeeping, trace-flag check, pending clear,
the raw persistence loop, entry to the reference critical section, comparator
masking and estimator timestamp setup. It is NOT the wall time of12 comparator
reads, and comparing9us directly to a PWM on-pulse width would be invalid.

Extended drv_direct_intervals report with strictly consecutive same-sector
six-identity deltas (no interpolation if predecessor absent). Boundary873 vs
867: open+6,closed-1,first-open-20.5us,last-open+111us,reference+111us. New test
checks exact retained values and missing/incorrect predecessor rejection.
All421 Python tests PASS. Counts remain observations, not an unconditional
persistence-failure counter; partial879 specifically contains a reference
acceptance vetoed by the motor guard.

No firmware edit/build/flash/UART/motor. ActualBC876 remains last verified OFF.
Next separate actual rejected persistence reads/elapsed time from surrounding
adapter work. No filter relaxation, repeated70run, new hardware diagnosis or
closed-gate storm fix is justified by this source audit alone.
