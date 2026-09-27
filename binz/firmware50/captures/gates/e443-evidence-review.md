Candidate held target for 2368 ms versus 3371 ms: 1003 ms shorter (29.8%). Late arms fell from 1 to 0; maximum recomputation time rose from 11 to 12 µs. These single runs establish neither reliability improvement nor a demonstrated cost benefit.

`track_fault=1` maps explicitly to `Stale`. The supplied watchdog latches this when elapsed time since its last observed accepted event exceeds its tightened limit, reported as 240 µs. The guard then requests `Tracking`. This supports a software watchdog stop; it does not identify the physical cause or establish slip.

The final captured acceptance was 303 µs before stop, numerically compatible with that deadline, but clock alignment, capture semantics, and terminal execution order remain unproven. Aggregate maxima cannot reconstruct that sequence.

Accepted-event rate implies 3034 eHz, versus baseline 3055 eHz; candidate coast reports 2982 eHz, approximately 1.0175× agreement. Neither certifies terminal rotor tracking.

Next: audit existing captures and complete event-feed/stop paths offline, reconstructing last watchdog update, deadline tightening, stale latch, and output disable. Avoid perturbing ORDER instrumentation until that evidence is exhausted.
