The foreground fix closes the shown stale-admission race **against maskable guard interrupts**, provided admission reads the authoritative stop state. ISR sameness does not establish unchanged response latency: these critical sections delay guard service.

**Concrete blocker: startup rearm compatibility is not demonstrated.** The test manually sets `latched = false` and `active = true`; it never exercises `guard_arm()`. The supplied implementation clears `reason` and sets `active`, but `guard_latched()` is missing. Therefore neither successful rearm nor a persistent-latch failure is established. Before accepting startup compatibility, trace the actual latch predicate, its reset, and startup ordering through a previous stop. Also establish that guard interrupts cannot observe partially reset state during the unmasked `guard_arm()` sequence.

Remaining limits:

- Zero compares establish register values, not electrically disabled complementary outputs; safety depends on MOE/ENABLE and startup ordering.
- String matching does not prove executable wrapper coverage.
- The reported mask durations are conditional estimates, not hardware worst-case bounds.

No demonstrated residual shutdown-overwrite defect appears in the three guarded wrappers. Proceed only with disabled verification; motor readiness remains unproven.
