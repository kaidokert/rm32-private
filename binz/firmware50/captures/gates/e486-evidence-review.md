Raw evidence: E485 stopped on Tracking (`reason=8`) after 4 ms closed, seven accepts, zero target hold. Staleness was `4718548−4717512=1036 µs`, exceeding 1000 by 36 µs. `applied_ccr=133` at period 1333 indicates approximately 10% applied duty, not the requested 15%. This capture does not test E486.

The supplied implementation supports the candidate configuration:

- `Production` retains default `FixedCarrier`; only the candidate selects `SlowEntry<2666,150>`.
- Handover installs 2666 ticks; reaching 150 tenths requests 1333, retained through foldback. At 64 MHz these are approximately 24.006/48.012 kHz; target CCR is `floor(1333×150/1000)=199`.
- Startup remains 6400 ticks; driven command remains 200 eHz, or 833 µs per sector. Compile-time assertions require advance 16.
- Retiming checks stopped/active ownership and MOE, stages CCR/ARR under UDIS, publishes plans, then releases UDIS. Refusal routes through `CarrierTransition` and safe-off. Atomicity depends on the omitted lock implementation.
- Boot checks exactly 2666→1333 with MOE clear and zero compares. This verifies disabled timing, not energized switching.

Reported evidence: 417 tests passed; ISR audit passed; instruction sequences unchanged. These do not independently bind the supplied SHA to the source.

**Conditional screen-only acceptance:** exact-image disabled preflight, then one `9`; require observed ≥19 s target hold. Preserve reverse/propless, confirmed 3 A, ≥120 s OFF, and fault-ended batch without retry. No qualification or cause claim.