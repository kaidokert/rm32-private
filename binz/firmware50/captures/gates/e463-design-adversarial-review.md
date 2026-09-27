Raw: `after_phase` currently executes the full masked scheduler even when COMP has masked IMR18. Generation invalidation protects publication only after `accept_seq` changes; it does not eliminate scheduler interference earlier in acceptance. Also, `com_arm` does not clear stale TIM16 NVIC pending; crossing preparation does. The one-shot model omits that distinction.

The proposed handshake preserves ownership and bounded liveness, provided:

- COM checks phase4/IMR immediately after acknowledgment and active validation, before logging, clock accounting, or scheduler work.
- Every ordinary closed-loop refusal invokes the typed helper after successful `comp_resume_powered()`. Both operations must remain ordered; the helper revalidates everything under PRIMASK.
- Wakeup only pends TIM16; COM retains exclusive schedule mutation.

Concrete interleavings: expiry during masked COMP parks phase4; refusal resumes it. Refusal before expiry sees CEN1 and needs no wake. Expiry between resume and helper lets COM progress first; helper must tolerate the resulting phase/timer state. Acceptance clears any parked wake through `prepare_crossing`; stop clears it through `com_stop`.

Shortest discriminating test: expire phase4 immediately after COMP masks IMR; assert zero scheduler observations, phase4/CEN0 retained; force refusal and assert eventual COM observation. Repeat with acceptance and stop: neither may dispatch obsolete phase4 work. Include hardware/NVIC pending state, not merely the truth table.
