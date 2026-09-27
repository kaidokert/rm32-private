Offline-only disposition: reasonable experiment, but correctness remains unproved; no candidate implementation or emitted assembly is supplied.

- **Bound the entire masked path.** Include worst-case persistence iterations, estimator/publication, nested timer preparation, and expired-arm shutdown—not merely successful acceptance. Prove guard and DMA blocking bounds against their deadlines; 109 µs observed guard gap supplies no WCET margin.
- **Restore the incoming PRIMASK**, not unconditional interrupt enable, on every return/refusal path. Assembly must establish restoration before margin, trace, counters, watch, and filter-cache work. Include compiler movement across the boundary.
- **Preserve cadence and semantics.** Same Boolean expression/read count does not prove identical comparator sampling cadence. Compare emitted persistence instructions and spacing, estimator updates, depth selection, wait arithmetic, and entry-relative elapsed accounting. Returning compact statistics can itself change register allocation and spills.
- **Exercise pending-event interleavings offline.** Cover guard pending before/during the window, COM expiry during persistence, acceptance replacing a pending observation, and refusal restoring service. Verify stale pending removal cannot discard a required commutation. After shutdown, resumed bookkeeping must neither rearm timers nor reopen the comparator.
- **Enforce policy selection structurally.** Default wrappers must instantiate `Open`; unsupported logged/`Masked` combinations must fail compilation in actual builds.

E466 demonstrates a late-arm stop after only 11 ms at target; E469 is a different-duty control. Neither establishes preemption causality or validates this candidate.
