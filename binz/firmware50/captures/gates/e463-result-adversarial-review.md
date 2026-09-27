- **Budget enforcement misses the new work.** `comp_root_scheduled` computes `elapsed` and checks `handler_overrun` before `comp_resume_powered()` and `resume_after_refusal()`. The added authority reads, PRIMASK section and TIM16 pend are excluded from `call_max_us` and overrun enforcement. Thus those measurements cannot bound the complete refusal path.

- **Stop evidence is decoded incorrectly.** `stop_expired_arm()` trips `Reason::LateArm` (documented code 15), but `reason_from_code()` has no 15 arm: it reports `UnknownGuard`. Shutdown still occurs; the reported cause loses fidelity.

- **The handshake model assumes the peripheral behavior requiring validation.** Its single `pending` boolean collapses UIF, NVIC pending and active state. Sequential `wake()` calls do not exercise interrupt assertion during acknowledgment or timer replacement. The existing `crossing_selftest_off()` masks TIM16 and never executes this handshake.

Required disabled test: with ENABLE low and MOE clear, exercise actual TIM16/COMP dispatch at the selected priorities. Force expiry during masked COMP, refusal before/after expiry, repeated wakes, acceptance replacement, and stop with a wake pending. Verify one observation, no premature replacement-timer dispatch, and no activity after stop; record UIF/CEN/NVIC/phase transitions.

Recomputed slots for average 80 are **41, 121, 161, 201, 241 µs**. The supplied audit cannot establish race correctness.
