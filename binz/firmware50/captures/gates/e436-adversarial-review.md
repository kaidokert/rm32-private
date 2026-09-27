61793 / 3.371070 s / 6 = **3055.06 eHz** across the recorded tail, versus 3086 from rounded mean-sector timing. Coast 3045 is 99.67% of that tail rate; inverse matching is 1003.31‰, so reported 1004‰ needs its denominator clarified. The tail spans 3.371 s despite `window_us=2000000`; it is not a verified two-second terminal window.

At ci=40, `(40>>1)-(40>>2)=10 µs`; spent=10 exhausts that budget. Guard15/LateArm explains the software shutdown, conditional on the omitted helper/reason mappings. It does **not** identify the physical cause of short intervals or processing delay. Zero forced events and similar coast frequency do not establish clean crossing detection.

The arm path improves safety: validation, timer preparation, elapsed sampling, and enable share PRIMASK; preparation disables the source before clearing pending state. Expiry invokes shutdown synchronously. However, `guard_trip`, helper arithmetic, and caller handling of `Some(spent)` remain unaudited. Poststamp instructions and ISR latency still delay commutation; zero margin histograms provide no bound.

Next bounded lever: inspect generated instructions between elapsed sampling and CEN, plus shutdown/caller control flow. Precompute only deadline-independent work if supported; preserve guards and duty.
