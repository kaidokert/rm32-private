Raw evidence:

- Host log reports 416 passing tests, plus an incremental-cache access warning.
- `powered_write` places admission and the write inside `interrupt::free`; plan, MOE-on, and nonzero compare writes use it. Zero compares bypass it.
- Tests encode stop-before/after, stale-precheck failure, and synthetic rearm. Actual `guard_arm` clears `reason` and sets `active`; the excerpt shows no separate latch reset.
- Six reported mask spans have conditional estimates of 0.969–6.203 µs, including two with loops.
- Audit reports four clean roots. ISR comparison reports identical instruction streams; ADC_COMP/TIM16 encoded bytes differ.
- Candidate869B7E4E remains unflashed.

Interpretation: **Ready to proceed with disabled-only verification; motor admission remains unsupported.** Source supports the intended CPU exclusion arrangement, but model atomicity is assumed and structural checks are textual. Startup coverage does not establish actual zero-write/rearm ordering or latch semantics: `guard_latched` and `arm_allowed` definitions are absent.

Mask estimates are not measured WCET; ISR summaries do not establish relocated-constant equivalence. Remaining motor-admission limits include artifact identity, compiled exclusion semantics, interrupt latency, peripheral preload/output behavior, and physical stop/restart behavior. NMI coverage is explicitly excluded.
