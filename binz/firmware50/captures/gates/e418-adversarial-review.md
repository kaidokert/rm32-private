Raw: PREFLIGHT reports outputs off and nFAULT high; DIODESELFTEST reports PASS. The latter is a cached boot result printed on `p`, not a fresh pad measurement. Neither establishes powered transition safety.

One concrete unresolved blocker: COMG transfers roles immediately, but CCRs transfer at the next native update. UDIS prevents partial CCR updates; it does not make roles and compares atomic together. New roles can therefore operate with old sector CCRs for part of a carrier period. The pad test waits before sampling and misses precisely that interval. Establish that every reachable transition, including startup handoff, remains electrically safe during this mismatch.

Concurrency: the masked, stop-checked COM transaction is sensible. Verify that pending stop handling cannot be followed by any separate MOE/ENABLE rearm path; this excerpt does not establish the claimed permanent stop latch.

Once those narrow points are resolved, the bounded 15% screen is reasonable. Record startup, target ACK, stop reason and final outputs. Success demonstrates this attempt’s operation; absence of faults does not exercise guard-trip behavior.
