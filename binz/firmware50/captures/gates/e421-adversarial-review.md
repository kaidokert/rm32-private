Established: this failed at 25%, with 976 ms target hold; it provides no 80% qualification.

- Hold: 17,388/0.976 = **17,815.6 accepts/s**, or **2,969.3 eHz** assuming six accepts/cycle. Reported 2,976 derives from rounded 56 µs.
- Tail: 17,386/0.976687 = **17,801.0 accepts/s**, or **2,966.8 eHz**. Its two-count difference from hold needs window-boundary reconciliation.
- Sector counts sum to **110,416**; adding **6,927 coalesced accepts** exactly recovers **117,343**. Phase bins also total 110,416. Those distributions omit coalesced events and may selectively miss busy intervals.

Coast agreement supports approximate terminal speed, not correct powered crossing identity or phase. Tracking subtype and terminal chronology remain unestablished. Missing crossings, rejected crossings, delayed servicing, and mailbox/order effects remain competing hypotheses. Whole-run `thin=81`, `ci_min=40`, and `fast_min=22` cannot locate the cause.

Safety claims exceed evidence: report-only fast events do not enforce rotor validity; tightening below `min_interval` is refused. Poststop PASS establishes sampled shutdown state, not shutdown latency. Current drift undermines current-margin inference.

Minimum next discriminator: use the existing decision-sequence capture/replay facility referenced in the source, retaining terminal offers/rejections/accepts. Correlate existing arm/commutation timestamps and watchdog subtype if available; otherwise acknowledge that replay alone cannot distinguish scheduling delay. Rejections before silence implicate acceptance; timely acceptance followed by late commutation implicates scheduling. Missing offers require independent edge evidence. Check capture observer effects; absent preemption counters prove nothing. Keep guards unchanged; no higher rung.
