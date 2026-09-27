The shown deferred-resume bypass is fixed: guard/driven checks, pending clear, shared permission check, enable, and conditional pend remain inside one critical section. Detector ownership takes precedence, preventing driven activity from overriding closed-loop blanking. `drv_begin` establishes driven ownership before arming; the shown boundary caller supports the intended acquisition path.

**Concrete remaining defects:** None demonstrated by the supplied evidence.

**Unproven general properties:** The caller excerpt does not establish that *all* `comp_arm` callers are acquisition-only. `comp_exti_arm` still selects edges and clears pending outside the helper’s mask; safety depends on that ownership restriction.

The 390 passing tests support the policy and textual routing checks. The race simulation assumes shutdown masks the line; it does not execute production interleavings. Text searches cannot establish global writer exclusivity. ISR equality establishes neither absolute latency nor hardware behavior; the candidate remains unflashed.
