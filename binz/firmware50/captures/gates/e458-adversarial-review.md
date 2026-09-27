**No concrete energization or ownership blocker is demonstrated for flashing this standalone off-only probe.** That verdict does not qualify powered operation.

- **Off-only safety:** The shown path never enables the bridge, changes gate roles, or invokes the commutating COM root. Both normal completion and timeout call `guard_trip`. This depends on the omitted `com_arm`, `com_stop`, and initialization helpers honoring their contracts.
- **Synthetic cleanup:** `guard_trip` revokes synthetic authority by clearing active flags and latching a reason. It does not clear `STATE.schedule`; cases 0, 1, and 5 can leave one behind. That is residual model state, not surviving drive authority: another preparation overwrites it, and an unpowered callback clears it.
- **Concurrency:** `line_enable` briefly unmasks COMP inside interrupt exclusion, then COMP is masked before exclusion ends. That is not a dispatch window. Guard can preempt outside the callback’s inner critical section and invalidate a test, but the shown shutdown cannot restore drive authority. The preparation prechecks outside exclusion are not proof of safe handover with real owners; this standalone program supplies no such activation path.
- **Timing and coverage:** `ticks125ns` measures the bracketed call, including its internal masking and possible preemption—not full ISR latency or WCET. CEN readback proves neither expiry timing nor repeat dispatch. Observations/retirements are printed, not asserted.

Missing validation: helper-contract inspection, timeout cleanup, timer expiry, and comparator-noise sensitivity.
