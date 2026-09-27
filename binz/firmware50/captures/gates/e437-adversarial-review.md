RAW: 0x8000912..928 reloads the comparator each iteration, masks bit 30, normalizes it with SUBS/SBCS, recomputes expected polarity with NEGS/ADCS, and checks the counter. This supports the proposed optimization; it does not establish equivalent physical filtering.

Proceed offline. No concrete semantic flaw is apparent provided:

- `level_word()` performs exactly one volatile read of the same register and returns only the comparator mask; expectedword is exactly zero or that mask.
- The predicate preserves polarity, read count/order, first-mismatch refusal, and all detector-state updates.
- The wrapper preserves existing behavior, including any supported zero-depth case.

Exhaustive injected sequences can establish unchanged **digital test behavior**. They cannot establish unchanged **physical aperture**: faster reads compress the persistence window, potentially accepting shorter disturbances and changing acceptance and scheduling times.

The supplied run is baseline evidence, not validation of the optimized binary or the 80% target. Its zero margin counters establish no timing margin.

Keep protections unchanged. Assembly verification and tests justify further review; they do not authorize flashing, bench escalation, or relaxed thresholds.
