Established: failed 25% propless exploration; 80% was not exercised. `333/1333 ≈ 24.98%`; `applied_cap=800` does not establish 80% operation. Tracking is flagged, but the specific fault and terminal sequence remain unresolved.

Recomputed:

- Hold: `17388/0.976 = 17815.57` accepts/s → **2969.26 eHz**, assuming six accepts/cycle.
- Tail: `(17386−1)/0.976687 = 17799.97` intervals/s → **2966.66 eHz**. Its span is only 0.977 s, despite the 2 s window.
- Rounded `mean_sector_us=56` implies **2976.19 eHz**, explaining the different reported estimate.
- First 32 coast intervals total **5439 µs**: **2941.72 eHz**, assuming two transitions/cycle. The fitted 2949 cannot be independently reconstructed here.
- Sector and phase totals both equal **110416**; adding **6927 coalesced accepts** exactly recovers **117343**. Hold exceeds tail by two accepts; boundary accounting is unspecified.

Hypotheses: missing/rejected crossings, scheduling delay, and monitor-observation effects remain distinguishable possibilities. Whole-run minima/maxima cannot establish their terminal order. Zero late arms does not exclude other delays; absent preemption counters prove nothing. Current carries −310 mA zero drift. Hash labels and excerpts cannot verify unchanged deployed guards.

Minimum discriminator: an existing terminal accepted-event/commutation trace, if available, correlated with latched `EventWatch::fault`, `last`, and deadline. Startup rows and margin histograms cannot supply terminal chronology. Existing trace availability is unestablished; no higher run is justified.
