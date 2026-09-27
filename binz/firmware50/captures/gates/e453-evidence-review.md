Raw arithmetic checks:

- Stale age: 11,527,950 − 11,527,707 = 243 µs; excess = 3 µs.
- Conditional same-epoch mapping: 49,965 − 49,939 = 26 µs; inferred entry = 11,527,941, 234 µs after prior watch feed, 9 µs before decision.
- Sector and phase totals both equal 83,440; adding 4,323 coalesced accepts gives 87,763. Coverage reconciles numerically, but those histograms omit coalesced events.
- 87,763 / 6.813 ≈ 12,881/s. Applied 306/1333 ≈ 22.96%.

No target hold is demonstrated: hold duration, accepts, current blocks, and tail coverage are zero. The 1-µs/166,666-Hz hold fields are unusable; zero margin bins provide no margin evidence. `worst_hold_ma=865` cannot establish hold coverage.

The recorded failure supports event-path staleness; sag and late-arm counters report no trips. However, **“service gap” is too causal**: 243 µs measures watch-feed timing, not isolated service latency or physical-edge spacing. The limit is tightened before `event`, potentially affecting the decision.

Poststop fields are not fault-time evidence; the mapping requires unproved epoch/event correspondence. Nine microseconds includes intervening acceptance work, not necessarily reducible prearm cost. E446’s comparison is unverifiable here. The proposed offline review is reasonable; its optimization premise remains unproved.
