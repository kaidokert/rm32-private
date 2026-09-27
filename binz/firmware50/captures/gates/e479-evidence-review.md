Raw evidence: E476 stopped with `reason=8`, `track_fault=1`: stale age 1029 µs exceeded 1000 by 29. Seven accepts/commutations, zero forced, zero late arms, zero hold; sag did not trip. However, poststop clock/accept raw stamps differ by only 21 ticks, versus the stale event’s 1029 µs. Reconcile that discrepancy before treating this as uninterrupted crossing loss. It does not establish masking causality.

The proposal is conditionally sound, not yet demonstrated. Exclusive borrowing freezes estimator state, not comparator polarity, sector ownership, or shutdown state. Qualification-to-mask remains interruptible; arm refusal alone does not prevent stale accepted-state publication.

Necessary tests:

- Differential old/new replay: both estimate policies, both polarities, gate−1/gate/gate+1; every dissent position at depths 0–255. Assert identical outcomes, full estimator/history/counters, and exact reads: zero gated, j+1 dissent, depth accepted.
- Assert independent blend/clamp/advance/wait arithmetic identities, including odd intervals, rounding boundaries, extrema, advances 0–64, fixed/runtime equivalence, and previous-estimate first acceptance.
- Verify cached depth initialization/refresh across every mapped/shallow boundary, refusals, expired/refused arms, and reinstall.
- Compile-fail token forgery, reuse, and estimator mutation while borrowed.
- Inject guard/COM at qualification→mask and every commit boundary; require shutdown dominance, coherent sector/publication, nested PRIMASK restoration, and no poststop rearm/resume. Include a deliberately broken control.
