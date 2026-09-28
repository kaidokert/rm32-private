# Move the restored controller toward AM32, without losing safeguards

2026-09-27. Documentation and proposed experiments, not authorization to change
protections or a claim that an architectural rewrite is necessary.

## Baseline and scope

Current controller: branch `codex/loaded60-restore`, tag
`binz-loaded60-backports-complete-20260927`, source commit `039f046`, evidence
commit `c4d4405`. ELF SHA256:
`236614731EB5E32BA293629A5F14E7D90F61BBA1D69C1B6CE28B80E1DCB2116F`.

Source worktree: `C:/Users/kaido/AppData/Local/Temp/binz-loaded60-restore`,
under `binz/firmware50`. Build: release, size optimization, thin LTO,
`advance-ref,deep-filter`; COM-top disabled. Complementary PWM, 48 kHz running
carrier, reverse direction. The propless/diode/timer-retry experiments remain
parked on `codex/park-propless-e489`; do not confuse that worktree with this build.

The five backports preserve stop-dominant foreground writes, stop/blank-dominant
comparator resume, fresh-sector rescue admission, coherent event accounting,
and hardened host assembly auditing. Latest loaded 60% screen completed
14.775 seconds at target with no late arms, protection trips or foldback.
This is a short regression screen, not renewed sustained qualification.

## Source comparison

AM32 means the locally cached G071 implementation in
`E:/m/robot/esc/AM32`, not every AM32 version or target. Its local board and
telemetry adaptations are not a pristine upstream baseline.

| Area | Cached AM32 G071 | Restored firmware50 |
|---|---|---|
| Comparator ISR | Persistence check, record interval, arm COM using a previously computed wait. | Validate crossing, update estimator, calculate wait, publish state, arm COM and record timing statistics. More work precedes arming. |
| Estimator and wait | COM callback commutates, blends intervals, computes advance and the next wait. | Estimator decision and wait calculation are on the COMP path. COM consumes the published accepted-event state. |
| Timestamp convention | Interval sampled after persistence; COM uses a relative wait. | Entry timestamp anchors the requested deadline; elapsed ISR time is subtracted before arming. These are not identical timing origins. |
| COM timer | Each accepted crossing schedules one commutation, followed by comparator re-enable. | Also one commutation per acceptance, but the timer additionally services explicit blanking/unmask phases. |
| Missed edges | Live comparator plus timeout/restart; no continuous predicted commutation. | Adds bounded foreground level-revisit rescue through the normal comparator decision path. |
| Priorities | COMP and COM highest; housekeeping lower. | COMP, COM and ADC DMA peers at 0x40; guard alone highest at 0x00. |
| Persistence | Speed-scaled reference schedule. | Reference-derived map with `deep-filter` raising its mapped floor from three to five reads; separate very-fast two-read override remains. |
| Advance | Configurable fixed advance or duty-based automatic advance. | Installed build uses fixed level 16. Equal level numbers alone do not establish equal effective timing. |
| Protection/telemetry | Production ESC protections and housekeeping. | Additional bench sag, timing, tracking and qualification machinery, including hot-path bookkeeping without diagnostic rings. |

Primary anchors: AM32 `Src/main.c:923` (COM), `:945` (COMP), `:2631`
(filter/advance), `:2664` (timeout), and `Mcu/g071/Src/peripherals.c`
(NVIC setup). Restored firmware50: `src/roots.rs` (`det_decide_plain`,
`com_root`, `comp_resume_powered`), `src/shared.rs` (priority ceilings),
`src/run/states.rs` (`revisit`), `src/bemf.rs` and `src/run/policy.rs`.
Read executable paths: some older comments still describe superseded ownership.

## Direction: copy the useful scheduling, not weaker protection

Both designs use comparator interrupts and software-serviced commutation timers.
The main opportunity is to shorten crossing-to-arm work, not replace Rust or
assume hardware commutation is required. Preserve our safety behavior while
moving nonessential work out of that deadline path.

1. **Priority experiment first, after the ownership audit.** An AM32-like
   candidate puts COMP/COM above ADC/housekeeping. Retain the guard and all its
   deadlines. Changing priorities changes preemption and the validity of typed
   shared-resource ceilings: inspect DMA/COMP/COM/guard borrows, atomic stop/arm
   ordering and pending interrupts before any register change. Measure maximum
   guard response and starvation under chatter, not just average occupancy.
   If the guard cannot meet its deadline, reject or redesign the candidate;
   do not widen its threshold. Keeping the guard highest while reducing its
   service cost remains a legitimate alternative.

2. **Move preparation off the COMP deadline path.** Prototype AM32's division
   of work: COM prepares the next estimate/wait; COMP qualifies, timestamps and
   arms promptly. Preserve which accepted edge and sector own each value.
   Match update order, initial seed, wrap handling and timestamp origin in
   replay before powered testing. Moving a calculation changes semantics unless
   demonstrated otherwise; it is not merely a performance refactor.

3. **Simplify only where evidence supports it.** Examine blanking services,
   foreground rescue and statistics separately. Do not delete a demonstrated
   rescue mechanism, shorten persistence or alter advance merely to resemble
   AM32. Retire diagnostic overhead by build/type choice, with emitted-code
   checks; retain the information needed to prove protection operation.

## Acceptance discipline

One change at a time from the tagged baseline. Preserve current limiting,
sharp and slow sag protection, nFAULT, tracking, late-arm/blanking stops,
watchdog, host abort, and dominant bridge-off behavior. No threshold relaxation
bundled with scheduling changes. No sibling-project edits.

For each candidate: predeclare mechanism and falsifiable prediction; run host
tests/replay, release/LTO/clippy and four-root assembly audits; inspect changed
paths and interrupt blocking. Record source/configuration/ELF identity and
before/after evidence in the notebook. Review whether the evidence actually
supports the conclusion before choosing the next change. Retain failed runs;
do not retry until passing or turn a counter into independent rotor truth.

Use bounded loaded 60% regression screens before longer qualification; compare
effective commutation timing, late arms, guard latency, speed and current.
Protection provocations must still demonstrate bridge-off on the candidate.
Diagnostic results do not qualify a different image. No claim of zero regression
from one run, and no claim of thermal safety without temperature evidence:
the current motor is boxed with restricted airflow and has no thermal sensor.

Success is a simpler, shorter critical path with retained protection coverage
and re-earned loaded performance—not fewer guards, a larger throttle number,
or architectural similarity asserted from source alone.
