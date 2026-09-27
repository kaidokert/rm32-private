**No concrete blocker is demonstrated for the proposed single 15% propless screen.** This supports an experiment, not candidate qualification.

- **Validation before mutation:** The successful qualification path checks stopped/active/detector, phase and matching sector under PRIMASK before estimator/history/acceptance publication or timer preparation. Assembly places those checks before the first acceptance stores. Refusal counters and the separate oversized-count rebase remain outside this guarantee.
- **Persistence-to-mask gap:** An interrupt can occur after the final comparator read. A serviced stop or sector change is rejected by the masked validation; elapsed time is included in the arm calculation, with expiry causing shutdown. There is no fresh polarity check after masking, so qualification can age across a preemption. That is a residual sampling limitation, not a demonstrated blocker here.
- **No new reenable:** The shown commit path neither clears the stop latch nor enables bridge outputs. Nested arm masking preserves the outer mask; expired-arm shutdown completes before restoration.
- **Priority budget:** PRIMASK newly delays higher-priority protection and commutation service. The 15.953 µs conditional model establishes neither hardware WCET nor pending-guard latency. This is the main screen risk; the supplied evidence does not establish a violated protection deadline.
- **Existing limitation:** `guard_event` remains after arming and unmasking; commutation can precede that check. Atomic commit does not fix this.

E476’s seven accepts, zero hold and tracking stop provide no candidate endurance evidence. Keep the stated current limit, cooldown and fault-ends-batch scope.
