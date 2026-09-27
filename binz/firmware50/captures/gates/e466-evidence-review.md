Raw evidence supports an exhausted arm budget, not a proven underlying cause.

- At ci=45, level16: wait=(45>>1)−floor(45/4)=22−11=11µs; spent=11 leaves zero. This matches late_arms=1. Guard15/top28 mapping and FreshEstimate selection require omitted caller/enum evidence.
- Closed time is 7.512s; target hold only 11ms, not the requested 28s. Hold rate: 202/0.011=18,364 accepts/s.
- Sector and phase totals both equal 64,060; adding 33,881 coalesced accepts gives 97,941 exactly. zc_acc and com_count are each two higher; their accounting needs explanation.
- Recheck deltas: +8,647 requests, +150,855 observations, zero retirements. These are activity counts, not demonstrated rescues.
- Tail: 11,519−126=11,393µs. If spanning first-to-last acceptance, 196 accepts represent 195 intervals: ≈2,853eHz, ≈992‰ of 2,876; using 196 yields the reported ≈997‰.

Maxima do not identify the expensive path; zero margin histograms provide no distribution. Prior 15% success is not supplied here.

Next: inspect existing experiments and actual arm callers; isolate preparation cost versus estimate contraction before selecting a timing change. No blind repeat, escalation, or threshold relaxation.
