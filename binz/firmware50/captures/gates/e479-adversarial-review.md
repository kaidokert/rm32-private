Raw evidence: E476 stopped after seven accepts, with `hold_ms=0`, tracking age 1029 µs against 1000 µs, and zero recorded preemptions. It neither establishes a masking cause nor validates commit safety.

**Concrete blocker in the proposal: qualification carries no stated sector-generation authorization.** An exclusive estimator borrow protects Rust access; it does not freeze hardware sector, comparator selection, phase, or stop state. COMP admitted in phase 0/4 can qualify, be interrupted by COM, then commit against another sector. Consuming the token once prevents duplication, not stale authorization. Masking only commit cannot repair this.

The raw path also snapshots `start` and `step` before `W::run`. Even masking persistence through arm therefore needs entry validation.

The estimator borrow must exclude every interrupting estimator accessor throughout qualification and commit. COM reading snapshots helps, but the supplied excerpt cannot establish that for all interrupt paths.

Latched-stop dominance survives **if** commit uses the existing atomic arm check and never clears the latch. However, `guard_event` currently executes after arm: if it rejects this acceptance, a short timer could fire first. PRIMASK also defers pending guard service; that is distinct from honoring an already latched stop.

Require generation revalidation before estimator mutation and rejection-capable guards before enabling. High-duty qualification is unnecessary for bounded low-duty exploration.
