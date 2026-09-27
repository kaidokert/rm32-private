One remaining assertion gap: `handshake::interrupt()` checks the second dispatch’s `expired` witness only for cases 10/11. Cases 9/12 can pass if refusal incorrectly rearms TIM16 and a hardware expiry causes the second callback instead of the intended software wake. Require `!expired` on their second dispatch, ideally also verify the timer is stopped before `after_phase()`.

The corrected snapshots are consistent: `33` means stopped/off with active, CEN, NVIC pending and UIF clear; `38` means active/running/off with stopped and pending flags clear. Pending injection and cancellation checks occur under PRIMASK, before case 11 reinjection.

No bridge-enabling path is visible in this probe. Its bounded wait and binary-specific audit exception are appropriately scoped. Case 12 verifies the refusal-tail shutdown helper, not the complete production COMP overrun path.
