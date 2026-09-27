Raw evidence: E466 records one late arm (`ci_at_late=45`, `spent_at_late=11`), only 11 ms at target, and guard reason 15. E469 records 20,278 ms at 150 with no late arms. Both identify the same ELF and pass poststop checks. This does not isolate preemption as the cause or establish candidate safety.

The proposed window is reasonable for offline investigation, but no candidate implementation or emitted code is supplied, so correctness remains unproven.

Required obligations:

- Preserve persistence reads, Boolean evaluation, filter depth, estimator updates, publication order, and elapsed subtraction including timer preparation. Identical source logic alone does not establish identical instruction cadence.
- Restore the **previous PRIMASK**, including nested critical sections, before margin, trace, counters, watch, and filter-cache work on every return path.
- Bound the entire masked path, including worst-case persistence and expired-arm shutdown. Establish guard/DMA latency bounds; 109 µs observed guard gaps do not prove margin against 200 µs.
- Audit pending COM dispatch during acceptance publication and immediately after unmask. A pending guard must stop an armed timer; subsequent bookkeeping must neither revive outputs nor overwrite first-fault evidence.
- Ensure default wrappers instantiate `Open`, the separate binary selects `Masked`, and unsupported logged combinations fail compilation.
- Tie assembly audits and bridge-disabled tests to the exact candidate artifact.

Offline work only; these captures justify no envelope increase.
