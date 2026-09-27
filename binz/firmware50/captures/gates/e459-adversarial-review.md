Raw E458 establishes case1’s missing request, rearm, and sampled off readbacks. It does not identify the rejection cause; no captured-input row is supplied.

- Capture records the values actually passed to `admit`, but hardware reads are sequential. Masking interrupts does not freeze comparator level or pending state. This is an admission-input record, not a simultaneous hardware snapshot.
- Stores occur after `admit`, so they cannot change that invocation’s admission result. They extend exclusion before `observe`, pend, and arm; instrumentation can also shift earlier execution. E459 cannot establish E458’s historical cause or unchanged timing.
- Entry/exit off checks cannot prove continuous off. The shown path contains no direct gate-enable operation, but that conclusion depends on the omitted helper contracts and other interrupt paths.
- `bits=0` means capture absent; other input fields may be stale.

No concrete blocker to one off-only `t` emerges from the supplied code under those helper contracts.
