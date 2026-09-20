# Speed expansion and measured seed age — E370

## E441 — current85us recovery age and the next boundary

E436–439 recovery records on7F4B report edge_age_ticks170=85us. This is
measured, not WCET. E440 failed during initial acceleration, not recovery;
its cycle2844us refusal and this prospective arm constraint are distinct.

Using the actual reused advance_of/TEMP_ADVANCE16/wait_time functions:

| Prospective profile eHz | Floor seed ticks | Wait us | Remaining after85us | Margin over32us |
| ---: | ---: | ---: | ---: | ---: |
|350|952|119|34|2|
|355|938|117.5|32.5|0.5|
|356|936|117|32|0|
|357|933|116.5|31.5|-0.5|
|360|925|115.5|30.5|-1.5|

These floored seed boundaries are not fully qualified sequences or rotor
measurements. Existing350 admission refuses every listed lower seed. New
test measured85us_age_bounds_prospective355_without_authority verifies both
the reused arithmetic and unchanged admission;355 plus1us delay fails.
194library+34other Rust replay tests PASS; nonfatal incremental AccessDenied
notes remain. No embedded build/flash or motor operation occurred inE441.

Actual72 recovered seeds996..1007 retain7.5..9us margin; do not confuse that
observed point with the worst prospective profile boundary. A guarded355
trial is not mathematically forbidden, but does not buy robust boundary
recovery or justify a360 profile. A refused live seed must remain refused.

Next prioritize the final qualified-edge-to-arm sequence before expansion
beyond this narrow boundary. Reuse RECOVERY_POST_EDGE_AUDIT.md findings:
carrier relocation already produced no measured gain, bulk statistics are
already staged, report memcpy was not emitted. Investigate the remaining
acquisition epilogue/guard installation or a fresh qualified-edge rendezvous,
preserving seed identity, feedback age, owner checks, original deadline and
32us floor. Do not solve this by refreshing timestamps or prestaging authority.
Current7F4B stays installed/off;73 remains failed,72cohort3/3 unchanged.

## E384 - Updated lean recovery budget, 2026-09-14

E378-382 measured87us edge-to-arm after exact seed math; the91us analysis
below is historical. Reference advance_of/wait_time and TEMP_ADVANCE16 were
re-read from current sibling source. No sibling edits or new profile enabled.

| Prospective profile eHz | Minimum seed ticks | Arm remaining after87us | Spare above32us |
| ---: | ---: | ---: | ---: |
| 330 | 1010 | 39.5us | 7.5us |
| 340 | 980 | 35.5us | 3.5us |
| 345 | 966 | 34us | 2us |
| 350 | 952 | 32us | 0us |
| 351 | 949 | 31.5us | -0.5us |

These are reference integer arithmetic at floored prospective seed boundaries,
not measured speeds or WCET. An additional5us delay already invalidates340.
Current admission refuses every listed seed below1010 regardless of nominal
arm time. Test lean_seed_age_still_bounds_expansion_without_granting_authority
checks these facts using the actual reused functions.210 Rust tests PASS;
existing incremental-cache AccessDenied notes are nonfatal.

E383's3009us cycle refusal and this recovery budget are distinct constraints.
The guard and reference counters are not an independent rotor measurement;
no overspeed/CPU saturation cause established. Neither raising the cycle limit
nor averaging away its refusal solves acquisition-to-arm latency.

Next implementation target is the recovery preparation/qualification boundary:
inspect whether a final qualified-edge rendezvous can follow bridge preparation
without bridge_clear undoing it, while keeping gates disabled, live feedback,
original timestamps, fresh guard admission, complete seed validation and32us
arm floor. Do not repeat already-completed report-copy or statistics-staging
optimizations. Any change needs emitted-code/stack review and disabled checks
then matched existing-envelope recovery before a wider profile. ActualF715
unchanged, last finaloffE383; no hardware action in this audit.

## E371 ownership and generated-code audit

Important correction to the prospective design below: CURRENT recovery is
gate-disabled sensing, not continuous driven acquisition. resume_once calls
prepare/wake then flying_bench::reacquire_awake. acquire_inner explicitly
gates_off, masks COMP delivery, scans the mux and reads feedback while the
driver is awake. Initial driven handoff is a different path using Transfer
and adopt_driven. Do not describe an awake driver as powered commutation.

Source findings:

| Work | Current location / constraint | Earlier move verdict |
| --- | --- | --- |
| TIM6/reference initialization | resume_once prepare/observe_begin before wake/acquire | Already staged |
| Bulk run statistics | acquire_inner stage_reentry_statistics before scan | Already staged |
| 24kHz carrier preparation | coast_run_inner after bridge_clear | Earlier call undone by bridge_clear restoring PWM_ARR=6399 |
| Feedback conversion/age | baseline after acquire uses cached bus but original per-channel times | Fresh validity/age must remain |
| Guard admission/publication | after current feedback and remaining campaign computed | Do not prestage authority or token |
| Seed-dependent reference state/mux/arm | after twelve measured intervals | Cannot use unknown final sector/interval early |
| Recovery report and caller restoration | after final qualification | Diagnostic work, but generated code already avoids bulk report copies |

Hash-verified installed/root957F ELF: acquire_inner08008d70,size0xa88;
reacquire08009a68,size0xac. Recovery report publication080096e4..08009718
uses direct field stores and one16-byte cycle load/store, not an extra64-byte
memcpy. publish_report/publish_acquisition_report have no separate symbols;
they are inlined. Merely borrowing the report or adding inline annotations
does not remove a demonstrated copy. reacquire restores old baseline only
on the !awake branch; do not claim that copy is charged to successful awake
recovery. No runtime changes or new powered attempts during this audit.

Next discriminating step: quantify the successful acquisition epilogue and
return-to-caller path before implementing a continuation/rendezvous. Existing
49us edge-to-entry bracket includes mandatory20us confirmation and several
distinct operations; assigning all29us remainder to reporting is unjustified.
A direct continuation would keep acquire_inner's large diagnostic frame on
the powered stack (the existing track branch already calls coast_run_inner);
it needs generated-frame and stack-budget proof, not a blind callback edit.
Preserve frozen acquisition diagnostics on every later failure. A genuinely
driven recovery redesign is separate and must reuse guarded drive ownership,
not turn passive acquisition into output authority by changing a flag.

Installed957F remains unchanged, last powered/finaloff evidence E369.
Preceding6.5/6.6% hold and recovery passed; no new hardware activity here.

The reference uses TEMP_ADVANCE16, advance=(ci*16)>>6 and wait=(ci>>1)-advance.
ci and wait are half-microsecond ticks. Actual handoff requires remaining>=64
ticks (32us), independently of the speed profile. E368-369 measured91us
from the qualified edge to arm in all five recovery runs. This is observed
latency, NOT a worst-case guarantee, and includes instrumentation.

Using floor(2,000,000/(6*eHz)) as a prospective minimum seed interval:

| Prospective profile eHz | ci ticks | Reference wait us | Remaining after91us | Spare above32us |
| ---: | ---: | ---: | ---: | ---: |
| 320 | 1041 | 130 | 39 | 7 |
| 325 | 1025 | 128 | 37 | 5 |
| 330 | 1010 | 126.5 | 35.5 | 3.5 |
| 340 | 980 | 122.5 | 31.5 | -0.5 |

These are boundary arithmetic, not qualified rotor speeds. Acquisition also
checks seven cycles and twelve intervals; its same-phase minima may exclude
some uniform sequences at the floored seed minimum. A profile must coordinate
those checks with runtime cycle/event minima, not change only one constant.

Regression measured_seed_age_limits_prospective_speed_profiles evaluates the
actual reused reference arithmetic and proves the CURRENT admission still
rejects1010-tick seeds/unsupported profiles. All206 host Rust tests PASS with
bench-range320,bench-cpu-roots,bench-guard-install. No profile added or guard
relaxed. Nonfatal existing incremental-directory AccessDenied notes retained.

Conclusion: 340 is already incompatible with the observed handoff path at
the candidate boundary; 330 has only3.5us nominal slack, not robust headroom.
This does not prove a sustained-running speed ceiling, CPU saturation, or
that the prior6.7% cycle refusal was physical overspeed. Runtime cycle timing
and acquisition-to-arm timing remain separate limitations.

Next architectural audit: move legally preparable handoff work BEFORE the
final qualifying edge, rather than indefinitely tuning copies after it.
Identify which timer/bridge/observer initialization can occur under the
existing guarded acquisition owner without altering its waveform or sensing.
Keep final live feedback, original feedback age, original campaign budget,
current ownership checks, actual measured seed identity and32us arm floor.
Never prestage an Admission token, renew a timestamp, fabricate persistence,
or grant output authority from an old qualification. Existing statistics
staging is already implemented and is not a new optimization.

If preparation cannot be moved without disturbing acquisition, investigate
a real final qualified-edge rendezvous after preparation, with continuous
guarded drive and explicit ownership transfer. This is a design candidate,
not implemented or electrically qualified. Do not replace that proof with
repeated same-setting cohorts or a wider cycle guard.
