**Outcome: no concrete blocker to the proposed bounded screen is demonstrated by these excerpts.** This is limited support, not production qualification.

- Recomputing the visible predicates: all four rows satisfy their case-specific expectations, with two calls, one valid park, and `off=1`. Case 10’s 1004 µs meets 1000–1100 µs. `38 = active + timer-running + off`; `33 = stopped + off`, with active, timer-running, NVIC-pending, and update-pending clear.
- Case 12’s phase4/armed1 records the earlier callback. BEFORE33 records shutdown before final cleanup; those snapshots are consistent. The overrun check happens **after** wake preemption, so it demonstrates eventual shutdown, not prevention of work after the budget is exceeded.
- Synthetic acceptance/refusal and a substituted TIM16 handler do not establish production commutation behavior, actual BEMF persistence, continuous bridge-off behavior, or WCET.
- The fast return plausibly removes optional work from the masked interval; it does not establish the cause or resolution of the previous fault.

A concrete blocker would be frozen image `63F48607` lacking the stated priority ordering or protections. Their presence, test/audit results, and image identity remain supplied assertions here.
