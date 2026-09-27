Raw evidence supports a latched software Stale and safe poststop outputs; it does not establish the physical missed-crossing cause or shutdown latency. Coast/hold rates agree approximately; zero witness values and empty margin histograms provide no corroboration.

The proposal stays minimal: fault-branch-only timestamp/origin capture, unchanged precedence, poststop reporting, and assembly verification. Capture last, deadline, and first-Stale time coherently; distinguish detection age from lateness beyond the deadline. Label subsequent telemetry explicitly poststop. Replace the report’s default-watch fallback with “unavailable” to avoid fabricated healthy evidence.

First-fault preservation holds within EventWatch; guard_trip’s load-then-store needs proven exclusion across all callers or atomic first-writer arbitration. Verify resumed COMP/COM and handover paths cannot rearm after shutdown. Tracking arming also needs a latched-stop check.

Shutdown bounds require maximum interrupt masking/preemption, poll delay, and trip-to-MOE-off duration; observed 111 µs guard gaps are not worst-case bounds. Clock extension requires servicing within one raw-counter wrap.

Also test rejected tightening: a 238 µs minimum prevents the advertised 200 µs ceiling. No powered admission is justified yet.
