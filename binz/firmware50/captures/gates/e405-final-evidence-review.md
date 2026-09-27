No remaining E405 gap shown here blocks the proposed bounded powered screening. This closes the narrow review, not hardware qualification or a formal timing proof.

- **TIM16 handover:** supplied production source explicitly unpends and unmasks TIM16 after `guard_arm_tracking()`.
- **Arm containment:** validation, preparation, timestamp, expiry decision and timer start share one interrupt-masked closure. The five passing checks include mutations rejecting an empty critical section and comment-only operations. They are a narrow source recognizer, not a Rust proof.
- **Expired-arm shutdown:** source calls `guard_trip(Reason::LateArm)` synchronously under that mask. The supplied F6FAAC7A callee emits shutdown writes directly, without polling or further calls. Final BF725A29 root identity and clean four-root audit support preservation of the reviewed IRQ paths; the terminal disassembly itself is labeled F6FAAC7A.
- **Preemption:** configurable COM priority cannot permit COM execution inside the PRIMASK-protected arm sequence.
- **Wrap limitation:** bounded wait does not establish bounded elapsed time. The corrected argument depends on bounded masked execution and priority-zero tracking stopping stale work before arm validation. The 24µs figure is an engineering estimate, not WCET certification; debugger/peripheral stalls exceeding a clock wrap remain outside that contract.
- **Regression evidence:** 365 tests pass; Clippy finishes successfully. The incremental-cache access warning concerns reuse on the next build. The reported poststamp path falls from 50 to 30 instructions; this establishes no measured cycle gain.

Proceed with the named candidate retaining the old ratchet: flash, native TIM16 off-smoke checks at 2/10/40, then the specified bounded screening sequence, with 60 conditional on clean 50, ≥120s OFF, PSU 3A and unchanged guards. Do not promote historical baseline results into a new A/B or qualification claim.
