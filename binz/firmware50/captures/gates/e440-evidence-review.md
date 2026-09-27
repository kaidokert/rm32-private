Established: the candidate stopped with `reason=8`, `track_fault=1`, six accepted crossings, 3 ms closed loop, and zero hold. Poststop telemetry reports outputs disabled. This provides no sustained-speed optimization evidence. The earlier image also failed, but different binaries and requested duties prevent attributing—or exonerating—the candidate change.

Unknown: the physical cause of lost tracking. The supplied excerpts do not decode `track_fault=1`. The earlier 1,035 µs raw-entry gap is consistent with stale tracking, but guard timestamps differ; it does not prove the precise watchdog timeline, especially for the candidate. Zero-valued witness channels cannot establish absent BEMF. No reported sag trip or interrupt storm isolates the cause.

The bounded offline direction is sound: retire the unqualified persistence-aperture change, restore baseline sampling, and audit post-filter/pre-arm bookkeeping. Before changing `accept_seq.fetch_add`, enumerate writers, interrupt/preemption contexts, readers, and ordering requirements; single ownership must be demonstrated. Compare generated instructions and timing paths, then run relevant offline regressions.

This avoids another premature powered retry while developing a reviewable change. It neither resolves startup failure nor authorizes powered testing; guard thresholds remain unchanged.
