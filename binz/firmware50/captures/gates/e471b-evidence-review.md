**No phase-1 counterexample follows from these paths, given your stated predicate that comparator resume permits only phases 0/4.** That predicate’s implementation is absent from the packet, so this is a conditional source proof.

- **Startup:** `drv_end()` masks and clears COMP before `det_install()` activates detection. Installation does not reopen COMP. `com_handover()` arms phase 1 under PRIMASK while COMP remains masked. Phase-1 COM advances both steps before reopening COMP or scheduling blanking. Because COM outranks COMP, COMP cannot execute until that COM invocation returns.
- **Normal entry:** COMP enters through a reopened line in phase 0/4. Entry masks it. A phase-4 COM preemption before masking completes can perform optional observation work, but cannot advance the step. After masking, phase 4 parks. Neither path creates a phase-1 commutation.
- **Before the acceptance mask:** There is consequently no outstanding phase-1 event that can change `step` between its load at line 495 and `W::run` at line 503. Acceptance publishes and arms phase 1 inside the masked window. A refusal creates no phase-1 arm.

**COM after the arm is reachable and distinct.** Once `W::run` restores PRIMASK, phase-1 COM may preempt the remaining bookkeeping, advance the sector, and reopen or blank COMP. The suspended COMP continues using its old local snapshot for accounting and `finish_accept`; it does not perform another acceptance with that snapshot. COMP cannot preempt itself.

This establishes the requested exclusion subject to the resume predicate and supplied paths. It does not resolve pending-guard delay.
