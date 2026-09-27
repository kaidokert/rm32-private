No definite scheduler defect is evident, but the evidence supports conditional equivalence, not a complete proof.

- **Missing premise:** `Budget` is absent. Replacing `reserve(...)` with `live_admitted` requires proving its validity limits match, cancellation is permanent, and every reachable slot deadline satisfies the budget deadline. `used <= slot` establishes quota safety; alone it does not establish deadline equivalence.
- **No revival is observation-dependent:** an observed authority mismatch permanently exhausts this instance. A stop/acceptance followed by restored matching authority between observations is invisible. Atomic owner invalidation, generation discipline, and obsolete-IRQ clearing remain essential integration obligations.
- **Five slots do not bound callbacks:** arbitrarily many early observations return another delay without consuming a slot. The bound applies to due observations and requests. Timer rearming and duplicate interrupts need separate validation.
- **Tests:** broad differential coverage helps, but the oracle shares slot-transition arithmetic, allowing common defects. Independent assertions should establish strictly future delays, slot progress, and terminal permanence.
- **Cost:** the combined emitted change supports a 380-cycle model reduction only. Missing fetch/peripheral/preemption costs prevent a runtime bound.

Proceed with off-only validation; it cannot establish energized commutation timing or protection behavior.
