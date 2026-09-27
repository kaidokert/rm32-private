Minimal scope is appropriate, but this establishes software Stale provenance only.

- Capture first-Stale `now` and poll/event origin exactly when latching; retained `last` gives observed age. Preserve strict `>` and Stale-before-sector precedence. With `min_interval=238`, tightening to 200 is rejected even in report-only mode; `track_max_us=240` deserves explicit reconciliation.
- `EventWatch` preserves its first fault under serialized access. Global `guard_trip` uses load-then-store, so first-reason preservation requires proven exclusion across every caller or compare-exchange. A latch check before handover also needs exclusion through release; otherwise shutdown can be undone.
- POSTSTOP PASS proves eventual off, not bounded shutdown latency. Bound watchdog scheduling/masking delay plus trip-to-MOE-off execution, including `com_stop`; show pending/resumed handlers cannot rearm. Observed `gap_max_us=111` is no worst-case bound.
- A critical section makes raw/ext coherent, but cannot recover multiple missed 16-bit wraps. Label poststop fields accordingly; replace silent watch-lock fallback with explicit unavailable telemetry.
- Keep fault-only capture, assembly/root audits, and host tests. Add concurrency/shutdown evidence before powered admission; avoid expanding into physical-cause instrumentation.
