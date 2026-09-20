# DRV observer replay

Host harness for the same allocation-free detector intended for shell-pwm.
It consumes the decoded `_obs.csv` from `scripts/drv_observation.py` and calls
the actual sibling minz-core polarity and BEMF counter functions. It neither
flashes nor opens UART. The same module is linked into live shell-pwm as an
observe-only detector (Entry031). For v7 captures the harness asserts exact
agreement with all four recorded decision bits and the recorded reason on
every sample; any mismatch fails the process.

```text
cargo test --manifest-path tools/observer-replay/Cargo.toml --target x86_64-pc-windows-msvc
cargo check --manifest-path tools/observer-replay/Cargo.toml --lib --target thumbv6m-none-eabi
cargo run --manifest-path tools/observer-replay/Cargo.toml --target x86_64-pc-windows-msvc -- captures/bemf_dense_01_obs.csv
```

The explicit host target matters: binz's parent Cargo configuration otherwise
selects ARM and a hardware runner. Use `--lib` for the M0 compile-only check;
the CSV executable needs std. Cargo.lock pins harness dependencies separately.

Detector rules: reset on sector change; invalid input clears baseline/filter
support; two opposite samples arm; minz bemf_count_step with bad threshold0
requires two expected samples; report at most one candidate per sector. No
scheduler or motor-output interface exists. Before support can survive a brief
expected-level spike; this online candidate policy is less strict than the
offline full-sector audit, which rejects visits containing multiple transitions.

Every input has one explicit reason: invalid, needs baseline, opposite,
accumulating, candidate, already latched. Printed candidate ages are confirmation
times, not interpolated zero-cross times. Two-sample thresholds are bench
evidence settings, not validated control tuning. Candidate counts do not grant
permission for closed-loop commutation or a duty increase.

## Integrated reference sequence (Entry 035)

`tests/sequence.rs` now connects the actual minz-core polling, COMP, COM,
TIM6 and main-loop scheduling/recovery bands through a logical-time HAL.
`tests/support/mod.rs` adapts the frozen reference's owned-atomic stores and
call-recording mock, adding elapsed half-us time, comparator masking, pending
bits, COM deadlines and timestamped phase changes. No sibling source edits.

Eight tests exercise synthetic 200/250 eHz crossing trains, polling-to-interrupt
changeover, blocking polling wait, persistence rejection, pending-bit blanking,
mode hysteresis, desync, missing-edge timeout/re-kick, and pending-COM cancellation
on safety kill. The full TIM6 test also calls the duty pipeline and healthy
synthetic ADC harvest. Four existing detector tests remain unchanged.

Important boundaries:

- These are synthetic control-sequence regressions, not physical motor-lock
  evidence or replay of captured comparator edges. Constant-period stimulus
  has no mechanical dynamics and is independent of the controller's estimate.
- COM defaults to immediate ARR activation, overflow at ARR+1. A selectable
  preload model now retains the active period until overflow and has a seeded
  regression matched to Entry 036 G071 measurements. The reference uses ARPE
  without UG: do not claim default sequence results model that adapter verbatim.
  Free-running masked overflows, preload phase before arm and register-write
  costs remain outside this model. Interval count is 16-bit; test time is extended.
- Comparator persistence reads consume scripted levels, not measured M0 time.
  Camp-at-gate re-entry is explicitly dispatched, not an NVIC interrupt-storm
  model. Main-band cadence, priorities and preemption are not yet qualified.
- ADC inputs/calibration in the TIM6 test are reference-shaped, NOT DRV current
  protection. All-off is a recorded HAL request, not measured ENABLE/MOE safing.
- At 200 eHz, missing crossings halt COM while average_interval and the desync
  count can remain unchanged. The reference later re-kicks at >45000 half-us
  ticks. A fresh-event/tracking abort is required before envelope expansion.

Run `--check-source` from captures/reference/README before these tests. The
existing CSV executable still implements v7 detector replay only; it does not
  silently claim to feed sparse samples through this full sequence.

## Post-level feedback regression (Entry 081)

`post_level_after_every_com_can_accelerate_without_freshness_fault` supplies
the expected comparator level immediately after every synthetic commutation,
then services the real reference blank/filter/COM/main-loop sequence. There
is no independent periodic rotor stimulus. The fresh-COM seed matches the
bench prefix convention; execution has zero-cost persistence reads, unlike
the measured M0 ISR. This is an adversarial sequence test, not captured motor
replay and not proof of the physical cause of the bench's early acceptance.

Across twelve accepts, average_interval falls from1666 to583 half-us ticks;
the first inter-COM gap is782 ticks and the last312. A2500us accepted-event
freshness watchdog stays satisfied throughout. The reference desync guard
first trips at COM12, clears running and returns to polling. Thus freshness
and isolated prefix acceptance cannot qualify rotor tracking. The regression
asserts the eventual reference desync as well as the preceding acceleration;
it does not claim desync never detects the problem. No production guard or
minz-core algorithm is changed.

`accepted_timing` (Entry084) is an M0-compatible external event-envelope
monitor: individual acquisition timestamps, logical sector order, minimum
interval and maximum age. It is not linked to the live motor firmware yet.
Reference EV_ACC timestamps from the mock test both the clean120-event train
and the post-level feedback sequence. Test-only bounds1333..2000half-us ticks
reject the latter at its second event; they are not qualified hardware limits.
First event only seeds phase, faults latch, and regular polling is required
when events disappear. In-band false events can still pass: no lock claim.

## All-phase coast sensitivity (Entry087)

`tests/support/coast_trace.rs`, included in sequence tests, feeds measured
Entry086 bridge-disabled comparator samples through actual reference handlers.
Uses all96 individually timestamped reads from the32rows with COASTCOMP data.
All six seed sectors are reported. Three yield14..16COM requests; three return
to polling after4..5. This is sample-and-hold sensitivity, not continuous-edge
replay: persistence reads hold one level, IRQ service is at sample cadence,
and mux changes between samples are unresolved. COM deadlines remain ordered.
No measured motor output follows virtual COM, so it makes no torque/lock claim.
Run with `cargo test --test sequence coast_three -- --nocapture` plus the
explicit manifest and host-target arguments above.
