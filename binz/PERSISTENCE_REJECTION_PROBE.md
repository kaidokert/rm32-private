# E514 — direct persistence-rejection observation seam

Purpose: E512 establishes extra open visits before the final acceptance read,
but not whether each rejection occurs on read0 or after a run of matching
reads. Do not shorten filter12 or infer its wall duration from the9us bracket.

Added default no-op Recorder::persistence_rejected(read_index:u16) in shared
minz/core. interrupt_routine invokes it at the existing mismatch return with
the zero-based loop index. There are no new comparator reads, timestamps,
filter decisions, timer actions or output actions. Existing implementations
need not override it. This is an observer seam, not a controller rewrite.

Tests script every failure position0..11, retain an unread trailing sample,
verify exactly one callback and no timestamp/reset/COM-arm/EV_ACC. Closed gate
and successful acceptance produce no callbacks. Initial success-case test
omitted valid sector initialization and panicked on existing step-1; fixed
the test setup to step1. All85 minz-core tests and224+34 binz Rust tests PASS.
No sibling firmware or hardware run.

Binz does not override the callback yet. Release-s/thin-LTO root ELF SHA256:
`F21FBE7EAECD1737B0466108217BCF1B4071276517E739815C3B0B90BFDDECE9`.
Not flashed; actual frozenBC876 remains last verified OFF. All five PT_LOAD
segments (addresses, file/memory sizes, flags and contents) match BC876 exactly.
Their content SHA256s, in segment order:

- vector: c3bb5d10845a503f229f531caa8f9ff5439157a044ab971d25c9f93fbc85d27c
- executable flash: 97d6bf44de37c176a1c83e9744d8b6eb1d3fb4e964d3842d6bcd2a3ae617cedb
- read-only flash: c3b1e10e483a76a3ce7547de2348869302e3ea0bf201da2e59d44e13e37bd1fe
- initialized RAM: ef8b0ba76d676ea4f8b9812e431d87130e69a9baa369889fdc6be4b0fcfcd6be
- zero-file-size BSS segment: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855

No persistence_rejected symbol remains in target nm output. This proves the
default no-op for this exact build, not every compiler/platform/configuration.

## Next integration, not implemented or qualified

Prefer one12-bit presence mask per accepted bucket: bit i means at least one
reference rejection on sample i. This distinguishes immediate mismatches from
later failures without per-read instrumentation, timestamps or histograms.
It does not encode counts/order/duration/PWM phase or preemption.

An opt-in binz Recorder override should update only the active direct bucket,
with the same until-unwind partial semantics. Require current read ownership;
reject unsupported index>=12 instead of truncating. Reuse unused lower bits
of the packed dispatched word if codegen is favorable: sector occupies4bits,
mask occupies12bits, dispatched occupies upper16. That requires explicitly
versioned QD85-v2 parsing; never reinterpret v1's sector field silently.

Before powered use: pure mask/overflow/reset/ownership tests, strict v2 CRC/
identity decoder, rejected-callback cost in disabled actual-hook bracket,
unchanged128/256cycle and CPU10us limits, full preflights, and preceding69
campaign. Do not use the existing no-callback cost pass as proof of callback
cost. Preserve original reference sequence and all electrical/tracking guards.

## E515 — opt-in mask integrated, first hardware cost still fails

Counter stores12 presence bits in dispatched-word bits4..15, leaving sector
bits0..3 and checked count bits16..31 intact. Frozen partial preserves mask;
accept/reset clears it. Invalid index>=12 or missing current read rejects.
Three tests cover all12 positions, repeated bits, ownership/freeze,65535
dispatch overflow and no mask carry.227library+34other Rust tests PASS.

bench-qualification-reject implies direct RAM probe. Obs overrides the shared
callback only under this feature. QD85-v2 packs mask into upper12bits of the
sector word, keeps11u16 frame length and CRC. Partial mask explicitly retained.
Host v1 never reinterprets sector bits; v2 exposes positions, not counts/order.
Distinct mask bits must fit eligible open visits (complete needs one accept).
--qualification-reject requires --qualification-direct and v2 evidence.
All422 Python tests PASS. Raw check exercises runtime-opaque index11; v2 wire
test has one rejected visit and one accepted visit per bucket, all12 positions
across40accepts, plus rejected partial11. No extra comparator reads.

Actual/root release-s/thin-LTO SHA256:
`144CEBD00AF4872524CC1CB4DF64B517ABBF26181D250BD7FE5B12AABC69CFF2`.
Preflashoff/download/OpenOCD reset PASS. Guard3/18 PASS, SHA
`5474653004CD4760ADC017043289B4E957B7DE92D3265E8CC618C49311E3A2CC`.
reject_515_scope01 SHA
`B21925ACE4C212E08F4AD637BF5263394474B947631889EF3044C1C2B130A020`.
240semantics/v2 wrapped+partial wire PASS; raw cycles preload0=76/136/229/60/11,
preload16/32=76/136/234/60/11. Rejected path136>128 FAIL; acceptance passes256.
No motor/fullpreflight. Finaloff verified/UARTclosed. Next remove redundant
frozen check under READ invariant, preserving cold refusal/partial semantics.

## E516 — READ-first rejection check regresses the measured bracket

Changed rejection validation to check READ first, preserving frozen partial
without invalidation on the cold path. This is logically equivalent for the
supported calls;227library+34other Rust tests PASS. Hardware does NOT show
the expected gain. Actual/root release-s/thin-LTO SHA256:
`61402B05D9B5C25E5C99F69005994F054961D800CEAA296945E97BF8FE708C42`.
Preflashoff/download/OpenOCD reset PASS, no UART repair. Guard3/18 PASS, SHA
`D59088061FB269741544044BAB689851A3270CB5063FA527C81979D55678EAFF`.
reject_516_scope01 SHA
`5CEF1D03A84A01050584C8E7D1FF19A69B69EE91AC58A430AC04C82314A413C2`.
240semantics and actual v2 wrapped/partial wire PASS before cost refusal.
Raw cycles preload0=78/140/231/58/11; preload16/32=78/140/236/58/11.
Rejection140>128 FAIL, four cycles worse than E515; acceptance still passes.
No motor/fullpreflight. Finaloff verified, UART closed. Source and installed
6140 both retain this tested variant; no hidden source revert. Next inspect
the emitted rejection/state-check path before another tuning variant; do not
infer a gain from source-level removed checks or change numeric budgets.

## E517 — explicit open/consumed state passes cost and one powered recovery

READ now proves the current first read was open and not yet accepted. Closed
reads and accepted callbacks move to CONSUMED. This removes a hot open-bin
load and closes the stale-prior-open validation hole. Tests cover closed reads
after earlier open visits, duplicate acceptance and post-accept rejection.
229 library +34 other Rust tests,85 reference tests,422 Python tests PASS.
Release-s/thin-LTO build and automatic emitted math audit SHA256:
`7C7E773BB21E7CE50D00C5E1F2306228883CBFDB04E7EA20F72AAA362AAC32EC`.
Audit is advisory, not whole-program absence-of-soft-math proof; no direct
measure/accepted helper calls appear in its helper attribution list.

Installed after verified-off/download/OpenOCD reset. Guard3/18 PASS.
reject_517_scope01:240 semantics, wrapped/partial v2 wire PASS. Raw cycles
closed/rejected/accepted/no-read/disabled =76/126/231/60/11 at preload0;
preload16/32 acceptance236, others unchanged. Original128/256 limits PASS,
no subtraction. Legacy outer TIM17 gate still not passed (observer overhead).
Five disabled preflights PASS: pulse,atomic256,roles6,CPU2/6/9us,archive3x3.
ADC phase route3checks PASS. These are not powered WCET guarantees.

Same exact build, one preceding69/30s injected-loss recovery PASS:
`captures/reject_517_start61_reentry69_30s_01.txt`, SHA256
`A89E70826B28944CE81E7C26DBA4E71815660670EA58C981CB424DF3D12ED179`.
6.1% driven,+60degrees,6.9% BEMF,20kHz,original deadline retained.
Resumed27.990876s,333.903395eHz,sigma24.979443us,56078COM/56077accepted,
IRQ54.410521%,raw peak338,bus minimum10948mV,stack untouched2200.
COMP/COM maxima45us,commit21us,arm spare8.5us. ADC139258/3200 CRC PASS.
QD85-v2 identity/final partial verified. Late rejection positions exist in
this successful tail too. One pass is not a reliability cohort or isolated
probe-overhead A/B. Raw peak is not calibrated amps. Outputs off/UART closed.

## E518 — one70 diagnostic captures actual late persistence rejection

No firmware change. One10s requested hold at7.0% (same6.1% startup,+60deg,
20kHz) FAILED CycleTiming12 after222154us; no retries. Step4 guard2844<2858,
438COM/437accepted,raw peak312,bus11462mV,stack2200. Guard/reference-cycle
closure5us (2844 vs2839); not independent rotor speed or ISR latency proof.
`captures/reject_518_start61_hold70_10s_01.txt`, SHA256
`68510EB3F7FB5881A81B8B92268765E7C559AD5CD12F493ECE84C7864B033D27`.
Final off/timeline/CRC/direct identity and ADC1105/3200 verified despite
expected whole-campaign verifier refusal. UART closed, no processes left.

Earlier boundary431 step4:9open visits, rejected read indices[0,1,11],
first-open291.5us,last-open530us,reference539.5us. Compared to425 same sector:
two extra open visits,first+12.5us,last+48.5us,reference+49us. Following432
interval425us (prior same sector504.5us). The actual twelfth-read mismatch
is proven; it is not inferred from open-minus-one. Partial437 has[0] only;
its final reference callback passed persistence before motor guard veto.

Interpretation: both immediate and late persistence failures occur. This
does not prove all late failures are preemption, nor that the mask's bit11
belongs to the last rejected visit. Successful E517 also has late failures.
Do not label this an exact reproduction of E512's different boundary873.
Next compare the actual persistence read schedule with PWM phase and pending
edge handling, using the reference sequence and emitted code. Presence-only
data cannot locate the individual failing visit in time. No hardware changes,
priority flip, filter shortening or cycle-floor relaxation justified yet.

## E519 — emitted successful loop audit (offline, no new firmware)

Added scripts/drv_persistence_codegen.py: inspect ADC_COMP, resolve the COMP2
literal0x40010204 and identify the narrow successful-iteration control-flow
shape. Reject an unknown signal address, call or ambiguous/missing loop.
Three tests use the frozen BC876 ELF plus mutated disassembly. This is a
shape/footprint audit, not a general ARM interpreter or cycle estimator.

Reproduce with:
`python scripts/drv_persistence_codegen.py captures/reference/direct_bc876/shell-pwm.elf target/thumbv6m-none-eabi/release/examples/shell-pwm`

BC876 static successful iteration:15 executed instructions, loop08001cca to
08001ce8, comparator load08001cd2. Current7C7E:16 instructions, same loop head,
back-edge08001cea, comparator load08001cd4. An extra mov preserves the index
for the rejection callback. Thus the integrated probe changes sampling cadence
even on successful reads; the E514 default-noop binary identity never proved
the later active callback was timing-neutral. Both loops have one comparator
read and no calls in the recognized iteration. The dynamic fallback has a
different longer loop and must not be mistaken for the powered static path.

The callback's mask write occurs only AFTER a mismatch; it cannot cause that
same read to fail, although previous callback work and the extra loop move can
shift later timing. No statement of exact microseconds follows from instruction
count: flash/peripheral waits and preemption remain unmeasured by this audit.

Source reconfirmed: reference comp_isr clears EXTI before the open-gate loop;
only expected-edge polarity is armed by comp_input. A transition away from
the expected level can reject late without itself supplying the next expected
edge interrupt. This allows another PWM-dependent visit but does not establish
which physical transition or scheduler event occurred in E518.

No motor/flash/UART in E519; installed7C7E remains last verified OFF. Next
timing evidence must associate a specific failed visit with its PWM/interval
position; aggregated bit11 alone cannot do that. Prefer observation at the
mismatch return, not an instrument on all12 reads, and measure its perturbation.

## E520 — late-only callback coordinates implemented, cost refuses powered use

bench-reject-time opt-in depends on rejection mask. After a valid nonzero
mismatch index, read TIM2, TIM1 PWM CNT, TIM6 CNT, then TIM2 again. Store last
eight callbacks in a checked sequence ring. Each row carries accepted identity,
dispatch ordinal, rejection index, original first-read count and these four
register values. No comparator read added; no output/timer writes. Reset under
existing stopped observation ownership; dump only fixture capture after unwind.
Ring overflow invalidates rather than wraps sequence.160bytes row storage.

RT85-v1:14u16 words including epoch/sequence and five packed data words, CRC.
Host requires QD85-v2 same epoch/final identity, valid indices1..11, increasing
accepted/dispatch pairs and agreement with any retained direct bucket. TIM2
bracket delta is modulo65536, not a proven elapsed interval through arbitrary
wraps. Coordinates are sequential callback-time reads, NOT physical mismatch
timestamps, simultaneous samples or a preemption certificate. Powered campaign
fixture must still explicitly require this stream before future powered use.

231 library +34other Rust tests PASS (ring retention/reset/overflow included).
429Python PASS. A new corruption test exposed Python Ascii85 accepting a final
single character without producing a byte; RT85 now requires exactly eight
complete groups after zero-group expansion, rejecting this ambiguity.

First release-s build fails FLASH overflow952bytes (.gnu.sgstubs960). Nothing
flashed from that failure. CARGO_PROFILE_RELEASE_OPT_LEVEL=z with same thinLTO
and all features/guards retained fits. Exact installed/root SHA256:
`84DFB2A391E6CB13BA139061593BA538A0DBE81BA41463B54412EC47CD9FEF48`.
Preflashoff/download/OpenOCD reset succeeded. Guard3/18 PASS, capture
reject_520_guard01 SHA256
`5EA0018D4296874B0C31F7B5A4610EAFA78D32B79D1F7C7F3B0EE921376B359A`.
reject_520_scope01 SHA256
`F76480800F553D00C6E214AB77FAB24BF358CA0A1F99C4E036D5A90E8F397B83`.
240 semantics, QD85 wrapped/partial and actual RT85 sequence37/n8 PASS before
cost refusal. Raw closed/rejected/accepted/no-read/disabled174/546/379/81/77
at preload0; accepted387 at16/32, others unchanged. FAIL128/256 unchanged.
No motor or remaining preflight. Final off verified, UART closed.

This is not a clean overhead A/B: compiler profile AND capture changed.
Nevertheless closed and accepted paths already fail without taking a late
snapshot. Global opt-z is not qualified by fitting flash. Next restore opt-s
and reduce code footprint (particularly duplicated capture/formatting work)
before more bench use; then measure the late path separately. Installed84DF
must not inherit7C7E's powered qualification. Root source retains this attempt.
