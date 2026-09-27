The raw evidence supports an accepted-event watchdog expiry: `from_event=1`, age 243 µs versus limit 240 µs. Sag did not trip; no late arms were recorded. It does not establish why the accepted-event interval exceeded its limit. Zero hold time also makes the reported hold-rate values unusable.

The author’s 234 µs entry / 9 µs subsequent-work reconstruction is conditional. Poststop interrupt masking gives a coherent snapshot at collection; it does not establish fault-time correspondence between the clock pair, acceptance fields, and saved stale decision. Same-epoch mapping needs explicit justification.

Protection semantics matter: `guard_event` tightens the limit before calling `event`. Without those implementations, we cannot determine whether the arriving acceptance retroactively tightens the preceding interval, or whether a late acceptance must trip despite already publishing and arming commutation.

The watch borrow excludes the guard locally. It does not establish exclusion across acceptance publication, timer arming, and `finish_accept`. Inspect that entire priority/masking path and whether a pending guard can intervene.

Another tiny optimization is weakly discriminating: saving several microseconds could suppress this threshold crossing without identifying its cause. First establish fault-time timestamps, the applicable previous/new limit, and intervening preemption. An offline cycle reduction alone cannot distinguish service latency from changed acceptance behavior.
