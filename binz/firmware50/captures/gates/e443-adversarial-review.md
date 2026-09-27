Candidate held target for 2,368 ms versus 3,371 ms: 1,003 ms shorter (29.8%). Accepted-event rate fell 0.68% (18,331→18,206/s); current proxy rose 16.6% (338→394 mA), with substantial zero drift limiting interpretation.

`track_fault=1` maps explicitly to Stale: the watch observed elapsed time strictly greater than its 240 µs ceiling. The last recorded acceptance 303 µs before stop is consistent with that software diagnosis, but does not establish terminal chronology or the physical cause. The supplied snippets omit the `guard_event` body and numeric mapping for reason 8. Baseline DONE=28 versus GUARD=15 also needs reconciliation.

Zero recorded late arms versus one, and spent_max=12 versus 11 µs, establish neither benefit nor reliability improvement. Shorter exposure, one run per image, and the compiler branch change preclude attribution to the counter edit.

Coast transitions support continued rotation afterward; they do not establish powered rotor lock or slip. Poststop checks establish the sampled disabled state.

Next: audit existing capture timestamps, watch updates, interrupt ordering, and stop-record precedence offline. If unavailable, report chronology unresolved; another isolated optimization is unjustified.
