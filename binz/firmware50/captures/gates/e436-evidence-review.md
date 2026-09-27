CCR 333/1333 = 24.981%. Tail 61,793/(3.371070 s × 6) = **3055.06 eHz**; its span exceeds the advertised 2-second window. Full-hold 61,794/(3.371 s × 6) = 3055.18 eHz. Rounded mean-sector 54 µs yields 3086.42 eHz. Coast 3045 is 99.67% of tail rate; printed `matched_rate_permille=1004` needs its actual formula.

Terminal evidence supports deadline exhaustion: fixed-16 wait at CI 40 is (40>>1)−(40>>2)=10 µs, matching spent=10. Assuming the stated helper semantics, remaining time is zero, selecting `stop_expired_arm()` and LateArm. This explains the **guard trigger**, not the physical cause of CI shortening or time consumption. No sag trip, tracking fault, or forced commutations are reported; endpoint nFAULT is high.

Arm sequencing looks defensively ordered: validation, timer/source disable, flag/pending cleanup, elapsed sampling, then stop-or-enable under one interrupt mask. However, shutdown implementation and helper definitions are absent; complete safety is unproven. Poststamp instructions and ISR latency still delay commutation. Zero margin counters provide no latency bound.

Next bounded step: inspect helpers, shutdown, and generated pre-sample instructions offline before selecting an optimization. Endpoint OFF checks support shutdown state, not transient safety.
