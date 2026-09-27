Both runs stopped with reason=8 and track_fault=1 after only 3–4 ms closed loop; neither reached hold. Poststop telemetry reports outputs disabled. This establishes a tracking stop, not its physical cause or rotor lock.

The earlier 1035 µs raw acceptance gap is consistent with staleness, but raw-entry timestamps are not guard timestamps. The supplied evidence does not establish the exact watchdog event sequence or fault subtype. Zero-valued witness data cannot establish absent BEMF; coast transitions do not validate driven synchronization.

The candidate provides no at-speed optimization evidence. Different binaries and requested duties prevent a controlled comparison; the shared applied_ccr=133 also cautions against treating requested duty as delivered exposure. Earlier failure does not exonerate the shortened persistence aperture.

Retiring that aperture change and investigating post-filter bookkeeping is a sensible bounded offline direction. Audit every accept_seq writer, interrupt/preemption context, reader, and ordering dependency before replacing fetch_add. Then inspect generated code and test sequence publication, wrapping, and acceptance-to-arm behavior. Require demonstrated cycle savings while preserving persistence timing and guard thresholds. This produces reviewable evidence without another uninformative powered retry; it does not resolve startup’s physical cause.
