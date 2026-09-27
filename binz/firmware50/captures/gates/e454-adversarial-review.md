Same COM timer reuse is defensible conditionally; these excerpts do not establish safe ownership or stop dominance. E453 records a stale-event stop, but loop_gap=221 µs versus watch=240 µs does not establish its cause.

- **Ownership:** Add an explicit recheck phase, subordinate to accepted commutation and blanking. Physical COMP acceptance must atomically invalidate the recheck, clear stale timer status/pending IRQ, and install the commutation deadline. Otherwise an old expiry can dispatch the new phase prematurely. Phase alone is insufficient.
- **Physical COMP race:** Interrupt masking does not freeze comparator hardware. Recheck expiry should request ordinary COMP qualification, never accept or commutate directly. Coalesce physical/software requests; identify them by sector generation, not repeating step number.
- **Stop dominance:** Guard can preempt COM after its initial `active` check. On resumption, COM must not apply gates, unmask COMP, or rearm. Every enabling transaction needs stop revalidation under exclusion covering guard, with stop invalidating generation and pending work.
- **Budget:** Preserve one initial attempt plus four rescues, charged once per actual request. Repeated expiry, qualification failure, or coalesced notifications must not replenish quota. Snapshot timing consistently.
- **Starvation/coverage:** Bound timer and COMP work; exhaustion leaves recovery to existing guard deadlines. Higher-priority guard still suffers interrupt-masked latency.
- **Filtering:** Timer observation is no crossing timestamp. Retain live gate and persistence qualification; prevent software pending from bypassing filter timing.

Simplest viable design: one generation-tagged recheck state in the existing timer dispatcher, with accepted COM always superseding it.
